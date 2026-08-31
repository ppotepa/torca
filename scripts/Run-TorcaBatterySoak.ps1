[CmdletBinding()]
param(
    [Parameter(Mandatory = $false)][int]$DurationMinutes = 60,
    # Empty means "select from TORCA_SOAK_FLAVOR".  Direct invocations remain
    # on the normal package; the Rust soak cockpit exports TORCA_SOAK_FLAVOR=1
    # so its auto-deploy and diagnostics use the isolated SOAK namespace.
    [Parameter(Mandatory = $false)][string]$Package = '',
    [Parameter(Mandatory = $false)][string]$DeviceId,
    [Parameter(Mandatory = $false)][string]$OutputRoot,
    [Parameter(Mandatory = $false)][switch]$RequireUnplugged,
    [Parameter(Mandatory = $false)][switch]$RequireScreenOff,
    [Parameter(Mandatory = $false)][switch]$CollectNativeDiagnostics,
    [Parameter(Mandatory = $false)][string]$NativeLogRoot,
    [Parameter(Mandatory = $false)][ValidateSet('always', 'direct', 'local')][string]$ProviderProfile = 'direct',
    [Parameter(Mandatory = $false)][ValidateRange(1, 3)][int]$AutoDeployAttempts = 2,
    [Parameter(Mandatory = $false)][switch]$ValidateAfter
)

$ErrorActionPreference = 'Stop'
$scriptRoot = if ([string]::IsNullOrWhiteSpace($PSScriptRoot)) { Split-Path -Parent $MyInvocation.MyCommand.Path } else { $PSScriptRoot }
if ($DurationMinutes -lt 1) { throw 'DurationMinutes must be at least 1.' }
if ([string]::IsNullOrWhiteSpace($Package)) {
    $Package = if ($env:TORCA_SOAK_FLAVOR -eq '1' -or $env:TORCA_SOAK_FLAVOR -eq 'true') {
        'com.torca.torca_app.soak'
    } else {
        'com.torca.torca_app'
    }
}
if (-not $OutputRoot) {
    $OutputRoot = Join-Path $scriptRoot '../artifacts/soak'
}
if (-not $NativeLogRoot) {
    $NativeLogRoot = "/sdcard/Android/data/$Package/files/torca/logs"
}

$deviceLines = @(& adb devices 2>&1)
$readyDevices = @($deviceLines | Where-Object { $_ -match '^\S+\s+device\s*$' } | ForEach-Object { ($_ -split '\s+')[0] })
if ($DeviceId) {
    if ($readyDevices -notcontains $DeviceId) {
        throw "Requested ADB device '$DeviceId' is not ready. Ready devices: $($readyDevices -join ', ')."
    }
} elseif ($readyDevices.Count -ne 1) {
    throw "Expected exactly one ready ADB device; found $($readyDevices.Count). Pass -DeviceId explicitly. Ready devices: $($readyDevices -join ', ')."
} else {
    $DeviceId = $readyDevices[0]
}

$adbPrefix = @('-s', $DeviceId)
function Invoke-SelectedAdb([string[]]$Arguments) {
    & adb @($adbPrefix + $Arguments)
}

$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$output = Join-Path $OutputRoot "battery-$stamp"
New-Item -ItemType Directory -Force -Path $output | Out-Null

function Capture-Adb([string]$Name, [string[]]$Arguments) {
    $path = Join-Path $output $Name
    Invoke-SelectedAdb $Arguments 2>&1 | Out-File -Encoding utf8 $path
}

function Capture-NativeDiagnostics([string]$Destination) {
    $nativeOutput = Join-Path $output $Destination
    New-Item -ItemType Directory -Force -Path $nativeOutput | Out-Null
    $listing = Invoke-SelectedAdb @('shell', 'find', $NativeLogRoot, '-type', 'f', '-name', '*.json', '-o', '-name', '*.log') 2>&1 | Out-String
    $listing | Out-File -Encoding utf8 (Join-Path $nativeOutput 'listing.txt')
    $pull = Invoke-SelectedAdb @('pull', $NativeLogRoot, $nativeOutput) 2>&1 | Out-String
    $pull | Out-File -Encoding utf8 (Join-Path $nativeOutput 'pull.txt')
    return (($LASTEXITCODE -eq 0) -and ($listing -notmatch '(?im)(No such file|Permission denied|not found)'))
}

function Get-PowerSource([string]$BatteryText) {
    $sources = @('AC', 'USB', 'Wireless') | Where-Object {
        $BatteryText -match "(?im)^\s*$($_) powered:\s*true\s*$"
    }
    if ($sources.Count -eq 0) { return 'battery' }
    return ($sources -join ',').ToLowerInvariant()
}

function Get-BatteryLevel([string]$BatteryText) {
    $match = [regex]::Match($BatteryText, '(?im)^\s*level:\s*(\d+)\s*$')
    if (-not $match.Success) { return $null }
    return [int]$match.Groups[1].Value
}

function Test-PackageInstalled {
    $path = Invoke-SelectedAdb @('shell', 'pm', 'path', $Package) 2>$null | Out-String
    return ($LASTEXITCODE -eq 0 -and $path -match '(?im)^package:')
}

function Get-PackageLaunchableActivity {
    # PackageManager can briefly return an empty resolution immediately after
    # an APK install/force-stop. Retry without treating that transient state
    # as a missing installation.
    for ($attempt = 1; $attempt -le 3; $attempt++) {
        $resolved = Invoke-SelectedAdb @(
            'shell', 'cmd', 'package', 'resolve-activity', '--brief',
            '-a', 'android.intent.action.MAIN',
            '-c', 'android.intent.category.LAUNCHER',
            $Package
        ) 2>$null | Out-String
        if ($LASTEXITCODE -eq 0) {
            $component = $resolved -split '\r?\n' |
                Where-Object { $_.Trim() -match "^$([regex]::Escape($Package))/[^\s]+$" } |
                Select-Object -Last 1
            if (-not [string]::IsNullOrWhiteSpace($component)) {
                return $component.Trim()
            }
        }
        if ($attempt -lt 3) { Start-Sleep -Seconds 1 }
    }
    return $null
}

function Test-PackageLaunchable {
    return -not [string]::IsNullOrWhiteSpace((Get-PackageLaunchableActivity))
}

function Ensure-PackageInstalled {
    $installed = Test-PackageInstalled
    if ($installed -and (Test-PackageLaunchable)) { return }

    $repoRoot = Resolve-Path (Join-Path $scriptRoot '..')
    $deployLog = Join-Path $output 'auto-deploy.log'
    Write-Host "Android auto-deploy details will be saved to '$deployLog'."
    if ($installed) {
        Write-Host "Package '$Package' is installed but has no resolvable MAIN/LAUNCHER activity; running Android deploy before the battery soak."
    } else {
        Write-Host "Package '$Package' is not installed; running Android deploy before the battery soak."
    }
    for ($attempt = 1; $attempt -le $AutoDeployAttempts; $attempt++) {
        $deployArguments = @(
            'run', '-p', 'torca-deploy', '--',
            'deploy', '--target', 'android', '--device', $DeviceId,
            '--configuration', 'debug', '--client-build', 'if-required',
            '--client-data', 'preserve', '--validation', 'quick', '--launch', 'restart',
            '--provider-profile', $ProviderProfile
        )
        Write-Host "--- Android auto-deploy attempt $attempt/$AutoDeployAttempts (live output) ---"
        Push-Location $repoRoot
        try {
            # Keep the output visible for an interactive run while preserving
            # the exact merged stdout/stderr stream for postmortem analysis.
            & cargo @deployArguments 2>&1 | Tee-Object -FilePath $deployLog -Append
            $deployExitCode = $LASTEXITCODE
        } finally {
            Pop-Location
        }
        @(
            "--- auto-deploy attempt $attempt/$AutoDeployAttempts ---"
        ) | Out-File -Append -Encoding utf8 $deployLog
        if ($deployExitCode -eq 0) {
            # PackageManager can lag behind the installer by a few seconds,
            # especially for streamed split APK installation. Poll the same
            # device before deciding that deploy failed.
            for ($verifyAttempt = 1; $verifyAttempt -le 5; $verifyAttempt++) {
                if ((Test-PackageInstalled) -and (Test-PackageLaunchable)) {
                    Write-Host "Android package '$Package' is installed and launchable on '$DeviceId' (deploy attempt $attempt)."
                    return
                }
                if ($verifyAttempt -lt 5) { Start-Sleep -Seconds 2 }
            }
        }

        $tail = (Get-Content $deployLog -Tail 30 -ErrorAction SilentlyContinue) -join [Environment]::NewLine
        if ($tail -match 'INSTALL_FAILED_USER_RESTRICTED|Install canceled by user' -and $attempt -lt $AutoDeployAttempts) {
            Write-Warning "Android installation was blocked by the device. Approve the install prompt on '$DeviceId'; retrying automatically."
            Start-Sleep -Seconds 5
            continue
        }
        $packageState = (Invoke-SelectedAdb @('shell', 'pm', 'path', $Package) 2>&1 | Out-String).Trim()
        $activityState = (Invoke-SelectedAdb @('shell', 'cmd', 'package', 'resolve-activity', '--brief', '-a', 'android.intent.action.MAIN', '-c', 'android.intent.category.LAUNCHER', $Package) 2>&1 | Out-String).Trim()
        throw "Android auto-deploy failed (attempt=$attempt, exit=$deployExitCode); packagePath='$packageState'; launchActivity='$activityState'. See '$deployLog'.`n$tail"
    }
    throw "Android deploy completed without a launchable '$Package' activity on '$DeviceId'. Check '$deployLog' and approve the installation prompt on the device."
}

Capture-Adb 'device.txt' @('shell', 'getprop')
$batteryBeforeText = Invoke-SelectedAdb @('shell', 'dumpsys', 'battery') 2>&1 | Out-String
$batteryBeforeText | Out-File -Encoding utf8 (Join-Path $output 'battery-before.txt')
$powerSourceBefore = Get-PowerSource $batteryBeforeText
$batteryLevelBefore = Get-BatteryLevel $batteryBeforeText
Write-Host "Battery preflight: source=$powerSourceBefore level=$batteryLevelBefore output='$output'"
if ($RequireUnplugged -and $powerSourceBefore -ne 'battery') {
    Write-Host "Unplug the Android device, wait for dumpsys battery to report AC/USB/Wireless=false, then rerun the soak."
    throw "Battery soak requires an unplugged device, but '$DeviceId' reports power source '$powerSourceBefore'."
}
Capture-Adb 'power-before.txt' @('shell', 'dumpsys', 'power')
Ensure-PackageInstalled
$nativeDiagnosticsBefore = $false
if ($CollectNativeDiagnostics) {
    $nativeDiagnosticsBefore = Capture-NativeDiagnostics 'native-before'
}

Invoke-SelectedAdb @('shell', 'dumpsys', 'batterystats', '--reset') | Out-Null
Invoke-SelectedAdb @('shell', 'logcat', '-c') | Out-Null
$activity = Get-PackageLaunchableActivity
if ([string]::IsNullOrWhiteSpace($activity)) {
    throw "Torca package '$Package' has no launchable activity on '$DeviceId'. Install the debug APK and approve Android's installation prompt before starting the battery soak."
}
$launchOutput = Invoke-SelectedAdb @('shell', 'am', 'start', '-W', '-n', $activity) 2>&1 | Out-String
if ($LASTEXITCODE -ne 0 -or $launchOutput -notmatch '(?im)^Status:\s*ok\s*$') {
    throw "Torca activity '$activity' failed to start on '$DeviceId'. Details: $($launchOutput.Trim())"
}
Start-Sleep -Seconds 2
$appPid = (Invoke-SelectedAdb @('shell', 'pidof', $Package) 2>$null | Out-String).Trim()
if (-not $appPid) { throw "Torca process '$Package' did not start on ADB device '$DeviceId'." }
Start-Sleep -Seconds 20
Invoke-SelectedAdb @('shell', 'input', 'keyevent', 'KEYCODE_HOME') | Out-Null
if ($RequireScreenOff) {
    Invoke-SelectedAdb @('shell', 'input', 'keyevent', 'KEYCODE_SLEEP') | Out-Null
    Start-Sleep -Seconds 3
    $powerAtStartText = Invoke-SelectedAdb @('shell', 'dumpsys', 'power') 2>&1 | Out-String
    $powerAtStartText | Out-File -Encoding utf8 (Join-Path $output 'power-start.txt')
    if ($powerAtStartText -notmatch '(?im)mWakefulness=(Dozing|Asleep)') {
        throw "Battery soak requires the screen to be off, but Android did not enter Dozing/Asleep on '$DeviceId'."
    }
} else {
    $powerAtStartText = ''
}

$started = Get-Date
@{
    package = $Package
    deviceId = $DeviceId
    durationMinutes = $DurationMinutes
    requireUnplugged = [bool]$RequireUnplugged
    requireScreenOff = [bool]$RequireScreenOff
    powerSourceBefore = $powerSourceBefore
    batteryLevelBefore = $batteryLevelBefore
    appPid = $appPid
    communicationProvider = 'iroh'
    providerProfile = $ProviderProfile
    screenStateAtStart = if ($RequireScreenOff) { 'dozing_or_asleep' } else { 'unspecified' }
    startedAt = $started.ToString('o')
    scenario = 'warm-start then background idle'
    nativeLogRoot = $NativeLogRoot
    nativeDiagnosticsBeforeCollected = [bool]$nativeDiagnosticsBefore
} | ConvertTo-Json | Out-File -Encoding utf8 (Join-Path $output 'scenario.json')

Start-Sleep -Seconds ($DurationMinutes * 60)

Capture-Adb 'batterystats.txt' @('shell', 'dumpsys', 'batterystats', $Package)
Capture-Adb 'batterystats-checkin.txt' @('shell', 'dumpsys', 'batterystats', '--checkin', $Package)
Capture-Adb 'battery-after.txt' @('shell', 'dumpsys', 'battery')
Capture-Adb 'power-after.txt' @('shell', 'dumpsys', 'power')
Capture-Adb 'deviceidle-after.txt' @('shell', 'dumpsys', 'deviceidle')
Capture-Adb 'services-after.txt' @('shell', 'dumpsys', 'activity', 'services', $Package)
$processAfterText = Invoke-SelectedAdb @('shell', 'ps', '-A') 2>&1 | Out-String
$processAfterText | Out-File -Encoding utf8 (Join-Path $output 'process-after.txt')
Capture-Adb 'logcat.txt' @('logcat', '-d', '-v', 'threadtime')
$nativeDiagnosticsAfter = $false
if ($CollectNativeDiagnostics) {
    $nativeDiagnosticsAfter = Capture-NativeDiagnostics 'native-after'
}
$batteryAfterText = Get-Content (Join-Path $output 'battery-after.txt') -Raw
$powerSourceAfter = Get-PowerSource $batteryAfterText
$batteryLevelAfter = Get-BatteryLevel $batteryAfterText
$appRunningAtEnd = $processAfterText -match "(?m)\s$([regex]::Escape($Package))\s*$"

@{
    package = $Package
    deviceId = $DeviceId
    durationMinutes = $DurationMinutes
    requireUnplugged = [bool]$RequireUnplugged
    requireScreenOff = [bool]$RequireScreenOff
    powerSourceBefore = $powerSourceBefore
    powerSourceAfter = $powerSourceAfter
    batteryLevelBefore = $batteryLevelBefore
    batteryLevelAfter = $batteryLevelAfter
    appPid = $appPid
    appRunningAtEnd = [bool]$appRunningAtEnd
    screenStateAtStart = if ($RequireScreenOff) { 'dozing_or_asleep' } else { 'unspecified' }
    startedAt = $started.ToString('o')
    finishedAt = (Get-Date).ToString('o')
    output = (Resolve-Path $output).Path
    nativeLogRoot = $NativeLogRoot
    nativeDiagnosticsBeforeCollected = [bool]$nativeDiagnosticsBefore
    nativeDiagnosticsAfterCollected = [bool]$nativeDiagnosticsAfter
} | ConvertTo-Json | Out-File -Encoding utf8 (Join-Path $output 'result.json')

Write-Host "Battery soak capture complete: $output"
if ($ValidateAfter) {
    $validator = Join-Path $scriptRoot 'Validate-TorcaBatterySoak.ps1'
    $validationArguments = @(
        '-Path', $output,
        '-MinimumMinutes', $DurationMinutes
    )
    if ($CollectNativeDiagnostics) {
        $validationArguments += '-RequireNativeDiagnostics'
    }
    Write-Host "Validating battery soak evidence..."
    & $validator @validationArguments
    if ($LASTEXITCODE -and $LASTEXITCODE -ne 0) {
        throw "Battery soak evidence validation failed for '$output'."
    }
}
