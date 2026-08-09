Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Get-TorcaPaths {
    param([Parameter(Mandatory = $true)][string]$RepoRoot)
    $runtime = Join-Path $RepoRoot '.torca'
    [pscustomobject]@{
        RepoRoot = $RepoRoot; RuntimeRoot = $runtime
        StateFile = Join-Path $runtime 'state.json'
        BuildManifestRoot = Join-Path $runtime 'manifests'
        ManifestFile = Join-Path $runtime 'build-manifest.json'
        StackRoot = Join-Path $runtime 'stack'
        RelayLog = Join-Path $runtime 'logs/relay.log'
        RelayErrorLog = Join-Path $runtime 'logs/relay.error.log'
        RelayEndpoint = Join-Path $runtime 'stack/relay_endpoint.txt'
        ArtifactsRoot = Join-Path $RepoRoot 'artifacts'
        RelayExecutable = Join-Path $RepoRoot 'target/release/torca-relay.exe'
        DockerCompose = Join-Path $RepoRoot 'infra/docker/compose.yml'
        StackProvider = if ($env:TORCA_STACK_PROVIDER) { $env:TORCA_STACK_PROVIDER } else { 'auto' }
    }
}

function Initialize-TorcaPaths {
    param([Parameter(Mandatory = $true)]$Paths)
    foreach ($directory in @($Paths.RuntimeRoot, $Paths.BuildManifestRoot, $Paths.StackRoot, (Split-Path $Paths.RelayLog))) {
        New-Item -ItemType Directory -Path $directory -Force | Out-Null
    }
}

function Get-TorcaBuildManifestPath {
    param([Parameter(Mandatory = $true)]$Paths, [Parameter(Mandatory = $true)][string]$Target, [Parameter(Mandatory = $true)][string]$Configuration)
    Join-Path $Paths.BuildManifestRoot "$($Target.ToLowerInvariant())-$($Configuration.ToLowerInvariant()).json"
}

function Get-TorcaDefaultOptions {
    [pscustomobject]@{ OnionPolicy = 'Ensure'; ClientDataPolicy = 'Preserve'; BuildPolicy = 'IfRequired'; InstallPolicy = 'Selected'; RunPolicy = 'Restart'; Readiness = 'Strict' }
}

Export-ModuleMember -Function Get-TorcaPaths, Initialize-TorcaPaths, Get-TorcaBuildManifestPath, Get-TorcaDefaultOptions
