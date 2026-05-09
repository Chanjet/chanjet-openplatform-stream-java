param (
    [Parameter(Mandatory=$true)]
    [string]$SetupExePath
)

if (-not (Test-Path $SetupExePath)) {
    Write-Error "❌ 错误: 未找到安装包文件: $SetupExePath"
    exit 1
}

Write-Host "🔍 [Verify] 开始自动验证打包产物: $SetupExePath"

# 第一步：安装
Write-Host "➡️ 第1步: 执行安装..."
cmd /c "echo. | `"$SetupExePath`""
if ($LASTEXITCODE -ne 0) {
    Write-Error "❌ 安装包执行失败！"
    exit 1
}

$CowenPath = Join-Path $env:USERPROFILE ".cowen\bin\cowen.exe"
if (-not (Test-Path $CowenPath)) {
    Write-Error "❌ 安装完成后未能找到预期的可执行文件: $CowenPath"
    exit 1
}
Write-Host "✅ 安装成功，文件存在: $CowenPath"

# 第二步：查看版本
Write-Host "➡️ 第2步: 执行查看版本 --version"
$VersionOut = & $CowenPath --version 2>&1
if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($VersionOut)) {
    Write-Error "❌ --version 验证失败！输出: $VersionOut"
    exit 1
}
Write-Host "✅ 版本检查通过: $VersionOut"

# 第三步：查看状态
Write-Host "➡️ 第3步: 用status命令查看是否正常"
$StatusOut = & $CowenPath status 2>&1
if ($LASTEXITCODE -ne 0) {
    Write-Error "❌ status 验证失败！输出: $StatusOut"
    exit 1
}
Write-Host "✅ status 检查通过。"

# 第四步：查看api列表
Write-Host "➡️ 第4步: 用api list接口查看是否正常"
$ApiListOut = & $CowenPath api list 2>&1
if ($LASTEXITCODE -ne 0) {
    Write-Error "❌ api list 验证失败！输出: $ApiListOut"
    exit 1
}
Write-Host "✅ api list 检查通过。"

Write-Host "🎉 [Verify] 自动验证通过，测试完毕！"
exit 0
