[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$violations = @()
$forbiddenDomainTokens = @('rusqlite', 'flutter', 'std::net', 'TcpStream', 'DynamicLibrary', 'torca_storage_sqlite', 'torca_transport_tor')
Get-ChildItem (Join-Path $root 'crates/domains') -Recurse -Filter '*.rs' | ForEach-Object {
    $content = Get-Content $_.FullName -Raw
    foreach ($token in $forbiddenDomainTokens) { if ($content.Contains($token)) { $violations += "$($_.FullName): forbidden domain token '$token'" } }
}
$sqlOutsideStorage = Get-ChildItem (Join-Path $root 'crates') -Recurse -Filter '*.rs' | Where-Object { $_.FullName -notlike '*torca-storage-sqlite*' } | Where-Object { (Get-Content $_.FullName -Raw) -match '(?i)\b(SELECT|INSERT INTO|UPDATE\s+\w+\s+SET|CREATE TABLE|DELETE FROM)\b' }
foreach ($file in $sqlOutsideStorage) { $violations += "$($file.FullName): SQL text outside storage crate" }
if ($violations.Count -gt 0) { $violations | ForEach-Object { Write-Error $_ }; throw "Architecture boundary check failed with $($violations.Count) violation(s)." }
Write-Host 'Architecture boundary check passed.'
