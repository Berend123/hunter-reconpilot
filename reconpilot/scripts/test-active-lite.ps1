[CmdletBinding()]
param(
    [string]$ScopePath = "config/scope.txt",
    [string]$OutDir = "output",
    [switch]$Force
)

$ErrorActionPreference = "Stop"

$RepoRoot = Split-Path -Parent $PSScriptRoot
$ToolsBin = Join-Path $RepoRoot "tools\bin"
$CargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
$ToolchainRoot = Join-Path $env:USERPROFILE ".rustup\toolchains\stable-x86_64-pc-windows-gnullvm"
$ToolchainBin = Join-Path $ToolchainRoot "bin"
$ToolchainLibBin = Join-Path $ToolchainRoot "lib\rustlib\x86_64-pc-windows-gnullvm\bin"
$LlvmMingwPackageRoot = Join-Path $env:LOCALAPPDATA "Microsoft\WinGet\Packages\MartinStorsjo.LLVM-MinGW.UCRT_Microsoft.Winget.Source_8wekyb3d8bbwe"
$RequiredTools = @("subfinder", "httpx", "katana", "gau", "dnsx", "gowitness", "whatweb")
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
    Write-Warn "ReconPilot is not built yet. Build it before running active-lite."
    exit 1
}

$resolvedScopePath = Join-Path $RepoRoot $ScopePath
if (-not (Test-Path $resolvedScopePath)) {
    Write-Warn "Scope file not found: $resolvedScopePath"
    exit 1
}
if ([System.IO.Path]::GetFileName($resolvedScopePath) -eq "scope.example.txt") {
    Write-Warn "Active-lite refuses to use the example scope file."
    Write-Host "Create a real authorized scope file, for example config\scope.txt, before running target-touching phases." -ForegroundColor Gray
    exit 1
}

$missingTools = @()
foreach ($tool in $RequiredTools) {
    if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) {
        $missingTools += $tool
    }
}
if ($missingTools.Count -gt 0) {
    Write-Warn "Missing required external tools for active-lite:"
    $missingTools | ForEach-Object { Write-Host "  - $_" -ForegroundColor Gray }
    Write-Host "Install the required tools before allowing active-lite execution." -ForegroundColor Gray
    exit 1
}

Write-Host "Active-lite will run target-touching phases with --execute." -ForegroundColor Yellow
Write-Host "This may contact systems that are inside the scope file you provide." -ForegroundColor Yellow
Write-Host "Codex is not included and will not be executed by this script." -ForegroundColor Yellow
Write-Host ""

if (-not $Force) {
    $confirmation = Read-Host "Type EXECUTE to continue"
    if ($confirmation -ne "EXECUTE") {
        Write-Warn "Aborted by user."
        exit 1
    }
}

Write-Info "Launching active-lite profile with explicit execution enabled."
& $reconpilotExe "pipeline" "--scope" $ScopePath "--profile" "active-lite" "--out" $OutDir "--execute"
if ($LASTEXITCODE -ne 0) {
    throw "Active-lite pipeline failed with exit code $LASTEXITCODE"
}

Write-Host ""
Write-Ok "Active-lite run finished."
Write-Host "Inspect these outputs next:" -ForegroundColor White
Write-Host "  $OutDir\plans\pipeline-plan.md" -ForegroundColor Gray
Write-Host "  $OutDir\audit-log.jsonl" -ForegroundColor Gray
Write-Host "  $OutDir\run-manifest.json" -ForegroundColor Gray
Write-Host "  $OutDir\validation-report.md" -ForegroundColor Gray
