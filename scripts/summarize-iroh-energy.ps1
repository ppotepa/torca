[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$BatteryMatrix,
    [Parameter(Mandatory = $false)]
    [string[]]$CpuReport = @(),
    [string]$Output = '.torca/measurements/iroh-energy-report.json'
)

$ErrorActionPreference = 'Stop'
$matrixPath = [IO.Path]::GetFullPath($BatteryMatrix)
if (-not (Test-Path -LiteralPath $matrixPath -PathType Leaf)) {
    throw "battery matrix report not found: $matrixPath"
}
$matrix = Get-Content -LiteralPath $matrixPath -Raw | ConvertFrom-Json
$reportPath = [IO.Path]::GetFullPath($Output)
New-Item -ItemType Directory -Force -Path (Split-Path -Parent $reportPath) | Out-Null

$runs = @($matrix.cases)
$required = @('tor/managed', 'iroh/always', 'iroh/direct', 'iroh/local')
$present = @($runs | ForEach-Object { "{0}/{1}" -f $_.provider, $_.profile } | Sort-Object -Unique)
$missing = @($required | Where-Object { $_ -notin $present })
$invalidRuns = @($runs | Where-Object {
    $_.exitCode -ne 0 -or $_.networkStable -ne $true -or
    $null -eq $_.batteryStartPercent -or $null -eq $_.batteryEndPercent -or
    $null -eq $_.batteryDropPercent
})
$missingBatterySamples = @($runs | Where-Object {
    $null -eq $_.batteryStartPercent -or $null -eq $_.batteryEndPercent -or
    $null -eq $_.batteryDropPercent
})
$groups = @($runs | Group-Object { "{0}/{1}" -f $_.provider, $_.profile })
$tooFewRuns = @($groups | Where-Object { $_.Count -lt 3 } | ForEach-Object Name)

$cpu = [Collections.Generic.List[object]]::new()
foreach ($path in $CpuReport) {
    $fullPath = [IO.Path]::GetFullPath($path)
    if (-not (Test-Path -LiteralPath $fullPath -PathType Leaf)) {
        throw "CPU report not found: $fullPath"
    }
    $cpu.Add((Get-Content -LiteralPath $fullPath -Raw | ConvertFrom-Json))
}

$batteryEvidenceComplete = $missing.Count -eq 0 -and $invalidRuns.Count -eq 0 -and $tooFewRuns.Count -eq 0
$tor = @($matrix.summaries | Where-Object { $_.providerProfile -eq 'tor/managed' } | Select-Object -First 1)
$iroh = @($matrix.summaries | Where-Object { $_.providerProfile -in @('iroh/always', 'iroh/direct', 'iroh/local') })
$comparison = foreach ($row in $iroh) {
    [pscustomobject]@{
        providerProfile = $row.providerProfile
        torMedianDropPercent = if ($tor.Count -eq 1) { $tor[0].batteryDropMedianPercent } else { $null }
        irohMedianDropPercent = $row.batteryDropMedianPercent
        lowerMedianDropThanTor = if ($tor.Count -eq 1 -and $null -ne $row.batteryDropMedianPercent -and $null -ne $tor[0].batteryDropMedianPercent) {
            [double]$row.batteryDropMedianPercent -lt [double]$tor[0].batteryDropMedianPercent
        } else { $null }
    }
}

$result = [ordered]@{
    schema = 1
    generatedAtUtc = [DateTime]::UtcNow.ToString('o')
    sourceMatrix = $matrixPath
    evidenceStatus = if ($batteryEvidenceComplete) { 'complete' } else { 'incomplete' }
    missingCases = $missing
    tooFewRuns = $tooFewRuns
    invalidRunCount = $invalidRuns.Count
    missingBatterySampleCount = $missingBatterySamples.Count
    batteryEvidenceComplete = $batteryEvidenceComplete
    comparison = @($comparison)
    cpuReports = @($cpu)
    interpretation = 'A lower batteryDropMedianPercent is better. A null comparison means the evidence is incomplete; no provider advantage is inferred.'
}
$result | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath $reportPath -Encoding utf8
Write-Output $reportPath
if (-not $batteryEvidenceComplete) { exit 2 }
