Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$script:TorcaScriptLogDirectory = $null
$script:TorcaScriptLogStartedAt = [DateTime]::UtcNow

function Initialize-TorcaScriptLog {
    $root = if ($env:LOCALAPPDATA) { Join-Path $env:LOCALAPPDATA 'Torca/logs' } else { Join-Path $PSScriptRoot '../../logs' }
    $date = [DateTime]::UtcNow.ToString('yyyy-MM-dd')
    $deviceRoot = Join-Path $root "devices/windows-host/$date"
    New-Item -ItemType Directory -Force -Path $deviceRoot | Out-Null
    $numbers = @(Get-ChildItem -LiteralPath $deviceRoot -Directory -ErrorAction SilentlyContinue |
        Where-Object Name -match '^run-\d{6}$' | ForEach-Object { [int]$_.Name.Substring(4) })
    $number = if ($numbers) { (($numbers | Measure-Object -Maximum).Maximum + 1) } else { 1 }
    $script:TorcaScriptLogDirectory = Join-Path $deviceRoot ('run-{0:000000}' -f $number)
    New-Item -ItemType Directory -Force -Path $script:TorcaScriptLogDirectory | Out-Null
    [ordered]@{ schema = 1; status = 'running'; startedAt = [DateTime]::UtcNow.ToString('o'); runId = ('run-{0:000000}' -f $number); deviceId = 'windows-host'; platform = 'windows' } |
        ConvertTo-Json | Set-Content -LiteralPath (Join-Path $script:TorcaScriptLogDirectory 'run.start.json') -Encoding utf8
}

function Redact-TorcaScriptText {
    param([string]$Value)
    $result = $Value
    foreach ($needle in @('private_key', 'private-key', 'secret=', 'capability=', 'token=', 'password=', 'plaintext=')) {
        $result = [regex]::Replace($result, [regex]::Escape($needle) + '.*', '[REDACTED]', 'IgnoreCase')
    }
    return $result.Substring(0, [Math]::Min(512, $result.Length))
}

function Complete-TorcaScriptLog {
    param([ValidateSet('completed', 'failed', 'interrupted')][string]$Status = 'completed', [string]$Reason = 'script finished')
    if (-not $script:TorcaScriptLogDirectory) { return }
    [ordered]@{
        schema = 1; status = $Status; endedAt = [DateTime]::UtcNow.ToString('o')
        durationMs = ([DateTime]::UtcNow - $script:TorcaScriptLogStartedAt).TotalMilliseconds
        runId = (Split-Path $script:TorcaScriptLogDirectory -Leaf); reason = (Redact-TorcaScriptText $Reason)
    } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $script:TorcaScriptLogDirectory 'run.end.json') -Encoding utf8
}

Initialize-TorcaScriptLog
Register-EngineEvent -SourceIdentifier 'PowerShell.Exiting' -SupportEvent -MessageData $script:TorcaScriptLogDirectory -Action {
    $runDirectory = [string]$event.MessageData
    if ($runDirectory -and (Test-Path -LiteralPath $runDirectory)) {
        [ordered]@{ schema = 1; status = 'completed'; endedAt = [DateTime]::UtcNow.ToString('o'); reason = 'PowerShell session ended' } |
            ConvertTo-Json | Set-Content -LiteralPath (Join-Path $runDirectory 'run.end.json') -Encoding utf8
    }
} | Out-Null

function Write-TorcaLog {
    param([Parameter(Mandatory = $true)][string]$Message, [ValidateSet('Info','Warn','Error')][string]$Level = 'Info')
    $color = switch ($Level) { 'Warn' { 'Yellow' } 'Error' { 'Red' } default { 'Cyan' } }
    Write-Host "[$Level] $Message" -ForegroundColor $color
    if ($script:TorcaScriptLogDirectory) {
        $record = [ordered]@{
            schema = 1; ts = [DateTime]::UtcNow.ToString('o'); level = $Level.ToLowerInvariant()
            runId = (Split-Path $script:TorcaScriptLogDirectory -Leaf); deviceId = 'windows-host'
            domain = 'deploy'; component = 'powershell'; code = 'SCRIPT_LOG'; message = (Redact-TorcaScriptText $Message)
        }
        ($record | ConvertTo-Json -Compress) | Add-Content -LiteralPath (Join-Path $script:TorcaScriptLogDirectory 'deploy.log') -Encoding utf8
    }
}

function Assert-TorcaCommand {
    param([Parameter(Mandatory = $true)][string]$Name)
    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) { throw "Required command is missing: $Name" }
}

function Invoke-TorcaChecked {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][scriptblock]$Action
    )
    Write-Host "==> $Name"
    & $Action
    if ($LASTEXITCODE -ne 0) { throw "$Name failed with exit code $LASTEXITCODE." }
}

function Wait-TorcaCondition {
    param(
        [Parameter(Mandatory = $true)][scriptblock]$Condition,
        [Parameter(Mandatory = $true)][int]$TimeoutSeconds,
        [Parameter(Mandatory = $true)][string]$Description
    )
    $deadline = [DateTimeOffset]::UtcNow.AddSeconds($TimeoutSeconds)
    do {
        if (& $Condition) { return }
        Start-Sleep -Milliseconds 500
    } while ([DateTimeOffset]::UtcNow -lt $deadline)
    throw "Timed out waiting for $Description."
}

Export-ModuleMember -Function Write-TorcaLog, Assert-TorcaCommand, Invoke-TorcaChecked, Wait-TorcaCondition, Complete-TorcaScriptLog
