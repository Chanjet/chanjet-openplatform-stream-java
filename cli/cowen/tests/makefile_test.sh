#!/bin/bash
# TDD script for cowen Makefile professional naming refinement

# Red-Green-Refactor:
# 1. This test will initially FAIL (Red) because current Makefile uses linux-arm64, arm64, x64 etc.
# 2. We modify Makefile (Green).
# 3. Final verification.

set -e

MAKEFILE="Makefile"
BINARY="cowen"
VERSION=$(grep '^version =' Cargo.toml | cut -d '"' -f 2)

echo "🔍 Starting Makefile Naming Verification..."

check_target() {
    local target=$1
    local expected_pattern=$2
    echo "Testing target: $target..."
    # Use make -n (dry-run) to see the commands that would be executed
    local output=$(make -n "$target" 2>&1 || true)
    
    # Use Word boundary or exact match if possible
    # We want to ensure that for build targets, it's NOT cowen-v...
    if [[ "$target" != package-* ]]; then
        # For build targets, check that it's [path]/cowen and NOT followed by -v
        if echo "$output" | grep -q "$expected_pattern" && ! echo "$output" | grep -q "$expected_pattern-v"; then
             echo "✅ OK: $target -> Found '$expected_pattern' exactly"
             # Also verify checksum files exist in the dry-run output (they should be called)
             if echo "$output" | grep -q "$expected_pattern.md5" && echo "$output" | grep -q "$expected_pattern.sha1"; then
                 echo "✅ OK: $target -> Checksum commands for MD5 and SHA1 found."
             else
                 echo "❌ FAIL: $target -> Missing MD5 or SHA1 commands in output."
                 return 1
             fi
        else
             echo "❌ FAIL: $target -> Pattern '$expected_pattern' not found exactly or has version suffix."
             return 1
        fi
    else
        # For package targets, long name is expected
        if echo "$output" | grep -q "$expected_pattern"; then
            echo "✅ OK: $target -> Found '$expected_pattern'"
            if echo "$output" | grep -q "$expected_pattern.md5" && echo "$output" | grep -q "$expected_pattern.sha1"; then
                 echo "✅ OK: $target -> Checksum commands for MD5 and SHA1 found."
            else
                 echo "❌ FAIL: $target -> Missing MD5 or SHA1 commands in output."
                 return 1
            fi
        else
            echo "❌ FAIL: $target -> Pattern '$expected_pattern' not found."
            return 1
        fi
    fi
}


# Test Cases for professional lowercase names (Build targets produce short binary names)
# 1. macOS AArch64 (Build)
check_target "macos-aarch64" "macos-aarch64/$BINARY"

# 2. Linux x86_64 (Build)
check_target "linux-x86_64" "linux-x86_64/$BINARY"

# 3. Windows x86_64 (Build)
check_target "windows-x86_64" "windows-x86_64/$BINARY.exe"

# 4. macOS AArch64 (Package)
check_target "package-macos-aarch64" "macos-aarch64/release/$BINARY-v$VERSION-macos-aarch64.pkg"

# 5. Linux x86_64 (Package)
check_target "package-linux-x86_64" "linux-x86_64/release/$BINARY-v$VERSION-linux-x86_64.tar.gz"

# 6. Windows x86_64 (Package)
check_target "package-windows-x86_64" "windows-x86_64/release/$BINARY-v$VERSION-windows-x86_64-setup.exe"



echo "🎉 All naming tests passed!"
