[CmdletBinding()]
param([switch]$SkipRust, [switch]$SkipFlutter)
$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
$dartSchema = Join-Path $repoRoot 'crates/platform/torca-bridge/schema/torca_contract.dart'
Push-Location $repoRoot
try {
    if (-not $SkipRust) { cargo fmt --all; if ($LASTEXITCODE -ne 0) { throw 'Rust formatting failed.' } }
    if (-not $SkipFlutter) { Push-Location (Join-Path $repoRoot 'apps/client/flutter'); try { dart format lib test $dartSchema; if ($LASTEXITCODE -ne 0) { throw 'Dart formatting failed.' } } finally { Pop-Location } }
} finally { Pop-Location }
