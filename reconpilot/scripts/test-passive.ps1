[CmdletBinding()]
param(
    [string]$OutDir = "output"
)

$ErrorActionPreference = "Stop"

$RepoRoot = Split-Path -Parent $PSScriptRoot
$ScopePath = Join-Path $RepoRoot "config\scope.example.txt"
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

function Invoke-ReconPilotStep {
    param(
        [string]$Name,
        [string[]]$Arguments
    )

    Write-Info "Running $Name"
    & $script:ReconPilotExe @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "Step failed: $Name (exit code $LASTEXITCODE)"
    }
    Write-Ok "$Name completed"
}

Set-Location $RepoRoot
Add-RustRuntimePath

$script:ReconPilotExe = Get-ReconPilotExe
if (-not $script:ReconPilotExe) {
    Write-Warn "ReconPilot is not built yet. Build it before running the passive test."
    exit 1
}
if (-not (Test-Path $ScopePath)) {
    Write-Warn "Example scope file not found at $ScopePath"
    exit 1
}

Write-Host "Passive test flow:" -ForegroundColor White
Write-Host "  - doctor" -ForegroundColor Gray
Write-Host "  - passive pipeline" -ForegroundColor Gray
Write-Host "  - validate" -ForegroundColor Gray
Write-Host "  - llm-pack" -ForegroundColor Gray
Write-Host ""
Write-Host "Safety reminder:" -ForegroundColor White
Write-Host "  This script never passes --execute and will not launch external recon tools." -ForegroundColor Yellow
Write-Host ""

Invoke-ReconPilotStep -Name "doctor" -Arguments @("doctor")
Invoke-ReconPilotStep -Name "pipeline" -Arguments @("pipeline", "--scope", "config/scope.example.txt", "--profile", "passive", "--out", $OutDir)
Invoke-ReconPilotStep -Name "validate" -Arguments @("validate", "--input", $OutDir)
Invoke-ReconPilotStep -Name "llm-pack" -Arguments @("llm-pack", "--input", $OutDir, "--out", (Join-Path $OutDir "llm-pack"))

Write-Host ""
Write-Ok "Passive test complete."
Write-Host "Inspect these outputs next:" -ForegroundColor White
Write-Host "  $OutDir\plans\pipeline-plan.md" -ForegroundColor Gray
Write-Host "  $OutDir\maps\graph.md" -ForegroundColor Gray
Write-Host "  $OutDir\enrichment\enrichment-summary.md" -ForegroundColor Gray
Write-Host "  $OutDir\review\priority-queue.md" -ForegroundColor Gray
Write-Host "  $OutDir\llm-pack\reasoning-queue.md" -ForegroundColor Gray
Write-Host "  $OutDir\validation-report.md" -ForegroundColor Gray
