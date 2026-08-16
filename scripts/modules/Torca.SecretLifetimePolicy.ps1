[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$RepoRoot
)

$ErrorActionPreference = 'Stop'

$requirements = @(
    @{
        Path = 'crates/infrastructure/torca-storage-sqlite/src/sqlcipher.rs'
        Types = @('DatabaseKey', 'SensitiveSql')
    },
    @{
        Path = 'crates/infrastructure/torca-crypto/src/types.rs'
        Types = @('SigningSecretKey', 'SealingKey')
    },
    @{
        Path = 'crates/application/torca-pairing-coordinator/src/core/model_ports.rs'
        Types = @('PairingTransportSnapshot', 'PairingDerivedSecret')
    }
)

foreach ($requirement in $requirements) {
    $path = Join-Path $RepoRoot $requirement.Path
    if (-not (Test-Path -LiteralPath $path)) {
        throw "Secret-owner source is missing: $($requirement.Path)"
    }
    $text = Get-Content -LiteralPath $path -Raw
    foreach ($type in $requirement.Types) {
        if ($text -notmatch "impl\s+Drop\s+for\s+$type") {
            throw "Sensitive type must zero its memory on Drop: $type ($($requirement.Path))"
        }
    }
    if (-not $text.Contains('.fill(0)')) {
        throw "Secret-owner file must explicitly wipe sensitive byte storage: $($requirement.Path)"
    }
}

$peerSecrets = Join-Path $RepoRoot 'crates/infrastructure/torca-crypto/src/peer_secrets.rs'
$peerText = Get-Content -LiteralPath $peerSecrets -Raw
if (-not $peerText.Contains('stored.fill(0)')) {
    throw 'Protected peer-secret loads must wipe the temporary Vec after key construction.'
}

$pairingRuntime = Join-Path $RepoRoot 'crates/application/torca-pairing-coordinator/src/runtime/lifecycle_methods.rs'
$pairingText = Get-Content -LiteralPath $pairingRuntime -Raw
if (-not $pairingText.Contains('state.fill(0)')) {
    throw 'Pairing restart-state bytes must be wiped after decoding.'
}

$persistence = Join-Path $RepoRoot 'crates/application/torca-pairing-coordinator/src/runtime/completion_methods.rs'
if (Test-Path -LiteralPath $persistence) {
    $text = Get-Content -LiteralPath $persistence -Raw
    if ($text.Contains('encoded') -and -not $text.Contains('fill(0)')) {
        throw 'Encoded protected pairing state must be wiped after storage.'
    }
}

Write-Host 'Torca secret lifetime policy passed.'
