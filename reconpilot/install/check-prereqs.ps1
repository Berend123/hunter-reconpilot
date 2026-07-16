[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Test-Command {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Name
    )

    $command = Get-Command $Name -ErrorAction SilentlyContinue
    if ($null -ne $command) {
        return $command.Source
    }

    return $null
}

$checks = @(
    @{ Name = "rustc"; Label = "Rust compiler" },
    @{ Name = "cargo"; Label = "Cargo" },
    @{ Name = "go"; Label = "Go" },
    @{ Name = "python"; Label = "Python" },
    @{ Name = "git"; Label = "Git" },
    @{ Name = "choco"; Label = "Chocolatey" },
    @{ Name = "scoop"; Label = "Scoop" }
)

Write-Host "ReconPilot prerequisite check" -ForegroundColor Cyan
Write-Host "--------------------------------"

$missing = New-Object System.Collections.Generic.List[string]

foreach ($check in $checks) {
    $source = Test-Command -Name $check.Name
    if ($null -ne $source) {
        Write-Host ("[OK]   {0}: {1}" -f $check.Label, $source) -ForegroundColor Green
    }
    else {
        Write-Host ("[MISS] {0}: not found on PATH" -f $check.Label) -ForegroundColor Yellow
        $missing.Add($check.Label)
    }
}

Write-Host ""
if ($missing.Count -eq 0) {
    Write-Host "All listed prerequisites are available." -ForegroundColor Green
    exit 0
}

Write-Host "Missing prerequisites were detected. ReconPilot can still be scaffolded, but some tool installation steps will remain unavailable." -ForegroundColor Yellow
Write-Host ("Missing: {0}" -f ($missing -join ", "))
exit 0
