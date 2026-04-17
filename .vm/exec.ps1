<#
.SYNOPSIS
    Execute a command inside the Cable test VM via WinRM.

.DESCRIPTION
    Connects to the VM via WinRM and executes the given command.
    Returns the output.

    Prerequisites:
    - VM must be running (recommended: .\.vm\e2e.ps1)
    - WinRM must be configured in VM (run D:\setup-winrm.bat once)

.PARAMETER Command
    The command or PowerShell script block to execute in the VM.

.PARAMETER Username
    VM username (default: User)

.PARAMETER Password
    VM password (default: cable123)

.PARAMETER ComputerName
    VM WinRM host (default: 192.168.23.128)

.PARAMETER Port
    VM WinRM port (default: 5985)

.EXAMPLE
    .\.vm\exec.ps1 "hostname"
    .\.vm\exec.ps1 "Get-PnpDevice -Class MEDIA"
    .\.vm\exec.ps1 "Get-Content C:\Windows\INF\setupapi.dev.log -Tail 80"
#>

[CmdletBinding()]
param(
    [Parameter(Mandatory, Position = 0)]
    [string]$Command,

    [string]$Username = "cable",
    [string]$Password = "cable123",
    [string]$ComputerName = "192.168.23.128",
    [int]$Port = 5985
)

$ErrorActionPreference = "Stop"

# Build credential
$secPass = ConvertTo-SecureString $Password -AsPlainText -Force
$cred = New-Object System.Management.Automation.PSCredential($Username, $secPass)

# Create session options (allow unencrypted for local VM)
$so = New-PSSessionOption -SkipCACheck -SkipCNCheck -SkipRevocationCheck

function Test-TrustedHostMatch {
    param(
        [string]$TargetHost,
        [string]$TrustedHosts
    )

    if ([string]::IsNullOrWhiteSpace($TrustedHosts)) { return $false }
    if ($TrustedHosts -eq "*") { return $true }

    $entries = $TrustedHosts.Split(',') | ForEach-Object { $_.Trim() } | Where-Object { $_ }
    foreach ($entry in $entries) {
        if ($entry -eq $TargetHost) { return $true }
        if ($entry -eq "*") { return $true }
    }
    return $false
}

try {
    # For HTTP + Basic to an IP/non-Kerberos target, WinRM requires TrustedHosts.
    $trustedHostsValue = (Get-Item WSMan:\localhost\Client\TrustedHosts).Value
    if (-not (Test-TrustedHostMatch -TargetHost $ComputerName -TrustedHosts $trustedHostsValue)) {
        throw ("WinRM client TrustedHosts does not include '{0}'. Current TrustedHosts='{1}'. Add it with: " +
            "Set-Item WSMan:\localhost\Client\TrustedHosts -Value '{2},{0}' -Force (run as Administrator).") -f $ComputerName, $trustedHostsValue, $trustedHostsValue
    }

    $session = New-PSSession -ComputerName $ComputerName -Port $Port -Credential $cred `
        -Authentication Basic -SessionOption $so

    $result = Invoke-Command -Session $session -ScriptBlock {
        param($cmd)
        # Execute as PowerShell
        Invoke-Expression $cmd
    } -ArgumentList $Command

    $result

    Remove-PSSession $session
} catch {
    if ($_.Exception.Message -match "connect|refused|WinRM|TrustedHosts") {
        Write-Host "ERROR: Cannot connect to VM on ${ComputerName}:$Port" -ForegroundColor Red
        Write-Host "" -ForegroundColor Red
        Write-Host "Checklist:" -ForegroundColor Yellow
        Write-Host "  1. Is the VM running? (.\.vm\e2e.ps1)" -ForegroundColor White
        Write-Host "  2. Has WinRM been set up? (Run D:\setup-winrm.bat in VM)" -ForegroundColor White
        Write-Host "  3. Has the VM finished booting?" -ForegroundColor White
        Write-Host "  4. Is host TrustedHosts configured for '$ComputerName'? (run e2e.ps1 once as Administrator)" -ForegroundColor White
        Write-Host "" -ForegroundColor Red
        Write-Host "Details: $($_.Exception.Message)" -ForegroundColor DarkYellow
    } else {
        Write-Error $_
    }
    exit 1
}
