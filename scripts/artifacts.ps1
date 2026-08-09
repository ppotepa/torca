[CmdletBinding()]
param(
    [ValidateSet('list','latest')][string]$Action = 'list',
    [switch]$Verify
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$artifacts = Join-Path $root 'artifacts'
if (-not (Test-Path -LiteralPath $artifacts)) { throw "Artifacts directory is missing: $artifacts" }
$items = @(Get-ChildItem $artifacts -Directory | Sort-Object LastWriteTime -Descending)
if ($Action -eq 'latest') { $items = @($items | Select-Object -First 1) }
foreach ($item in $items) {
    $files = @(Get-ChildItem $item.FullName -Recurse -File)
    $size = ($files | Measure-Object Length -Sum).Sum
    Write-Host ("{0}  files={1}  size={2:N0}  updated={3}" -f $item.Name, $files.Count, $size, $item.LastWriteTime)
    if ($Verify) {
        $checksums = Join-Path $item.FullName 'SHA256SUMS.txt'
        if (-not (Test-Path -LiteralPath $checksums)) { throw "Checksum manifest missing: $checksums" }
        $certutil = Get-Command certutil -ErrorAction SilentlyContinue
        if ($certutil) {
            Write-Host '  checksum-manifest SHA256:'
            & $certutil.Source -hashfile $checksums SHA256
            if ($LASTEXITCODE -ne 0) { throw "Unable to hash checksum manifest: $checksums" }
        } else {
            Write-Warning 'certutil is unavailable; checksum manifest presence was verified only.'
        }
    }
}
