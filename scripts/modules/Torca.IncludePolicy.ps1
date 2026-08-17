[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$RepoRoot
)

$ErrorActionPreference = 'Stop'
$rustFiles = Get-ChildItem -LiteralPath (Join-Path $RepoRoot 'crates') -Recurse -File -Filter '*.rs' |
    Where-Object { $_.FullName -notmatch '[\\/]target[\\/]' }

$includeRegex = [regex]'include!\(\s*"([^"]+)"\s*\)'
foreach ($source in $rustFiles) {
    $text = Get-Content -LiteralPath $source.FullName -Raw
    foreach ($match in $includeRegex.Matches($text)) {
        $target = [IO.Path]::GetFullPath((Join-Path $source.DirectoryName $match.Groups[1].Value))
        if (-not (Test-Path -LiteralPath $target -PathType Leaf)) {
            $relative = [IO.Path]::GetRelativePath($RepoRoot, $source.FullName).Replace('\\', '/')
            throw "include! target does not exist: $relative -> $($match.Groups[1].Value)"
        }
        $targetText = Get-Content -LiteralPath $target -Raw
        $trimmed = $targetText.TrimStart()
        if ($trimmed.StartsWith('//!') -or $trimmed.StartsWith('#![')) {
            $relativeTarget = [IO.Path]::GetRelativePath($RepoRoot, $target).Replace('\\', '/')
            throw "include! fragment must not start with an inner doc/attribute: $relativeTarget"
        }
    }
}

Write-Host 'Torca include-fragment policy passed.'
