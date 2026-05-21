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

get_source_bin() {
    if [ -n "$COWEN_BIN" ]; then
        echo "$COWEN_BIN"
        return
    fi
    
    local bins=()
    if [[ "$OS_TYPE" == *"MINGW"* || "$OS_TYPE" == *"MSYS"* || "$OS_TYPE" == *"CYGWIN"* ]]; then
        bins=("./target/release/cowen-test.exe" "./target/debug/cowen-test.exe" "./target/release/cowen.exe" "./target/debug/cowen.exe")
    else
        bins=("./target/release/cowen-test" "./target/debug/cowen-test" "./target/release/cowen" "./target/debug/cowen")
    fi

    # Find the newest existing binary
    local newest=""
    local newest_time=0
    for b in "${bins[@]}"; do
        if [ -f "$b" ]; then
            local t=0
            if [[ "$OSTYPE" == "darwin"* ]]; then
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
    cp "$abs_source" "$COWEN_HOME/$unique_name"
    export COWEN_BIN="$COWEN_HOME/$unique_name"
    
    # 🚀 DAEMON EXTRACTION: Also copy and RENAME the standalone daemon binary
    local build_dir=$(dirname "$abs_source")
    local daemon_src="$build_dir/cowen-daemon"
    
    # Fallback to other build dirs if not found in current one
    if [ ! -f "$daemon_src" ]; then
        if [ -f "target/release/cowen-daemon" ]; then
            daemon_src="target/release/cowen-daemon"
        elif [ -f "target/debug/cowen-daemon" ]; then
            daemon_src="target/debug/cowen-daemon"
        fi
    fi

    if [ -f "$daemon_src" ]; then
        cp "$daemon_src" "$COWEN_HOME/$unique_daemon"
        export COWEN_DAEMON_BIN="$COWEN_HOME/$unique_daemon"
        chmod +x "$COWEN_DAEMON_BIN"
    else
        echo -e "${YELLOW}⚠️  cowen-daemon not found. Standard internal server logic will be used.${NC}"
    fi

    # Also copy search embedding plugin if exists
    if [ -f "$build_dir/libcowen_search_embedding.dylib" ]; then
        cp "$build_dir/libcowen_search_embedding.dylib" "$COWEN_HOME/"
    fi
    if [ -f "$build_dir/libcowen_search_embedding.so" ]; then
        cp "$build_dir/libcowen_search_embedding.so" "$COWEN_HOME/"
    fi

    # Ensure it is executable
    chmod +x "$COWEN_BIN"
    
    # Create isolated app.yaml with absolute DB path
    cat > "$COWEN_HOME/app.yaml" <<EOF
storage:
  store: innerdb
  db_url: "sqlite://$COWEN_HOME/cowen.db"
log:
  level: debug
telemetry_enabled: false
ai_enabled: false
EOF
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
                PID=$(cat "$pid_file" 2>/dev/null)
                if [ -n "$PID" ]; then
                    echo "     Killing daemon PID $PID..."
                    kill -9 "$PID" >/dev/null 2>&1 || true
                    sleep 0.5
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
            pkill -9 -x "$(basename "$COWEN_BIN")" >/dev/null 2>&1 || true
        fi
        if [ -n "$COWEN_DAEMON_BIN" ]; then
            pkill -9 -x "$(basename "$COWEN_DAEMON_BIN")" >/dev/null 2>&1 || true
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

# Mock Server Management
start_mock() {
    if [ "$COWEN_MOCK_MANAGED" == "true" ]; then
        return 0
    fi
    echo -n "  Starting Mock Server on port $MOCK_PORT..."
    # Ensure port is free
    if [ "$IS_WINDOWS" = true ]; then
        local pid=$(netstat -ano | grep ":$MOCK_PORT" | grep "LISTENING" | awk '{print $5}' | head -n 1)
        if [ -n "$pid" ]; then taskkill //F //PID "$pid" >/dev/null 2>&1 || true; fi
    else
        lsof -ti ":$MOCK_PORT" | xargs kill -9 2>/dev/null || true
    fi
    sleep 1
    # Bind to 0.0.0.0 for container accessibility
    PYTHONUNBUFFERED=1 MOCK_PORT=$MOCK_PORT python3 -u tests/infra/mock_server.py > "$TEST_BASE/mock_server_$MOCK_PORT.log" 2>&1 &
    MOCK_PID=$!
    for i in {1..10}; do
        if curl -s $MOCK_URL/v1/mock/ping > /dev/null; then
            echo -e " ${GREEN}[READY]${NC}"
            return 0
        fi
        sleep 1
    done
    echo -e " ${RED}[TIMEOUT]${NC}"
    cat mock_server.log
    exit 1
}

# Token Extraction
extract_token() {
    local prof=$1
    shift
    local extra_args="$@"
    RUST_LOG=info "$COWEN_BIN" auth token --profile "$prof" $extra_args --format json 2>&1 | python3 -c "
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
"
}

# Global Cleanup
cleanup_all_workspaces() {
    # 1. Kill all cowen related processes
    if [ "$IS_WINDOWS" = true ]; then
        taskkill //F //IM cowen_*.exe >/dev/null 2>&1 || true
    else
        # Kill by pattern since each test has a unique binary name
        pkill -9 "cowen_" >/dev/null 2>&1 || true
    fi

    # 2. Kill all mock servers
    if [ "$IS_WINDOWS" = true ]; then
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
            echo -e "  ${YELLOW}[RETRY] Postgres busy/recovering (Attempt $attempt/$max_retries), waiting ${wait_time}s...${NC}"
            sleep $wait_time
            ((attempt++))
        else
            echo -e "  ${RED}[FATAL] Postgres error not retryable:${NC} $err"
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
    echo -n "  Waiting for PostgreSQL at $host:$port to be ready..."
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
                echo -e " ${GREEN}[READY]${NC}"
                return 0
            fi
        fi
        echo -n "."
        sleep 2
    done
    echo -e " ${RED}[TIMEOUT]${NC}"
    return 1
}

# Helper to wait for MySQL to be ready
wait_for_mysql() {
    local host="${1:-$DB_HOST}"
    local port="${2:-3306}"
    echo -n "  Waiting for MySQL at $host:$port to be ready..."
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
                echo -e " ${GREEN}[READY]${NC}"
                return 0
            fi
        fi
        echo -n "."
        sleep 2
    done
    echo -e " ${RED}[TIMEOUT]${NC}"
    return 1
}

# Helper to get an unused TCP port from the OS
get_unused_port() {
    python3 -c 'import socket; s=socket.socket(); s.bind(("", 0)); print(s.getsockname()[1]); s.close()'
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
