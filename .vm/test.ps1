<#
.SYNOPSIS
    Run all CableAudio VM integration tests via Pester.

.DESCRIPTION
    Each test file under .vm/test/*.Tests.ps1 is a Pester suite.
    For every Describe block the VM is reverted to a clean snapshot and the
    driver is freshly installed before the Its run.

    Prerequisites:
    - VMware Workstation + vmrun
    - Host WinRM client configured (AllowUnencrypted, TrustedHosts)
    - Driver build artifacts present under driver/x64/Debug/package/
    - .env contains VM_PASSWORD=... (or pass -VmPassword)

.PARAMETER VmxPath
    Path to the .vmx file. Relative paths are resolved from the project root.

.PARAMETER SnapshotName
    Snapshot to revert to before each test suite.

.PARAMETER ComputerName
    Guest IP address used for WinRM.

.PARAMETER Port
    WinRM port (default 5985).

.PARAMETER Username
    Guest user account.

.PARAMETER Password
    Guest account password.

.PARAMETER StartMode
    "nogui" (default) or "gui".

.PARAMETER BootTimeoutSec
    Seconds to wait for WinRM after VM start.

.PARAMETER RenameLoopCount
    Number of rename iterations in the IOCTL rename loop test.

.PARAMETER VmPassword
    VMware Workstation encryption password for vmrun -vp.
    Falls back to VM_PASSWORD env var or .env file.

.PARAMETER TestFilter
    Pester -FullNameFilter pattern. Runs all tests by default.

.EXAMPLE
    .vm\test.ps1

.EXAMPLE
    .vm\test.ps1 -TestFilter "*IOCTL*"

.EXAMPLE
    .vm\test.ps1 -TestFilter "*PKEY*" -RenameLoopCount 1
#>
[CmdletBinding()]
param(
    [string]$VmxPath        = ".vm/cable-vm/cable-vm.vmx",
    [string]$SnapshotName   = "tiny11-winrm-enabled",
    [string]$ComputerName   = "192.168.23.128",
    [int]   $Port           = 5985,
    [string]$Username       = "cable",
    [string]$Password       = "cable123",
    [ValidateSet("gui", "nogui")]
    [string]$StartMode      = "nogui",
    [int]   $BootTimeoutSec = 240,
    [int]   $RenameLoopCount = 3,
    [string]$VmPassword,
    [string]$TestFilter
)

$ErrorActionPreference = "Stop"

$ScriptDir   = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRoot = Split-Path -Parent $ScriptDir
$TestDir     = Join-Path $ScriptDir "test"
$LogsDir     = Join-Path $ScriptDir "vm-logs"
if (-not (Test-Path $LogsDir)) {
    New-Item -ItemType Directory -Path $LogsDir -Force | Out-Null
}

# ------------------------------------------------------------------
# Resolve prerequisites before handing off to Pester so failures are
# surfaced clearly, outside of a test context.
# ------------------------------------------------------------------
. (Join-Path $TestDir "common.ps1")

Assert-HostWinRMConfig -TargetIp $ComputerName

$resolvedVmx = if ([System.IO.Path]::IsPathRooted($VmxPath)) {
    $VmxPath
} else {
    Join-Path $ProjectRoot $VmxPath
}
if (-not (Test-Path $resolvedVmx)) {
    throw "VMX not found: $resolvedVmx"
}

$vmPass    = Get-VMPassword -ProjectRoot $ProjectRoot -ExplicitPassword $VmPassword
$vmrunPath = Get-VmrunPath

# ------------------------------------------------------------------
# VmContext is a hashtable splatted into Reset-Vm inside each
# Describe/BeforeAll. Published as a global so Pester's child scopes
# can read it without ceremony.
# ------------------------------------------------------------------
$global:VmContext = @{
    VmxPath         = $resolvedVmx
    SnapshotName    = $SnapshotName
    VmrunPath       = $vmrunPath
    VmPass          = $vmPass
    ComputerName    = $ComputerName
    Port            = $Port
    Username        = $Username
    Password        = $Password
    ProjectRoot     = $ProjectRoot
    StartMode       = $StartMode
    BootTimeoutSec  = $BootTimeoutSec
    RenameLoopCount = $RenameLoopCount
}

# ------------------------------------------------------------------
# Pester configuration
# ------------------------------------------------------------------
Import-Module Pester -MinimumVersion 5.0 -ErrorAction Stop

$config = New-PesterConfiguration
$config.Run.Path           = $TestDir
$config.Run.PassThru       = $true
$config.Output.Verbosity   = "Detailed"
$config.TestResult.Enabled = $true
$config.TestResult.OutputPath = Join-Path $LogsDir ("test-results-" + (Get-Date -Format "yyyyMMdd-HHmmss") + ".xml")

if ($TestFilter) {
    $config.Filter.FullName = $TestFilter
}

Write-Host "`n==> Starting VM integration tests" -ForegroundColor Cyan
Write-Host "    VM:       $resolvedVmx" -ForegroundColor DarkGray
Write-Host "    Snapshot: $SnapshotName" -ForegroundColor DarkGray
Write-Host "    Guest:    $ComputerName`:$Port" -ForegroundColor DarkGray
Write-Host ""

$result = Invoke-Pester -Configuration $config

Write-Host ""
if ($result.FailedCount -gt 0) {
    Write-Host "FAILED: $($result.FailedCount) test(s) failed." -ForegroundColor Red
    exit 1
} else {
    Write-Host "PASSED: all $($result.PassedCount) test(s) passed." -ForegroundColor Green
}
