#!/usr/bin/env bash
# case_58_reset_reinit_sqlite.sh
# Verification of SQLite WAL and lock cleanup during system reset

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

PROFILE="case_58_reset"
setup_workspace "case_58_$PROFILE"

echo "Initializing profile to generate DB, WAL, SHM, and Lock files..."
"$COWEN_BIN" -p "$PROFILE" init --app-key "test-key" --app-secret "test-secret" --certificate "test-cert" --app-mode "self_built" --encrypt-key "test-encrypt-key"

# Ensure SQLite files exist. Usually, because we ran init, WAL files are generated.
# Let's verify files exist before reset
echo "Checking if profile configuration and databases were created..."
if [ ! -f "$COWEN_HOME/$PROFILE.yaml" ]; then
    fail_suite "Profile config file $PROFILE.yaml was not created!"
fi

if [ ! -f "$COWEN_HOME/cowen.db" ]; then
    fail_suite "Profile DB file cowen.db was not created!"
fi

# Fake/trigger telemetry database creation to ensure telemetry.db and sidecar files exist
touch "$COWEN_HOME/telemetry.db"
touch "$COWEN_HOME/telemetry.db-wal"
touch "$COWEN_HOME/telemetry.db-shm"

# Let's also create dummy db-wal, db-shm, and lock files for the profile to simulate them
touch "$COWEN_HOME/cowen.db-wal"
touch "$COWEN_HOME/cowen.db-shm"
touch "$COWEN_HOME/cowen.ddl.lock"

echo "Running system reset..."
"$COWEN_BIN" reset

echo "Verifying all SQLite files, lock files, and configurations are completely gone..."
REMAINING=""
for ext in "db" "db-wal" "db-shm"; do
    if [ -f "$COWEN_HOME/cowen.$ext" ]; then
        REMAINING="$REMAINING cowen.$ext"
    fi
done

if [ -f "$COWEN_HOME/cowen.ddl.lock" ]; then REMAINING="$REMAINING cowen.ddl.lock"; fi
if [ -f "$COWEN_HOME/$PROFILE.yaml" ]; then REMAINING="$REMAINING $PROFILE.yaml"; fi
if [ -f "$COWEN_HOME/telemetry.db" ]; then REMAINING="$REMAINING telemetry.db"; fi
if [ -f "$COWEN_HOME/telemetry.db-wal" ]; then REMAINING="$REMAINING telemetry.db-wal"; fi
if [ -f "$COWEN_HOME/telemetry.db-shm" ]; then REMAINING="$REMAINING telemetry.db-shm"; fi

if [ -n "$REMAINING" ]; then
    fail_suite "The following files were NOT deleted by reset:$REMAINING"
fi

echo "Attempting to re-initialize profile to verify SQLite initializes successfully without disk I/O error..."
"$COWEN_BIN" -p "$PROFILE" init --app-key "new-test-key" --app-secret "new-test-secret" --certificate "new-test-cert" --app-mode "self_built" --encrypt-key "new-test-encrypt-key"

pass_suite

