[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$ProcessName,
    [string]$ExecutablePath,
    [switch]$LaunchIfMissing,
    [ValidateSet('current', 'minimized')]
    [string]$WindowState = 'current',
    [int]$WarmupSeconds = 2,
    [int]$DurationSeconds = 30,
    [double]$SampleSeconds = 1.0,
    [string]$Output = '.torca/measurements/desktop-window-cpu.json'
)

$ErrorActionPreference = 'Stop'
if ($DurationSeconds -lt 5) { throw 'DurationSeconds must be at least 5 seconds.' }
if ($WarmupSeconds -lt 0) { throw 'WarmupSeconds must not be negative.' }
if ($SampleSeconds -le 0) { throw 'SampleSeconds must be greater than zero.' }

$launchedProcess = $null
$processes = @(Get-Process -Name $ProcessName -ErrorAction SilentlyContinue)
try {
    if ($processes.Count -eq 0 -and $LaunchIfMissing) {
        if ([string]::IsNullOrWhiteSpace($ExecutablePath)) {
            throw '-ExecutablePath is required with -LaunchIfMissing.'
        }
        $resolvedExecutable = (Resolve-Path -LiteralPath $ExecutablePath -ErrorAction Stop).Path
        $launchedProcess = Start-Process -FilePath $resolvedExecutable -PassThru
        $deadline = [DateTime]::UtcNow.AddSeconds(30)
        do {
            Start-Sleep -Milliseconds 250
            $launchedProcess.Refresh()
            if ($launchedProcess.HasExited) {
                throw "Launched process exited before creating a main window: $resolvedExecutable"
            }
        } while ($launchedProcess.MainWindowHandle -eq [IntPtr]::Zero -and [DateTime]::UtcNow -lt $deadline)
        if ($launchedProcess.MainWindowHandle -eq [IntPtr]::Zero) {
            throw "Timed out waiting for a main window from: $resolvedExecutable"
        }
        $processes = @($launchedProcess)
    }
} catch {
    if ($null -ne $launchedProcess -and -not $launchedProcess.HasExited) {
        $launchedProcess.CloseMainWindow() | Out-Null
        if (-not $launchedProcess.WaitForExit(5000)) { $launchedProcess.Kill() }
    }
    throw
}
if ($processes.Count -ne 1) {
    throw "Expected exactly one process named '$ProcessName'; found $($processes.Count)."
}
$process = $processes[0]
if ($process.MainWindowHandle -eq [IntPtr]::Zero) {
    throw "Process '$ProcessName' has no main window."
}

if (-not ('Torca.WindowInspector' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;

namespace Torca {
    public static class WindowInspector {
        [DllImport("user32.dll")]
        [return: MarshalAs(UnmanagedType.Bool)]
        public static extern bool IsIconic(IntPtr window);

        [DllImport("user32.dll")]
        [return: MarshalAs(UnmanagedType.Bool)]
        public static extern bool ShowWindowAsync(IntPtr window, int command);
    }
}
'@
}

$outputPath = [IO.Path]::GetFullPath($Output)
New-Item -ItemType Directory -Force -Path (Split-Path -Parent $outputPath) | Out-Null
$window = $process.MainWindowHandle
$initiallyMinimized = [Torca.WindowInspector]::IsIconic($window)
$changedWindowState = $false

function Get-ProcessSample {
    param([Diagnostics.Process]$Target)
    $Target.Refresh()
    [pscustomobject]@{
        CpuSeconds = $Target.TotalProcessorTime.TotalSeconds
        WorkingSetBytes = $Target.WorkingSet64
    }
}

try {
    if ($WindowState -eq 'minimized' -and -not $initiallyMinimized) {
        # SW_MINIMIZE. The original state is restored in finally.
        [void][Torca.WindowInspector]::ShowWindowAsync($window, 6)
        $changedWindowState = $true
        if ($WarmupSeconds -gt 0) { Start-Sleep -Seconds $WarmupSeconds }
    }

    $startedAt = [DateTime]::UtcNow
    $samples = [Collections.Generic.List[object]]::new()
    $previous = Get-ProcessSample -Target $process
    $stopwatch = [Diagnostics.Stopwatch]::StartNew()
    $previousElapsed = 0.0
    while ($stopwatch.Elapsed.TotalSeconds -lt $DurationSeconds) {
        Start-Sleep -Seconds $SampleSeconds
        $current = Get-ProcessSample -Target $process
        $elapsed = $stopwatch.Elapsed.TotalSeconds
        $cpuDelta = [Math]::Max(0.0, $current.CpuSeconds - $previous.CpuSeconds)
        $wallDelta = [Math]::Max(0.001, $elapsed - $previousElapsed)
        $samples.Add([pscustomobject]@{
            elapsedSeconds = [Math]::Round($elapsed, 3)
            cpuPercentOfOneLogicalCpu = [Math]::Round(100.0 * $cpuDelta / $wallDelta, 4)
            cpuPercentOfMachine = [Math]::Round(
                100.0 * $cpuDelta / $wallDelta / [Environment]::ProcessorCount,
                4
            )
            workingSetBytes = $current.WorkingSetBytes
        })
        $previous = $current
        $previousElapsed = $elapsed
    }
    $stopwatch.Stop()

    $oneCpuValues = @($samples | ForEach-Object { [double]$_.cpuPercentOfOneLogicalCpu } | Sort-Object)
    $machineValues = @($samples | ForEach-Object { [double]$_.cpuPercentOfMachine } | Sort-Object)
    $cpuSecondsObserved = [double](($samples | ForEach-Object { [double]$_.cpuPercentOfOneLogicalCpu } | Measure-Object -Sum).Sum)
    $measurementWarnings = [Collections.Generic.List[string]]::new()
    if ($cpuSecondsObserved -le 0) {
        $measurementWarnings.Add('No CPU time was observed; process may be suspended, exited, or the counter may be unavailable.')
    }
    $medianIndex = [int][Math]::Floor(($oneCpuValues.Count - 1) / 2)
    $p95Index = [int][Math]::Min($oneCpuValues.Count - 1, [Math]::Floor($oneCpuValues.Count * 0.95))
    $report = [ordered]@{
        schema = 1
        startedAtUtc = $startedAt.ToString('o')
        finishedAtUtc = [DateTime]::UtcNow.ToString('o')
        processName = $process.ProcessName
        processId = $process.Id
        executablePath = $process.Path
        requestedWindowState = $WindowState
        initiallyMinimized = $initiallyMinimized
        measuredMinimized = [Torca.WindowInspector]::IsIconic($window)
        warmupSeconds = $WarmupSeconds
        durationSeconds = [Math]::Round($stopwatch.Elapsed.TotalSeconds, 3)
        sampleSeconds = $SampleSeconds
        logicalProcessorCount = [Environment]::ProcessorCount
        validMeasurement = ($measurementWarnings.Count -eq 0)
        warnings = @($measurementWarnings)
        summary = [ordered]@{
            sampleCount = $oneCpuValues.Count
            medianPercentOfOneLogicalCpu = [Math]::Round($oneCpuValues[$medianIndex], 4)
            p95PercentOfOneLogicalCpu = [Math]::Round($oneCpuValues[$p95Index], 4)
            maximumPercentOfOneLogicalCpu = [Math]::Round(($oneCpuValues | Measure-Object -Maximum).Maximum, 4)
            medianPercentOfMachine = [Math]::Round($machineValues[$medianIndex], 4)
            p95PercentOfMachine = [Math]::Round($machineValues[$p95Index], 4)
        }
        samples = @($samples)
    }
    $report | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $outputPath -Encoding utf8
    Write-Host ("Window CPU measurement: state={0}, median={1}% of machine, p95={2}%" -f `
        $WindowState,
        $report.summary.medianPercentOfMachine,
        $report.summary.p95PercentOfMachine)
    Write-Host "Report: $outputPath"
} finally {
    if ($changedWindowState) {
        # SW_RESTORE. Benchmarking must not leave the user's application minimized.
        [void][Torca.WindowInspector]::ShowWindowAsync($window, 9)
    }
    if ($null -ne $launchedProcess -and -not $launchedProcess.HasExited) {
        $launchedProcess.CloseMainWindow() | Out-Null
        if (-not $launchedProcess.WaitForExit(5000)) { $launchedProcess.Kill() }
    }
}
