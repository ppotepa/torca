[CmdletBinding()]
param(
    [ValidateSet('auto', 'check', 'windows', 'android', 'all')]
    [string]$Target = 'auto',
    [ValidateSet('debug', 'release')]
    [string]$Configuration = 'debug',
    [ValidateSet('Full', 'Quick', 'Skip')]
    [string]$Validation = 'Full',
    [ValidateSet('tor', 'iroh', 'webrtc')]
    [string]$CommunicationProvider,
    [ValidateSet('always', 'direct', 'local')]
    [string]$ProviderProfile,
    [switch]$CI
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot

if ($PSBoundParameters.ContainsKey('CommunicationProvider')) {
    $env:TORCA_COMMUNICATION_PROVIDER = $CommunicationProvider
}
if ($PSBoundParameters.ContainsKey('ProviderProfile')) {
    if ($env:TORCA_COMMUNICATION_PROVIDER -ne 'iroh') {
        throw 'ProviderProfile is only valid when CommunicationProvider is iroh.'
    }
    $env:TORCA_IROH_PROFILE = $ProviderProfile
} elseif ($env:TORCA_COMMUNICATION_PROVIDER -eq 'iroh' -and
    [string]::IsNullOrWhiteSpace([string]$env:TORCA_IROH_PROFILE)) {
    # Direct invocations must still embed a deterministic profile. The Rust
    # deployer supplies this explicitly; the script keeps the same default for
    # developers who call build.ps1 without the wizard.
    $env:TORCA_IROH_PROFILE = 'always'
}

& (Join-Path $root 'scripts/modules/Torca.SourcePolicy.ps1') -RepoRoot $root

$assetsModule = Join-Path $root 'scripts/modules/Torca.PlatformAssets.psm1'
Import-Module $assetsModule -Force -ErrorAction Stop -Verbose:$false

switch ($Target) {
    'windows' { Prepare-TorcaPlatformAssets -RepoRoot $root -Platform windows }
    'android' { Prepare-TorcaPlatformAssets -RepoRoot $root -Platform android }
    'all' {
        Prepare-TorcaPlatformAssets -RepoRoot $root -Platform windows
        Prepare-TorcaPlatformAssets -RepoRoot $root -Platform android
    }
    'auto' {
        if ($env:OS -eq 'Windows_NT') {
            Prepare-TorcaPlatformAssets -RepoRoot $root -Platform windows
        }
    }
}

$module = Join-Path $root 'scripts/modules/Torca.BuildEngine.psm1'
# Direct builds must carry the same deterministic identity as orchestrated deploys.
# The native metadata is compiled from these values and Flutter validates the build id before
# executing any request.
$buildIdentityModule = Join-Path $root 'scripts/modules/Torca.Build.psm1'
Import-Module $buildIdentityModule -Force -ErrorAction Stop -Verbose:$false
foreach ($requiredCommand in @('Get-TorcaBuildId', 'Get-TorcaBuildSourceFingerprint', 'Write-TorcaBuildManifest')) {
    if (-not (Get-Command $requiredCommand -ErrorAction SilentlyContinue)) {
        throw "Torca build module did not expose required command: $requiredCommand"
    }
}
if ($Target -ne 'check' -and
    ($env:TORCA_ORCHESTRATED -ne '1' -or [string]::IsNullOrWhiteSpace($env:TORCA_BUILD_ID))) {
    $release = Get-Content (Join-Path $root 'release/version.json') -Raw | ConvertFrom-Json
    $provider = [string]$env:TORCA_COMMUNICATION_PROVIDER
    if ([string]::IsNullOrWhiteSpace($provider)) { $provider = 'tor' }
    $endpoint = [string]$env:TORCA_PROVIDER_ENDPOINT
    if ([string]::IsNullOrWhiteSpace($endpoint) -and $provider -eq 'tor') {
        # Temporary compatibility for direct Tor builds. New provider-aware
        # callers pass TORCA_PROVIDER_ENDPOINT instead.
        $endpoint = [string]$env:TORCA_RELAY_ENDPOINT
    }
    if ([string]::IsNullOrWhiteSpace($endpoint) -and $provider -eq 'tor') {
        $endpointFile = Join-Path $root '.torca/stack/relay_endpoint.txt'
        if (Test-Path -LiteralPath $endpointFile) { $endpoint = (Get-Content $endpointFile -Raw).Trim() }
    }
    if ([string]::IsNullOrWhiteSpace($endpoint) -and $provider -eq 'tor') {
        throw 'The selected Tor provider requires TORCA_PROVIDER_ENDPOINT. Start the managed rendezvous stack or set the provider endpoint.'
    }
    # The selected provider owns endpoint interpretation. The shared build
    # path embeds only a generic provider endpoint and configuration hash.
    $env:TORCA_COMMUNICATION_PROVIDER = $provider
    $env:TORCA_PROVIDER_ENDPOINT = $endpoint
    $env:TORCA_BUILD_ID = Get-TorcaBuildId -RepoRoot $root -Endpoint $endpoint -Target $Target -Configuration $Configuration
    $env:TORCA_PRODUCT_VERSION = [string]$release.version
    $env:TORCA_SOURCE_FINGERPRINT = Get-TorcaBuildSourceFingerprint -RepoRoot $root
    $env:TORCA_SOURCE_COMMIT = ((git -C $root rev-parse HEAD 2>$null | Out-String).Trim())
    if ([string]::IsNullOrWhiteSpace($env:TORCA_SOURCE_COMMIT)) { $env:TORCA_SOURCE_COMMIT = 'working-tree' }
    if ($provider -eq 'tor') {
        $endpointSha = [System.Security.Cryptography.SHA256]::Create()
        try {
            $env:TORCA_PROVIDER_ENDPOINT_HASH = [BitConverter]::ToString(
                $endpointSha.ComputeHash([Text.Encoding]::UTF8.GetBytes($endpoint))
            ).Replace('-', '').ToLowerInvariant()
        } finally { $endpointSha.Dispose() }
    } else {
        # Direct providers do not have a managed rendezvous endpoint. Clear a
        # value inherited from a previous Tor build so stale metadata cannot
        # make the runtime look partially configured.
        Remove-Item Env:TORCA_PROVIDER_ENDPOINT_HASH -ErrorAction SilentlyContinue
    }
}
Import-Module $module -Force -ErrorAction Stop -Verbose:$false
if (-not (Get-Command Invoke-TorcaBuild -ErrorAction SilentlyContinue)) {
    throw "Torca build engine did not expose required command: Invoke-TorcaBuild"
}
Invoke-TorcaBuild -Target $Target -Configuration $Configuration -Validation $Validation -CI:$CI

# `build.ps1` is also intentionally usable outside the deployment wizard.
# Record the exact identity embedded in that successful binary; otherwise a
# later `run`/`redeploy` can consult an older manifest and compare the device
# to a relay address it no longer contains.
if ($Target -ne 'check') {
    # Do not re-import Config/State with -Force here: when build.ps1 is called
    # from the wizard it would unload those exports from the parent
    # orchestrator. Write-TorcaBuildManifest only needs these two paths and
    # owns its own atomic directory creation.
    $paths = [pscustomobject]@{
        RepoRoot = $root
        BuildManifestRoot = Join-Path $root '.torca/manifests'
    }
    $resolvedTarget = if ($Target -eq 'auto') {
        if ($env:OS -eq 'Windows_NT') { 'windows' } else { 'android' }
    } else { $Target }
    Write-TorcaBuildManifest -Paths $paths -Endpoint $env:TORCA_PROVIDER_ENDPOINT -Targets @($resolvedTarget) -Configuration $Configuration -BuildId $env:TORCA_BUILD_ID -SourceFingerprint $env:TORCA_SOURCE_FINGERPRINT
    Write-Host "Torca build manifest recorded: target=$resolvedTarget configuration=$Configuration" -ForegroundColor Green
}
