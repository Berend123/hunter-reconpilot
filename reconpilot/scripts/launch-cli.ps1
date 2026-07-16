[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"

$RepoRoot = Split-Path -Parent $PSScriptRoot
$ToolsBin = Join-Path $RepoRoot "tools\bin"
$CargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
$ToolchainRoot = Join-Path $env:USERPROFILE ".rustup\toolchains\stable-x86_64-pc-windows-gnullvm"
$ToolchainBin = Join-Path $ToolchainRoot "bin"
$ToolchainLibBin = Join-Path $ToolchainRoot "lib\rustlib\x86_64-pc-windows-gnullvm\bin"
$LlvmMingwPackageRoot = Join-Path $env:LOCALAPPDATA "Microsoft\WinGet\Packages\MartinStorsjo.LLVM-MinGW.UCRT_Microsoft.Winget.Source_8wekyb3d8bbwe"
$GoBin = "C:\Program Files\Go\bin"
$GitBin = "C:\Program Files\Git\cmd"
$RubyBin = "C:\Ruby34-x64\bin"

function Write-Info([string]$Message) {
    Write-Host "[info] $Message" -ForegroundColor Cyan
}

function Write-Warn([string]$Message) {
    Write-Host "[warn] $Message" -ForegroundColor Yellow
}

function Write-Ok([string]$Message) {
    Write-Host "[ok]   $Message" -ForegroundColor Green
}

function Resolve-LlvmMingwRoot {
    if (-not (Test-Path $LlvmMingwPackageRoot)) {
        return $null
    }

    $candidate = Get-ChildItem -Path $LlvmMingwPackageRoot -Directory -ErrorAction SilentlyContinue |
        Where-Object { $_.Name -like "llvm-mingw-*-ucrt-x86_64" } |
        Sort-Object Name -Descending |
        Select-Object -First 1

    if ($candidate) {
        return $candidate.FullName
    }

    return $null
}

function Add-RustRuntimePath {
    $segments = @()
    if (Test-Path $ToolsBin) {
        $segments += $ToolsBin
    }
    if (Test-Path $CargoBin) {
        $segments += $CargoBin
    }
    if (Test-Path $ToolchainBin) {
        $segments += $ToolchainBin
    }
    if (Test-Path $ToolchainLibBin) {
        $segments += $ToolchainLibBin
    }
    $llvmMingwRoot = Resolve-LlvmMingwRoot
    if ($llvmMingwRoot) {
        $llvmMingwBin = Join-Path $llvmMingwRoot "bin"
        if (Test-Path $llvmMingwBin) {
            $segments += $llvmMingwBin
        }
    }
    foreach ($pathSegment in @($GoBin, $GitBin, $RubyBin)) {
        if (Test-Path $pathSegment) {
            $segments += $pathSegment
        }
    }
    if ($segments.Count -gt 0) {
        $env:PATH = (($segments -join ";") + ";$env:PATH")
    }
}

function Get-ReconPilotExe {
    $candidates = @(
        (Join-Path $RepoRoot "target\debug\reconpilot.exe"),
        (Join-Path $RepoRoot "target\release\reconpilot.exe")
    )
    foreach ($candidate in $candidates) {
        if (Test-Path $candidate) {
            return $candidate
        }
    }
    return $null
}

Set-Location $RepoRoot
Add-RustRuntimePath

$reconpilotExe = Get-ReconPilotExe
if (-not $reconpilotExe) {
    Write-Warn "ReconPilot is not built yet."
    Write-Host ""
    Write-Host "Build it first with one of these commands:" -ForegroundColor White
    Write-Host "  cargo build" -ForegroundColor Gray
    Write-Host "  cargo +stable-x86_64-pc-windows-gnullvm build" -ForegroundColor Gray
    exit 1
}

Write-Ok "Using ReconPilot binary: $reconpilotExe"
Write-Host ""
& $reconpilotExe --help

Write-Host ""
Write-Host "Common commands:" -ForegroundColor White
Write-Host "  .\target\debug\reconpilot.exe doctor" -ForegroundColor Gray
Write-Host "  .\target\debug\reconpilot.exe pipeline --scope config/scope.example.txt --profile passive --out output/" -ForegroundColor Gray
Write-Host "  .\scripts\test-passive.ps1" -ForegroundColor Gray
Write-Host "  .\scripts\launch-gui.ps1" -ForegroundColor Gray
Write-Host ""
Write-Host "Safety reminder:" -ForegroundColor White
Write-Host "  Dry-run remains the default. Do not use --execute against real targets without explicit authorization." -ForegroundColor Yellow
