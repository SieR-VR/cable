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

    Write-Host "  [VM] Waiting for Cable audio endpoints..." -ForegroundColor DarkCyan
    $deadline = (Get-Date).AddSeconds(60)
    $endpointsReady = $false
    while ((Get-Date) -lt $deadline) {
        $found = Invoke-Guest -Session $session -Command @'
            Get-PnpDevice -Class AudioEndpoint -ErrorAction SilentlyContinue |
                Where-Object { $_.FriendlyName -like "*Cable Virtual Audio Device*" } |
                Select-Object -First 1
'@
        if ($found) {
            $endpointsReady = $true
            Start-Sleep -Seconds 2  # brief extra settle time
            break
        }
        Start-Sleep -Seconds 2
    }
    if (-not $endpointsReady) {
        Write-Warning "Cable audio endpoints did not appear within 60 seconds"
    }

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
