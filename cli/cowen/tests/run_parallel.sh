#!/bin/bash
# Cowen CLI Parallel Test Runner (Final Robust Version)
# All artifacts are stored in target/cowen_tests

source tests/common.sh
# Disable set -e to handle failures gracefully in parallel
set +e

echo -e "${BLUE}${BOLD}========================================================${NC}"
echo -e "${BLUE}${BOLD}   Cowen CLI Parallel Verification Suite               ${NC}"
echo -e "${BLUE}${BOLD}========================================================${NC}"

# Configuration
MAX_PARALLEL=4
if [[ "$OSTYPE" == "darwin"* ]]; then
    MAX_PARALLEL=$(sysctl -n hw.ncpu)
fi
[ $MAX_PARALLEL -gt 8 ] && MAX_PARALLEL=8

TEST_BASE="target/cowen_tests"
RESULTS_DIR="$TEST_BASE/results"

final_parallel_cleanup() {
    # Only run once
    if [ "$CLEANUP_DONE" == "true" ]; then return; fi
    CLEANUP_DONE="true"

    echo -e "\n${BLUE}🧹 Performing final cleanup...${NC}"
    
    # 1. Remove logs and temp scripts
    rm -f "$TEST_BASE"/mock_server_*.log
    rm -f tests/*.bak
    
    # 2. Always cleanup the large workspace directories to keep env clean
    rm -rf "$TEST_BASE"/.cowen_test_*
    rm -f "$TEST_BASE"/.cowen_test_*.db "$TEST_BASE"/.cowen_test_*.db-shm "$TEST_BASE"/.cowen_test_*.db-wal
    
    # 3. Handle results directory
    # If failed_count was calculated by the summary part
    if [ "${FAILED_COUNT:-0}" -eq 0 ] && [ "$KEEP_TEST_ENV" != "true" ]; then
        rm -rf "$RESULTS_DIR"
        echo -e "${GREEN}✨ All temporary files cleared.${NC}"
    else
        echo -e "${YELLOW}⚠️  Failing logs preserved in $RESULTS_DIR for analysis.${NC}"
        [ "$KEEP_TEST_ENV" == "true" ] && echo -e "${YELLOW}⚠️  Results directory preserved as requested.${NC}"
    fi
}

# Ensure we cleanup on exit no matter what
trap "final_parallel_cleanup" EXIT

# Kill any orphan cowen processes from previous runs
echo -n "  Cleaning up orphan processes..."
pkill -9 cowen >/dev/null 2>&1 || true
echo -e " ${GREEN}[OK]${NC}"

# Ensure target dir exists
mkdir -p "$RESULTS_DIR/tmp_scripts"

# Build latest binary once
echo -n "  Building cowen binary..."
if cargo build --quiet 2>/dev/null; then
    echo -e " ${GREEN}[OK]${NC}"
else
    echo -e " ${RED}[FAILED]${NC}"
    exit 1
fi

SUITES=(
    "tests/case_01_self_built.sh"
    "tests/case_02_store_app.sh"
    "tests/case_03_oauth2.sh"
    "tests/case_04_migration.sh"
    "tests/case_05_proxy_interception.sh"
    "tests/case_06_webhook_forwarding.sh"
    "tests/case_07_token_lifecycle.sh"
    "tests/case_08_concurrent_stress.sh"
    "tests/case_09_dlq_retries.sh"
    "tests/case_10_profile_management.sh"
    "tests/case_11_reconnect_resilience.sh"
    "tests/case_12_daemon_recovery.sh"
    "tests/case_13_distributed_lb.sh"
    "tests/case_14_shared_storage.sh"
    "tests/case_15_store_app_shared_storage.sh"
    "tests/case_16_migration_block.sh"
)

echo -e "  Concurrency: $MAX_PARALLEL"
echo -e "  Total Suites: ${#SUITES[@]}"

# Disable global mock management - each job gets its own
export COWEN_MOCK_MANAGED="false"

run_job() {
    local suite=$1
    local job_id=$2
    local mock_port=$3
    local log_file="$RESULTS_DIR/job_${job_id}.log"
    
    # Run with isolated mock port
    MOCK_PORT=$mock_port bash "$suite" > "$log_file" 2>&1
    local exit_code=$?
    
    if [ $exit_code -eq 0 ]; then
        echo -e "  [JOB $job_id] ${GREEN}✅ $suite PASSED${NC}"
    else
        echo -e "  [JOB $job_id] ${RED}❌ $suite FAILED${NC}"
    fi
    return $exit_code
}

# Simple parallel execution loop for Bash 3.2
current_jobs=0
job_id=0
FAILED_COUNT=0

for suite in "${SUITES[@]}"; do
    mock_port=$((10000 + job_id))
    offset=$((job_id * 20))
    
    # Create an isolated version of the script with unique ports
    tmp_suite="$RESULTS_DIR/tmp_scripts/$(basename "$suite").$job_id"
    cp "$suite" "$tmp_suite"
    
    # On macOS, sed -i requires an extension argument
    sed -i.bak "s/9299/$mock_port/g" "$tmp_suite"
    sed -i.bak "s/8080/$((18080 + job_id))/g" "$tmp_suite"
    sed -i.bak "s/9091/$((20001 + offset))/g" "$tmp_suite"
    sed -i.bak "s/9092/$((20002 + offset))/g" "$tmp_suite"
    sed -i.bak "s/9093/$((20003 + offset))/g" "$tmp_suite"
    sed -i.bak "s/9094/$((20004 + offset))/g" "$tmp_suite"
    sed -i.bak "s/9095/$((20005 + offset))/g" "$tmp_suite"
    sed -i.bak "s/9096/$((20006 + offset))/g" "$tmp_suite"
    sed -i.bak "s/9901/$((30001 + offset))/g" "$tmp_suite"
    sed -i.bak "s/9902/$((30002 + offset))/g" "$tmp_suite"
    sed -i.bak "s/9903/$((30003 + offset))/g" "$tmp_suite"
    sed -i.bak "s/9908/$((30008 + offset))/g" "$tmp_suite"
    sed -i.bak "s/9909/$((30009 + offset))/g" "$tmp_suite"
    
    run_job "$tmp_suite" "$job_id" "$mock_port" &
    
    job_id=$((job_id + 1))
    current_jobs=$((current_jobs + 1))
    
    if [ $current_jobs -ge $MAX_PARALLEL ]; then
        wait
        current_jobs=0
    fi
done

wait

# Summary Analysis
echo -e "\n${BLUE}${BOLD}========================================================${NC}"
for log in "$RESULTS_DIR"/job_*.log; do
    # Skip if log doesn't exist (should not happen but safety first)
    [ ! -f "$log" ] && continue
    
    if perl -pe 's/\e\[[0-9;]*m//g' "$log" | grep -Eiq "Passed!|Successful!"; then
        continue
    else
        FAILED_COUNT=$((FAILED_COUNT + 1))
        # Try to find the suite path from the log
        # In our case, the suite path is always passed as $1 to run_job which is in the log header
        # Actually, let's just use the log filename as a hint
        echo -e "  ${RED}FAILED:${NC} Job log $log"
        tail -n 10 "$log" | sed 's/^/      /'
    fi
done

if [ $FAILED_COUNT -eq 0 ]; then
    echo -e "${GREEN}${BOLD}✅ ALL PARALLEL SUITES PASSED!${NC}"
    exit 0
else
    echo -e "${RED}${BOLD}❌ $FAILED_COUNT SUITES FAILED${NC}"
    exit 1
fi
