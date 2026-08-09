[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$Device,
    [ValidateSet('Runtime','Identity','Database','All')][string]$Scope = 'Runtime',
    [switch]$Confirm,
    [switch]$AllowDataReset
)

$ErrorActionPreference = 'Stop'
if ($Scope -eq 'Runtime') {
    $adb = Get-Command adb -ErrorAction Stop
    & $adb.Source -s $Device shell am force-stop com.torca.torca_app
    if ($LASTEXITCODE -ne 0) { throw "Unable to stop Android runtime on $Device." }
    Write-Host "Stopped Torca runtime on: $Device" -ForegroundColor Green
    return
}
if ($Scope -in @('Identity','Database','All') -and -not ($Confirm -or $AllowDataReset)) {
    throw "Reset scope '$Scope' clears Android application data. Re-run with -AllowDataReset."
}
$policy = if ($Scope -eq 'All') { 'ResetAll' } else { 'ResetSelected' }
& (Join-Path $PSScriptRoot 'torca.ps1') -Command deploy -Target android -Device $Device `
    -ClientDataPolicy $policy -Confirm -BuildPolicy Reuse -InstallPolicy Skip -RunPolicy Skip -NonInteractive
if ($LASTEXITCODE -ne 0) { throw "Client reset failed with code $LASTEXITCODE." }
