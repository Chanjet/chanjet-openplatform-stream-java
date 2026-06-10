#!/bin/bash
# Cowen CLI Parallel Test Runner (Hybrid Mode for 100% Stability)

# 确保脚本在 bash 下运行
if [ -z "$BASH_VERSION" ]; then
    exec bash "$0" "$@"
fi

# Load common utilities if available
[ -f crates/app/cowen-cli/tests/e2e/scripts/common.sh ] && source crates/app/cowen-cli/tests/e2e/scripts/common.sh

# 🚀 All-in-One: Start in-container databases if running inside Podman/Docker
if [ -f /.dockerenv ] || [ -f /run/.containerenv ]; then
    if [ -f crates/app/cowen-cli/tests/infra/start_services.sh ]; then
        source crates/app/cowen-cli/tests/infra/start_services.sh
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
cp crates/app/cowen-cli/tests/e2e/scripts/common.sh "$RESULTS_DIR/tmp_scripts/"
cp crates/app/cowen-cli/tests/e2e/scripts/verify-binary.sh "$RESULTS_DIR/tmp_scripts/"  || true


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

if COWEN_BUILD_CLIENT_ID=dummy cargo build --quiet $BUILD_ARGS -p cowen-cli -p cowen-daemon -p cowen-search-embedding -p cowen-signer -p cowen-mcp-plugin; then
    echo -e " ${GREEN}[OK]${NC}"
    export COWEN_BIN="$(pwd)/$BINARY_PATH"
    # Force common.sh to refresh its SOURCE_BIN
    if [ -f crates/app/cowen-cli/tests/e2e/scripts/common.sh ]; then
        # We need to make sure common.sh uses the same COWEN_BIN
        update_source_bin
    fi
else
    echo -e " ${RED}[FAILED]${NC}"
    exit 1
fi

# 🔌 🔌 ENSURE SEARCH PLUGINS ARE SIGNED FOR TESTS
PLUGIN_NAME="libcowen_search_embedding"
MCP_PLUGIN_NAME="cowen-mcp-plugin"
LOCAL_OS_TYPE=${OS_TYPE:-$(uname -s)}
if [[ "$LOCAL_OS_TYPE" == *"MINGW"* || "$LOCAL_OS_TYPE" == *"MSYS"* || "$LOCAL_OS_TYPE" == *"CYGWIN"* ]]; then 
    PLUGIN_NAME="libcowen_search_embedding.exe"
    MCP_PLUGIN_NAME="cowen-mcp-plugin.exe"
fi

BUILD_DIR="$(dirname "$BINARY_PATH")"
PLUGIN_SRC="$BUILD_DIR/$PLUGIN_NAME"
MCP_PLUGIN_SRC="$BUILD_DIR/$MCP_PLUGIN_NAME"

# If the plugin was built and we have dev keys, sign it so E2E tests pass PKI validation
if [ -f "$PLUGIN_SRC" ] && [ -f "dist_assets/keys/official_dev.pk8" ]; then
    cargo run --quiet $BUILD_ARGS -p cowen-signer -- sign-plugin \
        --dylib "$PLUGIN_SRC" \
        --name cowen-search-embedding \
        --version "0.4.0" \
        --dev-key dist_assets/keys/official_dev.pk8 \
        --dev-cert dist_assets/keys/official_dev_cert.json \
        --out-bundle "$BUILD_DIR/libcowen_search_embedding.bundle" \
        --manifest-file crates/plugins/cowen-search-embedding/plugin.json
    echo "✅ Plugin signed and bundle generated: \"$BUILD_DIR/libcowen_search_embedding.bundle\""
fi

if [ -f "$MCP_PLUGIN_SRC" ] && [ -f "dist_assets/keys/official_dev.pk8" ]; then
    cargo run --quiet $BUILD_ARGS -p cowen-signer -- sign-plugin \
        --dylib "$MCP_PLUGIN_SRC" \
        --name cowen-mcp-plugin \
        --version "0.4.0" \
        --dev-key dist_assets/keys/official_dev.pk8 \
        --dev-cert dist_assets/keys/official_dev_cert.json \
        --out-bundle "$BUILD_DIR/cowen-mcp-plugin.bundle" \
        --manifest-file crates/plugins/cowen-mcp-plugin/plugin.json
    echo "✅ Plugin signed and bundle generated: \"$BUILD_DIR/cowen-mcp-plugin.bundle\""
fi

cp "$BINARY_PATH" "$(dirname "$BINARY_PATH")/cowen-test"
export COWEN_BIN="$(pwd)/$(dirname "$BINARY_PATH")/cowen-test"

# --- Suite Discovery & LPT (Longest Processing Time) Sorting ---
declare -a PARALLEL_SUITES
if [ $# -gt 0 ]; then
    PARALLEL_SUITES=("$@")
else
    # Hardcoded list of slow/heavy jobs based on CPU and Time (LPT Scheduling)
    # This maximizes concurrency utilization and prevents trailing stragglers.
    HEAVY_JOBS=(
        "case_13_distributed_lb.sh"
        "case_60_monitor_port_fallback.sh"
        "case_27_store_app_multi_org_stress.sh"
        "case_79_status_selfbuilt_heartbeat.sh"
        "case_09_dlq_retries.sh"
        "case_39_profile_rename_comprehensive.sh"
        "case_15_store_app_shared_storage.sh"
        "case_63_daemon_startup_optimization.sh"
        "case_18_redis_fault_tolerance.sh"
        "case_50_graceful_shutdown.sh"
        "case_14_shared_storage.sh"
        "case_52_dlq_paging.sh"
        "case_25_cluster_idempotency.sh"
        "case_46_robustness_check.sh"
        "case_20_oauth2_refresh.sh"
        "case_36_store_app_activation.sh"
        "case_19_ticket_auto_resend.sh"
        "case_29_sidecar_scaling_stress.sh"
        "case_30_sidecar_self_built_stress.sh"
        "case_26_hybrid_data_drift.sh"
        "case_53_chaos_stress.sh"
        "case_68_slow_ping_recovery.sh"
        "case_17_redis_shared_storage.sh"
        "case_33_exclusive_connection.sh"
    )
    
    for heavy in "${HEAVY_JOBS[@]}"; do
        suite_path="crates/app/cowen-cli/tests/e2e/scripts/$heavy"
        if [ -f "$suite_path" ]; then
            PARALLEL_SUITES+=("$suite_path")
        fi
    done
    
    for suite_path in crates/app/cowen-cli/tests/e2e/scripts/case_*.sh; do
        basename_suite=$(basename "$suite_path")
        is_heavy=false
        for heavy in "${HEAVY_JOBS[@]}"; do
            if [ "$heavy" == "$basename_suite" ]; then
                is_heavy=true
                break
            fi
        done
        if [ "$is_heavy" = false ]; then
            PARALLEL_SUITES+=("$suite_path")
        fi
    done
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
    
    local time_file="$workspace/cpu_time.txt"
    export TIMEFORMAT="%3R,%P"
    
    (time bash "$suite" > "$log_file" 2>&1) 2> "$time_file"
    local exit_code=$?
    
    local time_output=$(cat "$time_file" 2>/dev/null | tr -d '\n')
    local elapsed_ms=$(echo "$time_output" | cut -d',' -f1)
    local cpu_usage=$(echo "$time_output" | cut -d',' -f2)
    [ -z "$elapsed_ms" ] && elapsed_ms="N/A"
    [ -z "$cpu_usage" ] && cpu_usage="0.00"
    
    echo "1" >> "$RESULTS_DIR/completed_jobs.txt"
    local completed=$(wc -l < "$RESULTS_DIR/completed_jobs.txt" | tr -d ' ' | awk '{print $1}')
    local overall_elapsed=$(( $(date +%s) - PHASE_1_START_TIME ))
    
    local eta_str=""
    if [ "$completed" -gt 0 ] && [ "$TOTAL_PARALLEL" -gt 0 ]; then
        local remain=$(( TOTAL_PARALLEL - completed ))
        local avg_ms=$(( overall_elapsed * 1000 / completed ))
        local remain_s=$(( avg_ms * remain / 1000 ))
        
        local m=$(( remain_s / 60 ))
        local s=$(( remain_s % 60 ))
        local pct=$(( completed * 100 / TOTAL_PARALLEL ))
        eta_str="[${pct}% | ETA: ${m}m ${s}s]"
    fi
    
    if [ $exit_code -eq 0 ]; then
        echo -e "  [JOB $job_id] ${GREEN}✅ $(basename "$suite") PASSED${NC} (${elapsed_ms}s, CPU: ${cpu_usage}%) ${eta_str}"
    else
        echo -e "  [JOB $job_id] ${RED}❌ $(basename "$suite") FAILED${NC} (${elapsed_ms}s, CPU: ${cpu_usage}%) ${eta_str}"
    fi

    # Bulletproof process teardown: kill all daemons belonging to this job's isolated workspace
    pkill -9 -f "cowen_job_${job_id}_" >/dev/null 2>&1 || true
    pkill -9 cowen-daemon >/dev/null 2>&1 || true

    return $exit_code
}

# --- Phase 1: Parallel ---
started_count=0
FAILED_COUNT=0
export TOTAL_PARALLEL=${#PARALLEL_SUITES[@]}

export PHASE_1_START_TIME=$(date +%s)
rm -f "$RESULTS_DIR/completed_jobs.txt"
touch "$RESULTS_DIR/completed_jobs.txt"

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
        
        # 🚀 Fix: Dynamic Concurrency Queue (keeps exactly MAX_PARALLEL jobs running without idle batch waiting)
        # 统计当前正在运行的后台任务数，如果达到 MAX_PARALLEL 则等待，实现真实的并行限流
        while [ $(jobs -pr | wc -l) -ge $MAX_PARALLEL ]; do
            sleep 0.2
        done
        
        # 稍微错峰启动，减少 DB 和 I/O 拥堵
        sleep 0.2
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
done < <(find "$RESULTS_DIR" -name "job_*.log" )

if [ "$FAILED_COUNT" -eq 0 ]; then
    echo -e "${GREEN}${BOLD}✅ ALL SUITES PASSED!${NC}"
    exit 0
else
    echo -e "${RED}${BOLD}❌ $FAILED_COUNT SUITES FAILED${NC}"
    exit 1
fi
