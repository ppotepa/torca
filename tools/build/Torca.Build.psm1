Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$script:RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$script:FlutterRoot = Join-Path $script:RepoRoot 'apps/client/flutter'
$script:DartSchema = Join-Path $script:RepoRoot 'crates/platform/torca-bridge/schema/torca_contract.dart'
$script:CargoNdkVersion = '4.1.2'

function Invoke-TorcaExternal {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][scriptblock]$Command
    )

    Write-Host "==> $Name"
    & $Command
    if ($LASTEXITCODE -ne 0) {
        throw "$Name failed with exit code $LASTEXITCODE."
    }
}

function Get-TorcaTarget {
    param([string]$Target)

    if ($Target -ne 'auto') {
        return $Target
    }
    if ($env:OS -eq 'Windows_NT') {
        return 'windows'
    }
    return 'check'
}

function Assert-TorcaReleaseMetadata {
    $release = Get-Content (Join-Path $script:RepoRoot 'release/version.json') -Raw | ConvertFrom-Json
    $cargo = Get-Content (Join-Path $script:RepoRoot 'Cargo.toml') -Raw
    $pubspec = Get-Content (Join-Path $script:FlutterRoot 'pubspec.yaml') -Raw
    $bridge = Get-Content (Join-Path $script:RepoRoot 'crates/platform/torca-bridge/src/lib.rs') -Raw

    $cargoMatch = [Regex]::Match($cargo, '(?m)^\s*version\s*=\s*"([^"]+)"\s*$')
    if (-not $cargoMatch.Success -or $cargoMatch.Groups[1].Value -ne [string]$release.version) {
        throw 'Cargo workspace version does not match release/version.json.'
    }

    $flutterMatch = [Regex]::Match($pubspec, '(?m)^\s*version:\s*([^\s+]+)\+(\d+)\s*$')
    if (-not $flutterMatch.Success) {
        throw 'Flutter version/build could not be read from pubspec.yaml.'
    }
    if ($flutterMatch.Groups[1].Value -ne [string]$release.version -or
        [int]$flutterMatch.Groups[2].Value -ne [int]$release.build) {
        throw 'Flutter version/build does not match release/version.json.'
    }

    $contractMatch = [Regex]::Match($bridge, 'CONTRACT_VERSION\s*:\s*u16\s*=\s*(\d+)\s*;')
    if (-not $contractMatch.Success -or [int]$contractMatch.Groups[1].Value -ne [int]$release.contractVersion) {
        throw 'Rust bridge contract version does not match release/version.json.'
    }

    Write-Host "Release metadata consistent: $($release.version)+$($release.build)"
}

function Assert-TorcaArchitecture {
    $violations = @()
    $forbiddenDomainTokens = @(
        'rusqlite',
        'flutter',
        'std::net',
        'TcpStream',
        'DynamicLibrary',
        'torca_storage_sqlite',
        'torca_transport_tor'
    )

    Get-ChildItem (Join-Path $script:RepoRoot 'crates/domains') -Recurse -Filter '*.rs' | ForEach-Object {
        $content = Get-Content $_.FullName -Raw
        foreach ($token in $forbiddenDomainTokens) {
            if ($content.Contains($token)) {
                $violations += "$($_.FullName): forbidden domain token '$token'"
            }
        }
    }

    Get-ChildItem (Join-Path $script:RepoRoot 'crates') -Recurse -Filter '*.rs' |
        Where-Object { $_.FullName -notlike '*torca-storage-sqlite*' } |
        Where-Object {
            (Get-Content $_.FullName -Raw) -match '(?i)\b(SELECT|INSERT INTO|UPDATE\s+\w+\s+SET|CREATE TABLE|DELETE FROM)\b'
        } |
        ForEach-Object { $violations += "$($_.FullName): SQL text outside storage crate" }

    $main = Get-Content (Join-Path $script:FlutterRoot 'lib/main.dart') -Raw
    if (-not $main.Contains('defaultValue: false')) {
        $violations += 'Flutter release entrypoint must default TORCA_USE_MEMORY_GATEWAY to false'
    }
    if (-not $main.Contains('FfiEngineGateway')) {
        $violations += 'Flutter release entrypoint must use the shared FfiEngineGateway'
    }
    if ($main -match 'runApp\(TorcaApp\(gateway:\s*MemoryEngineGateway\(\)\)\)') {
        $violations += 'Flutter release entrypoint directly selects MemoryEngineGateway'
    }

    $nativeManifest = Get-Content (Join-Path $script:RepoRoot 'crates/platform/torca-native/Cargo.toml') -Raw
    if (-not $nativeManifest.Contains('crate-type = ["cdylib", "rlib"]')) {
        $violations += 'torca-native must build the shared dynamic client runtime'
    }

    if ($violations.Count -gt 0) {
        $violations | ForEach-Object { Write-Error $_ }
        throw "Architecture boundary check failed with $($violations.Count) violation(s)."
    }
    Write-Host 'Architecture boundary check passed.'
}

function Get-VersionPrefix {
    param([string]$Value, [string]$Name)

    $match = [Regex]::Match($Value, '(\d+)\.(\d+)\.(\d+)')
    if (-not $match.Success) {
        throw "Unable to parse $Name version from '$Value'."
    }
    return [version]$match.Value
}

function Assert-TorcaFlutterToolchain {
    $machineOutput = (& flutter --version --machine 2>&1 | Out-String).Trim()
    if ($LASTEXITCODE -ne 0) {
        throw 'Flutter is unavailable. Install Flutter stable 3.44.x or newer.'
    }
    try {
        $info = $machineOutput | ConvertFrom-Json
    } catch {
        throw 'Unable to parse `flutter --version --machine` output.'
    }

    $flutterText = [string]$info.frameworkVersion
    $dartText = [string]$info.dartSdkVersion
    $flutterVersion = Get-VersionPrefix -Value $flutterText -Name 'Flutter'
    $dartVersion = Get-VersionPrefix -Value $dartText -Name 'Dart'
    if ($flutterVersion -lt [version]'3.44.0' -or $dartVersion -lt [version]'3.12.0') {
        throw "Torca requires Flutter >= 3.44.0 and Dart >= 3.12.0; detected Flutter $flutterText / Dart $dartText."
    }
    Write-Host "Flutter toolchain compatible: Flutter $flutterText, Dart $dartText."
}

function Ensure-TorcaCargoLock {
    param([switch]$CI)

    & cargo metadata --format-version 1 --locked --no-deps *> $null
    if ($LASTEXITCODE -eq 0) {
        return
    }

    if ($CI) {
        Write-Host 'Cargo.lock is stale; resolving it in CI so the failure is explicit and reproducible.' -ForegroundColor Yellow
    } else {
        Write-Host 'Cargo.lock is stale; refreshing it automatically.' -ForegroundColor Yellow
    }
    Invoke-TorcaExternal 'Cargo lockfile refresh' { cargo generate-lockfile }
    Invoke-TorcaExternal 'Cargo locked metadata' { cargo metadata --format-version 1 --locked --no-deps | Out-Null }
}

function Invoke-TorcaFormatAndCodegen {
    param([switch]$CI)

    if ($CI) {
        Invoke-TorcaExternal 'Rust formatting' { cargo fmt --all -- --check }
        Push-Location $script:FlutterRoot
        try {
            Invoke-TorcaExternal 'Dart formatting' {
                dart format --output=none --set-exit-if-changed lib test $script:DartSchema
            }
        } finally {
            Pop-Location
        }
        Invoke-TorcaExternal 'Generated contract' {
            cargo run -p torca-contract-gen -- --check apps/client/flutter/lib/generated/torca_contract.dart
        }
        return
    }

    Invoke-TorcaExternal 'Rust formatting' { cargo fmt --all }
    Push-Location $script:FlutterRoot
    try {
        Invoke-TorcaExternal 'Dart formatting' { dart format lib test $script:DartSchema }
    } finally {
        Pop-Location
    }
    Invoke-TorcaExternal 'Generated contract' {
        cargo run -p torca-contract-gen -- apps/client/flutter/lib/generated/torca_contract.dart
    }
}

function Invoke-TorcaValidation {
    param([switch]$CI)

    Assert-TorcaReleaseMetadata
    Assert-TorcaArchitecture
    Assert-TorcaFlutterToolchain
    Ensure-TorcaCargoLock -CI:$CI
    Invoke-TorcaFormatAndCodegen -CI:$CI

    Invoke-TorcaExternal 'Rust check' { cargo check --workspace --all-targets --all-features --locked }
    Invoke-TorcaExternal 'Rust clippy' {
        cargo clippy --workspace --all-targets --all-features --locked -- -D clippy::correctness -D clippy::suspicious -D clippy::perf
    }
    Invoke-TorcaExternal 'Rust tests' { cargo test --workspace --all-targets --all-features --locked }

    Push-Location $script:FlutterRoot
    try {
        Invoke-TorcaExternal 'Flutter dependencies' { flutter pub get }
        Invoke-TorcaExternal 'Flutter analysis' { flutter analyze }
        Invoke-TorcaExternal 'Flutter tests' { flutter test }
    } finally {
        Pop-Location
    }
}

function Ensure-TorcaFlutterPlatform {
    param([ValidateSet('windows', 'android')][string]$Platform)

    $marker = if ($Platform -eq 'windows') {
        Join-Path $script:FlutterRoot 'windows/CMakeLists.txt'
    } else {
        Join-Path $script:FlutterRoot 'android/settings.gradle.kts'
    }

    if (-not (Test-Path $marker)) {
        Push-Location $script:FlutterRoot
        try {
            Invoke-TorcaExternal "Flutter $Platform scaffold" {
                flutter create --platforms=$Platform --org com.torca --project-name torca_app .
            }
        } finally {
            Pop-Location
        }
    }

    if ($Platform -eq 'android') {
        Apply-TorcaAndroidOverlay
    }
}

function Apply-TorcaAndroidOverlay {
    $overlayRoot = Join-Path $script:RepoRoot 'tools/build/overlays/android'
    $androidRoot = Join-Path $script:FlutterRoot 'android/app/src/main'

    New-Item -ItemType Directory -Force -Path $androidRoot | Out-Null
    Copy-Item (Join-Path $overlayRoot 'AndroidManifest.xml') (Join-Path $androidRoot 'AndroidManifest.xml') -Force

    $appPackage = Join-Path $androidRoot 'kotlin/com/torca/app'
    $hostPackage = Join-Path $androidRoot 'kotlin/com/torca/host'
    New-Item -ItemType Directory -Force -Path $appPackage | Out-Null
    New-Item -ItemType Directory -Force -Path $hostPackage | Out-Null
    Copy-Item (Join-Path $overlayRoot 'MainActivity.kt') (Join-Path $appPackage 'MainActivity.kt') -Force
    Copy-Item (Join-Path $overlayRoot 'AndroidKeystoreSecretStore.kt') (Join-Path $hostPackage 'AndroidKeystoreSecretStore.kt') -Force
}

function Ensure-TorcaAndroidNativeToolchain {
    & cargo ndk --version *> $null
    if ($LASTEXITCODE -ne 0) {
        Invoke-TorcaExternal "cargo-ndk $script:CargoNdkVersion" {
            cargo install cargo-ndk --version $script:CargoNdkVersion --locked
        }
    }
    Invoke-TorcaExternal 'Android Rust targets' {
        rustup target add aarch64-linux-android x86_64-linux-android
    }
}

function Build-TorcaNative {
    param(
        [ValidateSet('windows', 'android')][string]$Target,
        [ValidateSet('debug', 'release')][string]$Configuration
    )

    if ($Target -eq 'windows') {
        if ($env:OS -ne 'Windows_NT') {
            throw 'Windows client builds require a Windows host.'
        }
        if ($Configuration -eq 'release') {
            Invoke-TorcaExternal 'Rust native Windows release' { cargo build -p torca-native --release --locked }
        } else {
            Invoke-TorcaExternal 'Rust native Windows debug' { cargo build -p torca-native --locked }
        }
        return
    }

    Ensure-TorcaAndroidNativeToolchain
    Ensure-TorcaFlutterPlatform -Platform android
    $jniRoot = Join-Path $script:FlutterRoot 'android/app/src/main/jniLibs'
    New-Item -ItemType Directory -Force -Path $jniRoot | Out-Null
    $arguments = @(
        'ndk', '-P', '23',
        '-t', 'arm64-v8a',
        '-t', 'x86_64',
        '-o', $jniRoot,
        'build', '-p', 'torca-native', '--locked'
    )
    if ($Configuration -eq 'release') {
        $arguments += '--release'
    }
    Invoke-TorcaExternal "Rust native Android $Configuration" { cargo @arguments }
}

function Build-TorcaFlutterTarget {
    param(
        [ValidateSet('windows', 'android')][string]$Target,
        [ValidateSet('debug', 'release')][string]$Configuration
    )

    Ensure-TorcaFlutterPlatform -Platform $Target
    Build-TorcaNative -Target $Target -Configuration $Configuration

    Push-Location $script:FlutterRoot
    try {
        if ($Target -eq 'windows') {
            Invoke-TorcaExternal "Flutter Windows $Configuration" { flutter build windows --$Configuration }
            $rustDll = Join-Path $script:RepoRoot "target/$Configuration/torca_bridge.dll"
            $runnerDir = Join-Path $script:FlutterRoot "build/windows/x64/runner/$([cultureinfo]::InvariantCulture.TextInfo.ToTitleCase($Configuration))"
            if (-not (Test-Path $rustDll)) {
                throw "Native Windows library missing: $rustDll"
            }
            Copy-Item $rustDll (Join-Path $runnerDir 'torca_bridge.dll') -Force
        } else {
            Invoke-TorcaExternal "Flutter Android $Configuration" { flutter build apk --$Configuration }
        }
    } finally {
        Pop-Location
    }
}

function Invoke-TorcaBuild {
    [CmdletBinding()]
    param(
        [ValidateSet('auto', 'check', 'windows', 'android', 'all')][string]$Target = 'auto',
        [ValidateSet('debug', 'release')][string]$Configuration = 'debug',
        [switch]$CI
    )

    Push-Location $script:RepoRoot
    try {
        $resolvedTarget = Get-TorcaTarget $Target
        Invoke-TorcaValidation -CI:$CI

        if ($resolvedTarget -eq 'check') {
            Write-Host 'Build validation completed successfully.'
            return
        }
        if ($resolvedTarget -in @('windows', 'all')) {
            Build-TorcaFlutterTarget -Target windows -Configuration $Configuration
        }
        if ($resolvedTarget -in @('android', 'all')) {
            Build-TorcaFlutterTarget -Target android -Configuration $Configuration
        }
        Write-Host "Build completed successfully: $resolvedTarget / $Configuration"
    } finally {
        Pop-Location
    }
}

function Invoke-TorcaRun {
    [CmdletBinding()]
    param(
        [ValidateSet('auto', 'windows', 'android')][string]$Target = 'auto',
        [string]$Device
    )

    Push-Location $script:RepoRoot
    try {
        $resolvedTarget = Get-TorcaTarget $Target
        if ($resolvedTarget -eq 'check') {
            $resolvedTarget = 'android'
        }

        Assert-TorcaReleaseMetadata
        Assert-TorcaArchitecture
        Assert-TorcaFlutterToolchain
        Ensure-TorcaCargoLock
        Invoke-TorcaFormatAndCodegen
        Push-Location $script:FlutterRoot
        try {
            Invoke-TorcaExternal 'Flutter dependencies' { flutter pub get }
        } finally {
            Pop-Location
        }

        Ensure-TorcaFlutterPlatform -Platform $resolvedTarget
        Build-TorcaNative -Target $resolvedTarget -Configuration debug

        Push-Location $script:FlutterRoot
        try {
            if ($resolvedTarget -eq 'windows') {
                $nativeDirectory = Join-Path $script:RepoRoot 'target/debug'
                $oldPath = $env:PATH
                $env:PATH = "$nativeDirectory;$oldPath"
                try {
                    Invoke-TorcaExternal 'Run Torca on Windows' { flutter run -d windows }
                } finally {
                    $env:PATH = $oldPath
                }
            } else {
                if ($Device) {
                    Invoke-TorcaExternal "Run Torca on $Device" { flutter run -d $Device }
                } else {
                    Invoke-TorcaExternal 'Run Torca on Android' { flutter run }
                }
            }
        } finally {
            Pop-Location
        }
    } finally {
        Pop-Location
    }
}

function Write-TorcaChecksums {
    param([string]$Root)

    $checksumFile = Join-Path $Root 'SHA256SUMS.txt'
    Get-ChildItem $Root -Recurse -File |
        Where-Object { $_.FullName -ne $checksumFile } |
        Sort-Object FullName |
        ForEach-Object {
            $hash = Get-FileHash $_.FullName -Algorithm SHA256 | Select-Object -ExpandProperty Hash
            $relative = $_.FullName.Substring($Root.Length + 1).Replace('\', '/')
            "$hash  $relative"
        } | Set-Content $checksumFile
}

function Invoke-TorcaDeploy {
    [CmdletBinding()]
    param(
        [ValidateSet('auto', 'windows', 'android', 'all')][string]$Target = 'auto',
        [string]$Device
    )

    Push-Location $script:RepoRoot
    try {
        $resolvedTarget = Get-TorcaTarget $Target
        if ($resolvedTarget -eq 'check') {
            throw 'Deploy target must be windows, android or all.'
        }

        Invoke-TorcaBuild -Target $resolvedTarget -Configuration release

        $release = Get-Content (Join-Path $script:RepoRoot 'release/version.json') -Raw | ConvertFrom-Json
        $artifactRoot = Join-Path $script:RepoRoot "artifacts/$($release.version)-$($release.build)"
        if (Test-Path $artifactRoot) {
            Remove-Item $artifactRoot -Recurse -Force
        }
        New-Item -ItemType Directory -Force -Path $artifactRoot | Out-Null

        if ($resolvedTarget -in @('windows', 'all')) {
            $windowsSource = Join-Path $script:FlutterRoot 'build/windows/x64/runner/Release'
            $windowsTarget = Join-Path $artifactRoot 'windows'
            Copy-Item $windowsSource $windowsTarget -Recurse -Force
            Compress-Archive -Path (Join-Path $windowsTarget '*') -DestinationPath (Join-Path $artifactRoot 'Torca-windows-x64.zip') -Force
        }

        if ($resolvedTarget -in @('android', 'all')) {
            Push-Location $script:FlutterRoot
            try {
                Invoke-TorcaExternal 'Android split APKs' { flutter build apk --release --split-per-abi }
                Invoke-TorcaExternal 'Android app bundle' { flutter build appbundle --release }
            } finally {
                Pop-Location
            }
            $androidTarget = Join-Path $artifactRoot 'android'
            New-Item -ItemType Directory -Force -Path $androidTarget | Out-Null
            Get-ChildItem (Join-Path $script:FlutterRoot 'build/app/outputs/flutter-apk') -Filter '*-release.apk' |
                ForEach-Object { Copy-Item $_.FullName (Join-Path $androidTarget $_.Name) -Force }
            $bundle = Join-Path $script:FlutterRoot 'build/app/outputs/bundle/release/app-release.aab'
            if (Test-Path $bundle) {
                Copy-Item $bundle (Join-Path $androidTarget 'Torca.aab') -Force
            }
            if ($Device) {
                Push-Location $script:FlutterRoot
                try {
                    Invoke-TorcaExternal "Install Torca on $Device" { flutter install -d $Device }
                } finally {
                    Pop-Location
                }
            }
        }

        Write-TorcaChecksums -Root $artifactRoot
        Write-Host "Artifacts: $artifactRoot"
    } finally {
        Pop-Location
    }
}

Export-ModuleMember -Function Invoke-TorcaBuild, Invoke-TorcaRun, Invoke-TorcaDeploy
