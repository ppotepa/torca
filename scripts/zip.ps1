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
$collectionDirectory = [IO.Path]::ChangeExtension($archive[0], $null)
$manifestPath = Join-Path $collectionDirectory 'collection-manifest.json'
if (Test-Path -LiteralPath $manifestPath -PathType Leaf) {
    $manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
    $windowsCount = @($manifest.devices | Where-Object platform -eq 'windows').Count
    $androidCount = @($manifest.devices | Where-Object platform -eq 'android').Count
    $runtimeRuns = @($manifest.devices.collected | Where-Object { $_ -match '(^|/)run-\d+' }).Count
    $deviceErrors = @($manifest.devices.errors | Where-Object { -not [string]::IsNullOrWhiteSpace($_) })
    $collectionErrors = @($manifest.errors | Where-Object { -not [string]::IsNullOrWhiteSpace($_) })
    $collectionWarnings = @($manifest.warnings | Where-Object { -not [string]::IsNullOrWhiteSpace($_) })
    $deployRuns = @(Get-ChildItem -LiteralPath (Join-Path $collectionDirectory 'sources/deploy/runs') -Recurse -File -Filter 'run.json' -ErrorAction SilentlyContinue).Count
    Write-Host "Collection summary: status=$($manifest.status) windows=$windowsCount android=$androidCount clientRuntimeRuns=$runtimeRuns deployRuns=$deployRuns errors=$($deviceErrors.Count + $collectionErrors.Count) warnings=$($collectionWarnings.Count)" -ForegroundColor Cyan
    foreach ($message in @($collectionErrors + $deviceErrors | Select-Object -Unique)) {
        Write-Warning $message
    }
    foreach ($message in @($collectionWarnings | Select-Object -Unique)) {
        Write-Warning $message
    }
    if ($runtimeRuns -eq 0) {
        Write-Warning 'No structured native runtime run was collected. Start the clients, wait until the failure is visible, keep Android connected and authorized, then rerun zip.ps1.'
    }
}
if ((Get-Item -LiteralPath $OutputPath).Length -lt 20KB) {
    Write-Warning "The archive is unusually small ($((Get-Item -LiteralPath $OutputPath).Length) bytes). Check collection-manifest.json for missing devices or collection errors."
}
Write-Output $OutputPath
