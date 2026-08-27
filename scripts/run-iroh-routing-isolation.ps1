[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)] [string]$AndroidSerial,
    [int]$DurationMinutes = 30,
    [int]$Repetitions = 3,
    [string]$OutputRoot = '.torca/measurements/iroh-routing-isolation',
    [string]$Cargo = 'cargo'
)

$ErrorActionPreference = 'Stop'
if ($DurationMinutes -lt 1) { throw 'DurationMinutes must be at least 1.' }
if ($Repetitions -lt 3) { throw 'Repetitions must be at least 3 for battery evidence.' }
$adb = Get-Command adb.exe -ErrorAction Stop
$root = [IO.Path]::GetFullPath($OutputRoot)
New-Item -ItemType Directory -Force -Path $root | Out-Null
$rows = [Collections.Generic.List[object]]::new()

function Get-Level([string]$runRoot, [string]$name) {
    $path = Join-Path $runRoot $name
    if (-not (Test-Path -LiteralPath $path)) { return $null }
    $match = Select-String -LiteralPath $path -Pattern '^\s*level:\s*(\d+)' | Select-Object -First 1
    if ($null -eq $match) { return $null }
    [int]$match.Matches[0].Groups[1].Value
}

foreach ($relay in @($false, $true)) {
    foreach ($discovery in @($false, $true)) {
        $case = 'relay-{0}-discovery-{1}' -f ($(if ($relay) { 'on' } else { 'off' })), ($(if ($discovery) { 'on' } else { 'off' }))
        for ($repeat = 1; $repeat -le $Repetitions; $repeat++) {
            $caseRoot = Join-Path $root "$case-run$repeat"
            New-Item -ItemType Directory -Force -Path $caseRoot | Out-Null
            $oldRelay = $env:TORCA_IROH_DISABLE_RELAY
            $oldDiscovery = $env:TORCA_IROH_DISABLE_DISCOVERY
            $oldProfile = $env:TORCA_SOAK_IROH_PROFILE
            try {
                # The matrix booleans describe the feature being enabled;
                # the provider knobs are deliberately named as disable flags.
                # Keep the polarity explicit so relay-on/discovery-on never
                # accidentally exercises the disabled configuration.
                if ($relay) { Remove-Item Env:TORCA_IROH_DISABLE_RELAY -ErrorAction SilentlyContinue } else { $env:TORCA_IROH_DISABLE_RELAY = '1' }
                if ($discovery) { Remove-Item Env:TORCA_IROH_DISABLE_DISCOVERY -ErrorAction SilentlyContinue } else { $env:TORCA_IROH_DISABLE_DISCOVERY = '1' }
                $env:TORCA_SOAK_IROH_PROFILE = 'always'
                & $Cargo run -p torca-soak -- --scenario idle-battery --android $AndroidSerial `
                    --duration-seconds ($DurationMinutes * 60) --communication-provider iroh `
                    --require-unplugged --require-screen-off --collect-native-diagnostics `
                    --output $caseRoot --plain
                if ($LASTEXITCODE -ne 0) { throw "soak failed for $case repetition $repeat" }
            } finally {
                if ($null -eq $oldRelay) { Remove-Item Env:TORCA_IROH_DISABLE_RELAY -ErrorAction SilentlyContinue } else { $env:TORCA_IROH_DISABLE_RELAY = $oldRelay }
                if ($null -eq $oldDiscovery) { Remove-Item Env:TORCA_IROH_DISABLE_DISCOVERY -ErrorAction SilentlyContinue } else { $env:TORCA_IROH_DISABLE_DISCOVERY = $oldDiscovery }
                if ($null -eq $oldProfile) { Remove-Item Env:TORCA_SOAK_IROH_PROFILE -ErrorAction SilentlyContinue } else { $env:TORCA_SOAK_IROH_PROFILE = $oldProfile }
            }
            $run = Get-ChildItem -LiteralPath $caseRoot -Directory | Sort-Object LastWriteTime | Select-Object -Last 1
            $start = Get-Level $run.FullName 'battery-start.txt'
            $end = Get-Level $run.FullName 'battery-end.txt'
            $rows.Add([pscustomobject]@{ case = $case; relayEnabled = $relay; discoveryEnabled = $discovery; repetition = $repeat; output = $run.FullName; batteryDropPercent = if ($null -ne $start -and $null -ne $end) { $start - $end } else { $null } })
        }
    }
}

[ordered]@{ schema = 1; generatedAtUtc = [DateTime]::UtcNow.ToString('o'); provider = 'iroh'; profile = 'always'; cases = @($rows) } |
    ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $root 'matrix.json') -Encoding utf8
Write-Host "Iroh routing isolation complete: $(Join-Path $root 'matrix.json')"
