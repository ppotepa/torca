Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Write-TorcaConsoleHeader {
    param([Parameter(Mandatory = $true)][string]$Title, [hashtable]$Details = @{})
    $width = [Math]::Max(42, $Title.Length + 8)
    $line = '═' * $width
    $content = "  $Title"
    Write-Host "`n╔$line╗" -ForegroundColor DarkCyan
    Write-Host ("║{0}║" -f $content.PadRight($width)) -ForegroundColor Cyan
    Write-Host "╚$line╝" -ForegroundColor DarkCyan
    foreach ($entry in $Details.GetEnumerator()) {
        Write-Host ("  {0,-14} {1}" -f ("$($entry.Key):"), $entry.Value) -ForegroundColor DarkGray
    }
}

function Write-TorcaStage {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][ValidateSet('pending', 'running', 'ready', 'warning', 'failed')][string]$State,
        [string]$Detail
    )
    $visual = @{
        pending = @{ Symbol = '○'; Color = 'DarkGray' }
        running = @{ Symbol = '◌'; Color = 'Cyan' }
        ready = @{ Symbol = '✓'; Color = 'Green' }
        warning = @{ Symbol = '!'; Color = 'Yellow' }
        failed = @{ Symbol = '✗'; Color = 'Red' }
    }[$State]
    $message = "  $($visual.Symbol) $Name"
    if ($Detail) { $message += " — $Detail" }
    Write-Host $message -ForegroundColor $visual.Color
}

function Update-TorcaActivity {
    param(
        [Parameter(Mandatory = $true)][int]$Id,
        [Parameter(Mandatory = $true)][string]$Activity,
        [Parameter(Mandatory = $true)][string]$Status,
        [ValidateRange(0, 100)][int]$PercentComplete = 0
    )
    Write-Progress -Id $Id -Activity $Activity -Status $Status -PercentComplete $PercentComplete
}

function Complete-TorcaActivity {
    param([Parameter(Mandatory = $true)][int]$Id, [string]$Status = 'Completed')
    Write-Progress -Id $Id -Activity 'Torca' -Status $Status -Completed
}

function Read-TorcaMenuChoice {
    param([string]$Prompt, [string[]]$Options, [string]$Default = '1')
    Write-Host "`n$Prompt" -ForegroundColor Cyan
    for ($i = 0; $i -lt $Options.Count; $i++) { Write-Host " [$($i + 1)] $($Options[$i])" }
    $value = Read-Host "Choice [$Default]"
    if ([string]::IsNullOrWhiteSpace($value)) { $value = $Default }
    $number = 0
    if (-not [int]::TryParse($value, [ref]$number) -or $number -lt 1 -or $number -gt $Options.Count) { throw 'Invalid menu choice.' }
    $Options[$number - 1]
}

function Get-TorcaInteractiveOptions {
    $onionPolicy = Read-TorcaMenuChoice -Prompt "Tor network" -Default "1" -Options @(
        "Ensure - preserve or create",
        "Restart - restart without changing onion",
        "Rotate - create new onion"
    )
    $installPolicy = Read-TorcaMenuChoice -Prompt "Install" -Default "1" -Options @(
        "Selected - install on selected",
        "Always - install on all detected",
        "Skip - build only"
    )
    $runPolicy = Read-TorcaMenuChoice -Prompt "Run" -Default "1" -Options @(
        "Restart - restart after deploy",
        "Start - start if stopped",
        "Skip - do not run"
    )
    [pscustomobject]@{
        OnionPolicy = $onionPolicy
        InstallPolicy = $installPolicy
        RunPolicy = $runPolicy
    }
}

Export-ModuleMember -Function Read-TorcaMenuChoice, Get-TorcaInteractiveOptions, Write-TorcaConsoleHeader, Write-TorcaStage, Update-TorcaActivity, Complete-TorcaActivity
