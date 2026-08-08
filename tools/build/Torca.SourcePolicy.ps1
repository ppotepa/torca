[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$RepoRoot
)

$ErrorActionPreference = 'Stop'
$forbiddenFiles = @(
    'crates/application/torca-pairing-coordinator/src/final_runtime.rs',
    'crates/infrastructure/torca-storage-sqlite/src/migration_v2.rs',
    'crates/infrastructure/torca-storage-sqlite/src/migration_v3.rs',
    'crates/platform/torca-native/src/retry_ffi.rs'
)
foreach ($relative in $forbiddenFiles) {
    if (Test-Path (Join-Path $RepoRoot $relative)) {
        throw "Obsolete source root returned: $relative"
    }
}

$header = Get-Content (Join-Path $RepoRoot 'crates/platform/torca-native/include/torca_native.h') -Raw
$legacyAbi = @(
    'torca_engine_create_identity(',
    'torca_engine_create_pairing(',
    'torca_engine_join_pairing(',
    'torca_engine_queue_message(',
    'torca_engine_retry_message(',
    'torca_engine_queue_attachment('
)
foreach ($symbol in $legacyAbi) {
    if ($header.Contains($symbol)) {
        throw "Legacy frontend-owned native mutation ABI returned: $symbol"
    }
}

$schemaPath = Join-Path $RepoRoot 'crates/platform/torca-bridge/schema/torca_contract.dart'
$generatedPath = Join-Path $RepoRoot 'apps/client/flutter/lib/generated/torca_contract.dart'
$schema = Get-Content $schemaPath -Raw
$generated = Get-Content $generatedPath -Raw
if ($schema -ne $generated) {
    throw 'Flutter bridge projection drifted from the canonical bridge schema.'
}
$legacyCommandFragments = @(
    'String? identityIdHex',
    'String? sessionIdHex',
    'String? messageIdHex',
    'String? attachmentIdHex',
    'int? atMs'
)
foreach ($fragment in $legacyCommandFragments) {
    if ($schema.Contains($fragment)) {
        throw "Bridge v11 presentation-ownership debt returned: $fragment"
    }
}

Write-Host 'Torca source policy passed.'
