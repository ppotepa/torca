[CmdletBinding()]
param(
    [ValidateSet('Flutter','Rust','Artifacts','Docker')][string]$Scope = 'Flutter',
    [switch]$Confirm
)

$ErrorActionPreference = 'Stop'
if (-not $Confirm) { throw "Cleaning '$Scope' changes generated data. Re-run with -Confirm." }
$root = Split-Path -Parent $PSScriptRoot
switch ($Scope) {
    'Flutter' {
        Push-Location (Join-Path $root 'apps/client/flutter')
        try { & flutter clean } finally { Pop-Location }
    }
    'Rust' { & cargo clean --manifest-path (Join-Path $root 'Cargo.toml') }
    'Artifacts' {
        $path = Join-Path $root 'artifacts'
        if (Test-Path -LiteralPath $path) { Remove-Item -LiteralPath $path -Recurse -Force }
    }
    'Docker' {
        & docker builder prune --filter 'label!=keep' --force
        & docker image prune --filter 'label!=keep' --force
    }
}
Write-Host "Clean completed: $Scope" -ForegroundColor Green
