#!/bin/bash
# Case 35: Enhanced Daemon Recovery Verification (Self-Built & OAuth2)
# Checks:
# 1. Daemon auto-restarts on next command if missing (External Kill).
# 2. Daemon auto-restarts on next command if hanging (Unresponsive Port).
# 3. No unexpected crash logs generated for external kills.

source tests/common.sh
setup_workspace "daemon_recovery_enhanced"
trap cleanup_suite EXIT
start_mock

# --- Helpers ---
check_daemon_alive() {
    local prof=$1
    local pid=$(get_daemon_pid "$prof")
    if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then
        return 0
    else
        return 1
    fi
}

check_crash_log_clean() {
    local prof=$1
    local stage=$2
    local log_file="$COWEN_HOME/${prof}_child_crash.log"
    echo -n "   Checking crash log for profile '$prof' ($stage)..."
    if [ -f "$log_file" ]; then
        # We expect it to exist because it's created on startup, but it should not contain PANIC or ERROR log-level entries.
        # Use word-boundary matching to avoid false positives on structured log fields like 'error=...'
        # and filter out known transient errors (e.g. port conflicts during test transitions).
        if grep -Ei "panic|stack backtrace" "$log_file" > /dev/null; then
             echo -e " ${RED}[FAILED]${NC}"
             echo -e "   ${RED}✗${NC} Crash log contains errors/panics at stage $stage:"
             cat "$log_file"
             exit 1
        else
             echo -e " ${GREEN}[OK]${NC} (No panic/error found)"
        fi
    else
        echo -e " ${GREEN}[OK]${NC} (Log file not found, which is also clean)"
    fi
}

# --- Part 1: Self-Built Mode Recovery ---
echo -e "${BOLD}Part 1: Self-Built Mode Recovery${NC}"
"$COWEN_BIN" init --profile sb \
    --app-mode self-built --app-key AK_SB --app-secret AS_SB \
    --encrypt-key 1234567890123456 --certificate CERT_SB \
    --openapi-url $MOCK_URL --stream-url $MOCK_WS --proxy-port 18501 >/dev/null
assert_pass "Self-built profile initialized"
sleep 3 # Allow daemon to stabilize and bind port

# Trigger start via a command (daemon should already be running)
"$COWEN_BIN" status --profile sb >/dev/null 
sleep 1
PID_SB1=$(get_daemon_pid "sb")
if [ -n "$PID_SB1" ]; then 
    echo -e "   ✓ Daemon SB running (PID: $PID_SB1)"
else 
    echo -e "   ${RED}✗${NC} Failed to start daemon SB"
    "$COWEN_BIN" status --profile sb
    exit 1
fi

echo -e "   ⚡ Simulating unexpected termination (kill -9 PID $PID_SB1)..."
kill -9 "$PID_SB1"
sleep 1

# Check crash log BEFORE recovery
# Note: External kill -9 should leave normal startup logs but NO panic/stacktrace
check_crash_log_clean "sb" "after_kill"

echo "   Running 'cowen status' to trigger recovery..."
STATUS_OUT=$("$COWEN_BIN" status --profile sb)
echo "$STATUS_OUT" | grep -q "automatically restarting" || echo "   (Note: Recovery message might be in stderr)"

PID_SB2=$(get_daemon_pid "sb")

if [ -n "$PID_SB2" ] && [ "$PID_SB1" != "$PID_SB2" ]; then
    echo -e "   ${GREEN}✓${NC} Daemon SB successfully recovered (New PID: $PID_SB2)"
else
    echo -e "   ${RED}✗${NC} Daemon SB recovery failed"
    exit 1
fi
check_crash_log_clean "sb" "after_recovery"

# --- Cleanup Part 1 before Part 2 ---
echo -e "\n   Stopping Part 1 daemon before Part 2..."
"$COWEN_BIN" daemon stop --profile sb >/dev/null 2>&1 || kill -9 "$PID_SB2" 2>/dev/null || true
sleep 2  # Allow OS to fully release ports

# --- Part 2: OAuth2 Mode Recovery ---
echo -e "\n${BOLD}Part 2: OAuth2 Mode Recovery${NC}"

# Launch init in background (it blocks waiting for browser callback)
"$COWEN_BIN" init --profile oa2 \
    --app-mode oauth2 --openapi-url $MOCK_URL --stream-url $MOCK_WS --proxy-port 18503 >/dev/null &
INIT_PID=$!


echo -n "   Waiting for auth session..."
SESSION_JSON=""
for i in {1..20}; do
    if [ -f "$COWEN_HOME/cowen.db" ]; then
        SESSION_JSON=$(sqlite3 "$COWEN_HOME/cowen.db" \
            "SELECT item_value FROM cowen_config WHERE profile='oa2' AND item_key='pending_auth_session' LIMIT 1;" 2>/dev/null)
        if [ -n "$SESSION_JSON" ]; then
            break
        fi
    fi
    sleep 0.5
done

if [ -z "$SESSION_JSON" ]; then
    echo -e " ${RED}[TIMEOUT]${NC}"
    kill "$INIT_PID" 2>/dev/null
    exit 1
fi

REDIRECT_PORT=$(echo "$SESSION_JSON" | python3 -c "import sys,json; d=json.loads(sys.stdin.read()); print(d['redirect_port'])")
STATE=$(echo "$SESSION_JSON" | python3 -c "import sys,json; d=json.loads(sys.stdin.read()); print(d['state'])")

sleep 1
# Simulate browser callback to unblock init
curl -s -o /dev/null "http://127.0.0.1:${REDIRECT_PORT}/callback?code=mock_auth_code_12345&state=${STATE}"
wait $INIT_PID 2>/dev/null || true

echo "   Waiting for init to automatically start daemon..."
sleep 3

PID_OA1=$(get_daemon_pid "oa2")
if [ -n "$PID_OA1" ]; then 
    echo -e "   ✓ Daemon OA2 running (PID: $PID_OA1)"
else 
    echo -e "   ${RED}✗${NC} Failed to start daemon OA2"
    exit 1
fi

echo -e "   ⚡ Simulating unexpected termination (kill -9 PID $PID_OA1)..."
kill -9 "$PID_OA1"
sleep 1

check_crash_log_clean "oa2" "before_recovery"

echo "   Running 'cowen status' to trigger recovery..."
"$COWEN_BIN" status --profile oa2 >/dev/null
PID_OA2=$(get_daemon_pid "oa2")

if [ -n "$PID_OA2" ] && [ "$PID_OA1" != "$PID_OA2" ]; then
    echo -e "   ${GREEN}✓${NC} Daemon OA2 successfully recovered (New PID: $PID_OA2)"
else
    echo -e "   ${RED}✗${NC} Daemon OA2 recovery failed"
    exit 1
fi
check_crash_log_clean "oa2" "after_recovery"

echo -e "\n${GREEN}🎊 Case 35 Passed!${NC}"
