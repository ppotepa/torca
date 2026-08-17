[CmdletBinding()]
param(
    [Parameter(Mandatory = $false)][string]$RepoRoot = (Resolve-Path "$PSScriptRoot/..").Path,
    [Parameter(Mandatory = $false)][switch]$Security,
    [Parameter(Mandatory = $false)][int]$SoakIterations = 0
)

$ErrorActionPreference = 'Stop'
if ($SoakIterations -lt 0) { throw 'SoakIterations cannot be negative.' }

$steps = @(
    'Validate-TorcaFormat.ps1',
    'Validate-TorcaWorkspace.ps1',
    'Test-TorcaCleanupCrates.ps1',
    'Validate-TorcaPolicies.ps1'
)

foreach ($step in $steps) {
    $path = Join-Path $PSScriptRoot $step
    Write-Host "Running $step..."
    & $path -RepoRoot $RepoRoot
    if ($LASTEXITCODE -ne 0) { throw "Validation failed: $step" }
}

if ($Security) {
    & (Join-Path $PSScriptRoot 'Validate-TorcaSecurity.ps1') -RepoRoot $RepoRoot
    if ($LASTEXITCODE -ne 0) { throw 'Security preflight failed.' }
}

if ($SoakIterations -gt 0) {
    & (Join-Path $PSScriptRoot 'Run-TorcaDeterministicSoak.ps1') -RepoRoot $RepoRoot -Iterations $SoakIterations
    if ($LASTEXITCODE -ne 0) { throw 'Deterministic soak failed.' }
}

Write-Host 'Torca cleanup validation completed.'
if (-not $Security) { Write-Host 'Security preflight was not requested; pass -Security for release-oriented validation.' }
if ($SoakIterations -eq 0) { Write-Host 'Deterministic soak was not requested; pass -SoakIterations N to run it.' }
Write-Host 'Android battery/connectivity soaks remain explicit physical-device validations.'
