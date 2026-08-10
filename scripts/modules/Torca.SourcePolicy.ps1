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
$obsoleteAbi = @(
    'torca_engine_create_identity(',
    'torca_engine_create_pairing(',
    'torca_engine_join_pairing(',
    'torca_engine_queue_message(',
    'torca_engine_retry_message(',
    'torca_engine_queue_attachment('
)
foreach ($symbol in $obsoleteAbi) {
    if ($header.Contains($symbol)) {
        throw "Obsolete frontend-owned native mutation ABI returned: $symbol"
    }
}

$canonicalSchemaPath = Join-Path $RepoRoot 'crates/platform/torca-contract/schema/torca_contract.json'
if (-not (Test-Path -LiteralPath $canonicalSchemaPath)) {
    throw 'Canonical language-neutral contract schema is missing.'
}
$canonicalSchema = Get-Content $canonicalSchemaPath -Raw
if (-not $canonicalSchema.Contains('"schema": 1') -or -not $canonicalSchema.Contains('"profile.set"')) {
    throw 'Canonical contract schema is invalid.'
}
$schemaPath = Join-Path $RepoRoot 'crates/platform/torca-contract/schema/torca_contract.dart'
$generatedPath = Join-Path $RepoRoot 'apps/client/flutter/lib/generated/torca_contract.dart'
$schema = Get-Content $schemaPath -Raw
$generated = Get-Content $generatedPath -Raw
if ($schema -ne $generated) {
    throw 'Flutter contract projection drifted from the canonical contract schema.'
}
$obsoleteCommandFragments = @(
    'String? identityIdHex',
    'String? sessionIdHex',
    'String? messageIdHex',
    'String? attachmentIdHex',
    'int? atMs'
)
foreach ($fragment in $obsoleteCommandFragments) {
    if ($schema.Contains($fragment)) {
        throw "Presentation-ownership debt returned: $fragment"
    }
}

$sourceRoots = @(
    (Join-Path $RepoRoot 'crates'),
    (Join-Path $RepoRoot 'apps/client/flutter/lib'),
    (Join-Path $RepoRoot 'scripts'),
    (Join-Path $RepoRoot 'tools')
)
$forbiddenFragments = @(
    'tor.exe', 'vendor/tor', 'vendor\\tor', 'torca-runtime-host', 'torca_runtime_host',
    'torca-bridge', 'torca_bridge', 'torca-read-state', 'torca_read_state',
    'torca-tor-driver', 'torca-transport-tor', 'PENDING_PROFILE_NAME',
    'CreateIdentityCommandDto', 'TORCA_USE_MEMORY_GATEWAY', 'Isolate.run',
    'Stop-TorcaOwnedWindowsTor'
)
foreach ($root in $sourceRoots) {
    if (-not (Test-Path -LiteralPath $root)) { continue }
    $files = Get-ChildItem -LiteralPath $root -Recurse -File -ErrorAction SilentlyContinue |
        Where-Object { $_.FullName -notmatch '\\target\\' -and $_.Name -ne 'Torca.SourcePolicy.ps1' }
    foreach ($file in $files) {
        $text = Get-Content -LiteralPath $file.FullName -Raw
        foreach ($fragment in $forbiddenFragments) {
            if ($text.Contains($fragment)) {
                throw "Forbidden obsolete source fragment '$fragment' in $($file.FullName)"
            }
        }
    }
}

$nativeJson = Join-Path $RepoRoot 'crates/platform/torca-native/src/json.rs'
if (Test-Path -LiteralPath $nativeJson) {
    $nativeJsonText = Get-Content -LiteralPath $nativeJson -Raw
    foreach ($fragment in @('push_json_string', 'push_bridge_message')) {
        if ($nativeJsonText.Contains($fragment)) {
            throw "Native bridge must use contract serialization, not manual JSON: $fragment"
        }
    }
}

$rustPlatformBoundary = [IO.Path]::GetFullPath((Join-Path $RepoRoot 'crates/platform'))
$platformConditionalFragments = @(
    '#[cfg(windows)]', '#[cfg(not(windows))]',
    '#[cfg(target_os = "android")]', '#[cfg(not(target_os = "android"))]'
)
$rustFilesOutsidePlatform = Get-ChildItem -LiteralPath (Join-Path $RepoRoot 'crates') -Recurse -Filter '*.rs' -File |
    Where-Object { $_.FullName -notmatch '[\\/]target[\\/]' -and -not $_.FullName.StartsWith($rustPlatformBoundary, [StringComparison]::OrdinalIgnoreCase) }
foreach ($file in $rustFilesOutsidePlatform) {
    $text = Get-Content -LiteralPath $file.FullName -Raw
    foreach ($fragment in $platformConditionalFragments) {
        if ($text.Contains($fragment)) {
            throw "Platform conditional escaped crates/platform: $($file.FullName) ($fragment)"
        }
    }
}

$flutterLib = Join-Path $RepoRoot 'apps/client/flutter/lib'
if (Test-Path -LiteralPath $flutterLib) {
    $uiFiles = Get-ChildItem -LiteralPath $flutterLib -Recurse -Filter '*.dart' -File |
        Where-Object { $_.FullName -notmatch '[\\/]platform[\\/]' }
    foreach ($file in $uiFiles) {
        $text = Get-Content -LiteralPath $file.FullName -Raw
        if ($text.Contains('Platform.is')) {
            throw "Platform detection escaped lib/platform: $($file.FullName)"
        }
        if ($text.Contains('DynamicLibrary') -and $file.Name -ne 'ffi_engine_gateway.dart') {
            throw "DynamicLibrary escaped the native runtime worker: $($file.FullName)"
        }
    }
}

$artiOwners = Get-ChildItem -LiteralPath (Join-Path $RepoRoot 'crates') -Recurse -File -Include '*.rs','*.toml' |
    Where-Object { $_.FullName -notmatch '\\torca-tor\\' }
foreach ($file in $artiOwners) {
    $text = Get-Content -LiteralPath $file.FullName -Raw
    if ($text.Contains('arti-client') -or $text.Contains('arti_client')) {
        throw "Arti may only be imported by torca-tor: $($file.FullName)"
    }
}

Write-Host 'Torca source policy passed.'
