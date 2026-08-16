[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$RepoRoot
)

$ErrorActionPreference = 'Stop'

$roots = @(
    'crates/platform/torca-native/src',
    'crates/infrastructure/torca-tor/src',
    'crates/infrastructure/torca-pairing-driver/src',
    'crates/application/torca-pairing-coordinator/src'
)

$forbiddenStructuredFields = @(
    '"ticket":',
    '"inviteTicket":',
    '"secret":',
    '"peerSecret":',
    '"privateKey":',
    '"databaseKey":',
    '"sourcePath":',
    '"previewSourcePath":',
    '"onionAddress":',
    '"ciphertext":',
    '"plaintext":'
)

foreach ($relativeRoot in $roots) {
    $root = Join-Path $RepoRoot $relativeRoot
    if (-not (Test-Path -LiteralPath $root)) { continue }
    foreach ($file in Get-ChildItem -LiteralPath $root -Recurse -File -Filter '*.rs') {
        $text = Get-Content -LiteralPath $file.FullName -Raw
        if ($text -notmatch '(?:logger\.|\.log\(|event_with_context|eprintln!|println!)') { continue }
        foreach ($field in $forbiddenStructuredFields) {
            if ($text.Contains($field)) {
                $relative = [IO.Path]::GetRelativePath($RepoRoot, $file.FullName).Replace('\\', '/')
                throw "Sensitive value must not be written to logs: $relative ($field)"
            }
        }
        if ($text -match '(?i)(?:ticket|secret|private[_ ]?key|database[_ ]?key)\s*=\s*\{') {
            $relative = [IO.Path]::GetRelativePath($RepoRoot, $file.FullName).Replace('\\', '/')
            throw "Sensitive value interpolation must not be written to logs: $relative"
        }
    }
}

# ABI/native error routing is descriptor based. Matching human-readable text
# in logging/error code would turn wording into protocol state.
$nativeRoot = Join-Path $RepoRoot 'crates/platform/torca-native/src'
foreach ($file in Get-ChildItem -LiteralPath $nativeRoot -Recurse -File -Filter '*.rs') {
    $text = Get-Content -LiteralPath $file.FullName -Raw
    if ($text -match '(?i)(?:error|message|display)\.to_string\(\)\.contains\s*\(') {
        $relative = [IO.Path]::GetRelativePath($RepoRoot, $file.FullName).Replace('\\', '/')
        throw "Native error classification must remain descriptor based: $relative"
    }
}

Write-Host 'Torca logging redaction policy passed.'
