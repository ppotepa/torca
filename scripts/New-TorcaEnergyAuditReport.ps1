[CmdletBinding()]
param(
    [string]$MeasurementsRoot = '.torca/measurements',
    [string]$Output = '.torca/measurements/ENERGY_AUDIT_EVIDENCE.md',
    [string]$HistoricalBatteryResult = 'artifacts/soak/battery-20260821-175135/result.json',
    [string]$HistoricalBatteryStats = 'artifacts/soak/battery-20260821-175135/batterystats.txt',
    [string]$HistoricalAppUid = 'u0a478',
    [int]$LegacyAndroidLogicalProcessorCount = 0
)

$ErrorActionPreference = 'Stop'
$measurementPath = [IO.Path]::GetFullPath($MeasurementsRoot)
$outputPath = [IO.Path]::GetFullPath($Output)
if (-not (Test-Path -LiteralPath $measurementPath -PathType Container)) {
    throw "Measurements directory does not exist: $measurementPath"
}
New-Item -ItemType Directory -Force -Path (Split-Path -Parent $outputPath) | Out-Null

function Read-JsonFile([string]$Path) {
    try {
        Get-Content -LiteralPath $Path -Raw | ConvertFrom-Json
    } catch {
        Write-Warning "Skipping invalid JSON '$Path': $($_.Exception.Message)"
        $null
    }
}

function Markdown-Cell([object]$Value) {
    if ($null -eq $Value) { return '-' }
    ([string]$Value).Replace('|', '\|').Replace("`r", ' ').Replace("`n", ' ')
}

function Number([object]$Value, [int]$Digits = 2) {
    if ($null -eq $Value) { return '-' }
    ([double]$Value).ToString("F$Digits", [Globalization.CultureInfo]::InvariantCulture)
}

function Percent([object]$Value, [int]$Digits = 2) {
    if ($null -eq $Value) { return '-' }
    "$(Number $Value $Digits)%"
}

$lines = [Collections.Generic.List[string]]::new()
function Add-Line([string]$Value = '') { $lines.Add($Value) | Out-Null }

Add-Line '# Torca energy audit - generated evidence index'
Add-Line
Add-Line "Wygenerowano UTC: $([DateTime]::UtcNow.ToString('yyyy-MM-dd HH:mm:ss'))"
Add-Line
Add-Line 'A value of 100% in `CPU / 1 CPU` means one fully utilized logical processor. `CPU / machine` divides it by the recorded logical processor count when available.'
Add-Line

$desktopThreads = [Collections.Generic.List[object]]::new()
$desktopWindows = [Collections.Generic.List[object]]::new()
$android = [Collections.Generic.List[object]]::new()
foreach ($file in Get-ChildItem -LiteralPath $measurementPath -Filter '*.json' -File -Recurse | Sort-Object FullName) {
    $value = Read-JsonFile $file.FullName
    if ($null -eq $value) { continue }
    if ($null -ne $value.processCpuPercentOfOneLogicalCpu) {
        $desktopThreads.Add([pscustomobject]@{
            file = $file.Name
            duration = $value.durationSeconds
            oneCpu = $value.processCpuPercentOfOneLogicalCpu
            machine = $value.processCpuPercentOfMachine
            processors = $value.logicalProcessorCount
            topShare = $value.topThreadSharePercent
        })
        continue
    }
    if ($null -ne $value.summary -and $null -ne $value.summary.medianPercentOfMachine) {
        $desktopWindows.Add([pscustomobject]@{
            file = $file.Name
            state = $value.requestedWindowState
            measuredMinimized = $value.measuredMinimized
            warmup = $value.warmupSeconds
            duration = $value.durationSeconds
            medianOne = $value.summary.medianPercentOfOneLogicalCpu
            p95One = $value.summary.p95PercentOfOneLogicalCpu
            medianMachine = $value.summary.medianPercentOfMachine
            p95Machine = $value.summary.p95PercentOfMachine
        })
        continue
    }
    if ($null -ne $value.summary -and $null -ne $value.summary.medianPercentOfOneLogicalCpu -and $null -ne $value.provider) {
        $stateCheck = if ($value.mode -eq 'foreground' -and $value.screenOff) {
            'invalid: screen off'
        } elseif ($value.mode -eq 'background' -and -not $value.screenOff) {
            'invalid: screen on'
        } else {
            'ok'
        }
        $processorCount = $value.logicalProcessorCount
        if ($null -eq $processorCount -and $LegacyAndroidLogicalProcessorCount -gt 0) {
            $processorCount = $LegacyAndroidLogicalProcessorCount
        }
        $deviceMedian = $value.summary.medianPercentOfTotalLogicalCapacity
        if ($null -eq $deviceMedian -and $processorCount -gt 0) {
            $deviceMedian = [double]$value.summary.medianPercentOfOneLogicalCpu / [double]$processorCount
        }
        $android.Add([pscustomobject]@{
            file = $file.Name
            build = if ($file.Name -like '*release*') { 'release' } else { 'debug' }
            provider = $value.provider
            profile = $value.profile
            mode = $value.mode
            duration = $value.durationSeconds
            median = $value.summary.medianPercentOfOneLogicalCpu
            p95 = $value.summary.p95PercentOfOneLogicalCpu
            processors = $processorCount
            deviceMedian = $deviceMedian
            batteryLevelDelta = $value.batteryDelta.levelPercentagePoints
            batteryChargeDeltaMah = $value.batteryDelta.chargeCounterMah
            currentBeforeMa = $value.batteryDelta.currentMilliampBefore
            currentAfterMa = $value.batteryDelta.currentMilliampAfter
            temperatureBeforeC = $value.batteryDelta.temperatureCelsiusBefore
            temperatureAfterC = $value.batteryDelta.temperatureCelsiusAfter
            stateCheck = $stateCheck
            power = $value.powerSource
        })
    }
}

Add-Line '## Desktop - thread profiles'
Add-Line
Add-Line '| File | Duration [s] | CPU / 1 CPU | CPU / machine | Logical CPUs | Hottest thread share |'
Add-Line '| --- | ---: | ---: | ---: | ---: | ---: |'
foreach ($row in $desktopThreads) {
    Add-Line "| $(Markdown-Cell $row.file) | $(Number $row.duration 1) | $(Percent $row.oneCpu) | $(Percent $row.machine) | $(Markdown-Cell $row.processors) | $(Percent $row.topShare) |"
}
if ($desktopThreads.Count -eq 0) { Add-Line '| no data | - | - | - | - | - |' }
Add-Line

Add-Line '## Desktop - window state'
Add-Line
Add-Line '| File | State | Warm-up [s] | Median / 1 CPU | P95 / 1 CPU | Median / machine | P95 / machine |'
Add-Line '| --- | --- | ---: | ---: | ---: | ---: | ---: |'
foreach ($row in $desktopWindows) {
    Add-Line "| $(Markdown-Cell $row.file) | $(Markdown-Cell $row.state) | $(Markdown-Cell $row.warmup) | $(Percent $row.medianOne) | $(Percent $row.p95One) | $(Percent $row.medianMachine) | $(Percent $row.p95Machine) |"
}
if ($desktopWindows.Count -eq 0) { Add-Line '| no data | - | - | - | - | - | - |' }
Add-Line

Add-Line '## Android - process CPU'
Add-Line
Add-Line '| File | Build | Provider/profile | Mode | Median CPU / 1 CPU | P95 | CPU / device | Battery Δ% | Charge ΔmAh | Current mA (before→after) | Temp °C (before→after) | State check | Power |'
Add-Line '| --- | --- | --- | --- | ---: | ---: | ---: | ---: | ---: | --- | --- | --- | --- |'
foreach ($row in $android) {
    $current = "$(Markdown-Cell $row.currentBeforeMa) → $(Markdown-Cell $row.currentAfterMa)"
    $temperature = "$(Markdown-Cell $row.temperatureBeforeC) → $(Markdown-Cell $row.temperatureAfterC)"
    Add-Line "| $(Markdown-Cell $row.file) | $(Markdown-Cell $row.build) | $(Markdown-Cell "$($row.provider)/$($row.profile)") | $(Markdown-Cell $row.mode) | $(Percent $row.median) | $(Percent $row.p95) | $(Percent $row.deviceMedian) | $(Markdown-Cell $row.batteryLevelDelta) | $(Markdown-Cell $row.batteryChargeDeltaMah) | $current | $temperature | $(Markdown-Cell $row.stateCheck) | $(Markdown-Cell $row.power) |"
}
if ($android.Count -eq 0) { Add-Line '| no data | - | - | - | - | - | - | - | - | - | - | - |' }
Add-Line
Add-Line '> Externally powered emulator measurements are CPU/thread evidence, not mAh or real handset battery-life evidence.'
Add-Line

$historicalResultPath = [IO.Path]::GetFullPath($HistoricalBatteryResult)
$historicalStatsPath = [IO.Path]::GetFullPath($HistoricalBatteryStats)
if ((Test-Path -LiteralPath $historicalResultPath) -and (Test-Path -LiteralPath $historicalStatsPath)) {
    $result = Read-JsonFile $historicalResultPath
    $stats = Get-Content -LiteralPath $historicalStatsPath -Raw
    $capacity = [regex]::Match($stats, 'Capacity:\s*([\d.]+),\s*Computed drain:\s*([\d.]+),\s*actual drain:\s*([\d.]+)')
    $uid = [regex]::Match($stats, "UID\s+$([regex]::Escape($HistoricalAppUid)):\s*([\d.]+)[^\r\n]*")
    $uidDetail = [regex]::Match($stats, "UID\s+$([regex]::Escape($HistoricalAppUid)):[^\r\n]*\r?\n\s*([^\r\n]+)")
    Add-Line '## Historical physical Android soak'
    Add-Line
    Add-Line '| Duration | Battery before | Battery after | Drop | Capacity | Actual drain | App estimate |'
    Add-Line '| ---: | ---: | ---: | ---: | ---: | ---: | ---: |'
    $drop = [int]$result.batteryLevelBefore - [int]$result.batteryLevelAfter
    Add-Line "| $($result.durationMinutes) min | $($result.batteryLevelBefore)% | $($result.batteryLevelAfter)% | $drop pp | $(if ($capacity.Success) { "$($capacity.Groups[1].Value) mAh" } else { '-' }) | $(if ($capacity.Success) { "$($capacity.Groups[3].Value) mAh" } else { '-' }) | $(if ($uid.Success) { "$($uid.Groups[1].Value) mAh" } else { '-' }) |"
    Add-Line
    if ($uidDetail.Success) {
        Add-Line ('UID attribution: `{0}`.' -f (Markdown-Cell $uidDetail.Groups[1].Value.Trim()))
        Add-Line
    }
    Add-Line '> This historical run used an older Tor debug build, not current Iroh. It demonstrates physical-device impact but is not a provider comparison.'
    Add-Line
}

Add-Line '## Preserved profiler traces'
Add-Line
foreach ($file in Get-ChildItem -LiteralPath $measurementPath -File | Where-Object { $_.Extension -in '.data', '.csv', '.perfetto-trace' } | Sort-Object Name) {
    Add-Line ('- `{0}` - {1} MiB' -f (Markdown-Cell $file.Name), [Math]::Round($file.Length / 1MB, 2))
}
if (-not (Get-ChildItem -LiteralPath $measurementPath -File | Where-Object { $_.Extension -in '.data', '.csv', '.perfetto-trace' })) {
    Add-Line '- no profiler traces'
}

$lines | Set-Content -LiteralPath $outputPath -Encoding utf8
Write-Host "Markdown evidence report: $outputPath"
