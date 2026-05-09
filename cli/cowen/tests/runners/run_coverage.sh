#!/bin/bash
# Cowen CLI Coverage Collector (Stable Version)
# This script manually manages instrumentation to ensure E2E tests are captured.

source tests/e2e/scripts/common.sh

echo -e "${BLUE}${BOLD}========================================================${NC}"
echo -e "${BLUE}${BOLD}   Cowen CLI Code Coverage Analysis                    ${NC}"
echo -e "${BLUE}${BOLD}========================================================${NC}"

# 1. Clean
cargo llvm-cov clean --workspace
rm -rf target/llvm-cov-data
mkdir -p target/llvm-cov-data

# 2. Run Unit Tests (Collecting data into default profraw location)
echo -e "\n${BOLD}1. Collecting Unit Test Coverage...${NC}"
cargo llvm-cov --no-report

# 3. Build Instrumented Binary for E2E
echo -e "\n${BOLD}2. Building Instrumented Binary for E2E...${NC}"
# Use the same flags cargo-llvm-cov uses internally
export RUSTFLAGS="-Cinstrument-coverage"
cargo build --quiet

# 4. Run E2E Suites
echo -e "\n${BOLD}3. Running E2E Test Suites...${NC}"
# Redirect LLVM output to our data dir
export LLVM_PROFILE_FILE="target/llvm-cov-data/cowen-%p-%m.profraw"

# Run suites (serial for reliability and better tracing)
bash tests/run_suites.sh

# 5. Merge and Report
echo -e "\n${BOLD}4. Generating Merged Visual Report...${NC}"
# Point llvm-cov to our custom profraw files
cargo llvm-cov report --show-missing-lines
cargo llvm-cov report --html

echo -e "\n${GREEN}${BOLD}✅ Coverage report available: cli/cowen/target/llvm-cov/html/index.html${NC}"
