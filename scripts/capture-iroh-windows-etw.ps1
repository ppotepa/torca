[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$ProcessName,
    [int]$DurationSeconds = 300,
    [string]$Provider = 'Iroh',
    [string]$Profile = 'direct',
    [ValidateSet('foreground', 'background')]
    [string]$Mode = 'foreground',
    [string]$Output = '.torca/measurements/iroh-windows-cpu.etl',
    [string]$MetadataOutput = '.torca/measurements/iroh-windows-cpu.json'
)

$ErrorActionPreference = 'Stop'
if ($DurationSeconds -lt 5) { throw 'DurationSeconds must be at least 5 seconds.' }

$wpr = Get-Command wpr.exe -ErrorAction SilentlyContinue
if ($null -eq $wpr) {
    throw 'wpr.exe is required. Install the Windows Performance Toolkit from the Windows ADK.'
}

$outputPath = [IO.Path]::GetFullPath($Output)
$metadataPath = [IO.Path]::GetFullPath($MetadataOutput)
New-Item -ItemType Directory -Force -Path (Split-Path -Parent $outputPath) | Out-Null
New-Item -ItemType Directory -Force -Path (Split-Path -Parent $metadataPath) | Out-Null

$startedAt = [DateTime]::UtcNow
$traceStarted = $false
try {
    # The built-in CPU profile records scheduler/thread activity system-wide.
    # ProcessName is retained in metadata so WPA can isolate Torca.
    & $wpr.Source -start CPU -filemode | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "wpr -start failed with exit code $LASTEXITCODE" }
    $traceStarted = $true
    Start-Sleep -Seconds $DurationSeconds
}
finally {
    if ($traceStarted) {
        & $wpr.Source -stop $outputPath -skipPdbGen | Out-Null
        if ($LASTEXITCODE -ne 0) { throw "wpr -stop failed with exit code $LASTEXITCODE" }
    }
}

$metadata = [pscustomobject]@{
    schema = 1
    processName = $ProcessName
    provider = $Provider
    profile = $Profile
    mode = $Mode
    startedAtUtc = $startedAt.ToString('o')
    finishedAtUtc = [DateTime]::UtcNow.ToString('o')
    durationSeconds = $DurationSeconds
    trace = $outputPath
    analysis = 'Open the ETL in WPA and filter CPU Usage (Precise) by processName; inspect Torca and torca-iroh threads separately.'
}
$metadata | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $metadataPath -Encoding utf8
Write-Output $metadataPath
