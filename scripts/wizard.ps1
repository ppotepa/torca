[CmdletBinding()]
param(
    [ValidateSet(
        'Menu', 'Run', 'Redeploy', 'Rebuild', 'FullRedeploy', 'FullRedeployNewOnion',
        'RelayRestart', 'RelayRepair', 'RelayRebuild', 'Status', 'Collect', 'Exit'
    )]
    [string]$Action = 'Menu',
    [string[]]$Components,
    [ValidateSet('debug', 'release')]
    [string]$Configuration,
    [ValidateSet('auto', 'docker', 'process')]
    [string]$StackProvider = 'auto',
    [switch]$PlanOnly,
    [switch]$Confirm
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $PSScriptRoot
$moduleRoot = Join-Path $PSScriptRoot 'modules'
foreach ($module in @('Torca.Core', 'Torca.Config', 'Torca.State', 'Torca.Devices', 'Torca.Data', 'Torca.Stack', 'Torca.PlatformAssets', 'Torca.Build', 'Torca.Ui')) {
    $modulePath = Join-Path $moduleRoot "$module.psm1"
    if (-not (Test-Path -LiteralPath $modulePath -PathType Leaf)) {
        throw "Torca module is missing: $modulePath"
    }
    Import-Module $modulePath -Force -ErrorAction Stop -Verbose:$false
}

foreach ($requiredCommand in @('Get-TorcaPaths', 'Initialize-TorcaPaths', 'Read-TorcaJsonState', 'Get-TorcaDevices')) {
    if (-not (Get-Command $requiredCommand -ErrorAction SilentlyContinue)) {
        throw "Torca module loader did not expose required command: $requiredCommand"
    }
}

$paths = Get-TorcaPaths -RepoRoot $root
Initialize-TorcaPaths -Paths $paths
$flutterRoot = Join-Path $root 'apps/client/flutter'
$orchestrator = Join-Path $PSScriptRoot 'torca.ps1'
$configurationSpecified = $PSBoundParameters.ContainsKey('Configuration')
$lastDeploy = Read-TorcaJsonState -Path $paths.LastDeployFile
$resolvedConfiguration = if ($configurationSpecified) {
    $Configuration
} elseif ($lastDeploy -and [string]$lastDeploy.Configuration -in @('debug', 'release')) {
    [string]$lastDeploy.Configuration
} else {
    'debug'
}
$devices = @(Get-TorcaDevices -FlutterRoot $flutterRoot)
$deployable = @($devices | Where-Object { $_.CanInstall -and $_.CanRun })

function Show-TorcaWizardOverview {
    $endpoint = if (Test-Path -LiteralPath $paths.RelayEndpoint) {
        (Get-Content -LiteralPath $paths.RelayEndpoint -Raw).Trim()
    } else {
        '(not created)'
    }
    Write-TorcaConsoleHeader -Title 'Torca workflow' -Details @{
        Build = $resolvedConfiguration
        Relay = $endpoint
        Devices = if ($deployable.Count -gt 0) { $deployable.Count } else { 'none' }
    }
    if ($devices.Count -eq 0) {
        Write-TorcaStage -Name 'Devices' -State 'warning' -Detail 'No Windows or Android targets detected'
        return
    }
    foreach ($device in $devices) {
        $state = if ($device.CanInstall -and $device.CanRun) { 'ready' } else { 'warning' }
        Write-TorcaStage -Name "$($device.Platform):$($device.Name)" -State $state -Detail "$($device.State), $($device.Id)"
    }
}

function Get-TorcaWizardTarget {
    param([Parameter(Mandatory = $true)][object[]]$SelectedDevices)
    $platforms = @($SelectedDevices | ForEach-Object { [string]$_.Platform } | Sort-Object -Unique)
    if ($platforms -contains 'android' -and $platforms -contains 'windows') { return 'all' }
    if ($platforms -contains 'android') { return 'android' }
    if ($platforms -contains 'windows') { return 'windows' }
    throw 'The selected workflow has no deployable client platform.'
}

function Read-TorcaBuildConfiguration {
    if ($configurationSpecified) { return $resolvedConfiguration }
    $default = if ($resolvedConfiguration -eq 'release') { '2' } else { '1' }
    $choice = Read-TorcaMenuChoice -Prompt 'Client build' -Default $default -Options @(
        'Debug - fast iteration and full diagnostics',
        'Release - optimized package and full validation'
    )
    if ($choice -like 'Release*') { return 'release' }
    return 'debug'
}

function Read-TorcaRebuildComponents {
    if ($Components -and $Components.Count -gt 0) {
        $normalized = @($Components | ForEach-Object {
            $_ -split ',' | ForEach-Object { $_.Trim().ToLowerInvariant() }
        } | Where-Object { $_ })
        $invalid = @($normalized | Where-Object { $_ -notin @('android', 'windows', 'relay', 'onion') })
        if ($invalid.Count -gt 0) { throw "Unknown rebuild component: $($invalid -join ', ')" }
        return @($normalized | Sort-Object -Unique)
    }

    $androidCount = @($deployable | Where-Object Platform -eq 'android').Count
    $windowsCount = @($deployable | Where-Object Platform -eq 'windows').Count
    Write-Host ''
    Write-Host 'Rebuild components' -ForegroundColor Cyan
    Write-Host " [1] Android clients ($androidCount detected)"
    Write-Host " [2] Windows client ($windowsCount detected)"
    Write-Host ' [3] Relay server (keep current onion and Tor cache)'
    Write-Host ' [4] New Onion address (forces relay + every client rebuild)'
    $defaults = @()
    if ($androidCount -gt 0) { $defaults += '1' }
    if ($windowsCount -gt 0) { $defaults += '2' }
    $defaultText = if ($defaults.Count -gt 0) { $defaults -join ',' } else { '3' }
    $answer = Read-Host "Numbers separated by commas, or A for all except Onion [$defaultText]"
    if ([string]::IsNullOrWhiteSpace($answer)) { $answer = $defaultText }
    if ($answer.Trim().ToUpperInvariant() -eq 'A') {
        $answer = (@($defaults) + '3') -join ','
    }
    $mapping = @{ '1' = 'android'; '2' = 'windows'; '3' = 'relay'; '4' = 'onion' }
    $result = foreach ($value in ($answer -split ',')) {
        $key = $value.Trim()
        if (-not $mapping.ContainsKey($key)) { throw "Invalid component choice: $key" }
        $mapping[$key]
    }
    return @($result | Sort-Object -Unique)
}

function Confirm-TorcaWizardPlan {
    param([Parameter(Mandatory = $true)]$Plan)
    if (-not $Plan.ConfirmationWord -or $Confirm) { return }
    Write-Host ''
    Write-Host $Plan.Warning -ForegroundColor Yellow
    $answer = Read-Host "Type $($Plan.ConfirmationWord) to continue"
    if ($answer -cne $Plan.ConfirmationWord) { throw 'Workflow cancelled before any data was changed.' }
}

function Write-TorcaWizardPlan {
    param([Parameter(Mandatory = $true)]$Plan)
    Write-TorcaConsoleHeader -Title "Plan: $($Plan.Name)" -Details @{
        Devices = if ($Plan.DeviceNames.Count -gt 0) { $Plan.DeviceNames -join ', ' } else { 'none' }
        Build = $Plan.Configuration
        Clients = $Plan.BuildPolicy
        Relay = $Plan.RelayBuildPolicy
        Onion = $Plan.OnionPolicy
        Data = $Plan.ClientDataPolicy
    }
}

function Invoke-TorcaWizardOrchestrator {
    param([Parameter(Mandatory = $true)]$Plan)
    if ($Plan.Command -eq 'run') {
        $arguments = @{
            Command = 'run'
            Target = $Plan.Target
            Device = $Plan.DeviceIds
            Configuration = $Plan.Configuration
            OnionPolicy = 'Ensure'
            RelayBuildPolicy = 'Reuse'
            StackProvider = $StackProvider
            NonInteractive = $true
        }
    } else {
        $arguments = @{
            Command = 'deploy'
            Target = $Plan.Target
            Configuration = $Plan.Configuration
            Validation = $Plan.Validation
            OnionPolicy = $Plan.OnionPolicy
            RelayBuildPolicy = $Plan.RelayBuildPolicy
            StackProvider = $StackProvider
            DeploymentScope = $Plan.DeploymentScope
            ClientDataPolicy = $Plan.ClientDataPolicy
            BuildPolicy = $Plan.BuildPolicy
            InstallPolicy = $Plan.InstallPolicy
            RunPolicy = $Plan.RunPolicy
            NonInteractive = $true
        }
        if ($Plan.DeviceIds.Count -gt 0) { $arguments.Device = $Plan.DeviceIds }
        if ($Plan.ClientDataPolicy -ne 'Preserve') { $arguments.AllowDataReset = $true }
    }
    & $orchestrator @arguments
    if ($LASTEXITCODE -ne 0) { throw "Torca workflow failed with exit code $LASTEXITCODE." }
}

Show-TorcaWizardOverview

if ($Action -eq 'Menu') {
    $choice = Read-TorcaMenuChoice -Prompt 'What do you want to do?' -Default '1' -Options @(
        'Run current build - no build, install or data changes',
        'Redeploy current build - reinstall existing artifacts on every device',
        'Rebuild selected components - preserve client data and Onion address',
        'Full redeploy - rebuild everything, reset clients, keep Onion address',
        'Full redeploy + new Onion - reset everything and generate a new endpoint',
        'Relay maintenance',
        'Status and diagnostics',
        'Exit'
    )
    $Action = switch -Wildcard ($choice) {
        'Run current*' { 'Run' }
        'Redeploy current*' { 'Redeploy' }
        'Rebuild selected*' { 'Rebuild' }
        'Full redeploy +*' { 'FullRedeployNewOnion' }
        'Full redeploy -*' { 'FullRedeploy' }
        'Relay maintenance*' { 'RelayMenu' }
        'Status and diagnostics*' { 'DiagnosticsMenu' }
        default { 'Exit' }
    }
}

if ($Action -eq 'RelayMenu') {
    $choice = Read-TorcaMenuChoice -Prompt 'Relay maintenance' -Default '1' -Options @(
        'Restart - keep Onion identity and warm Tor cache',
        'Repair Tor cache - keep Onion identity, redownload directory data',
        'Rebuild relay server - keep Onion identity and Tor cache',
        'Back / exit'
    )
    $Action = if ($choice -like 'Restart*') {
        'RelayRestart'
    } elseif ($choice -like 'Repair*') {
        'RelayRepair'
    } elseif ($choice -like 'Rebuild*') {
        'RelayRebuild'
    } else {
        'Exit'
    }
}

if ($Action -eq 'DiagnosticsMenu') {
    $choice = Read-TorcaMenuChoice -Prompt 'Status and diagnostics' -Default '1' -Options @(
        'Show relay and device status',
        'Collect logs.zip from desktop and every connected Android device',
        'Back / exit'
    )
    $Action = if ($choice -like 'Show*') { 'Status' } elseif ($choice -like 'Collect*') { 'Collect' } else { 'Exit' }
}

if ($Action -eq 'Exit') { return }
if ($Action -eq 'Status') {
    & $orchestrator -Command status -StackProvider $StackProvider -NonInteractive
    return
}
if ($Action -eq 'Collect') {
    & (Join-Path $PSScriptRoot 'zip.ps1')
    return
}

$selectedDevices = @()
$selectedComponents = @()
$planConfiguration = $resolvedConfiguration
$plan = $null

switch ($Action) {
    'Run' {
        $selectedDevices = $deployable
        if ($selectedDevices.Count -eq 0) { throw 'No deployable device is connected.' }
        $plan = [pscustomobject]@{
            Name = 'Run current build'; Command = 'run'; Target = Get-TorcaWizardTarget $selectedDevices
            DeviceIds = @($selectedDevices | ForEach-Object { $_.Id }); DeviceNames = @($selectedDevices | ForEach-Object { "$($_.Platform):$($_.Name)" })
            Configuration = $planConfiguration; Validation = 'Skip'; DeploymentScope = 'ClientsAndRelay'
            OnionPolicy = 'Ensure'; RelayBuildPolicy = 'Reuse'; ClientDataPolicy = 'Preserve'
            BuildPolicy = 'Existing'; InstallPolicy = 'Skip'; RunPolicy = 'Restart'
            ConfirmationWord = $null; Warning = $null
        }
    }
    'Redeploy' {
        $selectedDevices = $deployable
        if ($selectedDevices.Count -eq 0) { throw 'No deployable device is connected.' }
        $plan = [pscustomobject]@{
            Name = 'Redeploy current build'; Command = 'deploy'; Target = Get-TorcaWizardTarget $selectedDevices
            DeviceIds = @($selectedDevices | ForEach-Object { $_.Id }); DeviceNames = @($selectedDevices | ForEach-Object { "$($_.Platform):$($_.Name)" })
            Configuration = $planConfiguration; Validation = 'Skip'; DeploymentScope = 'ClientsAndRelay'
            OnionPolicy = 'Ensure'; RelayBuildPolicy = 'Reuse'; ClientDataPolicy = 'Preserve'
            BuildPolicy = 'Existing'; InstallPolicy = 'Selected'; RunPolicy = 'Restart'
            ConfirmationWord = $null; Warning = $null
        }
    }
    'Rebuild' {
        $selectedComponents = @(Read-TorcaRebuildComponents)
        if ($selectedComponents -contains 'onion') {
            $selectedComponents = @((@('relay', 'onion') + @($deployable | ForEach-Object { $_.Platform })) | Sort-Object -Unique)
        }
        foreach ($platform in @('android', 'windows')) {
            if ($selectedComponents -contains $platform -and -not ($deployable | Where-Object Platform -eq $platform)) {
                throw "No deployable $platform device was detected."
            }
        }
        $selectedDevices = @($deployable | Where-Object { $selectedComponents -contains $_.Platform })
        $hasClients = $selectedDevices.Count -gt 0
        if (-not $hasClients -and -not ($selectedComponents -contains 'relay')) {
            throw 'Select at least one available client platform or the relay server.'
        }
        if ($hasClients) { $planConfiguration = Read-TorcaBuildConfiguration }
        $rotate = $selectedComponents -contains 'onion'
        $plan = [pscustomobject]@{
            Name = if ($rotate) { 'Rebuild selected components + new Onion' } else { 'Rebuild selected components' }
            Command = 'deploy'; Target = if ($hasClients) { Get-TorcaWizardTarget $selectedDevices } else { 'auto' }
            DeviceIds = @($selectedDevices | ForEach-Object { $_.Id }); DeviceNames = @($selectedDevices | ForEach-Object { "$($_.Platform):$($_.Name)" })
            Configuration = $planConfiguration; Validation = if ($planConfiguration -eq 'release') { 'Full' } else { 'Quick' }
            DeploymentScope = if ($hasClients) { 'ClientsAndRelay' } else { 'RelayOnly' }
            OnionPolicy = if ($rotate) { 'Rotate' } else { 'Ensure' }
            RelayBuildPolicy = if ($selectedComponents -contains 'relay') { 'Rebuild' } else { 'Reuse' }
            ClientDataPolicy = 'Preserve'; BuildPolicy = if ($hasClients) { 'Rebuild' } else { 'IfRequired' }
            InstallPolicy = if ($hasClients) { 'Selected' } else { 'Skip' }
            RunPolicy = if ($hasClients) { 'Restart' } else { 'Skip' }
            ConfirmationWord = if ($rotate) { 'ROTATE' } else { $null }
            Warning = if ($rotate) { 'This replaces the relay Onion address and rebuilds every connected client against the new endpoint.' } else { $null }
        }
    }
    { $_ -in @('FullRedeploy', 'FullRedeployNewOnion') } {
        $selectedDevices = $deployable
        if ($selectedDevices.Count -eq 0) { throw 'No deployable device is connected.' }
        $planConfiguration = Read-TorcaBuildConfiguration
        $rotate = $Action -eq 'FullRedeployNewOnion'
        $plan = [pscustomobject]@{
            Name = if ($rotate) { 'Full redeploy + new Onion' } else { 'Full redeploy' }
            Command = 'deploy'; Target = Get-TorcaWizardTarget $selectedDevices
            DeviceIds = @($selectedDevices | ForEach-Object { $_.Id }); DeviceNames = @($selectedDevices | ForEach-Object { "$($_.Platform):$($_.Name)" })
            Configuration = $planConfiguration; Validation = if ($planConfiguration -eq 'release') { 'Full' } else { 'Quick' }
            DeploymentScope = 'FullReset'; OnionPolicy = if ($rotate) { 'Rotate' } else { 'Ensure' }
            RelayBuildPolicy = 'Rebuild'; ClientDataPolicy = 'ResetSelected'; BuildPolicy = 'Rebuild'
            InstallPolicy = 'Selected'; RunPolicy = 'Restart'; ConfirmationWord = 'RESET'
            Warning = if ($rotate) {
                'This deletes client identities/databases/Tor caches, clears relay runtime state and creates a new Onion address.'
            } else {
                'This deletes client identities/databases/Tor caches and clears relay runtime state. The Onion address and relay Tor cache are preserved.'
            }
        }
    }
    { $_ -in @('RelayRestart', 'RelayRepair', 'RelayRebuild') } {
        $plan = [pscustomobject]@{
            Name = $Action; Command = 'deploy'; Target = 'auto'; DeviceIds = @(); DeviceNames = @()
            Configuration = $planConfiguration; Validation = 'Skip'; DeploymentScope = 'RelayOnly'
            OnionPolicy = if ($Action -eq 'RelayRestart') { 'Restart' } elseif ($Action -eq 'RelayRepair') { 'Repair' } else { 'Ensure' }
            RelayBuildPolicy = if ($Action -eq 'RelayRebuild') { 'Rebuild' } else { 'Reuse' }
            ClientDataPolicy = 'Preserve'; BuildPolicy = 'IfRequired'; InstallPolicy = 'Skip'; RunPolicy = 'Skip'
            ConfirmationWord = $null; Warning = $null
        }
    }
    default { throw "Unsupported wizard action: $Action" }
}

Write-TorcaWizardPlan -Plan $plan
if ($PlanOnly) {
    $plan | ConvertTo-Json -Depth 6
    return
}
Confirm-TorcaWizardPlan -Plan $plan
Invoke-TorcaWizardOrchestrator -Plan $plan
