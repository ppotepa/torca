Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Get-TorcaDevices {
    param([Parameter(Mandatory = $true)][string]$FlutterRoot)
    $devices = [System.Collections.Generic.List[object]]::new()
    $androidDevicesByIdentity = @{}
    if ($env:OS -eq 'Windows_NT') {
        $devices.Add([pscustomobject]@{ Id = 'windows'; Platform = 'windows'; Kind = 'desktop'; Name = $env:COMPUTERNAME; State = 'online'; Architecture = 'x64'; CanInstall = $true; CanRun = $true })
    }

    # Discover paired wireless-debugging devices before reading the connected list.
    # `adb devices` only reports endpoints that are already connected.
    if (Get-Command adb -ErrorAction SilentlyContinue) {
        $mdnsOutput = & adb mdns services 2>$null | Out-String
        if ($LASTEXITCODE -eq 0) {
            $connectedServices = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::OrdinalIgnoreCase)
            foreach ($line in ($mdnsOutput -split "`r?`n")) {
                if ($line -match '^(?<service>\S+)\s+_adb-tls-connect\._tcp\s+(?<endpoint>\S+)') {
                    $service = ($Matches.service -replace '\s*\(\d+\)$', '')
                    if ($connectedServices.Add($service)) {
                        & adb connect $Matches.endpoint 2>$null | Out-Null
                    }
                }
            }
        }
    }

    # Flutter can report some mDNS wireless-debugging devices as unsupported
    # (notably ids containing " (2)._adb-tls-connect._tcp"). Read adb directly
    # so those devices remain deployable and selectable.
    if (Get-Command adb -ErrorAction SilentlyContinue) {
        $adbOutput = & adb devices -l 2>$null | Out-String
        if ($LASTEXITCODE -eq 0) {
            foreach ($line in ($adbOutput -split "`r?`n")) {
                if ($line -match '^(?<id>.+?)\s+(?<state>device|offline|unauthorized|unknown|no permissions)(?:\s+(?<details>.*))?$') {
                    $id = $Matches.id.Trim()
                    $state = $Matches.state.Trim()
                    $details = $Matches.details
                    if (-not $id) { continue }

                    $model = if ($details -match 'model:(\S+)') { $Matches[1] } else { $id }
                    $sdk = ''
                    $serial = $id
                    $reportedSerial = ''
                    if ($state -eq 'device') {
                        $sdk = (& adb -s $id shell getprop ro.build.version.sdk 2>$null | Out-String).Trim()
                        $reportedSerial = (& adb -s $id shell getprop ro.serialno 2>$null | Out-String).Trim()
                        if ($reportedSerial) { $serial = $reportedSerial }
                    }
                    $wireless = $id -match '_adb-tls-|:\d+$'
                    $kind = if ($id -like 'emulator-*' -or $details -match 'product:(sdk|generic)') { 'emulator' } elseif ($wireless) { 'android-wireless' } else { 'android' }
                    $online = $state -eq 'device'
                    $identity = if ($reportedSerial) { $reportedSerial } else { $id }
                    $connectionRank = if ($kind -eq 'android') { 0 } elseif ($id -match ':\d+$') { 1 } else { 2 }
                    $candidate = [pscustomobject]@{
                        Id = $id; Platform = 'android'; Kind = $kind; Name = $model; State = $state
                        Architecture = if ($sdk) { "android-api-$sdk" } else { '' }
                        CanInstall = $online; CanRun = $online
                    }
                    if ($androidDevicesByIdentity.ContainsKey($identity)) {
                        $existing = $androidDevicesByIdentity[$identity]
                        if ($connectionRank -ge $existing.ConnectionRank) { continue }
                        $devices.Remove($existing) | Out-Null
                    }
                    Add-Member -InputObject $candidate -NotePropertyName ConnectionRank -NotePropertyValue $connectionRank
                    $androidDevicesByIdentity[$identity] = $candidate
                    $devices.Add($candidate)
                }
            }
        }
    }

    Push-Location $FlutterRoot
    try {
        try { $json = & flutter devices --machine 2>$null | Out-String }
        catch { $json = '' }
    } finally { Pop-Location }
    if ($LASTEXITCODE -eq 0 -and $json.Trim()) {
        try {
            foreach ($device in ($json | ConvertFrom-Json)) {
                if ([string]$device.targetPlatform -like 'android*' -and -not ($androidDevicesByIdentity.ContainsKey([string]$device.id))) {
                    $kind = if ([string]$device.category -eq 'mobile') { 'android' } else { 'emulator' }
                    $devices.Add([pscustomobject]@{ Id = [string]$device.id; Platform = 'android'; Kind = $kind; Name = [string]$device.name; State = 'online'; Architecture = [string]$device.targetPlatform; CanInstall = $true; CanRun = $true })
                }
            }
        } catch {
            # ADB is the authoritative Android inventory. Flutter output is
            # best-effort because plugins may prepend diagnostics to machine JSON.
        }
    }
    return @($devices.ToArray())
}

function Select-TorcaDevices {
    param([Parameter(Mandatory = $true)][object[]]$Devices)
    $deployable = @($Devices | Where-Object { $_.CanInstall -and $_.CanRun })
    if ($deployable.Count -eq 0) { throw 'No online deployable devices were detected.' }
    for ($index = 0; $index -lt $Devices.Count; $index++) {
        $availability = if ($Devices[$index].CanInstall -and $Devices[$index].CanRun) { 'deployable' } else { 'not deployable' }
        Write-Host ("[{0}] {1} - {2} [{3}; {5}] ({4})" -f ($index + 1), $Devices[$index].Platform, $Devices[$index].Name, $Devices[$index].State, $Devices[$index].Id, $availability)
    }
    $answer = Read-Host 'Select devices by number separated with commas, or A for all'
    if ($answer.Trim().ToUpperInvariant() -eq 'A') { return $deployable }
    $selected = foreach ($value in $answer -split ',') {
        $number = 0
        if ([int]::TryParse($value.Trim(), [ref]$number) -and $number -ge 1 -and $number -le $Devices.Count) {
            if ($Devices[$number - 1].CanInstall -and $Devices[$number - 1].CanRun) {
                $Devices[$number - 1]
            }
        }
    }
    if (-not $selected) { throw 'No valid devices selected.' }
    return @($selected | Sort-Object Id -Unique)
}

Export-ModuleMember -Function Get-TorcaDevices, Select-TorcaDevices
