#Requires -Version 5.1
<#
.SYNOPSIS
    Package the Cable Audio Driver for distribution/testing.

.DESCRIPTION
    Creates a zip archive containing the driver package, test certificate,
    and convenience install/uninstall/status scripts. The zip can be
    transferred to a test VM or target machine.

.PARAMETER Configuration
    Build configuration to package: Debug or Release (default: Debug)

.PARAMETER Platform
    Target platform: x64 or ARM64 (default: x64)

.PARAMETER OutputPath
    Output zip file path (default: target\CableAudio-<Platform>-<Config>.zip)

.EXAMPLE
    .\scripts\package-driver.ps1
    .\scripts\package-driver.ps1 -Configuration Release -Platform x64
#>

[CmdletBinding()]
param(
    [ValidateSet("Debug", "Release")]
    [string]$Configuration = "Debug",

    [ValidateSet("x64", "ARM64")]
    [string]$Platform = "x64",

    [string]$OutputPath
)

$ErrorActionPreference = "Stop"
$ProjectRoot = Split-Path -Parent $PSScriptRoot

Write-Host "========================================" -ForegroundColor Cyan
Write-Host " Cable Audio Driver Packager" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# ---- Locate driver package ----
$DriverPkg = Join-Path $ProjectRoot "driver\$Platform\$Configuration\package"

if (-not (Test-Path (Join-Path $DriverPkg "CableAudio.sys"))) {
    Write-Host "ERROR: Driver package not found at $DriverPkg" -ForegroundColor Red
    Write-Host "Build the driver first:" -ForegroundColor Yellow
    Write-Host "  .\scripts\build.ps1 -Target Driver -Configuration $Configuration -Platform $Platform" -ForegroundColor Yellow
    exit 1
}

# ---- Create staging directory ----
$StagingDir = Join-Path $ProjectRoot "target\package-staging"
if (Test-Path $StagingDir) {
    Remove-Item $StagingDir -Recurse -Force
}
New-Item -ItemType Directory -Path $StagingDir -Force | Out-Null

Write-Host "Staging driver files..." -ForegroundColor Yellow

# Copy driver files
Copy-Item (Join-Path $DriverPkg "CableAudio.sys") $StagingDir
Copy-Item (Join-Path $DriverPkg "CableAudio.inf") $StagingDir
Copy-Item (Join-Path $DriverPkg "cableaudio.cat") $StagingDir

# Copy test certificates if they exist
$cerFile = Join-Path $ProjectRoot "target\debug\WDKTestCert.cer"
$pfxFile = Join-Path $ProjectRoot "target\debug\CableAudioTestCert.pfx"
if (Test-Path $cerFile) {
    Copy-Item $cerFile $StagingDir
    Write-Host "  Included: WDKTestCert.cer" -ForegroundColor Green
}
if (Test-Path $pfxFile) {
    Copy-Item $pfxFile $StagingDir
    Write-Host "  Included: CableAudioTestCert.pfx" -ForegroundColor Green
}

# Copy devcon.exe from WDK
$DevconSrc = "C:\Program Files (x86)\Windows Kits\10\Tools\10.0.26100.0\x64\devcon.exe"
if (-not (Test-Path $DevconSrc)) {
    $found = Get-ChildItem "C:\Program Files (x86)\Windows Kits\10\Tools" -Recurse -Filter "devcon.exe" -ErrorAction SilentlyContinue |
        Where-Object { $_.FullName -match "x64" } | Select-Object -First 1
    if ($found) { $DevconSrc = $found.FullName }
}
if (Test-Path $DevconSrc) {
    Copy-Item $DevconSrc $StagingDir
    Write-Host "  Included: devcon.exe" -ForegroundColor Green
} else {
    Write-Warning "devcon.exe not found in WDK - driver installation in the package will fail"
}

# ---- Create install script ----
$installBat = @"
@echo off
echo ========================================
echo  Cable Audio Driver Installer
echo ========================================
echo.

:: Check for admin
net session >nul 2>&1
if errorlevel 1 (
    echo ERROR: This script must be run as Administrator.
    echo Right-click and select "Run as administrator".
    pause
    exit /b 1
)

:: Enable test signing if not already
echo [1/4] Checking test signing...
bcdedit /enum {current} | findstr /i "testsigning.*Yes" >nul 2>&1
if errorlevel 1 (
    echo Enabling test signing...
    bcdedit /set testsigning on
    echo.
    echo Test signing enabled. A REBOOT is required before installing the driver.
    echo After reboot, run this script again.
    echo.
    pause
    shutdown /r /t 10
    exit /b 0
)
echo       Test signing is enabled.
echo.

:: Copy files to a local temp directory
echo [2/4] Copying driver files to local disk...
set LOCALDIR=%TEMP%\CableAudioInstall
if exist "%LOCALDIR%" rmdir /s /q "%LOCALDIR%"
mkdir "%LOCALDIR%"
copy "%~dp0CableAudio.sys" "%LOCALDIR%\" >nul
copy "%~dp0CableAudio.inf" "%LOCALDIR%\" >nul
copy "%~dp0cableaudio.cat" "%LOCALDIR%\" >nul
if exist "%~dp0devcon.exe" copy "%~dp0devcon.exe" "%LOCALDIR%\" >nul
if exist "%~dp0WDKTestCert.cer" copy "%~dp0WDKTestCert.cer" "%LOCALDIR%\" >nul
echo       Files copied to %LOCALDIR%
echo.

:: Import test certificate if present
echo [3/4] Importing test certificate...
if exist "%LOCALDIR%\WDKTestCert.cer" (
    certutil -addstore Root "%LOCALDIR%\WDKTestCert.cer"
    certutil -addstore TrustedPublisher "%LOCALDIR%\WDKTestCert.cer"
    echo       Certificate imported.
) else (
    echo       No certificate found, skipping.
)
echo.

:: Install driver using devcon
echo [4/4] Installing driver...
if not exist "%LOCALDIR%\devcon.exe" (
    echo ERROR: devcon.exe not found.
    pause
    exit /b 1
)

echo Running: devcon.exe install CableAudio.inf ROOT\CableAudio
echo.
"%LOCALDIR%\devcon.exe" install "%LOCALDIR%\CableAudio.inf" ROOT\CableAudio
echo.
echo devcon exit code: %errorlevel%

if %errorlevel% neq 0 (
    echo.
    echo === INSTALL FAILED ===
    echo.
    echo Check C:\Windows\INF\setupapi.dev.log for details.
    echo.
) else (
    echo.
    echo === INSTALL SUCCEEDED ===
    echo Check Device Manager for "Cable Virtual Audio Device"
)

echo.
pause
"@

Set-Content -Path (Join-Path $StagingDir "install.bat") -Value $installBat -Encoding ASCII

# ---- Create uninstall script ----
$uninstallBat = @"
@echo off
echo ========================================
echo  Cable Audio Driver Uninstaller
echo ========================================
echo.

net session >nul 2>&1
if errorlevel 1 (
    echo ERROR: This script must be run as Administrator.
    pause
    exit /b 1
)

echo Listing installed Cable drivers...
for /f "tokens=3" %%a in ('pnputil /enum-drivers ^| findstr /i "cableaudio.inf"') do (
    echo Found: %%a
    echo Removing %%a...
    pnputil /delete-driver %%a /uninstall /force
)

echo.
echo Uninstall complete.
pause
"@

Set-Content -Path (Join-Path $StagingDir "uninstall.bat") -Value $uninstallBat -Encoding ASCII

# ---- Create status script ----
$statusBat = @"
@echo off
echo ========================================
echo  Cable Audio Driver Status
echo ========================================
echo.

echo --- Installed drivers matching "Cable" ---
pnputil /enum-drivers | findstr /i "cable"
echo.

echo --- Audio endpoint devices ---
powershell -Command "Get-PnpDevice -Class AudioEndpoint -ErrorAction SilentlyContinue | Format-Table Status, FriendlyName -AutoSize"
echo.

echo --- MEDIA class devices ---
powershell -Command "Get-PnpDevice -Class MEDIA -ErrorAction SilentlyContinue | Format-Table Status, FriendlyName -AutoSize"
echo.
pause
"@

Set-Content -Path (Join-Path $StagingDir "status.bat") -Value $statusBat -Encoding ASCII

# ---- Create README ----
$readme = @"
Cable Audio Driver Package
==========================

Configuration: $Configuration
Platform: $Platform
Built: $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss')

Files:
  CableAudio.sys          - Kernel driver binary
  CableAudio.inf          - Driver installation file
  cableaudio.cat          - Driver catalog (signature)
  WDKTestCert.cer    - Test certificate (if included)
  CableAudioTestCert.pfx  - Test certificate with private key (if included)
  install.bat             - Automated installer (run as Administrator)
  uninstall.bat           - Automated uninstaller (run as Administrator)
  status.bat              - Check driver status

Installation:
  1. Right-click install.bat -> Run as administrator
  2. If prompted, reboot and run install.bat again
  3. Check Device Manager for "Cable Virtual Audio Device"

Requirements:
  - Windows 10/11 x64
  - Test signing must be enabled (install.bat handles this)
  - Test certificate must be trusted (install.bat handles this)
"@

Set-Content -Path (Join-Path $StagingDir "README.txt") -Value $readme -Encoding ASCII

# ---- Create zip ----
if (-not $OutputPath) {
    $targetDir = Join-Path $ProjectRoot "target"
    if (-not (Test-Path $targetDir)) {
        New-Item -ItemType Directory -Path $targetDir -Force | Out-Null
    }
    $OutputPath = Join-Path $targetDir "CableAudio-$Platform-$Configuration.zip"
}

if (Test-Path $OutputPath) {
    Remove-Item $OutputPath -Force
}

Write-Host ""
Write-Host "Creating archive..." -ForegroundColor Yellow

Compress-Archive -Path (Join-Path $StagingDir "*") -DestinationPath $OutputPath -CompressionLevel Optimal

# Cleanup staging
Remove-Item $StagingDir -Recurse -Force

# Report
$zipSize = (Get-Item $OutputPath).Length
$zipSizeMB = [math]::Round($zipSize / 1MB, 2)

Write-Host ""
Write-Host "========================================" -ForegroundColor Green
Write-Host " Package created successfully!" -ForegroundColor Green
Write-Host "========================================" -ForegroundColor Green
Write-Host ""
Write-Host "  File: $OutputPath" -ForegroundColor White
Write-Host "  Size: $zipSizeMB MB" -ForegroundColor White
Write-Host ""
Write-Host "Contents:" -ForegroundColor Cyan

# List zip contents
$zip = [System.IO.Compression.ZipFile]::OpenRead($OutputPath)
try {
    foreach ($entry in $zip.Entries) {
        Write-Host "  $($entry.FullName) ($($entry.Length) bytes)" -ForegroundColor White
    }
} finally {
    $zip.Dispose()
}

Write-Host ""
