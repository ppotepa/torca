[CmdletBinding()]
param(
    [ValidateSet('relay','android','all')][string]$Source = 'all',
    [string]$Device,
    [ValidateRange(20,5000)][int]$Tail = 120
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
if ($Source -in @('relay','all')) {
    Write-Host '=== Relay logs ===' -ForegroundColor Cyan
    & docker compose -f (Join-Path $root 'infra/docker/compose.yml') logs --tail=$Tail relay
}
if ($Source -in @('android','all')) {
    if ([string]::IsNullOrWhiteSpace($Device)) { throw '-Device is required for Android logs.' }
    $adb = Get-Command adb -ErrorAction Stop
    Write-Host "=== Android logs: $Device ===" -ForegroundColor Cyan
    & $adb.Source -s $Device logcat -d -t $Tail -v threadtime
    Write-Host '=== Android crash buffer ===' -ForegroundColor Cyan
    & $adb.Source -s $Device logcat -b crash -d -t $Tail -v threadtime
}
