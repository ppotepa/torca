[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$AndroidSerial,
    [string]$Package = 'com.torca.torca_app',
    [int]$DurationSeconds = 30,
    [string]$Provider = 'iroh',
    [string]$Profile = 'always',
    [ValidateSet('foreground', 'background')]
    [string]$Mode = 'foreground',
    [string]$Output = '.torca/measurements/android-cpu.json'
)

$ErrorActionPreference = 'Stop'
if ($DurationSeconds -lt 5) { throw 'DurationSeconds must be at least 5 seconds.' }
$adb = Get-Command adb.exe -ErrorAction SilentlyContinue
if ($null -eq $adb) { throw 'adb.exe is required.' }

& $adb.Source '-s' $AndroidSerial 'get-state' | Out-Null
if ($LASTEXITCODE -ne 0) { throw "Android device is not ready: $AndroidSerial" }
$pidText = (& $adb.Source '-s' $AndroidSerial 'shell' 'pidof' $Package 2>$null | Out-String).Trim()
$pids = @($pidText -split '\s+' | Where-Object { $_ -match '^\d+$' })
if ($pids.Count -ne 1) {
    throw "Expected exactly one Android process for '$Package'; found '$pidText'."
}
$appPid = [int]$pids[0]
$processorCountText = (& $adb.Source '-s' $AndroidSerial 'shell' 'getconf' '_NPROCESSORS_ONLN' 2>$null | Out-String).Trim()
$logicalProcessorCount = if ($processorCountText -match '^\d+$' -and [int]$processorCountText -gt 0) {
    [int]$processorCountText
} else {
    1
}
$outputPath = [IO.Path]::GetFullPath($Output)
New-Item -ItemType Directory -Force -Path (Split-Path -Parent $outputPath) | Out-Null

function Get-BatterySnapshot {
    $text = (& $adb.Source '-s' $AndroidSerial 'shell' 'dumpsys' 'battery' 2>&1 | Out-String)
    $levelMatch = [regex]::Match($text, '(?im)^\s*level:\s*(\d+)\s*$')
    $chargeMatch = [regex]::Match($text, '(?im)^\s*(?:charge counter|charge_counter):\s*(-?\d+)\s*$')
    $currentMatch = [regex]::Match($text, '(?im)^\s*(?:current now|current_now):\s*(-?\d+)\s*$')
    $temperatureMatch = [regex]::Match($text, '(?im)^\s*temperature:\s*(-?\d+)\s*$')
    [pscustomobject]@{
        levelPercent = if ($levelMatch.Success) { [int]$levelMatch.Groups[1].Value } else { $null }
        chargeCounterMah = if ($chargeMatch.Success) { [double]$chargeMatch.Groups[1].Value / 1000.0 } else { $null }
        currentMilliamp = if ($currentMatch.Success) { [double]$currentMatch.Groups[1].Value / 1000.0 } else { $null }
        temperatureCelsius = if ($temperatureMatch.Success) { [double]$temperatureMatch.Groups[1].Value / 10.0 } else { $null }
    }
}

$batteryBefore = Get-BatterySnapshot
$startedAt = [DateTime]::UtcNow
$topLines = @(& $adb.Source '-s' $AndroidSerial 'shell' 'top' '-b' '-q' '-d' '1' '-n' ([string]$DurationSeconds) '-p' ([string]$appPid) '-o' 'PID,%CPU,RES,CMDLINE' 2>&1)
if ($LASTEXITCODE -ne 0) { throw "Android top failed for '$Package'." }
$samples = [Collections.Generic.List[object]]::new()
foreach ($line in $topLines) {
    $match = [regex]::Match([string]$line, '^\s*(\d+)\s+([\d.]+)\s+(\S+)\s+(.+?)\s*$')
    if (-not $match.Success -or [int]$match.Groups[1].Value -ne $appPid) { continue }
    $samples.Add([pscustomobject]@{
        elapsedSeconds = $samples.Count + 1
        cpuPercentOfOneLogicalCpu = [double]::Parse(
            $match.Groups[2].Value,
            [Globalization.CultureInfo]::InvariantCulture
        )
        residentMemory = $match.Groups[3].Value
    })
}
if ($samples.Count -eq 0) { throw "Android top returned no samples for '$Package'." }

$values = @($samples | ForEach-Object { [double]$_.cpuPercentOfOneLogicalCpu } | Sort-Object)
$medianIndex = [int][Math]::Floor(($values.Count - 1) / 2)
$p95Index = [int][Math]::Min($values.Count - 1, [Math]::Floor($values.Count * 0.95))
$threadLines = @(& $adb.Source '-s' $AndroidSerial 'shell' 'top' '-H' '-b' '-q' '-n' '1' '-p' ([string]$appPid) '-m' '15' '-o' 'TID,%CPU,RES,CMDLINE' 2>&1)
$threads = [Collections.Generic.List[object]]::new()
foreach ($line in $threadLines) {
    $match = [regex]::Match([string]$line, '^\s*(\d+)\s+([\d.]+)\s+(\S+)\s+(.+?)\s*$')
    if (-not $match.Success) { continue }
    $threadId = [int]$match.Groups[1].Value
    $threadName = (& $adb.Source '-s' $AndroidSerial 'shell' 'cat' "/proc/$appPid/task/$threadId/comm" 2>$null | Out-String).Trim()
    $threads.Add([pscustomobject]@{
        threadId = $threadId
        cpuPercentOfOneLogicalCpu = [double]::Parse(
            $match.Groups[2].Value,
            [Globalization.CultureInfo]::InvariantCulture
        )
        residentMemory = $match.Groups[3].Value
        name = if ([string]::IsNullOrWhiteSpace($threadName)) { $match.Groups[4].Value } else { $threadName }
    })
}
$battery = (& $adb.Source '-s' $AndroidSerial 'shell' 'dumpsys' 'battery' 2>&1 | Out-String)
$batteryAfter = Get-BatterySnapshot
$power = (& $adb.Source '-s' $AndroidSerial 'shell' 'dumpsys' 'power' 2>&1 | Out-String)
$screenOff = $power -match '(?im)mWakefulness=(Dozing|Asleep)'
$powerSource = if ($battery -match '(?im)^\s*(AC|USB|Wireless) powered:\s*true\s*$') { 'external' } else { 'battery' }
$stateWarnings = [Collections.Generic.List[string]]::new()
if ($Mode -eq 'background' -and -not $screenOff) {
    $stateWarnings.Add('background measurement requires screen-off device state')
}
if ($Mode -eq 'foreground' -and $screenOff) {
    $stateWarnings.Add('foreground measurement unexpectedly ran with screen off')
}
if ($powerSource -eq 'external') {
    $stateWarnings.Add('device is externally powered; charge drain is not representative')
}
$medianPercent = [Math]::Round($values[$medianIndex], 4)
$p95Percent = [Math]::Round($values[$p95Index], 4)
$maximumPercent = [Math]::Round(($values | Measure-Object -Maximum).Maximum, 4)
$report = [ordered]@{
    schema = 2
    startedAtUtc = $startedAt.ToString('o')
    finishedAtUtc = [DateTime]::UtcNow.ToString('o')
    serial = $AndroidSerial
    package = $Package
    processId = $appPid
    provider = $Provider
    profile = $Profile
    mode = $Mode
    durationSeconds = $DurationSeconds
    logicalProcessorCount = $logicalProcessorCount
    cpuNormalization = 'Android top: one logical CPU = 100 percent; a multi-threaded process may exceed 100 percent'
    screenOff = $screenOff
    powerSource = $powerSource
    validMeasurement = ($stateWarnings.Count -eq 0)
    warnings = @($stateWarnings)
    batteryBefore = $batteryBefore
    batteryAfter = $batteryAfter
    batteryDelta = [ordered]@{
        levelPercentagePoints = if ($null -ne $batteryBefore.levelPercent -and $null -ne $batteryAfter.levelPercent) {
            $batteryAfter.levelPercent - $batteryBefore.levelPercent
        } else { $null }
        chargeCounterMah = if ($null -ne $batteryBefore.chargeCounterMah -and $null -ne $batteryAfter.chargeCounterMah) {
            $batteryAfter.chargeCounterMah - $batteryBefore.chargeCounterMah
        } else { $null }
        currentMilliampBefore = $batteryBefore.currentMilliamp
        currentMilliampAfter = $batteryAfter.currentMilliamp
        temperatureCelsiusBefore = $batteryBefore.temperatureCelsius
        temperatureCelsiusAfter = $batteryAfter.temperatureCelsius
    }
    summary = [ordered]@{
        sampleCount = $values.Count
        medianPercentOfOneLogicalCpu = $medianPercent
        p95PercentOfOneLogicalCpu = $p95Percent
        maximumPercentOfOneLogicalCpu = $maximumPercent
        medianPercentOfTotalLogicalCapacity = [Math]::Round($medianPercent / $logicalProcessorCount, 4)
        p95PercentOfTotalLogicalCapacity = [Math]::Round($p95Percent / $logicalProcessorCount, 4)
    }
    samples = @($samples)
    hottestThreads = @($threads | Sort-Object cpuPercentOfOneLogicalCpu -Descending | Select-Object -First 10)
}
$report | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $outputPath -Encoding utf8
Write-Host ("Android CPU measurement: {0}/{1} {2}, median={3}% p95={4}%" -f `
    $Provider,
    $Profile,
    $Mode,
    $report.summary.medianPercentOfOneLogicalCpu,
    $report.summary.p95PercentOfOneLogicalCpu)
Write-Host "Report: $outputPath"
