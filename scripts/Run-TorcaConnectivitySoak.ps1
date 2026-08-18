[CmdletBinding()]
param(
    [Parameter(Mandatory = $false)][int]$Iterations = 10,
    [Parameter(Mandatory = $false)][int]$SettleSeconds = 15,
    [Parameter(Mandatory = $false)][string]$Package = 'com.torca.torca_app',
    [Parameter(Mandatory = $false)][string]$DeviceId,
    [Parameter(Mandatory = $false)][switch]$ToggleMobileData,
    [Parameter(Mandatory = $false)][string]$OutputRoot
)

$ErrorActionPreference = 'Stop'
if ($Iterations -lt 1) { throw 'Iterations must be at least 1.' }
if ($SettleSeconds -lt 1) { throw 'SettleSeconds must be at least 1.' }
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
$output = Join-Path $OutputRoot "connectivity-$stamp"
New-Item -ItemType Directory -Force -Path $output | Out-Null
$timeline = Join-Path $output 'timeline.csv'
'iteration,action,timestamp' | Out-File -Encoding ascii $timeline

function Mark-Step([int]$Iteration, [string]$Action) {
    $line = '{0},{1},{2}' -f $Iteration, $Action, (Get-Date).ToString('o')
    Add-Content -Encoding ascii -Path $timeline -Value $line
}

function Capture-Adb([string]$Name, [string[]]$Arguments) {
    Invoke-SelectedAdb $Arguments 2>&1 | Out-File -Encoding utf8 (Join-Path $output $Name)
}

Invoke-SelectedAdb @('shell', 'logcat', '-c') | Out-Null
Invoke-SelectedAdb @('shell', 'monkey', '-p', $Package, '-c', 'android.intent.category.LAUNCHER', '1') | Out-Null
Start-Sleep -Seconds 2
$appPid = (Invoke-SelectedAdb @('shell', 'pidof', $Package) 2>$null | Out-String).Trim()
if (-not $appPid) { throw "Torca process '$Package' did not start on ADB device '$DeviceId'." }
Start-Sleep -Seconds $SettleSeconds

for ($iteration = 1; $iteration -le $Iterations; $iteration++) {
    Write-Host "Connectivity soak iteration $iteration/$Iterations"

    Mark-Step $iteration 'wifi_off'
    Invoke-SelectedAdb @('shell', 'svc', 'wifi', 'disable') | Out-Null
    Start-Sleep -Seconds $SettleSeconds
    Capture-Adb ("connectivity-{0:D2}-wifi-off.txt" -f $iteration) @('shell', 'dumpsys', 'connectivity')

    if ($ToggleMobileData) {
        Mark-Step $iteration 'mobile_data_off'
        Invoke-SelectedAdb @('shell', 'svc', 'data', 'disable') | Out-Null
        Start-Sleep -Seconds $SettleSeconds
        Mark-Step $iteration 'mobile_data_on'
        Invoke-SelectedAdb @('shell', 'svc', 'data', 'enable') | Out-Null
        Start-Sleep -Seconds $SettleSeconds
    }

    Mark-Step $iteration 'wifi_on'
    Invoke-SelectedAdb @('shell', 'svc', 'wifi', 'enable') | Out-Null
    Start-Sleep -Seconds $SettleSeconds
    Capture-Adb ("connectivity-{0:D2}-wifi-on.txt" -f $iteration) @('shell', 'dumpsys', 'connectivity')
    Capture-Adb ("services-{0:D2}.txt" -f $iteration) @('shell', 'dumpsys', 'activity', 'services', $Package)
}

Capture-Adb 'process-final.txt' @('shell', 'ps', '-A')
Capture-Adb 'connectivity-final.txt' @('shell', 'dumpsys', 'connectivity')
Capture-Adb 'logcat.txt' @('logcat', '-d', '-v', 'threadtime')

@{
    package = $Package
    deviceId = $DeviceId
    iterations = $Iterations
    settleSeconds = $SettleSeconds
    toggleMobileData = [bool]$ToggleMobileData
    finishedAt = (Get-Date).ToString('o')
    output = (Resolve-Path $output).Path
} | ConvertTo-Json | Out-File -Encoding utf8 (Join-Path $output 'result.json')

Write-Host "Connectivity soak capture complete: $output"
