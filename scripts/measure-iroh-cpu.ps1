[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$ProcessName,
    [int]$DurationSeconds = 300,
    [double]$SampleSeconds = 1.0,
    [string]$Provider = 'Iroh',
    [string]$Profile = 'direct',
    [ValidateSet('foreground', 'background')]
    [string]$Mode = 'foreground',
    [string]$Output = '.torca/measurements/iroh-cpu.json',
    [double]$ForegroundLimit = 1.0,
    [double]$BackgroundLimit = 0.25
)

$ErrorActionPreference = 'Stop'
if ($DurationSeconds -lt 5) { throw 'DurationSeconds must be at least 5 seconds.' }
if ($SampleSeconds -le 0) { throw 'SampleSeconds must be greater than zero.' }

$outputPath = [IO.Path]::GetFullPath($Output)
New-Item -ItemType Directory -Force -Path (Split-Path -Parent $outputPath) | Out-Null

function Get-ProcessSample {
    param([string]$Name)
    $processes = @(Get-Process -Name $Name -ErrorAction SilentlyContinue)
    if ($processes.Count -eq 0) { return $null }
    [pscustomobject]@{
        ProcessCount = $processes.Count
        CpuSeconds = [double](($processes | Measure-Object -Property CPU -Sum).Sum)
        WorkingSetBytes = [int64](($processes | Measure-Object -Property WorkingSet64 -Sum).Sum)
    }
}

$startedAt = [DateTime]::UtcNow
$samples = [Collections.Generic.List[object]]::new()
$previous = Get-ProcessSample -Name $ProcessName
if ($null -eq $previous) {
    throw "No process named '$ProcessName' was found. Start the selected Torca artifact first."
}

$stopwatch = [Diagnostics.Stopwatch]::StartNew()
$previousElapsed = 0.0
while ($stopwatch.Elapsed.TotalSeconds -lt $DurationSeconds) {
    Start-Sleep -Seconds $SampleSeconds
    $current = Get-ProcessSample -Name $ProcessName
    if ($null -eq $current) { throw "Process '$ProcessName' exited during measurement." }
    $elapsed = $stopwatch.Elapsed.TotalSeconds
    $cpuDelta = [Math]::Max(0.0, $current.CpuSeconds - $previous.CpuSeconds)
    $wallDelta = [Math]::Max(0.001, $elapsed - $previousElapsed)
    $cpuPercent = 100.0 * $cpuDelta / $wallDelta
    $samples.Add([pscustomobject]@{
        ElapsedSeconds = [Math]::Round($elapsed, 3)
        CpuPercentOfOneLogicalCpu = [Math]::Round($cpuPercent, 4)
        ProcessCount = $current.ProcessCount
        WorkingSetBytes = $current.WorkingSetBytes
    })
    $previous = $current
    $previousElapsed = $elapsed
}
$stopwatch.Stop()

$values = @($samples | ForEach-Object { [double]$_.CpuPercentOfOneLogicalCpu } | Sort-Object)
$medianIndex = [int][Math]::Floor(($values.Count - 1) / 2)
$p95Index = [int][Math]::Min($values.Count - 1, [Math]::Floor($values.Count * 0.95))
$median = $values[$medianIndex]
$p95 = $values[$p95Index]
$maximum = ($values | Measure-Object -Maximum).Maximum
$limit = if ($Mode -eq 'background') { $BackgroundLimit } else { $ForegroundLimit }
$hotSamples = @($values | Where-Object { $_ -gt 0.1 }).Count
$longestHotRun = 0
$currentHotRun = 0
foreach ($sample in $samples) {
    if ($sample.CpuPercentOfOneLogicalCpu -gt 0.1) {
        $currentHotRun++
        $longestHotRun = [Math]::Max($longestHotRun, $currentHotRun)
    } else {
        $currentHotRun = 0
    }
}
$persistentHotLimit = 5
$passed = ($median -lt $limit) -and ($longestHotRun -lt $persistentHotLimit)

$report = [ordered]@{
    schema = 1
    startedAtUtc = $startedAt.ToString('o')
    finishedAtUtc = [DateTime]::UtcNow.ToString('o')
    provider = $Provider
    profile = $Profile
    mode = $Mode
    processName = $ProcessName
    durationSeconds = [Math]::Round($stopwatch.Elapsed.TotalSeconds, 3)
    sampleSeconds = $SampleSeconds
    logicalCpuNormalization = 'one logical CPU = 100 percent'
    thresholds = [ordered]@{
        medianPercent = $limit
        noPersistentSampleAbovePercent = 0.1
    }
    summary = [ordered]@{
        sampleCount = $values.Count
        medianPercent = [Math]::Round($median, 4)
        p95Percent = [Math]::Round($p95, 4)
        maximumPercent = [Math]::Round($maximum, 4)
        samplesAbovePointOnePercent = $hotSamples
        longestConsecutiveHotRun = $longestHotRun
        persistentHotRunLimit = $persistentHotLimit
        verdict = if ($passed) { 'pass' } else { 'fail' }
    }
    samples = @($samples)
}

$report | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $outputPath -Encoding utf8
Write-Host ("CPU measurement: {0} (median={1}% p95={2}% max={3}%)" -f $report.summary.verdict, $report.summary.medianPercent, $report.summary.p95Percent, $report.summary.maximumPercent)
Write-Host "Report: $outputPath"
if (-not $passed) { exit 2 }
