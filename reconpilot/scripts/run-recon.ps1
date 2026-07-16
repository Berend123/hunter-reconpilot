[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$ScopePath,

    [string]$OutDir = ".\\output"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Assert-NonEmptyFile {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path
    )

    if (-not (Test-Path $Path)) {
        throw "Required file not found: $Path"
    }

    $lines = Get-Content $Path | Where-Object {
        $_.Trim().Length -gt 0 -and -not $_.Trim().StartsWith("#")
    }

    if ($lines.Count -eq 0) {
        throw "Scope file is empty after comments and blank lines are removed: $Path"
    }
}

Assert-NonEmptyFile -Path $ScopePath

$folders = @("assets", "urls", "js", "params", "screenshots", "findings", "reports")
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
foreach ($folder in $folders) {
    New-Item -ItemType Directory -Force -Path (Join-Path $OutDir $folder) | Out-Null
}

Write-Host "ReconPilot run skeleton" -ForegroundColor Cyan
Write-Host "----------------------"
Write-Host ("Scope file : {0}" -f (Resolve-Path $ScopePath))
Write-Host ("Output dir : {0}" -f (Resolve-Path $OutDir))
Write-Host ""
Write-Host "Planned phases:" -ForegroundColor Green
Write-Host "  1. Scope validation"
Write-Host "  2. Subdomain discovery"
Write-Host "  3. Live host probing"
Write-Host "  4. Crawling"
Write-Host "  5. Historical URLs"
Write-Host "  6. JavaScript extraction"
Write-Host "  7. Parameter extraction"
Write-Host "  8. Content discovery"
Write-Host "  9. Normalization"
Write-Host "  10. Enrichment"
Write-Host "  11. Scoring"
Write-Host "  12. Reporting"
Write-Host ""
Write-Host "No external recon tools are executed by this skeleton script yet." -ForegroundColor Yellow
Write-Host "TODO: Replace the placeholder phase list with explicit scope-aware command generation."

# Placeholder examples only. Intentionally not executed.
# subfinder -dL $ScopePath -oJ (Join-Path $OutDir 'assets\\subfinder.jsonl')
# httpx -l (Join-Path $OutDir 'assets\\subdomains.txt') -json -o (Join-Path $OutDir 'assets\\httpx.jsonl')
# katana -list (Join-Path $OutDir 'assets\\live-hosts.txt') -jsonl -o (Join-Path $OutDir 'urls\\katana.jsonl')
