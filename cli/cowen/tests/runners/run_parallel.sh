#!/bin/bash
# Cowen CLI Parallel Test Runner (Hybrid Mode for 100% Stability)

# 确保脚本在 bash 下运行
if [ -z "$BASH_VERSION" ]; then
    exec bash "$0" "$@"
fi

# Load common utilities if available
[ -f tests/e2e/scripts/common.sh ] && source tests/e2e/scripts/common.sh

# 🚀 All-in-One: Start in-container databases if running inside Podman/Docker
if [ -f /.dockerenv ] || [ -f /run/.containerenv ]; then
    if [ -f tests/infra/start_services.sh ]; then
        source tests/infra/start_services.sh
    fi
fi

# Basic colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[0;33m'
BOLD='\033[1m'
NC='\033[0m'

set +e

echo -e "${BLUE}${BOLD}========================================================${NC}"
echo -e "${BLUE}${BOLD}   Cowen CLI Hybrid Verification Suite (Stable)         ${NC}"
echo -e "${BLUE}${BOLD}========================================================${NC}"

# Configuration
MAX_PARALLEL="${MAX_PARALLEL:-32}"
TEST_BASE="${TEST_BASE:-target/cowen_tests}"
if [[ "$TEST_BASE" != /* ]]; then
    TEST_BASE="$(pwd)/$TEST_BASE"
fi
RESULTS_DIR="$TEST_BASE/results"
BASE_PORT_START="${BASE_PORT_START:-16000}"

final_parallel_cleanup() {
    if [ "$CLEANUP_DONE" == "true" ]; then return; fi
    CLEANUP_DONE="true"
    echo -e "\n${BLUE}🧹 Performing final cleanup...${NC}"
    
    # Try to cleanup workspaces if helper exists
    if command -v cleanup_all_workspaces >/dev/null 2>&1; then
        cleanup_all_workspaces
    fi
    
    rm -rf "$TEST_BASE"/.cowen_test_job_*
    if [ "${FAILED_COUNT:-0}" -eq 0 ] && [ "$KEEP_TEST_ENV" != "true" ]; then
        # rm -rf "$RESULTS_DIR"
        echo -e "${GREEN}✨ All temporary files cleared.${NC}"
    else
        echo -e "${YELLOW}⚠️  Failing logs preserved in $RESULTS_DIR.${NC}"
    fi
}
trap "final_parallel_cleanup" EXIT
pkill -9 cowen-test >/dev/null 2>&1 || true
pkill -9 cowen-daemon >/dev/null 2>&1 || true

# --- Initialization ---
echo -e "${BLUE}🧹 Cleaning up previous test artifacts in $TEST_BASE...${NC}"
rm -rf "$TEST_BASE"
mkdir -p "$RESULTS_DIR/tmp_scripts"
cp tests/e2e/scripts/common.sh "$RESULTS_DIR/tmp_scripts/"
cp tests/e2e/scripts/verify-binary.sh "$RESULTS_DIR/tmp_scripts/" 2>/dev/null || true


echo -n "  Building cowen binary and plugins (release)..."
export COWEN_BUILD_CLIENT_ID="dummy-parallel-client-id"
BUILD_ARGS="--release"

# Respect CARGO_TARGET_DIR if set
TARGET_BASE=${CARGO_TARGET_DIR:-target}
BINARY_PATH="$TARGET_BASE/release/cowen"

if [ -f /.dockerenv ] || [ -f /run/.containerenv ]; then
    BUILD_ARGS="--release --target x86_64-unknown-linux-gnu"
    BINARY_PATH="$TARGET_BASE/x86_64-unknown-linux-gnu/release/cowen"
fi

if cargo build --quiet $BUILD_ARGS -p cowen -p cowen-daemon -p cowen-search-embedding -p cowen-signer 2>/dev/null; then
    echo -e " ${GREEN}[OK]${NC}"
    export COWEN_BIN="$(pwd)/$BINARY_PATH"
    # Force common.sh to refresh its SOURCE_BIN
    if [ -f tests/e2e/scripts/common.sh ]; then
        # We need to make sure common.sh uses the same COWEN_BIN
        update_source_bin
    fi
else
    echo -e " ${RED}[FAILED]${NC}"
    exit 1
fi

# 🔌 🔌 ENSURE SEARCH PLUGINS ARE SIGNED FOR TESTS
PLUGIN_NAME="libcowen_search_embedding"
LOCAL_OS_TYPE=${OS_TYPE:-$(uname -s)}
if [[ "$LOCAL_OS_TYPE" == *"MINGW"* || "$LOCAL_OS_TYPE" == *"MSYS"* || "$LOCAL_OS_TYPE" == *"CYGWIN"* ]]; then PLUGIN_NAME="libcowen_search_embedding.exe"; fi

BUILD_DIR="$(dirname "$BINARY_PATH")"
PLUGIN_SRC="$BUILD_DIR/$PLUGIN_NAME"

# If the plugin was built and we have dev keys, sign it so E2E tests pass PKI validation
if [ -f "$PLUGIN_SRC" ] && [ -f "dist_assets/keys/official_dev.pk8" ]; then
    cargo run --quiet $BUILD_ARGS -p cowen-signer -- sign-plugin \
        --dylib "$PLUGIN_SRC" \
        --name cowen-search-embedding \
        --version "0.4.0" \
        --dev-key dist_assets/keys/official_dev.pk8 \
        --dev-cert dist_assets/keys/official_dev_cert.json \
        --out-bundle "$BUILD_DIR/libcowen_search_embedding.bundle" \
        --capabilities SearchProvider \
        --required-privileges LocalCacheAccess,ModelAssetFetch,ComputeHeavy
fi

cp "$BINARY_PATH" "$(dirname "$BINARY_PATH")/cowen-test"
export COWEN_BIN="$(pwd)/$(dirname "$BINARY_PATH")/cowen-test"

# Collect suites (All suites are parallelized)
if [ $# -gt 0 ]; then
    PARALLEL_SUITES=("$@")
else
    PARALLEL_SUITES=($(ls tests/e2e/scripts/case_*.sh 2>/dev/null))
fi
SEQUENTIAL_SUITES=()

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

    export TEST_BASE="$workspace"
    export COWEN_HOME="$workspace"
    export MOCK_PORT=$mock_port
    export COWEN_PORT_RANGE_START=$((mock_port + 10))
    
    bash "$suite" > "$log_file" 2>&1
    
    local exit_code=$?
    if [ $exit_code -eq 0 ]; then
        echo -e "  [JOB $job_id] ${GREEN}✅ $(basename "$suite") PASSED${NC}"
    else
        echo -e "  [JOB $job_id] ${RED}❌ $(basename "$suite") FAILED${NC}"
    fi

    # Bulletproof process teardown: kill all daemons belonging to this job's isolated workspace
    pkill -9 -f "cowen_job_${job_id}_" >/dev/null 2>&1 || true
    pkill -9 cowen-daemon >/dev/null 2>&1 || true

    return $exit_code
}

# --- Phase 1: Parallel ---
started_count=0
FAILED_COUNT=0
TOTAL_PARALLEL=${#PARALLEL_SUITES[@]}

if [ "$TOTAL_PARALLEL" -gt 0 ]; then
    echo -e "\n${BOLD}Phase 1: Running Parallel Suites ($TOTAL_PARALLEL)${NC}"
    for suite in "${PARALLEL_SUITES[@]}"; do
        # Extract case ID from filename (e.g., case_01 -> 1)
        case_id=$(basename "$suite" | cut -d'_' -f2 | sed 's/^0//')
        
        base_port=$((BASE_PORT_START + case_id * 50))
        tmp_suite="$RESULTS_DIR/tmp_scripts/$(basename "$suite").$case_id"
        cp "$suite" "$tmp_suite"
        
        for p in 29101 9909 9908 9903 9902 9901 9128 9127 9126 9122 9112 9111 9101 9098 9097 9096 9095 9094 9093 9092 9091 9299 8080 6387 6382 6381 6380 6379; do
            new_p=$((base_port + (p % 100)))
            [ "$p" -eq 9299 ] && new_p=$base_port
            perl -pi -e "s/\b${p}\b/${new_p}/g" "$tmp_suite"
        done
        
        perl -pi -e "s/\.cowen_test_/.cowen_test_job_${case_id}_/g" "$tmp_suite"
        
        run_job "$tmp_suite" "$case_id" "$base_port" &
        started_count=$((started_count + 1))
        
        # 🚀 Fix: Staggered start to reduce DB contention
        sleep 0.2
        
        [ $((started_count % MAX_PARALLEL)) -eq 0 ] && wait
    done
fi
wait

# --- Phase 2: Sequential (Heavy DB Suites) ---
TOTAL_SEQ=${#SEQUENTIAL_SUITES[@]}
if [ "$TOTAL_SEQ" -gt 0 ]; then
    echo -e "\n${BOLD}Phase 2: Running Sequential Suites ($TOTAL_SEQ)${NC}"
    for suite in "${SEQUENTIAL_SUITES[@]}"; do
        # Extract case ID from filename
        case_id=$(basename "$suite" | cut -d'_' -f2 | sed 's/^0//')
        
        base_port=$((BASE_PORT_START + case_id * 50))
        tmp_suite="$RESULTS_DIR/tmp_scripts/$(basename "$suite").$case_id"
        cp "$suite" "$tmp_suite"
        
        # Still remap ports to avoid collisions with any leftover background tasks
        for p in 29101 9909 9908 9903 9902 9901 9128 9127 9126 9122 9112 9111 9101 9098 9097 9096 9095 9094 9093 9092 9091 9299 8080 6387 6382 6381 6380 6379; do
            new_p=$((base_port + (p % 100)))
            [ "$p" -eq 9299 ] && new_p=$base_port
            perl -pi -e "s/\b${p}\b/${new_p}/g" "$tmp_suite"
        done
        
        perl -pi -e "s/\.cowen_test_/.cowen_test_job_${case_id}_/g" "$tmp_suite"
        
        run_job "$tmp_suite" "$case_id" "$base_port"
    done
fi

# Summary Analysis
echo -e "\n${BLUE}${BOLD}========================================================${NC}"
# Use find to avoid globbing issues if no files exist
while IFS= read -r log; do
    [ -z "$log" ] && continue
    # Clean ANSI codes and check for success keywords
    if perl -pe 's/\e\[[0-9;]*m//g' "$log" | grep -Eiq "Passed!|Successful!"; then
        continue
    else
        FAILED_COUNT=$((FAILED_COUNT + 1))
        echo -e "  ${RED}FAILED:${NC} Job log $log"
        tail -n 5 "$log" | sed 's/^/      /'
    fi
done < <(find "$RESULTS_DIR" -name "job_*.log" 2>/dev/null)

if [ "$FAILED_COUNT" -eq 0 ]; then
    echo -e "${GREEN}${BOLD}✅ ALL SUITES PASSED!${NC}"
    exit 0
else
    echo -e "${RED}${BOLD}❌ $FAILED_COUNT SUITES FAILED${NC}"
    exit 1
fi
