import re

with open("scripts/quality_gate.sh", "r") as f:
    content = f.read()

# Let's revert the [1/8] back to [1/6]
content = content.replace("[1/8]", "[1/6]")
content = content.replace("[2/8]", "[2/6]")
content = content.replace("[3/8]", "[3/6]")
content = content.replace("[4/8]", "[4/6]")
content = content.replace("[5/8]", "[5/6]")
content = content.replace("[6/8]", "[6/6]")

# Remove the 7 and 8 blocks we added
content = re.sub(r'# 7\. Check unused.*?✅ Dependency sorting check passed\.\nfi\n', '', content, flags=re.DOTALL)

# Now, add them under the Bonus section
bonus_gates = """
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
"""

content = content.replace("echo \"[Bonus] Checking security and dependencies (Warn only)...\"", bonus_gates + "\necho \"[Bonus] Checking security and dependencies (Warn only)...\"")

with open("scripts/quality_gate.sh", "w") as f:
    f.write(content)

