[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$RepoRoot
)

$ErrorActionPreference = 'Stop'

$foundationSecret = Join-Path $RepoRoot 'crates/foundation/torca-foundation/src/secret.rs'
$secretText = Get-Content -LiteralPath $foundationSecret -Raw
if ($secretText -notmatch 'impl<const N: usize>\s+Drop\s+for\s+SecretBytes<N>' -or
    -not $secretText.Contains('.fill(0)')) {
    throw 'SecretBytes<N> must remain the canonical wipe-on-drop container.'
}

$wrapperRequirements = @(
    @{
        Path = 'crates/infrastructure/torca-storage-sqlite/src/sqlcipher.rs'
        Fragment = 'DatabaseKey(SecretBytes<32>)'
    },
    @{
        Path = 'crates/infrastructure/torca-crypto/src/types.rs'
        Fragment = 'SigningSecretKey(SecretBytes<32>)'
    },
    @{
        Path = 'crates/infrastructure/torca-crypto/src/types.rs'
        Fragment = 'SealingKey(SecretBytes<32>)'
    },
    @{
        Path = 'crates/application/torca-pairing-coordinator/src/core/model_ports.rs'
        Fragment = 'PairingDerivedSecret(torca_foundation::SecretBytes<32>)'
    }
)
foreach ($requirement in $wrapperRequirements) {
    $text = Get-Content -LiteralPath (Join-Path $RepoRoot $requirement.Path) -Raw
    if (-not $text.Contains($requirement.Fragment)) {
        throw "Sensitive wrapper drifted from SecretBytes: $($requirement.Path)"
    }
}

# Restart transport snapshots expose a public fixed-size field for immediate
# protected-storage serialization, so they keep an explicit local Drop.
$pairingModel = Join-Path $RepoRoot 'crates/application/torca-pairing-coordinator/src/core/model_ports.rs'
$pairingText = Get-Content -LiteralPath $pairingModel -Raw
if ($pairingText -notmatch 'impl\s+Drop\s+for\s+PairingTransportSnapshot' -or
    -not $pairingText.Contains('self.private_key.fill(0)')) {
    throw 'PairingTransportSnapshot must wipe its exported private key.'
}

$sqlcipher = Get-Content -LiteralPath (Join-Path $RepoRoot 'crates/infrastructure/torca-storage-sqlite/src/sqlcipher.rs') -Raw
if ($sqlcipher -notmatch 'impl\s+Drop\s+for\s+SensitiveSql' -or -not $sqlcipher.Contains('self.0.fill(0)')) {
    throw 'SQLCipher raw-key PRAGMA bytes must be wiped after setup.'
}

$peerSecrets = Get-Content -LiteralPath (Join-Path $RepoRoot 'crates/infrastructure/torca-crypto/src/peer_secrets.rs') -Raw
if (-not $peerSecrets.Contains('stored.fill(0)')) {
    throw 'Protected peer-secret loads must wipe the temporary Vec after key construction.'
}

$pairingRuntime = Get-Content -LiteralPath (Join-Path $RepoRoot 'crates/application/torca-pairing-coordinator/src/runtime/lifecycle_methods.rs') -Raw
if (-not $pairingRuntime.Contains('state.fill(0)')) {
    throw 'Pairing restart-state bytes must be wiped after decoding.'
}

Write-Host 'Torca secret lifetime policy passed.'
