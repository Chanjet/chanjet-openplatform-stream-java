#!/bin/bash
# Cowen CLI Parallel Test Runner (Hybrid Mode for 100% Stability)
# 1. Parallel execute 25 standard suites.
# 2. Serial execute high-IO/conflict-prone suites (18, 20).

source tests/common.sh
set +e

echo -e "${BLUE}${BOLD}========================================================${NC}"
echo -e "${BLUE}${BOLD}   Cowen CLI Hybrid Verification Suite (Stable)         ${NC}"
echo -e "${BLUE}${BOLD}========================================================${NC}"

# Configuration
MAX_PARALLEL=8
TEST_BASE="${TEST_BASE:-target/cowen_tests}"
RESULTS_DIR="$TEST_BASE/results"
BASE_PORT_START="${BASE_PORT_START:-16000}"

final_parallel_cleanup() {
    if [ "$CLEANUP_DONE" == "true" ]; then return; fi
    CLEANUP_DONE="true"
    echo -e "\n${BLUE}🧹 Performing final cleanup...${NC}"
    rm -rf "$TEST_BASE"/.cowen_test_job_*
    if [ "${FAILED_COUNT:-0}" -eq 0 ] && [ "$KEEP_TEST_ENV" != "true" ]; then
        rm -rf "$RESULTS_DIR"
        echo -e "${GREEN}✨ All temporary files cleared.${NC}"
    else
        echo -e "${YELLOW}⚠️  Failing logs preserved in $RESULTS_DIR.${NC}"
    fi
}
trap "final_parallel_cleanup" EXIT
pkill -9 cowen >/dev/null 2>&1 || true

# --- Initialization ---
echo -e "${BLUE}🧹 Cleaning up previous test artifacts in $TEST_BASE...${NC}"
rm -rf "$TEST_BASE"
mkdir -p "$RESULTS_DIR/tmp_scripts"

echo -n "  Building cowen binary..."
if cargo build --quiet 2>/dev/null; then
    echo -e " ${GREEN}[OK]${NC}"
else
    echo -e " ${RED}[FAILED]${NC}"
    exit 1
fi

# Group 1: Reliable Parallel Suites
PARALLEL_SUITES=(
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
    "tests/case_17_redis_shared_storage.sh"
    "tests/case_19_ticket_auto_resend.sh"
    "tests/case_21_openapi_whitelist.sh"
    "tests/case_22_dlq_manual_retry.sh"
    "tests/case_24_completion.sh"
    "tests/case_25_status_all.sh"
    "tests/case_26_cluster_idempotency.sh"
    "tests/case_27_hybrid_data_drift.sh"
    "tests/case_28_store_app_multi_org_stress.sh"
    "tests/case_29_sidecar_startup.sh"
    "tests/case_18_redis_fault_tolerance.sh"
    "tests/case_20_oauth2_refresh.sh"
    "tests/case_30_sidecar_scaling_stress.sh"
    "tests/case_31_sidecar_self_built_stress.sh"
    "tests/case_32_mysql_shared_storage.sh"
    "tests/case_33_postgres_shared_storage.sh"
    "tests/case_34_exclusive_connection.sh"
    "tests/case_40_init_cleanup.sh"
)

# Group 2: Sensitive Serial Suites (Empty for now, all moved to parallel)
SERIAL_SUITES=(
)

run_job() {
    local suite=$1
    local job_id=$2
    local mock_port=$3
    local log_file="$RESULTS_DIR/job_${job_id}.log"
    local workspace="$TEST_BASE/.cowen_test_job_${job_id}"
    
    mkdir -p "$workspace"
    cat > "$workspace/app.yaml" <<EOF
storage:
  store: sqlite
  db_url: "sqlite://$workspace/cowen_job_${job_id}.db"
log:
  level: debug
telemetry_enabled: false
ai_enabled: false
EOF

    export COWEN_HOME="$workspace"
    export MOCK_PORT=$mock_port
    
    bash "$suite" > "$log_file" 2>&1
    
    local exit_code=$?
    if [ $exit_code -eq 0 ]; then
        echo -e "  [JOB $job_id] ${GREEN}✅ $(basename $suite) PASSED${NC}"
    else
        echo -e "  [JOB $job_id] ${RED}❌ $(basename $suite) FAILED${NC}"
    fi
    return $exit_code
}

# --- Phase 1: Parallel ---
job_id=0
FAILED_COUNT=0

if [ ${#PARALLEL_SUITES[@]} -gt 0 ]; then
    echo -e "\n${BOLD}Phase 1: Running Parallel Suites (${#PARALLEL_SUITES[@]})${NC}"
    for suite in "${PARALLEL_SUITES[@]}"; do
    base_port=$((BASE_PORT_START + job_id * 50))
    tmp_suite="$RESULTS_DIR/tmp_scripts/$(basename "$suite").$job_id"
    cp "$suite" "$tmp_suite"
    
    for p in 29101 9909 9908 9903 9902 9901 9128 9127 9126 9122 9112 9111 9101 9098 9097 9096 9095 9094 9093 9092 9091 9299 8080 6387 6382 6381 6380 6379; do
        new_p=$((base_port + (p % 100)))
        [ $p -eq 9299 ] && new_p=$base_port
        perl -pi -e "s/\b${p}\b/${new_p}/g" "$tmp_suite"
    done
    
    # 路径隔离：将所有 .cowen_test_ 替换为带有 Job ID 的唯一路径
    perl -pi -e "s/\.cowen_test_/.cowen_test_job_${job_id}_/g" "$tmp_suite"
    
    run_job "$tmp_suite" "$job_id" "$base_port" &
    job_id=$((job_id + 1))
    [ $((job_id % MAX_PARALLEL)) -eq 0 ] && wait
    done
fi
wait

# --- Phase 2: Serial ---
if [ ${#SERIAL_SUITES[@]} -gt 0 ]; then
    echo -e "\n${BOLD}Phase 2: Running Serial Suites (${#SERIAL_SUITES[@]})${NC}"
    for suite in "${SERIAL_SUITES[@]}"; do
        run_job "$suite" "$job_id" "19999"
        job_id=$((job_id + 1))
    done
fi

# Summary Analysis
echo -e "\n${BLUE}${BOLD}========================================================${NC}"
for log in "$RESULTS_DIR"/job_*.log; do
    [ ! -f "$log" ] && continue
    if perl -pe 's/\e\[[0-9;]*m//g' "$log" | grep -Eiq "Passed!|Successful!"; then
        continue
    else
        FAILED_COUNT=$((FAILED_COUNT + 1))
        echo -e "  ${RED}FAILED:${NC} Job log $log"
        tail -n 10 "$log" | sed 's/^/      /'
    fi
done

if [ $FAILED_COUNT -eq 0 ]; then
    echo -e "${GREEN}${BOLD}✅ ALL SUITES PASSED!${NC}"
    exit 0
else
    echo -e "${RED}${BOLD}❌ $FAILED_COUNT SUITES FAILED${NC}"
    exit 1
fi
