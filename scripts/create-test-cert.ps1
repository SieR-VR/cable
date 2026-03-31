#Requires -Version 5.1
#Requires -RunAsAdministrator
<#
.SYNOPSIS
    Create a self-signed test certificate for driver signing.

.DESCRIPTION
    Generates a self-signed code signing certificate suitable for test-signing
    kernel-mode drivers. Exports both .pfx (for signtool) and .cer (for VM
    import into Trusted Root CA store).

    The certificate is created in the CurrentUser\My store and then exported.

    IMPORTANT: This cert is for DEVELOPMENT ONLY. Production drivers must be
    signed with an EV certificate and submitted to Microsoft for attestation
    signing.

.PARAMETER OutputDir
    Directory to write the .pfx and .cer files (default: target\debug)

.PARAMETER Subject
    Certificate subject CN (default: CableAudioTestCert)

.PARAMETER Password
    PFX export password (default: cable-test)

.PARAMETER ValidYears
    Certificate validity in years (default: 5)

.EXAMPLE
    .\create-test-cert.ps1
    .\create-test-cert.ps1 -OutputDir C:\certs -Password "mysecret"
#>

[CmdletBinding()]
param(
    [string]$OutputDir,
    [string]$Subject = "CableAudioTestCert",
    [string]$Password = "cable-test",
    [int]$ValidYears = 5
)

$ErrorActionPreference = "Stop"

# Default output to target\debug relative to project root
if (-not $OutputDir) {
    $ProjectRoot = Split-Path -Parent $PSScriptRoot
    $OutputDir = Join-Path $ProjectRoot "target\debug"
}

if (-not (Test-Path $OutputDir)) {
    New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null
}

$PfxPath = Join-Path $OutputDir "CableAudioTestCert.pfx"
$CerPath = Join-Path $OutputDir "WDKTestCert.cer"

Write-Host "========================================" -ForegroundColor Cyan
Write-Host " Cable Test Certificate Generator" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "  Subject  : CN=$Subject" -ForegroundColor White
Write-Host "  Output   : $OutputDir" -ForegroundColor White
Write-Host "  Validity : $ValidYears years" -ForegroundColor White
Write-Host ""

# Check for existing cert with same subject and remove it
$existing = Get-ChildItem Cert:\CurrentUser\My | Where-Object { $_.Subject -eq "CN=$Subject" }
if ($existing) {
    Write-Host "Removing existing certificate with subject CN=$Subject..." -ForegroundColor Yellow
    $existing | Remove-Item -Force
}

# Create the self-signed certificate
Write-Host "Creating self-signed code signing certificate..." -ForegroundColor Yellow

$cert = New-SelfSignedCertificate `
    -Type CodeSigningCert `
    -Subject "CN=$Subject" `
    -FriendlyName "Cable Audio Driver Test Certificate" `
    -CertStoreLocation Cert:\CurrentUser\My `
    -NotAfter (Get-Date).AddYears($ValidYears) `
    -KeyUsage DigitalSignature `
    -TextExtension @("2.5.29.37={text}1.3.6.1.5.5.7.3.3")

Write-Host "  Thumbprint: $($cert.Thumbprint)" -ForegroundColor Green
Write-Host ""

# Export PFX (for signtool)
Write-Host "Exporting PFX..." -ForegroundColor Yellow
$securePassword = ConvertTo-SecureString -String $Password -Force -AsPlainText
Export-PfxCertificate -Cert $cert -FilePath $PfxPath -Password $securePassword | Out-Null
Write-Host "  $PfxPath" -ForegroundColor Green

# Export CER (for VM import)
Write-Host "Exporting CER..." -ForegroundColor Yellow
Export-Certificate -Cert $cert -FilePath $CerPath | Out-Null
Write-Host "  $CerPath" -ForegroundColor Green

# Also install to Trusted Root CA on this machine (for local validation)
Write-Host ""
Write-Host "Installing certificate to LocalMachine\Root (Trusted Root CA)..." -ForegroundColor Yellow
try {
    $rootStore = New-Object System.Security.Cryptography.X509Certificates.X509Store("Root", "LocalMachine")
    $rootStore.Open("ReadWrite")
    $rootStore.Add($cert)
    $rootStore.Close()
    Write-Host "  Installed to Trusted Root CA store." -ForegroundColor Green
} catch {
    Write-Warning "Could not install to Trusted Root CA store: $_"
    Write-Warning "You may need to manually import $CerPath into the Trusted Root CA store."
}

Write-Host ""
Write-Host "========================================" -ForegroundColor Green
Write-Host " Certificate created successfully!" -ForegroundColor Green
Write-Host "========================================" -ForegroundColor Green
Write-Host ""
Write-Host "Files:" -ForegroundColor Cyan
Write-Host "  PFX: $PfxPath" -ForegroundColor White
Write-Host "  CER: $CerPath" -ForegroundColor White
Write-Host ""
Write-Host "PFX Password: $Password" -ForegroundColor Yellow
Write-Host ""
Write-Host "Usage with signtool:" -ForegroundColor Cyan
Write-Host "  signtool sign /fd SHA256 /f `"$PfxPath`" /p `"$Password`" <file.sys>" -ForegroundColor White
Write-Host ""
Write-Host "To import CER in the VM (elevated cmd):" -ForegroundColor Cyan
Write-Host "  certutil -addstore Root WDKTestCert.cer" -ForegroundColor White
Write-Host "  certutil -addstore TrustedPublisher WDKTestCert.cer" -ForegroundColor White
