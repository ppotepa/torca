Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Test-TorcaPort { param([int]$Port)
    $client = [System.Net.Sockets.TcpClient]::new()
    try { $task = $client.ConnectAsync('127.0.0.1', $Port); if (-not $task.Wait(1000)) { return $false }; return $client.Connected } catch { return $false } finally { $client.Dispose() }
}
function Test-TorcaDocker { if (-not (Get-Command docker -ErrorAction SilentlyContinue)) { return $false }; try { & docker info --format '{{.ServerVersion}}' 2>$null | Out-Null; return ($LASTEXITCODE -eq 0) } catch { return $false } }
function Invoke-TorcaDockerCompose { param($Paths, [string[]]$Arguments)
    if (-not (Test-TorcaDocker)) { throw 'Docker is unavailable.' }
    Write-Verbose ("docker compose {0}" -f ($Arguments -join ' '))
    $output = @(& docker compose -f $Paths.DockerCompose @Arguments 2>&1 | ForEach-Object { "$($_)" })
    $exitCode = $LASTEXITCODE
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
    param($Paths, [ValidateRange(1, 900)][int]$TimeoutSeconds = 240)
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
    $diagnostics = Get-TorcaDockerRelayDiagnostics -Paths $Paths
    throw "Docker relay did not become ready within $TimeoutSeconds seconds (state=$($final.Status), health=$($final.Health), restarts=$($final.RestartCount)).`nRecent relay logs:`n$diagnostics"
}
function Start-TorcaStack {
    param($Paths, [ValidateSet('Ensure','Preserve','Restart','Rotate')][string]$OnionPolicy = 'Ensure')
    Initialize-TorcaPaths $Paths
    $state = Get-TorcaRuntimeState -Paths $Paths
    $endpoint = $null
    if ($OnionPolicy -in @('Restart','Rotate') -or -not (Test-TorcaPort 8844)) {
        Remove-Item -LiteralPath $Paths.RelayEndpoint -Force -ErrorAction SilentlyContinue
    }
    if ($Paths.StackProvider -eq 'docker' -or ($Paths.StackProvider -eq 'auto' -and (Test-TorcaDocker))) {
        # A host relay and the compose relay create two independent networks.
        # Docker is authoritative here, so remove the obsolete host process first.
        Stop-TorcaProcessRelay -Paths $Paths -KnownPid $state.RelayPid
        $state.RelayPid = $null
        if ($OnionPolicy -in @('Restart','Rotate')) { Invoke-TorcaDockerCompose $Paths @('down') }
        if ($OnionPolicy -eq 'Rotate') { Remove-Item (Join-Path $Paths.StackRoot 'relay_endpoint.txt') -Force -ErrorAction SilentlyContinue }
        Write-TorcaStackStage -Name 'Relay image' -State 'running' -Detail 'Building and starting Docker relay'
        Invoke-TorcaDockerCompose $Paths @('up','-d','--build')
        $state.Provider = 'docker'
        $endpoint = Wait-TorcaDockerRelayReady -Paths $Paths
    } else {
        if ($state.Provider -eq 'docker' -and (Test-TorcaDockerRelayRunning $Paths)) {
            Invoke-TorcaDockerCompose $Paths @('down')
        }
        if (-not (Test-Path $Paths.RelayExecutable)) { Push-Location $Paths.RepoRoot; try { & cargo build -p torca-relay --release --locked } finally { Pop-Location }; if ($LASTEXITCODE -ne 0) { throw 'Failed to build torca-relay.' } }
        if ($OnionPolicy -in @('Restart','Rotate') -and $state.RelayPid) { Stop-Process -Id ([int]$state.RelayPid) -Force -ErrorAction SilentlyContinue; $state.RelayPid = $null }
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
    Write-TorcaStackStage -Name 'Relay endpoint' -State 'ready' -Detail $endpoint
    $state.Endpoint = $endpoint; $state.UpdatedAt = [DateTime]::UtcNow.ToString('o'); Set-TorcaRuntimeState $Paths $state
    [pscustomobject]@{ Endpoint = $endpoint; OnionHost = ($endpoint -split ':')[0]; Provider = $state.Provider; RelayPid = $state.RelayPid; RelayPort = 8844 }
}
function Stop-TorcaStack { param($Paths)
    $state = Get-TorcaRuntimeState $Paths
    if ($state.Provider -eq 'docker') { Invoke-TorcaDockerCompose $Paths @('down') }
    Stop-TorcaProcessRelay -Paths $Paths -KnownPid $state.RelayPid
    $state.RelayPid = $null; $state.UpdatedAt = [DateTime]::UtcNow.ToString('o'); Set-TorcaRuntimeState $Paths $state
}
function Get-TorcaStackStatus { param($Paths)
    $state = Get-TorcaRuntimeState $Paths
    $dockerStatus = if ($state.Provider -eq 'docker') { Get-TorcaDockerRelayStatus -Paths $Paths } else { $null }
    $dockerRelay = $dockerStatus -and $dockerStatus.Status -eq 'running'
    $processRelay = [bool]($state.RelayPid -and (Get-Process -Id ([int]$state.RelayPid) -ErrorAction SilentlyContinue))
    [pscustomobject]@{ Provider = $state.Provider; RelayRunning = [bool]($dockerRelay -or $processRelay); Endpoint = $state.Endpoint; RelayPortOpen = if ($state.Provider -eq 'docker') { $dockerRelay } else { Test-TorcaPort 8844 }; ContainerState = if ($dockerStatus) { $dockerStatus.Status } else { $null }; ContainerHealth = if ($dockerStatus) { $dockerStatus.Health } else { $null }; RestartCount = if ($dockerStatus) { $dockerStatus.RestartCount } else { $null } }
}
Export-ModuleMember -Function Start-TorcaStack, Stop-TorcaStack, Get-TorcaStackStatus, Get-TorcaDockerRelayStatus, Get-TorcaDockerRelayDiagnostics
