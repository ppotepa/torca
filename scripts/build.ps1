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
$assetsModule = Join-Path $root 'tools/build/Torca.PlatformAssets.psm1'
Import-Module $assetsModule -Force

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

$module = Join-Path $root 'tools/build/Torca.Build.psm1'
Import-Module $module -Force
Invoke-TorcaBuild -Target $Target -Configuration $Configuration -CI:$CI
