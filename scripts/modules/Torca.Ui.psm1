Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Write-TorcaConsoleHeader {
    param([Parameter(Mandatory = $true)][string]$Title, [hashtable]$Details = @{})
    $width = [Math]::Max(42, $Title.Length + 8)
    $line = '═' * $width
    $content = "  $Title"
    Write-Host "`n╔$line╗" -ForegroundColor DarkCyan
    Write-Host ("║{0}║" -f $content.PadRight($width)) -ForegroundColor Cyan
    Write-Host "╚$line╝" -ForegroundColor DarkCyan
    foreach ($entry in $Details.GetEnumerator()) {
        Write-Host ("  {0,-14} {1}" -f ("$($entry.Key):"), $entry.Value) -ForegroundColor DarkGray
    }
}

function Write-TorcaStage {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][ValidateSet('pending', 'running', 'ready', 'warning', 'failed')][string]$State,
        [string]$Detail
    )
    $visual = @{
        pending = @{ Symbol = '○'; Color = 'DarkGray' }
        running = @{ Symbol = '◌'; Color = 'Cyan' }
        ready = @{ Symbol = '✓'; Color = 'Green' }
        warning = @{ Symbol = '!'; Color = 'Yellow' }
        failed = @{ Symbol = '✗'; Color = 'Red' }
    }[$State]
    $message = "  $($visual.Symbol) $Name"
    if ($Detail) { $message += " — $Detail" }
    Write-Host $message -ForegroundColor $visual.Color
}

function Update-TorcaActivity {
    param(
        [Parameter(Mandatory = $true)][int]$Id,
        [Parameter(Mandatory = $true)][string]$Activity,
        [Parameter(Mandatory = $true)][string]$Status,
        [ValidateRange(0, 100)][int]$PercentComplete = 0
    )
    Write-Progress -Id $Id -Activity $Activity -Status $Status -PercentComplete $PercentComplete
}

function Complete-TorcaActivity {
    param([Parameter(Mandatory = $true)][int]$Id, [string]$Status = 'Completed')
    Write-Progress -Id $Id -Activity 'Torca' -Status $Status -Completed
}

function Read-TorcaMenuChoice {
    param([string]$Prompt, [string[]]$Options, [string]$Default = '1')
    Write-Host "`n$Prompt" -ForegroundColor Cyan
    for ($i = 0; $i -lt $Options.Count; $i++) { Write-Host " [$($i + 1)] $($Options[$i])" }
    $value = Read-Host "Choice [$Default]"
    if ([string]::IsNullOrWhiteSpace($value)) { $value = $Default }
    $number = 0
    if (-not [int]::TryParse($value, [ref]$number) -or $number -lt 1 -or $number -gt $Options.Count) { throw 'Invalid menu choice.' }
    $Options[$number - 1]
}

function Read-TorcaRelayOptions {
    param([bool]$AllowRotation)
    $onionOptions = @(
        "Keep current relay onion - preserve the deployed endpoint (recommended)",
        "Restart relay - keep onion identity and warm Tor cache",
        "Rebuild + repair relay - keep onion address, reset disposable relay state/cache and force a new server image",
        "Repair relay Tor cache - keep onion address but redownload directory data"
    )
    if ($AllowRotation) {
        $onionOptions += "Generate new relay onion - replaces the address and requires a client rebuild"
    }
    $onionChoice = Read-TorcaMenuChoice -Prompt "Tor network" -Default "1" -Options $onionOptions
    $onionPolicy = if ($onionChoice -like 'Restart*') {
        'Restart'
    } elseif ($onionChoice -like 'Rebuild + repair*') {
        'Repair'
    } elseif ($onionChoice -like 'Repair*') {
        'Repair'
    } elseif ($onionChoice -like 'Generate new*') {
        'Rotate'
    } else {
        'Ensure'
    }
    $relayBuildPolicy = if ($onionChoice -like 'Rebuild + repair*') { 'Rebuild' } else { 'IfRequired' }

    [pscustomobject]@{
        OnionPolicy = $onionPolicy
        RelayBuildPolicy = $relayBuildPolicy
    }
}

function Get-TorcaInteractiveOptions {
    $modeChoice = Read-TorcaMenuChoice -Prompt "Deployment mode" -Default "1" -Options @(
        "Fast client update - preserve relay and client data; build only when required (recommended)",
        "Clean coordinated deploy - keep onion, rebuild/repair relay, wipe all clients and deploy everywhere",
        "Relay maintenance only - leave every client and its Tor session untouched",
        "Custom advanced - configure lifecycle policies separately"
    )

    if ($modeChoice -like 'Fast client update*') {
        return [pscustomobject]@{
            Preset = 'FastClientUpdate'
            DeploymentScope = 'ClientsAndRelay'
            OnionPolicy = 'Ensure'
            RelayBuildPolicy = 'IfRequired'
            ClientDataPolicy = 'Preserve'
            BuildPolicy = 'IfRequired'
            InstallPolicy = 'Selected'
            RunPolicy = 'Restart'
            DefaultConfiguration = 'debug'
        }
    }

    if ($modeChoice -like 'Clean coordinated deploy*') {
        return [pscustomobject]@{
            Preset = 'CleanCoordinatedDeploy'
            DeploymentScope = 'FullReset'
            OnionPolicy = 'Repair'
            RelayBuildPolicy = 'Rebuild'
            ClientDataPolicy = 'ResetAll'
            BuildPolicy = 'Rebuild'
            InstallPolicy = 'Always'
            RunPolicy = 'Restart'
            DefaultConfiguration = 'release'
        }
    }

    if ($modeChoice -like 'Relay maintenance only*') {
        $relay = Read-TorcaRelayOptions -AllowRotation $false
        return [pscustomobject]@{
            Preset = 'RelayOnly'
            DeploymentScope = 'RelayOnly'
            OnionPolicy = $relay.OnionPolicy
            RelayBuildPolicy = $relay.RelayBuildPolicy
            ClientDataPolicy = 'Preserve'
            BuildPolicy = 'IfRequired'
            InstallPolicy = 'Skip'
            RunPolicy = 'Skip'
            DefaultConfiguration = 'debug'
        }
    }

    $scopeChoice = Read-TorcaMenuChoice -Prompt "Advanced deployment scope" -Default "1" -Options @(
        "Clients + relay - deploy selected clients and preserve data by default",
        "Relay only - never enumerate, build, install or restart clients",
        "Full client reset - erase selected client profiles and persistent Tor cache"
    )
    $deploymentScope = if ($scopeChoice -like 'Relay only*') {
        'RelayOnly'
    } elseif ($scopeChoice -like 'Full client reset*') {
        'FullReset'
    } else {
        'ClientsAndRelay'
    }
    $relay = Read-TorcaRelayOptions -AllowRotation ($deploymentScope -ne 'RelayOnly')

    $installPolicy = 'Skip'
    $runPolicy = 'Skip'
    if ($deploymentScope -ne 'RelayOnly') {
        $installChoice = Read-TorcaMenuChoice -Prompt "Install" -Default "1" -Options @(
            "Selected - install on selected",
            "Always - install on all detected",
            "Skip - build only"
        )
        $installPolicy = if ($installChoice -like 'Always*') { 'Always' } elseif ($installChoice -like 'Skip*') { 'Skip' } else { 'Selected' }

        $runChoice = Read-TorcaMenuChoice -Prompt "Run" -Default "1" -Options @(
            "Restart - restart after deploy",
            "Start - start if stopped",
            "Skip - do not run"
        )
        $runPolicy = if ($runChoice -like 'Start*') { 'Start' } elseif ($runChoice -like 'Skip*') { 'Skip' } else { 'Restart' }
    }
    [pscustomobject]@{
        Preset = 'CustomAdvanced'
        DeploymentScope = $deploymentScope
        OnionPolicy = $relay.OnionPolicy
        RelayBuildPolicy = $relay.RelayBuildPolicy
        ClientDataPolicy = if ($deploymentScope -eq 'FullReset') { 'ResetSelected' } else { $null }
        BuildPolicy = $null
        InstallPolicy = $installPolicy
        RunPolicy = $runPolicy
        DefaultConfiguration = 'debug'
    }
}

Export-ModuleMember -Function Read-TorcaMenuChoice, Get-TorcaInteractiveOptions, Write-TorcaConsoleHeader, Write-TorcaStage, Update-TorcaActivity, Complete-TorcaActivity
