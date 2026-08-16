[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$RepoRoot
)

$ErrorActionPreference = 'Stop'
$storageRoot = Join-Path $RepoRoot 'crates/infrastructure/torca-storage-sqlite/src'
$factory = [IO.Path]::GetFullPath((Join-Path $storageRoot 'sqlcipher.rs'))

$violations = @()
$productionRust = Get-ChildItem -LiteralPath (Join-Path $RepoRoot 'crates') -Recurse -File -Filter '*.rs' |
    Where-Object {
        $_.FullName -notmatch '[\\/]target[\\/]' -and
        $_.FullName -notmatch '[\\/]tests?[\\/]' -and
        $_.Name -notmatch '(?:^|_)test\.rs$'
    }

foreach ($file in $productionRust) {
    $full = [IO.Path]::GetFullPath($file.FullName)
    if ($full -eq $factory) { continue }
    $text = Get-Content -LiteralPath $file.FullName -Raw
    if ($text -match '\bConnection::open(?:_with_flags|_in_memory)?\s*\(' -or
        $text -match '\brusqlite::Connection::open(?:_with_flags|_in_memory)?\s*\(') {
        $violations += [IO.Path]::GetRelativePath($RepoRoot, $file.FullName).Replace('\\', '/')
    }
}

if ($violations.Count -gt 0) {
    throw "SQLite connections must be created by SqlCipherBackend in sqlcipher.rs: $(($violations | Sort-Object -Unique) -join ', ')"
}

$bootstrap = Join-Path $RepoRoot 'crates/infrastructure/torca-storage-sqlite/sql/bootstrap.sql'
if (-not (Test-Path -LiteralPath $bootstrap)) {
    throw 'Storage bootstrap.sql is missing.'
}
$bootstrapText = Get-Content -LiteralPath $bootstrap -Raw
foreach ($required in @('PRAGMA foreign_keys', 'PRAGMA busy_timeout')) {
    if (-not $bootstrapText.Contains($required)) {
        throw "Storage bootstrap is missing required connection policy: $required"
    }
}

Write-Host 'Torca storage factory policy passed.'
