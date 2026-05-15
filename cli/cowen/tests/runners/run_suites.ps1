# Cowen CLI Windows PowerShell Test Runner
$ErrorActionPreference = "Stop"

# ANSI 颜色映射
$ESC = [char]27
$G = "$ESC[32m"; $R = "$ESC[31m"; $B = "$ESC[34m"; $Y = "$ESC[33m"; $BOLD = "$ESC[1m"; $NC = "$ESC[0m"

Write-Host "${B}${BOLD}========================================================${NC}"
Write-Host "${B}${BOLD}   Cowen CLI Windows PowerShell Test Runner            ${NC}"
Write-Host "${B}${BOLD}========================================================${NC}"

# 1. 编译二进制
Write-Host -NoNewline "  Building cowen binary..."
& cargo build --quiet
if ($LASTEXITCODE -ne 0) { Write-Host " ${R}[FAILED]${NC}"; exit 1 }
Write-Host " ${G}[OK]${NC}"

# 2. 启动 Mock Server
Write-Host -NoNewline "  Starting Mock Server..."
$MockProcess = Start-Process python3 -ArgumentList "tests/infra/mock_server.py" -NoNewWindow -PassThru -RedirectStandardOutput "mock_server.log" -RedirectStandardError "mock_server_err.log"
Start-Sleep -Seconds 2
Write-Host " ${G}[READY]${NC}"

# 3. 运行测试套件 (利用 Git Bash 的 sh 环境)
$Suites = @(
    "tests/e2e/scripts/case_01_self_built.sh",
    "tests/e2e/scripts/case_02_store_app.sh",
    "tests/e2e/scripts/case_03_oauth2.sh",
    "tests/e2e/scripts/case_04_migration.sh",
    "tests/e2e/scripts/case_05_proxy_interception.sh",
    "tests/e2e/scripts/case_06_webhook_forwarding.sh",
    "tests/e2e/scripts/case_07_token_lifecycle.sh",
    "tests/e2e/scripts/case_08_concurrent_stress.sh",
    "tests/e2e/scripts/case_09_dlq_retries.sh",
    "tests/e2e/scripts/case_10_profile_management.sh",
    "tests/e2e/scripts/case_11_reconnect_resilience.sh",
    "tests/e2e/scripts/case_12_daemon_recovery.sh",
    "tests/e2e/scripts/case_13_distributed_lb.sh",
    "tests/e2e/scripts/case_14_shared_storage.sh",
    "tests/e2e/scripts/case_15_store_app_shared_storage.sh",
    "tests/e2e/scripts/case_16_migration_block.sh",
    "tests/e2e/scripts/case_17_redis_shared_storage.sh",
    "tests/e2e/scripts/case_18_redis_fault_tolerance.sh",
    "tests/e2e/scripts/case_19_ticket_auto_resend.sh",
    "tests/e2e/scripts/case_20_oauth2_refresh.sh",
    "tests/e2e/scripts/case_21_openapi_whitelist.sh",
    "tests/e2e/scripts/case_22_dlq_manual_retry.sh",
    "tests/e2e/scripts/case_23_completion.sh",
    "tests/e2e/scripts/case_24_status_all.sh",
    "tests/e2e/scripts/case_25_cluster_idempotency.sh",
    "tests/e2e/scripts/case_26_hybrid_data_drift.sh",
    "tests/e2e/scripts/case_27_store_app_multi_org_stress.sh",
    "tests/e2e/scripts/case_28_sidecar_startup.sh",
    "tests/e2e/scripts/case_29_sidecar_scaling_stress.sh",
    "tests/e2e/scripts/case_30_sidecar_self_built_stress.sh",
    "tests/e2e/scripts/case_31_mysql_shared_storage.sh",
    "tests/e2e/scripts/case_32_postgres_shared_storage.sh",
    "tests/e2e/scripts/case_33_exclusive_connection.sh",
    "tests/e2e/scripts/case_34_daemon_recovery_enhanced.sh",
    "tests/e2e/scripts/case_35_store_app_pg_ticket.sh",
    "tests/e2e/scripts/case_36_store_app_activation.sh",
    "tests/e2e/scripts/case_37_init_cleanup.sh",
    "tests/e2e/scripts/case_38_init_deduplication.sh",
    "tests/e2e/scripts/case_39_profile_rename_comprehensive.sh",
    "tests/e2e/scripts/case_40_log_level_dynamic.sh",
    "tests/e2e/scripts/case_41_auth_logout_login_flow.sh"
)

$Passed = 0
$env:COWEN_MOCK_MANAGED = "true"

foreach ($suite in $Suites) {
    # Aggressive isolation: kill any lingering cowen processes from previous suites
    & sh -c "pkill -9 -f cowen" 2>$null
    
    Write-Host "`n${BOLD}⏳ Running $suite...${NC}"
    # 在 PowerShell 中通过 sh 调用脚本
    & sh $suite
    if ($LASTEXITCODE -eq 0) { 
        $Passed++ 
    } else { 
        Write-Host "${R}❌ $suite FAILED${NC}" 
    }
    
    # Ensure isolation by cleaning up after every single case
    & sh -c "source tests/e2e/scripts/common.sh; cleanup_suite $suite" 2>$null
}

# 4. 清理
Write-Host "`n${Y}  Cleaning up...${NC}"
Stop-Process -Id $MockProcess.Id -Force -ErrorAction SilentlyContinue
& sh -c "pkill -9 -f cowen" 2>$null

Write-Host "${B}${BOLD}========================================================${NC}"
if ($Passed -eq $Suites.Length) {
    Write-Host "${G}${BOLD}✅  ALL SUITES PASSED ($Passed/$($Suites.Length))${NC}"
    & sh -c "source tests/e2e/scripts/common.sh; cleanup_all_workspaces" 2>$null
    exit 0
} else {
    Write-Host "${R}${BOLD}❌  SOME SUITES FAILED ($($Suites.Length - $Passed)/$($Suites.Length))${NC}"
    & sh -c "source tests/e2e/scripts/common.sh; cleanup_all_workspaces" 2>$null
    exit 1
}
