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
    $packageName = if ($env:TORCA_ANDROID_PACKAGE) { $env:TORCA_ANDROID_PACKAGE } else { 'com.torca.torca_app' }
    & adb -s $DeviceId shell pm clear $packageName
    if ($LASTEXITCODE -ne 0) { throw "Unable to clear Android data on device $DeviceId." }
    Write-Host "Cleared Android client data on: $DeviceId" -ForegroundColor Yellow
}

function Reset-TorcaClientData {
    param([Parameter(Mandatory = $true)][object[]]$Devices)
    foreach ($device in $Devices) {
        if ($device.Platform -eq 'windows') { Reset-TorcaWindowsClientData }
        elseif ($device.Platform -eq 'android') { Reset-TorcaAndroidClientData -DeviceId $device.Id }
    }
}

Export-ModuleMember -Function Reset-TorcaClientData
