[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$repoRoot = Split-Path -Parent $PSScriptRoot
Push-Location $repoRoot
try {
    cargo generate-lockfile
    if ($LASTEXITCODE -ne 0) { throw "cargo generate-lockfile failed with exit code $LASTEXITCODE." }
    Write-Host 'Cargo.lock refreshed. Review and commit the resulting dependency graph.'
} finally {
    Pop-Location
}
