[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$RepoRoot
)

$ErrorActionPreference = 'Stop'

function Get-TorcaLayer([string]$ManifestPath) {
    $root = ((Resolve-Path -LiteralPath $RepoRoot).Path).TrimEnd('\') + '\'
    $full = ((Resolve-Path -LiteralPath $ManifestPath).Path)
    $relative = $full.Substring($root.Length).Replace('\', '/')
    if ($relative.StartsWith('crates/domains/')) { return 'domains' }
    if ($relative.StartsWith('crates/protocol/')) { return 'protocol' }
    if ($relative.StartsWith('crates/application/')) { return 'application' }
    if ($relative.StartsWith('crates/infrastructure/')) { return 'infrastructure' }
    if ($relative.StartsWith('crates/platform/')) { return 'platform' }
    return 'other'
}

# These are the known violations being removed in the named 0.3 milestones. No new exception is
# permitted without a matching tracker item and a removal condition.
$temporaryAllowedEdges = @{}

$metadata = & cargo metadata --format-version 1 --locked --no-deps | ConvertFrom-Json
$packagesByManifest = @{}
foreach ($package in $metadata.packages) {
    $packagesByManifest[(Resolve-Path -LiteralPath $package.manifest_path).Path] = $package
}

foreach ($package in $metadata.packages) {
    $fromLayer = Get-TorcaLayer $package.manifest_path
    if ($fromLayer -notin @('domains', 'protocol', 'application')) { continue }
    $packageDirectory = Split-Path -Parent $package.manifest_path
    foreach ($dependency in $package.dependencies) {
        # cargo metadata omits `path` for registry dependencies. Accessing a
        # missing PSCustomObject property is an error under StrictMode in the
        # release scripts, so inspect the property before reading it.
        $dependencyPathProperty = $dependency.PSObject.Properties['path']
        if ($null -eq $dependencyPathProperty -or [string]::IsNullOrWhiteSpace($dependencyPathProperty.Value)) {
            continue
        }
        $dependencyPath = [string]$dependencyPathProperty.Value
        $dependencyDirectory = if ([IO.Path]::IsPathRooted($dependencyPath)) {
            $dependencyPath
        } else {
            Join-Path $packageDirectory $dependencyPath
        }
        $manifest = (Resolve-Path -LiteralPath (Join-Path $dependencyDirectory 'Cargo.toml')).Path
        if (-not $packagesByManifest.ContainsKey($manifest)) { continue }
        $target = $packagesByManifest[$manifest]
        $toLayer = Get-TorcaLayer $target.manifest_path
        $edge = "$($package.name)->$($target.name)"
        $forbidden = ($fromLayer -in @('domains', 'protocol') -and $toLayer -in @('application', 'infrastructure', 'platform')) -or
            ($fromLayer -eq 'application' -and $toLayer -in @('infrastructure', 'platform'))
        if ($forbidden -and -not $temporaryAllowedEdges.ContainsKey($edge)) {
            throw "Forbidden architectural dependency $edge ($fromLayer -> $toLayer)."
        }
    }
}

$contract = $metadata.packages | Where-Object { $_.name -eq 'torca-contract' }
if ($null -ne $contract) {
    $contractDependencies = @($contract.dependencies | ForEach-Object { $_.name })
    foreach ($forbidden in @('torca-runtime', 'torca-client-engine')) {
        if ($contractDependencies -contains $forbidden) {
            throw "torca-contract must depend on the application facade, not $forbidden."
        }
    }
}

$contractSource = Get-Content -Raw -LiteralPath (Join-Path $RepoRoot 'crates/platform/torca-contract/src/lib.rs')
foreach ($forbiddenPattern in @('struct\s+ContractRuntime', 'fingerprint_for\s*\(', 'safety_number\s*\(')) {
    if ($contractSource -match $forbiddenPattern) {
        throw "torca-contract contains forbidden application/security policy: $forbiddenPattern"
    }
}

$storageSources = Get-ChildItem -LiteralPath (Join-Path $RepoRoot 'crates/infrastructure/torca-storage-sqlite/src') -Filter '*.rs' -File
foreach ($source in $storageSources) {
    $content = Get-Content -Raw -LiteralPath $source.FullName
    if ($content -match 'ApplicationPayloadCodec') {
        throw "SQLite repositories must not encode application payloads: $($source.FullName)"
    }
}

$receiptDerivations = Get-ChildItem -LiteralPath (Join-Path $RepoRoot 'crates') -Filter '*.rs' -File -Recurse |
    Where-Object { (Get-Content -Raw -LiteralPath $_.FullName) -match 'fn\s+derived_receipt_id' }
if (@($receiptDerivations).Count -gt 0) {
    throw 'Receipt identifiers must be derived by torca-receipts, not adapter-local helpers.'
}

Write-Host 'Torca architecture policy passed.'
