[CmdletBinding()]
param(
    [Parameter(Mandatory = $false)][int]$Iterations = 25,
    [Parameter(Mandatory = $false)][string]$RepoRoot
)

$ErrorActionPreference = 'Stop'
$scriptRoot = if ([string]::IsNullOrWhiteSpace($PSScriptRoot)) { (Get-Location).Path } else { $PSScriptRoot }
if ($Iterations -lt 1) { throw 'Iterations must be at least 1.' }
if (-not $RepoRoot) {
    $RepoRoot = (Resolve-Path (Join-Path $scriptRoot '..')).Path
}

Push-Location $RepoRoot
try {
    & (Join-Path $RepoRoot 'scripts/Validate-TorcaPolicies.ps1') -RepoRoot $RepoRoot
    if ($LASTEXITCODE -ne 0) { throw 'Repository policies failed before soak.' }

    $packages = @(
        'torca-runtime-policy',
        'torca-diagnostics',
        'torca-runtime',
        'torca-peer-link',
        'torca-storage-sqlite'
    )
    $providerPackage = 'torca-transport-iroh'
    $packages += $providerPackage

    for ($iteration = 1; $iteration -le $Iterations; $iteration++) {
        $started = Get-Date
        Write-Host "Deterministic soak iteration $iteration/$Iterations (provider=iroh)"
        $arguments = @('test', '--locked')
        foreach ($package in $packages) {
            $arguments += @('-p', $package)
        }
        & cargo @arguments
        if ($LASTEXITCODE -ne 0) {
            throw "Deterministic soak failed at iteration $iteration."
        }
        # Exercise the same default native composition used by deployment.
        $nativeArguments = @('test', '--locked', '-p', 'torca-native')
        & cargo @nativeArguments
        if ($LASTEXITCODE -ne 0) {
            throw "Native Iroh soak failed at iteration $iteration."
        }
        $elapsed = (Get-Date) - $started
        Write-Host ("Iteration {0} passed in {1:n1}s" -f $iteration, $elapsed.TotalSeconds)
    }

    Write-Host "Deterministic soak passed: $Iterations iterations."
}
finally {
    Pop-Location
}
