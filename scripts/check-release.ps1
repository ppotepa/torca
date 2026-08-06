[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$release = Get-Content (Join-Path $root 'release/version.json') -Raw | ConvertFrom-Json
$cargo = Get-Content (Join-Path $root 'Cargo.toml') -Raw
$pubspec = Get-Content (Join-Path $root 'apps/client/flutter/pubspec.yaml') -Raw
$escaped = [Regex]::Escape([string]$release.version)
if ($cargo -notmatch "(?m)^version = `"$escaped`"$") { throw 'Cargo workspace version does not match release/version.json.' }
if ($pubspec -notmatch "(?m)^version: $escaped\+$($release.build)$") { throw 'Flutter version/build does not match release/version.json.' }
$contract = Get-Content (Join-Path $root 'crates/platform/torca-bridge/src/lib.rs') -Raw
if ($contract -notmatch "CONTRACT_VERSION: u16 = $($release.contractVersion);") { throw 'Bridge contract version mismatch.' }
Write-Host "Release metadata consistent: $($release.version)+$($release.build)"
