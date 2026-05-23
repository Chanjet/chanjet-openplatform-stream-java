# Windows 一键自动化部署配置脚本 (install.ps1)
# 作用：自动创建持久安装目录，拷贝 CLI 与 AI 插件，更新环境变量，并配置自动补全和后台守护进程。

$install_dir = Join-Path $env:USERPROFILE ".cowen\bin"
if (!(Test-Path $install_dir)) {
    New-Item -ItemType Directory -Force -Path $install_dir | Out-Null
}

Write-Host "=== 🚀 Starting Cowen Windows installation ===" -ForegroundColor Cyan

# 1. 自动探测并拷贝核心二进制和插件 DLL
$current_dir = Get-Location
$files_to_copy = @("cowen.exe", "cowen-daemon.exe", "cowen_search_embedding.dll")
foreach ($file in $files_to_copy) {
    $src = Join-Path $current_dir $file
    if (Test-Path $src) {
        Copy-Item -Path $src -Destination $install_dir -Force
        Write-Host "✅ Copied $file to $install_dir" -ForegroundColor Green
    }
}

# 2. 自动添加持久安装目录到用户的 PATH 环境变量
$user_path = [Environment]::GetEnvironmentVariable('Path', 'User')
if (!$user_path.Contains($install_dir)) {
    Write-Host "⚙️ Adding $install_dir to User PATH..." -ForegroundColor Cyan
    $new_path = if ([string]::IsNullOrEmpty($user_path)) { $install_dir } else { "$user_path;$install_dir" }
    [Environment]::SetEnvironmentVariable('Path', $new_path, 'User')
    Write-Host "✅ PATH updated successfully." -ForegroundColor Green
} else {
    Write-Host "ℹ️ $install_dir is already in PATH." -ForegroundColor Yellow
}

# 3. 自动注入 Powershell 终端的 Tab 自动补全
$profile_dir = Join-Path $env:USERPROFILE "Documents\WindowsPowerShell"
$profile_path = Join-Path $profile_dir "Microsoft.PowerShell_profile.ps1"
if (!(Test-Path $profile_dir)) {
    New-Item -ItemType Directory -Force -Path $profile_dir | Out-Null
}

$completion_block = @"

# cowen completion
if (Get-Command cowen -ErrorAction SilentlyContinue) {
    cowen completion powershell | Out-String | Invoke-Expression
}
"@

$profile_content = ""
if (Test-Path $profile_path) {
    $profile_content = Get-Content $profile_path -Raw
}
if (!$profile_content.Contains("# cowen completion")) {
    Write-Host "⚙️ Setting up PowerShell auto-completion..." -ForegroundColor Cyan
    Add-Content -Path $profile_path -Value $completion_block
    Write-Host "✅ Auto-completion added to your PowerShell profile." -ForegroundColor Green
}

# 4. 注册系统自启动守护后台服务
$exe_path = Join-Path $install_dir "cowen.exe"
if (Test-Path $exe_path) {
    Write-Host "📟 Registering autostart daemon service..." -ForegroundColor Cyan
    Start-Process -FilePath $exe_path -ArgumentList "daemon", "service", "install" -NoNewWindow -Wait
}

Write-Host "`n🎉 Installation complete! Please RESTART your terminal/PowerShell." -ForegroundColor Green
Read-Host "Press Enter to exit..."
