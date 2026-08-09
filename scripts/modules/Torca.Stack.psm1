Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Test-TorcaPort { param([int]$Port)
    $client = [System.Net.Sockets.TcpClient]::new()
    try { $task = $client.ConnectAsync('127.0.0.1', $Port); if (-not $task.Wait(1000)) { return $false }; return $client.Connected } catch { return $false } finally { $client.Dispose() }
}
function Test-TorcaDocker { if (-not (Get-Command docker -ErrorAction SilentlyContinue)) { return $false }; try { & docker info --format '{{.ServerVersion}}' 2>$null | Out-Null; return ($LASTEXITCODE -eq 0) } catch { return $false } }
function Invoke-TorcaDockerCompose { param($Paths, [string[]]$Arguments)
    if (-not (Test-TorcaDocker)) { throw 'Docker is unavailable.' }
    & docker compose -f $Paths.DockerCompose @Arguments
    if ($LASTEXITCODE -ne 0) { throw "Docker Compose failed with exit code $LASTEXITCODE." }
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
    if (-not (Test-TorcaDocker)) { return $false }
    $id = (& docker compose -f $Paths.DockerCompose ps -q relay 2>$null | Out-String).Trim()
    return [bool]$id
}
function Start-TorcaStack {
    param($Paths, [ValidateSet('Ensure','Preserve','Restart','Rotate')][string]$OnionPolicy = 'Ensure')
    Initialize-TorcaPaths $Paths
    $state = Get-TorcaRuntimeState -Paths $Paths
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
        Invoke-TorcaDockerCompose $Paths @('up','-d','--build')
        $state.Provider = 'docker'
    } else {
        if ($state.Provider -eq 'docker' -and (Test-TorcaDockerRelayRunning $Paths)) {
            Invoke-TorcaDockerCompose $Paths @('down')
        }
        if (-not (Test-Path $Paths.RelayExecutable)) { Push-Location $Paths.RepoRoot; try { & cargo build -p torca-relay --release --locked } finally { Pop-Location }; if ($LASTEXITCODE -ne 0) { throw 'Failed to build torca-relay.' } }
        if ($OnionPolicy -in @('Restart','Rotate') -and $state.RelayPid) { Stop-Process -Id ([int]$state.RelayPid) -Force -ErrorAction SilentlyContinue; $state.RelayPid = $null }
        if (-not ($state.RelayPid -and (Get-Process -Id ([int]$state.RelayPid) -ErrorAction SilentlyContinue))) { $state.RelayPid = Start-TorcaDetachedProcess $Paths.RelayExecutable $Paths.RepoRoot $Paths.RelayLog $Paths.RelayErrorLog }
        $state.Provider = 'process'
    }
    $deadline = (Get-Date).AddSeconds(240)
    while ((Get-Date) -lt $deadline) { try { $endpoint = Read-TorcaEndpoint $Paths; break } catch { Start-Sleep -Seconds 2 } }
    if (-not $endpoint) { throw 'In-process Tor relay did not publish an endpoint.' }
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
    $dockerRelay = $state.Provider -eq 'docker' -and (Test-TorcaDockerRelayRunning $Paths)
    $processRelay = [bool]($state.RelayPid -and (Get-Process -Id ([int]$state.RelayPid) -ErrorAction SilentlyContinue))
    [pscustomobject]@{ Provider = $state.Provider; RelayRunning = [bool]($dockerRelay -or $processRelay); Endpoint = $state.Endpoint; RelayPortOpen = if ($state.Provider -eq 'docker') { $dockerRelay } else { Test-TorcaPort 8844 } }
}
Export-ModuleMember -Function Start-TorcaStack, Stop-TorcaStack, Get-TorcaStackStatus
