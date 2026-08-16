[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$RepoRoot
)

$ErrorActionPreference = 'Stop'
Push-Location $RepoRoot
try {
    $metadata = cargo metadata --format-version 1 --locked | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0) {
        throw 'cargo metadata --locked failed.'
    }

    $workspaceIds = [Collections.Generic.HashSet[string]]::new()
    foreach ($id in $metadata.workspace_members) { [void]$workspaceIds.Add([string]$id) }

    $workspace = @($metadata.packages | Where-Object { $workspaceIds.Contains([string]$_.id) })
    Write-Host "Workspace packages: $($workspace.Count)"

    foreach ($package in $workspace | Sort-Object name) {
        $features = @($package.features.PSObject.Properties.Name | Sort-Object)
        if ($features.Count -gt 0) {
            Write-Host ("{0}: features=[{1}]" -f $package.name, ($features -join ', '))
        }
    }

    # Multiple registry versions are not automatically wrong, but they are
    # dependency weight worth reviewing. `cargo tree -d` is the authoritative
    # report and intentionally does not mutate Cargo.toml/Cargo.lock.
    cargo tree --locked -d
    if ($LASTEXITCODE -ne 0) {
        throw 'cargo tree --locked -d failed.'
    }

    # The maintainability cleanup must remain dependency-neutral for the new
    # ownership helpers. They live in torca-foundation rather than pulling a
    # generic actor/zeroization framework into the graph.
    $forbiddenCleanupDeps = @('zeroize', 'async-trait')
    foreach ($package in $workspace) {
        foreach ($dependency in $package.dependencies) {
            if ($forbiddenCleanupDeps -contains [string]$dependency.name) {
                throw "Cleanup-only dependency is not allowed without a protocol/runtime need: $($dependency.name) in $($package.name)"
            }
        }
    }
} finally {
    Pop-Location
}

Write-Host 'Torca dependency/feature audit passed.'
