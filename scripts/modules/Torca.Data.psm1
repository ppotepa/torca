Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Reset-TorcaWindowsClientData {
    $processes = @(Get-Process torca_app -ErrorAction SilentlyContinue)
    foreach ($process in $processes) {
        $null = $process.CloseMainWindow()
        if (-not $process.WaitForExit(5000)) {
            throw "Torca desktop process $($process.Id) is still running; close it before resetting data."
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
