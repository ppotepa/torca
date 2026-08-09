[CmdletBinding()]
param(
    [ValidateSet('auto', 'check', 'windows', 'android', 'all')]
    [string]$Target = 'auto',
    [ValidateSet('debug', 'release')]
    [string]$Configuration = 'debug',
    [switch]$CI
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot

& (Join-Path $root 'scripts/modules/Torca.SourcePolicy.ps1') -RepoRoot $root

$assetsModule = Join-Path $root 'scripts/modules/Torca.PlatformAssets.psm1'
Import-Module $assetsModule -Force -WarningAction SilentlyContinue

switch ($Target) {
    'windows' { Prepare-TorcaPlatformAssets -RepoRoot $root -Platform windows }
    'android' { Prepare-TorcaPlatformAssets -RepoRoot $root -Platform android }
    'all' {
        Prepare-TorcaPlatformAssets -RepoRoot $root -Platform windows
        Prepare-TorcaPlatformAssets -RepoRoot $root -Platform android
    }
    'auto' {
        if ($env:OS -eq 'Windows_NT') {
            Prepare-TorcaPlatformAssets -RepoRoot $root -Platform windows
        }
    }
}

$module = Join-Path $root 'scripts/modules/Torca.BuildEngine.psm1'
# Direct builds must carry the same deterministic identity as orchestrated deploys.
# The native metadata is compiled from these values and Flutter validates the build id before
# executing any request.
$buildIdentityModule = Join-Path $root 'scripts/modules/Torca.Build.psm1'
Import-Module $buildIdentityModule -Force -WarningAction SilentlyContinue
if ($Target -ne 'check' -and [string]::IsNullOrWhiteSpace($env:TORCA_BUILD_ID)) {
    $release = Get-Content (Join-Path $root 'release/version.json') -Raw | ConvertFrom-Json
    $endpoint = [string]$env:TORCA_RELAY_ENDPOINT
    if ([string]::IsNullOrWhiteSpace($endpoint)) {
        $endpointFile = Join-Path $root 'release/relay_endpoint.txt'
        if (Test-Path -LiteralPath $endpointFile) { $endpoint = (Get-Content $endpointFile -Raw).Trim() }
    }
    if ([string]::IsNullOrWhiteSpace($endpoint)) {
        $endpointFile = Join-Path $root '.torca/stack/relay_endpoint.txt'
        if (Test-Path -LiteralPath $endpointFile) { $endpoint = (Get-Content $endpointFile -Raw).Trim() }
    }
    if ([string]::IsNullOrWhiteSpace($endpoint)) { $endpoint = 'configured-at-build' }
    $env:TORCA_BUILD_ID = Get-TorcaBuildId -RepoRoot $root -Endpoint $endpoint -Target $Target -Configuration $Configuration
    $env:TORCA_PRODUCT_VERSION = [string]$release.version
    $env:TORCA_SOURCE_FINGERPRINT = Get-TorcaBuildSourceFingerprint -RepoRoot $root
    $env:TORCA_SOURCE_COMMIT = ((git -C $root rev-parse HEAD 2>$null | Out-String).Trim())
    if ([string]::IsNullOrWhiteSpace($env:TORCA_SOURCE_COMMIT)) { $env:TORCA_SOURCE_COMMIT = 'working-tree' }
    $endpointSha = [System.Security.Cryptography.SHA256]::Create()
    try {
        $env:TORCA_RELAY_ENDPOINT_HASH = [BitConverter]::ToString(
            $endpointSha.ComputeHash([Text.Encoding]::UTF8.GetBytes($endpoint))
        ).Replace('-', '').ToLowerInvariant()
    } finally { $endpointSha.Dispose() }
}
Import-Module $module -Force -WarningAction SilentlyContinue
Invoke-TorcaBuild -Target $Target -Configuration $Configuration -CI:$CI
