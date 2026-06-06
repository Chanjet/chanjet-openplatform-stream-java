#!/bin/bash
source "$(dirname "$0")/common.sh"
setup_workspace "case_72_no_disk_ipc"

start_mock

echo "  Initializing workspace..."
"$COWEN_BIN" init --profile main \
    --app-mode self-built \
    --app-key AK_SB \
    --app-secret AS_SB \
    --encrypt-key 1234567890123456 \
    --certificate CERT_SB \
    --webhook-target "http://127.0.0.1:8080/cb" \
    --openapi-url $MOCK_URL \
    --stream-url $MOCK_WS >/dev/null

echo "  Starting daemon..."
"$COWEN_BIN" daemon start --profile main >/dev/null
wait_for_daemon main 10

echo "  Checking system status to verify IPC is functional..."
STATUS_OUT=$("$COWEN_BIN" system status 2>&1)
if ! echo "$STATUS_OUT" | grep -q "Daemon Build:"; then
    fail_suite "Failed to retrieve system status. Daemon IPC is likely broken. Output: $STATUS_OUT"
fi

echo "  Verifying ipc.port and ipc.token files do NOT exist on disk..."
if [ -f "$COWEN_HOME/profiles/main/app/ipc.port" ]; then
    fail_suite "Found ipc.port file on disk. This should no longer be written!"
fi

if [ -f "$COWEN_HOME/profiles/main/app/ipc.token" ]; then
    fail_suite "Found ipc.token file on disk. This should no longer be written!"
fi

echo -e "  ${GREEN}✓${NC} ipc.port and ipc.token disk dependence successfully eliminated"

echo -e "\n🎊 Case 72 Passed!\n"
exit 0
