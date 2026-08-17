[CmdletBinding()]
param(
    [Parameter(Mandatory = $false)][int]$Iterations = 10,
    [Parameter(Mandatory = $false)][int]$SettleSeconds = 15,
    [Parameter(Mandatory = $false)][string]$Package = 'com.torca.torca_app',
    [Parameter(Mandatory = $false)][switch]$ToggleMobileData,
    [Parameter(Mandatory = $false)][string]$OutputRoot = (Join-Path $PSScriptRoot '../artifacts/soak')
)

$ErrorActionPreference = 'Stop'
if ($Iterations -lt 1) { throw 'Iterations must be at least 1.' }
if ($SettleSeconds -lt 1) { throw 'SettleSeconds must be at least 1.' }

& adb get-state | Out-Null
if ($LASTEXITCODE -ne 0) { throw 'No ADB device is available.' }

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
    & adb @Arguments 2>&1 | Out-File -Encoding utf8 (Join-Path $output $Name)
}

& adb shell logcat -c | Out-Null
& adb shell monkey -p $Package -c android.intent.category.LAUNCHER 1 | Out-Null
Start-Sleep -Seconds $SettleSeconds

for ($iteration = 1; $iteration -le $Iterations; $iteration++) {
    Write-Host "Connectivity soak iteration $iteration/$Iterations"

    Mark-Step $iteration 'wifi_off'
    & adb shell svc wifi disable | Out-Null
    Start-Sleep -Seconds $SettleSeconds
    Capture-Adb ("connectivity-{0:D2}-wifi-off.txt" -f $iteration) @('shell', 'dumpsys', 'connectivity')

    if ($ToggleMobileData) {
        Mark-Step $iteration 'mobile_data_off'
        & adb shell svc data disable | Out-Null
        Start-Sleep -Seconds $SettleSeconds
        Mark-Step $iteration 'mobile_data_on'
        & adb shell svc data enable | Out-Null
        Start-Sleep -Seconds $SettleSeconds
    }

    Mark-Step $iteration 'wifi_on'
    & adb shell svc wifi enable | Out-Null
    Start-Sleep -Seconds $SettleSeconds
    Capture-Adb ("connectivity-{0:D2}-wifi-on.txt" -f $iteration) @('shell', 'dumpsys', 'connectivity')
    Capture-Adb ("services-{0:D2}.txt" -f $iteration) @('shell', 'dumpsys', 'activity', 'services', $Package)
}

Capture-Adb 'process-final.txt' @('shell', 'ps', '-A')
Capture-Adb 'connectivity-final.txt' @('shell', 'dumpsys', 'connectivity')
Capture-Adb 'logcat.txt' @('logcat', '-d', '-v', 'threadtime')

@{
    package = $Package
    iterations = $Iterations
    settleSeconds = $SettleSeconds
    toggleMobileData = [bool]$ToggleMobileData
    finishedAt = (Get-Date).ToString('o')
    output = (Resolve-Path $output).Path
} | ConvertTo-Json | Out-File -Encoding utf8 (Join-Path $output 'result.json')

Write-Host "Connectivity soak capture complete: $output"
