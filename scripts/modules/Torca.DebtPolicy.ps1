[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$RepoRoot
)

$ErrorActionPreference = 'Stop'
function Get-TorcaRelativePath { param([string]$BasePath,[string]$TargetPath); $base=(Resolve-Path -LiteralPath $BasePath).Path.TrimEnd('\')+'\'; $target=(Resolve-Path -LiteralPath $TargetPath).Path; if ($target.StartsWith($base,[StringComparison]::OrdinalIgnoreCase)) { return $target.Substring($base.Length).Replace('\','/') }; [Uri]::UnescapeDataString(([Uri]::new($base)).MakeRelativeUri([Uri]::new($target)).ToString()) }

$sourceRoots = @(
    (Join-Path $RepoRoot 'crates'),
    (Join-Path $RepoRoot 'apps/client/flutter/lib')
)

foreach ($root in $sourceRoots) {
    if (-not (Test-Path -LiteralPath $root)) { continue }
    foreach ($file in Get-ChildItem -LiteralPath $root -Recurse -File) {
        if ($file.FullName -match '[\\/]target[\\/]' -or $file.Name -eq 'concat.txt') { continue }
        $text = Get-Content -LiteralPath $file.FullName -Raw
        foreach ($marker in @('TODO', 'FIXME')) {
            if ($text.Contains($marker) -and $text -notmatch "${marker}\(#\d+\)") {
                $relative = Get-TorcaRelativePath $RepoRoot $file.FullName
                throw "$marker must reference a concrete tracker issue as ${marker}(#123): $relative"
            }
        }
    }
}

$obsolete = @(
    'crates/application/torca-pairing-coordinator/src/final_runtime.rs',
    'crates/infrastructure/torca-storage-sqlite/src/migration_v2.rs',
    'crates/infrastructure/torca-storage-sqlite/src/migration_v3.rs',
    'crates/platform/torca-native/src/retry_ffi.rs'
)
foreach ($relative in $obsolete) {
    if (Test-Path -LiteralPath (Join-Path $RepoRoot $relative)) {
        throw "Obsolete compatibility source returned: $relative"
    }
}

# This is the only intentionally retained source-compatibility shim. It exists
# until all downstream repositories construct typed EngineError variants.
$legacyError = Join-Path $RepoRoot 'crates/application/torca-client-engine/src/engine/legacy_error.rs'
if (Test-Path -LiteralPath $legacyError) {
    $text = Get-Content -LiteralPath $legacyError -Raw
    if (-not $text.Contains('#[doc(hidden)]') -or -not $text.Contains('EngineError::Repository')) {
        throw 'Legacy EngineError constructor must remain hidden and redacted until removed.'
    }
}

Write-Host 'Torca source debt policy passed.'
