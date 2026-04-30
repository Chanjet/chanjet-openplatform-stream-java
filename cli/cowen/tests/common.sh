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

# Binary and URL defaults
export COWEN_BIN="./target/debug/cowen"
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
    echo -e "${YELLOW}  Cleaning up daemons...${NC}"
    "$COWEN_BIN" daemon stop --all >/dev/null 2>&1 || true
    lsof -ti :9091,9092,9093 | xargs kill -9 2>/dev/null || true
    if [ "$COWEN_MOCK_MANAGED" != "true" ]; then
        lsof -ti :9299 | xargs kill -9 2>/dev/null || true
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
    lsof -ti :9299 | xargs kill -9 2>/dev/null || true
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
