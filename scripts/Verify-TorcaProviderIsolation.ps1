[CmdletBinding()]
param(
    [Parameter(Mandatory = $false)][string]$RepoRoot = $null
)

$ErrorActionPreference = 'Stop'
$scriptRoot = if ([string]::IsNullOrWhiteSpace($PSScriptRoot)) { (Get-Location).Path } else { $PSScriptRoot }
if ([string]::IsNullOrWhiteSpace($RepoRoot)) {
    $RepoRoot = (Resolve-Path (Join-Path $scriptRoot '..')).Path
}
Push-Location $RepoRoot
try {
    $variants = @{
        iroh = @('arti-', 'torca-tor', 'torca-transport-tor', 'torca-rendezvous-tor', 'torca-radio-tor')
        tor = @('iroh', 'torca-transport-iroh', 'torca-transport-webrtc')
        webrtc = @('iroh', 'arti-', 'torca-tor', 'torca-transport-iroh', 'torca-radio-tor')
    }

    foreach ($variant in $variants.Keys) {
        $features = "provider-$variant,radio-audio"
        $tree = (& cargo tree -p torca-native --no-default-features --features $features --locked 2>&1 | Out-String)
        if ($LASTEXITCODE -ne 0) {
            throw "Could not resolve dependency graph for provider '$variant': $tree"
        }
        foreach ($forbidden in $variants[$variant]) {
            if ($tree -match "(?m)(?:^|[\\/ ])$([regex]::Escape($forbidden))(?: [^\r\n]*| v)" -or
                $tree -match "(?m)\b$([regex]::Escape($forbidden))\b") {
                throw "Provider '$variant' unexpectedly contains forbidden dependency '$forbidden'."
            }
        }
        & cargo check -p torca-native --no-default-features --features $features --locked
        if ($LASTEXITCODE -ne 0) {
            throw "Provider '$variant' failed native composition compilation."
        }
        Write-Host "Provider isolation verified: $variant"
    }
} finally {
    Pop-Location
}
