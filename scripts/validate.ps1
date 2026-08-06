[CmdletBinding()]
param([switch]$SkipRust, [switch]$SkipFlutter)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$repoRoot = Split-Path -Parent $PSScriptRoot

function Invoke-Checked {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][scriptblock]$Command,
        [string]$Remediation
    )

    Write-Host "==> $Name"
    & $Command
    if ($LASTEXITCODE -ne 0) {
        if ($Remediation) {
            Write-Host "Next action: $Remediation" -ForegroundColor Yellow
        }
        throw "$Name failed with exit code $LASTEXITCODE."
    }
}

Push-Location $repoRoot
try {
    & (Join-Path $PSScriptRoot 'check-release.ps1')
    & (Join-Path $PSScriptRoot 'check-architecture.ps1')

    if (-not $SkipRust) {
        Invoke-Checked 'Rust formatting' { cargo fmt --all -- --check } './scripts/format.ps1 -SkipFlutter'
        Invoke-Checked 'Generated contract' { cargo run -p torca-contract-gen -- --check apps/client/flutter/lib/generated/torca_contract.dart }
        Invoke-Checked 'Rust check' { cargo check --workspace --all-targets --all-features }
        Invoke-Checked 'Rust clippy' { cargo clippy --workspace --all-targets --all-features -- -D clippy::correctness -D clippy::suspicious -D clippy::perf }
        Invoke-Checked 'Rust tests' { cargo test --workspace --all-targets --all-features }
    }

    if (-not $SkipFlutter) {
        Write-Host '==> Flutter toolchain'
        & (Join-Path $PSScriptRoot 'check-flutter-toolchain.ps1')

        Push-Location (Join-Path $repoRoot 'apps/client/flutter')
        try {
            Invoke-Checked 'Flutter dependencies' { flutter pub get }
            Invoke-Checked 'Dart formatting' { dart format --output=none --set-exit-if-changed lib test } '../../scripts/format.ps1 -SkipRust'
            Invoke-Checked 'Flutter analysis' { flutter analyze }
            Invoke-Checked 'Flutter tests' { flutter test }
        } finally {
            Pop-Location
        }
    }

    Write-Host 'Validation completed successfully.'
    Write-Host 'Review and commit Cargo.lock if Cargo refreshed dependency resolution.'
} finally {
    Pop-Location
}
