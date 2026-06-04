#!/bin/bash
# E2E Test: Phase 4 IPC Auth Sync (Case 51)
# Reference: cli/cowen/docs/WBS.md

if [ -f "crates/cowen-cli/tests/e2e/scripts/common.sh" ]; then
    source crates/cowen-cli/tests/e2e/scripts/common.sh
else
    source "$(dirname "$0")/common.sh"
fi

# Setup: Isolated environment
setup_workspace "case_51"
start_mock

echo "--- Test 1: Start Master Daemon ---"
# We need a running daemon to test IPC
"$COWEN_BIN" daemon start --foreground > "$TEST_BASE/master.log" 2>&1 &
DAEMON_PID=$!

echo "   Waiting for daemon to be ready..."
sleep 3

# Check for Monitor Port in PID file
PID_FILE="$COWEN_HOME/master_daemon.pid"
if [ ! -f "$PID_FILE" ]; then
    cat "$TEST_BASE/master.log"
    fail_suite "master_daemon.pid not found"
fi

MONITOR_PORT=$(grep "MONITOR_PORT=" "$PID_FILE" | cut -d'=' -f2)
if [ -z "$MONITOR_PORT" ]; then
    cat "$PID_FILE"
    fail_suite "MONITOR_PORT not found in PID file"
fi
echo "   Monitor Port detected: $MONITOR_PORT"

echo "--- Test 2: Trigger OAuth2 Init Flow ---"
# Initialize OAuth2 profile (non-interactive simulation)
# We pipe to a file to capture the redirect URL
"$COWEN_BIN" init \
    --app-mode oauth2 \
    --openapi-url "$MOCK_URL" \
    --stream-url "$MOCK_WS" \
    --webhook-target "$MOCK_URL/webhook_sink" \
    > "$TEST_BASE/init_output.log" 2>&1 &
INIT_PID=$!

echo "   Wait for init to start listener and print URL..."
PORT=""
for i in {1..20}; do
    URL=$(grep "redirect_uri=" "$TEST_BASE/init_output.log" | head -n 1)
    if [ -n "$URL" ]; then
        # Extract port from http%3A%2F%2F127.0.0.1%3A16094%2Fcallback
        PORT=$(echo "$URL" | grep -oE "127.0.0.1(%3A|:)[0-9]+" | sed 's/127.0.0.1//' | sed 's/%3A//' | sed 's/://')
        STATE=$(echo "$URL" | grep -oE "state=[^&]+" | cut -d= -f2)
        break
    fi
    sleep 0.5
done

if [ -z "$PORT" ]; then
    cat "$TEST_BASE/init_output.log"
    fail_suite "Could not detect redirect port from init output"
fi
echo "   Detected Redirect Port: $PORT, State: $STATE"

echo "--- Test 3: Simulate Browser Callback ---"
# Send mock callback to the CLI's local listener
# The CLI should then forward this to the Daemon via IPC
CODE="mock_auth_code_case_56"
curl -s "http://127.0.0.1:$PORT/callback?code=$CODE&state=$STATE" > /dev/null

echo "--- Test 4: Verify IPC Forwarding and Progress ---"
# Wait for the init process to complete via IPC sync
wait $INIT_PID
EXIT_CODE=$?

echo "   Init exited with code: $EXIT_CODE"
if [ "$EXIT_CODE" != "0" ]; then
    cat "$TEST_BASE/init_output.log"
    fail_suite "Init did not exit with code 0"
fi

# Verify IPC Forwarding and Progress
# (The CLI is now a pure dumb client, so it ALWAYS uses IPC and does not print the detection message anymore)

# Confirm login was successful
if ! grep -q "Login successful" "$TEST_BASE/init_output.log"; then
    cat "$TEST_BASE/init_output.log"
    fail_suite "Init did not complete login successfully"
fi

echo "--- Test 5: Verify Token in Vault ---"
# Check if the token was actually saved by the daemon
# We wait a bit to ensure the daemon finished writing to storage
sleep 1
TOKEN_CHECK=$("$COWEN_BIN" status | grep "AccessToken")
if echo "$TOKEN_CHECK" | grep -q "VALID"; then
    echo "   ✓ Token successfully synchronized via IPC"
else
    "$COWEN_BIN" status
    fail_suite "Token not found or invalid in status output"
fi


# Mandatory Sanitization Check
CONFIG_OUT=$("$COWEN_BIN" config --profile main 2>&1)
assert_sanitized "$CONFIG_OUT" "CLI Profile Config output"

echo -e "\n${GREEN}🎊 Case 51 Passed!${NC}"
cleanup_suite
exit 0
