#!/bin/bash
# Cowen CLI Test Framework - Common Utilities

set -e

# ANSI Colors
export GREEN='\033[0;32m'
export RED='\033[0;31m'
export YELLOW='\033[1;33m'
export BLUE='\033[1;34m'
export BOLD='\033[1m'
export NC='\033[0m'

# Detect OS and adjust binary name
OS_TYPE=$(uname -s)
if [ "$OS_NAME" = "windows-cross" ] || [[ "$OS_TYPE" == *"MINGW"* || "$OS_TYPE" == *"MSYS"* || "$OS_TYPE" == *"CYGWIN"* ]]; then
    export IS_WINDOWS="true"
fi

get_source_bin() {
    if [ -n "$COWEN_BIN" ]; then
        echo "$COWEN_BIN"
        return
    fi
    
    local target_base=${CARGO_TARGET_DIR:-target}
    local bins=()
    if [ "$IS_WINDOWS" = "true" ]; then
        bins=(
            "$target_base/release/cowen-test.exe" 
            "$target_base/debug/cowen-test.exe" 
            "$target_base/release/cowen.exe" 
            "$target_base/debug/cowen.exe"
            "$target_base/x86_64-unknown-linux-gnu/release/cowen-test.exe"
        )
    else
        bins=(
            "$target_base/release/cowen-test" 
            "$target_base/debug/cowen-test" 
            "$target_base/release/cowen" 
            "$target_base/debug/cowen"
            "$target_base/x86_64-unknown-linux-gnu/release/cowen-test"
            "$target_base/x86_64-unknown-linux-gnu/release/cowen"
        )
    fi

    # Find the newest existing binary
    local newest=""
    local newest_time=0
    for b in "${bins[@]}"; do
        if [ -f "$b" ]; then
            local t=0
            if [[ "$OS_TYPE" == "Darwin" ]]; then
                t=$(stat -f %m "$b")
            else
                t=$(stat -c %Y "$b")
            fi
            if [ "$t" -gt "$newest_time" ]; then
                newest_time=$t
                newest=$b
            fi
        fi
    done
    echo "$newest"
}

update_source_bin() {
    export SOURCE_BIN=$(get_source_bin)
    if [ -z "$SOURCE_BIN" ]; then
        echo -e "${RED}FATAL: No cowen binary found!${NC}"
        exit 1
    fi
    export COWEN_BUILD_DIR=$(dirname "$SOURCE_BIN")
}

update_source_bin

export MOCK_PORT="${MOCK_PORT:-9299}"
export MOCK_URL="http://127.0.0.1:$MOCK_PORT"
export MOCK_WS="ws://127.0.0.1:$MOCK_PORT"
export COWEN_RAW_OUTPUT="true"
export COWEN_EXCLUSIVE="false"
export COWEN_DEV_MODE="1"

# Detect Container Environment for DB Access (Podman on macOS support)
detect_db_host() {
    # If already detected (e.g., by start_services.sh), skip entirely
    if [ -n "$DB_HOST_DETECTED" ]; then
        return
    fi

    export DB_HOST="127.0.0.1"
    if [ -f /.dockerenv ] || [ -f /run/.containerenv ]; then
        # In-container mode: services should have been started by start_services.sh
        # If DB_HOST_DETECTED is not set, it means start_services.sh was not sourced
        # Fall back to probing external hosts
        TCP_CHECK_CMD="python3 -c \"import socket; 
def check(h, p):
    try:
        s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        s.settimeout(0.5)
        return s.connect_ex((h, p)) == 0
    except:
        return False

ports = [5432, 3306, 6379]
hosts = ['127.0.0.1', 'localhost']
found = any(check(h, p) for h in hosts for p in ports)
exit(0 if found else 1)\""
        
        echo -n "  [WAIT] Probing for local database services (127.0.0.1)..."
        for i in {1..10}; do
            if eval $TCP_CHECK_CMD 2>/dev/null; then
                echo -e " ${GREEN}[READY]${NC}"
                export DB_HOST="127.0.0.1"
                export DB_HOST_DETECTED="true"
                return
            fi
            echo -n "."
            sleep 1
        done
        echo -e " ${YELLOW}[NOT FOUND]${NC}"

        # Fallback to container host gateway
        if getent hosts host.containers.internal >/dev/null 2>&1; then
            export DB_HOST="host.containers.internal"
        elif getent hosts host.docker.internal >/dev/null 2>&1; then
            export DB_HOST="host.docker.internal"
        fi
        
        echo -e "  ${YELLOW}[INFO] Using DB_HOST fallback: $DB_HOST${NC}"
    fi
    export DB_HOST_DETECTED="true"
}

# Call detection at the start of any suite
detect_db_host
# Auto-detect PostgreSQL credentials and set PG_CMD
if PGPASSWORD=password psql -h "$DB_HOST" -U postgres -d postgres -c "select 1" &> /dev/null; then
    export PG_CMD="psql -h $DB_HOST -U postgres"
    export PGPASSWORD=password
elif psql -h "$DB_HOST" -d postgres -c "select 1" &> /dev/null; then
    export PG_CMD="psql -h $DB_HOST"
    unset PGPASSWORD
else
    export PG_CMD="psql -h $DB_HOST -U postgres"
    export PGPASSWORD=password
fi

# Database Isolation
get_case_db_name() {
    local suite=$1
    # Replace dots and dashes with underscores to ensure valid DB name
    echo "cowen_test_$(echo $suite | tr '.-' '__')"
}

# Isolation
setup_workspace() {
    local suite=$1
    if [[ "${TEST_BASE:-}" != /* ]]; then
        export TEST_BASE="$(pwd)/${TEST_BASE:-target/cowen_tests}"
    fi
    export COWEN_HOME="$TEST_BASE/.cowen_test_$suite"
    echo -e "${BLUE}▶ Starting Suite: $suite${NC}"
    echo -e "  Workspace: $COWEN_HOME"

    # 🚀 BUG FIX: Kill old daemons BEFORE nuking the directory containing their .pid files
    if [ -d "$COWEN_HOME" ]; then
        find "$COWEN_HOME" -name "*_daemon.pid" 2>/dev/null | while read pid_file; do
            PID=$(cat "$pid_file" 2>/dev/null)
            if [ -n "$PID" ]; then
                kill -9 "$PID" >/dev/null 2>&1 || true
                sleep 0.5
            fi
        done
        # Also kill by unified master daemon PID if present
        if [ -f "$COWEN_HOME/master_daemon.pid" ]; then
            PID=$(cat "$COWEN_HOME/master_daemon.pid" | head -n 1 2>/dev/null)
            if [ -n "$PID" ]; then
                kill -9 "$PID" >/dev/null 2>&1 || true
                sleep 0.5
            fi
        fi
    fi

    rm -rf "$COWEN_HOME"
    mkdir -p "$COWEN_HOME"

    # 🚀 PROCESS ISOLATION: Create a symbolic link with a unique name
    local unique_name="cowen_$suite"
    local unique_daemon="cowen_daemon_$suite"
    # Filter out dots and dashes from name
    unique_name=$(echo $unique_name | tr '.-' '_')
    unique_daemon=$(echo $unique_daemon | tr '.-' '_')
    
    # Use cp instead of symbolic link so the OS process manager shows the unique name
    # On modern filesystems, cp is fast. This ensures 'pkill' and process isolation works flawlessly.
    local abs_source=$(python3 -c "import os; print(os.path.abspath('$SOURCE_BIN'))")
    
    local ext=""
    if [ "$OS_NAME" = "windows-cross" ]; then
        ext=".exe"
    fi

    if [ "$OS_NAME" = "windows-cross" ]; then
        local wine_bin="wine64"
        if ! command -v wine64 >/dev/null 2>&1; then
            if command -v wine >/dev/null 2>&1; then
                wine_bin="wine"
            fi
        fi
        
        # Pre-initialize WINEPREFIX for the workspace to prevent races
        export WINEPREFIX="${TEST_BASE}/.wine_shared"
        export WINE_AUTO_DEBUGGER=0
        mkdir -p "$WINEPREFIX"

        cp "$abs_source" "$COWEN_HOME/$unique_name$ext"
        cat > "$COWEN_HOME/$unique_name" <<EOF
#!/bin/bash
export WINEPREFIX="${TEST_BASE}/.wine_shared"
export WINEDEBUG="-all"
export WINE_AUTO_DEBUGGER=0
export COWEN_HOME="Z:$COWEN_HOME"
exec $wine_bin "Z:$COWEN_HOME/$unique_name$ext" "\$@"
EOF
        chmod +x "$COWEN_HOME/$unique_name"
        export COWEN_BIN="$COWEN_HOME/$unique_name"
    else
        cp "$abs_source" "$COWEN_HOME/$unique_name"
        export COWEN_BIN="$COWEN_HOME/$unique_name"
    fi
    
    # 🚀 DAEMON EXTRACTION: Also copy and RENAME the standalone daemon binary
    local build_dir=$(dirname "$abs_source")
    local daemon_src="$build_dir/cowen-daemon$ext"
    
    # Fallback to other build dirs if not found in current one
    if [ ! -f "$daemon_src" ]; then
        if [ -f "target/release/cowen-daemon$ext" ]; then
            daemon_src="target/release/cowen-daemon$ext"
        elif [ -f "target/debug/cowen-daemon$ext" ]; then
            daemon_src="target/debug/cowen-daemon$ext"
        fi
    fi

    if [ -f "$daemon_src" ]; then
        cp "$daemon_src" "$COWEN_HOME/$unique_daemon$ext"
        
        if [ "$OS_NAME" = "windows-cross" ]; then
            # 🚀 OCP/Rosetta Fix: Point COWEN_DAEMON_BIN directly to the Windows .exe binary
            # under Wine's Z: drive mapping. Avoid Unix wrappers to bypass Rosetta index crashes.
            export COWEN_DAEMON_BIN="Z:$COWEN_HOME/$unique_daemon$ext"
            chmod +x "$COWEN_HOME/$unique_daemon$ext"
        else
            export COWEN_DAEMON_BIN="$COWEN_HOME/$unique_daemon$ext"
            chmod +x "$COWEN_DAEMON_BIN"
        fi
    else
        echo -e "${YELLOW}⚠️  cowen-daemon not found. Standard internal server logic will be used.${NC}"
    fi

    # stand-alone signer
    local signer_src="$build_dir/cowen-signer$ext"
    if [ ! -f "$signer_src" ]; then
        if [ -f "target/llvm-cov-target/debug/cowen-signer$ext" ]; then
            signer_src="target/llvm-cov-target/debug/cowen-signer$ext"
        elif [ -f "target/release/cowen-signer$ext" ]; then
            signer_src="target/release/cowen-signer$ext"
        elif [ -f "target/debug/cowen-signer$ext" ]; then
            signer_src="target/debug/cowen-signer$ext"
        fi
    fi
    if [ -f "$signer_src" ]; then
        cp "$signer_src" "$COWEN_HOME/cowen-signer$ext"
        
        if [ "$OS_NAME" = "windows-cross" ]; then
            cat > "$COWEN_HOME/cowen-signer" <<EOF
#!/bin/bash
export WINEPREFIX="${TEST_BASE}/.wine_shared"
export WINEDEBUG="-all"
export WINE_AUTO_DEBUGGER=0
export COWEN_HOME="Z:$COWEN_HOME"
exec $wine_bin "Z:$COWEN_HOME/cowen-signer$ext" "\$@"
EOF
            chmod +x "$COWEN_HOME/cowen-signer"
        fi
        chmod +x "$COWEN_HOME/cowen-signer$ext"
    fi

    # stand-alone mcp-plugin
    local mcp_src="$build_dir/cowen-mcp-plugin$ext"
    if [ ! -f "$mcp_src" ]; then
        if [ -f "target/llvm-cov-target/debug/cowen-mcp-plugin$ext" ]; then
            mcp_src="target/llvm-cov-target/debug/cowen-mcp-plugin$ext"
        elif [ -f "target/release/cowen-mcp-plugin$ext" ]; then
            mcp_src="target/release/cowen-mcp-plugin$ext"
        elif [ -f "target/debug/cowen-mcp-plugin$ext" ]; then
            mcp_src="target/debug/cowen-mcp-plugin$ext"
        fi
    fi
    if [ -f "$mcp_src" ]; then
        cp "$mcp_src" "$COWEN_HOME/cowen-mcp-plugin$ext"
        
        if [ "$OS_NAME" = "windows-cross" ]; then
            cat > "$COWEN_HOME/cowen-mcp-plugin" <<EOF
#!/bin/bash
export WINEPREFIX="${TEST_BASE}/.wine_shared"
export WINEDEBUG="-all"
export WINE_AUTO_DEBUGGER=0
export COWEN_HOME="Z:$COWEN_HOME"
exec $wine_bin "Z:$COWEN_HOME/cowen-mcp-plugin$ext" "\$@"
EOF
            chmod +x "$COWEN_HOME/cowen-mcp-plugin"
        fi
        chmod +x "$COWEN_HOME/cowen-mcp-plugin$ext"
    fi

    # Also copy search embedding plugin if exists
    if [ -f "$build_dir/libcowen_search_embedding" ]; then
        cp "$build_dir/libcowen_search_embedding" "$COWEN_HOME/"
        cp "$build_dir/libcowen_search_embedding.bundle" "$COWEN_HOME/" 2>/dev/null || true
    fi
    if [ -f "$build_dir/libcowen_search_embedding.exe" ]; then
        cp "$build_dir/libcowen_search_embedding.exe" "$COWEN_HOME/"
        cp "$build_dir/libcowen_search_embedding.bundle" "$COWEN_HOME/" 2>/dev/null || true
    fi

    # Ensure it is executable
    chmod +x "$COWEN_BIN"
    
    # 🚀 OCP: Globally skip browser popups in E2E tests, but verify the trigger log
    export COWEN_SKIP_BROWSER=true

    local monitor_port=$(get_unused_port)
    export COWEN_ALLOW_PORT_FALLBACK=1
    export COWEN_MONITOR_PORT=$monitor_port

    local db_path="$COWEN_HOME/cowen.db"
    if [ "$OS_NAME" = "windows-cross" ]; then
        db_path="Z:$COWEN_HOME/cowen.db"
    fi

    # Create isolated app.yaml with absolute DB path
    cat > "$COWEN_HOME/app.yaml" <<EOF
monitor_port: $monitor_port
storage:
  store: innerdb
  db_url: "sqlite://$db_path"
log:
  level: debug
telemetry_enabled: false
ai_enabled: false
EOF

    # 🚀 OCP Enforcement: Automatically register cleanups on exit for all suites using setup_workspace
    trap cleanup_suite EXIT
}


apply_fixture() {
    local name=$1
    local prof=$2
    cp "tests/infra/fixtures/$name.yaml" "$COWEN_HOME/$prof.yaml"
}

# Cleanup
cleanup_suite() {
    local exit_code=$?
    # Run in a subshell to isolate errors during cleanup
    (
        set +e
        echo -e "${YELLOW}  Cleaning up environment for $COWEN_HOME...${NC}"
        
        # 1. Surgical Kill: Kill ONLY daemons belonging to this workspace
        if [ -d "$COWEN_HOME" ]; then
            # Find all .pid files in COWEN_HOME
            find "$COWEN_HOME" -name "*_daemon.pid" 2>/dev/null | while read pid_file; do
                PID=$(head -n 1 "$pid_file" 2>/dev/null)
                if [ -n "$PID" ]; then
                    echo "     Killing daemon PID $PID..."
                    kill -15 "$PID" >/dev/null 2>&1 || true
                    for i in {1..10}; do
                        if ! kill -0 "$PID" 2>/dev/null; then
                            break
                        fi
                        sleep 0.1
                    done
                    if kill -0 "$PID" 2>/dev/null; then
                        kill -9 "$PID" >/dev/null 2>&1 || true
                    fi
                fi
                rm -f "$pid_file" >/dev/null 2>&1 || true
            done
            
            # Robust Lock File Cleanup: Ensure all db lock/wal/shm files are removed
            # This prevents SQLite from entering busy/locked states on next run
            find "$COWEN_HOME" -name "*.db-wal" -o -name "*.db-shm" -o -name "*.db-journal" | while read lock_file; do
                rm -f "$lock_file" >/dev/null 2>&1 || true
            done
        fi

        # 1.5 Global pkill as fallback (Robustness)
        if [ -n "$COWEN_BIN" ]; then
            pkill -15 -f "$(basename "$COWEN_BIN")" >/dev/null 2>&1 || true
        fi
        if [ -n "$COWEN_DAEMON_BIN" ]; then
            pkill -15 -f "$(basename "$COWEN_DAEMON_BIN")" >/dev/null 2>&1 || true
        fi
        sleep 1.0
        if [ -n "$COWEN_BIN" ]; then
            pkill -9 -f "$(basename "$COWEN_BIN")" >/dev/null 2>&1 || true
        fi
        if [ -n "$COWEN_DAEMON_BIN" ]; then
            pkill -9 -f "$(basename "$COWEN_DAEMON_BIN")" >/dev/null 2>&1 || true
        fi
        
        # 2. Cleanup mock server state for next case (Only if shared)
        if [ "$COWEN_MOCK_MANAGED" == "true" ]; then
            curl -s -X POST "$MOCK_URL/control/kill_connections" >/dev/null 2>&1 || true
            curl -s -X POST "$MOCK_URL/control/clear_webhooks" >/dev/null 2>&1 || true
        fi
        
        # 3. Handle Mock Server if it was started privately
        if [ "$COWEN_MOCK_MANAGED" != "true" ] && [ -n "$MOCK_PID" ]; then
            kill -9 "$MOCK_PID" >/dev/null 2>&1 || true
        fi

        # 4. Remove workspace directory (Only on success)
        if [ "$exit_code" -eq 0 ]; then
            if [ -n "$COWEN_HOME" ] && [[ "$COWEN_HOME" == *"_test_"* ]]; then
                rm -rf "$COWEN_HOME" >/dev/null 2>&1 || true
            fi
        else
            echo -e "${YELLOW}ℹ️  Workspace preserved for debugging: $COWEN_HOME${NC}"
        fi
    )
    return 0
}

# Assertions
pass_suite() {
    local name=$1
    if [ -z "$name" ]; then
        local base=$(basename "$0" .sh)
        if [[ "$base" =~ ^case_([0-9]+) ]]; then
            name="Case ${BASH_REMATCH[1]}"
        else
            name="$base"
        fi
    fi
    echo -e "\n${GREEN}🎊 $name Passed!${NC}"
}

fail_suite() {
    local msg=$1
    local name=$2
    if [ -z "$name" ]; then
        local base=$(basename "$0" .sh)
        if [[ "$base" =~ ^case_([0-9]+) ]]; then
            name="Case ${BASH_REMATCH[1]}"
        else
            name="$base"
        fi
    fi
    echo -e "\n${RED}✗ $name FAILED: $msg${NC}"
    exit 1
}

assert_pass() {
    if [ $? -eq 0 ]; then
        echo -e "  ${GREEN}✓${NC} $1"
    else
        echo -e "  ${RED}✗${NC} $1"
        exit 1
    fi
}

assert_match() {
    if echo "$1" | grep -q "$2"; then
        echo -e "  ${GREEN}✓${NC} $3"
    else
        echo -e "  ${RED}✗${NC} $3"
        echo "    Expected pattern: $2"
        echo "    Actual output:    $1"
        exit 1
    fi
}

assert_not_match() {
    if echo "$1" | grep -q "$2"; then
        echo -e "  ${RED}✗${NC} $3"
        echo "    Did not expect pattern: $2"
        echo "    Actual output:          $1"
        exit 1
    else
        echo -e "  ${GREEN}✓${NC} $3"
    fi
}

# Mock Server Management
start_mock() {
    if [ "$COWEN_MOCK_MANAGED" == "true" ]; then
        return 0
    fi
    echo -n "  Starting Mock Server on port $MOCK_PORT..."
    # Ensure port is free
    local is_real_windows=false
    if [ "$IS_WINDOWS" = true ] && [[ "$(uname -s)" == *"MINGW"* || "$(uname -s)" == *"MSYS"* ]]; then
        is_real_windows=true
    fi

    if [ "$is_real_windows" = true ]; then
        local pid=$(netstat -ano | grep ":$MOCK_PORT" | grep "LISTENING" | awk '{print $5}' | head -n 1)
        if [ -n "$pid" ]; then taskkill //F //PID "$pid" >/dev/null 2>&1 || true; fi
    else
        lsof -ti ":$MOCK_PORT" | xargs kill -9 2>/dev/null || true
    fi
    sleep 1
    # Bind to 0.0.0.0 for container accessibility
    PYTHONUNBUFFERED=1 MOCK_PORT=$MOCK_PORT python3 -u crates/app/cowen-cli/tests/infra/mock_server.py > "$TEST_BASE/mock_server_$MOCK_PORT.log" 2>&1 &
    MOCK_PID=$!
    for i in {1..10}; do
        if curl -s $MOCK_URL/v1/mock/ping > /dev/null; then
            echo -e " ${GREEN}[READY]${NC}"
            return 0
        fi
        sleep 1
    done
    echo -e " ${RED}[TIMEOUT]${NC}"
    cat "$TEST_BASE/mock_server_$MOCK_PORT.log"
    exit 1
}

# Token Extraction
extract_token() {
    local prof=$1
    shift
    local extra_args="$@"
    local T=$(RUST_LOG=info "$COWEN_BIN" auth token --profile "$prof" $extra_args --format json 2>&1 | python3 -c "
import sys, json, re
raw_input = sys.stdin.read()
# Remove log lines (timestamps or DEBUG: prefixes)
lines = [l for l in raw_input.splitlines() if not re.match(r'^\d{4}-\d{2}-\d{2}|DEBUG:', l)]
clean_text = '\n'.join(lines)
try:
    start = clean_text.find('{')
    end = clean_text.rfind('}') + 1
    if start >= 0 and end > start:
        d = json.loads(clean_text[start:end])
        print(d.get('access_token') or d.get('value') or '')
    else:
        # If no JSON found, print the first non-empty line
        for l in lines:
            if l.strip():
                print(l.strip())
                break
except:
    print('')
")
    if [ -n "$T" ]; then
        assert_sanitized "$T" "Token ($prof) security compliance check"
    fi
    echo "$T"
}


# Global Cleanup
cleanup_all_workspaces() {
    local is_real_windows=false
    if [ "$IS_WINDOWS" = true ] && [[ "$(uname -s)" == *"MINGW"* || "$(uname -s)" == *"MSYS"* ]]; then
        is_real_windows=true
    fi

    # 1. Kill all cowen related processes
    if [ "$is_real_windows" = true ]; then
        taskkill //F //IM cowen_*.exe >/dev/null 2>&1 || true
    else
        pkill -15 -f "cowen_" >/dev/null 2>&1 || true
        sleep 0.2
        pkill -9 -f "cowen_" >/dev/null 2>&1 || true
        if [ "$OS_NAME" = "windows-cross" ]; then
            WINEPREFIX="${TEST_BASE:-target/coverage_windows-cross}/.wine_shared" wineserver -k >/dev/null 2>&1 || true
        fi
    fi

    # 2. Kill all mock servers
    if [ "$is_real_windows" = true ]; then
        # On Windows, we usually kill by port or process name if known
        local pids=$(netstat -ano | grep LISTENING | grep -E ":(9299|16000|18000)" | awk '{print $5}' | sort -u)
        for pid in $pids; do taskkill //F //PID "$pid" >/dev/null 2>&1 || true; done
    else
        pkill -9 -f "mock_server.py" >/dev/null 2>&1 || true
        # Also kill anything listening on the test port range (16000-19500)
        # This is a bit aggressive but safe for a test environment
        if command -v lsof >/dev/null; then
            lsof -ti :9299,16000-19500 | xargs kill -9 >/dev/null 2>&1 || true
        fi
    fi

    # 3. Handle workspaces - Always cleanup the large workspace directories to keep env clean
    rm -rf target/cowen_tests/.cowen_test_*
    rm -f target/cowen_tests/.cowen_test_*.db target/cowen_tests/.cowen_test_*.db-shm target/cowen_tests/.cowen_test_*.db-wal
}

# Redis Helpers
clear_redis() {
    local url=${1:-"redis://127.0.0.1:6379/0"}
    echo -e "  Clearing Redis at $url..."
    if [[ "$url" =~ redis://([^:]+):([0-9]+)/([0-9]+) ]]; then
        local host=${BASH_REMATCH[1]}
        local port=${BASH_REMATCH[2]}
        local db=${BASH_REMATCH[3]}
        redis-cli -h "$host" -p "$port" -n "$db" FLUSHDB >/dev/null 2>&1 || true
    else
        redis-cli FLUSHALL >/dev/null 2>&1 || true
    fi
}

# Helper to execute psql commands with retries for transient errors
safe_psql_exec() {
    local cmd="$1"
    local db="${2:-postgres}"
    local max_retries=10
    local attempt=1
    local out_file=$(mktemp)
    
    while [ $attempt -le $max_retries ]; do
        # 🚀 Fix: Force LC_ALL=C for consistent English error matching
        if LC_ALL=C $PG_CMD -d "$db" -c "$cmd" 2>&1 | tee "$out_file"; then
            rm -f "$out_file"
            return 0
        fi
        
        local err=$(cat "$out_file")
        # 🚀 Fix: Expanded keywords and handle potential race conditions
        if [[ "$err" == *"recovery mode"* || "$err" == *"starting up"* || 
              "$err" == *"connection refused"* || "$err" == *"too many clients"* ||
              "$err" == *"closed the connection"* || "$err" == *"意外地关闭了联接"* ]]; then
            
            # Exponential backoff with jitter
            local wait_time=$(( (RANDOM % 3) + attempt * 2 ))
            echo -e "  ${YELLOW}[RETRY] Postgres busy/recovering (Attempt $attempt/$max_retries), waiting ${wait_time}s...${NC}" >&2
            sleep $wait_time
            ((attempt++))
        else
            echo -e "  ${RED}[FATAL] Postgres error not retryable:${NC} $err" >&2
            rm -f "$out_file"
            return 1
        fi
    done
    rm -f "$out_file"
    return 1
}

# Helper to wait for PostgreSQL to be ready
wait_for_postgres() {
    local host="${1:-$DB_HOST}"
    local port="${2:-5432}"
    echo -n "  Waiting for PostgreSQL at $host:$port to be ready..." >&2
    for i in {1..15}; do
        # 🚀 Cross-platform TCP check fallback
        local tcp_ok=false
        if command -v timeout >/dev/null 2>&1; then
            timeout 2 bash -c "</dev/tcp/$host/$port" 2>/dev/null && tcp_ok=true
        else
            python3 -c "import socket; s=socket.socket(); s.settimeout(2); exit(s.connect_ex(('$host', $port)))" 2>/dev/null && tcp_ok=true
        fi

        if [ "$tcp_ok" = true ]; then
            if psql -h "$host" -p "$port" -U postgres -d postgres -c "select 1" >/dev/null 2>&1 || \
               psql -h "$host" -p "$port" -d postgres -c "select 1" >/dev/null 2>&1 || \
               PGPASSWORD=password psql -h "$host" -p "$port" -U postgres -d postgres -c "select 1" >/dev/null 2>&1; then
                echo -e " ${GREEN}[READY]${NC}" >&2
                return 0
            fi
        fi
        echo -n "." >&2
        sleep 2
    done
    echo -e " ${RED}[TIMEOUT]${NC}" >&2
    return 1
}

# Helper to wait for MySQL to be ready
wait_for_mysql() {
    local host="${1:-$DB_HOST}"
    local port="${2:-3306}"
    echo -n "  Waiting for MySQL at $host:$port to be ready..." >&2
    for i in {1..15}; do
        local tcp_ok=false
        if command -v timeout >/dev/null 2>&1; then
            timeout 2 bash -c "</dev/tcp/$host/$port" 2>/dev/null && tcp_ok=true
        else
            python3 -c "import socket; s=socket.socket(); s.settimeout(2); exit(s.connect_ex(('$host', $port)))" 2>/dev/null && tcp_ok=true
        fi

        if [ "$tcp_ok" = true ]; then
            if mysql -h "$host" -P "$port" -u root -e "select 1" >/dev/null 2>&1 || \
               mysql -h "$host" -P "$port" -u root -proot -e "select 1" >/dev/null 2>&1; then
                echo -e " ${GREEN}[READY]${NC}" >&2
                return 0
            fi
        fi
        echo -n "." >&2
        sleep 2
    done
    echo -e " ${RED}[TIMEOUT]${NC}" >&2
    return 1
}

# Helper to get an unused TCP port from the OS
get_unused_port() {
    if [ -n "$COWEN_PORT_RANGE_START" ]; then
        local offset_file="${TEST_BASE:-/tmp}/.port_offset"
        local offset=0
        if [ -f "$offset_file" ]; then
            offset=$(cat "$offset_file")
        fi
        offset=$((offset + 1))
        echo "$offset" > "$offset_file"
        echo $((COWEN_PORT_RANGE_START + offset))
    else
        python3 -c 'import socket; s=socket.socket(); s.bind(("", 0)); print(s.getsockname()[1]); s.close()'
    fi
}

# Helper to get daemon PID from lock file
get_daemon_pid() {
    local prof=$1
    # Check for unified master daemon first
    local master_pid_file="$COWEN_HOME/master_daemon.pid"
    if [ -f "$master_pid_file" ]; then
        cat "$master_pid_file" | head -n 1
    else
        # Fallback to legacy profile-specific pid file
        local pid_file="$COWEN_HOME/${prof}_daemon.pid"
        if [ -f "$pid_file" ]; then
            cat "$pid_file" | head -n 1
        else
            echo ""
        fi
    fi
}

# Verify if sensitive data is sanitized / masked properly in CLI outputs or documents
assert_sanitized() {
    local text="$1"
    local desc="${2:-Sensitive data sanitization check}"
    local leak=false
    if echo "$text" | grep -qE "AS_[A-Z]{2}|AS_SB|AS_SA|CERT_SB|CERT_SA|1234567890123456"; then
        if ! echo "$text" | grep -qE "<[A-Z0-9_]+>"; then
            leak=true
        fi
    fi

    if [ "$leak" = true ]; then
        echo -e "  ${RED}✗${NC} $desc: FAILED! Sensitive data leakage detected." >&2
        echo "    Offending text: $text" >&2
        exit 1
    else
        echo -e "  ${GREEN}✓${NC} $desc: Passed (Sanitized)" >&2
    fi
}

# Wait for token to be acquired by profile with an expected prefix
wait_for_token() {
    local profile="$1"
    local expected_prefix="$2"
    local max_retries="${3:-10}"
    
    for i in $(seq 1 "$max_retries"); do
        local T=$(extract_token "$profile")
        if [ -n "$T" ] && [[ "$T" == $expected_prefix* ]]; then
            echo "$T"
            return 0
        fi
        sleep 1
    done
    return 1
}

wait_for_daemon() {
    local profile="$1"
    local max_retries="${2:-15}"
    
    local is_active=false
    for i in $(seq 1 "$max_retries"); do
        if COWEN_SKIP_DAEMON_RECOVERY=true "$COWEN_BIN" status --profile "$profile" 2>&1 | grep -q "ACTIVE\|RUNNING"; then
            is_active=true
            break
        fi
        sleep 1
    done

    if [ "$is_active" = false ]; then
        return 1
    fi

    # Long-term fix for Mock Server Push race conditions:
    # Wait for the WebSocket bridge to be fully established before returning.
    local log_file="$COWEN_HOME/logs/${profile}_sys.log"
    local config_file="$COWEN_HOME/profiles/${profile}/config.toml"
    
    local has_stream=false
    if [ -n "$COWEN_STREAM_URL" ]; then
        has_stream=true
    elif [ -f "$config_file" ] && grep -E -q 'stream_url = ".+"' "$config_file" 2>/dev/null; then
        has_stream=true
    fi

    if [ "$has_stream" = true ]; then
        for i in $(seq 1 "$max_retries"); do
            if [ -f "$log_file" ] && grep -q -E "Bridge connection established|Bridge synced token successfully|WebSocket connected" "$log_file" 2>/dev/null; then
                # Give it a tiny moment to process the initial mock server push
                sleep 0.5
                break
            fi
            sleep 1
        done
    fi

    return 0
}

# Setup and isolate PostgreSQL database
setup_postgres_db() {
    local db_name="$1"
    local host="${2:-$DB_HOST}"
    local port="${3:-5432}"
    
    wait_for_postgres "$host" "$port" || exit 1
    
    if PGPASSWORD=password psql -h "$host" -U postgres -d postgres -c "select 1" &> /dev/null; then
        export PG_BASE_URL="postgres://postgres:password@$host:$port"
        export PGPASSWORD=password
    elif psql -h "$host" -d postgres -c "select 1" &> /dev/null; then
        export PG_BASE_URL="postgres://$host:$port"
        unset PGPASSWORD
    else
        export PG_BASE_URL="postgres://postgres:password@$host:$port"
        export PGPASSWORD=password
    fi
    safe_psql_exec "DROP DATABASE IF EXISTS $db_name;" "postgres" >/dev/null 2>&1 || true
    safe_psql_exec "CREATE DATABASE $db_name;" "postgres" >/dev/null
    
    echo "$PG_BASE_URL/$db_name?sslmode=disable"
}

# Setup and isolate MySQL database
setup_mysql_db() {
    local db_name="$1"
    local host="${2:-$DB_HOST}"
    local port="${3:-3306}"
    
    wait_for_mysql "$host" "$port" || exit 1
    
    if mysql -u root -h "$host" -e "select 1" &>/dev/null; then
        export MYSQL_BASE_URL="mysql://root@$host:$port"
        export MYSQL_CMD="mysql -u root -h $host"
    elif mysql -u root -proot -h "$host" -e "select 1" &>/dev/null; then
        export MYSQL_BASE_URL="mysql://root:root@$host:$port"
        export MYSQL_CMD="mysql -u root -proot -h $host"
    else
        export MYSQL_BASE_URL="mysql://root:root@$host:$port"
        export MYSQL_CMD="mysql -u root -proot -h $host"
    fi
    
    if ! command -v mysql &> /dev/null; then
        podman exec cowen-mysql mysql -u root -proot -e "DROP DATABASE IF EXISTS $db_name; CREATE DATABASE $db_name;" >/dev/null 2>&1 || true
    else
        $MYSQL_CMD -e "DROP DATABASE IF EXISTS $db_name; CREATE DATABASE $db_name;" >/dev/null 2>&1
    fi
    
    echo "$MYSQL_BASE_URL/$db_name"
}

# 统一终止并回收传入的多个工作目录下的所有 master 和 profile 守护进程
kill_daemons_in_dirs() {
    for dir in "$@"; do
        if [ -d "$dir" ]; then
            for pattern in "*_daemon.pid" "master_daemon.pid"; do
                find "$dir" -name "$pattern" 2>/dev/null | while read pid_file; do
                    local PID=$(head -n 1 "$pid_file" 2>/dev/null)
                    if [ -n "$PID" ]; then
                        echo "     Killing daemon PID $PID in $dir..." >&2
                        kill -9 "$PID" >/dev/null 2>&1 || true
                    fi
                done
            done
        fi
    done
}

# 从给定的 JSON 字符串中安全提取指定的 Key 值，避免各脚本拼装重复的 inline python 管道
get_json_field() {
    local json_str="$1"
    local field_name="$2"
    echo "$json_str" | python3 -c "import sys,json; d=json.loads(sys.stdin.read()); print(d.get('$field_name', ''))" 2>/dev/null
}

# 轮询检测 Mock 服务上的 WS 活跃连接数是否达到预期的数值
wait_for_connections() {
    local expected="$1"
    local max_retries="${2:-15}"
    local conn=0
    local mock_url="${MOCK_URL:-http://127.0.0.1:9299}"
    for i in $(seq 1 "$max_retries"); do
        local raw_json=$(curl -s "$mock_url/control/connection_count" || echo "{}")
        conn=$(get_json_field "$raw_json" "count")
        if [ -n "$conn" ] && [ "$conn" -ge "$expected" ]; then
            echo "$conn"
            return 0
        fi
        sleep 1
    done
    echo "❌ wait_for_connections failed! Expected >= $expected, last count: $conn" >&2
    echo "   Mock connection state dump: $(curl -s "$mock_url/control/connection_count" || echo "failed to query mock")" >&2
    return 1
}

# 轮询检测 Webhook Sink 中指定 msg_type 的接收计数是否达到预期的数值
wait_for_webhook_count() {
    local expected_msg_type="$1"
    local expected_count="$2"
    local max_retries="${3:-25}"
    local recv_count=0
    local mock_url="${MOCK_URL:-http://127.0.0.1:9299}"
    for i in $(seq 1 "$max_retries"); do
        local raw_webhooks=$(curl -s "$mock_url/control/webhooks" || echo "[]")
        recv_count=$(echo "$raw_webhooks" | python3 -c "import sys, json; d=json.load(sys.stdin); print(len([m for m in d if (m.get('body') or m).get('msg_type') == '$expected_msg_type']))" 2>/dev/null)
        if [ -n "$recv_count" ] && [ "$recv_count" -ge "$expected_count" ]; then
            echo "$recv_count"
            return 0
        fi
        sleep 1
    done
    echo "$recv_count"
    return 1
}

# 轮询检测指定 profile 的守护进程是否已经启动（通过进程 PID 存活来判断）
# 参数：
#   $1 - profile 名字 (可选，默认为 main)
#   $2 - 预期匹配的状态关键字/正则 (可选，为了兼容性保留该参数占位，但不作为判断依据)
#   $3 - 最大尝试次数 (可选，默认为 15)
wait_for_daemon_status() {
    local profile="${1:-main}"
    local pattern="$2"
    local max_retries="${3:-15}"
    
    for i in $(seq 1 "$max_retries"); do
        local pid
        pid=$(get_daemon_pid "$profile")
        if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then
            return 0
        fi
        sleep 1
    done
    return 1
}

# 轮询直到指定区间的所有并发 Pods 全部就绪（状态匹配特定模式）
# 参数：
#   $1 - 基础测试路径 (如 $BASE_HOME)
#   $2 - pod 范围起始值 (如 1)
#   $3 - pod 范围结束值 (如 4)
#   $4 - 预期匹配的状态模式 (可选，默认为 "ACTIVE\|RUNNING")
#   $5 - 最大尝试秒数 (可选，默认为 15)
wait_for_pods_active() {
    local base_home="$1"
    local start="$2"
    local end="$3"
    local pattern="${4:-ACTIVE\|RUNNING}"
    local timeout="${5:-20}"
    
    local expected=$((end - start + 1))
    local all_active=false
    for elapsed in $(seq 1 "$timeout"); do
        local active_count=0
        for i in $(seq "$start" "$end"); do
            local POD_HOME="$base_home/pod_$i"
            if COWEN_HOME="$POD_HOME" "$COWEN_BIN" status 2>/dev/null | grep -q "$pattern"; then
                ((active_count++))
            fi
        done
        if [ "$active_count" -eq "$expected" ]; then
            all_active=true
            break
        fi
        sleep 1
    done

    if [ "$all_active" = false ]; then
        return 1
    fi

    # Long-term fix for Sidecar Bridge Race Conditions
    # If a stream URL is configured via ENV or in the first pod's profile, wait for all bridges to establish.
    local has_stream=false
    if [ -n "$COWEN_STREAM_URL" ]; then
        has_stream=true
    else
        local sample_config="$base_home/pod_${start}/profiles/main/config.toml"
        if [ -f "$sample_config" ] && grep -E -q 'stream_url = ".+"' "$sample_config" 2>/dev/null; then
            has_stream=true
        fi
    fi

    if [ "$has_stream" = true ]; then
        for elapsed in $(seq 1 "$timeout"); do
            local connected_count=0
            for i in $(seq "$start" "$end"); do
                local POD_HOME="$base_home/pod_$i"
                # Support both profile logs and daemon foreground output logs
                if grep -q -E "Bridge connection established|Bridge synced token successfully|WebSocket connected" "$POD_HOME/logs/"*_sys.log 2>/dev/null || grep -q -E "Bridge connection established|Bridge synced token successfully|WebSocket connected" "$POD_HOME/daemon.log" 2>/dev/null; then
                    ((connected_count++))
                fi
            done
            if [ "$connected_count" -eq "$expected" ]; then
                sleep 0.5
                break
            fi
            sleep 1
        done
    fi

    return 0
}



