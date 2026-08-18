[CmdletBinding()]
param(
    [Parameter(Mandatory = $false)][int]$DurationMinutes = 60,
    [Parameter(Mandatory = $false)][string]$Package = 'com.torca.torca_app',
    [Parameter(Mandatory = $false)][string]$DeviceId,
    [Parameter(Mandatory = $false)][string]$OutputRoot
)

$ErrorActionPreference = 'Stop'
if ($DurationMinutes -lt 1) { throw 'DurationMinutes must be at least 1.' }
if (-not $OutputRoot) {
    $scriptRoot = if ($PSScriptRoot) { $PSScriptRoot } else { Split-Path -Parent $MyInvocation.MyCommand.Path }
    $OutputRoot = Join-Path $scriptRoot '../artifacts/soak'
}

$deviceLines = @(& adb devices 2>&1)
$readyDevices = @($deviceLines | Where-Object { $_ -match '^\S+\s+device\s*$' } | ForEach-Object { ($_ -split '\s+')[0] })
if ($DeviceId) {
    if ($readyDevices -notcontains $DeviceId) {
        throw "Requested ADB device '$DeviceId' is not ready. Ready devices: $($readyDevices -join ', ')."
    }
} elseif ($readyDevices.Count -ne 1) {
    throw "Expected exactly one ready ADB device; found $($readyDevices.Count). Pass -DeviceId explicitly. Ready devices: $($readyDevices -join ', ')."
} else {
    $DeviceId = $readyDevices[0]
}

$adbPrefix = @('-s', $DeviceId)
function Invoke-SelectedAdb([string[]]$Arguments) {
    & adb @($adbPrefix + $Arguments)
}

$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$output = Join-Path $OutputRoot "battery-$stamp"
New-Item -ItemType Directory -Force -Path $output | Out-Null

function Capture-Adb([string]$Name, [string[]]$Arguments) {
    $path = Join-Path $output $Name
    Invoke-SelectedAdb $Arguments 2>&1 | Out-File -Encoding utf8 $path
}

Capture-Adb 'device.txt' @('shell', 'getprop')
Capture-Adb 'battery-before.txt' @('shell', 'dumpsys', 'battery')
Capture-Adb 'power-before.txt' @('shell', 'dumpsys', 'power')

Invoke-SelectedAdb @('shell', 'dumpsys', 'batterystats', '--reset') | Out-Null
Invoke-SelectedAdb @('shell', 'logcat', '-c') | Out-Null
Invoke-SelectedAdb @('shell', 'monkey', '-p', $Package, '-c', 'android.intent.category.LAUNCHER', '1') | Out-Null
Start-Sleep -Seconds 2
$appPid = (Invoke-SelectedAdb @('shell', 'pidof', $Package) 2>$null | Out-String).Trim()
if (-not $appPid) { throw "Torca process '$Package' did not start on ADB device '$DeviceId'." }
Start-Sleep -Seconds 20
Invoke-SelectedAdb @('shell', 'input', 'keyevent', 'KEYCODE_HOME') | Out-Null

$started = Get-Date
@{
    package = $Package
    deviceId = $DeviceId
    durationMinutes = $DurationMinutes
    startedAt = $started.ToString('o')
    scenario = 'warm-start then background idle'
} | ConvertTo-Json | Out-File -Encoding utf8 (Join-Path $output 'scenario.json')

Start-Sleep -Seconds ($DurationMinutes * 60)

Capture-Adb 'batterystats.txt' @('shell', 'dumpsys', 'batterystats', $Package)
Capture-Adb 'batterystats-checkin.txt' @('shell', 'dumpsys', 'batterystats', '--checkin', $Package)
Capture-Adb 'battery-after.txt' @('shell', 'dumpsys', 'battery')
Capture-Adb 'power-after.txt' @('shell', 'dumpsys', 'power')
Capture-Adb 'deviceidle-after.txt' @('shell', 'dumpsys', 'deviceidle')
Capture-Adb 'services-after.txt' @('shell', 'dumpsys', 'activity', 'services', $Package)
Capture-Adb 'process-after.txt' @('shell', 'ps', '-A')
Capture-Adb 'logcat.txt' @('logcat', '-d', '-v', 'threadtime')

@{
    package = $Package
    deviceId = $DeviceId
    durationMinutes = $DurationMinutes
    startedAt = $started.ToString('o')
    finishedAt = (Get-Date).ToString('o')
    output = (Resolve-Path $output).Path
} | ConvertTo-Json | Out-File -Encoding utf8 (Join-Path $output 'result.json')

Write-Host "Battery soak capture complete: $output"
