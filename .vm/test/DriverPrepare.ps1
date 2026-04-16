[CmdletBinding()]
param(
    [string]$VmxPath = ".vm/cable-vm/cable-vm.vmx",
    [string]$SnapshotName = "tiny11-winrm-enabled",
    [string]$ComputerName = "192.168.23.128",
    [int]$Port = 5985,
    [string]$Username = "cable",
    [string]$Password = "cable123",
    [ValidateSet("gui", "nogui")]
    [string]$StartMode = "nogui",
    [int]$BootTimeoutSec = 240,
    [string]$VmPassword,
    [switch]$SkipRevert
)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRoot = Split-Path -Parent (Split-Path -Parent $ScriptDir)
. (Join-Path $ScriptDir "common.ps1")

$resolvedVmx = if ([System.IO.Path]::IsPathRooted($VmxPath)) { $VmxPath } else { Join-Path $ProjectRoot $VmxPath }
if (-not (Test-Path $resolvedVmx)) { throw "VMX not found: $resolvedVmx" }

$vmPass = Get-VMPassword -ProjectRoot $ProjectRoot -ExplicitPassword $VmPassword
$vmrunPath = Get-VmrunPath

Assert-HostWinRMConfig -TargetIp $ComputerName

if (-not $SkipRevert) {
    Invoke-Vmrun -VmrunPath $vmrunPath -VmPass $vmPass -VmrunArgs @("revertToSnapshot", $resolvedVmx, $SnapshotName)
}
Invoke-Vmrun -VmrunPath $vmrunPath -VmPass $vmPass -VmrunArgs @("start", $resolvedVmx, $StartMode)

$secure = ConvertTo-SecureString $Password -AsPlainText -Force
$cred = New-Object System.Management.Automation.PSCredential($Username, $secure)
Wait-WinRM -ComputerName $ComputerName -Port $Port -Credential $cred -TimeoutSec $BootTimeoutSec

$session = $null
try {
    $session = New-GuestSession -ComputerName $ComputerName -Port $Port -Username $Username -Password $Password
    $bcd = (Invoke-Guest -Session $session -Command "cmd /c bcdedit /enum" | Out-String)
    if ($bcd -notmatch 'testsigning\s+Yes') {
        throw "testsigning is not enabled"
    }

    Install-DriverInGuest -ComputerName $ComputerName -Port $Port -Username $Username -Password $Password -ProjectRoot $ProjectRoot -Session $session
    Write-Host "VM is prepared for driver rename tests." -ForegroundColor Green
}
catch {
    if ($session) {
        Remove-PSSession $session -ErrorAction SilentlyContinue
        $session = $null
    }

    Write-Host "Transient WinRM/session error detected, retrying install once..." -ForegroundColor Yellow
    $session = New-GuestSession -ComputerName $ComputerName -Port $Port -Username $Username -Password $Password
    $bcd = (Invoke-Guest -Session $session -Command "cmd /c bcdedit /enum" | Out-String)
    if ($bcd -notmatch 'testsigning\s+Yes') {
        throw "testsigning is not enabled"
    }
    Install-DriverInGuest -ComputerName $ComputerName -Port $Port -Username $Username -Password $Password -ProjectRoot $ProjectRoot -Session $session
    Write-Host "VM is prepared for driver rename tests." -ForegroundColor Green
}
finally {
    if ($session) {
        Remove-PSSession $session -ErrorAction SilentlyContinue
    }
}
