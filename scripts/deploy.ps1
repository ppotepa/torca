[CmdletBinding()]
param(
    [ValidateSet('auto', 'windows', 'android', 'all')]
    [string]$Target = 'auto',
    [string]$Device
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot

$resolved = $Target
if ($resolved -eq 'auto') {
    $resolved = if ($env:OS -eq 'Windows_NT') { 'windows' } else { 'android' }
}
$assetsModule = Join-Path $root 'tools/build/Torca.PlatformAssets.psm1'
Import-Module $assetsModule -Force
if ($resolved -in @('windows','all')) {
    Prepare-TorcaPlatformAssets -RepoRoot $root -Platform windows
}
if ($resolved -in @('android','all')) {
    Prepare-TorcaPlatformAssets -RepoRoot $root -Platform android
}

$module = Join-Path $root 'tools/build/Torca.Build.psm1'
Import-Module $module -Force
Invoke-TorcaDeploy -Target $Target -Device $Device
