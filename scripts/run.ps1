[CmdletBinding()]
param(
    [ValidateSet('auto', 'windows', 'android')]
    [string]$Target = 'auto',
    [string]$Device
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
if (-not $env:TORCA_ORCHESTRATED) {
    & (Join-Path $PSScriptRoot 'torca.ps1') -Command run -Target $Target -Device $Device
    if ($LASTEXITCODE -ne 0) { throw "Orchestrated run failed with code $LASTEXITCODE." }
    return
}

if ($Target -eq 'auto') {
    $Target = if ($env:OS -eq 'Windows_NT') { 'windows' } else { 'android' }
}

$assetsModule = Join-Path $root 'scripts/modules/Torca.PlatformAssets.psm1'
Import-Module $assetsModule -Force -WarningAction SilentlyContinue
Prepare-TorcaPlatformAssets -RepoRoot $root -Platform $Target

if ($Target -eq 'android' -and -not $Device) {
    $deviceJson = (& flutter devices --machine 2>&1 | Out-String)
    if ($LASTEXITCODE -ne 0) {
        throw 'Unable to enumerate Flutter devices.'
    }
    $androidDevice = ($deviceJson | ConvertFrom-Json) |
        Where-Object { [string]$_.targetPlatform -like 'android*' } |
        Select-Object -First 1
    if (-not $androidDevice) {
        throw 'No Android device or emulator is available. Start/connect one or pass -Device explicitly.'
    }
    $Device = [string]$androidDevice.id
    Write-Host "Android device: $Device"
}

$module = Join-Path $root 'scripts/modules/Torca.BuildEngine.psm1'
Import-Module $module -Force -WarningAction SilentlyContinue
Invoke-TorcaRun -Target $Target -Device $Device
