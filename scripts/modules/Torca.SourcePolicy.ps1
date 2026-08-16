[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$RepoRoot
)

$ErrorActionPreference = 'Stop'
& (Join-Path $PSScriptRoot 'Torca.ArchitecturePolicy.ps1') -RepoRoot $RepoRoot
$contractManifest = Join-Path $RepoRoot 'crates/platform/torca-contract/Cargo.toml'
if ((Get-Content -LiteralPath $contractManifest -Raw).Contains('sha2')) {
    throw 'torca-contract must not own security hashing; use the identity domain projection.'
}
$readStatePath = Join-Path $RepoRoot 'crates/infrastructure/torca-storage-sqlite/src/read_state.rs'
if ((Get-Content -LiteralPath $readStatePath -Raw).Contains('ApplicationPayloadCodec')) {
    throw 'SQL read-state storage must execute pending jobs, not encode application payloads.'
}
$peerHealthPath = Join-Path $RepoRoot 'crates/infrastructure/torca-communication-adapters/src/peer_health.rs'
if (Test-Path -LiteralPath $peerHealthPath) {
    $peerHealth = Get-Content -LiteralPath $peerHealthPath -Raw
    foreach ($fragment in @('PROBE_INTERVAL', 'PROBE_RETRY', 'next_probe_at', 'fn probe_due')) {
        if ($peerHealth.Contains($fragment)) {
            throw "P2P probe cadence belongs to torca-connectivity application policy, not peer_health.rs: $fragment"
        }
    }
}
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

foreach ($relative in @('LICENSE', 'PRIVACY.md', 'THIRD_PARTY_NOTICES.md', 'SECURITY.md')) {
    if (-not (Test-Path -LiteralPath (Join-Path $RepoRoot $relative))) {
        throw "Required distribution document is missing: $relative"
    }
}

$rustSources = Get-ChildItem -LiteralPath (Join-Path $RepoRoot 'crates') -Recurse -File -Include '*.rs','Cargo.toml' |
    Where-Object { $_.FullName -notmatch '[\\/]target[\\/]' }
foreach ($file in $rustSources) {
    $text = Get-Content -LiteralPath $file.FullName -Raw
    if ($text.Contains('#![allow(clippy::all)]') -or
        $text.Contains('all = { level = "allow"') -or
        $text.Contains('pedantic = { level = "allow"')) {
        throw "Broad Clippy suppression is forbidden: $($file.FullName)"
    }
}

# Persistence SQL belongs in the SQL source tree. Test fixtures may use direct
# reads to assert persisted state, but production Rust must load named queries.
$productionStorageSources = Get-ChildItem -LiteralPath (Join-Path $RepoRoot 'crates/infrastructure/torca-storage-sqlite/src') -Recurse -File -Filter '*.rs' |
    Where-Object { $_.FullName -notmatch '[\\/]tests?[\\/]' -and $_.Name -notmatch '_test\\.rs$' }
foreach ($file in $productionStorageSources) {
    $text = Get-Content -LiteralPath $file.FullName -Raw
    if ($text -match 'query_(?:row|map|iter)\s*\(\s*"\s*(?:SELECT|INSERT|UPDATE|DELETE)\b' -or
        $text -match 'execute(?:_batch)?\s*\(\s*"\s*(?:SELECT|INSERT|UPDATE|DELETE)\b') {
        throw "Production storage SQL must live in named .sql files: $($file.FullName)"
    }
}

foreach ($relative in @(
    'crates/infrastructure/torca-storage-sqlite/src/pairing_repository.rs',
    'crates/infrastructure/torca-storage-sqlite/src/pending_operations.rs'
)) {
    $path = Join-Path $RepoRoot $relative
    $text = Get-Content -LiteralPath $path -Raw
    if ($text -match '"\s*(?:INSERT|SELECT|UPDATE|DELETE)\s') {
        throw "Business SQL must live in parameterized .sql files: $relative"
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
$generatedPath = Join-Path $RepoRoot 'apps/client/flutter/lib/generated/torca_contract.dart'
$contractTemplatePath = Join-Path $RepoRoot 'crates/platform/torca-contract/schema/torca_contract.dart'
if (-not (Test-Path -LiteralPath $contractTemplatePath) -or -not (Test-Path -LiteralPath $generatedPath)) {
    throw 'Contract Dart template or generated Flutter projection is missing.'
}
$contractTemplate = Get-Content $contractTemplatePath -Raw
$contractMarker = '__TORCA_CONTRACT_VERSION__'
$contractMarkerCount = ([regex]::Matches($contractTemplate, [regex]::Escape($contractMarker))).Count
if ($contractMarkerCount -ne 1) {
    throw 'Contract Dart template must contain exactly one __TORCA_CONTRACT_VERSION__ marker.'
}
Push-Location $RepoRoot
try {
    & cargo run -p torca-contract-gen -- --check apps/client/flutter/lib/generated/torca_contract.dart
    if ($LASTEXITCODE -ne 0) {
        throw 'Flutter contract projection drifted from the canonical contract schema. Run the contract generator and commit its output.'
    }
} finally {
    Pop-Location
}
$obsoleteCommandFragments = @(
    'String? identityIdHex',
    'String? sessionIdHex',
    'String? messageIdHex',
    'String? attachmentIdHex',
    'int? atMs'
)
foreach ($fragment in $obsoleteCommandFragments) {
    if ($contractTemplate.Contains($fragment)) {
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
        Where-Object {
            $_.FullName -notmatch '\\target\\' -and
            $_.Name -ne 'Torca.SourcePolicy.ps1' -and
            # `codecat` diagnostic bundles are ignored generated text, not
            # source. Scanning them makes policy results depend on a local
            # troubleshooting artifact and can block an otherwise valid build.
            $_.Name -ne 'concat.txt'
        }
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
    foreach ($fragment in @('push_json_string', 'push_bridge_message', 'classify_error')) {
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
# A small number of infrastructure crates are deliberately platform-backed
# adapters. `torca-radio-adapters` owns the cpal/Android capture bridge and
# exposes a platform-neutral RadioAudioAdapter to the application layer; its
# target gates are therefore part of the adapter boundary, not application
# policy. Keep this exception narrow and path-based so new platform conditionals
# still fail the architecture check by default.
$platformBackedAdapterRoot = [IO.Path]::GetFullPath((Join-Path $RepoRoot 'crates/infrastructure/torca-radio-adapters'))
$rustFilesOutsidePlatform = Get-ChildItem -LiteralPath (Join-Path $RepoRoot 'crates') -Recurse -Filter '*.rs' -File |
    Where-Object { $_.FullName -notmatch '[\\/]target[\\/]' -and -not $_.FullName.StartsWith($rustPlatformBoundary, [StringComparison]::OrdinalIgnoreCase) }
foreach ($file in $rustFilesOutsidePlatform) {
    if ($file.FullName.StartsWith($platformBackedAdapterRoot, [StringComparison]::OrdinalIgnoreCase)) { continue }
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
        $lineCount = (Get-Content -LiteralPath $file.FullName).Count
        # Generated ABI projections are intentionally large and are governed
        # by the contract generator drift check below, not the hand-written
        # feature-file maintainability threshold.
        $isGeneratedFlutterSource =
            $file.FullName -match '[\\/]generated[\\/]' -or
            $file.FullName -match '[\\/]l10n[\\/]app_localizations(?:_[a-z_]+)?\.dart$'
        if ($lineCount -gt 1200 -and -not $isGeneratedFlutterSource) {
            throw "Flutter source file exceeds the 1200-line maintainability gate: $($file.FullName) ($lineCount lines)"
        }
        if ($text.Contains('Platform.is')) {
            throw "Platform detection escaped lib/platform: $($file.FullName)"
        }
        if ($text.Contains('DynamicLibrary') -and $file.Name -ne 'ffi_engine_gateway.dart') {
            throw "DynamicLibrary escaped the native runtime worker: $($file.FullName)"
        }
        if ($text -cmatch '(?<!torca)Icons\.') {
            throw "Raw Material icon escaped the Torca semantic icon set: $($file.FullName)"
        }
        if ($text -match 'BorderRadius\.circular\(\s*\d') {
            throw "Hard-coded component radius escaped Torca geometry tokens: $($file.FullName)"
        }
        if ($text -match '\b(?:Linear|Radial|Sweep)Gradient\s*\(') {
            throw "Gradient escaped the flat Torca presentation policy: $($file.FullName)"
        }
        if ($text -match '\.(?:state|status|direction|quality|role|dependency|bootstrapPhase)\s*(?:==|!=)\s*[''"]') {
            throw "Raw wire-state comparison escaped generated typed accessors: $($file.FullName)"
        }
        if ($text -match 'Colors\.(?:green|orange|red|blue)') {
            throw "Status color escaped Torca semantic colors: $($file.FullName)"
        }
    }

    foreach ($relative in @(
        'widgets/peer_health_indicator.dart',
        'widgets/runtime_network_status.dart'
    )) {
        $path = Join-Path $flutterLib $relative
        $text = Get-Content -LiteralPath $path -Raw
        if ($text.Contains('AnimationController') -and
            (-not $text.Contains('MediaQuery.disableAnimationsOf') -or
             -not $text.Contains('torcaTokens.animationDuration'))) {
            throw "Continuous status animation must honor OS and Torca reduce-motion policy: $relative"
        }
    }
}

$localizationRoot = Join-Path $flutterLib 'l10n'
$localizationConfig = Join-Path (Split-Path $flutterLib -Parent) 'l10n.yaml'
if (-not (Test-Path -LiteralPath $localizationConfig -PathType Leaf)) {
    throw "Flutter localization config is missing: $localizationConfig"
}
$arbFiles = @{
    en = (Join-Path $localizationRoot 'app_en.arb')
    pl = (Join-Path $localizationRoot 'app_pl.arb')
}
$arbKeys = @{}
foreach ($locale in $arbFiles.Keys) {
    $arbPath = $arbFiles[$locale]
    if (-not (Test-Path -LiteralPath $arbPath -PathType Leaf)) {
        throw "Flutter ARB catalog is missing: $arbPath"
    }
    try {
        $catalog = Get-Content -LiteralPath $arbPath -Raw | ConvertFrom-Json
        $arbKeys[$locale] = @($catalog.PSObject.Properties.Name | Where-Object { $_ -notlike '@@*' } | Sort-Object)
    } catch {
        throw "Flutter ARB catalog is invalid: $arbPath ($($_.Exception.Message))"
    }
}
$arbDifferences = @(Compare-Object $arbKeys.en $arbKeys.pl)
if ($arbDifferences.Count -gt 0) {
    throw 'Flutter ARB catalogs en/pl have different message keys.'
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
