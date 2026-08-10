[CmdletBinding()]
param(
    [ValidateSet('menu','status','devices','stack','build','deploy','run','stop','logs','collect')]
    [string]$Command = 'menu',
    [ValidateSet('ensure','start','restart','rotate','stop')]
    [string]$StackAction = 'ensure',
    [ValidateSet('auto','windows','android','all')]
    [string]$Target = 'auto',
    [ValidateSet('debug','release')]
    [string]$Configuration = 'debug',
    [ValidateSet('Ensure','Preserve','Restart','Rotate')]
    [string]$OnionPolicy = 'Ensure',
    [ValidateSet('auto','docker','process')]
    [string]$StackProvider = 'auto',
    [ValidateSet('Preserve','ResetSelected','ResetAll')]
    [string]$ClientDataPolicy = 'Preserve',
    [ValidateSet('IfRequired','Rebuild','Reuse')]
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
    [ValidateRange(1,100)][int]$LastRuns = 10,
    [ValidateSet('basic','extended','incident')][string]$Profile = 'extended',
    [switch]$IncludeLogcat,
    [switch]$KeepDirectory
)

$ErrorActionPreference = 'Stop'
$clientDataPolicySpecified = $PSBoundParameters.ContainsKey('ClientDataPolicy')
$buildPolicySpecified = $PSBoundParameters.ContainsKey('BuildPolicy')
$interactiveDataResetConfirmed = $false
$root = Split-Path -Parent $PSScriptRoot
$previousStackProvider = $env:TORCA_STACK_PROVIDER
if ($StackProvider -ne 'auto') { $env:TORCA_STACK_PROVIDER = $StackProvider }
$moduleRoot = Join-Path $PSScriptRoot 'modules'
foreach ($module in @('Torca.Core','Torca.Config','Torca.State','Torca.Devices','Torca.Data','Torca.Stack','Torca.Build','Torca.Ui')) {
    Import-Module (Join-Path $moduleRoot "$module.psm1") -Force -WarningAction SilentlyContinue -Verbose:$false
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
        [Parameter(Mandatory = $true)][string]$BuildId
    )
    $manifestPath = Get-TorcaDeviceManifestPath -Paths $paths -DeviceId $Device.Id
    [pscustomobject]@{
        DeviceId = $Device.Id
        DeviceName = $Device.Name
        Platform = $Device.Platform
        ProductVersion = $Release.version
        BuildNumber = $Release.build
        BuildId = $BuildId
        StorageEpoch = $Release.storageEpoch
        SchemaVersion = $Release.schemaVersion
        ContractSchema = $Release.contractSchema
        WireVersion = $Release.wireVersion
        RelayEndpoint = $Endpoint
        RelayEndpointHash = Get-TorcaSha256Text -Text $Endpoint
        DeployedAt = [DateTime]::UtcNow.ToString('o')
    } | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $manifestPath -Encoding utf8
}

function Invoke-TorcaStackEnsure {
    param([string]$Policy)
    $result = Start-TorcaStack -Paths $paths -OnionPolicy $Policy
    Write-TorcaStage -Name 'Runtime stack' -State 'ready' -Detail ("provider={0}, endpoint={1}" -f $result.Provider, $result.Endpoint)
    return $result
}

if ($Command -eq 'menu') {
    $choice = Read-TorcaMenuChoice 'Torca' @('status', 'devices', 'stack ensure', 'deploy', 'build', 'run', 'collect', 'stop') '4'
    if ($choice -eq 'stack ensure') { $Command = 'stack'; $StackAction = 'ensure' } else { $Command = $choice }
    if ($Command -eq 'deploy' -and -not $NonInteractive) {
        $options = Get-TorcaInteractiveOptions
        $OnionPolicy = if ($options.OnionPolicy -like 'Restart*') { 'Restart' } elseif ($options.OnionPolicy -like 'Rotate*') { 'Rotate' } else { 'Ensure' }
        $InstallPolicy = if ($options.InstallPolicy -like 'Always*') { 'Always' } elseif ($options.InstallPolicy -like 'Skip*') { 'Skip' } else { 'Selected' }
        $RunPolicy = if ($options.RunPolicy -like 'Start*') { 'Start' } elseif ($options.RunPolicy -like 'Skip*') { 'Skip' } else { 'Restart' }
    }
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
        $arguments = @('-LastRuns', $LastRuns, '-Target', $Target, '-Profile', $Profile)
        if ($Device) { $arguments += @('-Device', $Device) }
        if ($IncludeLogcat) { $arguments += '-IncludeLogcat' }
        if ($KeepDirectory) { $arguments += '-KeepDirectory' }
        & (Join-Path $PSScriptRoot 'collect.ps1') @arguments
        if ($LASTEXITCODE -ne 0) { throw "Log collection failed with code $LASTEXITCODE." }
    }
    'stack' {
        if ($StackAction -eq 'stop') { Stop-TorcaStack -Paths $paths }
        else {
            $policy = if ($StackAction -eq 'rotate') { 'Rotate' } elseif ($StackAction -eq 'restart') { 'Restart' } else { 'Ensure' }
            Write-TorcaConsoleHeader -Title 'Torca stack' -Details @{ Provider = $StackProvider; Onion = $policy }
            Invoke-TorcaStackEnsure -Policy $policy
        }
    }
    'build' {
        Write-TorcaConsoleHeader -Title 'Torca build' -Details @{ Target = (Resolve-TorcaTarget $Target); Configuration = $Configuration; Policy = $BuildPolicy }
        $stack = Invoke-TorcaStackEnsure -Policy $OnionPolicy
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
        $required = Test-TorcaBuildRequired -Paths $paths -Endpoint $stack.Endpoint -Target $buildTarget -Configuration $Configuration
        if ($BuildPolicy -eq 'Reuse' -and $required) { throw 'Requested build reuse, but matching artifacts are not available.' }
        if ($BuildPolicy -eq 'Rebuild' -or ($BuildPolicy -eq 'IfRequired' -and $required)) {
            Invoke-TorcaClientBuild -RepoRoot $root -Target $buildTarget -Configuration $Configuration -Endpoint $stack.Endpoint
            Write-TorcaBuildManifest -Paths $paths -Endpoint $stack.Endpoint -Targets @($buildTarget) -Configuration $Configuration
        } else { Write-TorcaStage -Name 'Build artifacts' -State 'ready' -Detail 'Reused existing verified artifacts' }
    }
    'run' {
        Write-TorcaConsoleHeader -Title 'Torca run' -Details @{ Target = (Resolve-TorcaTarget $Target); Configuration = $Configuration }
        $stack = Invoke-TorcaStackEnsure -Policy $OnionPolicy
        Invoke-TorcaClientRun -RepoRoot $root -Target (Resolve-TorcaTarget $Target) -Device (($Device | Select-Object -First 1))
    }
    'deploy' {
        $available = @(Get-TorcaDevices -FlutterRoot (Join-Path $root 'apps/client/flutter'))
        $selected = @(Get-TorcaSelectedDevices -Available $available)
        if ($selected.Count -eq 0) { throw 'No deployable device selected.' }
        $release = Get-Content (Join-Path $root 'release/version.json') -Raw | ConvertFrom-Json
        $epochMismatches = @($selected | ForEach-Object {
            $previous = Get-TorcaDeviceDeploymentManifest -DeviceId $_.Id
            if ($previous -and [string]$previous.StorageEpoch -ne [string]$release.storageEpoch) {
                [pscustomobject]@{ Device = $_; Previous = $previous.StorageEpoch; Current = $release.storageEpoch }
            }
        })
        if (-not $NonInteractive -and -not $clientDataPolicySpecified) {
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
        Write-TorcaConsoleHeader -Title 'Torca deploy' -Details @{ Target = $Target; Configuration = $Configuration; Devices = (($selected | ForEach-Object { $_.Platform + ':' + $_.Name }) -join ', '); Build = $BuildPolicy; Install = $InstallPolicy; Run = $RunPolicy }
        Write-TorcaStage -Name 'Devices' -State 'ready' -Detail ("{0} selected" -f $selected.Count)
        if ($ClientDataPolicy -ne 'Preserve' -and -not ($Confirm -or $AllowDataReset -or $interactiveDataResetConfirmed)) { throw 'Reset danych wymaga jawnego -AllowDataReset.' }
        if ($ClientDataPolicy -ne 'Preserve' -and $NonInteractive -and -not $AllowDataReset) { throw 'Non-interactive reset requires -AllowDataReset.' }
        $dataDetail = if ($ClientDataPolicy -eq 'Preserve') { 'Existing identity and database will be preserved' } else { 'Confirmed full application-data reset before installation' }
        Write-TorcaStage -Name 'Client data' -State $(if ($ClientDataPolicy -eq 'Preserve') { 'ready' } else { 'warning' }) -Detail $dataDetail
        $stack = Invoke-TorcaStackEnsure -Policy $OnionPolicy
        if ($ClientDataPolicy -eq 'ResetAll') {
            $selected = @($available | Where-Object { $_.CanInstall -and $_.CanRun })
        }
        $android = @($selected | Where-Object Platform -eq 'android')
        $windows = @($selected | Where-Object Platform -eq 'windows')
        $buildTarget = if ($windows.Count -gt 0 -and $android.Count -gt 0) { 'all' } elseif ($android.Count -gt 0) { 'android' } else { 'windows' }
        $expectedBuildId = Get-TorcaBuildId -RepoRoot $root -Endpoint $stack.Endpoint -Target $buildTarget -Configuration $Configuration
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
        $buildDetail = if ($BuildPolicy -eq 'Rebuild') { 'Rebuild selected; endpoint and source fingerprint will be embedded' } else { 'Verified artifact reuse selected' }
        Write-TorcaStage -Name 'Build decision' -State 'ready' -Detail $buildDetail
        if ($ClientDataPolicy -ne 'Preserve') { Reset-TorcaClientData -Devices $selected }
        if ($BuildPolicy -eq 'Rebuild' -or ($BuildPolicy -eq 'IfRequired' -and $required)) {
            Write-TorcaStage -Name 'Application build' -State 'running' -Detail "$buildTarget / $Configuration"
            Invoke-TorcaClientBuild -RepoRoot $root -Target $buildTarget -Configuration $Configuration -Endpoint $stack.Endpoint
            Write-TorcaBuildManifest -Paths $paths -Endpoint $stack.Endpoint -Targets @($buildTarget) -Configuration $Configuration
            Write-TorcaStage -Name 'Application build' -State 'ready' -Detail 'Artifact manifest verified'
        } else { Write-TorcaStage -Name 'Application build' -State 'ready' -Detail 'Reused existing verified artifacts' }
        if ($Configuration -eq 'release') {
            $packageDevice = if ($InstallPolicy -eq 'Skip') { $null } else { ($selected | Where-Object Platform -eq 'android' | Select-Object -First 1).Id }
            Write-TorcaStage -Name 'Release package' -State 'running' -Detail 'Checking artifact and native library hashes'
            Invoke-TorcaClientReleaseDeploy -RepoRoot $root -Target $buildTarget -Device $packageDevice -Endpoint $stack.Endpoint
            Write-TorcaStage -Name 'Release package' -State 'ready' -Detail 'Package verification completed'
        }
        if ($InstallPolicy -ne 'Skip') {
            foreach ($item in $selected) {
                if ($item.Platform -eq 'android' -and $Configuration -ne 'release') { Install-TorcaClient -RepoRoot $root -Device $item.Id -Configuration $Configuration }
                elseif ($item.Platform -eq 'windows') { Write-Host 'Windows artifact built; use run command to launch it.' }
            }
        }
        if ($RunPolicy -ne 'Skip') {
            $runDevice = ($selected | Select-Object -First 1).Id
            if ($Configuration -eq 'release') {
                Invoke-TorcaClientRun -RepoRoot $root -Target (Resolve-TorcaTarget $Target) -Device $runDevice -Configuration $Configuration -Installed -ExpectedBuildId $expectedBuildId
            } else {
                Invoke-TorcaClientRun -RepoRoot $root -Target (Resolve-TorcaTarget $Target) -Device $runDevice -Configuration $Configuration -ExpectedBuildId $expectedBuildId
            }
            Write-TorcaStage -Name 'Application launch' -State 'ready' -Detail 'Launch command issued'
            Write-TorcaStage -Name 'Launch health' -State 'ready' -Detail "Process and native startup handoff verified; incident logs: $(Join-Path $root 'logs/collected')"
        }
        if ($InstallPolicy -ne 'Skip' -or $RunPolicy -ne 'Skip') {
            foreach ($item in $selected) { Write-TorcaDeviceDeploymentManifest -Device $item -Release $release -Endpoint $stack.Endpoint -BuildId $expectedBuildId }
            Write-TorcaStage -Name 'Device manifest' -State 'ready' -Detail 'Installed storage epoch recorded per device'
        }
    }
}
