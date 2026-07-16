[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$InputFile,

    [string]$OutputFile = ".\\output\\urls\\normalized.txt"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

if (-not (Test-Path $InputFile)) {
    throw "Input file not found: $InputFile"
}

New-Item -ItemType Directory -Force -Path (Split-Path -Parent $OutputFile) | Out-Null

$lines = Get-Content $InputFile | Where-Object {
    $_.Trim().Length -gt 0
} | Sort-Object -Unique

$lines | Set-Content $OutputFile

Write-Host "Normalized URL placeholder pipeline complete." -ForegroundColor Green
Write-Host ("Input  : {0}" -f (Resolve-Path $InputFile))
Write-Host ("Output : {0}" -f (Resolve-Path $OutputFile))
Write-Host ""
Write-Host "Where helper tools fit:" -ForegroundColor Cyan
Write-Host "  uro : reduce duplicate URL shapes and low-value variants"
Write-Host "  jq  : extract URL fields from JSON and JSONL records"
Write-Host "  gf  : label interesting parameter or path patterns after normalization"
Write-Host ""
Write-Host "TODO: Replace the simple PowerShell dedupe with a structured JSONL-aware normalization stage."
