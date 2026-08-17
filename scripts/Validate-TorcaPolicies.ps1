[CmdletBinding()]
param([Parameter(Mandatory = $false)][string]$RepoRoot = (Resolve-Path "$PSScriptRoot/..").Path)

$ErrorActionPreference = 'Stop'
$modules = Join-Path $RepoRoot 'scripts/modules'
$checks = @(
    'Torca.ArchitecturePolicy.ps1',
    'Torca.SourcePolicy.ps1',
    'Torca.SqlOwnershipPolicy.ps1',
    'Torca.StorageFactoryPolicy.ps1',
    'Torca.SecretLifetimePolicy.ps1',
    'Torca.LintPolicy.ps1',
    'Torca.DebtPolicy.ps1',
    'Torca.LogRedactionPolicy.ps1',
    'Torca.IncludePolicy.ps1'
)

foreach ($check in $checks) {
    $path = Join-Path $modules $check
    if (-not (Test-Path -LiteralPath $path)) {
        throw "Policy script is missing: $check"
    }
    Write-Host "Running $check..."
    & $path -RepoRoot $RepoRoot
    if ($LASTEXITCODE -ne 0) {
        throw "Policy failed: $check"
    }
}

Write-Host 'Torca architecture/source/contract policies passed.'
