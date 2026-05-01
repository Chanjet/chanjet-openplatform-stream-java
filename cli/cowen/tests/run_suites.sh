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
    "tests/case_12_daemon_recovery.sh"
    "tests/case_13_distributed_lb.sh"
    "tests/case_14_shared_storage.sh"
    "tests/case_15_store_app_shared_storage.sh"
    "tests/case_16_migration_block.sh"
    "tests/case_17_redis_shared_storage.sh"
    "tests/case_18_redis_fault_tolerance.sh"
    "tests/case_19_ticket_auto_resend.sh"
    "tests/case_20_oauth2_refresh.sh"
    "tests/case_21_openapi_whitelist.sh"
    "tests/case_22_dlq_manual_retry.sh"
    "tests/case_24_completion.sh"
    "tests/case_25_status_all.sh"
    "tests/case_26_cluster_idempotency.sh"
    "tests/case_27_hybrid_data_drift.sh"
    "tests/case_28_store_app_multi_org_stress.sh"
)

PASSED=0
TOTAL=${#SUITES[@]}

# Disable internal mock start in individual cases
export COWEN_MOCK_MANAGED="true"
RESULTS_DIR="target/cowen_tests/results"
mkdir -p "target/cowen_tests"

for suite in "${SUITES[@]}"; do
    echo -e "\n${BOLD}⏳ Running $suite...${NC}"
    if bash "$suite"; then
        PASSED=$((PASSED+1))
    else
        echo -e "${RED}❌ $suite FAILED${NC}"
    fi
    # Ensure isolation by cleaning up after every single case
    cleanup_suite "$suite"
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
