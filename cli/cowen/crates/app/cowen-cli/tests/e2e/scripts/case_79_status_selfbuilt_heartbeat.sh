#!/bin/bash
set -e
# Case 79: Status command does not report Stale for selfbuilt mode because of heartbeat
#
# Regression test for the heartbeat bug where self-built profiles with
# streams become Stale after 60 seconds of inactivity.

if [ -f "crates/app/cowen-cli/tests/e2e/scripts/common.sh" ]; then
    source crates/app/cowen-cli/tests/e2e/scripts/common.sh
else
    source "$(dirname "$0")/common.sh"
fi
PROXY_PORT=$(get_unused_port)

setup_workspace "case_79"
trap cleanup_suite EXIT
start_mock

PROF="sb_stale"

# ============================================================
# Phase 1: Self-built Init
# ============================================================

echo -e "${BOLD}1. Selfbuilt Init${NC}"

"$COWEN_BIN" init --profile "$PROF" \
    --app-mode self-built \
    --app-key "mock_key_12345" \
    --app-secret "mock_secret_67890" \
    --encrypt-key "1234567890123456" \
    --certificate "mock_cert" \
    --openapi-url "$MOCK_URL" \
    --stream-url "$MOCK_WS" \
    --proxy-port $PROXY_PORT \
    --webhook-target "http://127.0.0.1:8080/cb" > "$COWEN_HOME/init.log" 2>&1

# ============================================================
# Phase 2: Start Daemon and Test Heartbeat
# ============================================================

echo -e "\n${BOLD}2. Testing Status Heartbeat Recovery${NC}"

"$COWEN_BIN" daemon start --profile "$PROF" >/dev/null
sleep 3

# Status should be active normally
STATUS_OUT=$("$COWEN_BIN" status -p "$PROF")
if echo "$STATUS_OUT" | grep -q "(Stale)"; then
    fail_suite "Status was immediately Stale?"
fi

# Now artificially age the status file by 2 hours
STATUS_FILE="$COWEN_HOME/${PROF}_status.json"
echo "   Aging status file $STATUS_FILE..."
python3 -c "
import json
import datetime
import os

filepath = '$STATUS_FILE'
if os.path.exists(filepath):
    with open(filepath, 'r') as f:
        data = json.load(f)
    # Set updated_at to 2 hours ago
    past = datetime.datetime.utcnow() - datetime.timedelta(hours=2)
    data['updated_at'] = past.strftime('%Y-%m-%dT%H:%M:%SZ')
    with open(filepath, 'w') as f:
        json.dump(data, f)
"

# Status should be Stale IMMEDIATELY
STATUS_OUT=$("$COWEN_BIN" status -p "$PROF")
if ! echo "$STATUS_OUT" | grep -q "Stale"; then
    echo "$STATUS_OUT"
    fail_suite "Selfbuilt status should show 'Stale' immediately after aging!"
else
    echo -e "  ${GREEN}✓${NC} Selfbuilt status correctly showed 'Stale'."
fi

# Wait for 35 seconds to allow heartbeat to fire
echo "   Waiting for 35 seconds for heartbeat to self-heal..."
sleep 35

# Status should NO LONGER be Stale!
STATUS_OUT=$("$COWEN_BIN" status -p "$PROF")
if echo "$STATUS_OUT" | grep -q "Stale"; then
    echo "$STATUS_OUT"
    fail_suite "Heartbeat failed to self-heal the status file!"
else
    echo -e "  ${GREEN}✓${NC} Selfbuilt status correctly self-healed via heartbeat."
fi

echo -e "\n${GREEN}🎊 Case 79 Passed!${NC}"
