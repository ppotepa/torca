[CmdletBinding()]
param(
    [Parameter(Mandatory = $false)][int]$DurationMinutes = 60,
    [Parameter(Mandatory = $false)][string]$Package = 'com.torca.torca_app',
    [Parameter(Mandatory = $false)][string]$OutputRoot = (Join-Path $PSScriptRoot '../artifacts/soak')
)

$ErrorActionPreference = 'Stop'
if ($DurationMinutes -lt 1) { throw 'DurationMinutes must be at least 1.' }

& adb get-state | Out-Null
if ($LASTEXITCODE -ne 0) { throw 'No ADB device is available.' }

$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$output = Join-Path $OutputRoot "battery-$stamp"
New-Item -ItemType Directory -Force -Path $output | Out-Null

function Capture-Adb([string]$Name, [string[]]$Arguments) {
    $path = Join-Path $output $Name
    & adb @Arguments 2>&1 | Out-File -Encoding utf8 $path
}

Capture-Adb 'device.txt' @('shell', 'getprop')
Capture-Adb 'battery-before.txt' @('shell', 'dumpsys', 'battery')
Capture-Adb 'power-before.txt' @('shell', 'dumpsys', 'power')

& adb shell dumpsys batterystats --reset | Out-Null
& adb shell logcat -c | Out-Null
& adb shell monkey -p $Package -c android.intent.category.LAUNCHER 1 | Out-Null
Start-Sleep -Seconds 20
& adb shell input keyevent KEYCODE_HOME | Out-Null

$started = Get-Date
@{
    package = $Package
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
    durationMinutes = $DurationMinutes
    startedAt = $started.ToString('o')
    finishedAt = (Get-Date).ToString('o')
    output = (Resolve-Path $output).Path
} | ConvertTo-Json | Out-File -Encoding utf8 (Join-Path $output 'result.json')

Write-Host "Battery soak capture complete: $output"
