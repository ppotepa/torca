[CmdletBinding()]
param(
    [string]$Avd = 'TorChat_API36',
    [string]$Serial = 'emulator-5554',
    [string]$Apk = 'apps/client/flutter/build/app/outputs/flutter-apk/app-x86_64-release.apk',
    [string]$Package = 'com.torca.torca_app',
    [ValidateSet('always', 'direct', 'local')][string]$Profile = 'always',
    [int]$DurationSeconds = 30,
    [int]$Repetitions = 3,
    [string]$OutputRoot = '.torca/measurements/android-emulator',
    [ValidateRange(1, 8)][int]$Cores = 2,
    [ValidateRange(768, 8192)][int]$MemoryMb = 1536,
    [ValidateRange(1, 100)][int]$MaxHostCpuPercent = 15,
    [switch]$NoHostCpuGuard,
    [ValidateRange(10, 300)][int]$ReadyTimeoutSeconds = 90,
    [switch]$EnableUiProbe,
    [switch]$ActiveMessaging,
    [ValidateRange(1, 5)][int]$FakePeers = 1
)

$ErrorActionPreference = 'Stop'
if ($DurationSeconds -lt 5) { throw 'DurationSeconds must be at least 5.' }
if ($Repetitions -lt 1) { throw 'Repetitions must be positive.' }
if (-not (Test-Path -LiteralPath $Apk -PathType Leaf)) { throw "APK not found: $Apk" }
$adb = (Get-Command adb.exe -ErrorAction SilentlyContinue).Source
$emulatorCommand = 'emula' + 'tor' + '.exe'
$emulator = (Get-Command $emulatorCommand -ErrorAction SilentlyContinue).Source
if (-not $adb) { throw 'adb.exe is required.' }
if (-not $emulator) { throw "$emulatorCommand is required." }

$outputPath = [IO.Path]::GetFullPath($OutputRoot)
New-Item -ItemType Directory -Force -Path $outputPath | Out-Null
$launcher = $null
$reports = [Collections.Generic.List[object]]::new()
$hostCpuSamples = [Collections.Generic.Queue[double]]::new()

function Get-EmulatorProcesses([System.Diagnostics.Process]$Root) {
    $processes = [Collections.Generic.List[System.Diagnostics.Process]]::new()
    if ($null -ne $Root -and -not $Root.HasExited) { $processes.Add($Root) }
    foreach ($candidate in @(Get-Process -Name 'qemu-system-x86_64' -ErrorAction SilentlyContinue)) {
        try {
            if ($candidate.StartTime -ge $Root.StartTime) { $processes.Add($candidate) }
        } catch { }
    }
    return @($processes)
}

function Get-ProcessMachineCpuPercent([System.Diagnostics.Process[]]$Processes) {
    $active = @($Processes | Where-Object { $null -ne $_ -and -not $_.HasExited })
    if ($active.Count -eq 0) { return 0.0 }
    $before = @{}
    foreach ($process in $active) { $before[$process.Id] = $process.TotalProcessorTime.TotalMilliseconds }
    $stamp = [DateTime]::UtcNow
    Start-Sleep -Milliseconds 750
    $delta = 0.0
    foreach ($process in $active) {
        try {
            $process.Refresh()
            if (-not $process.HasExited) { $delta += $process.TotalProcessorTime.TotalMilliseconds - $before[$process.Id] }
        } catch { }
    }
    $elapsed = ([DateTime]::UtcNow - $stamp).TotalMilliseconds
    if ($elapsed -le 0) { return 0.0 }
    $logical = [Environment]::ProcessorCount
    return [Math]::Max(0.0, ($delta / $elapsed) * 100.0 / $logical)
}

function Set-BackgroundPriority([System.Diagnostics.Process]$Process) {
    if ($null -eq $Process -or $Process.HasExited) { return }
    try { $Process.PriorityClass = [Diagnostics.ProcessPriorityClass]::BelowNormal } catch { }
}

function Read-AdbText([string[]]$Arguments) {
    try {
        return (& $adb @Arguments 2>&1 | Out-String)
    } catch {
        # ADB can report a short-lived "device offline" while an emulator
        # finishes registering. Callers treat an empty probe as not-ready and
        # retry; fatal startup diagnostics are still captured once available.
        return ''
    }
}

function Read-UiDump([string]$ArtifactRoot) {
    $stdoutPath = Join-Path $ArtifactRoot 'uiautomator.stdout.txt'
    $stderrPath = Join-Path $ArtifactRoot 'uiautomator.stderr.txt'
    $process = Start-Process -FilePath $adb -ArgumentList @(
        '-s', $Serial, 'shell', 'uiautomator', 'dump', '--compressed', '/sdcard/torca-window.xml'
    ) -WindowStyle Hidden -RedirectStandardOutput $stdoutPath -RedirectStandardError $stderrPath -PassThru
    if (-not $process.WaitForExit(5000)) {
        Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
        return @{ xml = ''; timedOut = $true }
    }
    $xml = Read-AdbText @('-s', $Serial, 'shell', 'cat', '/sdcard/torca-window.xml')
    return @{ xml = $xml; timedOut = $false }
}

function Wait-TorcaAppReady([string]$PackageName, [string]$ArtifactRoot) {
    $logPath = Join-Path $ArtifactRoot 'startup-logcat.txt'
    $activityPath = Join-Path $ArtifactRoot 'startup-activity.txt'
    $uiPath = Join-Path $ArtifactRoot 'startup-ui.xml'
    $deadline = [DateTime]::UtcNow.AddSeconds($ReadyTimeoutSeconds)
    $lastLog = ''
    $lastActivity = ''
    $lastUi = ''
    $uiTimedOut = $false
    while ([DateTime]::UtcNow -lt $deadline) {
        $lastActivity = Read-AdbText @('-s', $Serial, 'shell', 'dumpsys', 'activity', 'activities')
        $lastLog = Read-AdbText @('-s', $Serial, 'logcat', '-d', '-t', '600', '-v', 'brief')
        if ($EnableUiProbe) {
            $ui = Read-UiDump -ArtifactRoot $ArtifactRoot
            $lastUi = [string]$ui.xml
            $uiTimedOut = [bool]$ui.timedOut
        } else {
            # The benchmark emulator is deliberately launched with -no-window.
            # On API 35/36 this makes uiautomator dump block in adb forever;
            # activity focus plus the native/logcat checks below is the
            # deterministic readiness signal for a headless run.
            $lastUi = 'UI_PROBE_SKIPPED: headless benchmark (-no-window).'
            $uiTimedOut = $false
        }
        $activityReady = $lastActivity -match "(?i)(mResumedActivity|ResumedActivity).*\b$([regex]::Escape($PackageName))\b"
        $menuReady = if ($EnableUiProbe) {
            $lastUi -match '(?i)(Torca|Contacts|Kontakty|Invitations|Zaproszenia|Settings|Ustawienia|Profile|Profil|Display name|Nazwa)'
        } else {
            $activityReady
        }
        if ($uiTimedOut -and $activityReady) {
            # Headless AVD images may not expose an accessibility hierarchy.
            # Activity focus plus a clean native log is still a useful startup
            # signal, but the limitation is persisted in the artifact.
            $menuReady = $true
            $lastUi = 'UI_PROBE_TIMEOUT: headless emulator did not return a uiautomator hierarchy.'
        }
        $fatal = $lastLog -match '(?i)(Native Torca runtime startup failed|COMPOSITION_FAILED|CONTRACT_DECODE_FAILED|FATAL EXCEPTION|INSTALL_FAILED|StartupFailure)'
        if ($activityReady -and $menuReady -and -not $fatal) {
            Set-Content -LiteralPath $logPath -Value $lastLog -Encoding utf8
            Set-Content -LiteralPath $activityPath -Value $lastActivity -Encoding utf8
            Set-Content -LiteralPath $uiPath -Value $lastUi -Encoding utf8
            return
        }
        if ($fatal) {
            Set-Content -LiteralPath $logPath -Value $lastLog -Encoding utf8
            Set-Content -LiteralPath $activityPath -Value $lastActivity -Encoding utf8
            Set-Content -LiteralPath $uiPath -Value $lastUi -Encoding utf8
            throw "Android app startup reported a fatal error; inspect $logPath"
        }
        Start-Sleep -Seconds 2
    }
    Set-Content -LiteralPath $logPath -Value $lastLog -Encoding utf8
    Set-Content -LiteralPath $activityPath -Value $lastActivity -Encoding utf8
    Set-Content -LiteralPath $uiPath -Value $lastUi -Encoding utf8
    throw "Android app did not reach the menu within ${ReadyTimeoutSeconds}s; inspect startup-logcat.txt, startup-activity.txt and startup-ui.xml"
}

function Invoke-Adb([string[]]$Arguments) {
    # adb writes normal progress (notably `install --no-streaming`) to stderr
    # even when the command succeeds. Temporarily disable PowerShell's native
    # stderr-as-error promotion and decide success from the actual exit code.
    $previousErrorAction = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try { $result = & $adb @Arguments 2>&1 }
    finally { $ErrorActionPreference = $previousErrorAction }
    if ($LASTEXITCODE -ne 0) { throw "adb failed ($($Arguments -join ' ')): $result" }
    return $result
}

function Invoke-AdbWithTimeout([string[]]$Arguments, [int]$TimeoutSeconds = 120) {
    $token = [Guid]::NewGuid().ToString('N')
    $stdoutPath = Join-Path $outputPath ("adb-$token.stdout.txt")
    $stderrPath = Join-Path $outputPath ("adb-$token.stderr.txt")
    $process = Start-Process -FilePath $adb -ArgumentList $Arguments -WindowStyle Hidden `
        -RedirectStandardOutput $stdoutPath -RedirectStandardError $stderrPath -PassThru
    if (-not $process.WaitForExit($TimeoutSeconds * 1000)) {
        Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
        throw "adb timed out after ${TimeoutSeconds}s ($($Arguments -join ' ')); inspect $stderrPath"
    }
    if ($process.ExitCode -ne 0) {
        $details = (Get-Content -Raw -LiteralPath $stderrPath -ErrorAction SilentlyContinue)
        throw "adb failed ($($Arguments -join ' ')): $details"
    }
    return (Get-Content -Raw -LiteralPath $stdoutPath -ErrorAction SilentlyContinue)
}

try {
    $launcher = Start-Process -FilePath $emulator -ArgumentList @(
        '-avd', $Avd, '-no-snapshot', '-no-audio', '-no-boot-anim', '-no-window',
        '-cores', $Cores, '-memory', $MemoryMb, '-gpu', 'swiftshader_indirect'
    ) -PassThru -WindowStyle Hidden
    Set-BackgroundPriority $launcher
    $ready = $false
    for ($attempt = 0; $attempt -lt 90; $attempt++) {
        Start-Sleep -Seconds 2
        try { $state = (& $adb '-s' $Serial 'get-state' 2>$null | Out-String).Trim() } catch { $state = '' }
        try { $boot = (& $adb '-s' $Serial 'shell' 'getprop' 'sys.boot_completed' 2>$null | Out-String).Trim() } catch { $boot = '' }
        $packageManagerReady = $false
        if ($state -eq 'device' -and $boot -eq '1') {
            try {
                $pmProbe = (& $adb '-s' $Serial 'shell' 'pm' 'path' 'android' 2>$null | Out-String)
                $packageManagerReady = ($LASTEXITCODE -eq 0 -and $pmProbe -match 'package:')
            } catch { $packageManagerReady = $false }
        }
        if ($state -eq 'device' -and $boot -eq '1' -and $packageManagerReady) { $ready = $true; break }
    }
    if (-not $ready) { throw "Android emulator did not become ready: $Serial" }
    # Streaming installs are flaky when a physical handset is connected to
    # the same ADB server. The push-based path is slightly slower but avoids
    # an opaque empty install failure in unattended background runs.
    $installedPackage = ''
    try { $installedPackage = (& $adb '-s' $Serial 'shell' 'pm' 'path' $Package 2>$null | Out-String).Trim() } catch { $installedPackage = '' }
    if ($installedPackage -match '^package:') {
        Write-Warning "Package $Package is already installed; skipping a potentially blocking reinstall."
    } else {
        Invoke-AdbWithTimeout @('-s', $Serial, 'install', '--no-streaming', '-r', (Resolve-Path $Apk).Path) | Out-Null
    }
    Invoke-Adb @('-s', $Serial, 'shell', 'am', 'force-stop', $Package) | Out-Null
    & $adb '-s' $Serial 'logcat' '-c' 2>$null | Out-Null
    Invoke-Adb @('-s', $Serial, 'shell', 'monkey', '-p', $Package, '1') | Out-Null
    Start-Sleep -Seconds 8
    Wait-TorcaAppReady -PackageName $Package -ArtifactRoot $outputPath

    if ($ActiveMessaging) {
        $conversationRoot = Join-Path $outputPath 'conversation'
        New-Item -ItemType Directory -Force -Path $conversationRoot | Out-Null
        # The SOAK binary performs its own package/ADB preflight. Give the
        # emulator a short settling window so a just-woken transport is not
        # mistaken for an unauthorized/offline device.
        for ($probe = 0; $probe -lt 30; $probe++) {
            $adbProbe = Read-AdbText @('-s', $Serial, 'shell', 'pm', 'path', 'android')
            if ($adbProbe -match '(?m)^\s*package:') { break }
            Start-Sleep -Seconds 2
        }
        $soakArgs = @(
            'run', '-p', 'torca-soak', '--locked', '--',
            '--scenario', 'active-messaging',
            '--android', $Serial,
            '--fake-peers', $FakePeers,
            '--duration-seconds', $DurationSeconds,
            '--communication-provider', 'iroh',
            '--android-auto-deploy',
            '--require-screen-off',
            '--plain',
            '--output', $conversationRoot
        )
        & cargo @soakArgs
        if ($LASTEXITCODE -ne 0) {
            $logPath = Join-Path $conversationRoot 'last-failure.log'
            $log = Read-AdbText @('-s', $Serial, 'logcat', '-d', '-t', '1200', '-v', 'brief')
            Set-Content -LiteralPath $logPath -Value $log -Encoding utf8
            throw "Iroh active-messaging scenario failed; inspect $logPath"
        }
    }

    $power = (& $adb '-s' $Serial 'shell' 'dumpsys' 'power' 2>$null | Out-String)
    # KEYCODE_SLEEP is more reliable on headless API 35/36 images than the
    # toggle-style power key. Keep the toggle as a fallback for older images.
    foreach ($keyCode in @('223', '26', '223')) {
        if ($power -match 'mWakefulness=(Dozing|Asleep)') { break }
        try { Invoke-Adb @('-s', $Serial, 'shell', 'input', 'keyevent', $keyCode) | Out-Null } catch { }
        Start-Sleep -Seconds 2
        try { $power = (& $adb '-s' $Serial 'shell' 'dumpsys' 'power' 2>$null | Out-String) } catch { $power = '' }
    }
    Set-Content -LiteralPath (Join-Path $outputPath 'screen-power.txt') -Value $power -Encoding utf8
    if ($power -notmatch 'mWakefulness=(Dozing|Asleep)') {
        throw 'Could not establish screen-off state for background measurement.'
    }

    if (-not $NoHostCpuGuard) {
        $hostCpuSamples.Enqueue((Get-ProcessMachineCpuPercent (Get-EmulatorProcesses $launcher))
        )
        if ($hostCpuSamples.Count -gt 2) { [void]$hostCpuSamples.Dequeue() }
        if (@($hostCpuSamples | Where-Object { $_ -gt $MaxHostCpuPercent }).Count -ge 2) {
            throw "Emulator exceeded host CPU guard ($MaxHostCpuPercent%)."
        }
    }

    for ($repeat = 1; $repeat -le $Repetitions; $repeat++) {
        $report = Join-Path $outputPath ("{0}-{1}.json" -f $Profile, $repeat)
        & (Join-Path (Get-Location) 'scripts/measure-android-process-cpu.ps1') `
            -AndroidSerial $Serial -Package $Package -Provider iroh -Profile $Profile `
            -Mode background -DurationSeconds $DurationSeconds -Output $report
        if ($null -ne $LASTEXITCODE -and $LASTEXITCODE -ne 0) { throw "measurement failed: repetition $repeat" }
        $reports.Add((Get-Content -Raw -LiteralPath $report | ConvertFrom-Json))
        if (-not $NoHostCpuGuard) {
            $hostCpuSamples.Enqueue((Get-ProcessMachineCpuPercent (Get-EmulatorProcesses $launcher))
            )
            if ($hostCpuSamples.Count -gt 2) { [void]$hostCpuSamples.Dequeue() }
            if (@($hostCpuSamples | Where-Object { $_ -gt $MaxHostCpuPercent }).Count -ge 2) {
                throw "Emulator exceeded host CPU guard ($MaxHostCpuPercent%)."
            }
        }
    }
    $values = @($reports | ForEach-Object { [double]$_.summary.medianPercentOfOneLogicalCpu } | Sort-Object)
    $median = $values[[int][Math]::Floor(($values.Count - 1) / 2)]
    [ordered]@{
        schema = 1
        generatedAtUtc = [DateTime]::UtcNow.ToString('o')
        avd = $Avd
        serial = $Serial
        package = $Package
        profile = $Profile
        durationSeconds = $DurationSeconds
        repetitions = $Repetitions
        cores = $Cores
        memoryMb = $MemoryMb
        maxHostCpuPercent = if ($NoHostCpuGuard) { $null } else { $MaxHostCpuPercent }
        screenOffRequired = $true
        medianCpuPercentOfOneLogicalCpu = $median
        reports = @($reports | ForEach-Object { $_.summary })
    } | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $outputPath 'summary.json') -Encoding utf8
    Write-Host "Android emulator benchmark complete: median=$median% CPU / logical CPU"
} finally {
    if ($adb) { & $adb '-s' $Serial 'emu' 'kill' 2>$null | Out-Null }
    if ($null -ne $launcher -and -not $launcher.HasExited) { Stop-Process -Id $launcher.Id -Force }
}
