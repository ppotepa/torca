Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

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
    # Remove leftovers produced by older builds. Tor is embedded in torca-native;
    $obsoleteTor = Join-Path $runner 'tor'
    if (Test-Path -LiteralPath $obsoleteTor) {
        Remove-Item -LiteralPath $obsoleteTor -Recurse -Force
    }
    $obsoleteEndpoint = Join-Path $runner 'relay_endpoint.txt'
    if (Test-Path -LiteralPath $obsoleteEndpoint) {
        Remove-Item -LiteralPath $obsoleteEndpoint -Force
    }
    # Remove sidecars left by builds made before the relay endpoint became a
    # compile-time native setting. They are not part of the Windows package.
    foreach ($configuration in @('Debug', 'Release')) {
        $outputEndpoint = Join-Path $flutterRoot "build/windows/x64/runner/$configuration/relay_endpoint.txt"
        if (Test-Path -LiteralPath $outputEndpoint) {
            Remove-Item -LiteralPath $outputEndpoint -Force
        }
    }
    Copy-Item (Join-Path $overlay 'main.cpp') (Join-Path $runner 'main.cpp') -Force

    $cmake = Join-Path $flutterRoot 'windows/CMakeLists.txt'
    $runnerCmake = Join-Path $flutterRoot 'windows/runner/CMakeLists.txt'
    $text = Get-Content $cmake -Raw
    $begin = '# TORCA_RUNTIME_ASSETS_BEGIN'
    $end = '# TORCA_RUNTIME_ASSETS_END'
    $pattern = '(?s)\r?\n?# TORCA_RUNTIME_ASSETS_BEGIN.*?# TORCA_RUNTIME_ASSETS_END\r?\n?'
    $text = [regex]::Replace($text, $pattern, "`n")
    $block = @"
$begin
add_custom_command(TARGET `${BINARY_NAME} POST_BUILD
  COMMAND `${CMAKE_COMMAND} -E copy_if_different
    "`${CMAKE_CURRENT_SOURCE_DIR}/resources/app_icon.ico"
    "`$<TARGET_FILE_DIR:`${BINARY_NAME}>/torca.ico")
$end
"@
    Set-Content -Path $cmake -Value ($text.TrimEnd() + "`r`n") -Encoding utf8
    $runnerText = Get-Content $runnerCmake -Raw
    $runnerText = [regex]::Replace($runnerText, $pattern, "`n")
    Set-Content -Path $runnerCmake -Value ($runnerText.TrimEnd() + "`r`n`r`n" + $block) -Encoding utf8
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

    $obsoleteJni = Join-Path $androidMain 'jniLibs'
    if (Test-Path -LiteralPath $obsoleteJni) {
        Remove-Item -LiteralPath $obsoleteJni -Recurse -Force
    }
    $assets = Join-Path $androidMain 'assets/torca'
    $obsoleteTorAssets = Join-Path $assets 'tor'
    if (Test-Path -LiteralPath $obsoleteTorAssets) {
        Remove-Item -LiteralPath $obsoleteTorAssets -Recurse -Force
    }
    $obsoleteEndpoint = Join-Path $assets 'relay_endpoint.txt'
    if (Test-Path -LiteralPath $obsoleteEndpoint) {
        Remove-Item -LiteralPath $obsoleteEndpoint -Force
    }
    New-Item -ItemType Directory -Force -Path $assets | Out-Null
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
