<#
.SYNOPSIS
    One-time setup for host WinRM client (run as Administrator).

.DESCRIPTION
    Configures the host machine's WinRM client to allow unencrypted
    connections to the QEMU VM. This only needs to be run once.
    MUST be run as Administrator.
#>

$ErrorActionPreference = "Stop"

# Check admin
$isAdmin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole] "Administrator")
if (-not $isAdmin) {
    Write-Host "ERROR: This script must be run as Administrator." -ForegroundColor Red
    Write-Host "Right-click PowerShell > Run as Administrator, then run this script." -ForegroundColor Yellow
    exit 1
}

Write-Host "Configuring host WinRM client for VM communication..." -ForegroundColor Yellow

# Start WinRM service
Start-Service WinRM -ErrorAction SilentlyContinue
Set-Service WinRM -StartupType Manual

# Allow unencrypted (required for basic auth to local VM)
Set-Item WSMan:\localhost\Client\AllowUnencrypted $true -Force
Write-Host "  AllowUnencrypted: true" -ForegroundColor Green

# Trust localhost
Set-Item WSMan:\localhost\Client\TrustedHosts "127.0.0.1" -Force
Write-Host "  TrustedHosts: 127.0.0.1" -ForegroundColor Green

Write-Host ""
Write-Host "Host WinRM client configured successfully." -ForegroundColor Green
Write-Host ""
Write-Host "Next steps:" -ForegroundColor Cyan
Write-Host "  1. Launch VM:       .\.vm\launch.ps1" -ForegroundColor White
Write-Host "  2. In VM (once):    Run D:\setup-winrm.bat as Administrator" -ForegroundColor White
Write-Host "  3. From host:       .\.vm\vm-exec.ps1 'hostname'" -ForegroundColor White
Write-Host "  4. Install driver:  .\.vm\vm-install.ps1" -ForegroundColor White
