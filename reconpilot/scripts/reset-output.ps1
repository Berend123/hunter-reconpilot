[CmdletBinding()]
param(
    [string]$OutDir = "output",
    [switch]$Force
)

$ErrorActionPreference = "Stop"

$RepoRoot = Split-Path -Parent $PSScriptRoot
$OutputRoot = Join-Path $RepoRoot $OutDir

function Write-Info([string]$Message) {
    Write-Host "[info] $Message" -ForegroundColor Cyan
}

function Write-Warn([string]$Message) {
    Write-Host "[warn] $Message" -ForegroundColor Yellow
}

function Write-Ok([string]$Message) {
    Write-Host "[ok]   $Message" -ForegroundColor Green
}

Set-Location $RepoRoot

if (-not (Test-Path $OutputRoot)) {
    New-Item -ItemType Directory -Path $OutputRoot | Out-Null
}

$resolvedRepoRoot = [System.IO.Path]::GetFullPath($RepoRoot)
$resolvedOutputRoot = [System.IO.Path]::GetFullPath($OutputRoot)
if (-not $resolvedOutputRoot.StartsWith($resolvedRepoRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "Refusing to clear output outside the repository root: $resolvedOutputRoot"
}

Write-Host "This will remove generated files under $resolvedOutputRoot" -ForegroundColor Yellow
Write-Host "It will preserve output\.gitkeep and will not touch examples/." -ForegroundColor Yellow
Write-Host ""

if (-not $Force) {
    $confirmation = Read-Host "Type RESET to continue"
    if ($confirmation -ne "RESET") {
        Write-Warn "Aborted by user."
        exit 1
    }
}

Get-ChildItem -LiteralPath $resolvedOutputRoot -Force |
    Where-Object { $_.Name -ne ".gitkeep" } |
    Remove-Item -Recurse -Force

if (-not (Test-Path (Join-Path $resolvedOutputRoot ".gitkeep"))) {
    New-Item -ItemType File -Path (Join-Path $resolvedOutputRoot ".gitkeep") | Out-Null
}

Write-Ok "Generated output was cleared safely."
