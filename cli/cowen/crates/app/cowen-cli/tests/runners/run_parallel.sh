#!/bin/bash
# Cowen CLI Parallel Test Runner (Hybrid Mode for 100% Stability)

# 确保脚本在 bash 下运行
if [ -z "$BASH_VERSION" ]; then
    exec bash "$0" "$@"
fi

# Pre-set COWEN_BIN before sourcing common.sh to bypass the exit-on-missing check during clean builds
TARGET_BASE=${CARGO_TARGET_DIR:-target}
if [[ "$RUSTFLAGS" == *"-Cinstrument-coverage"* ]]; then
    BUILD_ARGS="${BUILD_ARGS:---profile test}"
else
    BUILD_ARGS="${BUILD_ARGS:---release}"
fi

if [[ "$BUILD_ARGS" == *"--release"* ]]; then
    COWEN_BIN_TMP="$TARGET_BASE/release/cowen"
else
    COWEN_BIN_TMP="$TARGET_BASE/debug/cowen"
fi
if [[ "$COWEN_BIN_TMP" == /* ]]; then
    export COWEN_BIN="$COWEN_BIN_TMP"
else
    export COWEN_BIN="$(pwd)/$COWEN_BIN_TMP"
fi

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
# Dynamically adjust parallel limit based on CPU cores if not explicitly set
if [ -z "$MAX_PARALLEL" ]; then
    if command -v nproc &> /dev/null; then
        MAX_PARALLEL=$(nproc)
    elif command -v sysctl &> /dev/null; then
        MAX_PARALLEL=$(sysctl -n hw.ncpu)
    else
        MAX_PARALLEL=8
    fi
fi
export MAX_PARALLEL
TEST_BASE="${TEST_BASE:-target/cowen_tests}"
if [[ "$TEST_BASE" != /* ]]; then
    TEST_BASE="$(pwd)/$TEST_BASE"
fi
export RESULTS_DIR="$TEST_BASE/results"
BASE_PORT_START="${BASE_PORT_START:-16000}"

final_parallel_cleanup() {
    if [ "$CLEANUP_DONE" == "true" ]; then return; fi
    CLEANUP_DONE="true"
    echo -e "\n${BLUE}🧹 Performing final cleanup...${NC}"
    
    # Merge telemetry data
    if [ -f "$RESULTS_DIR/job_stats.csv" ] && command -v python3 >/dev/null 2>&1; then
        cat << 'EOF_MERGE' > "$RESULTS_DIR/merge_telemetry.py"
import os, csv
TELEMETRY_FILE = "crates/app/cowen-cli/tests/runners/test_telemetry.csv"
RESULTS_DIR = os.environ.get("RESULTS_DIR", "target/cowen_tests/results")
stats = {}
if os.path.exists(TELEMETRY_FILE):
    with open(TELEMETRY_FILE, 'r') as f:
        reader = csv.reader(f)
        for row in reader:
            if len(row) >= 3:
                stats[row[0]] = row
job_stats_path = os.path.join(RESULTS_DIR, "job_stats.csv")
if os.path.exists(job_stats_path):
    with open(job_stats_path, 'r') as f:
        reader = csv.reader(f)
        for row in reader:
            if len(row) >= 3:
                stats[row[0]] = row
with open(TELEMETRY_FILE, 'w') as f:
    writer = csv.writer(f)
    for row in stats.values():
        writer.writerow(row)
EOF_MERGE
        python3 "$RESULTS_DIR/merge_telemetry.py"
        echo -e "${GREEN}📊 Test telemetry updated.${NC}"
    fi

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


echo -n "  Building cowen binary and plugins..."
export COWEN_BUILD_CLIENT_ID="dummy-parallel-client-id"
if [[ "$RUSTFLAGS" == *"-Cinstrument-coverage"* ]]; then
    BUILD_ARGS="${BUILD_ARGS:---profile test}"
else
    BUILD_ARGS="${BUILD_ARGS:---release}"
fi

# Respect CARGO_TARGET_DIR if set
TARGET_BASE=${CARGO_TARGET_DIR:-target}
if [ "$OS_NAME" = "windows-cross" ]; then
    BINARY_EXT=".exe"
    TARGET_SUBDIR="x86_64-pc-windows-gnu/"
    BUILD_ARGS="$BUILD_ARGS --target x86_64-pc-windows-gnu"
elif [ -f /.dockerenv ] || [ -f /run/.containerenv ]; then
    BUILD_ARGS="--release --target x86_64-unknown-linux-gnu"
    TARGET_SUBDIR="x86_64-unknown-linux-gnu/"
fi

if [[ "$BUILD_ARGS" == *"--release"* ]]; then
    BINARY_PATH="${TARGET_BASE}/${TARGET_SUBDIR}release/cowen${BINARY_EXT}"
else
    BINARY_PATH="${TARGET_BASE}/${TARGET_SUBDIR}debug/cowen${BINARY_EXT}"
fi

SHOULD_BUILD=true
if [ "$SKIP_BUILD" = "true" ] && [ -f "$BINARY_PATH" ]; then
    SHOULD_BUILD=false
fi

if [ "$SHOULD_BUILD" = "true" ]; then
    if COWEN_BUILD_CLIENT_ID=dummy cargo build --quiet $BUILD_ARGS -p cowen-cli -p cowen-daemon -p cowen-search-embedding -p cowen-signer -p cowen-mcp-plugin; then
        echo -e " ${GREEN}[OK]${NC}"
    else
        echo -e " ${RED}[FAILED]${NC}"
        exit 1
    fi
else
    echo -e " ${GREEN}[SKIPPED BUILD]${NC}"
fi

if [[ "$BINARY_PATH" == /* ]]; then
    export COWEN_BIN="$BINARY_PATH"
else
    export COWEN_BIN="$(pwd)/$BINARY_PATH"
fi
# Force common.sh to refresh its SOURCE_BIN
if [ -f crates/app/cowen-cli/tests/e2e/scripts/common.sh ]; then
    # We need to make sure common.sh uses the same COWEN_BIN
    update_source_bin
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
        --version "0.5.0" \
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
        --version "0.5.0" \
        --dev-key dist_assets/keys/official_dev.pk8 \
        --dev-cert dist_assets/keys/official_dev_cert.json \
        --out-bundle "$BUILD_DIR/cowen-mcp-plugin.bundle" \
        --manifest-file crates/plugins/cowen-mcp-plugin/plugin.json
    echo "✅ Plugin signed and bundle generated: \"$BUILD_DIR/cowen-mcp-plugin.bundle\""
fi

# cp "$BINARY_PATH" "$(dirname "$BINARY_PATH")/cowen-test"
if [[ "$BINARY_PATH" == /* ]]; then
    export COWEN_BIN="$BINARY_PATH"
else
    export COWEN_BIN="$(pwd)/$BINARY_PATH"
fi

# --- Suite Discovery & LPT (Longest Processing Time) Sorting ---
declare -a PARALLEL_SUITES
if [ $# -gt 0 ]; then
    PARALLEL_SUITES=("$@")
else
    # Dynamic Longest Processing Time (LPT) Scheduling
    # We use python to read telemetry data and sort the suites descending by expected ms.
    if ! command -v python3 >/dev/null 2>&1; then
        echo -e "${YELLOW}python3 not found, falling back to basic glob sorting.${NC}"
        for suite_path in crates/app/cowen-cli/tests/e2e/scripts/case_*.sh; do
            PARALLEL_SUITES+=("$suite_path")
        done
    else
        cat << 'EOF_SCHED' > "$RESULTS_DIR/telemetry.py"
import sys, os, csv, glob

TELEMETRY_FILE = "crates/app/cowen-cli/tests/runners/test_telemetry.csv"

def schedule():
    stats = {}
    if os.path.exists(TELEMETRY_FILE):
        with open(TELEMETRY_FILE, 'r') as f:
            reader = csv.reader(f)
            for row in reader:
                if len(row) >= 3:
                    try:
                        stats[row[0]] = {'ms': int(row[1]), 'cpu': float(row[2])}
                    except ValueError:
                        pass
    
    scripts = glob.glob("crates/app/cowen-cli/tests/e2e/scripts/case_*.sh")
    scored = []
    for s in scripts:
        basename = os.path.basename(s)
        ms = stats[basename]['ms'] if basename in stats else 5000
        scored.append((ms, s))
    
    scored.sort(key=lambda x: x[0], reverse=True)
    
    total_ms = sum([x[0] for x in scored])
    results_dir = os.environ.get("RESULTS_DIR", "target/cowen_tests/results")
    expected_ms_file = os.path.join(results_dir, "total_expected_ms.txt")
    with open(expected_ms_file, "w") as f:
        f.write(str(total_ms))
        
    for ms, s in scored:
        print(s)

if __name__ == "__main__":
    schedule()
EOF_SCHED
        sorted_suites=$(python3 "$RESULTS_DIR/telemetry.py")
        for suite_path in $sorted_suites; do
            PARALLEL_SUITES+=("$suite_path")
        done
    fi
fi
SEQUENTIAL_SUITES=()

run_job() {
    local suite=$1
    local job_id=$2
    local mock_port=$3
    local log_file="$RESULTS_DIR/job_${job_id}.log"
    local workspace="$TEST_BASE/.cowen_test_job_${job_id}"
    
    mkdir -p "$workspace"

    if [ "$OS_NAME" = "windows-cross" ]; then
        mkdir -p "$workspace/.wine_shared"
        cat > "$workspace/.wine_shared/user.reg" <<EOF_REG
WINE REGISTRY Version 2

[Software\\\\Wine\\\\Drivers]
"Graphics"="null"
EOF_REG
    fi
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
    local elapsed_sec=$(echo "$time_output" | cut -d',' -f1)
    local cpu_usage=$(echo "$time_output" | cut -d',' -f2)
    
    # Extract precise ms using awk
    local elapsed_ms="N/A"
    if [[ "$elapsed_sec" =~ ^[0-9]+(\.[0-9]+)?$ ]]; then
        elapsed_ms=$(awk "BEGIN {print int($elapsed_sec * 1000)}")
    fi
    [ -z "$cpu_usage" ] && cpu_usage="0.00"
    
    # Save telemetry asynchronously
    if [ "$elapsed_ms" != "N/A" ]; then
                real_name=$(basename "$suite" | sed -E 's/\.[0-9]+$//')
        echo "${real_name},${elapsed_ms},${cpu_usage}" >> "$RESULTS_DIR/job_stats.csv"
    fi
    
    echo "1" >> "$RESULTS_DIR/completed_jobs.txt"
    local completed=$(wc -l < "$RESULTS_DIR/completed_jobs.txt" | tr -d ' ' | awk '{print $1}')
    local overall_elapsed=$(( $(date +%s) - PHASE_1_START_TIME ))
    
    local eta_str=""
    if [ "$completed" -gt 0 ] && [ "$TOTAL_PARALLEL" -gt 0 ]; then
        local pct=$(( completed * 100 / TOTAL_PARALLEL ))
        local remain_s=0
        
        if [ -f "$RESULTS_DIR/total_expected_ms.txt" ] && command -v python3 >/dev/null 2>&1; then
            cat << 'EOF_ETA' > "$RESULTS_DIR/eta.py"
import sys, os, csv
MAX_PARALLEL = int(os.environ.get("MAX_PARALLEL", "32"))
RESULTS_DIR = os.environ.get("RESULTS_DIR", "target/cowen_tests/results")
completed_ms = 0
job_stats_path = os.path.join(RESULTS_DIR, "job_stats.csv")
if os.path.exists(job_stats_path):
    with open(job_stats_path) as f:
        for row in csv.reader(f):
            if len(row) >= 2:
                try:
                    completed_ms += int(row[1])
                except ValueError:
                    pass
total_expected_ms = 0
expected_ms_path = os.path.join(RESULTS_DIR, "total_expected_ms.txt")
if os.path.exists(expected_ms_path):
    with open(expected_ms_path) as f:
        total_expected_ms = int(f.read().strip())
remaining_ms_total = max(0, total_expected_ms - completed_ms)
# Add some buffer for un-parallelizable overhead or final serial tests
print(int((remaining_ms_total / MAX_PARALLEL) / 1000))
EOF_ETA
            remain_s=$(python3 "$RESULTS_DIR/eta.py")
        else
            local remain=$(( TOTAL_PARALLEL - completed ))
            local avg_ms=$(( overall_elapsed * 1000 / completed ))
            remain_s=$(( avg_ms * remain / 1000 ))
        fi
        
        local m=$(( remain_s / 60 ))
        local s=$(( remain_s % 60 ))
        eta_str="[${pct}% | ETA: ${m}m ${s}s]"
    fi
    
    if [ $exit_code -eq 0 ]; then
        echo -e "  [JOB $job_id] ${GREEN}✅ ${real_name} PASSED${NC} (${elapsed_ms}ms, CPU: ${cpu_usage}%) ${eta_str}"
    else
        echo -e "  [JOB $job_id] ${RED}❌ ${real_name} FAILED${NC} (${elapsed_ms}ms, CPU: ${cpu_usage}%) ${eta_str}"
    fi

    # Try SIGTERM first to allow coverage flush
    pkill -15 -f "cowen_job_${job_id}_" >/dev/null 2>&1 || true
    if [ -f "$workspace/master_daemon.pid" ]; then
        local daemon_pid=$(cat "$workspace/master_daemon.pid" 2>/dev/null | tr -d ' ' || true)
        if [ -n "$daemon_pid" ]; then
            kill -15 "$daemon_pid" >/dev/null 2>&1 || true
        fi
    fi
    sleep 1.0
    # Bulletproof process teardown: kill all daemons belonging to this job's isolated workspace
    pkill -9 -f "cowen_job_${job_id}_" >/dev/null 2>&1 || true
    if [ -f "$workspace/master_daemon.pid" ]; then
        local daemon_pid=$(cat "$workspace/master_daemon.pid" 2>/dev/null | tr -d ' ' || true)
        if [ -n "$daemon_pid" ]; then
            kill -9 "$daemon_pid" >/dev/null 2>&1 || true
            rm -f "$workspace/master_daemon.pid"
        fi
    fi

    return $exit_code
}

# --- Phase 1: Parallel ---
started_count=0
FAILED_COUNT=0
export TOTAL_PARALLEL=${#PARALLEL_SUITES[@]}

export PHASE_1_START_TIME=$(date +%s)
rm -f "$RESULTS_DIR/completed_jobs.txt"
touch "$RESULTS_DIR/completed_jobs.txt"

# --- Pre-initialize Shared WINEPREFIX for Windows Cross Testing ---
if [ "$OS_NAME" = "windows-cross" ]; then
    echo "⌛ Pre-initializing shared Wine prefix to avoid concurrent initialization race..."
    export WINEPREFIX="${TEST_BASE}/.wine_shared"
    export WINE_AUTO_DEBUGGER=0
    export WINEDEBUG="-all"
    export WINEDLLOVERRIDES="mscoree=;mshtml=;winevulkan=;opengl32=;d3d11=;dxgi="
    mkdir -p "$WINEPREFIX"
    
    wine_bin="wine64"
    if ! command -v wine64 >/dev/null 2>&1; then
        if command -v wine >/dev/null 2>&1; then
            wine_bin="wine"
        fi
    fi
    
    $wine_bin cmd.exe /c echo "Wine Shared Prefix Initialized" >/dev/null 2>&1
    echo "✅ Shared Wine prefix ready."
fi

if [ "$TOTAL_PARALLEL" -gt 0 ]; then
    echo -e "\n${BOLD}Phase 1: Running Parallel Suites ($TOTAL_PARALLEL) [Concurrency: $MAX_PARALLEL]${NC}"
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
