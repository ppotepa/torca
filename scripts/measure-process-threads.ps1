[CmdletBinding()]
param(
    [string]$ProcessName = 'torca_app',
    [int]$ProcessId = 0,
    [int]$DurationSeconds = 30,
    [string]$Output = '.torca/measurements/process-threads.json'
)

$ErrorActionPreference = 'Stop'
if ($DurationSeconds -lt 5) { throw 'DurationSeconds must be at least 5 seconds.' }

$processes = if ($ProcessId -gt 0) {
    @(Get-Process -Id $ProcessId -ErrorAction SilentlyContinue)
} else {
    @(Get-Process -Name $ProcessName -ErrorAction SilentlyContinue)
}
if ($processes.Count -ne 1) {
    $selector = if ($ProcessId -gt 0) { "id '$ProcessId'" } else { "name '$ProcessName'" }
    throw "Expected exactly one process with $selector; found $($processes.Count)."
}
$process = $processes[0]
$outputPath = [IO.Path]::GetFullPath($Output)
New-Item -ItemType Directory -Force -Path (Split-Path -Parent $outputPath) | Out-Null

if (-not ('Torca.ThreadInspector' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;

namespace Torca {
    public static class ThreadInspector {
        private const uint THREAD_QUERY_LIMITED_INFORMATION = 0x0800;

        [DllImport("kernel32.dll", SetLastError = true)]
        private static extern IntPtr OpenThread(uint access, bool inheritHandle, uint threadId);

        [DllImport("kernel32.dll", CharSet = CharSet.Unicode)]
        private static extern int GetThreadDescription(IntPtr thread, out IntPtr description);

        [DllImport("kernel32.dll")]
        private static extern IntPtr LocalFree(IntPtr memory);

        [DllImport("kernel32.dll")]
        private static extern bool CloseHandle(IntPtr handle);

        public static string Description(int threadId) {
            IntPtr thread = OpenThread(THREAD_QUERY_LIMITED_INFORMATION, false, (uint)threadId);
            if (thread == IntPtr.Zero) return null;
            try {
                IntPtr value;
                if (GetThreadDescription(thread, out value) != 0 || value == IntPtr.Zero) return null;
                try {
                    return Marshal.PtrToStringUni(value);
                } finally {
                    LocalFree(value);
                }
            } finally {
                CloseHandle(thread);
            }
        }
    }
}
'@
}

function Get-ThreadSamples {
    param([Diagnostics.Process]$Target)

    $Target.Refresh()
    $samples = @{}
    foreach ($thread in @($Target.Threads)) {
        try {
            $samples[[int]$thread.Id] = [pscustomobject]@{
                Description = [Torca.ThreadInspector]::Description([int]$thread.Id)
                TotalSeconds = $thread.TotalProcessorTime.TotalSeconds
                UserSeconds = $thread.UserProcessorTime.TotalSeconds
                KernelSeconds = $thread.PrivilegedProcessorTime.TotalSeconds
                Priority = [int]$thread.CurrentPriority
                State = [string]$thread.ThreadState
                WaitReason = if ($thread.ThreadState -eq [Diagnostics.ThreadState]::Wait) {
                    [string]$thread.WaitReason
                } else {
                    $null
                }
            }
        } catch {
            # A thread may exit between enumerating the collection and reading
            # its counters. It is safe to omit that incomplete sample.
        }
    }
    return $samples
}

$startedAt = [DateTime]::UtcNow
$startCpu = $process.TotalProcessorTime.TotalSeconds
$startThreads = Get-ThreadSamples -Target $process
$stopwatch = [Diagnostics.Stopwatch]::StartNew()
Start-Sleep -Seconds $DurationSeconds
$stopwatch.Stop()
$process.Refresh()
$endCpu = $process.TotalProcessorTime.TotalSeconds
$endThreads = Get-ThreadSamples -Target $process
$wallSeconds = [Math]::Max(0.001, $stopwatch.Elapsed.TotalSeconds)

$threads = [Collections.Generic.List[object]]::new()
foreach ($entry in $endThreads.GetEnumerator()) {
    $threadId = [int]$entry.Key
    $end = $entry.Value
    $start = $startThreads[$threadId]
    if ($null -eq $start) { continue }
    $totalDelta = [Math]::Max(0.0, $end.TotalSeconds - $start.TotalSeconds)
    $userDelta = [Math]::Max(0.0, $end.UserSeconds - $start.UserSeconds)
    $kernelDelta = [Math]::Max(0.0, $end.KernelSeconds - $start.KernelSeconds)
    $threads.Add([pscustomobject]@{
        threadId = $threadId
        description = $end.Description
        cpuPercentOfOneLogicalCpu = [Math]::Round(100.0 * $totalDelta / $wallSeconds, 4)
        cpuSeconds = [Math]::Round($totalDelta, 6)
        userSeconds = [Math]::Round($userDelta, 6)
        kernelSeconds = [Math]::Round($kernelDelta, 6)
        currentPriority = $end.Priority
        finalState = $end.State
        finalWaitReason = $end.WaitReason
    })
}

$orderedThreads = @($threads | Sort-Object cpuSeconds -Descending)
$totalDelta = [Math]::Max(0.0, $endCpu - $startCpu)
$topCpu = [double](($orderedThreads | Select-Object -First 1).cpuSeconds)
$report = [ordered]@{
    schema = 1
    startedAtUtc = $startedAt.ToString('o')
    finishedAtUtc = [DateTime]::UtcNow.ToString('o')
    processName = $process.ProcessName
    processId = $process.Id
    executablePath = $process.Path
    durationSeconds = [Math]::Round($wallSeconds, 3)
    logicalProcessorCount = [Environment]::ProcessorCount
    processCpuSeconds = [Math]::Round($totalDelta, 6)
    processCpuPercentOfOneLogicalCpu = [Math]::Round(100.0 * $totalDelta / $wallSeconds, 4)
    processCpuPercentOfMachine = [Math]::Round(100.0 * $totalDelta / $wallSeconds / [Environment]::ProcessorCount, 4)
    sampledThreadCount = $orderedThreads.Count
    topThreadSharePercent = if ($totalDelta -gt 0) {
        [Math]::Round(100.0 * $topCpu / $totalDelta, 2)
    } else {
        0.0
    }
    threads = $orderedThreads
}

$report | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $outputPath -Encoding utf8
Write-Host ("Thread measurement: process={0}% of one CPU ({1}% of machine), top thread={2}%" -f `
    $report.processCpuPercentOfOneLogicalCpu,
    $report.processCpuPercentOfMachine,
    $report.topThreadSharePercent)
Write-Host "Report: $outputPath"
