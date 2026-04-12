#Requires -Version 5.1
<#
.SYNOPSIS
    Unified build script for the Cable project.

.DESCRIPTION
    Orchestrates building all components of Cable:
    - Driver (CableAudio.sys via MSBuild/WDK)
    - Frontend (React/Vite via pnpm)
    - Tauri app (Rust backend + bundling)

    Can build individual components or everything at once.

.PARAMETER Target
    What to build: All, Driver, Frontend, App (default: All)
    - Driver: builds the kernel driver (.sys + .inf + .cat)
    - Frontend: builds the React UI (pnpm build)
    - App: builds the Tauri desktop app (cargo + frontend)
    - All: builds Driver, then App (App includes Frontend)

.PARAMETER Configuration
    Driver build configuration: Debug or Release (default: Debug)

.PARAMETER Platform
    Driver target platform: x64 or ARM64 (default: x64)

.PARAMETER Clean
    Clean build outputs before building.

.PARAMETER Sign
    Test-sign the driver after building. Requires a test certificate
    (run scripts\create-test-cert.ps1 first).

.PARAMETER Release
    Build all components in release mode.

.EXAMPLE
    .\scripts\build.ps1                      # Build everything (Debug)
    .\scripts\build.ps1 -Target Driver       # Build driver only
    .\scripts\build.ps1 -Target App          # Build Tauri app only
    .\scripts\build.ps1 -Release             # Build everything (Release)
    .\scripts\build.ps1 -Target Driver -Sign # Build and test-sign driver
    .\scripts\build.ps1 -Clean               # Clean build
#>

[CmdletBinding()]
param(
    [ValidateSet("All", "Driver", "Frontend", "App")]
    [string]$Target = "All",

    [ValidateSet("Debug", "Release")]
    [string]$Configuration = "Debug",

    [ValidateSet("x64", "ARM64")]
    [string]$Platform = "x64",

    [switch]$Clean,
    [switch]$Sign,
    [switch]$Release
)

$ErrorActionPreference = "Stop"
$ProjectRoot = Split-Path -Parent $PSScriptRoot
$StartTime = Get-Date

if ($Release) {
    $Configuration = "Release"
}

Write-Host "========================================" -ForegroundColor Cyan
Write-Host " Cable Build System" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "  Target        : $Target" -ForegroundColor White
Write-Host "  Configuration : $Configuration" -ForegroundColor White
Write-Host "  Platform      : $Platform" -ForegroundColor White
Write-Host "  Clean         : $Clean" -ForegroundColor White
Write-Host "  Sign          : $Sign" -ForegroundColor White
Write-Host ""

# ============================================================
# Tool Discovery
# ============================================================

function Find-MSBuild {
    $msbuild = Get-Command msbuild -ErrorAction SilentlyContinue
    if ($msbuild) { return $msbuild.Source }

    $vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
    if (Test-Path $vswhere) {
        $vsPath = & $vswhere -latest -requires Microsoft.Component.MSBuild -property installationPath 2>$null
        if ($vsPath) {
            $msbuildPath = Join-Path $vsPath "MSBuild\Current\Bin\MSBuild.exe"
            if (Test-Path $msbuildPath) { return $msbuildPath }
        }
    }

    $searchPaths = @(
        "$env:ProgramFiles\Microsoft Visual Studio\2022\Community\MSBuild\Current\Bin\MSBuild.exe",
        "$env:ProgramFiles\Microsoft Visual Studio\2022\Professional\MSBuild\Current\Bin\MSBuild.exe",
        "$env:ProgramFiles\Microsoft Visual Studio\2022\Enterprise\MSBuild\Current\Bin\MSBuild.exe",
        "$env:ProgramFiles\Microsoft Visual Studio\2022\BuildTools\MSBuild\Current\Bin\MSBuild.exe",
        "${env:ProgramFiles(x86)}\Microsoft Visual Studio\2022\BuildTools\MSBuild\Current\Bin\MSBuild.exe",
        "${env:ProgramFiles(x86)}\Microsoft Visual Studio\2022\Community\MSBuild\Current\Bin\MSBuild.exe"
    )
    foreach ($path in $searchPaths) {
        if (Test-Path $path) { return $path }
    }
    return $null
}

function Find-SignTool {
    $wdkRoot = Get-ItemProperty -Path "HKLM:\SOFTWARE\Microsoft\Windows Kits\Installed Roots" -Name "KitsRoot10" -ErrorAction SilentlyContinue
    if (-not $wdkRoot) { return $null }
    $root = $wdkRoot.KitsRoot10
    $versions = Get-ChildItem (Join-Path $root "bin") -Directory -ErrorAction SilentlyContinue |
        Where-Object { $_.Name -match '^\d+\.\d+\.\d+\.\d+$' } |
        Sort-Object { [version]$_.Name } -Descending
    if ($versions) {
        $signTool = Join-Path $root "bin\$($versions[0].Name)\x64\signtool.exe"
        if (Test-Path $signTool) { return $signTool }
    }
    return $null
}

# ============================================================
# Build Functions
# ============================================================

function Build-Driver {
    Write-Host ""
    Write-Host "--- Building Driver ---" -ForegroundColor Cyan
    Write-Host ""

    $msbuild = Find-MSBuild
    if (-not $msbuild) {
        Write-Host "ERROR: MSBuild not found." -ForegroundColor Red
        Write-Host "  Install VS 2022 Build Tools with 'Desktop development with C++' workload." -ForegroundColor Yellow
        return $false
    }
    Write-Host "  MSBuild: $msbuild" -ForegroundColor DarkGray

    $solutionFile = Join-Path $ProjectRoot "driver\CableAudio.sln"
    if (-not (Test-Path $solutionFile)) {
        Write-Host "ERROR: Solution file not found: $solutionFile" -ForegroundColor Red
        return $false
    }

    # Clean
    if ($Clean) {
        Write-Host "  Cleaning driver..." -ForegroundColor Yellow
        & $msbuild $solutionFile /t:Clean /p:Configuration=$Configuration /p:Platform=$Platform /verbosity:quiet 2>$null
    }

    # Build
    Write-Host "  Compiling ($Configuration|$Platform)..." -ForegroundColor Yellow

    $msbuildArgs = @(
        $solutionFile,
        "/p:Configuration=$Configuration",
        "/p:Platform=$Platform",
        "/p:Inf2CatUseLocalTime=true",
        "/verbosity:minimal",
        "/m"
    )

    if ($Platform -eq "ARM64") {
        $msbuildArgs += @(
            "/p:RunCodeAnalysis=false",
            "/p:ValidateDrivers=false",
            "/p:StampInf=false",
            "/p:ApiValidator_Enable=false",
            "/p:InfVerif_Enable=false",
            "/p:DisableVerification=true",
            "/p:SignMode=Off",
            "/p:EnableInf2cat=false"
        )
    }

    & $msbuild @msbuildArgs
    $buildResult = $LASTEXITCODE

    if ($buildResult -ne 0) {
        Write-Host "  DRIVER BUILD FAILED - MSBuild exited with code $buildResult" -ForegroundColor Red
        return $false
    }

    $outputDir = Join-Path $ProjectRoot "driver\$Platform\$Configuration\package"
    $sysFile = Join-Path $outputDir "CableAudio.sys"

    if (-not (Test-Path $sysFile)) {
        Write-Host "  DRIVER BUILD FAILED - CableAudio.sys not produced" -ForegroundColor Red
        return $false
    }

    $sysSize = (Get-Item $sysFile).Length
    Write-Host "  CableAudio.sys: $sysSize bytes" -ForegroundColor Green

    # Test signing
    if ($Sign) {
        Write-Host ""
        Write-Host "  Test-signing driver..." -ForegroundColor Yellow

        $signTool = Find-SignTool
        if (-not $signTool) {
            Write-Host "  WARNING: signtool.exe not found, skipping signing." -ForegroundColor Yellow
            return $true
        }

        $pfxFile = Join-Path $ProjectRoot "target\debug\CableAudioTestCert.pfx"
        $pfxPassword = "cable-test"

        if (-not (Test-Path $pfxFile)) {
            Write-Host "  WARNING: Test certificate not found at $pfxFile" -ForegroundColor Yellow
            Write-Host "  Run: .\scripts\create-test-cert.ps1" -ForegroundColor Yellow
            return $true
        }

        & $signTool sign /fd SHA256 /f $pfxFile /p $pfxPassword /t http://timestamp.digicert.com $sysFile 2>&1
        if ($LASTEXITCODE -eq 0) {
            Write-Host "  Driver signed successfully." -ForegroundColor Green
        } else {
            Write-Host "  WARNING: Signing failed (exit code $LASTEXITCODE)." -ForegroundColor Yellow
            Write-Host "  The driver is still built but unsigned." -ForegroundColor Yellow
        }
    }

    Write-Host "  Output: $outputDir" -ForegroundColor Green
    return $true
}

function Build-Frontend {
    Write-Host ""
    Write-Host "--- Building Frontend ---" -ForegroundColor Cyan
    Write-Host ""

    $pnpm = Get-Command pnpm -ErrorAction SilentlyContinue
    if (-not $pnpm) {
        Write-Host "ERROR: pnpm not found. Install it with: npm install -g pnpm" -ForegroundColor Red
        return $false
    }

    if ($Clean) {
        $distDir = Join-Path $ProjectRoot "dist"
        if (Test-Path $distDir) {
            Write-Host "  Cleaning dist/..." -ForegroundColor Yellow
            Remove-Item $distDir -Recurse -Force
        }
    }

    Write-Host "  Running pnpm install..." -ForegroundColor Yellow
    Push-Location $ProjectRoot
    try {
        & pnpm install --frozen-lockfile 2>&1 | Out-Null
        if ($LASTEXITCODE -ne 0) {
            & pnpm install 2>&1 | Out-Null
        }

        Write-Host "  Type checking..." -ForegroundColor Yellow
        & npx tsc --noEmit 2>&1
        if ($LASTEXITCODE -ne 0) {
            Write-Host "  WARNING: TypeScript errors found (continuing anyway)." -ForegroundColor Yellow
        }

        Write-Host "  Building with Vite..." -ForegroundColor Yellow
        & pnpm build
        if ($LASTEXITCODE -ne 0) {
            Write-Host "  FRONTEND BUILD FAILED" -ForegroundColor Red
            return $false
        }
    } finally {
        Pop-Location
    }

    $distDir = Join-Path $ProjectRoot "dist"
    if (Test-Path $distDir) {
        $fileCount = (Get-ChildItem $distDir -Recurse -File).Count
        Write-Host "  Built $fileCount files to dist/" -ForegroundColor Green
    }

    return $true
}

function Build-App {
    Write-Host ""
    Write-Host "--- Building Tauri App ---" -ForegroundColor Cyan
    Write-Host ""

    # Check prerequisites
    $cargo = Get-Command cargo -ErrorAction SilentlyContinue
    if (-not $cargo) {
        Write-Host "ERROR: cargo not found. Install Rust from https://rustup.rs" -ForegroundColor Red
        return $false
    }

    $pnpm = Get-Command pnpm -ErrorAction SilentlyContinue
    if (-not $pnpm) {
        Write-Host "ERROR: pnpm not found." -ForegroundColor Red
        return $false
    }

    if ($Clean) {
        Write-Host "  Cleaning Rust build..." -ForegroundColor Yellow
        Push-Location $ProjectRoot
        try { & cargo clean 2>$null } finally { Pop-Location }
    }

    # Check Rust code first
    Write-Host "  Checking Rust code..." -ForegroundColor Yellow
    Push-Location $ProjectRoot
    try {
        & cargo check --workspace 2>&1
        if ($LASTEXITCODE -ne 0) {
            Write-Host "  CARGO CHECK FAILED" -ForegroundColor Red
            return $false
        }
    } finally {
        Pop-Location
    }

    # Build the Tauri app (includes frontend build via beforeBuildCommand if configured)
    Write-Host "  Building Tauri app..." -ForegroundColor Yellow

    $cargoProfile = if ($Configuration -eq "Release") { "--release" } else { "" }

    Push-Location $ProjectRoot
    try {
        # Build frontend first since tauri.conf.json has empty beforeBuildCommand
        $frontendOk = Build-Frontend
        if (-not $frontendOk) {
            return $false
        }

        Write-Host ""
        Write-Host "  Compiling Rust backend..." -ForegroundColor Yellow
        if ($Configuration -eq "Release") {
            & pnpm tauri build
        } else {
            # For debug, just cargo build (tauri build always does release)
            & cargo build -p cable-tauri
        }

        if ($LASTEXITCODE -ne 0) {
            Write-Host "  TAURI BUILD FAILED" -ForegroundColor Red
            return $false
        }
    } finally {
        Pop-Location
    }

    Write-Host "  Tauri app built successfully." -ForegroundColor Green
    return $true
}

# ============================================================
# Main
# ============================================================

$results = @{}

switch ($Target) {
    "Driver" {
        $results["Driver"] = Build-Driver
    }
    "Frontend" {
        $results["Frontend"] = Build-Frontend
    }
    "App" {
        $results["App"] = Build-App
    }
    "All" {
        $results["Driver"] = Build-Driver
        if ($results["Driver"]) {
            $results["App"] = Build-App
        } else {
            Write-Host ""
            Write-Host "Skipping app build due to driver build failure." -ForegroundColor Yellow
            $results["App"] = $false
        }
    }
}

# ============================================================
# Summary
# ============================================================

$elapsed = (Get-Date) - $StartTime
$allPassed = ($results.Values | Where-Object { $_ -eq $false }).Count -eq 0

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host " Build Summary" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan

foreach ($key in $results.Keys) {
    $status = if ($results[$key]) { "OK" } else { "FAILED" }
    $color = if ($results[$key]) { "Green" } else { "Red" }
    Write-Host "  $key : $status" -ForegroundColor $color
}

Write-Host ""
Write-Host "  Elapsed: $($elapsed.ToString('mm\:ss'))" -ForegroundColor DarkGray
Write-Host ""

if ($allPassed) {
    Write-Host "BUILD SUCCEEDED" -ForegroundColor Green
} else {
    Write-Host "BUILD FAILED" -ForegroundColor Red
    exit 1
}
