[CmdletBinding()]
param(
    [ValidateSet('auto', 'windows', 'android', 'all')]
    [string]$Target = 'auto',
    [string]$Device
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$nativeSource = Get-Content (Join-Path $root 'crates/platform/torca-native/src/lib.rs') -Raw
if ($nativeSource.Contains('ClientEngine::default()')) {
    Write-Warning 'Native production persistence/key composition is still open. deploy.ps1 will create TEST/ALPHA artifacts only; do not treat them as a production release.'
}

$module = Join-Path $root 'tools/build/Torca.Build.psm1'
Import-Module $module -Force
Invoke-TorcaDeploy -Target $Target -Device $Device
