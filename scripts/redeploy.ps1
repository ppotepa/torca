[CmdletBinding()]
param(
    [string[]]$Device,
    [switch]$UseLast,
    [ValidateSet('debug', 'release')][string]$Configuration,
    [ValidateSet('Full', 'Quick', 'Skip')][string]$Validation,
    [ValidateSet('IfRequired', 'Rebuild', 'Reuse', 'Existing')][string]$BuildPolicy,
    [switch]$NoRun
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$presetPath = Join-Path $root '.torca/last-deploy.json'

# PowerShell convention is `-UseLast`, but accept the commonly typed GNU form
# as well. Without this normalization `--UseLast` binds positionally to Device.
if ($Device -contains '--UseLast') {
    $UseLast = $true
    $Device = @($Device | Where-Object { $_ -ne '--UseLast' })
}

$preset = $null
if ($UseLast) {
    if (-not (Test-Path -LiteralPath $presetPath)) {
        # Older deploys wrote the preset only after installation and launch.
        # Recover an interrupted deploy from durable device/build manifests.
        $deviceManifests = @(Get-ChildItem (Join-Path $root '.torca/devices') -Filter '*.json' -ErrorAction SilentlyContinue | ForEach-Object {
            try { Get-Content -LiteralPath $_.FullName -Raw | ConvertFrom-Json } catch { $null }
        } | Where-Object { $_ -and $_.DeviceId })
        $buildManifestFile = Get-ChildItem (Join-Path $root '.torca/manifests') -Filter '*.json' -ErrorAction SilentlyContinue |
            Sort-Object LastWriteTime -Descending |
            Select-Object -First 1
        if ($deviceManifests.Count -eq 0 -or -not $buildManifestFile) {
            throw "No previous deployment preset exists: $presetPath. Complete one deploy first."
        }
        $buildManifest = Get-Content -LiteralPath $buildManifestFile.FullName -Raw | ConvertFrom-Json
        $platforms = @($deviceManifests | ForEach-Object { [string]$_.Platform } | Select-Object -Unique)
        $recoveredTarget = if ($platforms.Count -gt 1) { 'all' } else { $platforms[0] }
        $preset = [pscustomobject]@{
            Schema = 1
            Target = $recoveredTarget
            Devices = @($deviceManifests | ForEach-Object { [string]$_.DeviceId })
            Configuration = [string]$buildManifest.Configuration
            Validation = if ([string]$buildManifest.Configuration -eq 'release') { 'Full' } else { 'Quick' }
            BuildPolicy = 'IfRequired'
        }
        $preset | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $presetPath -Encoding utf8
        Write-Host "Recovered interrupted deployment preset from device/build manifests: $presetPath" -ForegroundColor Yellow
    } else {
        try { $preset = Get-Content -LiteralPath $presetPath -Raw | ConvertFrom-Json }
        catch { throw "Previous deployment preset is invalid: $presetPath" }
    }
}

$resolvedDevices = if ($Device) { @($Device) } elseif ($preset) { @($preset.Devices) } else { @() }
if ($resolvedDevices.Count -eq 0) {
    throw 'Specify -Device or use -UseLast after a successful deployment.'
}

$resolvedTarget = if ($preset -and $preset.Target) { [string]$preset.Target } else { 'android' }
$resolvedConfiguration = if ($PSBoundParameters.ContainsKey('Configuration')) {
    $Configuration
} elseif ($preset -and $preset.Configuration) {
    [string]$preset.Configuration
} else {
    'release'
}
$resolvedValidation = if ($PSBoundParameters.ContainsKey('Validation')) {
    $Validation
} elseif ($preset -and $preset.Validation) {
    [string]$preset.Validation
} elseif ($resolvedConfiguration -eq 'debug') {
    'Quick'
} else {
    'Full'
}
$resolvedBuildPolicy = if ($PSBoundParameters.ContainsKey('BuildPolicy')) {
    $BuildPolicy
} elseif ($preset -and $preset.BuildPolicy) {
    [string]$preset.BuildPolicy
} else {
    'Reuse'
}
$runPolicy = if ($NoRun) { 'Skip' } else { 'Restart' }

Write-Host "Redeploy preset: target=$resolvedTarget configuration=$resolvedConfiguration validation=$resolvedValidation build=$resolvedBuildPolicy devices=$($resolvedDevices -join ',')"
& (Join-Path $PSScriptRoot 'deploy.ps1') `
    -Target $resolvedTarget `
    -Device $resolvedDevices `
    -Configuration $resolvedConfiguration `
    -Validation $resolvedValidation `
    -OnionPolicy Ensure `
    -ClientDataPolicy Preserve `
    -BuildPolicy $resolvedBuildPolicy `
    -InstallPolicy Selected `
    -RunPolicy $runPolicy `
    -NonInteractive
if ($LASTEXITCODE -ne 0) { throw "Redeploy failed with code $LASTEXITCODE." }
