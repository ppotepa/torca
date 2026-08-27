[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$AndroidSerial,
    [int]$DurationSeconds = 1800,
    [string]$Provider = 'Iroh',
    [string]$Profile = 'direct',
    [string]$Output = '.torca/measurements/iroh-android.perfetto-trace',
    [string]$MetadataOutput = '.torca/measurements/iroh-android.perfetto.json'
)

$ErrorActionPreference = 'Stop'
if ($DurationSeconds -lt 10) { throw 'DurationSeconds must be at least 10 seconds.' }
$adb = Get-Command adb.exe -ErrorAction SilentlyContinue
if ($null -eq $adb) { throw 'adb.exe is required.' }

$outputPath = [IO.Path]::GetFullPath($Output)
$metadataPath = [IO.Path]::GetFullPath($MetadataOutput)
New-Item -ItemType Directory -Force -Path (Split-Path -Parent $outputPath) | Out-Null
New-Item -ItemType Directory -Force -Path (Split-Path -Parent $metadataPath) | Out-Null

$remotePath = "/data/local/tmp/torca-iroh-$PID.perfetto-trace"
$startedAt = [DateTime]::UtcNow
$arguments = @(
    '-s', $AndroidSerial, 'shell', 'perfetto', '--txt',
    '-o', $remotePath, '-t', "${DurationSeconds}s",
    'sched', 'freq', 'idle', 'am', 'wm', 'network'
)
& $adb.Source @arguments | Out-Null
if ($LASTEXITCODE -ne 0) { throw "perfetto failed with exit code $LASTEXITCODE" }

& $adb.Source '-s' $AndroidSerial 'pull' $remotePath $outputPath | Out-Null
if ($LASTEXITCODE -ne 0) { throw "adb pull failed with exit code $LASTEXITCODE" }
& $adb.Source '-s' $AndroidSerial 'shell' 'rm' '-f' $remotePath | Out-Null

$metadata = [pscustomobject]@{
    schema = 1
    serial = $AndroidSerial
    provider = $Provider
    profile = $Profile
    startedAtUtc = $startedAt.ToString('o')
    finishedAtUtc = [DateTime]::UtcNow.ToString('o')
    durationSeconds = $DurationSeconds
    trace = $outputPath
    prerequisites = 'Run unplugged and screen-off on the same device/network as the battery matrix.'
    analysis = 'Inspect sched, freq, idle and network tracks for periodic Torca-controlled wake or reconnect patterns.'
}
$metadata | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $metadataPath -Encoding utf8
Write-Output $metadataPath
