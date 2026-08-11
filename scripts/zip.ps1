[CmdletBinding()]
param(
    [ValidateRange(1, 100)][int]$LastRuns = 10,
    [ValidateSet('auto', 'windows', 'android', 'all')][string]$Target = 'all',
    [string[]]$Device,
    [ValidateSet('basic', 'extended', 'incident')][string]$Profile = 'extended',
    [string]$OutputPath,
    [switch]$KeepCollectionDirectory,
    [switch]$SkipLogcat,
    [switch]$SkipStackLogs
)

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
if ([string]::IsNullOrWhiteSpace($OutputPath)) {
    $OutputPath = Join-Path $repoRoot 'logs.zip'
}

# `collect.ps1` owns device discovery and deduplicates a phone exposed through
# USB, Wi-Fi ADB and mDNS.  The default therefore captures the Windows host and
# every currently connected Android device in one archive without a wizard.
$collectArguments = @{
    LastRuns = $LastRuns
    Target = $Target
    Profile = $Profile
}
if ($Device) { $collectArguments.Device = $Device }
if ($SkipLogcat) { $collectArguments.SkipLogcat = $true }
if ($SkipStackLogs) { $collectArguments.SkipStackLogs = $true }
if ($KeepCollectionDirectory) { $collectArguments.KeepDirectory = $true }

$archive = @(& (Join-Path $PSScriptRoot 'collect.ps1') @collectArguments |
    Where-Object { $_ -is [string] -and (Test-Path -LiteralPath $_ -PathType Leaf) } |
    Select-Object -Last 1)
if ($archive.Count -ne 1) {
    throw 'Diagnostic collection did not return an archive path.'
}

$outputDirectory = Split-Path -Parent $OutputPath
if ($outputDirectory) {
    New-Item -ItemType Directory -Force -Path $outputDirectory | Out-Null
}
Copy-Item -LiteralPath $archive[0] -Destination $OutputPath -Force
Write-Host "Combined diagnostics archive: $OutputPath" -ForegroundColor Green
Write-Output $OutputPath
