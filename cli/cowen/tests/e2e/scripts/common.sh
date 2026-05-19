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
find_bin() {
    if [[ "$OS_TYPE" == *"MINGW"* || "$OS_TYPE" == *"MSYS"* || "$OS_TYPE" == *"CYGWIN"* ]]; then
        if [ -f "./target/release/cowen-test.exe" ]; then echo "./target/release/cowen-test.exe";
        elif [ -f "./target/debug/cowen-test.exe" ]; then echo "./target/debug/cowen-test.exe";
        elif [ -f "./target/release/cowen.exe" ]; then echo "./target/release/cowen.exe";
        else echo "./target/debug/cowen.exe"; fi
    else
        if [ -f "./target/release/cowen-test" ]; then echo "./target/release/cowen-test";
        elif [ -f "./target/debug/cowen-test" ]; then echo "./target/debug/cowen-test";
        elif [ -f "./target/release/cowen" ]; then echo "./target/release/cowen";
        else echo "./target/debug/cowen"; fi
    fi
}
export COWEN_BIN="${COWEN_BIN:-$(find_bin)}"
export COWEN_BUILD_DIR=$(dirname "$COWEN_BIN")

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
# Force PG_CMD to include host
export PG_CMD="psql -h $DB_HOST -U postgres"
export PGPASSWORD="${PGPASSWORD:-password}"

# Database Isolation
get_case_db_name() {
    local suite=$1
    # Replace dots and dashes with underscores to ensure valid DB name
    echo "cowen_test_$(echo $suite | tr '.-' '__')"
}

# Isolation
setup_workspace() {
    local suite=$1
    export TEST_BASE="${TEST_BASE:-$(pwd)/target/cowen_tests}"
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
    fi

    rm -rf "$COWEN_HOME"
    mkdir -p "$COWEN_HOME"
    
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
        if [ "$COWEN_MOCK_MANAGED" == "true" ]; then
             pkill -9 cowen-test >/dev/null 2>&1 || true
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

        # 4. Remove workspace directory
        if [ -n "$COWEN_HOME" ] && [[ "$COWEN_HOME" == *"_test_"* ]]; then
            rm -rf "$COWEN_HOME" >/dev/null 2>&1 || true
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
    MOCK_PORT=$MOCK_PORT python3 tests/infra/mock_server.py > "$TEST_BASE/mock_server_$MOCK_PORT.log" 2>&1 &
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
    "$COWEN_BIN" auth token --profile "$prof" $extra_args --format json 2>/dev/null | python3 -c "
import sys, json
raw = sys.stdin.read()
try:
    # Try to find the JSON part if there is noise
    start = raw.find('{')
    end = raw.rfind('}') + 1
    if start >= 0 and end > start:
        d = json.loads(raw[start:end])
        print(d.get('access_token') or d.get('value') or '')
    else:
        print('')
except:
    print('')
"
}

# Global Cleanup
cleanup_all_workspaces() {
    # 1. Kill all cowen related processes
    if [ "$IS_WINDOWS" = true ]; then
        taskkill //F //IM cowen-test.exe >/dev/null 2>&1 || true
    else
        # Kill ONLY the test binary name
        pkill -9 cowen-test >/dev/null 2>&1 || true
        # Also kill by path if it contains cowen-test
        pkill -9 -f "cowen-test" >/dev/null 2>&1 || true
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
    local pid_file="$COWEN_HOME/${prof}_daemon.pid"
    if [ -f "$pid_file" ]; then
        cat "$pid_file" | head -n 1
    else
        echo ""
    fi
}
