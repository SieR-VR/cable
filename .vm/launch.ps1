<#
.SYNOPSIS
    Launch a QEMU VM for testing the CableAudio driver.

.DESCRIPTION
    Creates a fresh overlay from the base Windows install, prepares a staging
    ISO containing the driver package, and launches QEMU with WHPX acceleration.

    The driver files will be accessible inside the VM as a CD-ROM drive (D: or E:).

    Inside the VM, open an elevated command prompt and run:
        D:\install.bat
    Or manually:
        pnputil /add-driver D:\CableAudio.inf /install

.PARAMETER Fresh
    Create a fresh overlay from the clean install (ignore existing test image)

.PARAMETER Memory
    RAM in MB (default: 4096)

.PARAMETER Cores
    CPU cores (default: 2)

.EXAMPLE
    .\.vm\launch.ps1
    .\.vm\launch.ps1 -Memory 8192 -Cores 4
#>

[CmdletBinding()]
param(
    [switch]$Fresh,
    [switch]$Reuse,
    [int]$Memory = 4096,
    [int]$Cores = 2
)

$ErrorActionPreference = "Stop"
$VmDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRoot = Split-Path -Parent $VmDir
$DriverPkg = Join-Path $ProjectRoot "driver\x64\Debug\package"
$StagingDir = Join-Path $VmDir "staging"

Write-Host "========================================" -ForegroundColor Cyan
Write-Host " Cable VM Test Environment" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# ---- Verify driver package exists ----
if (-not (Test-Path (Join-Path $DriverPkg "CableAudio.sys"))) {
    Write-Host "ERROR: Driver package not found at $DriverPkg" -ForegroundColor Red
    Write-Host "Run the driver build first:" -ForegroundColor Yellow
    Write-Host "  .\driver\scripts\build.ps1 -Configuration Debug" -ForegroundColor Yellow
    exit 1
}

# ---- Prepare staging directory with driver files + install script ----
Write-Host "Preparing staging directory..." -ForegroundColor Yellow

if (Test-Path $StagingDir) {
    Remove-Item $StagingDir -Recurse -Force
}
New-Item -ItemType Directory -Path $StagingDir -Force | Out-Null

# Copy driver package
Copy-Item (Join-Path $DriverPkg "CableAudio.sys") $StagingDir
Copy-Item (Join-Path $DriverPkg "CableAudio.inf") $StagingDir
Copy-Item (Join-Path $DriverPkg "cableaudio.cat") $StagingDir

# Copy the signing certificate from the build output.
# The build produces a .cer alongside the package that matches the signing cert.
$CertSources = @(
    (Join-Path $ProjectRoot "driver\x64\Debug\package.cer"),
    (Join-Path $ProjectRoot "driver\Source\Main\x64\Debug\CableAudio.cer")
)
$CertFound = $false
foreach ($src in $CertSources) {
    if (Test-Path $src) {
        $CertDest = Join-Path $StagingDir "WDKTestCert.cer"
        Copy-Item $src $CertDest -Force
        $x509 = New-Object System.Security.Cryptography.X509Certificates.X509Certificate2($src)
        Write-Host "  Signing cert: $($x509.Subject) (thumbprint: $($x509.Thumbprint))" -ForegroundColor Green
        $CertFound = $true
        break
    }
}
if (-not $CertFound) {
    Write-Host "  WARNING: No signing certificate found - certificate import will be skipped" -ForegroundColor Yellow
}

# Copy devcon.exe from WDK (required to create ROOT-enumerated device node)
$DevconSrc = "C:\Program Files (x86)\Windows Kits\10\Tools\10.0.26100.0\x64\devcon.exe"
if (Test-Path $DevconSrc) {
    Copy-Item $DevconSrc $StagingDir
    Write-Host "  Included devcon.exe" -ForegroundColor Green
} else {
    # Search for any available version
    $found = Get-ChildItem "C:\Program Files (x86)\Windows Kits\10\Tools" -Recurse -Filter "devcon.exe" -ErrorAction SilentlyContinue |
        Where-Object { $_.FullName -match "x64" } | Select-Object -First 1
    if ($found) {
        Copy-Item $found.FullName $StagingDir
        Write-Host "  Included devcon.exe (from $($found.FullName))" -ForegroundColor Green
    } else {
        Write-Host "  WARNING: devcon.exe not found in WDK" -ForegroundColor Yellow
    }
}

# Create install batch script for convenience inside the VM
$installBat = @"
@echo off
echo ========================================
echo Cable Audio Driver Installer
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
echo [1/5] Checking test signing...
bcdedit /enum {current} | findstr /i "testsigning.*Yes" >nul 2>&1
if errorlevel 1 (
    echo Enabling test signing...
    bcdedit /set testsigning on
    echo.
    echo Test signing enabled. A REBOOT is required.
    echo After reboot, run this script again.
    echo.
    pause
    shutdown /r /t 5
    exit /b 0
)
echo       Test signing is enabled.
echo.

:: Copy all files to C:\CableAudio (short, no-spaces path avoids certutil bugs)
echo [2/5] Copying driver files to local disk...
set LOCALDIR=C:\CableAudio
if exist "%LOCALDIR%" rmdir /s /q "%LOCALDIR%"
mkdir "%LOCALDIR%"
copy "%~dp0CableAudio.sys" "%LOCALDIR%\" >nul
copy "%~dp0CableAudio.inf" "%LOCALDIR%\" >nul
copy "%~dp0cableaudio.cat" "%LOCALDIR%\" >nul
if exist "%~dp0devcon.exe" copy "%~dp0devcon.exe" "%LOCALDIR%\" >nul
if exist "%~dp0WDKTestCert.cer" copy "%~dp0WDKTestCert.cer" "%LOCALDIR%\" >nul
echo       Files copied to %LOCALDIR%
dir "%LOCALDIR%"
echo.

:: Import test certificate if present
echo [3/5] Importing test certificate...
if not exist "%LOCALDIR%\WDKTestCert.cer" (
    echo       No certificate file found, skipping.
    goto :skip_cert
)
echo       Certificate file: %LOCALDIR%\WDKTestCert.cer
echo.
echo       --- Adding to Root store ---
certutil -addstore Root "%LOCALDIR%\WDKTestCert.cer"
if errorlevel 1 (
    echo.
    echo       WARNING: Root store import failed. Trying alternate method...
    certutil -f -addstore Root "%LOCALDIR%\WDKTestCert.cer"
)
echo.
echo       --- Adding to TrustedPublisher store ---
certutil -addstore TrustedPublisher "%LOCALDIR%\WDKTestCert.cer"
if errorlevel 1 (
    echo.
    echo       WARNING: TrustedPublisher store import failed. Trying alternate method...
    certutil -f -addstore TrustedPublisher "%LOCALDIR%\WDKTestCert.cer"
)
echo.
:skip_cert

:: Verify cert is now trusted
echo [4/5] Verifying certificate trust...
certutil -verifystore Root >nul 2>&1
echo       Certificates in Root store:
certutil -store Root | findstr /i "WDKTestCert"
echo       Certificates in TrustedPublisher store:
certutil -store TrustedPublisher | findstr /i "WDKTestCert"
echo.

:: Install driver using devcon
echo [5/5] Installing driver...
if not exist "%LOCALDIR%\devcon.exe" (
    echo ERROR: devcon.exe not found.
    pause
    exit /b 1
)

echo Running: devcon.exe install CableAudio.inf ROOT\CableAudio
echo.
"%LOCALDIR%\devcon.exe" install "%LOCALDIR%\CableAudio.inf" ROOT\CableAudio
set DEVCON_RC=%errorlevel%
echo.
echo devcon exit code: %DEVCON_RC%

if %DEVCON_RC% neq 0 (
    echo.
    echo === INSTALL FAILED ===
    echo.
    echo Troubleshooting:
    echo   1. Check Device Manager for any yellow-bang devices
    echo   2. Check setupapi.dev.log:
    echo      powershell -Command "Get-Content C:\Windows\INF\setupapi.dev.log -Tail 100"
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

# Create uninstall script
$uninstallBat = @"
@echo off
echo ========================================
echo Cable Audio Driver Uninstaller
echo ========================================
echo.

net session >nul 2>&1
if errorlevel 1 (
    echo ERROR: This script must be run as Administrator.
    pause
    exit /b 1
)

echo Removing CableAudio driver...
pnputil /delete-driver CableAudio.inf /uninstall /force
echo.
echo Done. The driver should be removed.
pause
"@

Set-Content -Path (Join-Path $StagingDir "uninstall.bat") -Value $uninstallBat -Encoding ASCII

# Create a quick status check script
$statusBat = @"
@echo off
echo ========================================
echo Cable Audio Driver Status
echo ========================================
echo.
echo --- Installed drivers matching Cable ---
pnputil /enum-drivers | findstr /i "cable"
echo.
echo --- Audio devices ---
powershell -Command "Get-PnpDevice -Class AudioEndpoint -ErrorAction SilentlyContinue | Format-Table Status, FriendlyName -AutoSize"
echo.
echo --- Device Manager: MEDIA class ---
powershell -Command "Get-PnpDevice -Class MEDIA -ErrorAction SilentlyContinue | Format-Table Status, FriendlyName -AutoSize"
echo.
pause
"@

Set-Content -Path (Join-Path $StagingDir "status.bat") -Value $statusBat -Encoding ASCII

# Create a certificate-only fix script (for when device exists but has Code 52)
$fixCertBat = @"
@echo off
echo ========================================
echo Cable Audio - Fix Certificate (Code 52)
echo ========================================
echo.

net session >nul 2>&1
if errorlevel 1 (
    echo ERROR: This script must be run as Administrator.
    pause
    exit /b 1
)

:: Copy cert to a known-good local path
echo Copying certificate to C:\CableAudio ...
if not exist C:\CableAudio mkdir C:\CableAudio
copy "%~dp0WDKTestCert.cer" "C:\CableAudio\WDKTestCert.cer" >nul 2>&1
if not exist "C:\CableAudio\WDKTestCert.cer" (
    echo ERROR: Certificate file not found on source media.
    pause
    exit /b 1
)

echo.
echo --- Importing to Root store ---
certutil -f -addstore Root "C:\CableAudio\WDKTestCert.cer"
echo.
echo --- Importing to TrustedPublisher store ---
certutil -f -addstore TrustedPublisher "C:\CableAudio\WDKTestCert.cer"
echo.

echo --- Verifying import ---
echo Root store:
certutil -store Root | findstr /i "WDKTestCert nwh63"
echo TrustedPublisher store:
certutil -store TrustedPublisher | findstr /i "WDKTestCert nwh63"
echo.

echo Now re-scan the device in Device Manager:
echo   Right-click the Cable device ^> Disable ^> Enable
echo   Or run: pnputil /scan-devices
echo.
pnputil /scan-devices
echo.
pause
"@

Set-Content -Path (Join-Path $StagingDir "fix-cert.bat") -Value $fixCertBat -Encoding ASCII

# Create WinRM setup script (run ONCE in VM to enable remote management)
$setupWinrmBat = @"
@echo off
echo ========================================
echo Cable VM - WinRM Setup (run once)
echo ========================================
echo.

net session >nul 2>&1
if errorlevel 1 (
    echo ERROR: This script must be run as Administrator.
    pause
    exit /b 1
)

echo [1/6] Enabling test signing...
bcdedit /set testsigning on
echo.

echo [2/6] Network check...
ipconfig
echo.

echo [3/6] Starting WinRM service...
sc config WinRM start= auto
net start WinRM
if errorlevel 1 (
    echo.
    echo ERROR: WinRM service failed to start.
    echo This may be because the service is missing in this Windows edition.
    echo.
    echo Trying PowerShell remoting setup instead...
    powershell -Command "Enable-PSRemoting -Force -SkipNetworkProfileCheck" 2>nul
)
echo.

echo [4/6] Configuring WinRM...
winrm quickconfig -quiet -force
winrm set winrm/config/service @{AllowUnencrypted="true"}
winrm set winrm/config/service/auth @{Basic="true"}
echo.

echo [5/6] Setting password for cable account...
net user cable cable123
echo.

echo [6/6] Configuring firewall...
netsh advfirewall firewall add rule name="WinRM-HTTP" dir=in localport=5985 protocol=tcp action=allow 2>nul
echo.

echo ========================================
echo Verifying WinRM listener...
winrm enumerate winrm/config/listener
echo.
echo Testing WinRM locally...
winrm identify -r:http://localhost:5985 -auth:basic -u:cable -p:cable123 -encoding:utf-8
echo ========================================
echo.
echo If you see listener info above, WinRM is ready.
echo From the HOST, run:  .\.vm\vm-exec.ps1 "hostname"
echo.
echo SHUT DOWN this VM to commit changes to base image.
echo.
pause
"@

Set-Content -Path (Join-Path $StagingDir "setup-winrm.bat") -Value $setupWinrmBat -Encoding ASCII

Write-Host "  Staging directory ready: $StagingDir" -ForegroundColor Green
Get-ChildItem $StagingDir | ForEach-Object { Write-Host "    $($_.Name)" -ForegroundColor White }
Write-Host ""

# ---- VM disk setup ----
$BaseImg = Join-Path $VmDir "tiny11-cleaninstall.qcow2"
$TestImg = Join-Path $VmDir "tiny11-test.qcow2"

if (-not (Test-Path $BaseImg)) {
    Write-Host "ERROR: Base image not found: $BaseImg" -ForegroundColor Red
    exit 1
}

if ($Reuse -and (Test-Path $TestImg)) {
    # Reuse existing overlay (for WHPX reboot workaround: shutdown VM, then relaunch with -Reuse)
    Write-Host "Reusing existing test image (WHPX reboot workaround)..." -ForegroundColor Yellow
    Write-Host "  $TestImg" -ForegroundColor Green
} else {
    # Create fresh overlay
    if (Test-Path $TestImg) {
        Write-Host "Removing previous test image..." -ForegroundColor Yellow
        Remove-Item $TestImg -Force
    }
    Write-Host "Creating overlay from base install..." -ForegroundColor Yellow
    & qemu-img create -f qcow2 -b "tiny11-cleaninstall.qcow2" -F qcow2 $TestImg
    if ($LASTEXITCODE -ne 0) { Write-Host "ERROR: qemu-img create failed" -ForegroundColor Red; exit 1 }
    Write-Host "  Created: $TestImg" -ForegroundColor Green
}

Write-Host ""

# ---- Create ISO for driver staging (with long filename support) ----
# Uses Windows built-in IMAPI2 COM API to create a Joliet ISO that preserves
# full filenames. This works on all Windows 10/11 without external tools.
$IsoPath = Join-Path $VmDir "staging.iso"

if (Test-Path $IsoPath) { Remove-Item $IsoPath -Force }

Write-Host "Creating staging ISO..." -ForegroundColor Yellow

# Add a C# helper to write IMAPI2 IStream to a file (PowerShell cannot call
# IStream.Read directly due to COM interop limitations).
if (-not ([System.Management.Automation.PSTypeName]'IsoStreamHelper').Type) {
    Add-Type -TypeDefinition @"
using System;
using System.IO;
using System.Runtime.InteropServices;
using System.Runtime.InteropServices.ComTypes;

public class IsoStreamHelper {
    public static void WriteStreamToFile(object comStream, string outputPath) {
        IStream stream = (IStream)comStream;
        System.Runtime.InteropServices.ComTypes.STATSTG stat;
        stream.Stat(out stat, 0);
        long totalSize = stat.cbSize;

        byte[] buf = new byte[65536];
        using (FileStream fs = File.Create(outputPath)) {
            long written = 0;
            while (written < totalSize) {
                int toRead = (int)Math.Min(buf.Length, totalSize - written);
                IntPtr bytesReadPtr = Marshal.AllocCoTaskMem(sizeof(int));
                try {
                    stream.Read(buf, toRead, bytesReadPtr);
                    int bytesRead = Marshal.ReadInt32(bytesReadPtr);
                    if (bytesRead <= 0) break;
                    fs.Write(buf, 0, bytesRead);
                    written += bytesRead;
                } finally {
                    Marshal.FreeCoTaskMem(bytesReadPtr);
                }
            }
        }
    }
}
"@
}

try {
    $fsi = New-Object -ComObject IMAPI2FS.MsftFileSystemImage
    $fsi.FileSystemsToCreate = 3  # FsiFileSystemISO9660 + FsiFileSystemJoliet
    $fsi.VolumeName = "CABLE"
    $fsi.Root.AddTree($StagingDir, $false)

    $result = $fsi.CreateResultImage()
    [IsoStreamHelper]::WriteStreamToFile($result.ImageStream, $IsoPath)

    $isoSize = (Get-Item $IsoPath).Length
    Write-Host "  ISO created: $IsoPath ($([math]::Round($isoSize / 1KB)) KB)" -ForegroundColor Green
    $UseIso = $true
} catch {
    Write-Host "  IMAPI2 ISO creation failed: $_" -ForegroundColor Yellow
    Write-Host "  Falling back to fat: virtual drive (filenames may be truncated)." -ForegroundColor Yellow
    $UseIso = $false
}

# ---- Launch QEMU ----
Write-Host ""
Write-Host "Launching QEMU VM (cold boot with WHPX)..." -ForegroundColor Yellow
Write-Host "  Memory: ${Memory}MB" -ForegroundColor White
Write-Host "  Cores: $Cores" -ForegroundColor White
Write-Host "  Accelerator: whpx" -ForegroundColor White
if ($UseIso) {
    Write-Host "  Driver staging: ISO ($IsoPath)" -ForegroundColor White
} else {
    Write-Host "  Driver staging: fat: virtual drive ($StagingDir)" -ForegroundColor White
}
Write-Host ""
Write-Host "--- VM Instructions ---" -ForegroundColor Cyan
Write-Host "1. If first time: open elevated cmd, run 'D:\setup-winrm.bat'" -ForegroundColor White
Write-Host "2. After WinRM setup: use '.\.vm\vm-exec.ps1 <command>' from host" -ForegroundColor White
Write-Host "3. Or manually: run 'D:\install.bat' as Administrator in VM" -ForegroundColor White
Write-Host "4. WinRM port forwarded: localhost:15985 -> VM:5985" -ForegroundColor White
Write-Host "-----------------------" -ForegroundColor Cyan
Write-Host ""

$qemuArgs = @(
    "-accel", "whpx",
    "-cpu", "Broadwell",
    "-smp", "$Cores",
    "-m", "${Memory}M",
    "-drive", "file=$TestImg,format=qcow2,if=ide",
    "-device", "VGA,vgamem_mb=64",
    "-device", "e1000,netdev=net0",
    "-netdev", "user,id=net0,hostfwd=tcp::15985-:5985",
    "-usb",
    "-device", "usb-tablet",
    "-boot", "c",
    "-name", "Cable Driver Test VM",
    "-qmp", "tcp:127.0.0.1:14444,server,nowait"
)

# Add staging drive (ISO preferred; fat: as fallback)
if ($UseIso) {
    $qemuArgs += @("-drive", "file=$IsoPath,format=raw,media=cdrom,readonly=on")
} else {
    $qemuArgs += @("-drive", "file=fat:rw:$StagingDir,format=raw,if=ide")
}

Write-Host "qemu-system-x86_64 $($qemuArgs -join ' ')" -ForegroundColor DarkGray
Write-Host ""

& qemu-system-x86_64 @qemuArgs

# Cleanup after VM exits
Write-Host ""
Write-Host "Cleaning up temporary files..." -ForegroundColor Yellow
# Always preserve the overlay so it can be reused with -Reuse.
# Only -Fresh at next launch will create a new overlay.
if (Test-Path $TestImg) {
    Write-Host "  Preserving test image for reuse" -ForegroundColor Yellow
    Write-Host "  To relaunch with existing state: .\.vm\launch.ps1 -Reuse" -ForegroundColor Yellow
    Write-Host "  To start fresh:                  .\.vm\launch.ps1 -Fresh" -ForegroundColor Yellow
}
if (Test-Path $IsoPath) {
    Remove-Item $IsoPath -Force
    Write-Host "  Removed staging ISO" -ForegroundColor Green
}
Write-Host "Done." -ForegroundColor Green
