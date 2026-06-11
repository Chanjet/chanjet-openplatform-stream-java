#!/usr/bin/env bash
set -e

echo "======================================"
echo "    Running Code Quality Gate"
echo "======================================"

# 1. Check Cargo Fmt
echo ""
echo "[1/6] Checking code format (cargo fmt)..."
if ! cargo fmt --all -- --check; then
    echo "❌ Code format check failed! Please run 'cargo fmt --all'."
    exit 1
else
    echo "✅ Code format check passed."
fi

# 2. Check Cargo Clippy
echo ""
echo "[2/6] Checking code idioms and lints (cargo clippy)..."
if ! cargo clippy --workspace --all-targets --all-features -- -D warnings -A clippy::too_many_arguments -A clippy::type_complexity -A clippy::ptr_arg -A clippy::manual_clamp -A clippy::match_like_matches_macro; then
    echo "❌ Clippy check failed! Please fix the warnings."
    exit 1
else
    echo "✅ Clippy check passed."
fi

# 3. Check Cross-Platform compilation
echo ""
echo "[3/6] Checking static cross-platform compilation..."
# Verify if the required cross-compilation GCC toolchain is present
if ! command -v x86_64-linux-gnu-gcc &> /dev/null; then
    echo "⚠️  Cross-compiler 'x86_64-linux-gnu-gcc' not found. Skipping make check-cross. (Please run in CI or Docker for full cross-check)"
else
    if ! make check-cross; then
        echo "❌ Cross-platform compilation check failed!"
        exit 1
    else
        echo "✅ Cross-platform compilation check passed."
    fi
fi

# 4. Check Rust Code Duplication (<= 5% overall, and no >5 lines duplicate in the same file)
echo ""
echo "[4/6] Checking code duplication with jscpd (Threshold: 5% overall, max 5 lines within same file)..."
mkdir -p target/jscpd
if ! npx -y jscpd@latest crates/ --format rust --threshold 5 --reporters console,json --output target/jscpd --ignore "**/tests/**,**/*test*.rs,**/*_test.rs" --ignore-pattern "#\\[cfg\\(test\\)\\][\\s\\S]*" --min-lines 6; then
    echo "❌ Code duplication check failed! Rust duplication rate > 5%."
    exit 1
fi

# Run python script to enforce code duplication quality gates
if ! python3 scripts/check_duplication.py; then
    exit 1
fi
echo "✅ Code duplication checks passed."


# 5. Check Cyclomatic Complexity <= 15 and Method Lines of Code <= 100
echo ""
echo "[5/6] Checking cyclomatic complexity and method length with lizard (CCN <= 15, LOC <= 100)..."
# Check if lizard is available
if ! command -v lizard &> /dev/null; then
    echo "lizard not found globally, setting up local venv to run lizard..."
    if [ ! -d ".venv_lizard" ]; then
        python3 -m venv .venv_lizard
        source .venv_lizard/bin/activate
        pip install lizard --quiet
    else
        source .venv_lizard/bin/activate
    fi
fi

if ! lizard crates/ -C 15 -T nloc=100 -w -x "**/tests/**"; then
    echo "❌ Complexity or function length check failed! Found functions with CCN > 15 or NLOC > 100."
    exit 1
else
    echo "✅ Cyclomatic complexity and function length checks passed."
fi

# 6. Check Cargo Doc (Warn only for now, since it might fail immediately)
echo ""
echo "[6/6] Checking documentation compilation (cargo doc)..."
if ! RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --document-private-items > /dev/null 2>&1; then
    echo "⚠️  Doc check failed (Warnings found). Please check cargo doc warnings. (Non-blocking for now)"
else
    echo "✅ Doc check passed."
fi


# 7. Check Cargo Audit & Deny (Warn only for now)
echo ""

echo ""
echo "[7/8] Checking unused dependencies (cargo machete)..."
if ! command -v cargo-machete &> /dev/null; then
    echo "cargo-machete not found globally, installing..."
    cargo install cargo-machete
fi

if ! cargo machete; then
    echo "❌ Unused dependencies found! Please remove them from Cargo.toml."
    exit 1
else
    echo "✅ Unused dependencies check passed."
fi

echo ""
echo "[8/8] Checking dependency sorting (cargo sort)..."
if ! command -v cargo-sort &> /dev/null; then
    echo "cargo-sort not found globally, installing..."
    cargo install cargo-sort
fi

if ! cargo sort --workspace --check; then
    echo "❌ Cargo.toml dependencies are not sorted! Please run 'cargo sort --workspace'."
    exit 1
else
    echo "✅ Dependency sorting check passed."
fi

echo "[Bonus] Checking security and dependencies..."
if command -v cargo-audit &> /dev/null; then
    CACHE_MARKER="$HOME/.cargo/.advisory-db-last-fetch"
    CACHE_TTL=$((7 * 24 * 3600))
    NOW=$(date +%s)
    FETCH_NEEDED=1

    if [ -f "$CACHE_MARKER" ] && [ -d "$HOME/.cargo/advisory-db" ]; then
        LAST_FETCH=$(cat "$CACHE_MARKER" 2>/dev/null || echo 0)
        AGE=$((NOW - LAST_FETCH))
        if [ "$AGE" -lt "$CACHE_TTL" ]; then
            FETCH_NEEDED=0
            echo "ℹ️  Advisory DB cache is valid ($((AGE / 86400)) days old). Using --no-fetch to speed up..."
        fi
    fi

    if [ "$FETCH_NEEDED" -eq 1 ]; then
        if ! cargo audit; then
            echo "⚠️  cargo audit fetch failed (likely a network error). Retrying offline with --no-fetch..."
            if ! cargo audit --no-fetch; then
                echo "❌ Cargo audit found vulnerabilities! Please fix them or add to .cargo/audit.toml ignores."
                exit 1
            fi
        else
            mkdir -p "$HOME/.cargo"
            echo "$NOW" > "$CACHE_MARKER"
        fi
    else
        if ! cargo audit --no-fetch; then
            echo "❌ Cargo audit found vulnerabilities! Please fix them or add to .cargo/audit.toml ignores."
            exit 1
        fi
    fi
    echo "✅ Cargo audit check passed."
else
    echo "cargo-audit not installed. Skipping..."
fi

if command -v gitleaks &> /dev/null; then
    if ! gitleaks detect --source . -v; then
        echo "⚠️  Gitleaks found sensitive data! (Non-blocking for now)"
    fi
else
    echo "gitleaks not installed. Skipping..."
fi
# 9. Check Code Coverage (cargo llvm-cov)
echo ""
echo "[9/10] Checking code coverage (cargo llvm-cov)..."
if ! command -v cargo-llvm-cov &> /dev/null; then
    echo "⚠️  cargo-llvm-cov not found. Skipping coverage check."
else
    # Enforce minimum 8% total coverage on cowen-auth package (due to workspace dependency dilution)
    # NOTE: sccache interferes with llvm-cov by stripping profiling data, so we must explicitly disable it.
    if ! RUSTC_WRAPPER="" cargo llvm-cov test --package cowen-auth --fail-under-lines 8; then
        echo "❌ Code coverage check failed for cowen-auth (Target: >=8%)!"
        exit 1
    else
        echo "✅ Code coverage check passed."
    fi
fi

# 10. Check Dependency License & Compliance (cargo-deny)
echo ""
echo "[10/10] Checking dependency licenses & bans (cargo-deny)..."
if ! command -v cargo-deny &> /dev/null; then
    echo "cargo-deny not found, attempting to install..."
    cargo install --locked cargo-deny || echo "⚠️  Failed to install cargo-deny. Skipping check."
fi

if command -v cargo-deny &> /dev/null; then
    if ! cargo deny check licenses bans; then
        echo "❌ Dependency license or compliance check failed!"
        exit 1
    else
        echo "✅ Dependency license & bans check passed."
    fi
fi

echo ""
echo "======================================"
echo "    Code Quality Gate PASSED"
echo "======================================"
exit 0
