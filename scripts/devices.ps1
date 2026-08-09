[CmdletBinding()]
param(
    [switch]$Diagnostics,
    [switch]$AdbOnly
)

$ErrorActionPreference = 'Stop'
$arguments = @{ Command = 'devices'; NonInteractive = $true }
if ($Diagnostics) { $arguments = @{ Command = 'status'; NonInteractive = $true } }
if ($AdbOnly) {
    $adb = Get-Command adb -ErrorAction Stop
    & $adb.Source devices -l
} else {
    & (Join-Path $PSScriptRoot 'torca.ps1') @arguments
}
if ($LASTEXITCODE -ne 0) { throw "Device inspection failed with code $LASTEXITCODE." }
