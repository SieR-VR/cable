#Requires -Version 5.1
<#
.SYNOPSIS
    Build script for the Cable Virtual Audio Driver.

.DESCRIPTION
    Locates MSBuild and WDK tools automatically, then builds the driver solution.
    Supports Debug/Release configurations and x64/ARM64 platforms.

.PARAMETER Configuration
    Build configuration: Debug or Release (default: Release)

.PARAMETER Platform
    Target platform: x64 or ARM64 (default: x64)

.PARAMETER Clean
    Clean build output before building.

.PARAMETER TestSign
    Create a test-signed driver package after build.

.EXAMPLE
    .\build.ps1
    .\build.ps1 -Configuration Debug -Platform x64
    .\build.ps1 -Clean -TestSign
#>

[CmdletBinding()]
param(
    [ValidateSet("Debug", "Release")]
    [string]$Configuration = "Release",

    [ValidateSet("x64", "ARM64")]
    [string]$Platform = "x64",

    [switch]$Clean,
    [switch]$TestSign
)

$ErrorActionPreference = "Stop"
$DriverRoot = Split-Path -Parent $PSScriptRoot
$SolutionFile = Join-Path $DriverRoot "CableAudio.sln"

Write-Host "========================================" -ForegroundColor Cyan
Write-Host " Cable Audio Driver Build" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# ---- Locate MSBuild ----
function Find-MSBuild {
    # Try PATH first
    $msbuild = Get-Command msbuild -ErrorAction SilentlyContinue
    if ($msbuild) {
        Write-Host "Found MSBuild in PATH: $($msbuild.Source)" -ForegroundColor Green
        return $msbuild.Source
    }

    # Try vswhere
    $vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
    if (Test-Path $vswhere) {
        $vsPath = & $vswhere -latest -requires Microsoft.Component.MSBuild -property installationPath 2>$null
        if ($vsPath) {
            $msbuildPath = Join-Path $vsPath "MSBuild\Current\Bin\MSBuild.exe"
            if (Test-Path $msbuildPath) {
                Write-Host "Found MSBuild via vswhere: $msbuildPath" -ForegroundColor Green
                return $msbuildPath
            }
        }
    }

    # Search common paths
    $searchPaths = @(
        "$env:ProgramFiles\Microsoft Visual Studio\2022\Community\MSBuild\Current\Bin\MSBuild.exe",
        "$env:ProgramFiles\Microsoft Visual Studio\2022\Professional\MSBuild\Current\Bin\MSBuild.exe",
        "$env:ProgramFiles\Microsoft Visual Studio\2022\Enterprise\MSBuild\Current\Bin\MSBuild.exe",
        "$env:ProgramFiles\Microsoft Visual Studio\2022\BuildTools\MSBuild\Current\Bin\MSBuild.exe",
        "${env:ProgramFiles(x86)}\Microsoft Visual Studio\2022\BuildTools\MSBuild\Current\Bin\MSBuild.exe",
        "${env:ProgramFiles(x86)}\Microsoft Visual Studio\2022\Community\MSBuild\Current\Bin\MSBuild.exe"
    )

    foreach ($path in $searchPaths) {
        if (Test-Path $path) {
            Write-Host "Found MSBuild: $path" -ForegroundColor Green
            return $path
        }
    }

    throw "MSBuild not found. Install Visual Studio 2022 or VS Build Tools with the 'Desktop development with C++' workload."
}

# ---- Locate WDK ----
function Find-WDK {
    $wdkRoot = Get-ItemProperty -Path "HKLM:\SOFTWARE\Microsoft\Windows Kits\Installed Roots" -Name "KitsRoot10" -ErrorAction SilentlyContinue
    if ($wdkRoot) {
        $root = $wdkRoot.KitsRoot10
        # Find latest installed WDK version
        $versions = Get-ChildItem (Join-Path $root "Include") -Directory -ErrorAction SilentlyContinue | 
            Where-Object { $_.Name -match '^\d+\.\d+\.\d+\.\d+$' } |
            Sort-Object { [version]$_.Name } -Descending
        
        if ($versions) {
            $latest = $versions[0].Name
            Write-Host "Found WDK $latest at: $root" -ForegroundColor Green
            return @{ Root = $root; Version = $latest }
        }
    }

    # Fallback: check default path
    $defaultPath = "C:\Program Files (x86)\Windows Kits\10"
    if (Test-Path $defaultPath) {
        $versions = Get-ChildItem (Join-Path $defaultPath "Include") -Directory -ErrorAction SilentlyContinue | 
            Where-Object { $_.Name -match '^\d+\.\d+\.\d+\.\d+$' } |
            Sort-Object { [version]$_.Name } -Descending
        
        if ($versions) {
            $latest = $versions[0].Name
            Write-Host "Found WDK $latest at: $defaultPath" -ForegroundColor Green
            return @{ Root = $defaultPath; Version = $latest }
        }
    }

    Write-Warning "WDK not found. Build may fail if WDK is not configured in the solution."
    return $null
}

# ---- Locate SignTool ----
function Find-SignTool {
    param([hashtable]$Wdk)

    if (-not $Wdk) { return $null }

    $signTool = Join-Path $Wdk.Root "bin\$($Wdk.Version)\x64\signtool.exe"
    if (Test-Path $signTool) {
        return $signTool
    }

    # Try without version in path
    $signTool = Join-Path $Wdk.Root "bin\x64\signtool.exe"
    if (Test-Path $signTool) {
        return $signTool
    }

    return $null
}

# ---- Main ----
$msbuildExe = Find-MSBuild
$wdk = Find-WDK
$signTool = Find-SignTool -Wdk $wdk

Write-Host ""
Write-Host "Configuration : $Configuration" -ForegroundColor Yellow
Write-Host "Platform      : $Platform" -ForegroundColor Yellow
Write-Host "Solution      : $SolutionFile" -ForegroundColor Yellow
Write-Host ""

# Clean if requested
if ($Clean) {
    Write-Host "Cleaning..." -ForegroundColor Yellow
    & $msbuildExe $SolutionFile /t:Clean /p:Configuration=$Configuration /p:Platform=$Platform /verbosity:minimal
    if ($LASTEXITCODE -ne 0) {
        throw "Clean failed with exit code $LASTEXITCODE"
    }
    Write-Host "Clean complete." -ForegroundColor Green
    Write-Host ""
}

# Build
Write-Host "Building..." -ForegroundColor Yellow

$msbuildArgs = @(
    $SolutionFile,
    "/p:Configuration=$Configuration",
    "/p:Platform=$Platform",
    "/p:Inf2CatUseLocalTime=true",
    "/verbosity:minimal",
    "/m"  # Parallel build
)

if ($Platform -eq "ARM64") {
    $msbuildArgs += @(
        "/p:RunCodeAnalysis=false",
        "/p:DriverTargetPlatform=Universal",
        "/p:UseInfVerifierEx=false",
        "/p:ValidateDrivers=false",
        "/p:StampInf=false",
        "/p:ApiValidator_Enable=false",
        "/p:InfVerif_Enable=false",
        "/p:DisableVerification=true",
        "/p:SignMode=Off",
        "/p:EnableInf2cat=false"
    )
}

& $msbuildExe @msbuildArgs

if ($LASTEXITCODE -ne 0) {
    Write-Host ""
    Write-Host "BUILD FAILED (exit code $LASTEXITCODE)" -ForegroundColor Red
    exit $LASTEXITCODE
}

Write-Host ""
Write-Host "BUILD SUCCEEDED" -ForegroundColor Green

# Output location
$outputDir = Join-Path $DriverRoot "$Platform\$Configuration\package"
Write-Host ""
Write-Host "Output: $outputDir" -ForegroundColor Cyan

if (Test-Path $outputDir) {
    Write-Host ""
    Write-Host "Package contents:" -ForegroundColor Cyan
    Get-ChildItem $outputDir | ForEach-Object {
        Write-Host "  $($_.Name)" -ForegroundColor White
    }
}

# Test signing
if ($TestSign -and $signTool) {
    $sysFile = Join-Path $outputDir "CableAudio.sys"
    $pfxFile = Join-Path (Split-Path $DriverRoot) "target\debug\CableAudioTestCert.pfx"
    $pfxPassword = "cable-test"
    
    if ((Test-Path $sysFile) -and (Test-Path $pfxFile)) {
        Write-Host ""
        Write-Host "Test signing driver..." -ForegroundColor Yellow
        & $signTool sign /fd SHA256 /f $pfxFile /p $pfxPassword /t http://timestamp.digicert.com $sysFile
        if ($LASTEXITCODE -eq 0) {
            Write-Host "Test signing succeeded." -ForegroundColor Green
        } else {
            Write-Warning "Test signing failed (exit code $LASTEXITCODE)."
        }
    } else {
        Write-Warning "Cannot test sign: sys file or certificate not found."
        Write-Warning "  sys: $sysFile (exists: $(Test-Path $sysFile))"
        Write-Warning "  pfx: $pfxFile (exists: $(Test-Path $pfxFile))"
        Write-Warning "  Run: .\scripts\create-test-cert.ps1 (as Administrator)"
    }
}

Write-Host ""
Write-Host "Done." -ForegroundColor Green
