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
    "tests/case_09_dlq_retries.sh"
)

$Passed = 0
$env:COWEN_MOCK_MANAGED = "true"

foreach ($suite in $Suites) {
    Write-Host "`n${BOLD}⏳ Running $suite...${NC}"
    # 在 PowerShell 中通过 sh 调用脚本
    & sh $suite
    if ($LASTEXITCODE -eq 0) { $Passed++ } else { Write-Host "${R}❌ $suite FAILED${NC}" }
}

# 4. 清理
Write-Host "`n${Y}  Cleaning up...${NC}"
Stop-Process -Id $MockProcess.Id -Force -ErrorAction SilentlyContinue
& sh -c "pkill -9 -f cowen" 2>$null

Write-Host "${B}${BOLD}========================================================${NC}"
if ($Passed -eq $Suites.Length) {
    Write-Host "${G}${BOLD}✅  ALL SUITES PASSED ($Passed/$($Suites.Length))${NC}"
    exit 0
} else {
    Write-Host "${R}${BOLD}❌  SOME SUITES FAILED ($($Suites.Length - $Passed)/$($Suites.Length))${NC}"
    exit 1
}
