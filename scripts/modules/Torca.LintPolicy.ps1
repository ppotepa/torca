[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$RepoRoot
)

$ErrorActionPreference = 'Stop'

$roots = @(
    'crates/application/torca-runtime/src',
    'crates/application/torca-client-engine/src',
    'crates/application/torca-pairing-coordinator/src',
    'crates/infrastructure/torca-peer-link/src',
    'crates/platform/torca-native/src'
)

$forbidden = @(
    'allow(clippy::too_many_lines)',
    'allow(clippy::too_many_arguments)',
    '#[allow(dead_code)]'
)

foreach ($relativeRoot in $roots) {
    $root = Join-Path $RepoRoot $relativeRoot
    if (-not (Test-Path -LiteralPath $root)) { continue }
    foreach ($file in Get-ChildItem -LiteralPath $root -Recurse -File -Filter '*.rs') {
        $text = Get-Content -LiteralPath $file.FullName -Raw
        foreach ($fragment in $forbidden) {
            if ($text.Contains($fragment)) {
                $relative = [IO.Path]::GetRelativePath($RepoRoot, $file.FullName).Replace('\\', '/')
                throw "Cleanup hotspot still suppresses '$fragment': $relative"
            }
        }
    }
}

# Broad lint suppression is never acceptable. Narrow cfg_attr dead-code
# allowances for platform-only ABI members are checked separately.
foreach ($file in Get-ChildItem -LiteralPath (Join-Path $RepoRoot 'crates') -Recurse -File -Filter '*.rs') {
    $text = Get-Content -LiteralPath $file.FullName -Raw
    foreach ($fragment in @('#![allow(clippy::all)]', 'allow(clippy::pedantic)', 'allow(clippy::nursery)')) {
        if ($text.Contains($fragment)) {
            $relative = [IO.Path]::GetRelativePath($RepoRoot, $file.FullName).Replace('\\', '/')
            throw "Broad lint suppression is forbidden: $relative ($fragment)"
        }
    }
}

Write-Host 'Torca lint suppression policy passed.'
