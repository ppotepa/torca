[CmdletBinding()]
param(
    [ValidateSet('auto', 'windows', 'android')]
    [string]$Target = 'auto',
    [string]$Device
)

$ErrorActionPreference = 'Stop'
$module = Join-Path (Split-Path -Parent $PSScriptRoot) 'tools/build/Torca.Build.psm1'
Import-Module $module -Force
Invoke-TorcaRun -Target $Target -Device $Device
