Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$script:RelaySourceFingerprintCache = @{}

function Get-TorcaRelaySourceFingerprint {
    param([Parameter(Mandatory = $true)]$Paths)
    $cacheKey = [IO.Path]::GetFullPath($Paths.RepoRoot).ToLowerInvariant()
    if ($script:RelaySourceFingerprintCache.ContainsKey($cacheKey)) {
        return $script:RelaySourceFingerprintCache[$cacheKey]
    }
    # The relay binary may use shared workspace crates, but never Flutter
    # source. Hash the Rust and Docker inputs only so a UI-only deploy reuses
    # the existing image without weakening correctness for relay code edits.
    $roots = @(
        (Join-Path $Paths.RepoRoot 'services/relay'),
        (Join-Path $Paths.RepoRoot 'crates'),
        (Join-Path $Paths.RepoRoot 'Cargo.toml'),
        (Join-Path $Paths.RepoRoot 'Cargo.lock'),
        (Join-Path $Paths.RepoRoot 'infra/docker/Dockerfile.relay'),
        $Paths.DockerCompose
    )
    $files = foreach ($root in $roots) {
        if (Test-Path -LiteralPath $root -PathType Leaf) {
            Get-Item -LiteralPath $root
        } elseif (Test-Path -LiteralPath $root -PathType Container) {
            Get-ChildItem -LiteralPath $root -Recurse -File |
                Where-Object { $_.FullName -notmatch '[\\/](target|build)[\\/]' }
        }
    }
    $sha = [System.Security.Cryptography.SHA256]::Create()
    try {
        foreach ($file in @($files | Sort-Object FullName)) {
            $relative = $file.FullName.Substring($Paths.RepoRoot.Length).Replace('\', '/')
            $relativeBytes = [System.Text.Encoding]::UTF8.GetBytes($relative + "`n")
            [void]$sha.TransformBlock($relativeBytes, 0, $relativeBytes.Length, $relativeBytes, 0)
            $bytes = [System.IO.File]::ReadAllBytes($file.FullName)
            [void]$sha.TransformBlock($bytes, 0, $bytes.Length, $bytes, 0)
        }
        [void]$sha.TransformFinalBlock([byte[]]::new(0), 0, 0)
        $fingerprint = [BitConverter]::ToString($sha.Hash).Replace('-', '')
        $script:RelaySourceFingerprintCache[$cacheKey] = $fingerprint
        return $fingerprint
    } finally {
        $sha.Dispose()
    }
}

function Get-TorcaRelayBuildManifestPath { param($Paths) (Join-Path $Paths.StackRoot 'relay-build.json') }
function Get-TorcaProcessRelayBuildManifestPath { param($Paths) (Join-Path $Paths.StackRoot 'relay-process-build.json') }

function Get-TorcaRelaySourceCommit {
    param([Parameter(Mandatory = $true)]$Paths)
    try {
        $commit = (& git -C $Paths.RepoRoot rev-parse HEAD 2>$null | Out-String).Trim()
        if ($LASTEXITCODE -eq 0 -and $commit -match '^[0-9a-fA-F]{40,64}$') {
            return $commit.ToLowerInvariant()
        }
    } catch {}
    return 'working-tree'
}

function Test-TorcaDockerRelayImage {
    param($Paths)
    $image = (& docker compose -f $Paths.DockerCompose images -q relay 2>$null | Out-String).Trim()
    return -not [string]::IsNullOrWhiteSpace($image)
}

function Test-TorcaRelayImageBuildRequired {
    param($Paths)
    if (-not (Test-TorcaDockerRelayImage -Paths $Paths)) { return $true }
    $manifestPath = Get-TorcaRelayBuildManifestPath $Paths
    if (-not (Test-Path -LiteralPath $manifestPath)) { return $true }
    try {
        $manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
        return [string]$manifest.SourceFingerprint -ne (Get-TorcaRelaySourceFingerprint -Paths $Paths)
    } catch {
        return $true
    }
}

function Test-TorcaProcessRelayBuildRequired {
    param($Paths)
    if (-not (Test-Path -LiteralPath $Paths.RelayExecutable)) { return $true }
    $manifestPath = Get-TorcaProcessRelayBuildManifestPath $Paths
    if (-not (Test-Path -LiteralPath $manifestPath)) { return $true }
    try {
        $manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
        return [string]$manifest.SourceFingerprint -ne (Get-TorcaRelaySourceFingerprint -Paths $Paths)
    } catch {
        return $true
    }
}

function Write-TorcaRelayBuildManifest {
    param($Paths)
    [pscustomobject]@{
        SourceFingerprint = Get-TorcaRelaySourceFingerprint -Paths $Paths
        BuildId = Get-TorcaRelaySourceFingerprint -Paths $Paths
        SourceCommit = Get-TorcaRelaySourceCommit -Paths $Paths
        BuiltAt = [DateTime]::UtcNow.ToString('o')
    } | ConvertTo-Json | Set-Content -LiteralPath (Get-TorcaRelayBuildManifestPath $Paths) -Encoding utf8
}

function Write-TorcaProcessRelayBuildManifest {
    param($Paths)
    [pscustomobject]@{
        SourceFingerprint = Get-TorcaRelaySourceFingerprint -Paths $Paths
        BuildId = Get-TorcaRelaySourceFingerprint -Paths $Paths
        SourceCommit = Get-TorcaRelaySourceCommit -Paths $Paths
        BuiltAt = [DateTime]::UtcNow.ToString('o')
    } | ConvertTo-Json | Set-Content -LiteralPath (Get-TorcaProcessRelayBuildManifestPath $Paths) -Encoding utf8
}

function Test-TorcaPort { param([int]$Port)
    $client = [System.Net.Sockets.TcpClient]::new()
    try { $task = $client.ConnectAsync('127.0.0.1', $Port); if (-not $task.Wait(1000)) { return $false }; return $client.Connected } catch { return $false } finally { $client.Dispose() }
}
function Test-TorcaDocker { if (-not (Get-Command docker -ErrorAction SilentlyContinue)) { return $false }; try { & docker info --format '{{.ServerVersion}}' 2>$null | Out-Null; return ($LASTEXITCODE -eq 0) } catch { return $false } }
function Invoke-TorcaDockerCompose { param($Paths, [string[]]$Arguments)
    if (-not (Test-TorcaDocker)) { throw 'Docker is unavailable.' }
    Write-Verbose ("docker compose {0}" -f ($Arguments -join ' '))
    $previousBuildId = [Environment]::GetEnvironmentVariable('TORCA_RELAY_BUILD_ID', 'Process')
    $previousSourceCommit = [Environment]::GetEnvironmentVariable('TORCA_RELAY_SOURCE_COMMIT', 'Process')
    try {
        $env:TORCA_RELAY_BUILD_ID = Get-TorcaRelaySourceFingerprint -Paths $Paths
        $env:TORCA_RELAY_SOURCE_COMMIT = Get-TorcaRelaySourceCommit -Paths $Paths
        $output = @(& docker compose -f $Paths.DockerCompose @Arguments 2>&1 | ForEach-Object { "$($_)" })
        $exitCode = $LASTEXITCODE
    } finally {
        if ($null -eq $previousBuildId) { Remove-Item Env:TORCA_RELAY_BUILD_ID -ErrorAction SilentlyContinue } else { $env:TORCA_RELAY_BUILD_ID = $previousBuildId }
        if ($null -eq $previousSourceCommit) { Remove-Item Env:TORCA_RELAY_SOURCE_COMMIT -ErrorAction SilentlyContinue } else { $env:TORCA_RELAY_SOURCE_COMMIT = $previousSourceCommit }
    }
    foreach ($line in $output) { Write-Verbose $line }
    if ($exitCode -ne 0) {
        $recent = @($output | Select-Object -Last 80) -join [Environment]::NewLine
        throw "Docker Compose failed with exit code $exitCode.`nRecent compose output:`n$recent"
    }
}
function Read-TorcaEndpoint { param($Paths)
    if (-not (Test-Path $Paths.RelayEndpoint)) { throw "Relay endpoint is missing: $($Paths.RelayEndpoint)" }
    $value = (Get-Content $Paths.RelayEndpoint -Raw).Trim()
    if ($value -notmatch '^[a-z2-7]{56}\.onion:[1-9][0-9]{0,4}$') { throw "Invalid relay endpoint: $value" }
    $value
}

function Reset-TorcaRelayOnionIdentity {
    param([Parameter(Mandatory = $true)]$Paths)

    # The relay onion key is stored in Arti's state root. Removing only the
    # endpoint file republishes the same identity, so the wizard's new-onion
    # action must erase this narrowly scoped relay state after the process has
    # stopped.
    $stackRoot = [IO.Path]::GetFullPath($Paths.StackRoot).TrimEnd('\', '/')
    $torRoot = [IO.Path]::GetFullPath((Join-Path $Paths.StackRoot 'tor'))
    $prefix = $stackRoot + [IO.Path]::DirectorySeparatorChar
    if (-not $torRoot.StartsWith($prefix, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing to reset relay onion state outside the stack root: $torRoot"
    }
    if (Test-Path -LiteralPath $torRoot) {
        Remove-Item -LiteralPath $torRoot -Recurse -Force
    }
    foreach ($file in @($Paths.RelayEndpoint, $Paths.RelayReady, $Paths.RelayStatus)) {
        Remove-Item -LiteralPath $file -Force -ErrorAction SilentlyContinue
    }
    Write-TorcaStackStage -Name 'Relay onion identity' -State 'running' -Detail 'Previous relay identity removed; generating a new v3 onion address'
}
function Reset-TorcaRelayDirectoryCache {
    param([Parameter(Mandatory = $true)]$Paths)

    # Preserve state/hss and its key material (therefore the onion address),
    # but discard Arti's cached directory consensus and microdescriptors. This
    # is an explicit repair operation: a cold directory download can make the
    # next bootstrap slower and should never happen during a normal restart.
    $torRoot = [IO.Path]::GetFullPath((Join-Path $Paths.StackRoot 'tor')).TrimEnd('\', '/')
    $cacheRoot = [IO.Path]::GetFullPath((Join-Path $torRoot 'cache'))
    $prefix = $torRoot + [IO.Path]::DirectorySeparatorChar
    if (-not $cacheRoot.StartsWith($prefix, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing to reset relay Tor cache outside the relay state root: $cacheRoot"
    }
    if (Test-Path -LiteralPath $cacheRoot) {
        Remove-Item -LiteralPath $cacheRoot -Recurse -Force
    }
    foreach ($file in @($Paths.RelayEndpoint, $Paths.RelayReady, $Paths.RelayStatus)) {
        Remove-Item -LiteralPath $file -Force -ErrorAction SilentlyContinue
    }
    Write-TorcaStackStage -Name 'Relay Tor cache' -State 'running' -Detail 'Directory cache cleared; onion identity preserved'
}
function Start-TorcaDetachedProcess { param([string]$FilePath, [string]$WorkingDirectory, [string]$StdOut, [string]$StdErr)
    foreach ($file in @($StdOut, $StdErr)) { if ($file) { New-Item -ItemType Directory -Force -Path (Split-Path -Parent $file) | Out-Null } }
    (Start-Process -FilePath $FilePath -WorkingDirectory $WorkingDirectory -WindowStyle Hidden -RedirectStandardOutput $StdOut -RedirectStandardError $StdErr -PassThru).Id
}
function Stop-TorcaProcessRelay {
    param($Paths, [object]$KnownPid)
    $relayPath = [IO.Path]::GetFullPath($Paths.RelayExecutable)
    $ids = [System.Collections.Generic.HashSet[int]]::new()
    if ($KnownPid) { [void]$ids.Add([int]$KnownPid) }
    foreach ($process in @(Get-CimInstance Win32_Process -Filter "Name = 'torca-relay.exe'" -ErrorAction SilentlyContinue)) {
        if ($process.ExecutablePath -and [IO.Path]::GetFullPath($process.ExecutablePath) -eq $relayPath) {
            [void]$ids.Add([int]$process.ProcessId)
        }
    }
    foreach ($id in $ids) {
        Stop-Process -Id $id -Force -ErrorAction SilentlyContinue
    }
}
function Test-TorcaDockerRelayRunning {
    param($Paths)
    $status = Get-TorcaDockerRelayStatus -Paths $Paths
    return $status.Status -eq 'running'
}

function Get-TorcaDockerRelayStatus {
    param($Paths)
    if (-not (Test-TorcaDocker)) {
        return [pscustomobject]@{ ContainerId = $null; Status = 'unavailable'; Health = 'unavailable'; ExitCode = $null; Restarting = $false; RestartCount = 0; Error = 'Docker is unavailable.' }
    }
    $id = (& docker compose -f $Paths.DockerCompose ps -q relay 2>$null | Out-String).Trim()
    if (-not $id) {
        return [pscustomobject]@{ ContainerId = $null; Status = 'missing'; Health = 'unknown'; ExitCode = $null; Restarting = $false; RestartCount = 0; Error = $null }
    }
    $raw = (& docker inspect --format '{{json .State}}' $id 2>$null | Out-String).Trim()
    if (-not $raw) {
        return [pscustomobject]@{ ContainerId = $id; Status = 'unknown'; Health = 'unknown'; ExitCode = $null; Restarting = $false; RestartCount = 0; Error = 'Docker inspection returned no state.' }
    }
    try {
        $state = $raw | ConvertFrom-Json
        $health = if ($state.Health -and $state.Health.Status) { [string]$state.Health.Status } else { 'none' }
        $restartCount = (& docker inspect --format '{{.RestartCount}}' $id 2>$null | Out-String).Trim()
        return [pscustomobject]@{
            ContainerId = $id; Status = [string]$state.Status; Health = $health
            ExitCode = [int]$state.ExitCode; Restarting = [bool]$state.Restarting
            RestartCount = if ($restartCount -match '^\d+$') { [int]$restartCount } else { 0 }
            Error = [string]$state.Error
        }
    } catch {
        return [pscustomobject]@{ ContainerId = $id; Status = 'unknown'; Health = 'unknown'; ExitCode = $null; Restarting = $false; RestartCount = 0; Error = "Docker state cannot be parsed: $($_.Exception.Message)" }
    }
}

function Get-TorcaDockerRelayDiagnostics {
    param($Paths, [ValidateRange(1, 500)][int]$Tail = 80)
    $lines = (& docker compose -f $Paths.DockerCompose logs --no-color --tail $Tail relay 2>&1 | Out-String).Trim()
    if ($lines) { return $lines }
    return '(relay has not produced any logs)'
}

function Write-TorcaStackStage {
    param([string]$Name, [string]$State, [string]$Detail)
    if (Get-Command Write-TorcaStage -ErrorAction SilentlyContinue) {
        Write-TorcaStage -Name $Name -State $State -Detail $Detail
    } else {
        Write-Host "[$State] $Name $Detail"
    }
}

function Update-TorcaStackActivity {
    param([string]$Status, [int]$Percent)
    if (Get-Command Update-TorcaActivity -ErrorAction SilentlyContinue) {
        Update-TorcaActivity -Id 41 -Activity 'Torca relay bootstrap' -Status $Status -PercentComplete $Percent
    }
}

function Complete-TorcaStackActivity {
    param([string]$Status)
    if (Get-Command Complete-TorcaActivity -ErrorAction SilentlyContinue) {
        Complete-TorcaActivity -Id 41 -Status $Status
    }
}

function Wait-TorcaDockerRelayReady {
    param($Paths, [ValidateRange(1, 900)][int]$TimeoutSeconds = 300)
    $started = Get-Date
    $deadline = $started.AddSeconds($TimeoutSeconds)
    $lastStatus = $null
    while ((Get-Date) -lt $deadline) {
        $status = Get-TorcaDockerRelayStatus -Paths $Paths
        $elapsed = [int]((Get-Date) - $started).TotalSeconds
        $statusKey = "container=$($status.Status), health=$($status.Health), restarts=$($status.RestartCount)"
        $detail = "$statusKey, elapsed=$elapsed` s"
        if ($statusKey -ne $lastStatus) {
            Write-TorcaStackStage -Name 'Relay container' -State 'running' -Detail $detail
            $lastStatus = $statusKey
        }
        Update-TorcaStackActivity -Status $detail -Percent ([Math]::Min(99, [int](100 * $elapsed / $TimeoutSeconds)))
        if ($status.Restarting -or ($status.Status -in @('exited', 'dead') -and $status.ExitCode -ne 0)) {
            Complete-TorcaStackActivity -Status 'Relay failed'
            $diagnostics = Get-TorcaDockerRelayDiagnostics -Paths $Paths
            throw "Docker relay failed (state=$($status.Status), exit=$($status.ExitCode), restarts=$($status.RestartCount)).`nRecent relay logs:`n$diagnostics"
        }
        if ($status.Status -eq 'running' -and $status.Health -eq 'unhealthy') {
            Complete-TorcaStackActivity -Status 'Relay unhealthy'
            $diagnostics = Get-TorcaDockerRelayDiagnostics -Paths $Paths
            throw "Docker relay became unhealthy.`nRecent relay logs:`n$diagnostics"
        }
        try {
            $endpoint = Read-TorcaEndpoint $Paths
            if ($status.Status -eq 'running' -and $status.Health -in @('healthy', 'none')) {
                Complete-TorcaStackActivity -Status 'Relay ready'
                return $endpoint
            }
        } catch { Write-Verbose "Relay endpoint is not ready: $($_.Exception.Message)" }
        Start-Sleep -Seconds 2
    }
    Complete-TorcaStackActivity -Status 'Relay timed out'
    $final = Get-TorcaDockerRelayStatus -Paths $Paths
    $dockerActive = $false
    try { $dockerActive = $Paths.StackProvider -eq 'docker' -or (Test-TorcaDockerRelayRunning -Paths $Paths) } catch { $dockerActive = $false }
    $diagnostics = if ($dockerActive) {
        Get-TorcaDockerRelayDiagnostics -Paths $Paths
    } elseif (Test-Path -LiteralPath $Paths.RelayErrorLog) {
        (Get-Content -LiteralPath $Paths.RelayErrorLog -Tail 80 -ErrorAction SilentlyContinue) -join "`n"
    } else { '(relay has not produced any logs)' }
    throw "Docker relay did not become ready within $TimeoutSeconds seconds (state=$($final.Status), health=$($final.Health), restarts=$($final.RestartCount)).`nRecent relay logs:`n$diagnostics"
}

function Wait-TorcaRelayOnionReachable {
    param($Paths, [ValidateRange(1, 1800)][int]$TimeoutSeconds = 900)
    $started = Get-Date
    $deadline = $started.AddSeconds($TimeoutSeconds)
    $lastState = $null
    while ((Get-Date) -lt $deadline) {
        $elapsed = [int]((Get-Date) - $started).TotalSeconds
        $readyEndpoint = if (Test-Path -LiteralPath $Paths.RelayReady) {
            (Get-Content -LiteralPath $Paths.RelayReady -Raw -ErrorAction SilentlyContinue).Trim()
        } else { '' }
        # A stale readiness marker must never make a newly rotated relay look
        # ready. The marker belongs to the exact endpoint currently persisted
        # by the running stack.
        $state = if ($readyEndpoint -eq $endpoint) { 'reachable' } else { 'warming' }
        if ($state -ne $lastState) {
            $stageState = if ($state -eq 'reachable') { 'ready' } else { 'running' }
            $stageDetail = if ($state -eq 'reachable') {
                'public onion endpoint is reachable'
            } else {
                "waiting for public onion reachability, elapsed=$elapsed s"
            }
            Write-TorcaStackStage -Name 'Relay onion reachability' -State $stageState -Detail $stageDetail
            $lastState = $state
        }
        if ($state -eq 'reachable') {
            return $true
        }
        Start-Sleep -Seconds 2
    }
    $diagnostics = Get-TorcaDockerRelayDiagnostics -Paths $Paths
    throw "Relay onion endpoint did not become publicly reachable within $TimeoutSeconds seconds.`nRecent relay logs:`n$diagnostics"
}
function Assert-TorcaStackHealth {
    param($Paths, [Parameter(Mandatory = $true)]$Stack)
    $endpoint = Read-TorcaEndpoint $Paths
    if ([string]$Stack.Endpoint -ne $endpoint) {
        throw "Relay endpoint state mismatch. Runtime=$($Stack.Endpoint) File=$endpoint"
    }
    if ($Stack.Provider -eq 'docker') {
        $status = Get-TorcaDockerRelayStatus -Paths $Paths
        if ($status.Status -ne 'running' -or $status.Health -ne 'healthy' -or $status.RestartCount -ne 0) {
            $diagnostics = Get-TorcaDockerRelayDiagnostics -Paths $Paths
            throw "Relay health gate failed (state=$($status.Status), health=$($status.Health), restarts=$($status.RestartCount)).`nRecent relay logs:`n$diagnostics"
        }
        & docker exec $status.ContainerId /usr/local/bin/torca-relay health-check 2>$null
        if ($LASTEXITCODE -ne 0) {
            throw 'Relay health gate failed: protocol health check did not receive Healthy from the running container.'
        }
    } elseif (-not (Test-TorcaPort 8844)) {
        throw 'Relay health gate failed: the local relay server is not listening on port 8844.'
    } else {
        & $Paths.RelayExecutable health-check 2>$null
        if ($LASTEXITCODE -ne 0) {
            throw 'Relay health gate failed: local protocol health check did not receive Healthy.'
        }
    }
    Write-TorcaStackStage -Name 'Relay health gate' -State 'ready' -Detail "provider=$($Stack.Provider), endpoint=$endpoint, listener=8844"
    return $Stack
}
function Start-TorcaStack {
    param(
        $Paths,
        [ValidateSet('Ensure','Preserve','Restart','Repair','Rotate')][string]$OnionPolicy = 'Ensure',
        [switch]$ForceRebuild,
        [switch]$SkipSourceRebuild
    )
    if ($ForceRebuild -and $SkipSourceRebuild) {
        throw 'Relay image cannot be forced and preserved at the same time.'
    }
    Initialize-TorcaPaths $Paths
    $state = Get-TorcaRuntimeState -Paths $Paths
    $previousEndpoint = [string]$state.Endpoint
    $endpoint = $null
    if ($Paths.StackProvider -eq 'docker' -or ($Paths.StackProvider -eq 'auto' -and (Test-TorcaDocker))) {
        # A host relay and the compose relay create two independent networks.
        # Docker is authoritative here, so remove the obsolete host process first.
        Stop-TorcaProcessRelay -Paths $Paths -KnownPid $state.RelayPid
        $state.RelayPid = $null
        if ($OnionPolicy -in @('Restart','Repair','Rotate') -or $ForceRebuild) { Invoke-TorcaDockerCompose $Paths @('down') }
        if ($OnionPolicy -eq 'Repair') { Reset-TorcaRelayDirectoryCache -Paths $Paths }
        if ($OnionPolicy -eq 'Rotate') { Reset-TorcaRelayOnionIdentity -Paths $Paths }
        $rebuildRelay = [bool]$ForceRebuild -or (-not $SkipSourceRebuild -and (Test-TorcaRelayImageBuildRequired -Paths $Paths))
        $composeArguments = @('up','-d')
        if ($rebuildRelay) { $composeArguments += '--build' }
        elseif ($SkipSourceRebuild) { $composeArguments += '--no-build' }
        $imageDetail = if ($ForceRebuild) {
            'Forced rebuild selected; building and starting the current relay server sources'
        } elseif ($rebuildRelay) {
            'Relay sources changed; building and starting Docker relay'
        } else {
            'Relay image unchanged; starting without rebuild'
        }
        Write-TorcaStackStage -Name 'Relay image' -State 'running' -Detail $imageDetail
        Invoke-TorcaDockerCompose $Paths $composeArguments
        if ($rebuildRelay) { Write-TorcaRelayBuildManifest -Paths $Paths }
        $state.Provider = 'docker'
        $endpoint = Wait-TorcaDockerRelayReady -Paths $Paths
    } else {
        if ($state.Provider -eq 'docker' -and (Test-TorcaDockerRelayRunning $Paths)) {
            Invoke-TorcaDockerCompose $Paths @('down')
        }
        if ($OnionPolicy -in @('Repair','Rotate')) {
            Stop-TorcaProcessRelay -Paths $Paths -KnownPid $state.RelayPid
            $state.RelayPid = $null
        }
        if ($OnionPolicy -eq 'Repair') {
            Reset-TorcaRelayDirectoryCache -Paths $Paths
        }
        if ($OnionPolicy -eq 'Rotate') {
            Reset-TorcaRelayOnionIdentity -Paths $Paths
        }
        # Only a process relay owns the host listener. Docker intentionally
        # keeps 8844 in its own network namespace, so probing that host port
        # would erase a valid Docker endpoint on every subsequent deploy.
        if ($OnionPolicy -in @('Restart','Repair','Rotate') -or -not (Test-TorcaPort 8844)) {
            Remove-Item -LiteralPath $Paths.RelayEndpoint -Force -ErrorAction SilentlyContinue
        }
        if ($SkipSourceRebuild -and -not (Test-Path -LiteralPath $Paths.RelayExecutable)) {
            throw 'Relay binary reuse was requested, but no existing relay executable is available.'
        }
        $rebuildProcessRelay = [bool]$ForceRebuild -or (-not $SkipSourceRebuild -and (Test-TorcaProcessRelayBuildRequired -Paths $Paths))
        if ($rebuildProcessRelay) {
            # Windows cannot replace a running executable. A source change is
            # also a process-version change, so stop the exact known relay
            # before compiling and let the common start path launch it again.
            Stop-TorcaProcessRelay -Paths $Paths -KnownPid $state.RelayPid
            $state.RelayPid = $null
            $previousBuildId = [Environment]::GetEnvironmentVariable('TORCA_RELAY_BUILD_ID', 'Process')
            $previousSourceCommit = [Environment]::GetEnvironmentVariable('TORCA_RELAY_SOURCE_COMMIT', 'Process')
            Push-Location $Paths.RepoRoot
            try {
                $env:TORCA_RELAY_BUILD_ID = Get-TorcaRelaySourceFingerprint -Paths $Paths
                $env:TORCA_RELAY_SOURCE_COMMIT = Get-TorcaRelaySourceCommit -Paths $Paths
                & cargo build -p torca-relay --release --locked
                if ($LASTEXITCODE -ne 0) { throw 'Failed to build torca-relay.' }
                Write-TorcaProcessRelayBuildManifest -Paths $Paths
            } finally {
                Pop-Location
                if ($null -eq $previousBuildId) { Remove-Item Env:TORCA_RELAY_BUILD_ID -ErrorAction SilentlyContinue } else { $env:TORCA_RELAY_BUILD_ID = $previousBuildId }
                if ($null -eq $previousSourceCommit) { Remove-Item Env:TORCA_RELAY_SOURCE_COMMIT -ErrorAction SilentlyContinue } else { $env:TORCA_RELAY_SOURCE_COMMIT = $previousSourceCommit }
            }
        }
        if ($OnionPolicy -in @('Restart','Repair','Rotate') -and $state.RelayPid) { Stop-Process -Id ([int]$state.RelayPid) -Force -ErrorAction SilentlyContinue; $state.RelayPid = $null }
        if (-not ($state.RelayPid -and (Get-Process -Id ([int]$state.RelayPid) -ErrorAction SilentlyContinue))) { $state.RelayPid = Start-TorcaDetachedProcess $Paths.RelayExecutable $Paths.RepoRoot $Paths.RelayLog $Paths.RelayErrorLog }
        $state.Provider = 'process'
    }
    if ($state.Provider -eq 'process') {
        Write-TorcaStackStage -Name 'Relay process' -State 'running' -Detail 'Waiting for onion endpoint'
        $deadline = (Get-Date).AddSeconds(240)
        while ((Get-Date) -lt $deadline) {
            try { $endpoint = Read-TorcaEndpoint $Paths; break } catch { Start-Sleep -Seconds 2 }
        }
    }
    if (-not $endpoint) { throw 'In-process Tor relay did not publish an endpoint.' }
    if ($OnionPolicy -eq 'Rotate' -and $previousEndpoint -and $endpoint -eq $previousEndpoint) {
        throw 'Relay onion rotation did not produce a new endpoint; refusing to deploy clients against an unchanged identity.'
    }
    # Persist the allocated endpoint before waiting for external reachability.
    # If the readiness gate times out, status/resume must still describe the
    # current onion rather than the endpoint from the previous relay process.
    $state.Endpoint = $endpoint
    $state.UpdatedAt = [DateTime]::UtcNow.ToString('o')
    Set-TorcaRuntimeState $Paths $state
    # An endpoint file only means that Arti allocated an onion address. Do not
    # deploy clients until the relay has proved public reachability as well.
    $onionReachable = Wait-TorcaRelayOnionReachable -Paths $Paths
    $endpointState = if ($onionReachable) { 'ready' } else { 'running' }
    $endpointDetail = if ($onionReachable) {
        "$endpoint (public onion reachable)"
    } else {
        "$endpoint (public onion warming; local relay is healthy)"
    }
    Write-TorcaStackStage -Name 'Relay endpoint' -State $endpointState -Detail $endpointDetail
    $stack = [pscustomobject]@{ Endpoint = $endpoint; OnionHost = ($endpoint -split ':')[0]; Provider = $state.Provider; RelayPid = $state.RelayPid; RelayPort = 8844; OnionReachable = $onionReachable }
    Assert-TorcaStackHealth -Paths $Paths -Stack $stack
}
function Stop-TorcaStack { param($Paths)
    $state = Get-TorcaRuntimeState $Paths
    if ($state.Provider -eq 'docker') { Invoke-TorcaDockerCompose $Paths @('down') }
    Stop-TorcaProcessRelay -Paths $Paths -KnownPid $state.RelayPid
    $state.RelayPid = $null; $state.UpdatedAt = [DateTime]::UtcNow.ToString('o'); Set-TorcaRuntimeState $Paths $state
}
function Get-TorcaStackStatus { param($Paths)
    $state = Get-TorcaRuntimeState $Paths
    # The endpoint marker is written immediately when Arti allocates the onion
    # and is the authoritative value during warm-up. Runtime state can lag
    # after an interrupted deploy, so never report that stale value to the
    # wizard or diagnostics when the marker is available.
    $endpoint = [string]$state.Endpoint
    if (Test-Path -LiteralPath $Paths.RelayEndpoint) {
        $marker = (Get-Content -LiteralPath $Paths.RelayEndpoint -Raw).Trim()
        if (-not [string]::IsNullOrWhiteSpace($marker)) { $endpoint = $marker }
    }
    $dockerStatus = if ($state.Provider -eq 'docker') { Get-TorcaDockerRelayStatus -Paths $Paths } else { $null }
    $dockerRelay = $dockerStatus -and $dockerStatus.Status -eq 'running'
    $processRelay = [bool]($state.RelayPid -and (Get-Process -Id ([int]$state.RelayPid) -ErrorAction SilentlyContinue))
    [pscustomobject]@{
        Provider = $state.Provider
        RelayRunning = [bool]($dockerRelay -or $processRelay)
        Endpoint = $endpoint
        OnionReachable = Test-Path -LiteralPath $Paths.RelayReady
        RelayPortOpen = if ($state.Provider -eq 'docker') { $dockerRelay } else { Test-TorcaPort 8844 }
        ContainerState = if ($dockerStatus) { $dockerStatus.Status } else { $null }
        ContainerHealth = if ($dockerStatus) { $dockerStatus.Health } else { $null }
        RestartCount = if ($dockerStatus) { $dockerStatus.RestartCount } else { $null }
    }
}
Export-ModuleMember -Function Start-TorcaStack, Stop-TorcaStack, Get-TorcaStackStatus, Get-TorcaDockerRelayStatus, Get-TorcaDockerRelayDiagnostics, Assert-TorcaStackHealth, Get-TorcaRelaySourceFingerprint
