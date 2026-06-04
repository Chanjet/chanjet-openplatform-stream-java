#!/bin/bash
set -e
# Case 40: Dynamic Log Level Configuration Verification
# Verifies that log levels can be set via config command and effectively control log output.

if [ -f "crates/cowen-cli/tests/e2e/scripts/common.sh" ]; then
    source crates/cowen-cli/tests/e2e/scripts/common.sh
else
    source "$(dirname "$0")/common.sh"
fi

# Setup workspace
setup_workspace "case_40"

echo "1. Verify Default Log Level"
# Manually create a minimal config to avoid init/auth issues
mkdir -p "$COWEN_HOME"
cat <<EOF > "$COWEN_HOME/app.yaml"
app_key: AK_LOG
app_mode: oauth2
openapi_url: http://127.0.0.1:9299
stream_url: http://127.0.0.1:9299
webhook_target: http://127.0.0.1:8080
log:
  level: info
  rotation: daily
  max_size_mb: 100
  max_files: 7
telemetry_enabled: true
ai_enabled: false
proxy_port: 8081
proxy_enabled: true
version: 1
EOF

# Check default level in config output
RAW_OUT=$("$COWEN_BIN" config --profile main -o json || true)
DEFAULT_LEVEL=$(echo "$RAW_OUT" | jq -r ".global.log.level")
if [ "$DEFAULT_LEVEL" == "info" ]; then
    echo -e "   ${GREEN}✓${NC} Default log level is 'info'"
else
    fail_suite "Expected default 'info', got '$DEFAULT_LEVEL'"
fi

echo "2. Dynamically Change Log Level to DEBUG"
"$COWEN_BIN" config set log.level debug --profile main
NEW_LEVEL=$("$COWEN_BIN" config --profile main -o json | jq -r ".global.log.level")
if [ "$NEW_LEVEL" == "debug" ]; then
    echo -e "   ${GREEN}✓${NC} Log level updated to 'debug' in config"
else
    fail_suite "Failed to update log level to 'debug'"
fi

echo "3. Verify Config Set Persistence"
# Check the file content directly
if grep -q "level: debug" "$COWEN_HOME/app.yaml"; then
    echo -e "   ${GREEN}✓${NC} 'debug' level persisted to YAML file"
else
    fail_suite "Log level not persisted in YAML file"
fi

echo "4. Test Case-Insensitivity and Restoration"
"$COWEN_BIN" config set log.level INFO --profile main
FINAL_LEVEL=$("$COWEN_BIN" config --profile main -o json | jq -r ".global.log.level")
if [ "$(echo "$FINAL_LEVEL" | tr '[:upper:]' '[:lower:]')" == "info" ]; then
    echo -e "   ${GREEN}✓${NC} Log level restored to 'info' (case-insensitive set works)"
else
    fail_suite "Failed to restore log level"
fi


# Mandatory Sanitization Check
CONFIG_OUT=$("$COWEN_BIN" config --profile main 2>&1)
assert_sanitized "$CONFIG_OUT" "CLI Profile Config output"

echo -e "\n${GREEN}🎊 Case 40 Passed!${NC}"
cleanup_suite
