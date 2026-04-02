# cowen CLI Installer for Windows
$ErrorActionPreference = "Stop"

$BinaryName = "cowen.exe"
$InstallDir = "$HOME\.cowen\bin"

Write-Host "🚀 Starting cowen installation..." -ForegroundColor Cyan

# 1. Create install directory
if (!(Test-Path $InstallDir)) {
    New-Item -Path $InstallDir -ItemType Directory | Out-Null
    Write-Host "Created installation directory: $InstallDir"
}

# 2. Copy binary
if (!(Test-Path $BinaryName)) {
    Write-Error "Could not find $BinaryName in the current directory. Please ensure you downloaded both the exe and this script."
    return
}
Copy-Item $BinaryName -Destination $InstallDir -Force
Write-Host "✅ Copied $BinaryName to $InstallDir" -ForegroundColor Green

# 3. Add to PATH
$CurrentPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($CurrentPath -notlike "*$InstallDir*") {
    Write-Host "Adding $InstallDir to User PATH..."
    $NewPath = "$CurrentPath;$InstallDir"
    [Environment]::SetEnvironmentVariable("Path", $NewPath, "User")
    Write-Host "✅ PATH updated successfully." -ForegroundColor Green
} else {
    Write-Host "ℹ️ $InstallDir is already in PATH."
}

# 4. Setup Completion for PowerShell
Write-Host "⚙️ Setting up PowerShell auto-completion..."
$ProfileDir = Split-Path $PROFILE -Parent
if (!(Test-Path $ProfileDir)) { New-Item -Path $ProfileDir -ItemType Directory | Out-Null }
if (!(Test-Path $PROFILE)) { New-Item -Path $PROFILE -ItemType File | Out-Null }

$CompletionCmd = "`n# cowen completion`nif (Get-Command cowen -ErrorAction SilentlyContinue) { cowen completion powershell | Out-String | Invoke-Expression }"
$ProfileContent = Get-Content $PROFILE -ErrorAction SilentlyContinue
if ($ProfileContent -notlike "*# cowen completion*") {
    Add-Content -Path $PROFILE -Value $CompletionCmd
    Write-Host "✅ Auto-completion added to your PowerShell profile." -ForegroundColor Green
}

Write-Host "`n🎉 Installation complete! Please RESTART your terminal." -ForegroundColor Cyan
Write-Host "Try running: cowen --help"
