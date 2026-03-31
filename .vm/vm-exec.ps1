<#
.SYNOPSIS
    Execute a command inside the Cable test VM via WinRM.

.DESCRIPTION
    Connects to the QEMU VM via WinRM (port-forwarded to localhost:15985)
    and executes the given command. Returns the output.

    Prerequisites:
    - VM must be running (launched via .\.vm\launch.ps1)
    - WinRM must be configured in VM (run D:\setup-winrm.bat once)

.PARAMETER Command
    The command or PowerShell script block to execute in the VM.

.PARAMETER Username
    VM username (default: User)

.PARAMETER Password
    VM password (default: cable123)

.PARAMETER Port
    WinRM port on localhost (default: 15985)

.EXAMPLE
    .\.vm\vm-exec.ps1 "hostname"
    .\.vm\vm-exec.ps1 "Get-PnpDevice -Class MEDIA"
    .\.vm\vm-exec.ps1 "Get-Content C:\Windows\INF\setupapi.dev.log -Tail 80"
#>

[CmdletBinding()]
param(
    [Parameter(Mandatory, Position = 0)]
    [string]$Command,

    [string]$Username = "cable",
    [string]$Password = "cable123",
    [int]$Port = 15985
)

$ErrorActionPreference = "Stop"

# Build credential
$secPass = ConvertTo-SecureString $Password -AsPlainText -Force
$cred = New-Object System.Management.Automation.PSCredential($Username, $secPass)

# Create session options (allow unencrypted for local VM)
$so = New-PSSessionOption -SkipCACheck -SkipCNCheck -SkipRevocationCheck

try {
    $session = New-PSSession -ComputerName "127.0.0.1" -Port $Port -Credential $cred `
        -Authentication Basic -SessionOption $so

    $result = Invoke-Command -Session $session -ScriptBlock {
        param($cmd)
        # Execute as PowerShell
        Invoke-Expression $cmd
    } -ArgumentList $Command

    $result

    Remove-PSSession $session
} catch {
    if ($_.Exception.Message -match "connect|refused|WinRM") {
        Write-Host "ERROR: Cannot connect to VM on localhost:$Port" -ForegroundColor Red
        Write-Host "" -ForegroundColor Red
        Write-Host "Checklist:" -ForegroundColor Yellow
        Write-Host "  1. Is the VM running? (.\.vm\launch.ps1)" -ForegroundColor White
        Write-Host "  2. Has WinRM been set up? (Run D:\setup-winrm.bat in VM)" -ForegroundColor White
        Write-Host "  3. Has the VM finished booting?" -ForegroundColor White
    } else {
        Write-Error $_
    }
    exit 1
}
