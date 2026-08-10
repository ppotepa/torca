Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Reset-TorcaWindowsClientData {
    $processes = @(Get-Process torca_app -ErrorAction SilentlyContinue)
    foreach ($process in $processes) {
        $null = $process.CloseMainWindow()
        if (-not $process.WaitForExit(5000)) {
            # Normal desktop close deliberately keeps the runtime alive in the tray.
            # A confirmed data reset is an explicit terminal action, so finish that
            # process here before removing its verified application directory.
            Write-Warning "Stopping Torca desktop process $($process.Id) for the confirmed data reset."
            Stop-Process -Id $process.Id -Force -ErrorAction Stop
            $process.WaitForExit(5000)
            if (-not $process.HasExited) {
                throw "Torca desktop process $($process.Id) could not be stopped for data reset."
            }
        }
    }
    $root = Join-Path $env:LOCALAPPDATA 'Torca'
    if (-not (Test-Path -LiteralPath $root)) { return }
    Remove-Item -LiteralPath $root -Recurse -Force
    Write-Host "Removed Windows client data: $root" -ForegroundColor Yellow
}

function Reset-TorcaAndroidClientData {
    param([Parameter(Mandatory = $true)][string]$DeviceId)
    if (-not (Get-Command adb -ErrorAction SilentlyContinue)) { throw 'adb is required to reset Android client data.' }
    Ensure-TorcaAndroidDeviceOnline -DeviceId $DeviceId
    $packageName = if ($env:TORCA_ANDROID_PACKAGE) { $env:TORCA_ANDROID_PACKAGE } else { 'com.torca.torca_app' }
    & adb -s $DeviceId shell am force-stop $packageName *> $null
    $clearOutput = (& adb -s $DeviceId shell pm clear $packageName 2>&1 | Out-String).Trim()
    if ($LASTEXITCODE -eq 0 -and $clearOutput -match '(?i)^(Success|new data cleared)$') {
        Write-Host "Cleared Android client data on: $DeviceId" -ForegroundColor Yellow
        return
    }

    # Some OEM Android builds deny pm clear to the adb shell even for a
    # user-installed package (CLEAR_APP_USER_DATA). Removing the package for
    # user 0 is the equivalent recoverable reset; deploy installs the APK again
    # immediately afterwards. Keep the original error in the diagnostic.
    if ($clearOutput -match '(?i)CLEAR_APP_USER_DATA|SecurityException|not permitted|Exception occurred') {
        Write-Host "pm clear denied on $DeviceId; falling back to package removal for user 0." -ForegroundColor Yellow
        $removeOutput = (& adb -s $DeviceId shell pm uninstall --user 0 $packageName 2>&1 | Out-String).Trim()
        if ($LASTEXITCODE -eq 0 -and $removeOutput -match '(?i)^Success') {
            Write-Host "Removed Android package and local data on: $DeviceId" -ForegroundColor Yellow
            return
        }
        # `pm uninstall --user 0` reports this when a previous install/reset
        # has already removed the package from the primary user.  There is no
        # app sandbox left to clear, so this is a successful idempotent reset;
        # the next deploy installs a fresh package.
        if ($removeOutput -match '(?i)not installed for (user )?0') {
            Write-Host "Android package was already absent for user 0 on: $DeviceId; local data is already reset." -ForegroundColor Yellow
            return
        }
        throw "Android data reset was denied on $DeviceId. pm clear: $clearOutput; package removal fallback: $removeOutput"
    }

    throw "Unable to clear Android data on device $DeviceId. Details: $clearOutput"
}

function Ensure-TorcaAndroidDeviceOnline {
    param([Parameter(Mandatory = $true)][string]$DeviceId)
    $state = (& adb -s $DeviceId get-state 2>&1 | Out-String).Trim()
    if ($LASTEXITCODE -eq 0 -and $state -eq 'device') { return }

    # Wireless-debug endpoints can expire between device selection and reset.
    # Reconnect once and use a bounded readiness wait; never wait forever nor
    # clear any other device by falling back to an unqualified adb command.
    if ($DeviceId -match ':\d+$') {
        Write-Host "Android endpoint $DeviceId is $state; reconnecting wireless debugging." -ForegroundColor Yellow
        & adb connect $DeviceId 2>&1 | Out-Null
        $deadline = [DateTime]::UtcNow.AddSeconds(10)
        do {
            Start-Sleep -Milliseconds 500
            $state = (& adb -s $DeviceId get-state 2>&1 | Out-String).Trim()
            if ($LASTEXITCODE -eq 0 -and $state -eq 'device') { return }
        } while ([DateTime]::UtcNow -lt $deadline)
    }
    throw "Android device $DeviceId is not ready for a data reset (adb state: $state). Reconnect/authorize wireless debugging and retry; no selected device data was changed."
}

function Reset-TorcaClientData {
    param([Parameter(Mandatory = $true)][object[]]$Devices)
    # Verify every Android target before changing any local data.  This avoids
    # a partially reset multi-device deployment when Wi-Fi debugging drops.
    foreach ($device in $Devices) {
        if ($device.Platform -eq 'android') { Ensure-TorcaAndroidDeviceOnline -DeviceId $device.Id }
    }
    foreach ($device in $Devices) {
        if ($device.Platform -eq 'windows') { Reset-TorcaWindowsClientData }
        elseif ($device.Platform -eq 'android') { Reset-TorcaAndroidClientData -DeviceId $device.Id }
    }
}

Export-ModuleMember -Function Reset-TorcaClientData
