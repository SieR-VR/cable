<#
.SYNOPSIS
    Prepare a fresh Cable test VM for user testing.

.DESCRIPTION
    Performs all steps needed to go from a clean VM snapshot to a running Cable
    session:

      1. (Optional) Build the driver from source
      2. (Optional) Build the Tauri app from source  (pnpm tauri build --debug)
      3. Generate the driver catalog (.cat) with inf2cat + signtool
      4. Revert the VM to a known-good snapshot and start it
      5. Wait for WinRM to become available
      6. Install the driver (devcon install) into the guest
      7. Restart audio services and wait for Cable endpoints to appear
      8. Copy + install the Cable app MSI into the guest
      9. Launch cable-tauri.exe in the guest (visible on screen)
     10. Open VMware Workstation GUI so the user can interact with the VM

    After the script finishes the VM desktop is live with the Cable app running.

.PARAMETER VmxPath
    Path to the .vmx file (default: .vm/cable-vm/cable-vm.vmx).

.PARAMETER SnapshotName
    Snapshot to revert to (default: tiny11-winrm-enabled).

.PARAMETER ComputerName
    Guest IP address for WinRM (default: 192.168.23.128).

.PARAMETER Port
    WinRM port (default: 5985).

.PARAMETER Username
    Guest account name (default: cable).

.PARAMETER Password
    Guest account password (default: cable123).

.PARAMETER VmPassword
    VMware Workstation encryption password for vmrun -vp.
    Falls back to VM_PASSWORD env var or .env file.

.PARAMETER SkipDriverBuild
    Skip driver build and use the existing artifacts in driver/x64/Debug/package/.

.PARAMETER SkipAppBuild
    Skip Tauri app build and use the existing MSI in target/debug/bundle/msi/.

.PARAMETER SkipRevert
    Do not revert the VM to a snapshot; just start (or re-use) the current state.

.PARAMETER BootTimeoutSec
    Seconds to wait for WinRM after VM start (default: 240).

.PARAMETER StartMode
    "nogui" (headless, then opens GUI at the end) or "gui" (opens GUI immediately).

.EXAMPLE
    # Full setup from scratch — build everything, reset VM, launch app
    .\.vm\setup.ps1

.EXAMPLE
    # Skip builds, just reset VM and deploy current artifacts
    .\.vm\setup.ps1 -SkipDriverBuild -SkipAppBuild

.EXAMPLE
    # Re-use already-running VM (don't revert snapshot)
    .\.vm\setup.ps1 -SkipDriverBuild -SkipAppBuild -SkipRevert
#>
[CmdletBinding()]
param(
    [string]$VmxPath        = ".vm/cable-vm/cable-vm.vmx",
    [string]$SnapshotName   = "tiny11-winrm-enabled",
    [string]$ComputerName   = "192.168.23.128",
    [int]   $Port           = 5985,
    [string]$Username       = "cable",
    [string]$Password       = "cable123",
    [string]$VmPassword,
    [switch]$SkipDriverBuild,
    [switch]$SkipAppBuild,
    [switch]$SkipRevert,
    [int]   $BootTimeoutSec = 240,
    [ValidateSet("gui", "nogui")]
    [string]$StartMode = "nogui"
)

$ErrorActionPreference = "Stop"

$ScriptDir   = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRoot = Split-Path -Parent $ScriptDir
$TestDir     = Join-Path $ScriptDir "test"

. (Join-Path $TestDir "common.ps1")

# ──────────────────────────────────────────────────────────────────────────────
# Helper: section banner
# ──────────────────────────────────────────────────────────────────────────────
function Write-Step {
    param([string]$Text)
    Write-Host ""
    Write-Host "  [$Text]" -ForegroundColor Cyan
}

# ──────────────────────────────────────────────────────────────────────────────
# Resolve paths
# ──────────────────────────────────────────────────────────────────────────────
$resolvedVmx = if ([System.IO.Path]::IsPathRooted($VmxPath)) {
    $VmxPath
} else {
    Join-Path $ProjectRoot $VmxPath
}
if (-not (Test-Path $resolvedVmx)) {
    throw "VMX not found: $resolvedVmx"
}

$driverPkgDir  = Join-Path $ProjectRoot "driver\x64\Debug\package"
$msiGlob       = Join-Path $ProjectRoot "target\debug\bundle\msi\*.msi"

# ──────────────────────────────────────────────────────────────────────────────
# Step 1 — Build driver (optional)
# ──────────────────────────────────────────────────────────────────────────────
if (-not $SkipDriverBuild) {
    Write-Step "Building driver"
    $buildScript = Join-Path $ProjectRoot "driver\scripts\build.ps1"
    & $buildScript -Configuration Debug -Platform x64
    if ($LASTEXITCODE -ne 0) { throw "Driver build failed (exit $LASTEXITCODE)" }
} else {
    Write-Step "Skipping driver build"
}

# ──────────────────────────────────────────────────────────────────────────────
# Step 2 — Build Tauri app (optional)
# ──────────────────────────────────────────────────────────────────────────────
if (-not $SkipAppBuild) {
    Write-Step "Building Tauri app  (pnpm tauri build --debug)"
    Push-Location $ProjectRoot
    try {
        pnpm tauri build --debug
        if ($LASTEXITCODE -ne 0) { throw "Tauri build failed (exit $LASTEXITCODE)" }
    } finally {
        Pop-Location
    }
} else {
    Write-Step "Skipping app build"
}

# ──────────────────────────────────────────────────────────────────────────────
# Step 3 — Ensure driver catalog (.cat) exists
# ──────────────────────────────────────────────────────────────────────────────
Write-Step "Verifying driver package"

foreach ($f in @("CableAudio.sys", "CableAudio.inf")) {
    $p = Join-Path $driverPkgDir $f
    if (-not (Test-Path $p)) { throw "Missing driver artifact: $p" }
}

$catPath = Join-Path $driverPkgDir "cableaudio.cat"
$catStale = $false
if (Test-Path $catPath) {
    # Regenerate the catalog when .sys or .inf are newer (e.g. after a rebuild).
    $catTime = (Get-Item $catPath).LastWriteTime
    $sysTime = (Get-Item (Join-Path $driverPkgDir "CableAudio.sys")).LastWriteTime
    $infTime = (Get-Item (Join-Path $driverPkgDir "CableAudio.inf")).LastWriteTime
    if ($sysTime -gt $catTime -or $infTime -gt $catTime) {
        $catStale = $true
        Remove-Item $catPath -Force
        Write-Host "    cableaudio.cat is stale (older than .sys/.inf) — regenerating..." -ForegroundColor Yellow
    }
}
if (-not (Test-Path $catPath)) {
    if (-not $catStale) {
        Write-Host "    cableaudio.cat not found — generating with inf2cat..." -ForegroundColor Yellow
    }

    $wdkBin = Get-ChildItem "C:\Program Files (x86)\Windows Kits\10\bin" -Filter "Inf2Cat.exe" -Recurse -ErrorAction SilentlyContinue |
        Sort-Object { [version]($_.Directory.Parent.Name) } -Descending |
        Select-Object -First 1

    if (-not $wdkBin) { throw "Inf2Cat.exe not found — install Windows Driver Kit (WDK)" }

    & $wdkBin.FullName /os:10_x64 /driver:$driverPkgDir /uselocaltime
    if ($LASTEXITCODE -ne 0) { throw "inf2cat failed (exit $LASTEXITCODE)" }

    # Sign the catalog with the test certificate already used to sign the .sys
    $signtool = Get-ChildItem "C:\Program Files (x86)\Windows Kits\10\bin" -Filter "signtool.exe" -Recurse -ErrorAction SilentlyContinue |
        Where-Object { $_.FullName -match "x64" } |
        Sort-Object { [version]($_.Directory.Parent.Name) } -Descending |
        Select-Object -First 1

    if ($signtool) {
        # Use the same thumbprint stamped into the .sys during MSBuild
        $thumbprint = (Get-AuthenticodeSignature (Join-Path $driverPkgDir "CableAudio.sys")).SignerCertificate.Thumbprint
        if ($thumbprint) {
            & $signtool.FullName sign /ph /fd sha256 /sha1 $thumbprint $catPath
        }
    }

    if (-not (Test-Path $catPath)) { throw "cableaudio.cat was not created" }
    Write-Host "    cableaudio.cat created." -ForegroundColor Green
} else {
    Write-Host "    cableaudio.cat already present." -ForegroundColor DarkGray
}

# ──────────────────────────────────────────────────────────────────────────────
# Step 4 — Find MSI installer
# ──────────────────────────────────────────────────────────────────────────────
$msiPath = Get-ChildItem (Join-Path $ProjectRoot "target\debug\bundle\msi") -Filter "*.msi" -ErrorAction SilentlyContinue |
    Sort-Object LastWriteTime -Descending |
    Select-Object -First 1 -ExpandProperty FullName

if (-not $msiPath) {
    throw "No MSI found under target\debug\bundle\msi\.  Run without -SkipAppBuild or build manually with 'pnpm tauri build --debug'."
}
Write-Host "    MSI: $msiPath" -ForegroundColor DarkGray

# ──────────────────────────────────────────────────────────────────────────────
# Step 5 — WinRM prereq check
# ──────────────────────────────────────────────────────────────────────────────
Assert-HostWinRMConfig -TargetIp $ComputerName

# ──────────────────────────────────────────────────────────────────────────────
# Step 6 — Revert + start VM
# ──────────────────────────────────────────────────────────────────────────────
$vmPass    = Get-VMPassword -ProjectRoot $ProjectRoot -ExplicitPassword $VmPassword
$vmrunPath = Get-VmrunPath

if (-not $SkipRevert) {
    Write-Step "Reverting VM to snapshot '$SnapshotName'"
    Invoke-Vmrun -VmrunPath $vmrunPath -VmPass $vmPass -VmrunArgs @("revertToSnapshot", $resolvedVmx, $SnapshotName)
}

# Check if VM is already running; only call vmrun start if it isn't.
$runningVms = & $vmrunPath -T ws -vp $vmPass list 2>$null
$vmAlreadyRunning = ($runningVms | Where-Object { $_ -like "*cable-vm.vmx*" }) -ne $null

if (-not $vmAlreadyRunning) {
    Write-Step "Starting VM"
    Invoke-Vmrun -VmrunPath $vmrunPath -VmPass $vmPass -VmrunArgs @("start", $resolvedVmx, $StartMode)
} else {
    Write-Step "VM already running — skipping start"
}

# ──────────────────────────────────────────────────────────────────────────────
# Step 7 — Wait for WinRM
# ──────────────────────────────────────────────────────────────────────────────
Write-Step "Waiting for WinRM ($ComputerName`:$Port)"
$secure = ConvertTo-SecureString $Password -AsPlainText -Force
$cred   = New-Object System.Management.Automation.PSCredential($Username, $secure)
Wait-WinRM -ComputerName $ComputerName -Port $Port -Credential $cred -TimeoutSec $BootTimeoutSec

$session = New-GuestSession -ComputerName $ComputerName -Port $Port -Username $Username -Password $Password

# ──────────────────────────────────────────────────────────────────────────────
# Step 8 — Install driver
# ──────────────────────────────────────────────────────────────────────────────
Write-Step "Installing driver"

# Check whether the driver is already present in the guest.
# If it is (and we skipped the snapshot revert), skip the install to avoid the
# lengthy devcon update + audio service restart that drops the WinRM session.
$driverAlreadyPresent = $false
if ($SkipRevert) {
    try {
        $checkCmd = @'
if (Test-Path C:\CableAudio\devcon.exe) {
    & C:\CableAudio\devcon.exe find ROOT\CableAudio 2>&1 | Out-String
} else { '' }
'@
        $found = Invoke-GuestRetry -ComputerName $ComputerName -Port $Port -Username $Username -Password $Password -Command $checkCmd
        if ($found -match 'ROOT\\CableAudio') {
            $driverAlreadyPresent = $true
            Write-Host "    Driver already installed — skipping devcon (SkipRevert mode)." -ForegroundColor DarkGray
        }
    } catch {
        # Could not query — fall through to normal install path
    }
}

if (-not $driverAlreadyPresent) {
    Install-DriverInGuest -ComputerName $ComputerName -Port $Port -Username $Username -Password $Password `
        -ProjectRoot $ProjectRoot -Session $session

    # devcon install may restart audio services and drop the current session.
    # Open a fresh session with retry for subsequent commands.
    Remove-PSSession $session -ErrorAction SilentlyContinue
    $session = $null

    Write-Host "    Restarting audio services (new session)..." -ForegroundColor DarkGray
    Invoke-GuestRetry -ComputerName $ComputerName -Port $Port -Username $Username -Password $Password -Command @'
        pnputil /scan-devices | Out-Null
        Start-Sleep -Seconds 2
        Restart-Service -Name AudioEndpointBuilder -Force -ErrorAction SilentlyContinue
        Restart-Service -Name Audiosrv             -Force -ErrorAction SilentlyContinue
'@ | Out-Null

    $session = New-GuestSession -ComputerName $ComputerName -Port $Port -Username $Username -Password $Password
}

# ──────────────────────────────────────────────────────────────────────────────
# Step 9 — Install Cable app
# ──────────────────────────────────────────────────────────────────────────────
Write-Step "Installing Cable app"
Install-AppInGuest -Session $session -MsiPath $msiPath

# ──────────────────────────────────────────────────────────────────────────────
# Step 10 — Launch app
# ──────────────────────────────────────────────────────────────────────────────
Write-Step "Launching cable-tauri.exe"
$appExe = Resolve-GuestAppExePath -Session $session
Write-Host "    Exe: $appExe" -ForegroundColor DarkGray

Invoke-Guest -Session $session -Command "Start-Process '$appExe'"

Remove-PSSession $session -ErrorAction SilentlyContinue

# ──────────────────────────────────────────────────────────────────────────────
# Step 11 — Open VMware GUI so the user can see the VM desktop
# ──────────────────────────────────────────────────────────────────────────────
Write-Step "Opening VMware Workstation GUI"
$vmwareExe = "C:\Program Files (x86)\VMware\VMware Workstation\vmware.exe"
if (Test-Path $vmwareExe) {
    Start-Process $vmwareExe -ArgumentList "`"$resolvedVmx`""
} else {
    Write-Warning "vmware.exe not found at default path — open VMware manually."
}

# ──────────────────────────────────────────────────────────────────────────────
Write-Host ""
Write-Host "  Setup complete." -ForegroundColor Green
Write-Host "  The Cable app is running inside the VM at $ComputerName." -ForegroundColor Green
Write-Host ""
