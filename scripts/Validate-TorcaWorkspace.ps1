[CmdletBinding()]
param([Parameter(Mandatory = $false)][string]$RepoRoot = (Resolve-Path "$PSScriptRoot/..").Path)

$ErrorActionPreference = 'Stop'
Push-Location $RepoRoot
try {
    cargo check --workspace --all-targets --locked
    if ($LASTEXITCODE -ne 0) { throw 'cargo check --workspace --all-targets --locked failed.' }
} finally {
    Pop-Location
}
Write-Host 'Torca workspace check passed.'
