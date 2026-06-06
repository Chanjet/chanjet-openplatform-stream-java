#!/bin/bash
set -e
# Case 77: Status command does not report Stale for oauth2 mode
#
# Regression test for SYKFPT-1093. Oauth2 profiles do not have stream
# heartbeats, so their _status.json does not update frequently.
# The CLI should not mistakenly apply the 60s freshness check to it.

if [ -f "crates/app/cowen-cli/tests/e2e/scripts/common.sh" ]; then
    source crates/app/cowen-cli/tests/e2e/scripts/common.sh
else
    source "$(dirname "$0")/common.sh"
fi
PROXY_PORT=$(get_unused_port)

setup_workspace "case_77"
trap cleanup_suite EXIT
start_mock

PROF="oa2stale"

# ============================================================
# Phase 1: OAuth2 Init (Full PKCE Authorization Flow)
# ============================================================

echo -e "${BOLD}1. OAuth2 Init (PKCE Flow)${NC}"

"$COWEN_BIN" init --profile "$PROF" \
    --app-mode oauth2 \
    --openapi-url "$MOCK_URL" \
    --stream-url "$MOCK_WS" \
    --proxy-port $PROXY_PORT \
    --webhook-target "http://127.0.0.1:8080/cb" > "$COWEN_HOME/init.log" 2>&1 &
INIT_PID=$!

echo "   Init PID: $INIT_PID (blocking, waiting for browser callback)"

SESSION_JSON=""
for i in {1..30}; do
    if [ -f "$COWEN_HOME/cowen.db" ]; then
        SESSION_JSON=$(sqlite3 "$COWEN_HOME/cowen.db" \
            "SELECT item_value FROM cowen_token WHERE profile='global' AND item_key LIKE 'session:%' LIMIT 1;" 2>/dev/null)
        if [ -n "$SESSION_JSON" ]; then
            break
        fi
    fi
    sleep 0.5
done

if [ -z "$SESSION_JSON" ]; then
    kill "$INIT_PID" 2>/dev/null
    fail_suite "[TIMEOUT] waiting for auth session"
fi

REDIRECT_PORT=$(get_json_field "$SESSION_JSON" "redirect_port")
STATE=$(get_json_field "$SESSION_JSON" "state")

sleep 2

curl -s -o /dev/null "http://127.0.0.1:${REDIRECT_PORT}/callback?code=mock_auth_code_12345&state=${STATE}"

for i in {1..15}; do
    if ! kill -0 "$INIT_PID" 2>/dev/null; then
        break
    fi
    sleep 1
done

if kill -0 "$INIT_PID" 2>/dev/null; then
    kill "$INIT_PID" 2>/dev/null
    fail_suite "[TIMEOUT - init still blocking]"
fi

# ============================================================
# Phase 2: Start Daemon and Manually Stale the Status
# ============================================================

echo -e "\n${BOLD}2. Testing Status Freshness Exemption${NC}"

"$COWEN_BIN" daemon start --profile "$PROF" >/dev/null
sleep 2

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

# Run status again. It should NOT be Stale for oauth2!
STATUS_OUT=$("$COWEN_BIN" status -p "$PROF")
if echo "$STATUS_OUT" | grep -q "Stale"; then
    echo "$STATUS_OUT"
    fail_suite "Oauth2 status showed 'Stale' despite being exempt from webhooks!"
else
    echo -e "  ${GREEN}✓${NC} Oauth2 status correctly ignored freshness requirement."
fi

echo -e "\n${GREEN}🎊 Case 77 Passed!${NC}"
