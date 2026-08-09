[CmdletBinding()]
param([switch]$Quick)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$checks = @(
    @{ Name = 'PowerShell'; Command = 'powershell' },
    @{ Name = 'Cargo'; Command = 'cargo' },
    @{ Name = 'Flutter'; Command = 'flutter' },
    @{ Name = 'ADB'; Command = 'adb' },
    @{ Name = 'Docker'; Command = 'docker' }
)
$failed = 0
Write-Host 'Torca environment doctor' -ForegroundColor Cyan
foreach ($check in $checks) {
    $found = Get-Command $check.Command -ErrorAction SilentlyContinue
    if ($found) { Write-Host ("[OK]   {0}: {1}" -f $check.Name, $found.Source) -ForegroundColor Green }
    else { Write-Host ("[MISS] {0}: {1}" -f $check.Name, $check.Command) -ForegroundColor Red; $failed++ }
}

foreach ($path in @(
    (Join-Path $root 'Cargo.toml'),
    (Join-Path $root 'apps/client/flutter/pubspec.yaml'),
    (Join-Path $root 'infra/docker/compose.yml')
)) {
    if (Test-Path -LiteralPath $path) { Write-Host "[OK]   $path" -ForegroundColor Green }
    else { Write-Host "[MISS] $path" -ForegroundColor Red; $failed++ }
}

if (-not $Quick) {
    & (Join-Path $PSScriptRoot 'torca.ps1') -Command status -NonInteractive
    if ($LASTEXITCODE -ne 0) { $failed++ }
    & (Join-Path $PSScriptRoot 'collect.ps1') -Target all -LastRuns 1 -Profile basic -KeepDirectory
    if ($LASTEXITCODE -ne 0) { $failed++ }
}
if ($failed -gt 0) { throw "Doctor found $failed problem(s)." }
Write-Host 'Doctor completed successfully.' -ForegroundColor Green
