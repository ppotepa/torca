[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$Device,
    [switch]$NoRun
)

$ErrorActionPreference = 'Stop'
$runPolicy = if ($NoRun) { 'Skip' } else { 'Restart' }
& (Join-Path $PSScriptRoot 'deploy.ps1') -Target android -Device $Device `
    -BuildPolicy Reuse -InstallPolicy Selected -RunPolicy $runPolicy -ReuseBuild
if ($LASTEXITCODE -ne 0) { throw "Android redeploy failed with code $LASTEXITCODE." }
