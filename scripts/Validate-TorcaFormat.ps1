[CmdletBinding()]
param([Parameter(Mandatory = $false)][string]$RepoRoot = $null)

$ErrorActionPreference = 'Stop'
$scriptRoot = if ([string]::IsNullOrWhiteSpace($PSScriptRoot)) { (Get-Location).Path } else { $PSScriptRoot }
if ([string]::IsNullOrWhiteSpace($RepoRoot)) { $RepoRoot = (Resolve-Path (Join-Path $scriptRoot '..')).Path }
Push-Location $RepoRoot
try {
    cargo fmt --all -- --check
    if ($LASTEXITCODE -ne 0) { throw 'cargo fmt --all -- --check failed.' }
} finally {
    Pop-Location
}
Write-Host 'Torca formatting is clean.'
