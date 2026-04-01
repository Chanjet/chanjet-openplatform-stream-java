# owenc CLI Installer for Windows
$ErrorActionPreference = "Stop"

$BinaryName = "owenc.exe"
$InstallDir = "$HOME\.owenc\bin"

Write-Host "🚀 Starting owenc installation..." -ForegroundColor Cyan

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

$CompletionCmd = "`n# owenc completion`nif (Get-Command owenc -ErrorAction SilentlyContinue) { owenc completion powershell | Out-String | Invoke-Expression }"
$ProfileContent = Get-Content $PROFILE -ErrorAction SilentlyContinue
if ($ProfileContent -notlike "*# owenc completion*") {
    Add-Content -Path $PROFILE -Value $CompletionCmd
    Write-Host "✅ Auto-completion added to your PowerShell profile." -ForegroundColor Green
}

Write-Host "`n🎉 Installation complete! Please RESTART your terminal." -ForegroundColor Cyan
Write-Host "Try running: owenc --help"
