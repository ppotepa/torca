[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$AndroidSerial,
    [int]$DurationMinutes = 30,
    [int]$Repetitions = 3,
    [string]$OutputRoot = '.torca/measurements/iroh-battery-matrix',
    [string]$Cargo = 'cargo',
    [switch]$SkipTor,
    [switch]$SkipIroh
)

$ErrorActionPreference = 'Stop'
if ($DurationMinutes -lt 1) { throw 'DurationMinutes must be at least 1.' }
if ($Repetitions -lt 3) { throw 'Repetitions must be at least 3 for battery evidence.' }
if ($SkipTor -and $SkipIroh) { throw 'At least one provider must be enabled.' }

$root = [IO.Path]::GetFullPath($OutputRoot)
New-Item -ItemType Directory -Force -Path $root | Out-Null
$startedAt = [DateTime]::UtcNow
$matrix = [Collections.Generic.List[object]]::new()
$adb = Get-Command adb.exe -ErrorAction SilentlyContinue
if ($null -eq $adb) { throw 'adb.exe is required.' }

function Get-NetworkFingerprint {
    # Hash only coarse connectivity state. The raw dumpsys output can contain
    # identifiers, so it is never written to the matrix report.
    $networkType = & $adb.Source '-s' $AndroidSerial 'shell' 'getprop' 'gsm.network.type' 2>$null
    $networkTypeExit = $LASTEXITCODE
    $operator = & $adb.Source '-s' $AndroidSerial 'shell' 'getprop' 'gsm.operator.alpha' 2>$null
    $operatorExit = $LASTEXITCODE
    $wifiDump = & $adb.Source '-s' $AndroidSerial 'shell' 'dumpsys' 'wifi' 2>$null
    $wifiExit = $LASTEXITCODE
    $wifiState = $wifiDump | Select-String -Pattern 'mNetworkInfo|mWifiInfo|SSID|Supplicant state'
    $state = @($networkType, $operator, $wifiState) -join "`n"
    if ($networkTypeExit -ne 0 -or $operatorExit -ne 0 -or $wifiExit -ne 0 -or [string]::IsNullOrWhiteSpace($state)) {
        throw "could not read network state from Android device $AndroidSerial"
    }
    $sha = [Security.Cryptography.SHA256]::Create()
    try {
        $bytes = [Text.Encoding]::UTF8.GetBytes($state.Trim())
        return ([BitConverter]::ToString($sha.ComputeHash($bytes))).Replace('-', '').ToLowerInvariant()
    } finally {
        $sha.Dispose()
    }
}

& $adb.Source '-s' $AndroidSerial 'get-state' | Out-Null
if ($LASTEXITCODE -ne 0) { throw "Android device is not ready: $AndroidSerial" }
$referenceNetworkFingerprint = $null

$cases = [Collections.Generic.List[object]]::new()
if (-not $SkipTor) {
    $cases.Add([pscustomobject]@{ Provider = 'tor'; Profile = 'managed' })
}

function Get-BatteryLevel {
    param([string]$Root, [string]$Name)
    $path = Join-Path $Root $Name
    if (-not (Test-Path -LiteralPath $path)) { return $null }
    $match = Select-String -LiteralPath $path -Pattern '^\s*level:\s*(\d+)' | Select-Object -First 1
    if ($null -eq $match) { return $null }
    [int]$match.Matches[0].Groups[1].Value
}

function Get-SoakRunRoot {
    param([string]$CaseRoot)
    $runs = @(Get-ChildItem -LiteralPath $CaseRoot -Directory -ErrorAction SilentlyContinue |
        Sort-Object LastWriteTime)
    if ($runs.Count -eq 0) {
        throw "torca-soak produced no run directory under $CaseRoot"
    }
    return $runs[-1].FullName
}

function Assert-RunManifest {
    param(
        [string]$RunRoot,
        [string]$ExpectedProvider,
        [string]$ExpectedProfile
    )
    $manifestPath = Join-Path $RunRoot 'manifest.json'
    if (-not (Test-Path -LiteralPath $manifestPath)) {
        throw "torca-soak run is missing manifest.json: $RunRoot"
    }
    $manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
    if ($manifest.communication_provider -ne $ExpectedProvider) {
        throw "run manifest provider mismatch: expected $ExpectedProvider, found $($manifest.communication_provider)"
    }
    $manifestProfile = if ($null -eq $manifest.iroh_profile) { $null } else { [string]$manifest.iroh_profile }
    if ($ExpectedProvider -eq 'iroh' -and $manifestProfile -ne $ExpectedProfile) {
        throw "run manifest Iroh profile mismatch: expected $ExpectedProfile, found $manifestProfile"
    }
}

function Get-Median {
    param([double[]]$Values)
    if ($Values.Count -eq 0) { return $null }
    $ordered = @($Values | Sort-Object)
    $middle = [int][Math]::Floor($ordered.Count / 2)
    if ($ordered.Count % 2 -eq 0) {
        return ($ordered[$middle - 1] + $ordered[$middle]) / 2.0
    }
    return $ordered[$middle]
}
if (-not $SkipIroh) {
    foreach ($profile in @('always', 'direct', 'local')) {
        $cases.Add([pscustomobject]@{ Provider = 'iroh'; Profile = $profile })
    }
}

foreach ($case in $cases) {
    for ($repeat = 1; $repeat -le $Repetitions; $repeat++) {
        $caseRoot = Join-Path $root ("{0}-{1}-run{2}" -f $case.Provider, $case.Profile, $repeat)
        New-Item -ItemType Directory -Force -Path $caseRoot | Out-Null
        $networkStart = Get-NetworkFingerprint
        if ($null -eq $referenceNetworkFingerprint) {
            $referenceNetworkFingerprint = $networkStart
        } elseif ($networkStart -ne $referenceNetworkFingerprint) {
            throw "network changed before $($case.Provider)/$($case.Profile) repetition $repeat; discard matrix and restart on one network"
        }
        $arguments = @(
            'run', '-p', 'torca-soak', '--',
            '--scenario', 'idle-battery',
            '--android', $AndroidSerial,
            '--duration-seconds', ([string]($DurationMinutes * 60)),
            '--communication-provider', $case.Provider,
            '--require-unplugged',
            '--require-screen-off',
            '--collect-native-diagnostics',
            '--output', $caseRoot,
            '--plain'
        )
        $previousProfile = $env:TORCA_SOAK_IROH_PROFILE
        try {
            if ($case.Provider -eq 'iroh') {
                $env:TORCA_SOAK_IROH_PROFILE = $case.Profile
            } else {
                Remove-Item Env:TORCA_SOAK_IROH_PROFILE -ErrorAction SilentlyContinue
            }
            & $Cargo @arguments
            $exitCode = $LASTEXITCODE
        } finally {
            if ($null -eq $previousProfile) {
                Remove-Item Env:TORCA_SOAK_IROH_PROFILE -ErrorAction SilentlyContinue
            } else {
                $env:TORCA_SOAK_IROH_PROFILE = $previousProfile
            }
        }
        $networkEnd = Get-NetworkFingerprint
        $networkStable = $networkStart -eq $networkEnd -and $networkEnd -eq $referenceNetworkFingerprint
        $runRoot = Get-SoakRunRoot -CaseRoot $caseRoot
        Assert-RunManifest -RunRoot $runRoot -ExpectedProvider $case.Provider -ExpectedProfile $case.Profile
        $matrix.Add([pscustomobject]@{
            provider = $case.Provider
            profile = $case.Profile
            repetition = $repeat
            output = $runRoot
            exitCode = $exitCode
            networkFingerprint = $networkStart
            networkFingerprintEnd = $networkEnd
            networkStable = $networkStable
            batteryStartPercent = Get-BatteryLevel -Root $runRoot -Name 'battery-start.txt'
            batteryEndPercent = Get-BatteryLevel -Root $runRoot -Name 'battery-end.txt'
            batteryDropPercent = if ((Get-BatteryLevel -Root $runRoot -Name 'battery-start.txt') -ne $null -and (Get-BatteryLevel -Root $runRoot -Name 'battery-end.txt') -ne $null) {
                (Get-BatteryLevel -Root $runRoot -Name 'battery-start.txt') - (Get-BatteryLevel -Root $runRoot -Name 'battery-end.txt')
            } else { $null }
            completedAtUtc = [DateTime]::UtcNow.ToString('o')
        })
        if (-not $networkStable) {
            throw "network changed during $($case.Provider)/$($case.Profile) repetition $repeat; battery evidence is invalid"
        }
        if ($exitCode -ne 0) {
            throw "Battery matrix case failed: $($case.Provider)/$($case.Profile), repetition $repeat (exit $exitCode)."
        }
    }
}

$summaries = @(
    $matrix | Group-Object { "{0}/{1}" -f $_.provider, $_.profile } | ForEach-Object {
        $drops = @($_.Group | Where-Object { $null -ne $_.batteryDropPercent } | ForEach-Object { [double]$_.batteryDropPercent } | Sort-Object)
        $median = Get-Median -Values ([double[]]$drops)
        [pscustomobject]@{
            providerProfile = $_.Name
            runs = $_.Count
            batterySamples = $drops.Count
            batteryDropMedianPercent = $median
            batteryDropMinimumPercent = if ($drops.Count -gt 0) { $drops[0] } else { $null }
            batteryDropMaximumPercent = if ($drops.Count -gt 0) { $drops[$drops.Count - 1] } else { $null }
        }
    }
)

$report = [ordered]@{
    schema = 1
    startedAtUtc = $startedAt.ToString('o')
    finishedAtUtc = [DateTime]::UtcNow.ToString('o')
    device = $AndroidSerial
    durationMinutes = $DurationMinutes
    repetitions = $Repetitions
    sameDeviceRequired = $true
    sameNetworkRequired = $true
    networkFingerprint = $referenceNetworkFingerprint
    cases = @($matrix)
    summaries = $summaries
}
$report | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $root 'matrix.json') -Encoding utf8
Write-Host "Battery matrix complete: $(Join-Path $root 'matrix.json')"
