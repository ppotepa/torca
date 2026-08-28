[CmdletBinding()]
param([string]$RepoRoot = (Get-Location).Path)

$ErrorActionPreference = 'Stop'
$nativeManifest = Join-Path $RepoRoot 'crates/platform/torca-native/Cargo.toml'
$native = Get-Content -Raw -LiteralPath $nativeManifest
if ($native -match 'provider-tor|provider-webrtc|torca-transport-tor|torca-transport-webrtc|torca-tor') {
    throw 'torca-native still references a removed provider'
}
$tree = cargo tree --manifest-path (Join-Path $RepoRoot 'Cargo.toml') -p torca-native | Out-String
if ($tree -match '(?i)arti|torca-tor|torca-transport-tor|torca-transport-webrtc') {
    throw 'native dependency graph contains a removed provider'
}
Write-Host 'Provider boundary OK: production graph is Iroh-only.'
