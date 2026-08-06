[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$violations = @()
$forbiddenDomainTokens = @('rusqlite', 'flutter', 'std::net', 'TcpStream', 'DynamicLibrary', 'torca_storage_sqlite', 'torca_transport_tor')
Get-ChildItem (Join-Path $root 'crates/domains') -Recurse -Filter '*.rs' | ForEach-Object {
    $content = Get-Content $_.FullName -Raw
    foreach ($token in $forbiddenDomainTokens) { if ($content.Contains($token)) { $violations += "$($_.FullName): forbidden domain token '$token'" } }
}
$sqlOutsideStorage = Get-ChildItem (Join-Path $root 'crates') -Recurse -Filter '*.rs' | Where-Object { $_.FullName -notlike '*torca-storage-sqlite*' } | Where-Object { (Get-Content $_.FullName -Raw) -match '(?i)\b(SELECT|INSERT INTO|UPDATE\s+\w+\s+SET|CREATE TABLE|DELETE FROM)\b' }
foreach ($file in $sqlOutsideStorage) { $violations += "$($file.FullName): SQL text outside storage crate" }

$flutterMain = Get-Content (Join-Path $root 'apps/client/flutter/lib/main.dart') -Raw
if (-not $flutterMain.Contains("defaultValue: false")) { $violations += 'Flutter release entrypoint must default TORCA_USE_MEMORY_GATEWAY to false' }
if (-not $flutterMain.Contains('MethodChannelEngineGateway')) { $violations += 'Flutter release entrypoint must use MethodChannelEngineGateway' }
if ($flutterMain -match 'runApp\(TorcaApp\(gateway:\s*MemoryEngineGateway\(\)\)\)') { $violations += 'Flutter release entrypoint directly selects MemoryEngineGateway' }

$channel = 'torca.engine.v1'
$gateway = Get-Content (Join-Path $root 'apps/client/flutter/lib/gateway/method_channel_engine_gateway.dart') -Raw
if (-not $gateway.Contains($channel)) { $violations += "Flutter native gateway is not bound to $channel" }
foreach ($manifest in @('apps/client/windows/host.json', 'apps/client/android/host.json')) {
    $manifestContent = Get-Content (Join-Path $root $manifest) -Raw
    if (-not $manifestContent.Contains($channel)) { $violations += "$manifest is not bound to $channel" }
    if (-not $manifestContent.Contains('"memoryGatewayAllowedInRelease": false')) { $violations += "$manifest permits memory gateway in release" }
}

if ($violations.Count -gt 0) { $violations | ForEach-Object { Write-Error $_ }; throw "Architecture boundary check failed with $($violations.Count) violation(s)." }
Write-Host 'Architecture boundary check passed.'
