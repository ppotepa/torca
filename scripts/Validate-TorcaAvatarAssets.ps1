[CmdletBinding()]
param(
    [string]$AssetsRoot = 'packages/torca_avatar/assets',
    [string]$Manifest = 'packages/torca_avatar/avatar_assets.json'
)

$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath($AssetsRoot)
if (-not (Test-Path -LiteralPath $root)) {
    Write-Host "Avatar asset root does not exist; generated spritesheets are used: $root"
    exit 0
}

$forbidden = @('.gif', '.apng', '.webp')
$violations = @(Get-ChildItem -LiteralPath $root -Recurse -File | Where-Object {
    $forbidden -contains $_.Extension.ToLowerInvariant()
})
if ($violations.Count -gt 0) {
    $paths = $violations | ForEach-Object { $_.FullName }
    throw "Animated avatar formats are forbidden; convert assets to spritesheets: $($paths -join ', ')"
}

if (Test-Path -LiteralPath $Manifest) {
    $value = Get-Content -Raw -Encoding UTF8 -LiteralPath $Manifest | ConvertFrom-Json
    foreach ($entry in @($value.avatars)) {
        if ([string]::IsNullOrWhiteSpace([string]$entry.spriteSheet) -or
            [int]$entry.frameCount -lt 1) {
            throw "Avatar manifest entry lacks a valid spriteSheet/frameCount: $($entry.id)"
        }
    }
}
Write-Host 'Avatar assets satisfy the spritesheet contract.'
