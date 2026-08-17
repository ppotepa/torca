[CmdletBinding()]
param([Parameter(Mandatory = $false)][string]$RepoRoot = (Resolve-Path "$PSScriptRoot/..").Path)

$ErrorActionPreference = 'Stop'
$packages = @(
    'torca-runtime',
    'torca-client-engine',
    'torca-native',
    'torca-peer-link',
    'torca-pairing-coordinator',
    'torca-storage-sqlite',
    'torca-communication-adapters',
    'torca-tor',
    'torca-runtime-policy'
)

Push-Location $RepoRoot
try {
    foreach ($package in $packages) {
        Write-Host "Testing $package..."
        cargo test -p $package --all-targets --locked
        if ($LASTEXITCODE -ne 0) {
            throw "Tests failed for $package."
        }
    }
} finally {
    Pop-Location
}
Write-Host 'Cleanup-sensitive Torca crate tests passed.'
