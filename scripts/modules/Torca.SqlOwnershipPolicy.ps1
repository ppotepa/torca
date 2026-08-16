[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$RepoRoot
)

$ErrorActionPreference = 'Stop'
$cratesRoot = Join-Path $RepoRoot 'crates'

$productionRust = Get-ChildItem -LiteralPath $cratesRoot -Recurse -File -Filter '*.rs' |
    Where-Object {
        $_.FullName -notmatch '[\\/]target[\\/]' -and
        $_.FullName -notmatch '[\\/]tests?[\\/]' -and
        $_.Name -notmatch '(?:^|_)test\.rs$' -and
        $_.Name -ne 'build.rs'
    }

# SQLCipher keying necessarily emits PRAGMA statements assembled from a raw
# key. It does not own business SELECT/INSERT/UPDATE/DELETE statements, so it
# remains covered by the keyword checks below without banning its PRAGMA code.
$callPattern = '(?is)\b(?:execute|execute_batch|prepare|query|query_row|query_map|query_and_then)\s*\(\s*(?:r#+)?["'']\s*(SELECT|INSERT|UPDATE|DELETE)\b'
$constPattern = '(?is)\b(?:const|static)\s+[A-Z0-9_]+[^=]*=\s*(?:r#+)?["'']\s*(SELECT|INSERT|UPDATE|DELETE)\b'

$violations = @()
foreach ($file in $productionRust) {
    $text = Get-Content -LiteralPath $file.FullName -Raw
    if ($text -match $callPattern -or $text -match $constPattern) {
        $relative = [IO.Path]::GetRelativePath($RepoRoot, $file.FullName).Replace('\\', '/')
        $violations += $relative
    }
}

if ($violations.Count -gt 0) {
    $listed = ($violations | Sort-Object -Unique) -join ', '
    throw "Production business SQL must live in named .sql files: $listed"
}

# Every storage SQL asset must be a non-empty text file. This catches orphaned
# zero-byte placeholders while allowing directories to be grouped by repository.
$sqlRoot = Join-Path $RepoRoot 'crates/infrastructure/torca-storage-sqlite/sql'
if (Test-Path -LiteralPath $sqlRoot) {
    $empty = Get-ChildItem -LiteralPath $sqlRoot -Recurse -File -Filter '*.sql' |
        Where-Object { [string]::IsNullOrWhiteSpace((Get-Content -LiteralPath $_.FullName -Raw)) }
    if (@($empty).Count -gt 0) {
        throw "Empty SQL assets are forbidden: $(@($empty.FullName) -join ', ')"
    }
}

Write-Host 'Torca SQL ownership policy passed.'
