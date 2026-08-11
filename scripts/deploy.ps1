[CmdletBinding()]
param(
    [ValidateSet('auto', 'windows', 'android', 'all')]
    [string]$Target = 'auto',
    [string[]]$Device,
    [ValidateSet('debug','release')]
    [string]$Configuration = 'release',
    [ValidateSet('Full','Quick','Skip')]
    [string]$Validation = 'Full',
    [ValidateSet('Ensure','Preserve','Restart','Repair','Rotate')]
    [string]$OnionPolicy = 'Ensure',
    [ValidateSet('IfRequired','Rebuild','Reuse')]
    [string]$RelayBuildPolicy = 'IfRequired',
    [ValidateSet('auto','docker','process')]
    [string]$StackProvider = 'auto',
    [ValidateSet('Preserve','ResetSelected','ResetAll')]
    [string]$ClientDataPolicy = 'Preserve',
    [ValidateSet('ClientsAndRelay','RelayOnly','FullReset')]
    [string]$DeploymentScope = 'ClientsAndRelay',
    [ValidateSet('IfRequired','Rebuild','Reuse','Existing')]
    [string]$BuildPolicy = 'IfRequired',
    [ValidateSet('Selected','Always','Skip')]
    [string]$InstallPolicy = 'Selected',
    [ValidateSet('Restart','Start','Skip')]
    [string]$RunPolicy = 'Restart',
    [switch]$NonInteractive,
    [switch]$Confirm,
    [switch]$AllowDataReset,
    [switch]$ReuseBuild,
    [switch]$Wizard,
    [switch]$SkipLaunch
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
if ($StackProvider -ne 'auto') { $env:TORCA_STACK_PROVIDER = $StackProvider }

if (-not $env:TORCA_ORCHESTRATED) {
    $explicitLifecyclePolicy = @(
        @(
            'DeploymentScope', 'OnionPolicy', 'RelayBuildPolicy', 'ClientDataPolicy',
            'BuildPolicy', 'InstallPolicy', 'RunPolicy'
        ) | Where-Object { $PSBoundParameters.ContainsKey($_) }
    )
    $useWizard = [bool]$Wizard -or (-not $NonInteractive -and $explicitLifecyclePolicy.Count -eq 0)
    if ($useWizard) {
        & (Join-Path $PSScriptRoot 'wizard.ps1')
        if ($LASTEXITCODE -ne 0) { throw "Wizard failed with code $LASTEXITCODE." }
        return
    }
    $arguments = @{
        Command = 'deploy'; Target = $Target
        OnionPolicy = $OnionPolicy
        RelayBuildPolicy = $RelayBuildPolicy
        StackProvider = $StackProvider
        DeploymentScope = $DeploymentScope
        InstallPolicy = $InstallPolicy; RunPolicy = $RunPolicy
    }
    if ($PSBoundParameters.ContainsKey('Configuration') -or $NonInteractive) { $arguments.Configuration = $Configuration }
    if ($PSBoundParameters.ContainsKey('Validation') -or $NonInteractive) { $arguments.Validation = $Validation }
    if ($PSBoundParameters.ContainsKey('ClientDataPolicy')) { $arguments.ClientDataPolicy = $ClientDataPolicy }
    if ($PSBoundParameters.ContainsKey('BuildPolicy')) { $arguments.BuildPolicy = $BuildPolicy }
    if ($Device) { $arguments.Device = $Device }
    if ($NonInteractive) { $arguments.NonInteractive = $true }
    if ($Confirm) { $arguments.Confirm = $true }
    if ($AllowDataReset) { $arguments.AllowDataReset = $true }
    if ($ReuseBuild) { $arguments.ReuseBuild = $true }
    if ($PSBoundParameters.ContainsKey('Verbose')) { $arguments.Verbose = $true }
    & (Join-Path $PSScriptRoot 'torca.ps1') @arguments
    if ($LASTEXITCODE -ne 0) { throw "Orchestrated deploy failed with code $LASTEXITCODE." }
    return
}

$resolved = $Target
if ($resolved -eq 'auto') {
    $resolved = if ($env:OS -eq 'Windows_NT') { 'windows' } else { 'android' }
}
$assetsModule = Join-Path $root 'scripts/modules/Torca.PlatformAssets.psm1'
Import-Module $assetsModule -Force -WarningAction SilentlyContinue -Verbose:$false
if ($resolved -in @('windows','all')) {
    Prepare-TorcaPlatformAssets -RepoRoot $root -Platform windows
}
if ($resolved -in @('android','all')) {
    Prepare-TorcaPlatformAssets -RepoRoot $root -Platform android
}

$module = Join-Path $root 'scripts/modules/Torca.BuildEngine.psm1'
Import-Module $module -Force -WarningAction SilentlyContinue -Verbose:$false
$deployArguments = @{ Target = $Target; Device = $Device }
if ($ReuseBuild) { $deployArguments.ReuseBuild = $true }
if ($SkipLaunch) { $deployArguments.SkipLaunch = $true }
Invoke-TorcaDeploy @deployArguments
