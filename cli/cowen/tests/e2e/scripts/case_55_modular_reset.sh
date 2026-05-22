#!/usr/bin/env bash
# case_55_modular_reset.sh
# Tests the modular reset functionality (cowen reset --dry-run vs actual reset)

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

PROFILE="case_55_reset"
setup_workspace "$PROFILE"

# Setup dummy config
echo "Creating dummy config..."
"$COWEN_BIN" -p "$PROFILE" init --app-key "dummy-reset-key" --app-secret "dummy-secret" --certificate "dummy-cert" --app-mode "self_built" --encrypt-key "dummy-encrypt-key"

# Generate some telemetry data (fake it)
touch "$COWEN_HOME/telemetry.db"
mkdir -p "$COWEN_HOME/logs"
touch "$COWEN_HOME/logs/test.log"

echo "Running dry-run reset..."
DRY_RUN_OUT=$("$COWEN_BIN" reset --dry-run)

echo "$DRY_RUN_OUT"

if ! echo "$DRY_RUN_OUT" | grep -q "\[DRY RUN\]"; then
    echo "❌ Dry-run output missing DRY RUN header"
    exit 1
fi

if ! echo "$DRY_RUN_OUT" | grep -q "telemetry.db"; then
    echo "❌ Dry-run output missing telemetry.db deletion plan"
    exit 1
fi

# Verify files still exist
if [ ! -f "$COWEN_HOME/telemetry.db" ]; then
    echo "❌ telemetry.db was deleted during dry-run!"
    exit 1
fi

echo "Running actual reset..."
"$COWEN_BIN" reset

# Verify files are deleted
if [ -f "$COWEN_HOME/telemetry.db" ]; then
    echo "❌ telemetry.db was NOT deleted!"
    exit 1
fi

if [ -d "$COWEN_HOME/logs" ]; then
    echo "❌ logs directory was NOT deleted!"
    exit 1
fi

echo "✅ Modular reset test Passed!"
