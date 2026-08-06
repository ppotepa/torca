[CmdletBinding()]
param([ValidateSet('windows','android','all')][string]$Target = 'all', [switch]$SkipValidation)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
if (-not $SkipValidation) { & (Join-Path $PSScriptRoot 'validate.ps1') }
$release = Get-Content (Join-Path $root 'release/version.json') -Raw | ConvertFrom-Json
$artifactRoot = Join-Path $root "artifacts/$($release.version)-$($release.build)"
New-Item -ItemType Directory -Force -Path $artifactRoot | Out-Null
Push-Location $root
try {
    cargo build --workspace --release --locked
    Push-Location 'apps/client/flutter'
    try {
        if ($Target -in @('windows','all')) { flutter build windows --release; if ($LASTEXITCODE -ne 0) { throw 'Windows build failed.' }; Copy-Item 'build/windows/x64/runner/Release/*' (Join-Path $artifactRoot 'windows') -Recurse -Force }
        if ($Target -in @('android','all')) { flutter build apk --release; if ($LASTEXITCODE -ne 0) { throw 'Android build failed.' }; New-Item -ItemType Directory -Force -Path (Join-Path $artifactRoot 'android') | Out-Null; Copy-Item 'build/app/outputs/flutter-apk/app-release.apk' (Join-Path $artifactRoot 'android/torca.apk') -Force }
    } finally { Pop-Location }
    Get-ChildItem $artifactRoot -Recurse -File | ForEach-Object { "$(Get-FileHash $_.FullName -Algorithm SHA256 | Select-Object -ExpandProperty Hash)  $($_.FullName.Substring($artifactRoot.Length + 1).Replace('\','/'))" } | Set-Content (Join-Path $artifactRoot 'SHA256SUMS.txt')
} finally { Pop-Location }
Write-Host "Artifacts: $artifactRoot"
