[CmdletBinding()]
param(
    [ValidateSet('menu','status','devices','stack','build','deploy','run','stop','logs','collect')]
    [string]$Command = 'menu',
    [ValidateSet('ensure','start','restart','repair','rotate','stop')]
    [string]$StackAction = 'ensure',
    [ValidateSet('auto','windows','android','all')]
    [string]$Target = 'auto',
    [ValidateSet('debug','release')]
    [string]$Configuration = 'debug',
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
    [string[]]$Device,
    [switch]$NonInteractive,
    [switch]$Confirm,
    [switch]$AllowDataReset,
    [switch]$ReuseBuild,
    [switch]$Wizard,
    [ValidateRange(1,100)][int]$LastRuns = 10,
    [ValidateSet('basic','extended','incident')][string]$Profile = 'extended',
    [switch]$IncludeLogcat,
    [switch]$KeepDirectory
)

$ErrorActionPreference = 'Stop'
$clientDataPolicySpecified = $PSBoundParameters.ContainsKey('ClientDataPolicy')
$buildPolicySpecified = $PSBoundParameters.ContainsKey('BuildPolicy')
$configurationSpecified = $PSBoundParameters.ContainsKey('Configuration')
$interactiveDataResetConfirmed = $false
$deployDefaultConfiguration = 'debug'
$runDeployWizard = [bool]$Wizard
$root = Split-Path -Parent $PSScriptRoot
$previousStackProvider = $env:TORCA_STACK_PROVIDER
if ($StackProvider -ne 'auto') { $env:TORCA_STACK_PROVIDER = $StackProvider }
$moduleRoot = Join-Path $PSScriptRoot 'modules'
foreach ($module in @('Torca.Core','Torca.Config','Torca.State','Torca.Devices','Torca.Data','Torca.Stack','Torca.PlatformAssets','Torca.Build','Torca.Ui')) {
    $modulePath = Join-Path $moduleRoot "$module.psm1"
    if (-not (Test-Path -LiteralPath $modulePath -PathType Leaf)) {
        throw "Torca module is missing: $modulePath"
    }
    Import-Module $modulePath -Force -ErrorAction Stop -Verbose:$false
}
$requiredCommands = @(
    'Get-TorcaPaths', 'Initialize-TorcaPaths', 'Read-TorcaJsonState',
    'Get-TorcaDevices', 'Get-TorcaScopedBuildPaths',
    'Assert-TorcaAndroidInstalledArtifact'
)
foreach ($requiredCommand in $requiredCommands) {
    if (-not (Get-Command $requiredCommand -ErrorAction SilentlyContinue)) {
        throw "Torca module loader did not expose required command: $requiredCommand"
    }
}
$paths = Get-TorcaPaths -RepoRoot $root
Initialize-TorcaPaths -Paths $paths
if ($ReuseBuild) { $BuildPolicy = 'Reuse' }
function Resolve-TorcaTarget {
    param([string]$Value)
    if ($Value -ne 'auto') { return $Value }
    if ($env:OS -eq 'Windows_NT') { return 'windows' }
    return 'android'
}

function Get-TorcaSha256Text {
    param([Parameter(Mandatory = $true)][string]$Text)
    $sha = [System.Security.Cryptography.SHA256]::Create()
    try {
        return ([BitConverter]::ToString($sha.ComputeHash([Text.Encoding]::UTF8.GetBytes($Text))).Replace('-', '')).ToLowerInvariant()
    } finally { $sha.Dispose() }
}

function Get-TorcaSelectedDevices {
    param([object[]]$Available)
    if ($Device) {
        return @($Available | Where-Object {
            $Device -contains $_.Id -and $_.CanInstall -and $_.CanRun
        })
    }
    if ($NonInteractive) {
        $resolved = Resolve-TorcaTarget $Target
        if ($resolved -eq 'all') { return @($Available | Where-Object { $_.CanInstall -and $_.CanRun }) }
        return @($Available | Where-Object { $_.Platform -eq $resolved -and $_.CanInstall -and $_.CanRun })
    }
    return @(Select-TorcaDevices -Devices $Available)
}

function Get-TorcaDeviceDeploymentManifest {
    param([Parameter(Mandatory = $true)][string]$DeviceId)
    $manifestPath = Get-TorcaDeviceManifestPath -Paths $paths -DeviceId $DeviceId
    if (-not (Test-Path -LiteralPath $manifestPath)) { return $null }
    try { return Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json }
    catch { Write-Warning "Ignoring unreadable device deployment manifest: $manifestPath"; return $null }
}

function Write-TorcaDeviceDeploymentManifest {
    param(
        [Parameter(Mandatory = $true)]$Device,
        [Parameter(Mandatory = $true)]$Release,
        [Parameter(Mandatory = $true)][string]$Endpoint,
        [Parameter(Mandatory = $true)][string]$BuildId,
        [Parameter(Mandatory = $true)][ValidateSet('debug','release')][string]$Configuration
    )
    $manifestPath = Get-TorcaDeviceManifestPath -Paths $paths -DeviceId $Device.Id
    # A deploy can be interrupted by an Android confirmation dialog or a
    # device disconnect.  Never leave a partially-written manifest that then
    # claims a different artifact is installed on the next wizard run.
    Write-TorcaJsonState -Path $manifestPath -Value ([pscustomobject]@{
        Schema = 2
        DeviceId = $Device.Id
        DeviceName = $Device.Name
        Platform = $Device.Platform
        ProductVersion = $Release.version
        BuildNumber = $Release.build
        BuildId = $BuildId
        Configuration = $Configuration
        StorageEpoch = $Release.storageEpoch
        SchemaVersion = $Release.schemaVersion
        ContractSchema = $Release.contractSchema
        WireVersion = $Release.wireVersion
        RelayEndpoint = $Endpoint
        ProviderEndpointHash = Get-TorcaSha256Text -Text $Endpoint
        DeployedAt = [DateTime]::UtcNow.ToString('o')
        Verified = $true
    })
}

function Invoke-TorcaStackEnsure {
    param([string]$Policy, [string]$BuildPolicy = 'IfRequired')
    $result = Start-TorcaStack -Paths $paths -OnionPolicy $Policy `
        -ForceRebuild:($BuildPolicy -eq 'Rebuild') `
        -SkipSourceRebuild:($BuildPolicy -eq 'Reuse')
    $state = if ($result.OnionReachable) { 'ready' } else { 'running' }
    $readiness = if ($result.OnionReachable) { 'public onion reachable' } else { 'public onion warming' }
    Write-TorcaStage -Name 'Runtime stack' -State $state -Detail ("provider={0}, endpoint={1}, {2}" -f $result.Provider, $result.Endpoint, $readiness)
    return $result
}

if ($Command -eq 'menu' -or ($Command -eq 'deploy' -and $runDeployWizard)) {
    & (Join-Path $PSScriptRoot 'wizard.ps1')
    if ($LASTEXITCODE -ne 0) { throw "Wizard failed with code $LASTEXITCODE." }
    return
}

switch ($Command) {
    'status' {
        Get-TorcaStackStatus -Paths $paths | Format-List
        Get-TorcaDevices -FlutterRoot (Join-Path $root 'apps/client/flutter') | Format-Table
    }
    'devices' { Get-TorcaDevices -FlutterRoot (Join-Path $root 'apps/client/flutter') | Format-Table }
    'stop' { Stop-TorcaStack -Paths $paths }
    'logs' {
        $stackStatus = Get-TorcaStackStatus -Paths $paths
        if ($stackStatus.Provider -eq 'docker') {
            Write-TorcaConsoleHeader -Title 'Torca relay logs' -Details @{ Provider = 'Docker'; State = $stackStatus.ContainerState; Health = $stackStatus.ContainerHealth }
            & docker compose -f $paths.DockerCompose logs --no-color --tail 120 relay
        } else {
            if (Test-Path $paths.RelayLog) { Get-Content $paths.RelayLog -Tail 80 }
            if (Test-Path $paths.RelayErrorLog) { Get-Content $paths.RelayErrorLog -Tail 40 }
        }
    }
    'collect' {
        $collectParameters = @{
            LastRuns = $LastRuns
            Target = $Target
            Profile = $Profile
            IncludeLogcat = [bool]$IncludeLogcat
            KeepDirectory = [bool]$KeepDirectory
        }
        if ($Device) { $collectParameters.Device = $Device }
        & (Join-Path $PSScriptRoot 'collect.ps1') @collectParameters
        if ($LASTEXITCODE -ne 0) { throw "Log collection failed with code $LASTEXITCODE." }
    }
    'stack' {
        if ($StackAction -eq 'stop') { Stop-TorcaStack -Paths $paths }
        else {
            $policy = if ($StackAction -eq 'rotate') { 'Rotate' } elseif ($StackAction -eq 'repair') { 'Repair' } elseif ($StackAction -eq 'restart') { 'Restart' } else { 'Ensure' }
            Write-TorcaConsoleHeader -Title 'Torca stack' -Details @{ Provider = $StackProvider; Onion = $policy }
            Invoke-TorcaStackEnsure -Policy $policy -BuildPolicy $RelayBuildPolicy
        }
    }
    'build' {
        Write-TorcaConsoleHeader -Title 'Torca build' -Details @{ Target = (Resolve-TorcaTarget $Target); Configuration = $Configuration; Policy = $BuildPolicy }
        $stack = Invoke-TorcaStackEnsure -Policy $OnionPolicy -BuildPolicy $RelayBuildPolicy
        Write-TorcaStage -Name 'Preflight' -State 'running' -Detail 'Checking device connectivity, endpoint provenance and toolchain'
        $preflightErrors = [System.Collections.Generic.List[string]]::new()
        foreach ($item in $selected) {
            if (-not $item.CanInstall -or -not $item.CanRun -or [string]$item.State -notin @('online','device')) {
                $preflightErrors.Add("$($item.Platform):$($item.Name) is not online/deployable")
            }
            if ($item.Platform -eq 'android' -and -not (Get-Command adb -ErrorAction SilentlyContinue)) {
                $preflightErrors.Add('adb is required for the selected Android device')
            }
        }
        if ([string]$stack.Endpoint -notmatch '^[a-z2-7]{56}\.onion:\d+$') {
            $preflightErrors.Add("Relay endpoint is not a v3 onion endpoint: $($stack.Endpoint)")
        }
        if ($preflightErrors.Count -gt 0) {
            Write-TorcaStage -Name 'Preflight' -State 'failed' -Detail ($preflightErrors -join '; ')
            throw "Preflight failed: $($preflightErrors -join '; ')"
        }
        Write-TorcaStage -Name 'Preflight' -State 'ready' -Detail "endpoint=$($stack.Endpoint), endpointHash=$(Get-TorcaSha256Text -Text $stack.Endpoint)"
        $androidAuthorizationDevices = @($selected | Where-Object { $_.Platform -eq 'android' -and $_.Id -match ':\d+$' })
        if ($androidAuthorizationDevices.Count -gt 0) {
            $devicesText = ($androidAuthorizationDevices | ForEach-Object Id) -join ', '
            Write-TorcaStage -Name 'Android install authorization' -State 'warning' -Detail "Keep $devicesText unlocked; approve ADB installation. HyperOS/MIUI also requires Developer options > USB debugging (Security settings) / Install via USB."
            if (-not $NonInteractive) {
                $authorizationChoice = Read-TorcaMenuChoice 'Android Wi-Fi installation authorization' @(
                    'Continue - device is unlocked and ADB/USB installation is allowed',
                    'Abort deployment to change Android security settings'
                ) '1'
                if ($authorizationChoice -like 'Abort*') { throw 'Deployment cancelled before the selected device data reset.' }
            }
        }
        $endpointMismatches = @($selected | ForEach-Object {
            $previous = Get-TorcaDeviceDeploymentManifest -DeviceId $_.Id
            if ($previous -and $previous.PSObject.Properties.Name -contains 'RelayEndpoint' -and
                [string]$previous.RelayEndpoint -ne [string]$stack.Endpoint) {
                [pscustomobject]@{ Device = $_; Previous = $previous.RelayEndpoint; Current = $stack.Endpoint }
            }
        })
        if ($endpointMismatches.Count -gt 0) {
            $summary = ($endpointMismatches | ForEach-Object { "$($_.Device.Platform):$($_.Device.Name)" }) -join ', '
            Write-TorcaStage -Name 'Relay endpoint' -State 'warning' -Detail "Installed artifact endpoint differs on $summary; matching native rebuild is required"
        } else {
            Write-TorcaStage -Name 'Relay endpoint' -State 'ready' -Detail "Embedded at build time: $($stack.Endpoint)"
        }
        $buildTarget = Resolve-TorcaTarget $Target
        if ($buildTarget -in @('windows','all')) { Prepare-TorcaPlatformAssets -RepoRoot $root -Platform windows }
        if ($buildTarget -in @('android','all')) { Prepare-TorcaPlatformAssets -RepoRoot $root -Platform android }
        Clear-TorcaBuildSourceFingerprintCache
        $frozenSourceFingerprint = Get-TorcaBuildSourceFingerprint -RepoRoot $root
        $frozenBuildId = Get-TorcaBuildId -RepoRoot $root -Endpoint $stack.Endpoint -Target $buildTarget -Configuration $Configuration
        $required = Test-TorcaBuildRequired -Paths $paths -Endpoint $stack.Endpoint -Target $buildTarget -Configuration $Configuration
        if ($BuildPolicy -eq 'Reuse' -and $required) { throw 'Requested build reuse, but matching artifacts are not available.' }
        if ($BuildPolicy -eq 'Rebuild' -or ($BuildPolicy -eq 'IfRequired' -and $required)) {
            Invoke-TorcaClientBuild -RepoRoot $root -Target $buildTarget -Configuration $Configuration -Endpoint $stack.Endpoint -Validation $Validation
            Write-TorcaBuildManifest -Paths $paths -Endpoint $stack.Endpoint -Targets @($buildTarget) -Configuration $Configuration -BuildId $frozenBuildId -SourceFingerprint $frozenSourceFingerprint
        } else { Write-TorcaStage -Name 'Build artifacts' -State 'ready' -Detail 'Reused existing verified artifacts' }
    }
    'run' {
        $available = @(Get-TorcaDevices -FlutterRoot (Join-Path $root 'apps/client/flutter'))
        $selected = @(Get-TorcaSelectedDevices -Available $available)
        if ($selected.Count -eq 0) { throw 'No runnable device was detected.' }
        Write-TorcaConsoleHeader -Title 'Torca run' -Details @{
            Configuration = $Configuration
            Devices = (($selected | ForEach-Object { $_.Platform + ':' + $_.Name }) -join ', ')
        }
        $stack = Invoke-TorcaStackEnsure -Policy $OnionPolicy -BuildPolicy $RelayBuildPolicy
        $launches = @()
        foreach ($item in $selected) {
            $deviceManifest = Get-TorcaDeviceDeploymentManifest -DeviceId $item.Id
            if ($deviceManifest -and $deviceManifest.PSObject.Properties.Name -contains 'RelayEndpoint' -and
                [string]$deviceManifest.RelayEndpoint -ne [string]$stack.Endpoint) {
                throw "Current build on $($item.Platform):$($item.Name) uses relay $($deviceManifest.RelayEndpoint), but the active relay is $($stack.Endpoint). Choose Rebuild instead of Run."
            }
            if ($deviceManifest -and $deviceManifest.PSObject.Properties.Name -contains 'Configuration' -and
                [string]$deviceManifest.Configuration -ne $Configuration) {
                throw "Current build on $($item.Platform):$($item.Name) is $($deviceManifest.Configuration), but the wizard selected $Configuration. Rebuild/redeploy that configuration first."
            }
            $expectedBuildId = if ($deviceManifest) { [string]$deviceManifest.BuildId } else { $null }
            Write-TorcaStage -Name "$($item.Platform) launch" -State 'running' -Detail $item.Name
            $launch = Invoke-TorcaClientRun -RepoRoot $root -Target $item.Platform -Device $item.Id -Configuration $Configuration -Installed -ExpectedBuildId $expectedBuildId -DeferHealthCheck
            $launches += [pscustomobject]@{ Item = $item; Launch = $launch; BuildId = $expectedBuildId }
        }
        foreach ($entry in $launches) {
            if ($entry.Launch.Platform -eq 'windows') {
                Wait-TorcaClientLaunch -Platform windows -ExpectedBuildId $entry.BuildId -ExpectedWindowsProcessId $entry.Launch.ProcessId -ExpectedWindowsExecutable $entry.Launch.Executable
            } else {
                Wait-TorcaClientLaunch -Platform android -Device $entry.Launch.Device -ExpectedBuildId $entry.BuildId
            }
            Write-TorcaStage -Name "$($entry.Item.Platform) launch" -State 'ready' -Detail 'Existing installed/built client reached NETWORK_READY'
        }
    }
    'deploy' {
        if ($DeploymentScope -eq 'RelayOnly') {
            if ($OnionPolicy -eq 'Rotate') {
                throw 'Relay-only deployment cannot rotate the onion endpoint because clients must be rebuilt with the new address.'
            }
            if ($ClientDataPolicy -ne 'Preserve') {
                throw 'Relay-only deployment cannot reset client data. Use ClientsAndRelay or FullReset instead.'
            }
            Write-TorcaConsoleHeader -Title 'Torca relay-only maintenance' -Details @{ Provider = $StackProvider; Onion = $OnionPolicy; RelayBuild = $RelayBuildPolicy; Clients = 'untouched' }
            $stack = Invoke-TorcaStackEnsure -Policy $OnionPolicy -BuildPolicy $RelayBuildPolicy
            Write-TorcaStage -Name 'Client Tor state' -State 'ready' -Detail 'No devices selected; builds, installs, launches and client Arti caches were left untouched'
            Write-TorcaStage -Name 'Relay-only maintenance' -State 'ready' -Detail "endpoint=$($stack.Endpoint), provider=$($stack.Provider)"
            return
        }
        if (-not $NonInteractive -and -not $configurationSpecified) {
            $configurationChoice = Read-TorcaMenuChoice 'Build configuration' @(
                'Debug - fastest iterative build, diagnostics enabled',
                'Release - optimized production artifact, slower build'
            ) $(if ($deployDefaultConfiguration -eq 'release') { '2' } else { '1' })
            $Configuration = if ($configurationChoice -like 'Release*') { 'release' } else { 'debug' }
            if (-not $PSBoundParameters.ContainsKey('Validation')) {
                $Validation = if ($Configuration -eq 'release') { 'Full' } else { 'Quick' }
            }
        }
        $available = @(Get-TorcaDevices -FlutterRoot (Join-Path $root 'apps/client/flutter'))
        $selected = @(
            if ($InstallPolicy -eq 'Always') {
                $available | Where-Object { $_.CanInstall -and $_.CanRun }
            } else {
                Get-TorcaSelectedDevices -Available $available
            }
        )
        if ($selected.Count -eq 0) { throw 'No deployable device selected.' }
        $release = Get-Content (Join-Path $root 'release/version.json') -Raw | ConvertFrom-Json
        $epochMismatches = @($selected | ForEach-Object {
            $previous = Get-TorcaDeviceDeploymentManifest -DeviceId $_.Id
            if ($previous -and [string]$previous.StorageEpoch -ne [string]$release.storageEpoch) {
                [pscustomobject]@{ Device = $_; Previous = $previous.StorageEpoch; Current = $release.storageEpoch }
            }
        })
        if ($DeploymentScope -eq 'FullReset') {
            if ($ClientDataPolicy -eq 'Preserve') { $ClientDataPolicy = 'ResetSelected' }
            if (-not $NonInteractive) {
                $deviceSummary = ($selected | ForEach-Object { "$($_.Platform):$($_.Name)" }) -join ', '
                $resetChoice = Read-TorcaMenuChoice "Confirm full reset on selected devices ($deviceSummary)" @(
                    'Abort - keep identity, database and Tor cache',
                    'RESET EVERYTHING - delete identity, database, preferences and persistent Tor cache'
                ) '1'
                if ($resetChoice -like 'Abort*') { throw 'Full client reset cancelled before any device was modified.' }
                $interactiveDataResetConfirmed = $true
            }
        } elseif (-not $NonInteractive -and -not $clientDataPolicySpecified) {
            $deviceSummary = ($selected | ForEach-Object { "$($_.Platform):$($_.Name)" }) -join ', '
            $dataChoice = Read-TorcaMenuChoice "Client data on selected devices ($deviceSummary)" @(
                'Keep existing identity and local database (recommended)',
                'RESET EVERYTHING - delete identity, local database, Tor cache and preferences; next launch creates a new identity'
            ) '1'
            if ($dataChoice -like 'RESET EVERYTHING*') {
                $ClientDataPolicy = 'ResetSelected'
                $interactiveDataResetConfirmed = $true
            } else {
                $ClientDataPolicy = 'Preserve'
            }
        }
        if ($epochMismatches.Count -gt 0 -and $ClientDataPolicy -eq 'Preserve') {
            $summary = ($epochMismatches | ForEach-Object { "$($_.Device.Platform):$($_.Device.Name) ($($_.Previous) -> $($_.Current))" }) -join ', '
            Write-TorcaStage -Name 'Storage epoch' -State 'warning' -Detail "Installed epoch differs: $summary"
            if ($NonInteractive -and -not $AllowDataReset) {
                throw 'Storage epoch changed on selected devices; non-interactive deployment requires -AllowDataReset.'
            }
            if (-not $NonInteractive) {
                $epochChoice = Read-TorcaMenuChoice 'Storage epoch changed; reset selected device data before installation?' @(
                    'RESET selected devices and continue',
                    'Abort deployment'
                ) '1'
                if ($epochChoice -like 'Abort*') { throw 'Deployment cancelled because storage epoch changed.' }
                $ClientDataPolicy = 'ResetSelected'
                $interactiveDataResetConfirmed = $true
            } else {
                $ClientDataPolicy = 'ResetSelected'
            }
        }
        Write-TorcaConsoleHeader -Title 'Torca deploy' -Details @{ Scope = $DeploymentScope; Target = $Target; Configuration = $Configuration; Devices = (($selected | ForEach-Object { $_.Platform + ':' + $_.Name }) -join ', '); RelayBuild = $RelayBuildPolicy; Build = $BuildPolicy; Install = $InstallPolicy; Run = $RunPolicy }
        Write-TorcaStage -Name 'Devices' -State 'ready' -Detail ("{0} selected" -f $selected.Count)
        if ($ClientDataPolicy -ne 'Preserve' -and -not ($Confirm -or $AllowDataReset -or $interactiveDataResetConfirmed)) { throw 'Reset danych wymaga jawnego -AllowDataReset.' }
        if ($ClientDataPolicy -ne 'Preserve' -and $NonInteractive -and -not $AllowDataReset) { throw 'Non-interactive reset requires -AllowDataReset.' }
        $dataDetail = if ($ClientDataPolicy -eq 'Preserve') {
            'Existing identity, database and persistent client Tor cache will be preserved'
        } else {
            'Confirmed full reset: next launch requires a cold Tor bootstrap and may spend 15-90+ seconds downloading directory data'
        }
        Write-TorcaStage -Name 'Client data' -State $(if ($ClientDataPolicy -eq 'Preserve') { 'ready' } else { 'warning' }) -Detail $dataDetail
        $stack = Invoke-TorcaStackEnsure -Policy $OnionPolicy -BuildPolicy $RelayBuildPolicy
        if ($OnionPolicy -eq 'Rotate') {
            Write-TorcaStage -Name 'Relay endpoint rotation' -State 'ready' -Detail "New endpoint=$($stack.Endpoint); a matching client rebuild and installation are required"
        }
        if ($ClientDataPolicy -eq 'ResetAll') {
            $selected = @($available | Where-Object { $_.CanInstall -and $_.CanRun })
        }
        $android = @($selected | Where-Object Platform -eq 'android')
        $windows = @($selected | Where-Object Platform -eq 'windows')
        $buildTarget = if ($windows.Count -gt 0 -and $android.Count -gt 0) { 'all' } elseif ($android.Count -gt 0) { 'android' } else { 'windows' }
        # Platform overlays are source inputs. Materialize them before freezing
        # the identity so the manifest, native library and Flutter package all
        # receive exactly the same fingerprint.
        if ($buildTarget -in @('windows','all')) { Prepare-TorcaPlatformAssets -RepoRoot $root -Platform windows }
        if ($buildTarget -in @('android','all')) { Prepare-TorcaPlatformAssets -RepoRoot $root -Platform android }
        Clear-TorcaBuildSourceFingerprintCache
        $existingManifest = if ($BuildPolicy -eq 'Existing') {
            Get-TorcaExistingBuildManifest -Paths $paths -Endpoint $stack.Endpoint -Target $buildTarget -Configuration $Configuration
        } else { $null }
        if ($BuildPolicy -eq 'Existing' -and -not $existingManifest) {
            throw "No existing $buildTarget/$Configuration artifact matches relay endpoint $($stack.Endpoint). Choose Rebuild instead."
        }
        $expectedSourceFingerprint = if ($existingManifest) {
            [string]$existingManifest.SourceFingerprint
        } else {
            Get-TorcaBuildSourceFingerprint -RepoRoot $root
        }
        $expectedBuildId = if ($existingManifest) {
            [string]$existingManifest.BuildId
        } else {
            Get-TorcaBuildId -RepoRoot $root -Endpoint $stack.Endpoint -Target $buildTarget -Configuration $Configuration
        }
        Write-TorcaStage -Name 'Build identity' -State 'ready' -Detail "buildId=$expectedBuildId, endpointHash=$(Get-TorcaSha256Text -Text $stack.Endpoint)"
        $buildMismatches = @($selected | ForEach-Object {
            $previous = Get-TorcaDeviceDeploymentManifest -DeviceId $_.Id
            if ($previous -and $previous.PSObject.Properties.Name -contains 'BuildId' -and
                -not [string]::IsNullOrWhiteSpace([string]$previous.BuildId) -and
                [string]$previous.BuildId -ne $expectedBuildId) {
                [pscustomobject]@{ Device = $_; Previous = $previous.BuildId; Current = $expectedBuildId }
            }
        })
        if ($buildMismatches.Count -gt 0) {
            $summary = ($buildMismatches | ForEach-Object { "$($_.Device.Platform):$($_.Device.Name)" }) -join ', '
            Write-TorcaStage -Name 'Build identity' -State 'warning' -Detail "Installed build ID differs on $summary; install will use $expectedBuildId"
        }
        $required = Test-TorcaBuildRequired -Paths $paths -Endpoint $stack.Endpoint -Target $buildTarget -Configuration $Configuration
        if ($BuildPolicy -eq 'Reuse' -and $required) { throw 'Requested build reuse, but matching artifacts are not available.' }
        if (-not $NonInteractive -and -not $buildPolicySpecified -and -not $ReuseBuild) {
            $manifestState = if ($required) { 'no verified matching artifact is available' } else { 'a verified matching artifact is available' }
            $buildChoice = if ($required) {
                Read-TorcaMenuChoice "Build decision ($buildTarget/$Configuration; relay $($stack.Endpoint)) - $manifestState" @(
                    'Rebuild now (required)',
                    'Abort deployment'
                ) '1'
            } else {
                Read-TorcaMenuChoice "Build decision ($buildTarget/$Configuration; relay $($stack.Endpoint)) - $manifestState" @(
                    'Reuse verified artifact',
                    'Rebuild anyway',
                    'Abort deployment'
                ) '1'
            }
            if ($buildChoice -like 'Abort*') { throw 'Deployment cancelled before data reset or installation.' }
            $BuildPolicy = if ($buildChoice -like 'Rebuild*') { 'Rebuild' } else { 'Reuse' }
        }
        $buildDetail = if ($BuildPolicy -eq 'Rebuild') {
            'Rebuild selected; endpoint and source fingerprint will be embedded'
        } elseif ($BuildPolicy -eq 'Existing') {
            'Existing artifact selected; current source changes are intentionally ignored'
        } else {
            'Verified artifact reuse selected'
        }
        Write-TorcaStage -Name 'Build decision' -State 'ready' -Detail $buildDetail
        if ($BuildPolicy -eq 'Rebuild' -or ($BuildPolicy -eq 'IfRequired' -and $required)) {
            Write-TorcaStage -Name 'Application build' -State 'running' -Detail "$buildTarget / $Configuration"
            Invoke-TorcaClientBuild -RepoRoot $root -Target $buildTarget -Configuration $Configuration -Endpoint $stack.Endpoint -Validation $Validation
            Write-TorcaBuildManifest -Paths $paths -Endpoint $stack.Endpoint -Targets @($buildTarget) -Configuration $Configuration -BuildId $expectedBuildId -SourceFingerprint $expectedSourceFingerprint
            Write-TorcaStage -Name 'Application build' -State 'ready' -Detail 'Artifact manifest verified'
        } else { Write-TorcaStage -Name 'Application build' -State 'ready' -Detail 'Reused existing verified artifacts' }
        # Persist the complete user intent before reset/install/launch. Those
        # external stages can fail independently (for example an Android
        # confirmation being rejected), and UseLast must still resume them.
        Write-TorcaJsonState -Path $paths.LastDeployFile -Value ([pscustomobject]@{
            Schema = 1
            Target = $buildTarget
            Devices = @($selected | ForEach-Object { $_.Id })
            Configuration = $Configuration
            Validation = $Validation
            OnionPolicy = $OnionPolicy
            RelayBuildPolicy = 'IfRequired'
            StackProvider = $StackProvider
            DeploymentScope = 'ClientsAndRelay'
            ClientDataPolicy = 'Preserve'
            BuildPolicy = 'IfRequired'
            InstallPolicy = $InstallPolicy
            RunPolicy = $RunPolicy
            UpdatedAt = [DateTime]::UtcNow.ToString('o')
            Completed = $false
        })
        Write-TorcaStage -Name 'Deploy preset' -State 'ready' -Detail 'Saved before device mutation; UseLast can resume an interrupted deployment'
        if ($ClientDataPolicy -ne 'Preserve') {
            Write-TorcaStage -Name 'Client data reset' -State 'running' -Detail 'Build succeeded; resetting selected devices immediately before installation'
            Reset-TorcaClientData -Devices $selected
            Write-TorcaStage -Name 'Client data reset' -State 'ready' -Detail 'Selected application data reset completed'
        }
        if ($Configuration -eq 'release') {
            $packageDevice = if ($InstallPolicy -eq 'Skip') { $null } else { ($selected | Where-Object Platform -eq 'android' | Select-Object -First 1).Id }
            Write-TorcaStage -Name 'Release package' -State 'running' -Detail 'Checking artifact and native library hashes'
            Invoke-TorcaClientReleaseDeploy -RepoRoot $root -Target $buildTarget -Device $packageDevice -Endpoint $stack.Endpoint -SkipLaunch
            Write-TorcaStage -Name 'Release package' -State 'ready' -Detail 'Package verification completed'
        }
        if ($InstallPolicy -ne 'Skip') {
            foreach ($item in $selected) {
                if ($item.Platform -eq 'android' -and $Configuration -ne 'release') { Install-TorcaClient -RepoRoot $root -Device $item.Id -Configuration $Configuration }
                elseif ($item.Platform -eq 'windows') { Write-Host 'Windows artifact built; use run command to launch it.' }
            }
        }
        if ($RunPolicy -ne 'Skip') {
            $launches = @()
            foreach ($item in $selected) {
                Write-TorcaStage -Name "$($item.Platform) launch" -State 'running' -Detail $item.Name
                $launch = Invoke-TorcaClientRun -RepoRoot $root -Target $item.Platform -Device $item.Id -Configuration $Configuration -Installed -ExpectedBuildId $expectedBuildId -DeferHealthCheck
                $launches += [pscustomobject]@{ Item = $item; Launch = $launch }
            }
            # Start every selected client first so Windows and Android perform
            # their independent Tor warm-up concurrently. Health verification
            # remains deterministic and reports each device separately.
            foreach ($entry in $launches) {
                if ($entry.Launch.Platform -eq 'windows') {
                    Wait-TorcaClientLaunch -Platform windows -ExpectedBuildId $expectedBuildId -ExpectedWindowsProcessId $entry.Launch.ProcessId -ExpectedWindowsExecutable $entry.Launch.Executable
                } else {
                    Wait-TorcaClientLaunch -Platform android -Device $entry.Launch.Device -ExpectedBuildId $expectedBuildId
                }
                Write-TorcaStage -Name "$($entry.Item.Platform) launch" -State 'ready' -Detail 'Process, native runtime and NETWORK_READY verified'
            }
            Write-TorcaStage -Name 'Application launch' -State 'ready' -Detail 'Launch command issued'
            Write-TorcaStage -Name 'Launch health' -State 'ready' -Detail "All selected clients reached NETWORK_READY; incident logs: $(Join-Path $root 'logs/collected')"
        }
        if ($InstallPolicy -ne 'Skip' -or $RunPolicy -ne 'Skip') {
            foreach ($item in $selected) {
                Write-TorcaDeviceDeploymentManifest -Device $item -Release $release -Endpoint $stack.Endpoint -BuildId $expectedBuildId -Configuration $Configuration
            }
            Write-TorcaStage -Name 'Device manifest' -State 'ready' -Detail 'Installed storage epoch recorded per device'
        }
        Write-TorcaJsonState -Path $paths.LastDeployFile -Value ([pscustomobject]@{
            Schema = 1
            Target = $buildTarget
            Devices = @($selected | ForEach-Object { $_.Id })
            Configuration = $Configuration
            Validation = $Validation
            OnionPolicy = $OnionPolicy
            RelayBuildPolicy = 'IfRequired'
            StackProvider = $StackProvider
            DeploymentScope = 'ClientsAndRelay'
            ClientDataPolicy = 'Preserve'
            BuildPolicy = 'IfRequired'
            InstallPolicy = $InstallPolicy
            RunPolicy = $RunPolicy
            UpdatedAt = [DateTime]::UtcNow.ToString('o')
            Completed = $true
        })
        Write-TorcaStage -Name 'Deploy preset' -State 'ready' -Detail "Saved for redeploy.ps1 -UseLast ($Configuration/$Validation)"
    }
}
