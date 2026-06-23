#!/usr/bin/env bash
# scripts/run_tests_with_coverage.sh
# Runs workspace tests (unit, integration) and E2E parallel tests with LLVM instrumented coverage,
# merges profraw traces, and runs scripts/coverage_gate.py to enforce dynamic coverage quality gate.

set -e

# Make sure we are in the workspace root
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR/.."

# Maximize file descriptor limit for high concurrency testing
ulimit -n 65536 2>/dev/null || ulimit -n 10240 2>/dev/null || true

# 1. Prepare local databases if running locally on macOS/Linux (excluding Docker/Podman environments)
if [ -z "$IN_PODMAN" ] && [ -z "$IN_DOCKER" ]; then
    echo "🐘 Preparing local database services..."
    make local-db-up || echo "⚠️ Warning: Database startup failed, proceeding anyway."
fi

# 2. Setup instrumentation environment variables
export CARGO_TARGET_DIR="$(pwd)/target/llvm-cov-target"
export RUSTFLAGS="-Cinstrument-coverage --cfg=coverage"
export LLVM_PROFILE_FILE="$(pwd)/target/llvm-cov-data/cowen-%p-%m.profraw"
export RUSTC_WRAPPER=""

echo "🧹 1. Cleaning previous coverage telemetry and build artifacts..."
if command -v cargo-llvm-cov &> /dev/null; then
    cargo llvm-cov clean --workspace
fi
rm -rf target/llvm-cov-data target/llvm-cov-target || (sleep 1 && rm -rf target/llvm-cov-data target/llvm-cov-target) || true
mkdir -p target/llvm-cov-data target/llvm-cov-target/debug

# 3. Run unit and integration tests (injects coverage)
echo "🧪 2. Running Rust unit and integration tests with coverage..."
if command -v cargo-llvm-cov &> /dev/null; then
    cargo llvm-cov --no-report --workspace --lib --bins
else
    echo "❌ Error: cargo-llvm-cov is required to run tests with coverage."
    exit 1
fi

# 4. Build instrumented binaries for E2E tests
echo "🚀 3. Compiling core binaries with coverage instrumentations..."
cargo build -p cowen-cli -p cowen-daemon -p cowen-search-embedding -p cowen-signer -p cowen-mcp-plugin

# 5. Run parallel E2E tests (injects coverage)
echo "🏃 4. Launching parallel E2E test suites..."
export SKIP_BUILD=true
export COWEN_SKIP_BROWSER=true
export TEST_BASE=target/cowen_tests_macos
export BASE_PORT_START=18000

# Execute parallel E2E test runner and capture exit code
set +e
crates/app/cowen-cli/tests/runners/run_parallel.sh
E2E_EXIT_CODE=$?
set -e


# 6. Copy E2E profraw telemetry data to merge folder
echo "📦 5. Copying E2E telemetry trace data..."
cp target/llvm-cov-data/*.profraw target/llvm-cov-target/llvm-cov-target/ || echo "⚠️ Warning: No E2E profraw files found."

# 7. Merge profraw traces using llvm-profdata
echo "📊 6. Merging all profiling telemetry..."
LLVM_PROFDATA=$(rustc --print sysroot)/lib/rustlib/$(rustc -vV | grep host | cut -d' ' -f2)/bin/llvm-profdata

# Run rapid multi-threaded corrupt profraw validation
echo "🧹 Filtering out corrupt profiling files..."
python3 -c "
import os, glob, subprocess
from concurrent.futures import ThreadPoolExecutor
files = glob.glob('target/llvm-cov-target/llvm-cov-target/*.profraw')
def check_file(f):
    if not os.path.exists(f): return
    if os.path.getsize(f) < 1024:
        try: os.remove(f)
        except: pass
        return
    res = subprocess.run(['$LLVM_PROFDATA', 'show', f], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    if res.returncode != 0:
        try: os.remove(f)
        except: pass
with ThreadPoolExecutor(max_workers=32) as ex:
    list(ex.map(check_file, files))
"

$LLVM_PROFDATA merge -sparse target/llvm-cov-target/llvm-cov-target/*.profraw -o target/llvm-cov-target/llvm-cov-target/cowen.profdata

# 8. Generate unified coverage report using llvm-cov
echo "📊 7. Extracting and generating code coverage reports..."
LLVM_COV=$(rustc --print sysroot)/lib/rustlib/$(rustc -vV | grep host | cut -d' ' -f2)/bin/llvm-cov

# Search for the compiled binary objects
OBJECTS=""
for f in target/llvm-cov-target/debug/deps/*; do
    if [ -f "$f" ]; then
        if [ -x "$f" ] || [[ "$f" == *.dylib ]]; then
            if [[ "$f" != *.d ]]; then
                OBJECTS="$OBJECTS -object $f"
            fi
        fi
    fi
done

# Explicitly add core executable targets for symbol resolution
OBJECTS="$OBJECTS -object target/llvm-cov-target/debug/cowen -object target/llvm-cov-target/debug/cowen-daemon -object target/llvm-cov-target/debug/cowen-signer -object target/llvm-cov-target/debug/cowen-mcp-plugin"

$LLVM_COV report -use-color=0 -instr-profile=target/llvm-cov-target/llvm-cov-target/cowen.profdata $OBJECTS -ignore-filename-regex "(registry/src|toolchains/|debug/build|cranelift|target-lexicon|mssql\.rs)" > target/coverage_report.txt

# 9. Execute python coverage gate to print breakdown and enforce quality gate threshold
if [ "$E2E_EXIT_CODE" -ne 0 ]; then
    echo "❌ E2E tests failed (Exit Code: $E2E_EXIT_CODE)! Coverage report generated for debugging, but skipping quality gate."
    exit "$E2E_EXIT_CODE"
fi

echo "📈 8. Executing coverage quality gate check..."
python3 scripts/coverage_gate.py
