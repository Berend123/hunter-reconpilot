[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$InputFile,

    [string]$OutputFile = ".\\output\\findings\\scored.jsonl"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

if (-not (Test-Path $InputFile)) {
    throw "Input file not found: $InputFile"
}

New-Item -ItemType Directory -Force -Path (Split-Path -Parent $OutputFile) | Out-Null

$keywords = @("admin", "login", "debug", "token", "internal", "upload", "staging")
$results = New-Object System.Collections.Generic.List[string]

foreach ($line in Get-Content $InputFile) {
    if ([string]::IsNullOrWhiteSpace($line)) {
        continue
    }

    $score = 0
    foreach ($keyword in $keywords) {
        if ($line -match [regex]::Escape($keyword)) {
            $score += 10
        }
    }

    $record = [pscustomobject]@{
        raw_line = $line
        score = $score
        rationale = "Placeholder keyword scoring only. No LLM calls."
    }

    $results.Add(($record | ConvertTo-Json -Compress))
}

$results | Set-Content $OutputFile

Write-Host "Scoring placeholder pipeline complete." -ForegroundColor Green
Write-Host ("Input  : {0}" -f (Resolve-Path $InputFile))
Write-Host ("Output : {0}" -f (Resolve-Path $OutputFile))
Write-Host "TODO: Replace line-based scoring with structured JSONL parsing and rationale capture."
