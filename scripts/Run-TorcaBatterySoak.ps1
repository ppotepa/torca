[CmdletBinding()]
param(
    [Parameter(Mandatory = $false)][int]$DurationMinutes = 60,
    [Parameter(Mandatory = $false)][string]$Package = 'com.torca.torca_app',
    [Parameter(Mandatory = $false)][string]$DeviceId,
    [Parameter(Mandatory = $false)][string]$OutputRoot,
    [Parameter(Mandatory = $false)][switch]$RequireUnplugged,
    [Parameter(Mandatory = $false)][switch]$RequireScreenOff
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

function Get-PowerSource([string]$BatteryText) {
    $sources = @('AC', 'USB', 'Wireless') | Where-Object {
        $BatteryText -match "(?im)^\s*$($_) powered:\s*true\s*$"
    }
    if ($sources.Count -eq 0) { return 'battery' }
    return ($sources -join ',').ToLowerInvariant()
}

function Get-BatteryLevel([string]$BatteryText) {
    $match = [regex]::Match($BatteryText, '(?im)^\s*level:\s*(\d+)\s*$')
    if (-not $match.Success) { return $null }
    return [int]$match.Groups[1].Value
}

Capture-Adb 'device.txt' @('shell', 'getprop')
$batteryBeforeText = Invoke-SelectedAdb @('shell', 'dumpsys', 'battery') 2>&1 | Out-String
$batteryBeforeText | Out-File -Encoding utf8 (Join-Path $output 'battery-before.txt')
$powerSourceBefore = Get-PowerSource $batteryBeforeText
$batteryLevelBefore = Get-BatteryLevel $batteryBeforeText
if ($RequireUnplugged -and $powerSourceBefore -ne 'battery') {
    throw "Battery soak requires an unplugged device, but '$DeviceId' reports power source '$powerSourceBefore'."
}
Capture-Adb 'power-before.txt' @('shell', 'dumpsys', 'power')

Invoke-SelectedAdb @('shell', 'dumpsys', 'batterystats', '--reset') | Out-Null
Invoke-SelectedAdb @('shell', 'logcat', '-c') | Out-Null
Invoke-SelectedAdb @('shell', 'monkey', '-p', $Package, '-c', 'android.intent.category.LAUNCHER', '1') | Out-Null
Start-Sleep -Seconds 2
$appPid = (Invoke-SelectedAdb @('shell', 'pidof', $Package) 2>$null | Out-String).Trim()
if (-not $appPid) { throw "Torca process '$Package' did not start on ADB device '$DeviceId'." }
Start-Sleep -Seconds 20
Invoke-SelectedAdb @('shell', 'input', 'keyevent', 'KEYCODE_HOME') | Out-Null
if ($RequireScreenOff) {
    Invoke-SelectedAdb @('shell', 'input', 'keyevent', 'KEYCODE_SLEEP') | Out-Null
    Start-Sleep -Seconds 3
    $powerAtStartText = Invoke-SelectedAdb @('shell', 'dumpsys', 'power') 2>&1 | Out-String
    $powerAtStartText | Out-File -Encoding utf8 (Join-Path $output 'power-start.txt')
    if ($powerAtStartText -notmatch '(?im)mWakefulness=(Dozing|Asleep)') {
        throw "Battery soak requires the screen to be off, but Android did not enter Dozing/Asleep on '$DeviceId'."
    }
} else {
    $powerAtStartText = ''
}

$started = Get-Date
@{
    package = $Package
    deviceId = $DeviceId
    durationMinutes = $DurationMinutes
    requireUnplugged = [bool]$RequireUnplugged
    requireScreenOff = [bool]$RequireScreenOff
    powerSourceBefore = $powerSourceBefore
    batteryLevelBefore = $batteryLevelBefore
    appPid = $appPid
    screenStateAtStart = if ($RequireScreenOff) { 'dozing_or_asleep' } else { 'unspecified' }
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
$processAfterText = Invoke-SelectedAdb @('shell', 'ps', '-A') 2>&1 | Out-String
$processAfterText | Out-File -Encoding utf8 (Join-Path $output 'process-after.txt')
Capture-Adb 'logcat.txt' @('logcat', '-d', '-v', 'threadtime')
$batteryAfterText = Get-Content (Join-Path $output 'battery-after.txt') -Raw
$powerSourceAfter = Get-PowerSource $batteryAfterText
$batteryLevelAfter = Get-BatteryLevel $batteryAfterText
$appRunningAtEnd = $processAfterText -match "(?m)\s$([regex]::Escape($Package))\s*$"

@{
    package = $Package
    deviceId = $DeviceId
    durationMinutes = $DurationMinutes
    requireUnplugged = [bool]$RequireUnplugged
    requireScreenOff = [bool]$RequireScreenOff
    powerSourceBefore = $powerSourceBefore
    powerSourceAfter = $powerSourceAfter
    batteryLevelBefore = $batteryLevelBefore
    batteryLevelAfter = $batteryLevelAfter
    appPid = $appPid
    appRunningAtEnd = [bool]$appRunningAtEnd
    screenStateAtStart = if ($RequireScreenOff) { 'dozing_or_asleep' } else { 'unspecified' }
    startedAt = $started.ToString('o')
    finishedAt = (Get-Date).ToString('o')
    output = (Resolve-Path $output).Path
} | ConvertTo-Json | Out-File -Encoding utf8 (Join-Path $output 'result.json')

Write-Host "Battery soak capture complete: $output"
