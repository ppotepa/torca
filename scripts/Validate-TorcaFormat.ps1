[CmdletBinding()]
param([Parameter(Mandatory = $false)][string]$RepoRoot = (Resolve-Path "$PSScriptRoot/..").Path)

$ErrorActionPreference = 'Stop'
Push-Location $RepoRoot
try {
    cargo fmt --all -- --check
    if ($LASTEXITCODE -ne 0) { throw 'cargo fmt --all -- --check failed.' }
} finally {
    Pop-Location
}
Write-Host 'Torca formatting is clean.'
