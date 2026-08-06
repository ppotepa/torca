[CmdletBinding()]
param(
    [ValidateSet('auto', 'windows', 'android', 'all')]
    [string]$Target = 'auto',
    [string]$Device
)

$ErrorActionPreference = 'Stop'
$module = Join-Path (Split-Path -Parent $PSScriptRoot) 'tools/build/Torca.Build.psm1'
Import-Module $module -Force
Invoke-TorcaDeploy -Target $Target -Device $Device
