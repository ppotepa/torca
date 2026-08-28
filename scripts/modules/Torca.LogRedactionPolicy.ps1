[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$RepoRoot
)

$ErrorActionPreference = 'Stop'
function Get-TorcaRelativePath { param([string]$BasePath,[string]$TargetPath); $base=(Resolve-Path -LiteralPath $BasePath).Path.TrimEnd('\')+'\'; $target=(Resolve-Path -LiteralPath $TargetPath).Path; if ($target.StartsWith($base,[StringComparison]::OrdinalIgnoreCase)) { return $target.Substring($base.Length).Replace('\','/') }; [Uri]::UnescapeDataString(([Uri]::new($base)).MakeRelativeUri([Uri]::new($target)).ToString()) }

$roots = @(
    'crates/platform/torca-native/src',
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
        # Native query payloads are ABI responses, not diagnostic output.  Do
        # not classify their private fields as logs merely because the same
        # implementation file also owns lifecycle logging.
        $logText = [regex]::Replace($text, '(?s)self\.query_json\s*=.*?\.to_string\(\);', '')
        foreach ($field in $forbiddenStructuredFields) {
            if ($logText.Contains($field)) {
                $relative = Get-TorcaRelativePath $RepoRoot $file.FullName
                throw "Sensitive value must not be written to logs: $relative ($field)"
            }
        }
        if ($logText -match '(?i)(?:ticket|secret|private[_ ]?key|database[_ ]?key)\s*=\s*\{') {
            $relative = Get-TorcaRelativePath $RepoRoot $file.FullName
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
        $relative = Get-TorcaRelativePath $RepoRoot $file.FullName
        throw "Native error classification must remain descriptor based: $relative"
    }
}

Write-Host 'Torca logging redaction policy passed.'
