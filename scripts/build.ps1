[CmdletBinding()]
param(
    [ValidateSet('auto', 'check', 'windows', 'android', 'all')]
    [string]$Target = 'auto',
    [ValidateSet('debug', 'release')]
    [string]$Configuration = 'debug',
    [switch]$CI
)

$ErrorActionPreference = 'Stop'
$module = Join-Path (Split-Path -Parent $PSScriptRoot) 'tools/build/Torca.Build.psm1'
Import-Module $module -Force
Invoke-TorcaBuild -Target $Target -Configuration $Configuration -CI:$CI
