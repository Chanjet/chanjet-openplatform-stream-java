#!/usr/bin/env bash
set -e

echo "======================================"
echo "    Running Code Quality Gate"
echo "======================================"

# 1. Check Rust Code Duplication <= 5%
echo ""
echo "[1/2] Checking code duplication with jscpd (Threshold: 5%)..."
if ! npx -y jscpd@latest crates/ --format rust --threshold 5 --reporters console --ignore-pattern "**/tests/**"; then
    echo "❌ Code duplication check failed! Rust duplication rate > 5%."
    exit 1
else
    echo "✅ Code duplication check passed."
fi

# 2. Check Cyclomatic Complexity <= 15
echo ""
echo "[2/2] Checking cyclomatic complexity with lizard (Threshold: 15)..."
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

echo ""
echo "======================================"
echo "    Code Quality Gate PASSED"
echo "======================================"
exit 0
