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

# 4. Check Rust Code Duplication <= 5%
echo ""
echo "[4/6] Checking code duplication with jscpd (Threshold: 5%)..."
if ! npx -y jscpd@latest crates/ --format rust --threshold 5 --reporters console --ignore-pattern "**/tests/**"; then
    echo "❌ Code duplication check failed! Rust duplication rate > 5%."
    exit 1
else
    echo "✅ Code duplication check passed."
fi

# 5. Check Cyclomatic Complexity <= 15
echo ""
echo "[5/6] Checking cyclomatic complexity with lizard (Threshold: 15)..."
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

if ! lizard crates/ -C 15 -w -x "**/tests/**"; then
    echo "❌ Cyclomatic complexity check failed! Found functions with CCN > 15."
    exit 1
else
    echo "✅ Cyclomatic complexity check passed."
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
echo "[Bonus] Checking unused dependencies (cargo machete)..."
if command -v cargo-machete &> /dev/null; then
    if ! cargo machete; then
        echo "⚠️  Unused dependencies found! (Non-blocking for now)"
    fi
else
    echo "cargo-machete not installed. Skipping..."
fi

echo ""
echo "[Bonus] Checking dependency sorting (cargo sort)..."
if command -v cargo-sort &> /dev/null; then
    if ! cargo sort --workspace --check; then
        echo "⚠️  Cargo.toml dependencies are not sorted! (Non-blocking for now)"
    fi
else
    echo "cargo-sort not installed. Skipping..."
fi

echo "[Bonus] Checking security and dependencies (Warn only)..."
if command -v cargo-audit &> /dev/null; then
    if ! cargo audit; then
        echo "⚠️  Cargo audit found vulnerabilities! (Non-blocking for now)"
    fi
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

echo ""
echo "======================================"
echo "    Code Quality Gate PASSED"
echo "======================================"
exit 0
