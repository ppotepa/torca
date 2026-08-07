[CmdletBinding()]
param(
    [ValidateSet('auto', 'windows', 'android')]
    [string]$Target = 'auto',
    [string]$Device
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot

if ($Target -eq 'auto') {
    $Target = if ($env:OS -eq 'Windows_NT') { 'windows' } else { 'android' }
}

$assetsModule = Join-Path $root 'tools/build/Torca.PlatformAssets.psm1'
Import-Module $assetsModule -Force
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

$module = Join-Path $root 'tools/build/Torca.Build.psm1'
Import-Module $module -Force
Invoke-TorcaRun -Target $Target -Device $Device
