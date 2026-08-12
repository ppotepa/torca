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
$sourcesRoot = Join-Path $collectRoot 'sources'
New-Item -ItemType Directory -Force -Path $sourcesRoot | Out-Null
$collectionStartedAt = [DateTime]::UtcNow

$deviceResults = [System.Collections.Generic.List[object]]::new()
$collectionErrors = [System.Collections.Generic.List[string]]::new()
$collectionWarnings = [System.Collections.Generic.List[string]]::new()

function Add-Error {
    param([string]$Message)
    $collectionErrors.Add($Message)
    Write-Warning $Message
}

function Add-Warning {
    param([string]$Message)
    $collectionWarnings.Add($Message)
    Write-Warning $Message
}

function Invoke-NativeText {
    param(
        [Parameter(Mandatory = $true)][string]$Executable,
        [string[]]$Arguments = @()
    )
    # Windows PowerShell promotes native stderr records to errors when the
    # script uses ErrorActionPreference=Stop. adb pull and Docker Compose both
    # write normal progress to stderr, so the exit code—not the stream—is the
    # only reliable success signal.
    $previousPreference = $ErrorActionPreference
    try {
        $ErrorActionPreference = 'Continue'
        $lines = @(& $Executable @Arguments 2>&1 | ForEach-Object { $_.ToString() })
        $exitCode = $LASTEXITCODE
        [pscustomobject]@{
            exitCode = $exitCode
            output = ($lines -join [Environment]::NewLine)
        }
    } finally {
        $ErrorActionPreference = $previousPreference
    }
}

function Invoke-Capture {
    param([string]$Endpoint, [string[]]$Arguments, [string]$Destination, [string]$Label)
    try {
        $capture = Invoke-NativeText -Executable 'adb' -Arguments (@('-s', $Endpoint) + $Arguments)
        Set-Content -LiteralPath $Destination -Value $capture.output -Encoding utf8
        if ($capture.exitCode -ne 0) { Add-Error "$Endpoint/$Label failed with exit code $($capture.exitCode)"; return $false }
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

function Get-SourceFileMetadata {
    param(
        [Parameter(Mandatory = $true)][string]$Source,
        [Parameter(Mandatory = $true)][string]$Destination
    )
    $item = Get-Item -LiteralPath $Source -ErrorAction Stop
    $modifiedUtc = $item.LastWriteTimeUtc
    [pscustomobject]@{
        source = $item.FullName
        destination = $Destination.Replace('\', '/')
        bytes = $item.Length
        modifiedAt = $modifiedUtc.ToString('o')
        ageSecondsAtCollection = [Math]::Max(0, [int]($collectionStartedAt - $modifiedUtc).TotalSeconds)
    }
}

function Get-RepositoryIdentity {
    $commit = $null
    $dirty = $null
    if (Get-Command git -ErrorAction SilentlyContinue) {
        $commitResult = Invoke-NativeText -Executable 'git' -Arguments @('-C', $repoRoot, 'rev-parse', 'HEAD')
        if ($commitResult.exitCode -eq 0) { $commit = $commitResult.output.Trim() }
        $statusResult = Invoke-NativeText -Executable 'git' -Arguments @('-C', $repoRoot, 'status', '--porcelain=v1')
        if ($statusResult.exitCode -eq 0) { $dirty = -not [string]::IsNullOrWhiteSpace($statusResult.output) }
    }
    [pscustomobject]@{
        commit = $commit
        dirty = $dirty
        collectScriptSha256 = Get-FileSha256 -Path $PSCommandPath
    }
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
    $relayDestination = Join-Path $sourcesRoot 'relay'
    $relayStateDestination = Join-Path $relayDestination 'state'
    $relayLiveDestination = Join-Path $relayDestination 'live'
    $deployStateDestination = Join-Path $sourcesRoot 'deploy/state'
    foreach ($directory in @($relayStateDestination, $relayLiveDestination, $deployStateDestination)) {
        New-Item -ItemType Directory -Force -Path $directory | Out-Null
    }
    $stackSources = [System.Collections.Generic.List[object]]::new()
    foreach ($file in @($paths.RelayEndpoint, $paths.RelayReady)) {
        if (Test-Path -LiteralPath $file -PathType Leaf) {
            $name = Split-Path $file -Leaf
            $destination = Join-Path $relayStateDestination $name
            Copy-Item -LiteralPath $file -Destination $destination -Force
            $stackSources.Add((Get-SourceFileMetadata -Source $file -Destination "sources/relay/state/$name"))
        }
    }
    foreach ($file in @($paths.StateFile, $paths.LastDeployFile, $paths.ManifestFile, (Join-Path $paths.RuntimeRoot 'deploy/current.json'))) {
        if (Test-Path -LiteralPath $file -PathType Leaf) {
            $name = Split-Path $file -Leaf
            if ($file -eq (Join-Path $paths.RuntimeRoot 'deploy/current.json')) { $name = 'deploy-current.json' }
            $destination = Join-Path $deployStateDestination $name
            Copy-Item -LiteralPath $file -Destination $destination -Force
            $stackSources.Add((Get-SourceFileMetadata -Source $file -Destination "sources/deploy/state/$name"))
        }
    }
    foreach ($directory in @($paths.BuildManifestRoot, $paths.DeviceManifestRoot)) {
        if (Test-Path -LiteralPath $directory -PathType Container) {
            $directoryName = Split-Path $directory -Leaf
            $destinationRoot = Join-Path $deployStateDestination $directoryName
            New-Item -ItemType Directory -Force -Path $destinationRoot | Out-Null
            foreach ($file in Get-ChildItem -LiteralPath $directory -File -ErrorAction SilentlyContinue) {
                $destination = Join-Path $destinationRoot $file.Name
                Copy-Item -LiteralPath $file.FullName -Destination $destination -Force
                $stackSources.Add((Get-SourceFileMetadata -Source $file.FullName -Destination "sources/deploy/state/$directoryName/$($file.Name)"))
            }
        }
    }
    $rustDeployRuns = Join-Path $paths.RuntimeRoot 'deploy/runs'
    if (Test-Path -LiteralPath $rustDeployRuns -PathType Container) {
        $runManifests = @(Get-ChildItem -LiteralPath $rustDeployRuns -File -Filter '*.json' -ErrorAction SilentlyContinue |
            Where-Object { $_.Name -notlike '*.events.json' } |
            Sort-Object LastWriteTimeUtc -Descending |
            Select-Object -First $LastRuns)
        foreach ($runManifest in $runManifests) {
            $runId = $runManifest.BaseName
            $runDestination = Join-Path $sourcesRoot (Join-Path 'deploy/runs/rust' (Get-SafeDeviceId $runId))
            New-Item -ItemType Directory -Force -Path $runDestination | Out-Null
            Copy-Item -LiteralPath $runManifest.FullName -Destination (Join-Path $runDestination 'run.json') -Force
            $stackSources.Add((Get-SourceFileMetadata -Source $runManifest.FullName -Destination "sources/deploy/runs/rust/$runId/run.json"))
            $events = Join-Path $rustDeployRuns "$runId.events.jsonl"
            if (Test-Path -LiteralPath $events -PathType Leaf) {
                Copy-Item -LiteralPath $events -Destination (Join-Path $runDestination 'events.jsonl') -Force
                $stackSources.Add((Get-SourceFileMetadata -Source $events -Destination "sources/deploy/runs/rust/$runId/events.jsonl"))
            }
        }
    }
    $persistedLogRoot = Join-Path $paths.RuntimeRoot 'logs'
    if (Test-Path -LiteralPath $persistedLogRoot -PathType Container) {
        $persistedDestination = Join-Path $relayDestination 'persisted'
        New-Item -ItemType Directory -Force -Path $persistedDestination | Out-Null
        foreach ($file in Get-ChildItem -LiteralPath $persistedLogRoot -File -ErrorAction SilentlyContinue) {
            $destination = Join-Path $persistedDestination $file.Name
            Copy-Item -LiteralPath $file.FullName -Destination $destination -Force
            $stackSources.Add((Get-SourceFileMetadata -Source $file.FullName -Destination "sources/relay/persisted/$($file.Name)"))
        }
    }
    if ((Get-Command docker -ErrorAction SilentlyContinue) -and (Test-Path -LiteralPath $paths.DockerCompose)) {
        $dockerOutput = Invoke-NativeText -Executable 'docker' -Arguments @('compose', '-f', $paths.DockerCompose, 'ps')
        Set-Content -LiteralPath (Join-Path $relayLiveDestination 'docker-compose.ps.log') -Value $dockerOutput.output -Encoding utf8
        $dockerLogs = Invoke-NativeText -Executable 'docker' -Arguments @('compose', '-f', $paths.DockerCompose, 'logs', '--no-color', '--timestamps')
        Set-Content -LiteralPath (Join-Path $relayLiveDestination 'docker-compose.log') -Value $dockerLogs.output -Encoding utf8
        $dockerInspect = Invoke-NativeText -Executable 'docker' -Arguments @('inspect', 'torca-relay-1')
        Set-Content -LiteralPath (Join-Path $relayLiveDestination 'docker-inspect.log') -Value $dockerInspect.output -Encoding utf8
    }
    $configuredEndpoint = if (Test-Path -LiteralPath $paths.RelayEndpoint) { (Get-Content -LiteralPath $paths.RelayEndpoint -Raw).Trim() } else { $null }
    $readyEndpoint = if (Test-Path -LiteralPath $paths.RelayReady) { (Get-Content -LiteralPath $paths.RelayReady -Raw).Trim() } else { $null }
    $persistedRelayText = if (Test-Path -LiteralPath $paths.RelayLog) { Get-Content -LiteralPath $paths.RelayLog -Raw } else { '' }
    $persistedEndpoints = @([regex]::Matches($persistedRelayText, '[a-z2-7]{56}\.onion:\d+') | ForEach-Object Value | Select-Object -Unique)
    $endpointConsistency = [ordered]@{
        configured = $configuredEndpoint
        ready = $readyEndpoint
        configuredMatchesReady = (-not $configuredEndpoint -or -not $readyEndpoint -or $configuredEndpoint -eq $readyEndpoint)
        persistedRelayLogEndpoints = $persistedEndpoints
        persistedRelayLogMatchesConfigured = ($persistedEndpoints.Count -eq 0 -or $persistedEndpoints -contains $configuredEndpoint)
    }
    Write-JsonFile (Join-Path $relayStateDestination 'endpoint-consistency.json') $endpointConsistency
    if (-not $endpointConsistency.configuredMatchesReady) {
        Add-Error "Relay endpoint state is inconsistent: configured=$configuredEndpoint ready=$readyEndpoint"
    }
    if (-not $endpointConsistency.persistedRelayLogMatchesConfigured) {
        Add-Warning "Persisted relay.log belongs to another endpoint and is stale for this deployment: configured=$configuredEndpoint logged=$($persistedEndpoints -join ',')"
    }
    Write-JsonFile (Join-Path $collectRoot 'source-origins.json') @($stackSources)
}

$discoveryRoot = Join-Path $sourcesRoot 'host/discovery'
New-Item -ItemType Directory -Force -Path $discoveryRoot | Out-Null
if (Get-Command adb -ErrorAction SilentlyContinue) {
    $adbDiscovery = Invoke-NativeText -Executable 'adb' -Arguments @('devices', '-l')
    Set-Content -LiteralPath (Join-Path $discoveryRoot 'adb-devices.log') -Value $adbDiscovery.output -Encoding utf8
} else {
    Set-Content -LiteralPath (Join-Path $discoveryRoot 'adb-devices.log') -Value 'adb executable was not found' -Encoding utf8
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
    $platformName = if ($item.Platform -eq 'windows') { 'windows' } else { 'android' }
    $deviceName = Get-SafeDeviceId $item.Id
    $destination = Join-Path $sourcesRoot (Join-Path "clients/$platformName" $deviceName)
    $platformDestination = Join-Path $destination 'platform'
    $runtimeDestination = Join-Path $destination 'runtime'
    New-Item -ItemType Directory -Force -Path $destination | Out-Null
    $result = [ordered]@{ id = $item.Id; platform = $item.Platform; name = $item.Name; state = $item.State; endpoints = @($item.Endpoints); selectedEndpoint = $item.SelectedEndpoint; transport = $item.Transport; profile = $Profile; collected = @(); errors = @() }
    try {
        if ($item.Platform -eq 'windows') {
            $localRoot = if ($env:LOCALAPPDATA) { Join-Path $env:LOCALAPPDATA 'Torca/logs' } else { $null }
            if ($localRoot -and (Test-Path -LiteralPath $localRoot)) {
                $localDevicesRoot = Join-Path $localRoot 'devices'
                if (Test-Path -LiteralPath $localDevicesRoot) {
                    foreach ($sourceDevice in Get-ChildItem -LiteralPath $localDevicesRoot -Directory -ErrorAction SilentlyContinue) {
                        if ($sourceDevice.Name -eq 'windows-host') {
                            $deployRuns = Join-Path $sourcesRoot 'deploy/runs/windows-host'
                            $runs = @(Copy-RecentRuns -SourceRoot $sourceDevice.FullName -DestinationRoot $deployRuns)
                            $result.collected += @($runs | ForEach-Object { "sources/deploy/runs/windows-host/$_" })
                        } else {
                            $runtimeRoot = Join-Path $runtimeDestination (Get-SafeDeviceId $sourceDevice.Name)
                            $runs = @(Copy-RecentRuns -SourceRoot $sourceDevice.FullName -DestinationRoot $runtimeRoot)
                            $result.collected += @($runs | ForEach-Object { "sources/clients/windows/$deviceName/runtime/$($sourceDevice.Name)/$_" })
                        }
                    }
                } else {
                    # Older Windows builds wrote run directories directly under
                    # Torca/logs. Do not silently discard those logs just
                    # because the newer devices/ nesting is absent.
                    $legacyRuntime = Join-Path $runtimeDestination 'legacy'
                    $runs = @(Copy-RecentRuns -SourceRoot $localRoot -DestinationRoot $legacyRuntime)
                    $result.collected += @($runs | ForEach-Object { "sources/clients/windows/$deviceName/runtime/legacy/$_" })
                }
                $directLogs = Join-Path $runtimeDestination 'legacy-files'
                foreach ($file in Get-ChildItem -LiteralPath $localRoot -File -ErrorAction SilentlyContinue) {
                    New-Item -ItemType Directory -Force -Path $directLogs | Out-Null
                    Copy-Item -LiteralPath $file.FullName -Destination (Join-Path $directLogs $file.Name) -Force
                    $result.collected += "sources/clients/windows/$deviceName/runtime/legacy-files/$($file.Name)"
                }
            } else {
                $result.errors += "Windows runtime log root does not exist: $localRoot"
            }
            foreach ($file in @('torca-build.json', 'torca-artifact.json')) {
                $candidate = Join-Path $repoRoot "artifacts/$file"
                if (Test-Path $candidate) {
                    New-Item -ItemType Directory -Force -Path $platformDestination | Out-Null
                    Copy-Item $candidate (Join-Path $platformDestination $file) -Force
                    $result.collected += "sources/clients/windows/$deviceName/platform/$file"
                }
            }
            $process = Get-Process -ErrorAction SilentlyContinue | Where-Object ProcessName -match 'torca|tor|relay' | Select-Object Id, ProcessName, Path, StartTime
            New-Item -ItemType Directory -Force -Path $platformDestination | Out-Null
            Write-JsonFile (Join-Path $platformDestination 'process.snapshot.json') $process
            $result.collected += "sources/clients/windows/$deviceName/platform/process.snapshot.json"
            $windowsRunEntries = @($result.collected | Where-Object { $_ -match '(^|/)run-\d+' })
            if ($windowsRunEntries.Count -eq 0) {
                $result.errors += 'No current Windows runtime run was collected; the application may not have started after the last data reset.'
            }
        } elseif (Get-Command adb -ErrorAction SilentlyContinue) {
            $endpoint = $item.SelectedEndpoint
            New-Item -ItemType Directory -Force -Path $platformDestination | Out-Null
            $platformCaptures = @(
                [pscustomobject]@{ Name = 'device-properties.log'; Label = 'getprop'; Arguments = @('shell', 'getprop') },
                [pscustomobject]@{ Name = 'package.log'; Label = 'package'; Arguments = @('shell', 'dumpsys', 'package', 'com.torca.torca_app') },
                [pscustomobject]@{ Name = 'services.log'; Label = 'services'; Arguments = @('shell', 'dumpsys', 'activity', 'services') },
                [pscustomobject]@{ Name = 'processes.log'; Label = 'processes'; Arguments = @('shell', 'ps', '-A') },
                [pscustomobject]@{ Name = 'memory.log'; Label = 'memory'; Arguments = @('shell', 'dumpsys', 'meminfo', 'com.torca.torca_app') },
                [pscustomobject]@{ Name = 'connectivity.log'; Label = 'connectivity'; Arguments = @('shell', 'dumpsys', 'connectivity') },
                [pscustomobject]@{ Name = 'activity-top.log'; Label = 'activity top'; Arguments = @('shell', 'dumpsys', 'activity', 'top') },
                [pscustomobject]@{ Name = 'lock-state.log'; Label = 'lock state'; Arguments = @('shell', 'dumpsys', 'window', 'policy') },
                [pscustomobject]@{ Name = 'abi.log'; Label = 'ABI'; Arguments = @('shell', 'getprop', 'ro.product.cpu.abilist') }
            )
            foreach ($capture in $platformCaptures) {
                if (Invoke-Capture $endpoint $capture.Arguments (Join-Path $platformDestination $capture.Name) $capture.Label) {
                    $result.collected += "sources/clients/android/$deviceName/platform/$($capture.Name)"
                }
            }
            if ($collectLogcat) {
                $appPid = (& adb -s $endpoint shell pidof com.torca.torca_app 2>$null | Out-String).Trim()
                $logcatArgs = @('logcat', '-d', '-v', 'threadtime', '-b', 'main', '-b', 'system', '-b', 'crash')
                if ($appPid) { $logcatArgs += "--pid=$appPid" }
                if (Invoke-Capture $endpoint $logcatArgs (Join-Path $platformDestination 'android-logcat.log') 'filtered logcat') { $result.collected += "sources/clients/android/$deviceName/platform/android-logcat.log" }
                if (Invoke-Capture $endpoint @('logcat', '-b', 'crash', '-d', '-v', 'threadtime') (Join-Path $platformDestination 'android-crash.log') 'crash log') { $result.collected += "sources/clients/android/$deviceName/platform/android-crash.log" }
            }
            $remoteLogs = '/sdcard/Android/data/com.torca.torca_app/files/torca/logs'
            # Android's scoped storage can deny `adb shell test -d` even though
            # `adb pull` is permitted for the app-owned external directory.
            # Attempt the transfer directly and retain its output for diagnosis.
            $pullDestination = Join-Path $destination '_adb-pull'
            New-Item -ItemType Directory -Force -Path $pullDestination | Out-Null
            $pull = Invoke-NativeText -Executable 'adb' -Arguments @('-s', $endpoint, 'pull', $remoteLogs, $pullDestination)
            $pullExitCode = $pull.exitCode
            Set-Content -LiteralPath (Join-Path $destination 'android-runtime-pull.log') -Value $pull.output -Encoding utf8
            if ($pullExitCode -eq 0) {
                $pulledRoot = Join-Path $pullDestination 'logs'
                if (-not (Test-Path $pulledRoot)) { $pulledRoot = $pullDestination }
                $pulledDevices = Join-Path $pulledRoot 'devices'
                if (Test-Path $pulledDevices) {
                    foreach ($sourceDevice in Get-ChildItem -LiteralPath $pulledDevices -Directory -ErrorAction SilentlyContinue) {
                        $runtimeRoot = Join-Path $runtimeDestination (Get-SafeDeviceId $sourceDevice.Name)
                        $runs = @(Copy-RecentRuns -SourceRoot $sourceDevice.FullName -DestinationRoot $runtimeRoot)
                        $result.collected += @($runs | ForEach-Object { "sources/clients/android/$deviceName/runtime/$($sourceDevice.Name)/$_" })
                    }
                } else {
                    $legacyRuntime = Join-Path $runtimeDestination 'legacy'
                    $runs = @(Copy-RecentRuns -SourceRoot $pulledRoot -DestinationRoot $legacyRuntime)
                    $result.collected += @($runs | ForEach-Object { "sources/clients/android/$deviceName/runtime/legacy/$_" })
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
                $internalListing = Invoke-NativeText -Executable 'adb' -Arguments @(
                    '-s', $endpoint, 'shell', 'run-as', 'com.torca.torca_app',
                    'find', 'files/torca/logs', '-type', 'f'
                )
                $internalFiles = if ($internalListing.exitCode -eq 0) {
                    @($internalListing.output -split "`r?`n" |
                        ForEach-Object { $_.Trim() } |
                        Where-Object { $_ -match '^files/torca/logs/.+' })
                } else {
                    @()
                }
                if ($internalFiles.Count -gt 0) {
                    foreach ($relativeFile in $internalFiles) {
                        $relativeName = $relativeFile.Substring('files/torca/logs/'.Length).Replace('/', '\')
                        $normalizedName = if ($relativeName.StartsWith('devices\')) {
                            $relativeName.Substring('devices\'.Length)
                        } else {
                            Join-Path 'legacy' $relativeName
                        }
                        $localFile = Join-Path $runtimeDestination $normalizedName
                        New-Item -ItemType Directory -Force -Path (Split-Path -Parent $localFile) | Out-Null
                        & adb -s $endpoint exec-out run-as com.torca.torca_app cat $relativeFile > $localFile
                        if ($LASTEXITCODE -eq 0) { $result.collected += "sources/clients/android/$deviceName/runtime/$($normalizedName.Replace('\', '/'))" }
                    }
                } else {
                    $lockState = Get-Content -LiteralPath (Join-Path $platformDestination 'lock-state.log') -Raw -ErrorAction SilentlyContinue
                    if ($lockState -match '(?m)\bshowing\s*=\s*true\b') {
                        $result.errors += "$endpoint/runtime logs unavailable because the Android device is locked; unlock it and rerun collection"
                    } elseif ($internalListing.output -match 'not debuggable') {
                        $result.errors += "$endpoint/runtime log directory is absent and the installed release package does not permit run-as fallback; launch the app once and rerun collection"
                    } else {
                        $result.errors += "$endpoint/runtime logs pull failed (exit $pullExitCode) and no private logs were accessible"
                    }
                }
            }
            $runtimeEntries = @($result.collected | Where-Object { $_ -match '(^|/)run-\d+|runtime-logs' })
            if ($runtimeEntries.Count -eq 0) {
                $result.errors += "$endpoint/no current native runtime run was collected"
            }
        }
    } catch { $result.errors += $_.Exception.Message; Add-Error "$($item.Id): $($_.Exception.Message)" }
    $deviceResults.Add([pscustomobject]$result)
}

$requestedDeviceIds = [string[]]@()
if ($Device) { $requestedDeviceIds = [string[]]$Device }
$sourceSummary = @(
    foreach ($source in Get-ChildItem -LiteralPath $sourcesRoot -Directory -ErrorAction SilentlyContinue) {
        $files = @(Get-ChildItem -LiteralPath $source.FullName -Recurse -File -ErrorAction SilentlyContinue)
        [pscustomobject]@{
            source = $source.Name
            files = $files.Count
            bytes = [long](($files | Measure-Object -Property Length -Sum).Sum)
        }
    }
)
$fileInventory = @(
    foreach ($file in Get-ChildItem -LiteralPath $sourcesRoot -Recurse -File -ErrorAction SilentlyContinue | Sort-Object FullName) {
        [pscustomobject]@{
            path = $file.FullName.Substring($collectRoot.Length + 1).Replace('\', '/')
            bytes = $file.Length
            modifiedAt = $file.LastWriteTimeUtc.ToString('o')
        }
    }
)
Write-JsonFile (Join-Path $collectRoot 'file-inventory.json') $fileInventory
$manifest = [ordered]@{
    schema = 3
    collectionId = $collectionId
    startedAt = $collectionStartedAt.ToString('o')
    collectedAt = [DateTime]::UtcNow.ToString('o')
    requestedTarget = $Target
    requestedDevices = $requestedDeviceIds
    lastRuns = $LastRuns
    profile = $Profile
    includeLogcat = $collectLogcat
    includeStackLogs = $collectStack
    repository = Get-RepositoryIdentity
    fileInventory = 'file-inventory.json'
    sourceSummary = $sourceSummary
    devices = @($deviceResults)
    errors = @($collectionErrors)
    warnings = @($collectionWarnings)
}
if ($Target -in @('all', 'auto', 'android') -and -not (@($logicalDevices | Where-Object Platform -eq 'android').Count)) {
    $message = 'No Android device was detected by adb; Android logs were not collected. Connect and authorize the device, then rerun zip.ps1.'
    $collectionErrors.Add($message)
    $manifest.errors += $message
    Write-Warning $message
}
$deviceErrorCount = @($deviceResults | ForEach-Object errors | Where-Object { -not [string]::IsNullOrWhiteSpace($_) }).Count
$manifest['status'] = if ($manifest.errors.Count -eq 0 -and $deviceErrorCount -eq 0) { 'complete' } else { 'partial' }
Write-JsonFile (Join-Path $collectRoot 'collection-manifest.json') $manifest
@'
# Torca diagnostic bundle

All paths below `sources/` identify the producer before the log type:

- `clients/windows/<host>/runtime` - structured Rust client runs from Windows.
- `clients/windows/<host>/platform` - Windows process and artifact metadata.
- `clients/android/<serial>/runtime` - structured Rust client runs copied from Android.
- `clients/android/<serial>/platform` - logcat, dumpsys and Android package state.
- `deploy/runs` - structured deployment runs.
- `deploy/state` - deploy checkpoints, build manifests and installed-device manifests.
- `relay/live` - output captured live from the current Docker container.
- `relay/persisted` - older persisted relay/Tor files; check freshness metadata before use.
- `relay/state` - configured/ready endpoint and consistency report.
- `host/discovery` - raw device discovery evidence.

`collection-manifest.json` is authoritative for completeness and errors.
`checksums.sha256` covers every included file.
'@ | Set-Content -LiteralPath (Join-Path $collectRoot 'README.md') -Encoding utf8
Set-Content -LiteralPath (Join-Path $collectRoot 'collector.log') -Value ("Torca diagnostic collection $collectionId`nProfile: $Profile`nCollected: $([DateTime]::UtcNow.ToString('o'))") -Encoding utf8
Set-Content -LiteralPath (Join-Path $collectRoot 'collector-errors.jsonl') -Value ($collectionErrors | ForEach-Object { '{"schema":1,"message":"' + (($_ -replace '\\', '\\') -replace '"', '\"') + '"}' }) -Encoding utf8
Set-Content -LiteralPath (Join-Path $collectRoot 'collector-warnings.jsonl') -Value ($collectionWarnings | ForEach-Object { '{"schema":1,"message":"' + (($_ -replace '\\', '\\') -replace '"', '\"') + '"}' }) -Encoding utf8

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
