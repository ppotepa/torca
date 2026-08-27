[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidateSet('desktop', 'android')]
    [string]$Platform,
    [string]$ProcessName = 'torca_app',
    [string]$ExecutablePath,
    [switch]$LaunchIfMissing,
    [string]$AndroidSerial,
    [string]$Package = 'com.torca.torca_app',
    [string]$Provider = 'iroh',
    [ValidateSet('always', 'direct', 'local')]
    [string]$Profile = 'always',
    [ValidateSet('foreground', 'background')]
    [string]$Mode = 'background',
    [ValidateSet('visible', 'minimized')]
    [string]$WindowState = 'minimized',
    [int]$DurationSeconds = 30,
    [int]$Repetitions = 3,
    [string]$Output = '.torca/measurements/energy-gate.json',
    [switch]$RequireBatteryTelemetry,
    [switch]$FailOnRegression
)

$ErrorActionPreference = 'Stop'
if ($DurationSeconds -lt 5) { throw 'DurationSeconds must be at least 5 seconds.' }
if ($Repetitions -lt 1) { throw 'Repetitions must be positive.' }
if ($Platform -eq 'android' -and [string]::IsNullOrWhiteSpace($AndroidSerial)) {
    throw '-AndroidSerial is required for Android measurements.'
}

$root = [IO.Path]::GetFullPath((Split-Path -Parent $Output))
New-Item -ItemType Directory -Force -Path $root | Out-Null
$rows = [System.Collections.Generic.List[object]]::new()
$measurementWarnings = [System.Collections.Generic.List[string]]::new()
$repoRoot = (Get-Location).Path

function Get-Median([double[]]$Values) {
    $ordered = @($Values | Sort-Object)
    return $ordered[[int][Math]::Floor(($ordered.Count - 1) / 2)]
}

for ($index = 1; $index -le $Repetitions; $index++) {
    $reportPath = Join-Path $root ("{0}-{1}-{2}.json" -f $Platform, $Profile, $index)
    if ($Platform -eq 'desktop') {
        $measurementWindowState = if ($WindowState -eq 'visible') { 'current' } else { 'minimized' }
        & (Join-Path $repoRoot 'scripts/measure-desktop-window-cpu.ps1') `
            -ProcessName $ProcessName -WindowState $measurementWindowState `
            -ExecutablePath $ExecutablePath -LaunchIfMissing:$LaunchIfMissing `
            -DurationSeconds $DurationSeconds -Output $reportPath
    } else {
        & (Join-Path $repoRoot 'scripts/measure-android-process-cpu.ps1') `
            -AndroidSerial $AndroidSerial -Package $Package `
            -DurationSeconds $DurationSeconds -Provider $Provider `
            -Profile $Profile -Mode $Mode -Output $reportPath
    }
    # A child PowerShell script can complete successfully without setting
    # `$LASTEXITCODE`; only treat an explicitly non-zero native exit as a
    # failed measurement.
    if ($null -ne $LASTEXITCODE -and $LASTEXITCODE -ne 0) {
        throw "measurement failed for repetition $index"
    }
    $report = Get-Content -Raw -Encoding UTF8 -LiteralPath $reportPath | ConvertFrom-Json
    $rows.Add($report)
    if ($null -ne $report.validMeasurement -and -not $report.validMeasurement) {
        $measurementWarnings.Add("${reportPath}: $(@($report.warnings) -join '; ')")
    }
    if ($Platform -eq 'android' -and $RequireBatteryTelemetry) {
        $batteryDelta = $report.batteryDelta
        $missing = @(
            if ($null -eq $batteryDelta -or $null -eq $batteryDelta.chargeCounterMah) { 'charge-counter' }
            if ($null -eq $batteryDelta -or $null -eq $batteryDelta.currentMilliampBefore -or $null -eq $batteryDelta.currentMilliampAfter) { 'current-now' }
            if ($null -eq $batteryDelta -or $null -eq $batteryDelta.temperatureCelsiusBefore -or $null -eq $batteryDelta.temperatureCelsiusAfter) { 'temperature' }
        )
        if ($missing.Count -gt 0) {
            $measurementWarnings.Add("${reportPath}: required battery telemetry missing ($($missing -join ', '))")
        }
    }
}

$values = if ($Platform -eq 'desktop') {
    @($rows | ForEach-Object { [double]$_.summary.medianPercentOfMachine })
} else {
    @($rows | ForEach-Object { [double]$_.summary.medianPercentOfOneLogicalCpu })
}
$median = Get-Median $values
$p95 = [double](@($rows | ForEach-Object {
    if ($Platform -eq 'desktop') { $_.summary.p95PercentOfMachine }
    else { $_.summary.p95PercentOfOneLogicalCpu }
} | Sort-Object)[[int][Math]::Floor(($rows.Count - 1) / 2)])
$limit = if ($Platform -eq 'desktop') {
    if ($WindowState -eq 'minimized') { 0.25 } else { 0.5 }
} else { 1.0 }
$passed = ($median -le $limit) -and ($measurementWarnings.Count -eq 0)

$result = [ordered]@{
    schemaVersion = 1
    generatedAtUtc = [DateTime]::UtcNow.ToString('o')
    platform = $Platform
    provider = $Provider
    profile = $Profile
    mode = if ($Platform -eq 'desktop') { $WindowState } else { $Mode }
    durationSeconds = $DurationSeconds
    repetitions = $Repetitions
    metric = if ($Platform -eq 'desktop') { 'medianPercentOfMachine' } else { 'medianPercentOfOneLogicalCpu' }
    median = [Math]::Round($median, 4)
    p95 = [Math]::Round($p95, 4)
    limit = $limit
    passed = $passed
    warnings = @($measurementWarnings)
    reports = @($rows)
}
$result | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath $Output -Encoding utf8
Write-Host ("Energy gate: {0}/{1} median={2}% limit={3}% passed={4}" -f $Platform, $Profile, $result.median, $limit, $passed)
Write-Host "Report: $([IO.Path]::GetFullPath($Output))"
if ($FailOnRegression -and -not $passed) { exit 2 }
