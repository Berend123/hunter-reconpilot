[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"

$RepoRoot = Split-Path -Parent $PSScriptRoot
$GuiRoot = Join-Path $RepoRoot "gui"
$GuiSrcTauriRoot = Join-Path $GuiRoot "src-tauri"
$GuiDevToolsRoot = Join-Path $GuiRoot "dev-tools"
$ToolsBin = Join-Path $RepoRoot "tools\bin"
$GuiWindresSource = Join-Path $GuiDevToolsRoot "windres-shim.cs"
$GuiWindresExe = Join-Path $GuiDevToolsRoot "windres.exe"
$RustupToolchain = "stable-x86_64-pc-windows-gnullvm"
$ToolchainRoot = Join-Path $env:USERPROFILE ".rustup\toolchains\$RustupToolchain"
$ToolchainBin = Join-Path $ToolchainRoot "bin"
$CargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
$LlvmMingwPackageRoot = Join-Path $env:LOCALAPPDATA "Microsoft\WinGet\Packages\MartinStorsjo.LLVM-MinGW.UCRT_Microsoft.Winget.Source_8wekyb3d8bbwe"
$LlvmMingwRoot = $null
$LlvmMingwBin = $null
$LlvmMingwLinker = $null
$LlvmMingwArchiver = $null
$GuiDevPort = 1420

function Write-Info([string]$Message) {
    Write-Host "[info] $Message" -ForegroundColor Cyan
}

function Write-Warn([string]$Message) {
    Write-Host "[warn] $Message" -ForegroundColor Yellow
}

function Write-Ok([string]$Message) {
    Write-Host "[ok]   $Message" -ForegroundColor Green
}

function Test-PortInUse([int]$Port) {
    try {
        $connection = Get-NetTCPConnection -LocalPort $Port -ErrorAction Stop | Select-Object -First 1
        return $null -ne $connection
    }
    catch {
        return $false
    }
}

function Find-CSharpCompiler {
    $candidates = @(
        "C:\Windows\Microsoft.NET\Framework64\v4.0.30319\csc.exe",
        "C:\Windows\Microsoft.NET\Framework\v4.0.30319\csc.exe",
        "C:\Windows\Microsoft.NET\Framework64\v3.5\csc.exe",
        "C:\Windows\Microsoft.NET\Framework\v3.5\csc.exe"
    )

    foreach ($candidate in $candidates) {
        if (Test-Path $candidate) {
            return $candidate
        }
    }

    return $null
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

function Ensure-WindresShim {
    if (Test-Path $GuiWindresExe) {
        return
    }
    if (-not (Test-Path $GuiWindresSource)) {
        throw "Missing windres shim source: $GuiWindresSource"
    }

    $compiler = Find-CSharpCompiler
    if (-not $compiler) {
        throw "Could not find csc.exe to build the local windres shim."
    }

    New-Item -ItemType Directory -Force -Path $GuiDevToolsRoot | Out-Null
    & $compiler /nologo /target:exe /out:$GuiWindresExe $GuiWindresSource | Out-Null
    if ($LASTEXITCODE -ne 0 -or -not (Test-Path $GuiWindresExe)) {
        throw "Failed to build the local windres shim executable."
    }
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
    if ($LlvmMingwBin -and (Test-Path $LlvmMingwBin)) {
        $segments += $LlvmMingwBin
    }
    if (Test-Path $GuiDevToolsRoot) {
        $segments += $GuiDevToolsRoot
    }
    if ($segments.Count -gt 0) {
        $env:PATH = (($segments -join ";") + ";$env:PATH")
    }
}

Set-Location $RepoRoot
$LlvmMingwRoot = Resolve-LlvmMingwRoot
if ($LlvmMingwRoot) {
    $LlvmMingwBin = Join-Path $LlvmMingwRoot "bin"
    $LlvmMingwLinker = Join-Path $LlvmMingwBin "x86_64-w64-mingw32-clang.exe"
    $LlvmMingwArchiver = Join-Path $LlvmMingwBin "llvm-ar.exe"
}
Add-RustRuntimePath

$env:RUSTUP_TOOLCHAIN = $RustupToolchain
$env:CARGO_BUILD_TARGET = "x86_64-pc-windows-gnullvm"

if (-not (Test-Path $GuiRoot)) {
    Write-Warn "GUI workspace not found at $GuiRoot"
    exit 1
}
if (-not (Test-Path $GuiSrcTauriRoot)) {
    Write-Warn "Tauri workspace not found at $GuiSrcTauriRoot"
    exit 1
}
try {
    Ensure-WindresShim
}
catch {
    Write-Warn "Failed to prepare the local windres shim."
    Write-Host $_.Exception.Message -ForegroundColor DarkGray
    exit 1
}

$node = Get-Command node -ErrorAction SilentlyContinue
$npm = Get-Command npm.cmd -ErrorAction SilentlyContinue
if (-not $npm) {
    $npm = Get-Command npm -ErrorAction SilentlyContinue
}
$cargo = Get-Command cargo.exe -ErrorAction SilentlyContinue
if (-not $cargo) {
    $cargo = Get-Command cargo -ErrorAction SilentlyContinue
}

if (-not $node) {
    Write-Warn "Node.js is not available on PATH."
    Write-Host "Install Node.js first, then run npm install inside gui/." -ForegroundColor Gray
    exit 1
}
if (-not $npm) {
    Write-Warn "npm is not available on PATH."
    Write-Host "Install Node.js with npm support before launching the GUI." -ForegroundColor Gray
    exit 1
}
if (-not $cargo) {
    Write-Warn "Cargo is not available on PATH."
    Write-Host "Tauri dev mode needs Rust tooling. Install Rust and make sure $CargoBin is available." -ForegroundColor Gray
    exit 1
}
if (-not (Test-Path $ToolchainBin)) {
    Write-Warn "The Rust gnullvm toolchain was not found at $ToolchainRoot"
    Write-Host "Install it with: rustup toolchain install $RustupToolchain --profile minimal" -ForegroundColor Gray
    exit 1
}
if (-not $LlvmMingwRoot -or -not (Test-Path $LlvmMingwLinker) -or -not (Test-Path $LlvmMingwArchiver)) {
    Write-Warn "LLVM-MinGW was not found in the expected WinGet package location."
    Write-Host "Install it with: winget install --id MartinStorsjo.LLVM-MinGW.UCRT --exact --source winget" -ForegroundColor Gray
    exit 1
}

$env:CARGO_TARGET_X86_64_PC_WINDOWS_GNULLVM_LINKER = $LlvmMingwLinker
$env:CARGO_TARGET_X86_64_PC_WINDOWS_GNULLVM_AR = $LlvmMingwArchiver

try {
    Push-Location $GuiSrcTauriRoot
    & $cargo.Source metadata --no-deps --format-version 1 | Out-Null
}
catch {
    Write-Warn "Cargo is present, but Tauri could not query the Rust workspace."
    Write-Host "Check your Rust toolchain setup, then try again." -ForegroundColor Gray
    Write-Host $_.Exception.Message -ForegroundColor DarkGray
    exit 1
}
finally {
    Pop-Location
}
if (-not (Test-Path (Join-Path $GuiRoot "node_modules"))) {
    Write-Warn "GUI dependencies are missing."
    Write-Host "Run these commands first:" -ForegroundColor White
    Write-Host "  cd gui" -ForegroundColor Gray
    Write-Host "  npm install" -ForegroundColor Gray
    exit 1
}
if (-not (Test-Path (Join-Path $GuiRoot "node_modules\@tauri-apps\cli"))) {
    Write-Warn "The local Tauri CLI dependency was not found in gui/node_modules."
    Write-Host "Run npm install inside gui/ to restore the dev dependencies." -ForegroundColor Gray
    exit 1
}
if (Test-PortInUse -Port $GuiDevPort) {
    Write-Warn "Port $GuiDevPort is already in use."
    Write-Host "Close the existing Vite or Tauri dev session, then try again." -ForegroundColor Gray
    Write-Host "If needed, inspect the listener with:" -ForegroundColor White
    Write-Host "  Get-NetTCPConnection -LocalPort $GuiDevPort | Format-Table LocalAddress,LocalPort,State,OwningProcess" -ForegroundColor Gray
    exit 1
}

Write-Ok "GUI dependencies look present."
Write-Host "Workspace instructions:" -ForegroundColor White
Write-Host "  1. Use the repo root as the workspace." -ForegroundColor Gray
Write-Host "  2. Generate artifacts with .\scripts\test-passive.ps1 if the viewers are empty." -ForegroundColor Gray
Write-Host "  3. Review exact command previews before any execution from the GUI." -ForegroundColor Gray
Write-Host ""
Write-Host "Safety reminder:" -ForegroundColor White
Write-Host "  The GUI keeps dry-run as the default and never makes Codex execution implicit." -ForegroundColor Yellow
Write-Host ""
Write-Info "Launching Tauri dev mode from $GuiRoot"
Write-Info "If the app stays empty, run the passive test first so output/ contains artifacts."
Write-Info "Using Rust toolchain: $RustupToolchain"
Write-Info "Using native linker: $LlvmMingwLinker"

Push-Location $GuiRoot
try {
    & $npm.Source run tauri:dev
}
finally {
    Pop-Location
}
