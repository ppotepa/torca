[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$root = Split-Path -Parent $PSScriptRoot
$release = Get-Content (Join-Path $root 'release/version.json') -Raw | ConvertFrom-Json
$cargo = Get-Content (Join-Path $root 'Cargo.toml') -Raw
$pubspec = Get-Content (Join-Path $root 'apps/client/flutter/pubspec.yaml') -Raw
$contract = Get-Content (Join-Path $root 'crates/platform/torca-bridge/src/lib.rs') -Raw

$cargoVersionMatch = [Regex]::Match(
    $cargo,
    '(?m)^\s*version\s*=\s*"([^"]+)"\s*$'
)
if (-not $cargoVersionMatch.Success) {
    throw 'Cargo workspace version could not be read from Cargo.toml.'
}
$cargoVersion = $cargoVersionMatch.Groups[1].Value
if ($cargoVersion -ne [string]$release.version) {
    throw "Cargo workspace version '$cargoVersion' does not match release version '$($release.version)'."
}

$flutterVersionMatch = [Regex]::Match(
    $pubspec,
    '(?m)^\s*version:\s*([^\s+]+)\+(\d+)\s*$'
)
if (-not $flutterVersionMatch.Success) {
    throw 'Flutter version/build could not be read from pubspec.yaml.'
}
$flutterVersion = $flutterVersionMatch.Groups[1].Value
$flutterBuild = [int]$flutterVersionMatch.Groups[2].Value
if ($flutterVersion -ne [string]$release.version -or $flutterBuild -ne [int]$release.build) {
    throw "Flutter version '$flutterVersion+$flutterBuild' does not match release '$($release.version)+$($release.build)'."
}

$contractVersionMatch = [Regex]::Match(
    $contract,
    'CONTRACT_VERSION\s*:\s*u16\s*=\s*(\d+)\s*;'
)
if (-not $contractVersionMatch.Success) {
    throw 'Bridge contract version could not be read.'
}
$contractVersion = [int]$contractVersionMatch.Groups[1].Value
if ($contractVersion -ne [int]$release.contractVersion) {
    throw "Bridge contract version '$contractVersion' does not match release '$($release.contractVersion)'."
}

Write-Host "Release metadata consistent: $($release.version)+$($release.build)"
