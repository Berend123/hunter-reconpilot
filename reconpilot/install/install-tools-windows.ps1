[CmdletBinding()]
param(
    [switch]$Execute
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Get-CommandPath {
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

$ScriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRoot = Split-Path -Parent $ScriptRoot
$ToolsRoot = Join-Path $ProjectRoot "tools"
$BinRoot = Join-Path $ToolsRoot "bin"
$SrcRoot = Join-Path $ToolsRoot "src"
$SourcesPath = Join-Path $ScriptRoot "tool-sources.json"

New-Item -ItemType Directory -Force -Path $ToolsRoot | Out-Null
New-Item -ItemType Directory -Force -Path $BinRoot | Out-Null
New-Item -ItemType Directory -Force -Path $SrcRoot | Out-Null

$goPath = Get-CommandPath -Name "go"
$gitPath = Get-CommandPath -Name "git"
$pythonPath = Get-CommandPath -Name "python"

Write-Host "ReconPilot tool installer" -ForegroundColor Cyan
Write-Host "-------------------------" 
Write-Host ("Project root : {0}" -f $ProjectRoot)
Write-Host ("Tools root   : {0}" -f $ToolsRoot)
Write-Host ("Bin root     : {0}" -f $BinRoot)
Write-Host ("Source root  : {0}" -f $SrcRoot)
Write-Host ""

if (-not (Test-Path $SourcesPath)) {
    throw "tool-sources.json was not found at $SourcesPath"
}

$sources = Get-Content $SourcesPath -Raw | ConvertFrom-Json

Write-Host ("Loaded {0} tool definitions." -f $sources.Count) -ForegroundColor Green

if (-not $Execute) {
    Write-Host ""
    Write-Host "Dry-run mode is active. Review the plan below, then rerun with -Execute to perform the scripted actions." -ForegroundColor Yellow
}

foreach ($tool in $sources) {
    Write-Host ""
    Write-Host ("[{0}] {1}" -f $tool.category, $tool.name) -ForegroundColor Cyan
    Write-Host ("  Install method : {0}" -f $tool.install_method)
    Write-Host ("  Output format  : {0}" -f $tool.output_format)

    switch -Regex ($tool.install_method) {
        "^go install " {
            if (-not $goPath) {
                Write-Host "  Skipped: Go is not available on PATH." -ForegroundColor Yellow
                continue
            }

            if (-not $Execute) {
                Write-Host ("  Planned: {0}" -f $tool.install_method)
                continue
            }

            Write-Host ("  Running: {0}" -f $tool.install_method) -ForegroundColor Green
            Push-Location $ProjectRoot
            try {
                Invoke-Expression $tool.install_method
            }
            finally {
                Pop-Location
            }
        }
        "^git clone " {
            if (-not $gitPath) {
                Write-Host "  Skipped: Git is not available on PATH." -ForegroundColor Yellow
                continue
            }

            $target = Join-Path $SrcRoot $tool.name
            if (Test-Path $target) {
                Write-Host ("  Exists: {0}" -f $target) -ForegroundColor Green
                continue
            }

            if (-not $Execute) {
                Write-Host ("  Planned clone target: {0}" -f $target)
                continue
            }

            Write-Host ("  Cloning into: {0}" -f $target) -ForegroundColor Green
            git clone $tool.repo_url $target
        }
        "^manual" {
            Write-Host ("  Manual step required: {0}" -f $tool.notes) -ForegroundColor Yellow
        }
        default {
            Write-Host ("  TODO: Handle install method '{0}' in a future adapter." -f $tool.install_method) -ForegroundColor Yellow
        }
    }
}

Write-Host ""
Write-Host "Scripted installation flow complete." -ForegroundColor Green
Write-Host "TODO: Add per-tool version pinning, checksum verification, and virtualenv setup for Python tools."
