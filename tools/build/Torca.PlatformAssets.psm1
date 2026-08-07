Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Get-TorcaRelayEndpoint {
    param([Parameter(Mandatory = $true)][string]$RepoRoot)

    $value = [string]$env:TORCA_RELAY_ENDPOINT
    if ([string]::IsNullOrWhiteSpace($value)) {
        $file = Join-Path $RepoRoot 'release/relay_endpoint.txt'
        if (Test-Path $file) {
            $value = (Get-Content $file -Raw).Trim()
        }
    }
    if ($value -notmatch '^[a-z2-7]{56}\.onion:[1-9][0-9]{0,4}$') {
        throw 'Set TORCA_RELAY_ENDPOINT (or release/relay_endpoint.txt) to a v3 host.onion:port.'
    }
    $port = [int]($value -split ':')[-1]
    if ($port -gt 65535) { throw 'TORCA_RELAY_ENDPOINT port is outside 1..65535.' }
    return $value
}

function Ensure-TorcaPlatformScaffold {
    param(
        [Parameter(Mandatory = $true)][string]$RepoRoot,
        [Parameter(Mandatory = $true)][ValidateSet('windows','android')][string]$Platform
    )
    $flutterRoot = Join-Path $RepoRoot 'apps/client/flutter'
    $marker = if ($Platform -eq 'windows') {
        Join-Path $flutterRoot 'windows/CMakeLists.txt'
    } else {
        Join-Path $flutterRoot 'android/settings.gradle.kts'
    }
    if (Test-Path $marker) { return }
    Push-Location $flutterRoot
    try {
        & flutter create --platforms=$Platform --org com.torca --project-name torca_app .
        if ($LASTEXITCODE -ne 0) { throw "Flutter $Platform scaffold failed." }
    } finally { Pop-Location }
}

function Prepare-TorcaWindowsAssets {
    param([Parameter(Mandatory = $true)][string]$RepoRoot)
    if ($env:OS -ne 'Windows_NT') { throw 'Windows packaging requires a Windows host.' }
    Ensure-TorcaPlatformScaffold -RepoRoot $RepoRoot -Platform windows
    $flutterRoot = Join-Path $RepoRoot 'apps/client/flutter'
    $runner = Join-Path $flutterRoot 'windows/runner'
    $overlay = Join-Path $RepoRoot 'tools/build/overlays/windows'
    Copy-Item (Join-Path $overlay 'main.cpp') (Join-Path $runner 'main.cpp') -Force

    $vendorTor = Join-Path $RepoRoot 'vendor/tor/windows'
    if (-not (Test-Path (Join-Path $vendorTor 'tor.exe'))) {
        throw "Packaged Windows Tor is missing: $vendorTor"
    }
    $runnerTor = Join-Path $runner 'tor'
    Remove-Item $runnerTor -Recurse -Force -ErrorAction SilentlyContinue
    Copy-Item $vendorTor $runnerTor -Recurse -Force

    $endpoint = Get-TorcaRelayEndpoint -RepoRoot $RepoRoot
    Set-Content -Path (Join-Path $runner 'relay_endpoint.txt') -Value $endpoint -NoNewline -Encoding ascii

    $cmake = Join-Path $flutterRoot 'windows/CMakeLists.txt'
    $text = Get-Content $cmake -Raw
    $begin = '# TORCA_RUNTIME_ASSETS_BEGIN'
    $end = '# TORCA_RUNTIME_ASSETS_END'
    $pattern = '(?s)\r?\n?# TORCA_RUNTIME_ASSETS_BEGIN.*?# TORCA_RUNTIME_ASSETS_END\r?\n?'
    $text = [regex]::Replace($text, $pattern, "`n")
    $block = @"
$begin
add_custom_command(TARGET `${BINARY_NAME} POST_BUILD
  COMMAND `${CMAKE_COMMAND} -E copy_directory
    "`${CMAKE_CURRENT_SOURCE_DIR}/runner/tor"
    "`$<TARGET_FILE_DIR:`${BINARY_NAME}>/tor"
  COMMAND `${CMAKE_COMMAND} -E copy_if_different
    "`${CMAKE_CURRENT_SOURCE_DIR}/runner/relay_endpoint.txt"
    "`$<TARGET_FILE_DIR:`${BINARY_NAME}>/relay_endpoint.txt"
  COMMAND `${CMAKE_COMMAND} -E copy_if_different
    "`${CMAKE_CURRENT_SOURCE_DIR}/runner/resources/app_icon.ico"
    "`$<TARGET_FILE_DIR:`${BINARY_NAME}>/torca.ico")
$end
"@
    Set-Content -Path $cmake -Value ($text.TrimEnd() + "`r`n`r`n" + $block) -Encoding utf8
}

function Prepare-TorcaAndroidAssets {
    param([Parameter(Mandatory = $true)][string]$RepoRoot)
    Ensure-TorcaPlatformScaffold -RepoRoot $RepoRoot -Platform android
    $flutterRoot = Join-Path $RepoRoot 'apps/client/flutter'
    $androidMain = Join-Path $flutterRoot 'android/app/src/main'
    $overlay = Join-Path $RepoRoot 'tools/build/overlays/android'
    $appPackage = Join-Path $androidMain 'kotlin/com/torca/app'
    $hostPackage = Join-Path $androidMain 'kotlin/com/torca/host'
    New-Item -ItemType Directory -Force -Path $appPackage | Out-Null
    New-Item -ItemType Directory -Force -Path $hostPackage | Out-Null
    Copy-Item (Join-Path $overlay 'MainActivity.kt') (Join-Path $appPackage 'MainActivity.kt') -Force
    Copy-Item (Join-Path $overlay 'AndroidKeystoreSecretStore.kt') (Join-Path $hostPackage 'AndroidKeystoreSecretStore.kt') -Force
    Copy-Item (Join-Path $overlay 'TorcaForegroundService.kt') (Join-Path $hostPackage 'TorcaForegroundService.kt') -Force
    Copy-Item (Join-Path $overlay 'AndroidManifest.xml') (Join-Path $androidMain 'AndroidManifest.xml') -Force

    $jni = Join-Path $androidMain 'jniLibs'
    foreach ($abi in @('arm64-v8a','x86_64')) {
        $source = Join-Path $RepoRoot "vendor/tor/android/$abi/libtor.so"
        if (-not (Test-Path $source)) { throw "Packaged Android Tor is missing: $source" }
        $target = Join-Path $jni $abi
        New-Item -ItemType Directory -Force -Path $target | Out-Null
        Copy-Item $source (Join-Path $target 'libtor.so') -Force
    }
    $assets = Join-Path $androidMain 'assets/torca'
    New-Item -ItemType Directory -Force -Path $assets | Out-Null
    Set-Content -Path (Join-Path $assets 'relay_endpoint.txt') -Value (Get-TorcaRelayEndpoint -RepoRoot $RepoRoot) -NoNewline -Encoding ascii
}

function Prepare-TorcaPlatformAssets {
    param(
        [Parameter(Mandatory = $true)][string]$RepoRoot,
        [Parameter(Mandatory = $true)][ValidateSet('windows','android')][string]$Platform
    )
    if ($Platform -eq 'windows') { Prepare-TorcaWindowsAssets -RepoRoot $RepoRoot }
    else { Prepare-TorcaAndroidAssets -RepoRoot $RepoRoot }
}

Export-ModuleMember -Function Prepare-TorcaPlatformAssets
