[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$minimumFlutter = [version]'3.44.0'
$minimumDart = [version]'3.12.0'
$ciFlutter = '3.44.7'

function Get-VersionPrefix {
    param(
        [Parameter(Mandatory = $true)][string]$Value,
        [Parameter(Mandatory = $true)][string]$Name
    )

    $match = [regex]::Match($Value, '(\d+)\.(\d+)\.(\d+)')
    if (-not $match.Success) {
        throw "Unable to parse $Name version from '$Value'."
    }

    return [version]$match.Value
}

$machineOutput = (& flutter --version --machine 2>&1 | Out-String).Trim()
if ($LASTEXITCODE -ne 0) {
    throw 'Flutter is unavailable. Install Flutter stable 3.44.x or newer before running Flutter validation.'
}

try {
    $info = $machineOutput | ConvertFrom-Json
} catch {
    throw 'Unable to parse `flutter --version --machine` output.'
}

$flutterText = [string]$info.frameworkVersion
$dartText = [string]$info.dartSdkVersion
$flutterVersion = Get-VersionPrefix -Value $flutterText -Name 'Flutter'
$dartVersion = Get-VersionPrefix -Value $dartText -Name 'Dart'

if ($flutterVersion -lt $minimumFlutter -or $dartVersion -lt $minimumDart) {
    throw @"
Unsupported local Flutter toolchain.
Required: Flutter >= $minimumFlutter and Dart >= $minimumDart.
Detected: Flutter $flutterText and Dart $dartText.
CI baseline: Flutter $ciFlutter on stable.
Update the stable Flutter SDK (`flutter channel stable`, then `flutter upgrade`) and verify with `flutter --version`.
"@
}

Write-Host "Flutter toolchain compatible: Flutter $flutterText, Dart $dartText (CI baseline Flutter $ciFlutter)."
