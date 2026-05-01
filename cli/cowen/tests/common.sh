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

export MOCK_URL="http://127.0.0.1:9299"
export MOCK_WS="ws://127.0.0.1:9299"
export COWEN_RAW_OUTPUT="true"

# Isolation
setup_workspace() {
    local suite=$1
    export COWEN_HOME="$(pwd)/.cowen_test_$suite"
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
    echo -e "${YELLOW}  Cleaning up daemons and environment...${NC}"
    
    # 1. Force kill all cowen processes (daemons and any stuck CLI commands)
    if [ "$IS_WINDOWS" = true ]; then
        taskkill //F //IM cowen.exe >/dev/null 2>&1 || true
    else
        pkill -9 cowen >/dev/null 2>&1 || true
    fi
    
    # 2. Cleanup mock server state for next case
    if [ "$COWEN_MOCK_MANAGED" == "true" ]; then
        curl -s -X POST "$MOCK_URL/control/kill_connections" >/dev/null 2>&1 || true
        curl -s -X POST "$MOCK_URL/control/clear_webhooks" >/dev/null 2>&1 || true
    fi
    
    # 3. Double check and kill anything on known ports
    kill_port() {
        local port=$1
        if [ "$IS_WINDOWS" = true ]; then
            local pid=$(netstat -ano | grep ":$port" | grep "LISTENING" | awk '{print $5}' | head -n 1)
            if [ -n "$pid" ]; then taskkill //F //PID "$pid" >/dev/null 2>&1 || true; fi
        else
            pids=$(lsof -ti ":$port" 2>/dev/null) || true
            if [ -n "$pids" ]; then
                echo "$pids" | xargs kill -9 >/dev/null 2>&1 || true
            fi
        fi
    }

    for p in 9091 9092 9093 9901 9902 9903 9908 9909; do
        kill_port $p
    done
    
    if [ "$COWEN_MOCK_MANAGED" != "true" ]; then
        kill_port 9299
    fi

    # 4. Remove workspace directory if it's a test one
    if [ -n "$COWEN_HOME" ] && [[ "$COWEN_HOME" == *"_test_"* ]]; then
        rm -rf "$COWEN_HOME"
    fi
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
    echo -n "  Starting Mock Server..."
    # Ensure port is free
    if [ "$IS_WINDOWS" = true ]; then
        local pid=$(netstat -ano | grep ":9299" | grep "LISTENING" | awk '{print $5}' | head -n 1)
        if [ -n "$pid" ]; then taskkill //F //PID "$pid" >/dev/null 2>&1 || true; fi
    else
        lsof -ti :9299 | xargs kill -9 2>/dev/null || true
    fi
    sleep 1
    python3 tests/mock_server.py > mock_server.log 2>&1 &
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
    "$COWEN_BIN" auth token --profile "$prof" --format json 2>/dev/null | python3 -c "import sys, json; d=json.loads(sys.stdin.read() or '{}'); print(d.get('access_token') or d.get('value') or '')"
}

# Global Cleanup
cleanup_all_workspaces() {
    echo -e "\n${BLUE}🧹 Cleaning up temporary test workspaces...${NC}"
    rm -rf .cowen_test_*
    # Force kill all processes
    if [ "$IS_WINDOWS" = true ]; then
        taskkill //F //IM cowen.exe >/dev/null 2>&1 || true
        local pid=$(netstat -ano | grep ":9299" | grep "LISTENING" | awk '{print $5}' | head -n 1)
        if [ -n "$pid" ]; then taskkill //F //PID "$pid" >/dev/null 2>&1 || true; fi
    else
        pkill -9 cowen >/dev/null 2>&1 || true
        lsof -ti :9299 | xargs kill -9 2>/dev/null || true
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
