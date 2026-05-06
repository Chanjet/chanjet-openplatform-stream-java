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
if [[ "$OS_TYPE" == *"MINGW"* || "$OS_TYPE" == *"MSYS"* || "$OS_TYPE" == *"CYGWIN"* ]]; then
    export COWEN_BIN="./target/debug/cowen.exe"
    export IS_WINDOWS=true
else
    export COWEN_BIN="./target/debug/cowen"
    export IS_WINDOWS=false
fi

export MOCK_PORT="${MOCK_PORT:-9299}"
export MOCK_URL="http://127.0.0.1:$MOCK_PORT"
export MOCK_WS="ws://127.0.0.1:$MOCK_PORT"
export COWEN_RAW_OUTPUT="true"
export COWEN_EXCLUSIVE="false"

# Database Isolation
get_case_db_name() {
    local suite=$1
    # Replace dots and dashes with underscores to ensure valid DB name
    echo "cowen_test_$(echo $suite | tr '.-' '__')"
}

# Isolation
setup_workspace() {
    local suite=$1
    export TEST_BASE="$(pwd)/target/cowen_tests"
    export COWEN_HOME="$TEST_BASE/.cowen_test_$suite"
    echo -e "${BLUE}▶ Starting Suite: $suite${NC}"
    echo -e "  Workspace: $COWEN_HOME"
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
    cp "tests/fixtures/$name.yaml" "$COWEN_HOME/$prof.yaml"
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
                fi
                rm -f "$pid_file" >/dev/null 2>&1 || true
            done
        fi

        # 1.5 Global pkill as fallback (Robustness)
        if [ "$COWEN_MOCK_MANAGED" == "true" ]; then
             pkill -9 cowen >/dev/null 2>&1 || true
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
    MOCK_PORT=$MOCK_PORT python3 tests/mock_server.py > "target/cowen_tests/mock_server_$MOCK_PORT.log" 2>&1 &
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
    "$COWEN_BIN" auth token --profile "$prof" --format json 2>/dev/null | python3 -c "
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
    # 3. Handle workspaces - Always cleanup the large workspace directories to keep env clean
    rm -rf target/cowen_tests/.cowen_test_*
    rm -f target/cowen_tests/.cowen_test_*.db target/cowen_tests/.cowen_test_*.db-shm target/cowen_tests/.cowen_test_*.db-wal
    if [ "$IS_WINDOWS" = true ]; then
        taskkill //F //IM cowen.exe >/dev/null 2>&1 || true
        local pid=$(netstat -ano | grep ":9299" | grep "LISTENING" | awk '{print $5}' | head -n 1)
        if [ -n "$pid" ]; then taskkill //F //PID "$pid" >/dev/null 2>&1 || true; fi
    else
        pkill -9 cowen >/dev/null 2>&1 || true
        lsof -ti :9299 | xargs kill -9 2>/dev/null || true
    fi
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
