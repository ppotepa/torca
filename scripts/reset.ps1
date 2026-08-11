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
Write-Warning 'This reset removes the selected client persistent Arti cache. Its next launch requires a cold Tor bootstrap and may spend 15-90+ seconds downloading directory data.'
Import-Module (Join-Path $PSScriptRoot 'modules/Torca.Data.psm1') -Force -WarningAction SilentlyContinue -Verbose:$false
Reset-TorcaClientData -Devices @([pscustomobject]@{ Platform = 'android'; Id = $Device })
Write-Host "Reset Torca application data on: $Device" -ForegroundColor Green
