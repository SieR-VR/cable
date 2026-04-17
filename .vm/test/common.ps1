[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"

function Get-VMPassword {
    param(
        [string]$ProjectRoot,
        [string]$ExplicitPassword
    )

    if ($ExplicitPassword) { return $ExplicitPassword }
    if ($env:VM_PASSWORD) { return $env:VM_PASSWORD }

    $envFile = Join-Path $ProjectRoot ".env"
    if (-not (Test-Path $envFile)) {
        throw "VM password not found. Set VM_PASSWORD in env/.env or pass -VmPassword."
    }

    $line = Get-Content $envFile | Where-Object { $_ -match '^\s*VM_PASSWORD\s*=' } | Select-Object -First 1
    if (-not $line) { throw "VM_PASSWORD entry not found in .env" }

    $value = ($line -split '=', 2)[1].Trim()
    if (($value.StartsWith('"') -and $value.EndsWith('"')) -or ($value.StartsWith("'") -and $value.EndsWith("'"))) {
        $value = $value.Substring(1, $value.Length - 2)
    }
    if ([string]::IsNullOrWhiteSpace($value)) { throw "VM_PASSWORD in .env is empty" }

    return $value
}

function Get-VmrunPath {
    $vmrun = Get-Command vmrun -ErrorAction SilentlyContinue
    if ($vmrun) { return $vmrun.Source }

    $defaultPath = "C:\Program Files (x86)\VMware\VMware Workstation\vmrun.exe"
    if (Test-Path $defaultPath) { return $defaultPath }

    throw "vmrun.exe not found"
}

function Assert-HostWinRMConfig {
    param([string]$TargetIp)

    $issues = @()

    try {
        $allowUnencrypted = (Get-Item WSMan:\localhost\Client\AllowUnencrypted).Value
        if (-not [System.Convert]::ToBoolean($allowUnencrypted)) {
            $issues += "WSMan Client AllowUnencrypted is not true"
        }
    } catch {
        $issues += "Cannot read WSMan Client AllowUnencrypted"
    }

    $current = ""
    try {
        $current = (Get-Item WSMan:\localhost\Client\TrustedHosts).Value
    } catch {
        $issues += "Cannot read WSMan Client TrustedHosts"
    }

    $entries = @()
    if ($current) {
        $entries = $current.Split(',') | ForEach-Object { $_.Trim() } | Where-Object { $_ }
    }

    if (-not ($entries -contains "*" -or $entries -contains $TargetIp)) {
        $issues += "TrustedHosts does not include $TargetIp"
    }

    try {
        $svc = Get-Service WinRM -ErrorAction Stop
        if ($svc.Status -ne "Running") {
            $issues += "WinRM service is not running on host"
        }
    } catch {
        $issues += "WinRM service is not available on host"
    }

    if ($issues.Count -gt 0) {
        $fix = @(
            "Host WinRM client is not ready:",
            ($issues | ForEach-Object { "  - $_" }),
            "",
            "Run these commands in Administrator PowerShell:",
            "  Start-Service WinRM",
            "  Set-Service WinRM -StartupType Manual",
            "  Set-Item WSMan:\localhost\Client\AllowUnencrypted `$true -Force",
            "  Set-Item WSMan:\localhost\Client\TrustedHosts -Value '$TargetIp' -Force"
        ) -join [Environment]::NewLine
        throw $fix
    }
}

function Invoke-Vmrun {
    param(
        [string]$VmrunPath,
        [string]$VmPass,
        [string[]]$VmrunArgs
    )

    & $VmrunPath -T ws -vp $VmPass @VmrunArgs
    if ($LASTEXITCODE -ne 0) {
        throw "vmrun failed: $($VmrunArgs -join ' ')"
    }
}

function Wait-WinRM {
    param(
        [string]$ComputerName,
        [int]$Port,
        [pscredential]$Credential,
        [int]$TimeoutSec
    )

    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    $so = New-PSSessionOption -SkipCACheck -SkipCNCheck -SkipRevocationCheck

    while ((Get-Date) -lt $deadline) {
        try {
            $s = New-PSSession -ComputerName $ComputerName -Port $Port -Credential $Credential -Authentication Basic -SessionOption $so
            if ($s) {
                Remove-PSSession $s
                return
            }
        } catch {
            Start-Sleep -Seconds 3
        }
    }

    throw "WinRM did not become available on $ComputerName`:$Port"
}

function New-GuestSession {
    param(
        [string]$ComputerName,
        [int]$Port,
        [string]$Username,
        [string]$Password
    )

    $secure = ConvertTo-SecureString $Password -AsPlainText -Force
    $cred = New-Object System.Management.Automation.PSCredential($Username, $secure)
    $so = New-PSSessionOption -SkipCACheck -SkipCNCheck -SkipRevocationCheck
    return New-PSSession -ComputerName $ComputerName -Port $Port -Credential $cred -Authentication Basic -SessionOption $so
}

function Invoke-Guest {
    param(
        [System.Management.Automation.Runspaces.PSSession]$Session,
        [string]$Command
    )

    Invoke-Command -Session $Session -ScriptBlock {
        param($cmd)
        Invoke-Expression $cmd
    } -ArgumentList $Command
}

function Invoke-GuestRetry {
    param(
        [string]$ComputerName,
        [int]$Port,
        [string]$Username,
        [string]$Password,
        [string]$Command,
        [int]$MaxAttempts = 5
    )

    $lastErr = $null
    for ($attempt = 1; $attempt -le $MaxAttempts; $attempt++) {
        $s = $null
        try {
            $s = New-GuestSession -ComputerName $ComputerName -Port $Port -Username $Username -Password $Password
            return Invoke-Guest -Session $s -Command $Command
        }
        catch {
            $lastErr = $_
            Start-Sleep -Seconds ([Math]::Min(2 * $attempt, 8))
        }
        finally {
            if ($s) {
                Remove-PSSession $s -ErrorAction SilentlyContinue
            }
        }
    }

    throw $lastErr
}

function Find-DevconPath {
    $default = "C:\Program Files (x86)\Windows Kits\10\Tools\10.0.26100.0\x64\devcon.exe"
    if (Test-Path $default) { return $default }

    $found = Get-ChildItem "C:\Program Files (x86)\Windows Kits\10\Tools" -Recurse -Filter devcon.exe -ErrorAction SilentlyContinue |
        Where-Object { $_.FullName -match "x64" } |
        Select-Object -First 1
    if ($found) { return $found.FullName }

    throw "devcon.exe not found"
}

function Install-DriverInGuest {
    param(
        [string]$ComputerName,
        [int]$Port,
        [string]$Username,
        [string]$Password,
        [string]$ProjectRoot,
        [System.Management.Automation.Runspaces.PSSession]$Session
    )

    $driverPkg = Join-Path $ProjectRoot "driver\x64\Debug\package"
    $driverSys = Join-Path $driverPkg "CableAudio.sys"
    $driverInf = Join-Path $driverPkg "CableAudio.inf"
    $driverCat = Join-Path $driverPkg "cableaudio.cat"

    foreach ($path in @($driverSys, $driverInf, $driverCat)) {
        if (-not (Test-Path $path)) { throw "Missing driver file: $path" }
    }

    $certCandidates = @(
        (Join-Path $ProjectRoot "driver\x64\Debug\package.cer"),
        (Join-Path $ProjectRoot "driver\Source\Main\x64\Debug\CableAudio.cer")
    )
    $certPath = $certCandidates | Where-Object { Test-Path $_ } | Select-Object -First 1
    $devconPath = Find-DevconPath

    $prepCmd = "if (Test-Path C:\CableAudio) { Remove-Item C:\CableAudio -Recurse -Force }; New-Item -ItemType Directory -Path C:\CableAudio -Force | Out-Null"
    try {
        Invoke-Guest -Session $Session -Command $prepCmd
    }
    catch {
        Invoke-GuestRetry -ComputerName $ComputerName -Port $Port -Username $Username -Password $Password -Command $prepCmd | Out-Null
    }

    Copy-Item $driverSys -Destination "C:\CableAudio\CableAudio.sys" -ToSession $Session -Force
    Copy-Item $driverInf -Destination "C:\CableAudio\CableAudio.inf" -ToSession $Session -Force
    Copy-Item $driverCat -Destination "C:\CableAudio\cableaudio.cat" -ToSession $Session -Force
    Copy-Item $devconPath -Destination "C:\CableAudio\devcon.exe" -ToSession $Session -Force
    if ($certPath) {
        Copy-Item $certPath -Destination "C:\CableAudio\WDKTestCert.cer" -ToSession $Session -Force

        $certImport = @"
`$cert = New-Object System.Security.Cryptography.X509Certificates.X509Certificate2('C:\CableAudio\WDKTestCert.cer')
`$rootStore = New-Object System.Security.Cryptography.X509Certificates.X509Store('Root','LocalMachine')
`$rootStore.Open('ReadWrite')
`$rootStore.Add(`$cert)
`$rootStore.Close()
`$pubStore = New-Object System.Security.Cryptography.X509Certificates.X509Store('TrustedPublisher','LocalMachine')
`$pubStore.Open('ReadWrite')
`$pubStore.Add(`$cert)
`$pubStore.Close()
"@
        Invoke-Guest -Session $Session -Command $certImport | Out-Null
    }

    $exists = $null
    try {
        $exists = (Invoke-Guest -Session $Session -Command "& C:\CableAudio\devcon.exe find ROOT\CableAudio 2>&1 | Out-String")
    } catch {
        $exists = (Invoke-GuestRetry -ComputerName $ComputerName -Port $Port -Username $Username -Password $Password -Command "& C:\CableAudio\devcon.exe find ROOT\CableAudio 2>&1 | Out-String")
    }
    if ($exists -match 'ROOT\\CableAudio') {
        try {
            Invoke-Guest -Session $Session -Command "& C:\CableAudio\devcon.exe update C:\CableAudio\CableAudio.inf ROOT\CableAudio 2>&1 | Out-String" | Out-Host
        } catch {
            Invoke-GuestRetry -ComputerName $ComputerName -Port $Port -Username $Username -Password $Password -Command "& C:\CableAudio\devcon.exe update C:\CableAudio\CableAudio.inf ROOT\CableAudio 2>&1 | Out-String" | Out-Host
        }
    }
    else {
        try {
            Invoke-Guest -Session $Session -Command "& C:\CableAudio\devcon.exe install C:\CableAudio\CableAudio.inf ROOT\CableAudio 2>&1 | Out-String" | Out-Host
        } catch {
            Invoke-GuestRetry -ComputerName $ComputerName -Port $Port -Username $Username -Password $Password -Command "& C:\CableAudio\devcon.exe install C:\CableAudio\CableAudio.inf ROOT\CableAudio 2>&1 | Out-String" | Out-Host
        }
    }
}

# Revert VM to snapshot, start it, wait for WinRM, install driver.
# Returns an open PSSession ready for testing.
function Reset-Vm {
    param(
        [string]$VmxPath,
        [string]$SnapshotName,
        [string]$VmrunPath,
        [string]$VmPass,
        [string]$ComputerName,
        [int]$Port,
        [string]$Username,
        [string]$Password,
        [string]$ProjectRoot,
        [string]$StartMode = "nogui",
        [int]$BootTimeoutSec = 240
    )

    Write-Host "  [VM] Reverting to snapshot '$SnapshotName'..." -ForegroundColor DarkCyan
    Invoke-Vmrun -VmrunPath $VmrunPath -VmPass $VmPass -VmrunArgs @("revertToSnapshot", $VmxPath, $SnapshotName)
    Invoke-Vmrun -VmrunPath $VmrunPath -VmPass $VmPass -VmrunArgs @("start", $VmxPath, $StartMode)

    Write-Host "  [VM] Waiting for WinRM..." -ForegroundColor DarkCyan
    $secure = ConvertTo-SecureString $Password -AsPlainText -Force
    $cred = New-Object System.Management.Automation.PSCredential($Username, $secure)
    Wait-WinRM -ComputerName $ComputerName -Port $Port -Credential $cred -TimeoutSec $BootTimeoutSec

    $session = New-GuestSession -ComputerName $ComputerName -Port $Port -Username $Username -Password $Password

    $bcd = (Invoke-Guest -Session $session -Command "cmd /c bcdedit /enum" | Out-String)
    if ($bcd -notmatch 'testsigning\s+Yes') {
        Remove-PSSession $session -ErrorAction SilentlyContinue
        throw "testsigning is not enabled in VM snapshot '$SnapshotName'."
    }

    Write-Host "  [VM] Installing driver..." -ForegroundColor DarkCyan
    Install-DriverInGuest -ComputerName $ComputerName -Port $Port -Username $Username -Password $Password `
        -ProjectRoot $ProjectRoot -Session $session

    # Restart audio services and poll until the Cable endpoints appear.
    Write-Host "  [VM] Restarting audio services..." -ForegroundColor DarkCyan
    Invoke-Guest -Session $session -Command @'
        pnputil /scan-devices | Out-Null
        Start-Sleep -Seconds 2
        Restart-Service -Name AudioEndpointBuilder -Force -ErrorAction SilentlyContinue
        Restart-Service -Name Audiosrv -Force -ErrorAction SilentlyContinue
'@

    # Cable audio endpoints are created dynamically via IOCTL, not at driver
    # startup.  Individual tests that need WASAPI endpoints create their own
    # virtual devices.  No polling is needed here.
    Write-Host "  [VM] Ready." -ForegroundColor DarkCyan
    return $session
}

function Install-AppInGuest {
    param(
        [System.Management.Automation.Runspaces.PSSession]$Session,
        [string]$MsiPath
    )

    Copy-Item $MsiPath -Destination "C:\CableAudio\cable-ui.msi" -ToSession $Session -Force
    Invoke-Guest -Session $Session -Command "Start-Process msiexec.exe -ArgumentList '/i ""C:\CableAudio\cable-ui.msi"" /qn /norestart' -Wait -NoNewWindow"
}

function Get-GuestBugCheckEvidence {
    param(
        [string]$ComputerName,
        [int]$Port,
        [string]$Username,
        [string]$Password
    )

    $script = @'
$ErrorActionPreference = "Stop"

$boot = (Get-CimInstance Win32_OperatingSystem).LastBootUpTime

$bugcheckEvents = Get-WinEvent -FilterHashtable @{ LogName = "System"; Id = 1001; StartTime = $boot } -ErrorAction SilentlyContinue |
    Where-Object {
        $_.ProviderName -eq "BugCheck" -or
        $_.ProviderName -eq "Microsoft-Windows-WER-SystemErrorReporting" -or
        $_.Message -match "bugcheck"
    }

$kernelPower41 = Get-WinEvent -FilterHashtable @{ LogName = "System"; Id = 41; StartTime = $boot } -ErrorAction SilentlyContinue

$dumpItems = @()
$memoryDmp = "C:\Windows\MEMORY.DMP"
if (Test-Path $memoryDmp) {
    $item = Get-Item $memoryDmp -ErrorAction SilentlyContinue
    if ($item -and $item.LastWriteTime -ge $boot) { $dumpItems += $item }
}

$miniDir = "C:\Windows\Minidump"
if (Test-Path $miniDir) {
    $dumpItems += Get-ChildItem $miniDir -Filter "*.dmp" -ErrorAction SilentlyContinue |
        Where-Object { $_.LastWriteTime -ge $boot }
}

$recentEvents = @($bugcheckEvents | Select-Object -First 3 | ForEach-Object {
    $msg = [string]$_.Message
    if ([string]::IsNullOrWhiteSpace($msg)) { $msg = "(no message)" }
    $msg = ($msg -replace "`r?`n", " ")
    if ($msg.Length -gt 200) { $msg = $msg.Substring(0, 200) }
    "[{0}] {1}: {2}" -f $_.TimeCreated, $_.ProviderName, $msg
})

$payload = [PSCustomObject]@{
    BootTime       = $boot
    BugCheckCount  = @($bugcheckEvents).Count
    Kernel41Count  = @($kernelPower41).Count
    DumpCount      = @($dumpItems).Count
    BugCheckRecent = $recentEvents
}

$payload | ConvertTo-Json -Depth 4 -Compress
'@

    $json = Invoke-GuestRetry -ComputerName $ComputerName -Port $Port -Username $Username -Password $Password -Command $script
    return ($json | ConvertFrom-Json)
}

function Assert-NoGuestBugCheck {
    param(
        [string]$ComputerName,
        [int]$Port,
        [string]$Username,
        [string]$Password,
        [string]$Context = "VM test"
    )

    $evidence = Get-GuestBugCheckEvidence -ComputerName $ComputerName -Port $Port -Username $Username -Password $Password

    if ($evidence.BugCheckCount -gt 0 -or $evidence.DumpCount -gt 0) {
        $recent = if ($evidence.BugCheckRecent) { ($evidence.BugCheckRecent -join " | ") } else { "(no event text)" }
        throw "$Context failed: Guest bugcheck evidence detected. Boot=$($evidence.BootTime), BugCheckCount=$($evidence.BugCheckCount), DumpCount=$($evidence.DumpCount), Kernel41Count=$($evidence.Kernel41Count), Recent=$recent"
    }
}

# ---------------------------------------------------------------------------
# Shared C# interop library loader and guest test runner.
# ---------------------------------------------------------------------------

function Get-CSharpLib {
    param([string]$Name)
    $path = Join-Path $PSScriptRoot "lib\$Name.cs"
    if (-not (Test-Path $path)) { throw "C# library not found: $path" }
    return (Get-Content $path -Raw)
}

function Invoke-GuestCSharpTest {
    param(
        [Parameter(Mandatory)]
        [System.Management.Automation.Runspaces.PSSession]$Session,
        [Parameter(Mandatory)]
        [string[]]$CSharpSources,
        [string]$HelperFunctions = "",
        [Parameter(Mandatory)]
        [object]$Script,
        [string]$TempFileName = "cable-test"
    )

    # Concatenate all C# sources, then hoist 'using' directives to the top.
    # Without this, concatenating CableIoctl.cs (types) + CableWasapi.cs (using + types)
    # places using directives after type definitions, which is a C# compilation error.
    $rawCs = $CSharpSources -join "`n`n"
    $lines = $rawCs -split "`n"
    $usings = [System.Collections.Generic.List[string]]::new()
    $body   = [System.Collections.Generic.List[string]]::new()
    foreach ($line in $lines) {
        if ($line.TrimStart() -match '^using\s+') {
            $trimmed = $line.Trim()
            if (-not $usings.Contains($trimmed)) {
                $usings.Add($trimmed)
            }
        } else {
            $body.Add($line)
        }
    }
    $combinedCs = ($usings -join "`n") + "`n`n" + ($body -join "`n")
    $callerBody = if ($Script -is [scriptblock]) { $Script.ToString() } else { [string]$Script }

    # Build the guest script using string concatenation to avoid here-string nesting.
    $fullScript = '$ErrorActionPreference = "Stop"' + "`n" +
                  'Add-Type -TypeDefinition @''' + "`n" +
                  $combinedCs + "`n" +
                  "'" + '@' + "`n`n"

    if ($HelperFunctions) {
        $fullScript += $HelperFunctions + "`n`n"
    }

    $fullScript += '& { ' + $callerBody + ' }'

    return Invoke-Command -Session $Session -ScriptBlock {
        param($body, $tmpName)
        $tmp = "C:\CableAudio\$tmpName.ps1"
        Set-Content -Path $tmp -Value $body -Encoding UTF8
        & powershell -NoProfile -ExecutionPolicy Bypass -File $tmp
    } -ArgumentList $fullScript, $TempFileName
}

function Resolve-GuestAppExePath {
    param([System.Management.Automation.Runspaces.PSSession]$Session)

    $cmd = @'
$candidates = @(
    "C:\Users\cable\AppData\Local\cable-ui\cable-tauri.exe",
    "C:\Program Files\cable-ui\cable-tauri.exe",
    "C:\Program Files\cable\cable-tauri.exe"
)

$existing = $candidates | Where-Object { Test-Path $_ } | Select-Object -First 1
if ($existing) { $existing; return }

$fallback = Get-ChildItem "C:\Users" -Directory -ErrorAction SilentlyContinue |
    ForEach-Object { Join-Path $_.FullName "AppData\Local\cable-ui\cable-tauri.exe" } |
    Where-Object { Test-Path $_ } |
    Select-Object -First 1

if ($fallback) { $fallback }
'@

    $resolved = Invoke-Guest -Session $Session -Command $cmd | Select-Object -First 1
    if (-not $resolved) {
        throw "cable-tauri.exe not found after install."
    }
    return [string]$resolved
}
