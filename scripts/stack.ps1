[CmdletBinding()]
param(
    [ValidateSet('status','start','restart','rotate','stop')]
    [string]$Action = 'status',
    [ValidateSet('auto','docker','process')]
    [string]$Provider = 'auto'
)

$ErrorActionPreference = 'Stop'
$arguments = if ($Action -eq 'status') {
    @{ Command = 'status'; NonInteractive = $true }
} else {
    @{ Command = 'stack'; StackAction = $Action; StackProvider = $Provider; NonInteractive = $true }
}
& (Join-Path $PSScriptRoot 'torca.ps1') @arguments
if ($LASTEXITCODE -ne 0) { throw "Stack operation failed with code $LASTEXITCODE." }
