[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$RepoRoot
)

$ErrorActionPreference = 'Stop'
function Get-TorcaRelativePath { param([string]$BasePath,[string]$TargetPath); $base=(Resolve-Path -LiteralPath $BasePath).Path.TrimEnd('\')+'\'; $target=(Resolve-Path -LiteralPath $TargetPath).Path; if ($target.StartsWith($base,[StringComparison]::OrdinalIgnoreCase)) { return $target.Substring($base.Length).Replace('\','/') }; [Uri]::UnescapeDataString(([Uri]::new($base)).MakeRelativeUri([Uri]::new($target)).ToString()) }
$rustFiles = Get-ChildItem -LiteralPath (Join-Path $RepoRoot 'crates') -Recurse -File -Filter '*.rs' |
    Where-Object { $_.FullName -notmatch '[\\/]target[\\/]' }

$includeRegex = [regex]'include!\(\s*"([^"]+)"\s*\)'
foreach ($source in $rustFiles) {
    $text = Get-Content -LiteralPath $source.FullName -Raw
    foreach ($match in $includeRegex.Matches($text)) {
        $target = [IO.Path]::GetFullPath((Join-Path $source.DirectoryName $match.Groups[1].Value))
        if (-not (Test-Path -LiteralPath $target -PathType Leaf)) {
            $relative = Get-TorcaRelativePath $RepoRoot $source.FullName
            throw "include! target does not exist: $relative -> $($match.Groups[1].Value)"
        }
        $targetText = Get-Content -LiteralPath $target -Raw
        $trimmed = $targetText.TrimStart()
        if ($trimmed.StartsWith('//!') -or $trimmed.StartsWith('#![')) {
            $relativeTarget = Get-TorcaRelativePath $RepoRoot $target
            throw "include! fragment must not start with an inner doc/attribute: $relativeTarget"
        }
    }
}

Write-Host 'Torca include-fragment policy passed.'
