[CmdletBinding()]
param(
    [ValidateSet('cockpit', 'plain')]
    [string]$Mode = 'cockpit',
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$Arguments = @()
)

# Canonical SOAK1 entry point. Build/deploy output is deliberately kept out of
# the user's terminal; the Rust cockpit receives runtime output and exposes it
# through its Logs view. The bootstrap log is retained for failed compilation.
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$artifactRoot = Join-Path $repo '.torca\soak\bootstrap'
$null = New-Item -ItemType Directory -Force -Path $artifactRoot
$bootstrapLog = Join-Path $artifactRoot 'latest.log'
$exe = Join-Path $repo 'target\debug\torca-soak.exe'
$previousSoakFlavor = $env:TORCA_SOAK_FLAVOR

Push-Location $repo
try {
    Write-Host "SOAK1 bootstrap: preparing cockpit (build details are captured in $bootstrapLog)"
    & cargo build -q -p torca-soak --locked *> $bootstrapLog
    if ($LASTEXITCODE -ne 0 -or -not (Test-Path -LiteralPath $exe)) {
        Write-Error "SOAK1 bootstrap build failed. Details: $bootstrapLog"
        if (Test-Path -LiteralPath $bootstrapLog) {
            Get-Content -LiteralPath $bootstrapLog -Tail 30 | ForEach-Object { Write-Host $_ }
        }
        exit 1
    }

    Write-Host "SOAK1 bootstrap: cockpit ready; starting run"
    # The Android deployer and ScenarioBridge use this flag to select the
    # isolated com.torca.torca_app.soak flavor. Ordinary deploys never inherit it.
    $env:TORCA_SOAK_FLAVOR = '1'
    if ($Mode -eq 'plain') {
        & $exe --plain @Arguments
    } elseif ($Arguments.Count -eq 0) {
        & $exe --tui
    } else {
        # `--tui` is a launcher switch only when it is the sole argument.
        # With an explicit plan the Rust binary detects the interactive
        # terminal itself; passing `--tui` would otherwise be an unknown CLI
        # option and make the cockpit fail before rendering.
        & $exe @Arguments
    }
    exit $LASTEXITCODE
} finally {
    if ($null -eq $previousSoakFlavor) {
        Remove-Item Env:TORCA_SOAK_FLAVOR -ErrorAction SilentlyContinue
    } else {
        $env:TORCA_SOAK_FLAVOR = $previousSoakFlavor
    }
    Pop-Location
}
