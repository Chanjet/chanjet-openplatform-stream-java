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
$MockProcess = Start-Process python3 -ArgumentList "tests/mock_server.py" -NoNewWindow -PassThru -RedirectStandardOutput "mock_server.log" -RedirectStandardError "mock_server_err.log"
Start-Sleep -Seconds 2
Write-Host " ${G}[READY]${NC}"

# 3. 运行测试套件 (利用 Git Bash 的 sh 环境)
$Suites = @(
    "tests/case_01_self_built.sh",
    "tests/case_02_store_app.sh",
    "tests/case_03_oauth2.sh",
    "tests/case_04_migration.sh",
    "tests/case_05_proxy_interception.sh",
    "tests/case_06_webhook_forwarding.sh",
    "tests/case_07_token_lifecycle.sh",
    "tests/case_08_concurrent_stress.sh",
    "tests/case_09_dlq_retries.sh",
    "tests/case_10_profile_management.sh",
    "tests/case_11_reconnect_resilience.sh",
    "tests/case_12_daemon_recovery.sh",
    "tests/case_13_distributed_lb.sh",
    "tests/case_14_shared_storage.sh",
    "tests/case_15_store_app_shared_storage.sh",
    "tests/case_16_migration_block.sh",
    "tests/case_17_redis_shared_storage.sh",
    "tests/case_18_redis_fault_tolerance.sh",
    "tests/case_19_ticket_auto_resend.sh",
    "tests/case_20_oauth2_refresh.sh",
    "tests/case_21_openapi_whitelist.sh",
    "tests/case_22_dlq_manual_retry.sh",
    "tests/case_24_completion.sh",
    "tests/case_25_status_all.sh",
    "tests/case_26_cluster_idempotency.sh",
    "tests/case_27_hybrid_data_drift.sh",
    "tests/case_28_store_app_multi_org_stress.sh",
    "tests/case_29_sidecar_startup.sh",
    "tests/case_30_sidecar_scaling_stress.sh"
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
    & sh -c "source tests/common.sh; cleanup_suite $suite" 2>$null
}

# 4. 清理
Write-Host "`n${Y}  Cleaning up...${NC}"
Stop-Process -Id $MockProcess.Id -Force -ErrorAction SilentlyContinue
& sh -c "pkill -9 -f cowen" 2>$null

Write-Host "${B}${BOLD}========================================================${NC}"
if ($Passed -eq $Suites.Length) {
    Write-Host "${G}${BOLD}✅  ALL SUITES PASSED ($Passed/$($Suites.Length))${NC}"
    & sh -c "source tests/common.sh; cleanup_all_workspaces" 2>$null
    exit 0
} else {
    Write-Host "${R}${BOLD}❌  SOME SUITES FAILED ($($Suites.Length - $Passed)/$($Suites.Length))${NC}"
    & sh -c "source tests/common.sh; cleanup_all_workspaces" 2>$null
    exit 1
}
