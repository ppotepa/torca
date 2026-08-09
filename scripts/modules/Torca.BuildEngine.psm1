Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$script:RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$script:FlutterRoot = Join-Path $script:RepoRoot 'apps/client/flutter'
$script:DartSchema = Join-Path $script:RepoRoot 'crates/platform/torca-contract/schema/torca_contract.dart'
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
    $contract = Get-Content (Join-Path $script:RepoRoot 'crates/platform/torca-contract/src/lib.rs') -Raw

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

    $contractMatch = [Regex]::Match($contract, 'CONTRACT_VERSION\s*:\s*u16\s*=\s*(\d+)\s*;')
    if (-not $contractMatch.Success -or [int]$contractMatch.Groups[1].Value -ne [int]$release.contractSchema) {
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
        'torca_storage_sqlite'
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
            # The SQL policy applies to executable Rust/string literals, not prose.
            # In particular, `tokio::select!` and explanatory comments must not be
            # mistaken for a SQL SELECT statement.
            $source = Get-Content $_.FullName -Raw
            $source = [regex]::Replace($source, '(?s)/\*.*?\*/', '')
            $source = [regex]::Replace($source, '(?m)//.*$', '')
            $source -match '(?i)\b(SELECT\s+|INSERT\s+INTO\s+|UPDATE\s+\w+\s+SET\s+|CREATE\s+TABLE\s+|DELETE\s+FROM\s+)'
        } |
        ForEach-Object { $violations += "$($_.FullName): SQL text outside storage crate" }

    $main = Get-Content (Join-Path $script:FlutterRoot 'lib/main.dart') -Raw
    if (-not $main.Contains('FfiEngineGateway')) {
        $violations += 'Flutter release entrypoint must use the shared FfiEngineGateway'
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

    $oldRustFlags = $env:RUSTFLAGS
    try {
        $rustFlags = if ($oldRustFlags) { $oldRustFlags } else { '' }
        if ($env:OS -eq 'Windows_NT') { $rustFlags = "$rustFlags -C link-arg=/IGNORE:4099" }
        $env:RUSTFLAGS = "$rustFlags -A warnings".Trim()
        Invoke-TorcaExternal 'Rust check' { cargo check --workspace --all-targets --all-features --locked }
        Invoke-TorcaExternal 'Rust clippy' {
            cargo clippy --workspace --all-targets --all-features --locked -- -A warnings -A clippy::all -A clippy::pedantic -D clippy::correctness -D clippy::suspicious -D clippy::perf
        }
        Invoke-TorcaExternal 'Rust tests' { cargo test --workspace --all-targets --all-features --locked }
    } finally {
        $env:RUSTFLAGS = $oldRustFlags
    }

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
        $gradleProperties = Join-Path $script:FlutterRoot 'android/gradle.properties'
        $properties = if (Test-Path $gradleProperties) { Get-Content $gradleProperties } else { @() }
        if (-not ($properties -match '^kotlin\.incremental=false$')) {
            Add-Content -LiteralPath $gradleProperties -Value 'kotlin.incremental=false' -Encoding ascii
        }
    }
}

function Assert-TorcaNativeAbi {
    param(
        [Parameter(Mandatory = $true)][string]$Library,
        [ValidateSet('windows', 'android')][string]$Platform
    )

    if (-not (Test-Path -LiteralPath $Library)) {
        throw "Native bridge library is missing: $Library"
    }
    $tool = Get-Command llvm-objdump -ErrorAction SilentlyContinue
    if (-not $tool) {
        throw 'llvm-objdump is required to validate the Torca native ABI.'
    }
    $arguments = if ($Platform -eq 'windows') { @('-p', $Library) } else { @('-T', $Library) }
    $exports = (& $tool.Source @arguments | Out-String)
    if ($LASTEXITCODE -ne 0) {
        throw "Unable to inspect native bridge exports: $Library"
    }
    $ffiSource = Get-Content (Join-Path $script:FlutterRoot 'lib/gateway/ffi_engine_gateway.dart') -Raw
    $required = @([Regex]::Matches($ffiSource, "'((?:torca)_[a-z0-9_]+)'") |
        ForEach-Object { $_.Groups[1].Value } | Sort-Object -Unique)
    $missing = @($required | Where-Object { -not $exports.Contains($_) })
    if ($missing.Count -gt 0) {
        throw "Native bridge ABI is stale or incompatible. Missing export(s): $($missing -join ', '). Rebuild before deploy."
    }
    Write-Host "Native $Platform ABI verified: $([IO.Path]::GetFileName($Library))"
}

function Assert-TorcaAndroidPackage {
    param(
        [Parameter(Mandatory = $true)][string]$Apk
    )
    if (-not (Test-Path -LiteralPath $Apk)) { throw "Android package is missing: $Apk" }
    $unpackRoot = Join-Path ([IO.Path]::GetTempPath()) ("torca-apk-check-" + [Guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Force -Path $unpackRoot | Out-Null
    try {
        $archiver = Get-Command 7z -ErrorAction SilentlyContinue
        if (-not $archiver) { throw '7z is required to inspect Android APK contents.' }
        & $archiver.Source x '-y' ("-o$unpackRoot") $Apk | Out-Null
        if ($LASTEXITCODE -ne 0) { throw "Unable to unpack Android package: $Apk" }
        $forbiddenNativeNames = @('torca_' + 'bridge.dll', 'torca_' + 'contract.dll', 'libtorca_' + 'bridge.so', 'libtorca_' + 'contract.so')
        $obsoleteTorFiles = @(Get-ChildItem $unpackRoot -Recurse -File |
            Where-Object { $_.Name -match '^(tor\.exe|libtor\.so|torrc(?:-defaults)?)$' -or $forbiddenNativeNames -contains $_.Name })
        if ($obsoleteTorFiles.Count -gt 0) {
            throw "Android package contains forbidden external Tor assets: $($obsoleteTorFiles.FullName -join ', ')"
        }
        $libraries = @(Get-ChildItem $unpackRoot -Recurse -Filter 'libtorca_native.so' -File)
        if ($libraries.Count -eq 0) { throw "Android package contains no libtorca_native.so: $Apk" }
        foreach ($library in $libraries) {
            Assert-TorcaNativeAbi -Library $library.FullName -Platform android
            $flutterLibrary = Join-Path $library.DirectoryName 'libflutter.so'
            if (-not (Test-Path -LiteralPath $flutterLibrary)) {
                throw "Android package is missing libflutter.so beside $($library.FullName): $Apk"
            }
        }
        Write-Host "Android package ABI verified: $([IO.Path]::GetFileName($Apk))"
    } finally {
        if (Test-Path -LiteralPath $unpackRoot) { Remove-Item -LiteralPath $unpackRoot -Recurse -Force }
    }
}

function Get-TorcaAndroidDeviceAbis {
    param([Parameter(Mandatory = $true)][string]$Device)

    if (-not (Get-Command adb -ErrorAction SilentlyContinue)) {
        throw 'ADB is required to select an ABI-specific Android APK.'
    }

    $observations = [System.Collections.Generic.List[string]]::new()
    foreach ($attempt in 1..3) {
        $output = (& adb -s $Device shell getprop ro.product.cpu.abilist 2>&1 | Out-String).Trim()
        $exitCode = $LASTEXITCODE
        $observations.Add("attempt=$attempt exit=$exitCode abilist='$output'")
        if ($exitCode -eq 0) {
            $abis = @($output -split ',' | ForEach-Object { $_.Trim() } | Where-Object { $_ })
            if ($abis.Count -gt 0) { return $abis }
        }
        if ($attempt -lt 3) { Start-Sleep -Seconds 1 }
    }

    $primary = (& adb -s $Device shell getprop ro.product.cpu.abi 2>&1 | Out-String).Trim()
    $primaryExitCode = $LASTEXITCODE
    $observations.Add("fallback exit=$primaryExitCode abi='$primary'")
    if ($primaryExitCode -eq 0 -and -not [string]::IsNullOrWhiteSpace($primary)) {
        return @($primary)
    }

    throw "Could not determine Android ABIs for $Device. ADB observations: $($observations -join '; ')"
}

function Select-TorcaAndroidApk {
    param(
        [Parameter(Mandatory = $true)][string]$OutputRoot,
        [Parameter(Mandatory = $true)][string]$UniversalApk,
        [Parameter(Mandatory = $true)][string]$Device
    )

    $abis = @(Get-TorcaAndroidDeviceAbis -Device $Device)
    Write-Verbose "Android ABIs for ${Device}: $($abis -join ', ')"
    $candidates = foreach ($abi in $abis) {
        switch ($abi.Trim()) {
            'arm64-v8a' { Join-Path $OutputRoot 'app-arm64-v8a-release.apk' }
            'x86_64' { Join-Path $OutputRoot 'app-x86_64-release.apk' }
        }
    }
    foreach ($candidate in $candidates) {
        if (Test-Path -LiteralPath $candidate) {
            Write-Host "Selected Android APK for $Device`: $([IO.Path]::GetFileName($candidate))"
            return $candidate
        }
    }
    if (-not (Test-Path -LiteralPath $UniversalApk)) {
        $available = @(Get-ChildItem -LiteralPath $OutputRoot -Filter 'app-*-release.apk' -File -ErrorAction SilentlyContinue |
            ForEach-Object Name) -join ', '
        throw "No compatible Android APK found for $Device. Device ABIs=$($abis -join ', '); available APKs=$available"
    }
    Write-Host "No ABI-specific APK found for $Device; using validated universal APK."
    return $UniversalApk
}

function Assert-TorcaWindowsPackage {
    param([Parameter(Mandatory = $true)][string]$Root)

    if (-not (Test-Path -LiteralPath $Root)) { throw "Windows package is missing: $Root" }
    $forbiddenNativeNames = @('torca_' + 'bridge.dll', 'torca_' + 'contract.dll', 'libtorca_' + 'bridge.so', 'libtorca_' + 'contract.so')
    $obsoleteTorFiles = @(Get-ChildItem $Root -Recurse -File |
        Where-Object { $_.Name -match '^(tor\.exe|libtor\.so|torrc(?:-defaults)?)$' -or $forbiddenNativeNames -contains $_.Name })
    if ($obsoleteTorFiles.Count -gt 0) {
        throw "Windows package contains forbidden external Tor assets: $($obsoleteTorFiles.FullName -join ', ')"
    }
    Write-Host "Windows package contains no external Tor runtime: $Root"
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
    Copy-Item (Join-Path $overlayRoot 'TorcaForegroundService.kt') (Join-Path $hostPackage 'TorcaForegroundService.kt') -Force
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
        $oldRustFlags = $env:RUSTFLAGS
        $oldPath = $env:PATH
        try {
            # openssl-src invokes Perl during the Windows OpenSSL build. Never
            # let MSYS/Cygwin Perl win PATH resolution: it emits POSIX paths
            # and cannot configure OpenSSL with Windows build directories.
            $nativePerl = 'C:\Strawberry\perl\bin'
            $nativePerlC = 'C:\Strawberry\c\bin'
            $pathParts = @($oldPath -split ';' | Where-Object { $_ -and $_ -notmatch '(?i)(msys|cygwin|Git\\usr\\bin)' })
            if (-not (Test-Path (Join-Path $nativePerl 'perl.exe'))) {
                throw "Native Strawberry Perl was not found: $(Join-Path $nativePerl 'perl.exe')"
            }
            $env:PATH = ((@($nativePerl, $nativePerlC) + $pathParts) -join ';')
            $rustFlags = if ($oldRustFlags) { $oldRustFlags } else { '' }
            $env:RUSTFLAGS = "$rustFlags -C link-arg=/IGNORE:4099".Trim()
            if ($Configuration -eq 'release') {
                Invoke-TorcaExternal 'Rust native Windows release' { cargo build -p torca-native --release --locked }
            } else {
                Invoke-TorcaExternal 'Rust native Windows debug' { cargo build -p torca-native --locked }
            }
        } finally {
            $env:RUSTFLAGS = $oldRustFlags
            $env:PATH = $oldPath
        }
        return
    }

    Ensure-TorcaAndroidNativeToolchain
    Ensure-TorcaFlutterPlatform -Platform android
    $jniRoot = Join-Path $script:FlutterRoot 'android/app/src/main/jniLibs'
    New-Item -ItemType Directory -Force -Path $jniRoot | Out-Null
    $oldPath = $env:PATH
    $msysPerlRoot = 'C:\msys64\usr\bin'
    $ndkHome = [string]$env:ANDROID_NDK_HOME
    if ([string]::IsNullOrWhiteSpace($ndkHome)) {
        $sdkRoot = [string]$env:ANDROID_HOME
        if ([string]::IsNullOrWhiteSpace($sdkRoot)) { $sdkRoot = [string]$env:ANDROID_SDK_ROOT }
        if ([string]::IsNullOrWhiteSpace($sdkRoot)) {
            $adb = Get-Command adb -ErrorAction SilentlyContinue
            if ($adb) { $sdkRoot = Split-Path (Split-Path $adb.Source -Parent) -Parent }
        }
        if (-not [string]::IsNullOrWhiteSpace($sdkRoot)) {
            $ndkHome = (Get-ChildItem (Join-Path $sdkRoot 'ndk') -Directory -ErrorAction SilentlyContinue | Sort-Object Name -Descending | Select-Object -First 1).FullName
        }
    }
    if ([string]::IsNullOrWhiteSpace($ndkHome)) { throw 'Android NDK location is unavailable.' }
    $ndkBin = Join-Path $ndkHome 'toolchains/llvm/prebuilt/windows-x86_64/bin'
    if (-not (Test-Path (Join-Path $ndkBin 'clang.exe'))) {
        throw "Android NDK clang was not found: $ndkBin"
    }
    $env:PATH = "$ndkBin;$msysPerlRoot;$oldPath"
    # MSYS make interprets absolute Windows compiler paths as escape sequences.
    # Build each ABI directly so the compiler is resolved by name from PATH.
    Set-Item -Path 'Env:CC_aarch64-linux-android' -Value 'clang'
    Set-Item -Path 'Env:CC_x86_64-linux-android' -Value 'clang'
    Set-Item -Path 'Env:AR_aarch64-linux-android' -Value 'llvm-ar'
    Set-Item -Path 'Env:AR_x86_64-linux-android' -Value 'llvm-ar'
    try {
        foreach ($androidTarget in @(
            [pscustomobject]@{ Triple = 'aarch64-linux-android'; Abi = 'arm64-v8a'; Linker = (Join-Path $ndkBin 'aarch64-linux-android23-clang.cmd') },
            [pscustomobject]@{ Triple = 'x86_64-linux-android'; Abi = 'x86_64'; Linker = (Join-Path $ndkBin 'x86_64-linux-android23-clang.cmd') }
        )) {
            $linkerVariable = 'CARGO_TARGET_' + $androidTarget.Triple.Replace('-', '_').ToUpperInvariant() + '_LINKER'
            Set-Item -Path ("Env:$linkerVariable") -Value $androidTarget.Linker
            $cflagsVariable = 'CFLAGS_' + $androidTarget.Triple
            Set-Item -Path ("Env:$cflagsVariable") -Value ('--target=' + $androidTarget.Triple + '23')
            $cargoArguments = @('build', '-p', 'torca-native', '--target', $androidTarget.Triple, '--locked')
            if ($Configuration -eq 'release') { $cargoArguments += '--release' }
            Invoke-TorcaExternal "Rust native Android $Configuration $($androidTarget.Abi)" { cargo @cargoArguments }
            $profile = if ($Configuration -eq 'release') { 'release' } else { 'debug' }
            $source = Join-Path $script:RepoRoot "target/$($androidTarget.Triple)/$profile/libtorca_native.so"
            $destination = Join-Path $jniRoot "$($androidTarget.Abi)/libtorca_native.so"
            if (-not (Test-Path $source)) { throw "Android native library missing: $source" }
            New-Item -ItemType Directory -Force -Path (Split-Path $destination) | Out-Null
            Copy-Item $source $destination -Force
            Assert-TorcaNativeAbi -Library $destination -Platform android
        }
    } finally {
        $env:PATH = $oldPath
    }
}

function Stop-TorcaWindowsClientForBuild {
    $running = @(Get-Process -Name torca_app -ErrorAction SilentlyContinue)
    if ($running.Count -eq 0) { return }

    Write-Warning 'A running Torca Windows client is locking the native DLL; closing it before the build.'
    foreach ($process in $running) {
        if ($process.MainWindowHandle -ne 0) {
            [void]$process.CloseMainWindow()
        }
    }
    $deadline = [DateTime]::UtcNow.AddSeconds(5)
    do {
        Start-Sleep -Milliseconds 250
        $remaining = @(Get-Process -Name torca_app -ErrorAction SilentlyContinue)
    } while ($remaining.Count -gt 0 -and [DateTime]::UtcNow -lt $deadline)

    if ($remaining.Count -gt 0) {
        Write-Warning 'Torca Windows client did not close gracefully; terminating it so the native DLL can be replaced.'
        foreach ($process in $remaining) {
            Stop-Process -Id $process.Id -Force -ErrorAction Stop
        }
        $stopDeadline = [DateTime]::UtcNow.AddSeconds(5)
        do {
            Start-Sleep -Milliseconds 250
            $remaining = @(Get-Process -Name torca_app -ErrorAction SilentlyContinue)
        } while ($remaining.Count -gt 0 -and [DateTime]::UtcNow -lt $stopDeadline)
    if ($remaining.Count -gt 0) {
        throw 'Torca Windows client could not be stopped; release DLL is still locked.'
    }
}
}

function Build-TorcaFlutterTarget {
    param(
        [ValidateSet('windows', 'android')][string]$Target,
        [ValidateSet('debug', 'release')][string]$Configuration
    )

    if ($Target -eq 'windows') {
        Stop-TorcaWindowsClientForBuild
    }
    Ensure-TorcaFlutterPlatform -Platform $Target
    Build-TorcaNative -Target $Target -Configuration $Configuration

    Push-Location $script:FlutterRoot
    try {
        $dartDefine = @()
        if (-not [string]::IsNullOrWhiteSpace($env:TORCA_BUILD_ID)) {
            $dartDefine = @("--dart-define=TORCA_BUILD_ID=$($env:TORCA_BUILD_ID)")
        }
        if ($Target -eq 'windows') {
            Invoke-TorcaExternal "Flutter Windows $Configuration" { flutter build windows --$Configuration @dartDefine }
            $rustDll = Join-Path $script:RepoRoot "target/$Configuration/torca_native.dll"
            $runnerDir = Join-Path $script:FlutterRoot "build/windows/x64/runner/$([cultureinfo]::InvariantCulture.TextInfo.ToTitleCase($Configuration))"
            if (-not (Test-Path $rustDll)) {
                throw "Native Windows library missing: $rustDll"
            }
            Assert-TorcaNativeAbi -Library $rustDll -Platform windows
            Copy-Item $rustDll (Join-Path $runnerDir 'torca_native.dll') -Force
            Assert-TorcaNativeAbi -Library (Join-Path $runnerDir 'torca_native.dll') -Platform windows
            $forbiddenNativeNames = @('torca_' + 'bridge.dll', 'torca_' + 'contract.dll')
            Get-ChildItem $runnerDir -Recurse -File |
                Where-Object { $forbiddenNativeNames -contains $_.Name } |
                Remove-Item -Force -ErrorAction SilentlyContinue
            Assert-TorcaWindowsPackage -Root $runnerDir
        } else {
            Invoke-TorcaExternal "Flutter Android $Configuration ABI packages" { flutter build apk --$Configuration --split-per-abi @dartDefine }
            $apkOutput = Join-Path $script:FlutterRoot 'build/app/outputs/flutter-apk'
            $releaseApks = @(Get-ChildItem $apkOutput -Filter "*-$Configuration.apk" -File -ErrorAction SilentlyContinue |
                Where-Object { $_.Name -in @("app-arm64-v8a-$Configuration.apk", "app-x86_64-$Configuration.apk") })
            if ($releaseApks.Count -eq 0) { throw "Flutter produced no Android ABI APKs in $apkOutput" }
            foreach ($apk in $releaseApks) { Assert-TorcaAndroidPackage -Apk $apk.FullName }
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
            $sha = [System.Security.Cryptography.SHA256]::Create()
            try {
                $hash = [BitConverter]::ToString($sha.ComputeHash([System.IO.File]::ReadAllBytes($_.FullName))).Replace('-', '')
            } finally { $sha.Dispose() }
            $relative = $_.FullName.Substring($Root.Length + 1).Replace('\', '/')
            "$hash  $relative"
        } | Set-Content $checksumFile
}

function Write-TorcaArtifactManifest {
    param([string]$Root)

    $buildManifestPath = Join-Path $Root 'torca-build.json'
    $buildManifest = if (Test-Path -LiteralPath $buildManifestPath) {
        Get-Content $buildManifestPath -Raw | ConvertFrom-Json
    } else { $null }
    $files = @(Get-ChildItem $Root -Recurse -File |
        Where-Object { $_.Name -notin @('torca-artifact.json', 'SHA256SUMS.txt') } |
        Sort-Object FullName |
        ForEach-Object {
            $relative = $_.FullName.Substring($Root.Length + 1).Replace('\', '/')
            $sha = [System.Security.Cryptography.SHA256]::Create()
            try {
                $hash = [BitConverter]::ToString(
                    $sha.ComputeHash([System.IO.File]::ReadAllBytes($_.FullName))
                ).Replace('-', '')
            } finally {
                $sha.Dispose()
            }
            [pscustomobject]@{
                Path = $relative
                Size = $_.Length
                Sha256 = $hash
            }
        })
    $nativeFiles = @($files | Where-Object { $_.Path -match '(^|/)torca_native\.(dll|so)$' })
    if ($nativeFiles.Count -eq 0) {
        throw "Artifact does not contain the canonical torca_native library: $Root"
    }
    $nativeLibraryHash = ($nativeFiles | ForEach-Object { $_.Sha256.ToLowerInvariant() }) -join ','
    $artifactInput = ($files | ForEach-Object { "$($_.Path)|$($_.Sha256.ToLowerInvariant())" }) -join "`n"
    $artifactSha = [System.Security.Cryptography.SHA256]::Create()
    try {
        $applicationArtifactHash = [BitConverter]::ToString(
            $artifactSha.ComputeHash([Text.Encoding]::UTF8.GetBytes($artifactInput))
        ).Replace('-', '').ToLowerInvariant()
    } finally { $artifactSha.Dispose() }
    $release = Get-Content (Join-Path $script:RepoRoot 'release/version.json') -Raw | ConvertFrom-Json
    $artifactEndpoint = if ($buildManifest -and $buildManifest.PSObject.Properties.Name -contains 'Endpoint') {
        [string]$buildManifest.Endpoint
    } else { '' }
    $relayEndpointHash = [string]$release.relayEndpointHash
    if (-not [string]::IsNullOrWhiteSpace($artifactEndpoint)) {
        $endpointSha = [System.Security.Cryptography.SHA256]::Create()
        try {
            $relayEndpointHash = [BitConverter]::ToString(
                $endpointSha.ComputeHash([Text.Encoding]::UTF8.GetBytes($artifactEndpoint))
            ).Replace('-', '').ToLowerInvariant()
        } finally { $endpointSha.Dispose() }
    }
    [pscustomobject]@{
        Schema = 1
        Product = 'Torca'
        Version = $release.version
        BuildNumber = $release.build
        BuildId = if ($buildManifest -and $buildManifest.PSObject.Properties.Name -contains 'BuildId') { $buildManifest.BuildId } else { 'unknown' }
        ContractSchema = if ($buildManifest -and $buildManifest.PSObject.Properties.Name -contains 'ContractSchema') { $buildManifest.ContractSchema } else { $null }
        WireVersion = $release.wireVersion
        NativeAbi = $release.nativeAbi
        StorageEpoch = $release.storageEpoch
        SchemaVersion = $release.schemaVersion
        SourceCommit = if ($buildManifest -and $buildManifest.PSObject.Properties.Name -contains 'Commit') { $buildManifest.Commit } else { $release.sourceCommit }
        SourceFingerprint = if ($buildManifest -and $buildManifest.PSObject.Properties.Name -contains 'SourceFingerprint') { $buildManifest.SourceFingerprint } else { $release.sourceFingerprint }
        RelayEndpointHash = $relayEndpointHash
        NativeLibraryHash = $nativeLibraryHash
        ApplicationArtifactHash = $applicationArtifactHash
        TargetPlatform = if ($buildManifest -and $buildManifest.PSObject.Properties.Name -contains 'Targets') { (@($buildManifest.Targets) -join '|') } else { $release.targetPlatform }
        TargetArchitecture = $release.targetArchitecture
        Endpoint = if (-not [string]::IsNullOrWhiteSpace($artifactEndpoint)) { $artifactEndpoint } else { $null }
        Configuration = if ($buildManifest -and $buildManifest.PSObject.Properties.Name -contains 'Configuration') { $buildManifest.Configuration } else { $null }
        Files = $files
    } | ConvertTo-Json -Depth 6 | Set-Content (Join-Path $Root 'torca-artifact.json')
}

function Get-TorcaBuildManifestPathForTarget {
    param([string]$Target, [string]$Configuration)
    $manifestRoot = Join-Path $script:RepoRoot '.torca/manifests'
    return Join-Path $manifestRoot "$($Target.ToLowerInvariant())-$($Configuration.ToLowerInvariant()).json"
}

function Invoke-TorcaDeploy {
    [CmdletBinding()]
    param(
        [ValidateSet('auto', 'windows', 'android', 'all')][string]$Target = 'auto',
        [string]$Device,
        [switch]$ReuseBuild
    )

    Push-Location $script:RepoRoot
    try {
        $resolvedTarget = Get-TorcaTarget $Target
        if ($resolvedTarget -eq 'check') {
            throw 'Deploy target must be windows, android or all.'
        }

        if (-not $ReuseBuild) {
            Invoke-TorcaBuild -Target $resolvedTarget -Configuration release
        } else {
            Write-Host 'Reusing existing release build.'
        }

        $release = Get-Content (Join-Path $script:RepoRoot 'release/version.json') -Raw | ConvertFrom-Json
        $artifactRoot = Join-Path $script:RepoRoot "artifacts/$($release.version)-$($release.build)"
        if (Test-Path $artifactRoot) {
            Remove-Item $artifactRoot -Recurse -Force
        }
        New-Item -ItemType Directory -Force -Path $artifactRoot | Out-Null
        $buildManifest = Get-TorcaBuildManifestPathForTarget -Target $resolvedTarget -Configuration 'release'
        if (Test-Path -LiteralPath $buildManifest) {
            Copy-Item $buildManifest (Join-Path $artifactRoot 'torca-build.json') -Force
        }

        if ($resolvedTarget -in @('windows', 'all')) {
            $windowsSource = Join-Path $script:FlutterRoot 'build/windows/x64/runner/Release'
            $windowsTarget = Join-Path $artifactRoot 'windows'
            Copy-Item $windowsSource $windowsTarget -Recurse -Force
            Assert-TorcaWindowsPackage -Root $windowsTarget
            if (Test-Path -LiteralPath $buildManifest) {
                Copy-Item $buildManifest (Join-Path $windowsTarget 'torca-build.json') -Force
            }
            Compress-Archive -Path (Join-Path $windowsTarget '*') -DestinationPath (Join-Path $artifactRoot 'Torca-windows-x64.zip') -Force
        }

        if ($resolvedTarget -in @('android', 'all')) {
            $androidTarget = Join-Path $artifactRoot 'android'
            New-Item -ItemType Directory -Force -Path $androidTarget | Out-Null
            if (Test-Path -LiteralPath $buildManifest) {
                Copy-Item $buildManifest (Join-Path $androidTarget 'torca-build.json') -Force
            }
            $apkOutput = Join-Path $script:FlutterRoot 'build/app/outputs/flutter-apk'
            $universalApk = Join-Path $apkOutput 'app-release.apk'
            $splitApks = @(Get-ChildItem $apkOutput -Filter '*-release.apk' -File -ErrorAction SilentlyContinue |
                Where-Object { $_.Name -in @('app-arm64-v8a-release.apk', 'app-x86_64-release.apk') })
            if (-not $ReuseBuild -or $splitApks.Count -eq 0) {
                Push-Location $script:FlutterRoot
                try {
                    foreach ($staleApk in @(Get-ChildItem $apkOutput -Filter '*.apk' -ErrorAction SilentlyContinue)) {
                        Remove-Item -LiteralPath $staleApk.FullName -Force
                    }
                    Invoke-TorcaExternal 'Android split APKs' { flutter build apk --release --split-per-abi }
                    Invoke-TorcaExternal 'Android app bundle' { flutter build appbundle --release }
                } finally {
                    Pop-Location
                }
                Get-ChildItem $apkOutput -Filter '*-release.apk' |
                    Where-Object { $_.Name -in @('app-arm64-v8a-release.apk', 'app-x86_64-release.apk') } |
                    ForEach-Object {
                        Assert-TorcaAndroidPackage -Apk $_.FullName
                        Copy-Item $_.FullName (Join-Path $androidTarget $_.Name) -Force
                    }
                $bundle = Join-Path $script:FlutterRoot 'build/app/outputs/bundle/release/app-release.aab'
                if (Test-Path $bundle) {
                    Copy-Item $bundle (Join-Path $androidTarget 'Torca.aab') -Force
                }
            } else {
                # The orchestrator has already proven source/endpoint equality.
                # Reuse the already validated ABI packages; universal APKs are
                # not assumed because Flutter may omit libflutter.so there.
                foreach ($apk in $splitApks) {
                    Assert-TorcaAndroidPackage -Apk $apk.FullName
                    Copy-Item $apk.FullName (Join-Path $androidTarget $apk.Name) -Force
                }
                Write-Host 'Reusing existing Android ABI APKs; split APK/AAB packaging skipped.'
            }
            if ($Device) {
                $selectedApk = Select-TorcaAndroidApk -OutputRoot $apkOutput -UniversalApk $universalApk -Device $Device
                Assert-TorcaAndroidPackage -Apk $selectedApk
                Push-Location $script:FlutterRoot
                try {
                    if (Get-Command adb -ErrorAction SilentlyContinue) {
                        $installOutput = (& adb -s $Device install -r $selectedApk 2>&1 | Out-String).Trim()
                        $installCode = $LASTEXITCODE
                        if ($installCode -ne 0) {
                            if ($installOutput -match 'INSTALL_FAILED_USER_RESTRICTED') {
                                throw "Android blocked ADB installation on $Device. Enable USB/ADB installation permission on the device and retry. Details: $installOutput"
                            }
                            throw "ADB install failed with code $installCode on $Device. Details: $installOutput"
                        }
                    } else {
                        # Fallback for environments without adb. Flutter must still install
                        # the release variant, never app-debug.apk.
                        Invoke-TorcaExternal "Install Torca release on $Device" { flutter install --release -d $Device }
                    }
                    if (Get-Command adb -ErrorAction SilentlyContinue) {
                        $packagePath = (& adb -s $Device shell pm path com.torca.torca_app 2>&1 | Out-String).Trim()
                        if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($packagePath)) {
                            throw "Android installation verification failed for $Device."
                        }
                        $sha = [System.Security.Cryptography.SHA256]::Create()
                        try { $installedHash = ([BitConverter]::ToString($sha.ComputeHash([IO.File]::ReadAllBytes($selectedApk))).Replace('-', '')).ToLowerInvariant() }
                        finally { $sha.Dispose() }
                        $remotePath = (($packagePath -split "`r?`n" | Where-Object { $_ -match '^package:' } | Select-Object -First 1) -replace '^package:', '')
                        $remoteHashOutput = (& adb -s $Device shell sha256sum $remotePath 2>&1 | Out-String).Trim()
                        $remoteHash = ([Regex]::Match($remoteHashOutput, '(?i)\b[0-9a-f]{64}\b')).Value.ToLowerInvariant()
                        if ([string]::IsNullOrWhiteSpace($remoteHash) -or $remoteHash -ne $installedHash) {
                            throw "Installed Android APK hash mismatch on $Device. Local=$installedHash Remote=$remoteHash"
                        }
                        Write-Host "Android release artifact verified on ${Device}: $installedHash" -ForegroundColor Green
                        $activityOutput = (& adb -s $Device shell am start -W -n com.torca.torca_app/com.torca.app.MainActivity 2>&1 | Out-String).Trim()
                        if ($activityOutput -notmatch '(?m)^Status:\s*ok\s*$') {
                            throw "Android MainActivity failed to start on $Device. Details: $activityOutput"
                        }
                        Start-Sleep -Seconds 2
                        $activityPid = (& adb -s $Device shell pidof com.torca.torca_app 2>&1 | Out-String).Trim()
                        if ([string]::IsNullOrWhiteSpace($activityPid)) {
                            $crashLog = (& adb -s $Device logcat -b crash -d -t 120 2>&1 | Out-String).Trim()
                            throw "Android process exited after MainActivity start on $Device. Crash log: $crashLog"
                        }
                        Write-Host "Android MainActivity is running on ${Device}: PID $activityPid" -ForegroundColor Green
                    }
                } finally {
                    Pop-Location
                }
            }
        }

        Write-TorcaArtifactManifest -Root $artifactRoot
        Write-TorcaChecksums -Root $artifactRoot
        Write-Host "Artifacts: $artifactRoot"
    } finally {
        Pop-Location
    }
}

Export-ModuleMember -Function Invoke-TorcaBuild, Invoke-TorcaRun, Invoke-TorcaDeploy
