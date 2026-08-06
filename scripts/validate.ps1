[CmdletBinding()]
param(
    [switch]$SkipRust,
    [switch]$SkipFlutter
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repoRoot = Split-Path -Parent $PSScriptRoot

function Invoke-Checked {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Name,
        [Parameter(Mandatory = $true)]
        [scriptblock]$Command
    )

    Write-Host "==> $Name"
    & $Command
    if ($LASTEXITCODE -ne 0) {
        throw "$Name failed with exit code $LASTEXITCODE."
    }
}

Push-Location $repoRoot
try {
    if (-not $SkipRust) {
        Invoke-Checked 'Rust formatting' { cargo fmt --all -- --check }
        Invoke-Checked 'Rust check' { cargo check --workspace --all-targets --all-features --locked }
        Invoke-Checked 'Rust clippy' { cargo clippy --workspace --all-targets --all-features --locked -- -D warnings }
        Invoke-Checked 'Rust tests' { cargo test --workspace --all-targets --all-features --locked }
    }

    if (-not $SkipFlutter) {
        Push-Location (Join-Path $repoRoot 'apps/client/flutter')
        try {
            Invoke-Checked 'Flutter dependencies' { flutter pub get }
            Invoke-Checked 'Dart formatting' { dart format --output=none --set-exit-if-changed lib test }
            Invoke-Checked 'Flutter analysis' { flutter analyze }
            Invoke-Checked 'Flutter tests' { flutter test }
        }
        finally {
            Pop-Location
        }
    }

    Write-Host 'Validation completed successfully.'
}
finally {
    Pop-Location
}
