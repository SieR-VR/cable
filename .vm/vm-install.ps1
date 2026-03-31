<#
.SYNOPSIS
    Automated driver install and diagnostics via WinRM.

.DESCRIPTION
    Runs the full driver install sequence in the VM through WinRM,
    capturing all output for diagnosis. This replaces the manual
    install.bat workflow.

.PARAMETER Port
    WinRM port (default: 15985)

.PARAMETER Username
    VM user (default: User)

.PARAMETER Password
    VM password (default: cable123)

.PARAMETER DiagOnly
    Only run diagnostics, skip install

.EXAMPLE
    .\.vm\vm-install.ps1
    .\.vm\vm-install.ps1 -DiagOnly
#>

[CmdletBinding()]
param(
    [int]$Port = 15985,
    [string]$Username = "cable",
    [string]$Password = "cable123",
    [switch]$DiagOnly
)

$ErrorActionPreference = "Stop"
$VmDir = Split-Path -Parent $MyInvocation.MyCommand.Path

# Helper: run a command in the VM and return output
function Invoke-VM {
    param([string]$Cmd, [System.Management.Automation.Runspaces.PSSession]$Session)
    Write-Host "  > $Cmd" -ForegroundColor DarkGray
    $out = Invoke-Command -Session $Session -ScriptBlock {
        param($c)
        try { Invoke-Expression $c 2>&1 | Out-String } catch { $_.Exception.Message }
    } -ArgumentList $Cmd
    $out = $out.TrimEnd()
    if ($out) { Write-Host $out }
    return $out
}

# Connect
Write-Host "Connecting to VM on localhost:$Port ..." -ForegroundColor Yellow
$secPass = ConvertTo-SecureString $Password -AsPlainText -Force
$cred = New-Object System.Management.Automation.PSCredential($Username, $secPass)
$so = New-PSSessionOption -SkipCACheck -SkipCNCheck -SkipRevocationCheck

try {
    $session = New-PSSession -ComputerName "127.0.0.1" -Port $Port -Credential $cred `
        -Authentication Basic -SessionOption $so
} catch {
    Write-Host "ERROR: Cannot connect to VM." -ForegroundColor Red
    Write-Host "  - Is the VM running?" -ForegroundColor White
    Write-Host "  - Was setup-winrm.bat run inside the VM?" -ForegroundColor White
    exit 1
}

Write-Host "Connected!" -ForegroundColor Green
Write-Host ""

# ---- Diagnostics ----
Write-Host "========== VM DIAGNOSTICS ==========" -ForegroundColor Cyan

Write-Host "`n--- OS Version ---" -ForegroundColor Yellow
Invoke-VM "[System.Environment]::OSVersion.Version.ToString()" $session

Write-Host "`n--- Test Signing ---" -ForegroundColor Yellow
Invoke-VM "cmd /c bcdedit /enum `"{current}`" 2>&1 | Select-String 'testsigning'" $session

Write-Host "`n--- CD-ROM / Staging Drive ---" -ForegroundColor Yellow
Invoke-VM "Get-Volume | Where-Object { `$_.DriveType -eq 'CD-ROM' } | Format-Table DriveLetter, FileSystemLabel, DriveType -AutoSize" $session

Write-Host "`n--- Certificate Stores ---" -ForegroundColor Yellow
Invoke-VM "Get-ChildItem Cert:\LocalMachine\Root | Where-Object { `$_.Subject -match 'WDKTestCert|CableAudio|WDRLocal' } | Format-Table Subject, Thumbprint -AutoSize" $session
Invoke-VM "Get-ChildItem Cert:\LocalMachine\TrustedPublisher | Where-Object { `$_.Subject -match 'WDKTestCert|CableAudio|WDRLocal' } | Format-Table Subject, Thumbprint -AutoSize" $session

Write-Host "`n--- MEDIA class devices ---" -ForegroundColor Yellow
Invoke-VM "Get-PnpDevice -Class MEDIA -ErrorAction SilentlyContinue | Format-Table Status, InstanceId, FriendlyName -AutoSize" $session

Write-Host "`n--- Audio Endpoints ---" -ForegroundColor Yellow
Invoke-VM "Get-PnpDevice -Class AudioEndpoint -ErrorAction SilentlyContinue | Format-Table Status, FriendlyName -AutoSize" $session

if ($DiagOnly) {
    Write-Host "`n--- setupapi.dev.log (last 80 lines) ---" -ForegroundColor Yellow
    Invoke-VM "if (Test-Path C:\Windows\INF\setupapi.dev.log) { Get-Content C:\Windows\INF\setupapi.dev.log -Tail 80 } else { 'Log not found' }" $session
    Remove-PSSession $session
    exit 0
}

# ---- Install Sequence ----
Write-Host "`n========== DRIVER INSTALL ==========" -ForegroundColor Cyan

# Step 1: Ensure test signing
Write-Host "`n[1/6] Ensuring test signing is enabled..." -ForegroundColor Yellow
$ts = Invoke-VM "cmd /c bcdedit /enum `"{current}`" 2>&1 | Out-String" $session
if ($ts -match "testsigning\s+Yes") {
    Write-Host "  Test signing is active." -ForegroundColor Green
} else {
    Write-Host "  Enabling test signing (requires reboot)..." -ForegroundColor Yellow
    Invoke-VM "bcdedit /set testsigning on" $session
    Write-Host ""
    Write-Host "  TEST SIGNING WAS JUST ENABLED. The VM needs to REBOOT." -ForegroundColor Red
    Write-Host "  After reboot, run this script again." -ForegroundColor Red
    Invoke-VM "shutdown /r /t 5" $session
    Remove-PSSession $session
    exit 0
}

# Step 2: Find staging drive
Write-Host "`n[2/6] Finding staging drive..." -ForegroundColor Yellow
$drive = Invoke-VM "Get-Volume | Where-Object { `$_.FileSystemLabel -eq 'CABLE' } | Select-Object -ExpandProperty DriveLetter" $session
$drive = ($drive -replace '\s', '').Trim()
if (-not $drive) {
    Write-Host "  ERROR: CABLE staging drive not found!" -ForegroundColor Red
    Remove-PSSession $session
    exit 1
}
Write-Host "  Staging drive: ${drive}:" -ForegroundColor Green

# Step 3: Copy files locally
Write-Host "`n[3/6] Copying files to C:\CableAudio ..." -ForegroundColor Yellow
Invoke-VM "if (Test-Path C:\CableAudio) { Remove-Item C:\CableAudio -Recurse -Force }; New-Item -ItemType Directory -Path C:\CableAudio -Force | Out-Null" $session
Invoke-VM "Copy-Item ${drive}:\* C:\CableAudio\ -Force" $session
Invoke-VM "Get-ChildItem C:\CableAudio | Format-Table Name, Length -AutoSize" $session

# Step 4: Import certificate
Write-Host "`n[4/6] Importing certificate..." -ForegroundColor Yellow
$certPath = "C:\CableAudio\WDKTestCert.cer"
$certExists = Invoke-VM "Test-Path '$certPath'" $session
if ($certExists -match "True") {
    # Import using .NET directly (avoids certutil path issues)
    Invoke-VM @"
`$cert = New-Object System.Security.Cryptography.X509Certificates.X509Certificate2('$certPath')
Write-Output "Certificate: `$(`$cert.Subject)  Thumbprint: `$(`$cert.Thumbprint)"

# Add to Root store
`$rootStore = New-Object System.Security.Cryptography.X509Certificates.X509Store('Root', 'LocalMachine')
`$rootStore.Open('ReadWrite')
`$rootStore.Add(`$cert)
`$rootStore.Close()
Write-Output "  Added to Root store"

# Add to TrustedPublisher store
`$pubStore = New-Object System.Security.Cryptography.X509Certificates.X509Store('TrustedPublisher', 'LocalMachine')
`$pubStore.Open('ReadWrite')
`$pubStore.Add(`$cert)
`$pubStore.Close()
Write-Output "  Added to TrustedPublisher store"
"@ $session
} else {
    Write-Host "  WARNING: Certificate file not found at $certPath" -ForegroundColor Yellow
}

# Verify cert import
Write-Host "`n  Verifying cert in stores..." -ForegroundColor Yellow
Invoke-VM "Get-ChildItem Cert:\LocalMachine\Root | Where-Object { `$_.Subject -match 'WDKTestCert' } | Format-Table Subject, Thumbprint -AutoSize" $session
Invoke-VM "Get-ChildItem Cert:\LocalMachine\TrustedPublisher | Where-Object { `$_.Subject -match 'WDKTestCert' } | Format-Table Subject, Thumbprint -AutoSize" $session

# Step 5: Install driver
Write-Host "`n[5/6] Installing driver with devcon..." -ForegroundColor Yellow
$devconResult = Invoke-VM "& C:\CableAudio\devcon.exe install C:\CableAudio\CableAudio.inf ROOT\CableAudio 2>&1 | Out-String" $session
Write-Host "  devcon output:" -ForegroundColor White
Write-Host $devconResult

# Step 6: Check result
Write-Host "`n[6/6] Checking result..." -ForegroundColor Yellow
Invoke-VM "Get-PnpDevice -Class MEDIA -ErrorAction SilentlyContinue | Format-Table Status, InstanceId, FriendlyName -AutoSize" $session
Invoke-VM "Get-PnpDevice -Class AudioEndpoint -ErrorAction SilentlyContinue | Format-Table Status, FriendlyName -AutoSize" $session

Write-Host "`n--- setupapi.dev.log (last 60 lines) ---" -ForegroundColor Yellow
Invoke-VM "if (Test-Path C:\Windows\INF\setupapi.dev.log) { Get-Content C:\Windows\INF\setupapi.dev.log -Tail 60 } else { 'Log not found' }" $session

Remove-PSSession $session
Write-Host "`nDone." -ForegroundColor Green
