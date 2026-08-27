[CmdletBinding()]
param(
    [ValidateSet('smoke', 'conversation', 'soak', 'full')][string]$Mode = 'smoke',
    [ValidateSet('always', 'direct', 'local')][string]$Profile = 'always',
    [string]$Apk = 'apps/client/flutter/build/app/outputs/flutter-apk/app-x86_64-release.apk',
    [string]$Avd = 'TorChat_API36',
    [string]$Serial = 'emulator-5554',
    [int]$DurationSeconds = 30,
    [int]$Repetitions = 3,
    [ValidateRange(1, 5)][int]$FakePeers = 1,
    [ValidateRange(10, 300)][int]$ReadyTimeoutSeconds = 90,
    [switch]$EnableUiProbe,
    [string]$OutputRoot = '.torca/measurements/background',
    [switch]$NoHostCpuGuard
)

$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath($OutputRoot)
New-Item -ItemType Directory -Force -Path $root | Out-Null
$lockPath = Join-Path $root '.torca-background.lock'
$lock = $null
$lockAcquired = $false
$status = 'failed'
$reason = $null
$started = [DateTime]::UtcNow

try {
    try {
        $lock = [IO.File]::Open($lockPath, [IO.FileMode]::OpenOrCreate, [IO.FileAccess]::ReadWrite, [IO.FileShare]::None)
        $lockAcquired = $true
    } catch {
        throw 'Another Torca background test is already running.'
    }

    $runId = '{0:yyyyMMdd-HHmmss}-{1}' -f (Get-Date), ([Guid]::NewGuid().ToString('N').Substring(0, 8))
    $runRoot = Join-Path $root $runId
    New-Item -ItemType Directory -Force -Path $runRoot | Out-Null

    if ($Mode -in @('smoke', 'conversation', 'full')) {
        & cargo test -p torca-transport-iroh --locked
        if ($LASTEXITCODE -ne 0) { throw 'Iroh transport smoke tests failed.' }
        & cargo test -p torca-provider-conformance --locked
        if ($LASTEXITCODE -ne 0) { throw 'Provider conformance smoke tests failed.' }
    }

    $profiles = if ($Mode -eq 'full') { @('always', 'direct', 'local') } else { @($Profile) }
    foreach ($runProfile in $profiles) {
        $runArgs = @{
            Avd = $Avd
            Serial = $Serial
            Apk = $Apk
            Profile = $runProfile
            DurationSeconds = $DurationSeconds
            Repetitions = $(if ($Mode -eq 'smoke') { 1 } else { $Repetitions })
            OutputRoot = (Join-Path $runRoot $runProfile)
            FakePeers = $FakePeers
            ReadyTimeoutSeconds = $ReadyTimeoutSeconds
        }
        if ($NoHostCpuGuard) { $runArgs.NoHostCpuGuard = $true }
        if ($EnableUiProbe) { $runArgs.EnableUiProbe = $true }
        if ($Mode -eq 'conversation') { $runArgs.ActiveMessaging = $true }
        & (Join-Path $PSScriptRoot 'run-android-emulator-cpu.ps1') @runArgs
        if ($LASTEXITCODE -ne 0) { throw "Android emulator benchmark failed for $runProfile." }
    }

    $status = 'passed'
} catch {
    $reason = $_.Exception.Message
    if ($reason -like '*already running*') { $status = 'pending' }
} finally {
    $finished = [DateTime]::UtcNow
    [ordered]@{
        schema = 1
        runId = $(if ($runId) { $runId } else { $null })
        mode = $Mode
        provider = 'iroh'
        profile = $Profile
        status = $status
        reason = $reason
        startedAtUtc = $started.ToString('o')
        finishedAtUtc = $finished.ToString('o')
        durationSeconds = [int]($finished - $started).TotalSeconds
        outputRoot = $root
    } | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $root 'latest.json') -Encoding utf8

    if ($null -ne $lock) { $lock.Dispose() }
    if ($lockAcquired) {
        Remove-Item -LiteralPath $lockPath -Force -ErrorAction SilentlyContinue
    }
}

if ($status -ne 'passed') { throw "Background test ${status}: $reason" }
Write-Host "Torca background test passed: $Mode/$Profile"
