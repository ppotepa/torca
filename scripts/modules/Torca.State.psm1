Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Read-TorcaJsonState {
    param([Parameter(Mandatory = $true)][string]$Path, [object]$Default = $null)
    if (-not (Test-Path -LiteralPath $Path)) { return $Default }
    try { return Get-Content -LiteralPath $Path -Raw | ConvertFrom-Json } catch { throw "Invalid Torca state file: $Path" }
}

function Write-TorcaJsonState {
    param([Parameter(Mandatory = $true)][string]$Path, [Parameter(Mandatory = $true)]$Value)
    $parent = Split-Path -Parent $Path
    New-Item -ItemType Directory -Path $parent -Force | Out-Null
    $temporary = "$Path.tmp"
    $Value | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath $temporary -Encoding utf8
    Move-Item -LiteralPath $temporary -Destination $Path -Force
}

function Get-TorcaRuntimeState {
    param([Parameter(Mandatory = $true)]$Paths)
    $state = Read-TorcaJsonState -Path $Paths.StateFile
    if ($null -eq $state) {
        return [pscustomobject]@{ Schema = 4; Provider = 'process'; RelayPid = $null; Endpoint = $null; UpdatedAt = $null }
    }
    if (-not ($state.PSObject.Properties.Name -contains 'Provider')) {
        $state | Add-Member -MemberType NoteProperty -Name Provider -Value 'process'
    }
    if ($state.PSObject.Properties.Name -contains 'TorPid') {
        $state.PSObject.Properties.Remove('TorPid')
    }
    $state.Schema = 4
    return $state
}

function Set-TorcaRuntimeState {
    param([Parameter(Mandatory = $true)]$Paths, [Parameter(Mandatory = $true)]$State)
    Write-TorcaJsonState -Path $Paths.StateFile -Value $State
}

function Get-TorcaBuildManifest {
    param([Parameter(Mandatory = $true)]$Paths)
    return Read-TorcaJsonState -Path $Paths.ManifestFile
}

function Set-TorcaBuildManifest {
    param([Parameter(Mandatory = $true)]$Paths, [Parameter(Mandatory = $true)]$Manifest)
    Write-TorcaJsonState -Path $Paths.ManifestFile -Value $Manifest
}

Export-ModuleMember -Function Read-TorcaJsonState, Write-TorcaJsonState, Get-TorcaRuntimeState, Set-TorcaRuntimeState, Get-TorcaBuildManifest, Set-TorcaBuildManifest
