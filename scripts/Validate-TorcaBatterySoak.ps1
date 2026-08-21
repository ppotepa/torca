[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$Path,
    [Parameter(Mandatory = $false)][int]$MinimumMinutes = 60,
    [Parameter(Mandatory = $false)][switch]$RequireNativeDiagnostics
)

$ErrorActionPreference = 'Stop'
if ($MinimumMinutes -lt 1) { throw 'MinimumMinutes must be at least 1.' }

$resolved = Resolve-Path -LiteralPath $Path -ErrorAction Stop
$resultPath = Join-Path $resolved 'result.json'
if (-not (Test-Path -LiteralPath $resultPath -PathType Leaf)) {
    throw "Battery soak result is missing: $resultPath"
}

$result = Get-Content -LiteralPath $resultPath -Raw | ConvertFrom-Json
$failures = [System.Collections.Generic.List[string]]::new()

if ([int]$result.durationMinutes -lt $MinimumMinutes) {
    $failures.Add("durationMinutes=$($result.durationMinutes), minimum=$MinimumMinutes")
}
if (-not [bool]$result.requireUnplugged) {
    $failures.Add('result was not run with -RequireUnplugged')
}
if (-not [bool]$result.requireScreenOff) {
    $failures.Add('result was not run with -RequireScreenOff')
}
if ($result.powerSourceBefore -ne 'battery' -or $result.powerSourceAfter -ne 'battery') {
    $failures.Add("power source was before=$($result.powerSourceBefore), after=$($result.powerSourceAfter)")
}
if (-not [bool]$result.appRunningAtEnd) {
    $failures.Add('Torca process was not present at the end of the measured window')
}
if ($result.screenStateAtStart -ne 'dozing_or_asleep') {
    $failures.Add("screenStateAtStart=$($result.screenStateAtStart)")
}

if ($RequireNativeDiagnostics -and -not [bool]$result.nativeDiagnosticsAfterCollected) {
    $failures.Add('native diagnostics were not collected after the soak')
}

$powerStartPath = Join-Path $resolved 'power-start.txt'
if (-not (Test-Path -LiteralPath $powerStartPath -PathType Leaf)) {
    $failures.Add('power-start.txt is missing')
} elseif ((Get-Content -LiteralPath $powerStartPath -Raw) -notmatch '(?im)mWakefulness=(Dozing|Asleep)') {
    $failures.Add('power-start.txt does not prove Dozing/Asleep')
}

if ($failures.Count -gt 0) {
    throw "Battery soak validation failed:`n - $($failures -join "`n - ")"
}

if ($RequireNativeDiagnostics -and [bool]$result.nativeDiagnosticsAfterCollected) {
    $nativeLogs = Get-ChildItem -LiteralPath (Join-Path $resolved 'native-after') -Recurse -File -Include '*.log','*.json' -ErrorAction SilentlyContinue
    $nativeText = ($nativeLogs | ForEach-Object { Get-Content -LiteralPath $_.FullName -Raw -ErrorAction SilentlyContinue }) -join "`n"
    if ($nativeText -match 'BACKGROUND_RENDEZVOUS|BACKGROUND_SYNC_LEASE') {
        $failures.Add('native diagnostics contain a periodic background rendezvous/lease marker')
    }
    if ($nativeText -notmatch 'BACKGROUND_GRACE_EXPIRED') {
        $failures.Add('native diagnostics do not prove background grace expiry')
    }
}

if ($failures.Count -gt 0) {
    throw "Battery soak validation failed:`n - $($failures -join "`n - ")"
}

$delta = if ($null -ne $result.batteryLevelBefore -and $null -ne $result.batteryLevelAfter) {
    [int]$result.batteryLevelBefore - [int]$result.batteryLevelAfter
} else {
    'unknown'
}

Write-Host "Battery soak valid: duration=$($result.durationMinutes)m device=$($result.deviceId) battery_delta=$delta% process_alive=$($result.appRunningAtEnd)"
