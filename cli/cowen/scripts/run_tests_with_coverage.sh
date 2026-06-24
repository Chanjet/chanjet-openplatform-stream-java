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

# 2. Detect OS and set isolated coverage directories
OS_NAME=$(uname -s | tr '[:upper:]' '[:lower:]' | sed 's/mingw.*/windows/;s/msys.*/windows/;s/cygwin.*/windows/')
if [ -n "$CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUNNER" ]; then
    OS_NAME="windows-cross"
fi
if [ "$IN_DOCKER" = "true" ] || [ -f /.dockerenv ]; then
    if [ "$OS_NAME" != "windows-cross" ]; then
        OS_NAME="linux-docker"
    fi
fi

echo "🌍 Detected OS Environment for Coverage: $OS_NAME"
COV_BASE="target/coverage_${OS_NAME}"
COV_TARGET="${COV_BASE}/llvm-cov-target"
COV_DATA="${COV_BASE}/llvm-cov-data"
REPORT_TXT="${COV_BASE}/coverage_report.txt"
GATE_FILE=".coverage_gate_${OS_NAME}"

export CARGO_TARGET_DIR="$(pwd)/$COV_TARGET"
export RUSTFLAGS="-Cinstrument-coverage --cfg=coverage"
export LLVM_PROFILE_FILE="$(pwd)/$COV_DATA/cowen-%p-%m.profraw"
export RUSTC_WRAPPER=""

# 2.5 Setup ORT_DYLIB_PATH for ONNX runtime tests
if [ "$(uname -s)" = "Darwin" ]; then
    export ORT_DYLIB_PATH="$(pwd)/dist_assets/macos/libonnxruntime.dylib"
elif [ "$(uname -s)" = "Linux" ] && [ "$OS_NAME" != "windows-cross" ]; then
    export ORT_DYLIB_PATH="$(pwd)/dist_assets/linux/libonnxruntime.so"
fi

echo "🧹 1. Cleaning previous coverage telemetry and build artifacts..."
if command -v cargo-llvm-cov &> /dev/null; then
    cargo llvm-cov clean --workspace
fi
rm -rf "$COV_BASE" || (sleep 1 && rm -rf "$COV_BASE") || true
mkdir -p "$COV_DATA" "$COV_TARGET/debug"

# 3. Run unit and integration tests (injects coverage)
echo "🧪 2. Running Rust unit and integration tests with coverage..."

TARGET_FLAG=""
if [ -n "$CARGO_BUILD_TARGET" ]; then
    TARGET_FLAG="--target $CARGO_BUILD_TARGET"
fi

if [ "$OS_NAME" = "windows-cross" ]; then
    echo "⚠️  Cross-compiling coverage for Windows (GNU) lacks profiler_builtins in stable Rust. Running standard tests without coverage."
    export RUSTFLAGS="-D warnings"
    cargo test --workspace --exclude cowen-wasm-auth-selfbuilt --exclude cowen-wasm-auth-storeapp --exclude cowen-server --lib --bins $TARGET_FLAG -- --skip test_report_event_timeout_and_spin_prevention
    
    echo "🚀 3. Compiling core binaries..."
    cargo build -j ${MAX_PARALLEL:-4} -p cowen-cli -p cowen-daemon -p cowen-search-embedding -p cowen-signer -p cowen-mcp-plugin $TARGET_FLAG
    
    if [ "$SKIP_E2E" != "true" ]; then
        echo "🌐 Running E2E tests for Windows..."
        export SKIP_BUILD=true
        export COWEN_SKIP_BROWSER=true
        export TEST_BASE="$COV_BASE/cowen_tests"
        export BASE_PORT_START=18000
        bash crates/app/cowen-cli/tests/runners/run_parallel.sh
    fi
    
    echo "✅ Windows cross-compilation tests passed. Skipping coverage gate."
    exit 0
fi

if command -v cargo-llvm-cov &> /dev/null; then
    cargo llvm-cov --no-report --workspace -j ${MAX_PARALLEL:-4} \
        --exclude cowen-wasm-auth-selfbuilt \
        --exclude cowen-wasm-auth-storeapp \
        --exclude cowen-grpc-facade \
        --exclude cowen-wasm-facade \
        --exclude cowen-daemon \
        --exclude cowen-macros \
        --exclude cowen-capabilities \
        --exclude cowen-ai \
        --exclude cowen-doctor \
        --exclude cowen-signer \
        --lib --bins $TARGET_FLAG -- --test-threads=4
else
    echo "❌ Error: cargo-llvm-cov is required to run tests with coverage."
    exit 1
fi

# 4. Build instrumented binaries for E2E tests
echo "🚀 3. Compiling core binaries with coverage instrumentations..."
CARGO_TARGET_DIR="$CARGO_TARGET_DIR/llvm-cov-target" cargo build -j ${MAX_PARALLEL:-4} -p cowen-cli -p cowen-daemon -p cowen-search-embedding -p cowen-signer -p cowen-mcp-plugin $TARGET_FLAG

# Copy the binaries to the single-nested directory where run_parallel.sh expects them
mkdir -p "$COV_TARGET/debug"
cp "$COV_TARGET/llvm-cov-target/debug/cowen" "$COV_TARGET/debug/" || true
cp "$COV_TARGET/llvm-cov-target/debug/cowen-daemon" "$COV_TARGET/debug/" || true
cp "$COV_TARGET/llvm-cov-target/debug/cowen-signer" "$COV_TARGET/debug/" || true
cp "$COV_TARGET/llvm-cov-target/debug/cowen-mcp-plugin" "$COV_TARGET/debug/" || true
cp "$COV_TARGET/llvm-cov-target/debug/libcowen_search_embedding" "$COV_TARGET/debug/" || true

# 5. Run parallel E2E tests (injects coverage)
if [ "$SKIP_E2E" != "true" ]; then
    echo "🏃 4. Launching parallel E2E test suites..."
    export SKIP_BUILD=true
    export COWEN_SKIP_BROWSER=true
    export TEST_BASE="$COV_BASE/cowen_tests"
    export BASE_PORT_START=18000

    # Execute parallel E2E test runner and capture exit code
    set +e
    crates/app/cowen-cli/tests/runners/run_parallel.sh
    E2E_EXIT_CODE=$?
    set -e

    # 6. Copy E2E profraw telemetry data to merge folder
    echo "📦 5. Copying E2E telemetry trace data..."
    cp "$COV_DATA"/*.profraw "$COV_TARGET/" || echo "⚠️ Warning: No E2E profraw files found."
else
    echo "⏭️ 4. Skipping E2E test suites (SKIP_E2E=true)..."
    E2E_EXIT_CODE=0
    echo "📦 5. Skipping E2E profraw copy..."
fi

# 7. Merge profraw traces using llvm-profdata
echo "📊 6. Merging all profiling telemetry..."
LLVM_PROFDATA=$(rustc --print sysroot)/lib/rustlib/$(rustc -vV | grep host | cut -d' ' -f2)/bin/llvm-profdata

# Run rapid multi-threaded corrupt profraw validation
echo "🧹 Filtering out corrupt profiling files..."
python3 -c "
import os, glob, subprocess
from concurrent.futures import ThreadPoolExecutor
files = glob.glob('$COV_TARGET/*.profraw')
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

$LLVM_PROFDATA merge -sparse "$COV_TARGET"/*.profraw -o "$COV_TARGET/cowen.profdata"

# 8. Generate unified coverage report using llvm-cov
echo "📊 7. Extracting and generating code coverage reports..."
LLVM_COV=$(rustc --print sysroot)/lib/rustlib/$(rustc -vV | grep host | cut -d' ' -f2)/bin/llvm-cov

# Search for the compiled binary objects
OBJECTS=""
for f in "$COV_TARGET/debug/deps"/*; do
    if [ -f "$f" ]; then
        if [ -x "$f" ] || [[ "$f" == *.dylib ]]; then
            if [[ "$f" != *.d ]]; then
                OBJECTS="$OBJECTS -object $f"
            fi
        fi
    fi
done

# Explicitly add core executable targets for symbol resolution
OBJECTS="$OBJECTS -object $COV_TARGET/debug/cowen -object $COV_TARGET/debug/cowen-daemon -object $COV_TARGET/debug/cowen-signer -object $COV_TARGET/debug/cowen-mcp-plugin"

$LLVM_COV report -use-color=0 -instr-profile="$COV_TARGET/cowen.profdata" $OBJECTS -ignore-filename-regex "(registry/src|toolchains/|debug/build|cranelift|target-lexicon|mssql\.rs)" > "$REPORT_TXT"

# 9. Execute python coverage gate to print breakdown and enforce quality gate threshold
if [ "$E2E_EXIT_CODE" -ne 0 ]; then
    echo "❌ E2E tests failed (Exit Code: $E2E_EXIT_CODE)! Coverage report generated for debugging, but skipping quality gate."
    exit "$E2E_EXIT_CODE"
fi

echo "📈 8. Executing coverage quality gate check..."
python3 scripts/coverage_gate.py --report "$REPORT_TXT" --gate "$GATE_FILE"
