#!/bin/bash
# Cowen CLI Professional Test Runner - Robust Version
source tests/common.sh

GREEN='\033[0;32m'
RED='\033[0;31m'
BLUE='\033[1;34m'
BOLD='\033[1m'
NC='\033[0m'

echo -e "${BLUE}${BOLD}========================================================${NC}"
echo -e "${BLUE}${BOLD}   Cowen CLI Full Capability Verification Suite        ${NC}"
echo -e "${BLUE}${BOLD}========================================================${NC}"

# Build latest binary
echo -n "  Building cowen binary..."
if cargo build --quiet 2>/dev/null; then
    echo -e " ${GREEN}[OK]${NC}"
else
    echo -e " ${RED}[FAILED]${NC}"
    exit 1
fi

# Start Mock Server once for all suites
start_mock

SUITES=(
    "tests/case_01_self_built.sh"
    "tests/case_02_store_app.sh"
    "tests/case_03_oauth2.sh"
    "tests/case_04_migration.sh"
    "tests/case_05_proxy_interception.sh"
    "tests/case_06_webhook_forwarding.sh"
    "tests/case_07_token_lifecycle.sh"
    "tests/case_08_concurrent_stress.sh"
    "tests/case_09_dlq_retries.sh"
    "tests/case_10_profile_management.sh"
    "tests/case_11_reconnect_resilience.sh"
)

PASSED=0
TOTAL=${#SUITES[@]}

# Disable internal mock start in individual cases
export COWEN_MOCK_MANAGED="true"

for suite in "${SUITES[@]}"; do
    echo -e "\n${BOLD}⏳ Running $suite...${NC}"
    if bash "$suite"; then
        ((PASSED+=1))
    else
        echo -e "${RED}❌ $suite FAILED${NC}"
    fi
done

echo -e "\n${BLUE}${BOLD}========================================================${NC}"
if [ $PASSED -eq $TOTAL ]; then
    echo -e "${GREEN}${BOLD}✅  ALL SUITES PASSED ($PASSED/$TOTAL)${NC}"
    cleanup_all_workspaces
    exit 0
else
    echo -e "${RED}${BOLD}❌  SOME SUITES FAILED ($((TOTAL-PASSED))/$TOTAL)${NC}"
    cleanup_all_workspaces
    exit 1
fi
