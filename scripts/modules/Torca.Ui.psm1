Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

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
    [pscustomobject]@{
        OnionPolicy = Read-TorcaMenuChoice 'Tor network' @('Ensure - preserve or create', 'Restart - restart without changing onion', 'Rotate - create new onion') '1'
        ClientDataPolicy = Read-TorcaMenuChoice 'Client data / database' @('Preserve - keep data', 'ResetSelected - clear selected devices', 'ResetAll - clear all devices') '1'
        BuildPolicy = Read-TorcaMenuChoice 'Build' @('IfRequired - rebuild if endpoint changed', 'Rebuild - always rebuild', 'Reuse - use existing artifacts') '1'
        InstallPolicy = Read-TorcaMenuChoice 'Install' @('Selected - install on selected', 'Always - install on all detected', 'Skip - build only') '1'
        RunPolicy = Read-TorcaMenuChoice 'Run' @('Restart - restart after deploy', 'Start - start if stopped', 'Skip - do not run') '1'
    }
}

Export-ModuleMember -Function Read-TorcaMenuChoice, Get-TorcaInteractiveOptions
