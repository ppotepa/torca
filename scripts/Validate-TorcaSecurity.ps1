[CmdletBinding()]
param(
    [Parameter(Mandatory = $false)][string]$RepoRoot = $null,
    [Parameter(Mandatory = $false)][string]$OutputRoot = $null
)

$ErrorActionPreference = 'Stop'
# Cargo writes normal compilation progress to stderr. Keep native exit codes as
# the authority so PowerShell does not turn successful progress into an error.
$PSNativeCommandUseErrorActionPreference = $false
$scriptRoot = if ([string]::IsNullOrWhiteSpace($PSScriptRoot)) { (Get-Location).Path } else { $PSScriptRoot }
if ([string]::IsNullOrWhiteSpace($RepoRoot)) { $RepoRoot = (Resolve-Path (Join-Path $scriptRoot '..')).Path }
if ([string]::IsNullOrWhiteSpace($OutputRoot)) { $OutputRoot = (Join-Path $scriptRoot '../artifacts/security') }
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$output = Join-Path $OutputRoot $stamp
New-Item -ItemType Directory -Force -Path $output | Out-Null

Push-Location $RepoRoot
try {
    & (Join-Path $RepoRoot 'scripts/Validate-TorcaPolicies.ps1') -RepoRoot $RepoRoot
    if ($LASTEXITCODE -ne 0) { throw 'Repository policies failed security preflight.' }

    $suspiciousNames = @(
        '*.pem', '*.key', '*.p12', '*.pfx', '*.jks', '*.keystore',
        'id_rsa', 'id_ed25519', '.env'
    )
    $tracked = & git ls-files
    if ($LASTEXITCODE -ne 0) { throw 'git ls-files failed.' }
    $matches = foreach ($file in $tracked) {
        foreach ($pattern in $suspiciousNames) {
            if ([IO.Path]::GetFileName($file) -like $pattern) { $file; break }
        }
    }
    $matches | Sort-Object -Unique | Out-File -Encoding utf8 (Join-Path $output 'suspicious-tracked-files.txt')
    if ($matches) {
        throw 'Potential secret/key files are tracked; inspect artifacts/security output.'
    }

    & cargo tree --locked -d 2>&1 | Out-File -Encoding utf8 (Join-Path $output 'duplicate-dependencies.txt')
    if ($LASTEXITCODE -ne 0) { throw 'cargo tree dependency audit failed.' }

    $packages = @('torca-crypto', 'torca-peer-protocol', 'torca-pairing-protocol', 'torca-storage-sqlite')
    $arguments = @('test', '--locked')
    foreach ($package in $packages) { $arguments += @('-p', $package) }
    $previousErrorAction = $ErrorActionPreference
    try {
        $ErrorActionPreference = 'Continue'
        & cargo @arguments 2>&1 | Tee-Object -FilePath (Join-Path $output 'security-tests.txt')
        $securityTestExitCode = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $previousErrorAction
    }
    if ($securityTestExitCode -ne 0) { throw 'Security-sensitive package tests failed.' }

    $audit = Get-Command cargo-audit -ErrorAction SilentlyContinue
    if ($audit) {
        try {
            $ErrorActionPreference = 'Continue'
            & cargo audit 2>&1 | Tee-Object -FilePath (Join-Path $output 'cargo-audit.txt')
            $cargoAuditExitCode = $LASTEXITCODE
        } finally {
            $ErrorActionPreference = $previousErrorAction
        }
        if ($cargoAuditExitCode -ne 0) { throw 'cargo audit reported a blocking advisory.' }
    }
    else {
        'cargo-audit not installed; dependency advisory scan not executed.' |
            Out-File -Encoding utf8 (Join-Path $output 'cargo-audit.txt')
    }

    @{
        finishedAt = (Get-Date).ToString('o')
        packages = $packages
        cargoAuditExecuted = [bool]$audit
        status = 'preflight-complete'
    } | ConvertTo-Json | Out-File -Encoding utf8 (Join-Path $output 'result.json')

    Write-Host "Security preflight complete: $output"
}
finally {
    Pop-Location
}
