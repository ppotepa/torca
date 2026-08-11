[CmdletBinding()]
param(
    [ValidateRange(1, 100)][int]$LastRuns = 10,
    [ValidateSet('auto', 'windows', 'android', 'all')][string]$Target = 'all',
    [string[]]$Device,
    [ValidateSet('basic', 'extended', 'incident')][string]$Profile = 'extended',
    [switch]$IncludeLogcat,
    [switch]$SkipLogcat,
    [switch]$IncludeStackLogs,
    [switch]$SkipStackLogs,
    [switch]$KeepDirectory,
    [switch]$RemoveDirectoryAfterArchive
)

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
$moduleRoot = Join-Path $PSScriptRoot 'modules'
foreach ($module in @('Torca.Config', 'Torca.State', 'Torca.Devices')) {
    $modulePath = Join-Path $moduleRoot "$module.psm1"
    if (-not (Test-Path -LiteralPath $modulePath -PathType Leaf)) {
        throw "Torca module is missing: $modulePath"
    }
    Import-Module $modulePath -Force -ErrorAction Stop
}
foreach ($requiredCommand in @('Get-TorcaPaths', 'Initialize-TorcaPaths', 'Get-TorcaDevices')) {
    if (-not (Get-Command $requiredCommand -ErrorAction SilentlyContinue)) {
        throw "Torca collection module loader did not expose required command: $requiredCommand"
    }
}
$paths = Get-TorcaPaths -RepoRoot $repoRoot
Initialize-TorcaPaths -Paths $paths

$collectLogcat = -not $SkipLogcat -and ($Profile -ne 'basic' -or [bool]$IncludeLogcat)
$collectStack = -not $SkipStackLogs
if ($Profile -eq 'incident' -and -not $PSBoundParameters.ContainsKey('Profile')) {
    throw 'Incident profile must be selected explicitly with -Profile incident.'
}

$date = [DateTime]::UtcNow.ToString('yyyy-MM-dd')
$collectionParent = Join-Path $repoRoot 'logs/collected'
$dateRoot = Join-Path $collectionParent $date
New-Item -ItemType Directory -Force -Path $dateRoot | Out-Null
$nextNumber = @(Get-ChildItem -LiteralPath $dateRoot -Directory -ErrorAction SilentlyContinue |
    Where-Object Name -match '^collect-\d{6}$' |
    ForEach-Object { [int]$_.Name.Substring(8) } | Measure-Object -Maximum).Maximum
if (-not $nextNumber) { $nextNumber = 0 }
$collectionId = 'collect-{0:000000}' -f ($nextNumber + 1)
$collectRoot = Join-Path $dateRoot $collectionId
New-Item -ItemType Directory -Force -Path (Join-Path $collectRoot 'devices') | Out-Null

$deviceResults = [System.Collections.Generic.List[object]]::new()
$collectionErrors = [System.Collections.Generic.List[string]]::new()

function Add-Error {
    param([string]$Message)
    $collectionErrors.Add($Message)
    Write-Warning $Message
}

function Invoke-Capture {
    param([string]$Endpoint, [string[]]$Arguments, [string]$Destination, [string]$Label)
    try {
        $output = (& adb -s $Endpoint @Arguments 2>&1 | Out-String)
        $exitCode = $LASTEXITCODE
        Set-Content -LiteralPath $Destination -Value $output -Encoding utf8
        if ($exitCode -ne 0) { Add-Error "$Endpoint/$Label failed with exit code $exitCode"; return $false }
        return $true
    } catch {
        Add-Error "$Endpoint/$Label failed: $($_.Exception.Message)"
        return $false
    }
}

function Copy-RecentRuns {
    param([string]$SourceRoot, [string]$DestinationRoot)
    if (-not (Test-Path -LiteralPath $SourceRoot)) { return @() }
    $runs = @(Get-ChildItem -LiteralPath $SourceRoot -Recurse -Directory -ErrorAction SilentlyContinue |
        Where-Object { $_.Name -like 'run-*' -and (Test-Path (Join-Path $_.FullName 'run.start.json')) } |
        Sort-Object LastWriteTime -Descending | Select-Object -First $LastRuns)
    foreach ($run in $runs) {
        $dateDirectory = $run.Parent.Name
        $destination = Join-Path $DestinationRoot (Join-Path $dateDirectory $run.Name)
        New-Item -ItemType Directory -Force -Path (Split-Path -Parent $destination) | Out-Null
        Copy-Item -LiteralPath $run.FullName -Destination $destination -Recurse -Force
    }
    return @($runs | ForEach-Object { $_.FullName.Substring($SourceRoot.TrimEnd('\').Length).TrimStart('\').Replace('\', '/') })
}

function Get-SafeDeviceId {
    param([string]$Value)
    $safe = ($Value -replace '[^A-Za-z0-9_.-]', '_').Trim('_')
    if (-not $safe) { return 'unknown' }
    return $safe.Substring(0, [Math]::Min(80, $safe.Length))
}

function Get-FileSha256 {
    param([string]$Path)
    $sha = [System.Security.Cryptography.SHA256]::Create()
    try { return ([BitConverter]::ToString($sha.ComputeHash([IO.File]::ReadAllBytes($Path))).Replace('-', '')).ToLowerInvariant() }
    finally { $sha.Dispose() }
}

function Write-JsonFile {
    param([string]$Path, [object]$Value)
    $Value | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath $Path -Encoding utf8
}

function Prune-CollectionHistory {
    param([string]$Root, [string]$Current)
    $collections = @(Get-ChildItem -LiteralPath $Root -Recurse -Directory -ErrorAction SilentlyContinue |
        Where-Object { $_.Name -match '^collect-\d{6}$' -and $_.FullName -ne $Current } |
        Sort-Object LastWriteTime)
    $cutoff = [DateTime]::UtcNow.AddDays(-30)
    foreach ($collection in @($collections | Where-Object LastWriteTime -lt $cutoff)) {
        $zip = "$($collection.FullName).zip"
        Remove-Item -LiteralPath $collection.FullName -Recurse -Force -ErrorAction SilentlyContinue
        if (Test-Path -LiteralPath $zip) { Remove-Item -LiteralPath $zip -Force -ErrorAction SilentlyContinue }
    }
    $remaining = @(Get-ChildItem -LiteralPath $Root -Recurse -File -ErrorAction SilentlyContinue | Sort-Object LastWriteTime)
    $bytes = ($remaining | Measure-Object -Property Length -Sum).Sum
    while ($bytes -gt (10GB) -and $remaining.Count -gt 0) {
        $oldest = $remaining[0]
        $owner = $oldest.Directory
        while ($owner -and $owner.Name -notmatch '^collect-\d{6}$') { $owner = $owner.Parent }
        if (-not $owner -or $owner.FullName -eq $Current) { break }
        $ownerSize = (Get-ChildItem -LiteralPath $owner.FullName -Recurse -File -ErrorAction SilentlyContinue | Measure-Object -Property Length -Sum).Sum
        $zip = "$($owner.FullName).zip"
        Remove-Item -LiteralPath $owner.FullName -Recurse -Force -ErrorAction SilentlyContinue
        if (Test-Path -LiteralPath $zip) { Remove-Item -LiteralPath $zip -Force -ErrorAction SilentlyContinue }
        $bytes -= $ownerSize
        $remaining = @(Get-ChildItem -LiteralPath $Root -Recurse -File -ErrorAction SilentlyContinue | Sort-Object LastWriteTime)
    }
}

if ($collectStack) {
    $stackDestination = Join-Path $collectRoot 'host/stack'
    New-Item -ItemType Directory -Force -Path $stackDestination | Out-Null
    foreach ($file in @($paths.RelayLog, $paths.RelayErrorLog, $paths.StateFile, $paths.RelayEndpoint)) {
        if (Test-Path -LiteralPath $file) { Copy-Item -LiteralPath $file -Destination (Join-Path $stackDestination (Split-Path $file -Leaf)) -Force }
    }
    if ((Get-Command docker -ErrorAction SilentlyContinue) -and (Test-Path -LiteralPath $paths.DockerCompose)) {
        $dockerOutput = (& docker compose -f $paths.DockerCompose ps 2>&1 | Out-String)
        Set-Content -LiteralPath (Join-Path $stackDestination 'docker-compose.ps.log') -Value $dockerOutput -Encoding utf8
        $dockerLogs = (& docker compose -f $paths.DockerCompose logs --no-color --timestamps 2>&1 | Out-String)
        Set-Content -LiteralPath (Join-Path $stackDestination 'docker-compose.log') -Value $dockerLogs -Encoding utf8
        $dockerInspect = (& docker inspect torca-relay-1 2>&1 | Out-String)
        Set-Content -LiteralPath (Join-Path $stackDestination 'docker-inspect.log') -Value $dockerInspect -Encoding utf8
    }
}

$available = @(Get-TorcaDevices -FlutterRoot (Join-Path $repoRoot 'apps/client/flutter'))
$selected = if ($Device) { @($available | Where-Object { $Device -contains $_.Id }) } else { $available }
if ($Target -ne 'all' -and $Target -ne 'auto') { $selected = @($selected | Where-Object Platform -eq $Target) }

# ADB may expose one phone through USB, an IP endpoint and an mDNS alias.
# Keep one logical device and preserve every endpoint in its manifest.
$logicalDevices = [System.Collections.Generic.List[object]]::new()
$windows = @($selected | Where-Object Platform -eq 'windows')
foreach ($item in $windows) { $logicalDevices.Add([pscustomobject]@{ Platform = 'windows'; Id = 'windows-host'; Name = $env:COMPUTERNAME; State = $item.State; Endpoints = @(); SelectedEndpoint = $null; Items = @($item) }) }
$androidItems = @($selected | Where-Object Platform -eq 'android')
$androidGroups = @{}
foreach ($item in $androidItems) {
    $serial = $item.Id
    if ((Get-Command adb -ErrorAction SilentlyContinue) -and $item.State -eq 'device') {
        $reported = (& adb -s $item.Id shell getprop ro.serialno 2>$null | Out-String).Trim()
        if ($reported) { $serial = $reported }
    }
    if (-not $androidGroups.ContainsKey($serial)) { $androidGroups[$serial] = [System.Collections.Generic.List[object]]::new() }
    $androidGroups[$serial].Add($item)
}
foreach ($entry in $androidGroups.GetEnumerator()) {
    $items = @($entry.Value)
    $chosen = $items | Sort-Object @{Expression={ if ($_.Id -match '_adb-tls-|:\d+$') { 1 } else { 0 } }}, Id | Select-Object -First 1
    $logicalId = 'android-' + (Get-SafeDeviceId $entry.Key)
    $transport = if ($chosen.Id -match '_adb-tls-|:\d+$') { 'wireless' } else { 'usb-or-local' }
    $logicalDevices.Add([pscustomobject]@{ Platform = 'android'; Id = $logicalId; Name = $chosen.Name; State = $chosen.State; Endpoints = @($items | ForEach-Object Id); SelectedEndpoint = $chosen.Id; Transport = $transport; Items = $items })
}

foreach ($item in $logicalDevices) {
    $destination = Join-Path $collectRoot (Join-Path 'devices' (Get-SafeDeviceId $item.Id))
    New-Item -ItemType Directory -Force -Path $destination | Out-Null
    $result = [ordered]@{ id = $item.Id; platform = $item.Platform; name = $item.Name; state = $item.State; endpoints = @($item.Endpoints); selectedEndpoint = $item.SelectedEndpoint; transport = $item.Transport; profile = $Profile; collected = @(); errors = @() }
    try {
        if ($item.Platform -eq 'windows') {
            $localRoot = if ($env:LOCALAPPDATA) { Join-Path $env:LOCALAPPDATA 'Torca/logs' } else { $null }
            if ($localRoot -and (Test-Path (Join-Path $localRoot 'devices'))) {
                foreach ($sourceDevice in Get-ChildItem -LiteralPath (Join-Path $localRoot 'devices') -Directory -ErrorAction SilentlyContinue) {
                    $result.collected += Copy-RecentRuns -SourceRoot $sourceDevice.FullName -DestinationRoot $destination
                }
            }
            foreach ($file in @('torca-build.json', 'torca-artifact.json')) {
                $candidate = Join-Path $repoRoot "artifacts/$file"
                if (Test-Path $candidate) { Copy-Item $candidate (Join-Path $destination $file) -Force; $result.collected += $file }
            }
            $process = Get-Process -ErrorAction SilentlyContinue | Where-Object ProcessName -match 'torca|tor|relay' | Select-Object Id, ProcessName, Path, StartTime
            Write-JsonFile (Join-Path $destination 'process.snapshot.json') $process
            $result.collected += 'process.snapshot.json'
        } elseif (Get-Command adb -ErrorAction SilentlyContinue) {
            $endpoint = $item.SelectedEndpoint
            Invoke-Capture $endpoint @('shell', 'getprop') (Join-Path $destination 'device-properties.log') 'getprop' | Out-Null
            Invoke-Capture $endpoint @('shell', 'dumpsys', 'package', 'com.torca.torca_app') (Join-Path $destination 'package.log') 'package' | Out-Null
            Invoke-Capture $endpoint @('shell', 'dumpsys', 'activity', 'services') (Join-Path $destination 'services.log') 'services' | Out-Null
            Invoke-Capture $endpoint @('shell', 'ps', '-A') (Join-Path $destination 'processes.log') 'processes' | Out-Null
            Invoke-Capture $endpoint @('shell', 'dumpsys', 'meminfo', 'com.torca.torca_app') (Join-Path $destination 'memory.log') 'memory' | Out-Null
            Invoke-Capture $endpoint @('shell', 'dumpsys', 'connectivity') (Join-Path $destination 'connectivity.log') 'connectivity' | Out-Null
            Invoke-Capture $endpoint @('shell', 'dumpsys', 'activity', 'top') (Join-Path $destination 'activity-top.log') 'activity top' | Out-Null
            Invoke-Capture $endpoint @('shell', 'dumpsys', 'window', 'policy') (Join-Path $destination 'lock-state.log') 'lock state' | Out-Null
            Invoke-Capture $endpoint @('shell', 'getprop', 'ro.product.cpu.abilist') (Join-Path $destination 'abi.log') 'ABI' | Out-Null
            $result.collected += @('device-properties.log', 'package.log', 'services.log', 'processes.log', 'memory.log', 'connectivity.log', 'activity-top.log', 'lock-state.log', 'abi.log')
            if ($collectLogcat) {
                $appPid = (& adb -s $endpoint shell pidof com.torca.torca_app 2>$null | Out-String).Trim()
                $logcatArgs = @('logcat', '-d', '-v', 'threadtime', '-b', 'main', '-b', 'system', '-b', 'crash')
                if ($appPid) { $logcatArgs += "--pid=$appPid" }
                if (Invoke-Capture $endpoint $logcatArgs (Join-Path $destination 'android-logcat.log') 'filtered logcat') { $result.collected += 'android-logcat.log' }
                if (Invoke-Capture $endpoint @('logcat', '-b', 'crash', '-d', '-v', 'threadtime') (Join-Path $destination 'android-crash.log') 'crash log') { $result.collected += 'android-crash.log' }
            }
            $remoteLogs = '/sdcard/Android/data/com.torca.torca_app/files/torca/logs'
            # Android's scoped storage can deny `adb shell test -d` even though
            # `adb pull` is permitted for the app-owned external directory.
            # Attempt the transfer directly and retain its output for diagnosis.
            $pullDestination = Join-Path $destination '_adb-pull'
            New-Item -ItemType Directory -Force -Path $pullDestination | Out-Null
            $pullOutput = (& adb -s $endpoint pull $remoteLogs $pullDestination 2>&1 | Out-String)
            $pullExitCode = $LASTEXITCODE
            Set-Content -LiteralPath (Join-Path $destination 'android-runtime-pull.log') -Value $pullOutput -Encoding utf8
            if ($pullExitCode -eq 0) {
                $pulledRoot = Join-Path $pullDestination 'logs'
                if (-not (Test-Path $pulledRoot)) { $pulledRoot = $pullDestination }
                $pulledDevices = Join-Path $pulledRoot 'devices'
                if (Test-Path $pulledDevices) {
                    foreach ($sourceDevice in Get-ChildItem -LiteralPath $pulledDevices -Directory -ErrorAction SilentlyContinue) {
                        $result.collected += Copy-RecentRuns -SourceRoot $sourceDevice.FullName -DestinationRoot $destination
                    }
                } else {
                    $result.collected += Copy-RecentRuns -SourceRoot $pulledRoot -DestinationRoot $destination
                }
                Remove-Item -LiteralPath $pullDestination -Recurse -Force
            } else {
                Remove-Item -LiteralPath $pullDestination -Recurse -Force -ErrorAction SilentlyContinue
                # Recent Android builds keep runtime logs in the app-private
                # files directory. The external scoped-storage path above is
                # optional and often does not exist. Debuggable packages can
                # still expose the same files through run-as, which gives the
                # incident collector the native attachment/relay diagnostics
                # instead of silently returning an empty archive.
                $internalFiles = @(& adb -s $endpoint shell run-as com.torca.torca_app find files/torca/logs -type f 2>$null |
                    ForEach-Object { $_.ToString().Trim() } |
                    Where-Object { $_ -match '^files/torca/logs/.+' })
                if ($internalFiles.Count -gt 0) {
                    $internalRoot = Join-Path $destination 'internal-runtime-logs'
                    foreach ($relativeFile in $internalFiles) {
                        $relativeName = $relativeFile.Substring('files/torca/logs/'.Length).Replace('/', '\')
                        $localFile = Join-Path $internalRoot $relativeName
                        New-Item -ItemType Directory -Force -Path (Split-Path -Parent $localFile) | Out-Null
                        & adb -s $endpoint exec-out run-as com.torca.torca_app cat $relativeFile > $localFile
                        if ($LASTEXITCODE -eq 0) { $result.collected += "internal-runtime-logs/$($relativeName.Replace('\', '/'))" }
                    }
                } else {
                    $lockState = Get-Content -LiteralPath (Join-Path $destination 'lock-state.log') -Raw -ErrorAction SilentlyContinue
                    if ($lockState -match '(?m)\bshowing\s*=\s*true\b') {
                        $result.errors += "$endpoint/runtime logs unavailable because the Android device is locked; unlock it and rerun collection"
                    } else {
                        $result.errors += "$endpoint/runtime logs pull failed (exit $pullExitCode) and no private logs were accessible"
                    }
                }
            }
        }
    } catch { $result.errors += $_.Exception.Message; Add-Error "$($item.Id): $($_.Exception.Message)" }
    $deviceResults.Add([pscustomobject]$result)
}

$manifest = [ordered]@{ schema = 2; collectionId = $collectionId; collectedAt = [DateTime]::UtcNow.ToString('o'); lastRuns = $LastRuns; profile = $Profile; includeLogcat = $collectLogcat; includeStackLogs = $collectStack; devices = @($deviceResults); errors = @($collectionErrors) }
Write-JsonFile (Join-Path $collectRoot 'collection-manifest.json') $manifest
Set-Content -LiteralPath (Join-Path $collectRoot 'collector.log') -Value ("Torca diagnostic collection $collectionId`nProfile: $Profile`nCollected: $([DateTime]::UtcNow.ToString('o'))") -Encoding utf8
Set-Content -LiteralPath (Join-Path $collectRoot 'collector-errors.jsonl') -Value ($collectionErrors | ForEach-Object { '{"schema":1,"message":"' + (($_ -replace '\\', '\\') -replace '"', '\"') + '"}' }) -Encoding utf8

$checksumFile = Join-Path $collectRoot 'checksums.sha256'
$lines = foreach ($file in Get-ChildItem -LiteralPath $collectRoot -Recurse -File | Where-Object Name -ne 'checksums.sha256' | Sort-Object FullName) {
    "$(Get-FileSha256 -Path $file.FullName)  $($file.FullName.Substring($collectRoot.Length + 1).Replace('\', '/'))"
}
Set-Content -LiteralPath $checksumFile -Value $lines -Encoding ascii
$zip = "$collectRoot.zip"
Compress-Archive -Path (Join-Path $collectRoot '*') -DestinationPath $zip -Force
Write-Host "Diagnostics package: $zip" -ForegroundColor Green
if ($RemoveDirectoryAfterArchive -and -not $KeepDirectory) { Remove-Item -LiteralPath $collectRoot -Recurse -Force }
Prune-CollectionHistory -Root $collectionParent -Current $collectRoot
Write-Output $zip
