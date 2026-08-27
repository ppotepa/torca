Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$script:SourceFingerprintCache = @{}

function Get-TorcaBuildSourceFingerprint {
    param([string]$RepoRoot)

    $cacheKey = [IO.Path]::GetFullPath($RepoRoot).ToLowerInvariant()
    if ($script:SourceFingerprintCache.ContainsKey($cacheKey)) {
        return $script:SourceFingerprintCache[$cacheKey]
    }

    # A commit id is insufficient during normal local development: the worktree
    # can contain ABI-changing edits that have not been committed yet. Hash the
    # sources that can influence either the Flutter client or native bridge.
    $roots = @(
        (Join-Path $RepoRoot 'apps/client/flutter/lib'),
        (Join-Path $RepoRoot 'apps/client/flutter/android'),
        (Join-Path $RepoRoot 'apps/client/flutter/windows'),
        (Join-Path $RepoRoot 'apps/client/flutter/pubspec.yaml'),
        (Join-Path $RepoRoot 'apps/client/flutter/pubspec.lock'),
        (Join-Path $RepoRoot 'crates'),
        (Join-Path $RepoRoot 'scripts/modules'),
        (Join-Path $RepoRoot 'scripts/build.ps1'),
        (Join-Path $RepoRoot 'scripts/deploy.ps1'),
        (Join-Path $RepoRoot 'tools/build/overlays'),
        (Join-Path $RepoRoot 'tools/torca-contract-gen'),
        (Join-Path $RepoRoot 'Cargo.toml'),
        (Join-Path $RepoRoot 'Cargo.lock'),
        (Join-Path $RepoRoot 'release/version.json')
    )
    $files = foreach ($root in $roots) {
        if (Test-Path -LiteralPath $root -PathType Leaf) {
            Get-Item -LiteralPath $root
        } elseif (Test-Path -LiteralPath $root -PathType Container) {
            Get-ChildItem -LiteralPath $root -Recurse -File |
                Where-Object {
                    $_.FullName -notmatch '[\\/](target|build|\.dart_tool|\.gradle|\.cxx|node_modules|jniLibs)[\\/]' -and
                    $_.FullName -notmatch '[\\/]windows[\\/]flutter[\\/]generated_(plugin_registrant\.(cc|h)|plugins\.cmake)$' -and
                    $_.FullName -notmatch '[\\/]android[\\/]local\.properties$'
                }
        }
    }
    $sha = [System.Security.Cryptography.SHA256]::Create()
    try {
        foreach ($file in @($files | Sort-Object FullName)) {
            $relative = $file.FullName.Substring($RepoRoot.Length).Replace('\', '/')
            $relativeBytes = [System.Text.Encoding]::UTF8.GetBytes($relative + "`n")
            [void]$sha.TransformBlock($relativeBytes, 0, $relativeBytes.Length, $relativeBytes, 0)
            $bytes = [System.IO.File]::ReadAllBytes($file.FullName)
            [void]$sha.TransformBlock($bytes, 0, $bytes.Length, $bytes, 0)
        }
        [void]$sha.TransformFinalBlock([byte[]]::new(0), 0, 0)
        $fingerprint = [BitConverter]::ToString($sha.Hash).Replace('-', '')
        $script:SourceFingerprintCache[$cacheKey] = $fingerprint
        return $fingerprint
    } finally {
        $sha.Dispose()
    }
}

function Get-TorcaScopedBuildPaths {
    param(
        [Parameter(Mandatory = $true)]$Paths,
        [Parameter(Mandatory = $true)][string]$Target,
        [Parameter(Mandatory = $true)][string]$Configuration
    )
    [pscustomobject]@{
        RepoRoot = $Paths.RepoRoot
        # Keep the build-identity module self-contained. Importing Config with
        # -Force here can unload the caller's exported functions (including
        # Get-TorcaPaths) while `torca.ps1` is still starting up.
        ManifestFile = Join-Path $Paths.BuildManifestRoot "$($Target.ToLowerInvariant())-$($Configuration.ToLowerInvariant()).json"
    }
}

function Get-TorcaScopedBuildManifest {
    param(
        [Parameter(Mandatory = $true)]$Paths,
        [Parameter(Mandatory = $true)][string]$Target,
        [Parameter(Mandatory = $true)][string]$Configuration
    )
    $scopedPaths = Get-TorcaScopedBuildPaths -Paths $Paths -Target $Target -Configuration $Configuration
    if (-not (Test-Path -LiteralPath $scopedPaths.ManifestFile)) { return $null }
    try { return Get-Content -LiteralPath $scopedPaths.ManifestFile -Raw | ConvertFrom-Json }
    catch { throw "Invalid Torca build manifest: $($scopedPaths.ManifestFile)" }
}

function Clear-TorcaBuildSourceFingerprintCache {
    $script:SourceFingerprintCache = @{}
}

function Test-TorcaClientArtifactsExist {
    param(
        [Parameter(Mandatory = $true)][string]$RepoRoot,
        [Parameter(Mandatory = $true)][ValidateSet('windows','android','all')][string]$Target,
        [Parameter(Mandatory = $true)][ValidateSet('debug','release')][string]$Configuration
    )

    $flutterRoot = Join-Path $RepoRoot 'apps/client/flutter'
    if ($Target -in @('windows','all')) {
        $name = if ($Configuration -eq 'release') { 'Release' } else { 'Debug' }
        if (-not (Test-Path -LiteralPath (Join-Path $flutterRoot "build/windows/x64/runner/$name/torca_app.exe"))) {
            return $false
        }
    }
    if ($Target -in @('android','all')) {
        $apkOutput = Join-Path $flutterRoot 'build/app/outputs/flutter-apk'
        $universalApk = Join-Path $apkOutput "app-$Configuration.apk"
        $requiredSplitApks = @(Get-ChildItem -LiteralPath $apkOutput -Filter "app-*-$Configuration.apk" -File -ErrorAction SilentlyContinue |
            Where-Object { $_.Name -match "^app-(normal-)?(arm64-v8a|x86_64)-$Configuration\.apk$" } |
            Select-Object -ExpandProperty FullName)
        $hasUniversalApk = Test-Path -LiteralPath $universalApk
        $hasRequiredSplitApks = @($requiredSplitApks | Where-Object { Test-Path -LiteralPath $_ }).Count -ge 2
        if (-not $hasUniversalApk -and -not $hasRequiredSplitApks) { return $false }
    }
    return $true
}

function Get-TorcaExistingBuildManifest {
    param(
        [Parameter(Mandatory = $true)]$Paths,
        [Parameter(Mandatory = $true)][string]$Endpoint,
        [Parameter(Mandatory = $true)][ValidateSet('windows','android','all')][string]$Target,
        [Parameter(Mandatory = $true)][ValidateSet('debug','release')][string]$Configuration
    )

    # "Existing" deliberately ignores current source changes: it means the
    # operator wants to reinstall the last already-built binary.  The relay
    # endpoint and artifacts are still verified because deploying a binary
    # compiled for another onion would create a misleading, unusable install.
    $candidateTargets = if ($Target -eq 'all') { @('all') } else { @($Target, 'all') }
    foreach ($candidateTarget in $candidateTargets) {
        $manifest = Get-TorcaScopedBuildManifest -Paths $Paths -Target $candidateTarget -Configuration $Configuration
        if (-not $manifest) { continue }
        if ([string]$manifest.Endpoint -ne $Endpoint -or [string]$manifest.Configuration -ne $Configuration) { continue }
        if (-not (@($manifest.Targets) -contains $candidateTarget)) { continue }
        $provider = [string]$env:TORCA_COMMUNICATION_PROVIDER
        if ([string]::IsNullOrWhiteSpace($provider)) { $provider = 'tor' }
        if ($provider.ToLowerInvariant() -eq 'iroh') {
            $expectedProfile = Get-TorcaIrohProfile
            $recordedProfile = if ($manifest.PSObject.Properties.Name -contains 'irohProfile' -and
                -not [string]::IsNullOrWhiteSpace([string]$manifest.irohProfile)) {
                [string]$manifest.irohProfile
            } else { 'always' }
            if ($recordedProfile -ne $expectedProfile) { continue }
        }
        if (-not (Test-TorcaClientArtifactsExist -RepoRoot $Paths.RepoRoot -Target $Target -Configuration $Configuration)) { continue }
        return $manifest
    }
    return $null
}

function Test-TorcaBuildRequired {
    param(
        [pscustomobject]$Paths,
        [string]$Endpoint,
        [string]$Target,
        [string]$Configuration
    )
    $manifest = Get-TorcaScopedBuildManifest -Paths $Paths -Target $Target -Configuration $Configuration
    if (-not $manifest) { return $true }
    if ([string]$manifest.Endpoint -ne $Endpoint -or [string]$manifest.Configuration -ne $Configuration) { return $true }
    if (-not (@($manifest.Targets) -contains $Target)) { return $true }
    $provider = [string]$env:TORCA_COMMUNICATION_PROVIDER
    if ([string]::IsNullOrWhiteSpace($provider)) { $provider = 'tor' }
    if ($provider.ToLowerInvariant() -eq 'iroh') {
        $recordedProfile = if ($manifest.PSObject.Properties.Name -contains 'irohProfile' -and
            -not [string]::IsNullOrWhiteSpace([string]$manifest.irohProfile)) {
            [string]$manifest.irohProfile
        } else { 'always' }
        if ($recordedProfile -ne (Get-TorcaIrohProfile)) { return $true }
    }
    if ([string]$manifest.SourceFingerprint -ne (Get-TorcaBuildSourceFingerprint -RepoRoot $Paths.RepoRoot)) { return $true }
    if ([string]$manifest.BuildId -ne (Get-TorcaBuildId -RepoRoot $Paths.RepoRoot -Endpoint $Endpoint -Target $Target -Configuration $Configuration)) { return $true }
    if (-not (Test-TorcaClientArtifactsExist -RepoRoot $Paths.RepoRoot -Target $Target -Configuration $Configuration)) { return $true }
    return $false
}

function Get-TorcaBuildId {
    param(
        [string]$RepoRoot,
        [string]$Endpoint,
        [string]$Target,
        [string]$Configuration
    )
    $provider = [string]$env:TORCA_COMMUNICATION_PROVIDER
    if ([string]::IsNullOrWhiteSpace($provider)) { $provider = 'tor' }
    $profile = if ($provider.ToLowerInvariant() -eq 'iroh') { Get-TorcaIrohProfile } else { 'none' }
    $payload = "$(Get-TorcaBuildSourceFingerprint -RepoRoot $RepoRoot)|$Endpoint|$Target|$Configuration|$provider|$profile"
    $sha = [System.Security.Cryptography.SHA256]::Create()
    try {
        return [BitConverter]::ToString(
            $sha.ComputeHash([System.Text.Encoding]::UTF8.GetBytes($payload))
        ).Replace('-', '')
    } finally {
        $sha.Dispose()
    }
}

function Invoke-TorcaClientBuild {
    param(
        [string]$RepoRoot,
        [string]$Target,
        [string]$Configuration,
        [string]$Endpoint,
        [ValidateSet('Full','Quick','Skip')][string]$Validation = 'Full'
    )
    $old = $env:TORCA_RELAY_ENDPOINT; $oldOrchestrated = $env:TORCA_ORCHESTRATED; $oldBuildId = $env:TORCA_BUILD_ID
    $oldProductVersion = $env:TORCA_PRODUCT_VERSION; $oldSourceFingerprint = $env:TORCA_SOURCE_FINGERPRINT
    $oldSourceCommit = $env:TORCA_SOURCE_COMMIT; $oldRelayHash = $env:TORCA_RELAY_ENDPOINT_HASH
    $oldIrohProfile = $env:TORCA_IROH_PROFILE
    try {
        $env:TORCA_RELAY_ENDPOINT = $Endpoint
        $env:TORCA_ORCHESTRATED = '1'
        $env:TORCA_BUILD_ID = Get-TorcaBuildId -RepoRoot $RepoRoot -Endpoint $Endpoint -Target $Target -Configuration $Configuration
        $release = Get-Content (Join-Path $RepoRoot 'release/version.json') -Raw | ConvertFrom-Json
        $env:TORCA_PRODUCT_VERSION = [string]$release.version
        $env:TORCA_SOURCE_FINGERPRINT = Get-TorcaBuildSourceFingerprint -RepoRoot $RepoRoot
        $env:TORCA_SOURCE_COMMIT = ((git -C $RepoRoot rev-parse HEAD 2>$null | Out-String).Trim())
        if ([string]::IsNullOrWhiteSpace($env:TORCA_SOURCE_COMMIT)) { $env:TORCA_SOURCE_COMMIT = 'working-tree' }
        $endpointSha = [System.Security.Cryptography.SHA256]::Create()
        try {
            $env:TORCA_RELAY_ENDPOINT_HASH = [BitConverter]::ToString(
                $endpointSha.ComputeHash([Text.Encoding]::UTF8.GetBytes($Endpoint))
            ).Replace('-', '').ToLowerInvariant()
        } finally { $endpointSha.Dispose() }
        $buildProviderArgs = @{}
        if (-not [string]::IsNullOrWhiteSpace([string]$env:TORCA_COMMUNICATION_PROVIDER)) {
            $buildProviderArgs['CommunicationProvider'] = [string]$env:TORCA_COMMUNICATION_PROVIDER
        }
        & (Join-Path $RepoRoot 'scripts/build.ps1') -Target $Target -Configuration $Configuration -Validation $Validation @buildProviderArgs
        if ($LASTEXITCODE -ne 0) { throw "Build failed with code $LASTEXITCODE." }
    } finally {
        $env:TORCA_RELAY_ENDPOINT = $old; $env:TORCA_ORCHESTRATED = $oldOrchestrated; $env:TORCA_BUILD_ID = $oldBuildId
        $env:TORCA_PRODUCT_VERSION = $oldProductVersion; $env:TORCA_SOURCE_FINGERPRINT = $oldSourceFingerprint
        $env:TORCA_SOURCE_COMMIT = $oldSourceCommit; $env:TORCA_RELAY_ENDPOINT_HASH = $oldRelayHash
        $env:TORCA_IROH_PROFILE = $oldIrohProfile
    }
}

function Invoke-TorcaClientDeploy {
    param([string]$RepoRoot, [string]$Target, [string]$Device, [string]$Endpoint)
    $old = $env:TORCA_RELAY_ENDPOINT; $oldOrchestrated = $env:TORCA_ORCHESTRATED
    try {
        $env:TORCA_RELAY_ENDPOINT = $Endpoint
        $env:TORCA_ORCHESTRATED = '1'
        & (Join-Path $RepoRoot 'scripts/deploy.ps1') -Target $Target -Device $Device -ReuseBuild
        if ($LASTEXITCODE -ne 0) { throw "Deploy failed with code $LASTEXITCODE." }
    } finally { $env:TORCA_RELAY_ENDPOINT = $old; $env:TORCA_ORCHESTRATED = $oldOrchestrated }
}

function Install-TorcaClient {
    param(
        [string]$RepoRoot,
        [string]$Device,
        [ValidateSet('debug','release')][string]$Configuration = 'debug'
    )
    if (-not $Device) { throw 'An Android device id is required for installation.' }
    $apkOutput = Join-Path $RepoRoot 'apps/client/flutter/build/app/outputs/flutter-apk'
    $universalApk = Join-Path $apkOutput "app-$Configuration.apk"
    $apk = $null
    $abis = @()
    if (Get-Command adb -ErrorAction SilentlyContinue) {
        $abisOutput = (& adb -s $Device shell getprop ro.product.cpu.abilist 2>$null | Out-String).Trim()
        $abis = @($abisOutput -split ',' | Where-Object { -not [string]::IsNullOrWhiteSpace($_) })
        if ($abis.Count -eq 0) {
            $primaryAbi = (& adb -s $Device shell getprop ro.product.cpu.abi 2>$null | Out-String).Trim()
            if (-not [string]::IsNullOrWhiteSpace($primaryAbi)) { $abis = @($primaryAbi) }
        }
        foreach ($abi in $abis) {
            $candidates = switch ($abi.Trim()) {
                'arm64-v8a' {
                    @((Join-Path $apkOutput "app-arm64-v8a-$Configuration.apk"), (Join-Path $apkOutput "app-normal-arm64-v8a-$Configuration.apk")); break
                }
                'x86_64' {
                    @((Join-Path $apkOutput "app-x86_64-$Configuration.apk"), (Join-Path $apkOutput "app-normal-x86_64-$Configuration.apk")); break
                }
                default { @() }
            }
            foreach ($candidate in @($candidates)) {
                if (Test-Path -LiteralPath $candidate) {
                    $apk = $candidate
                    break
                }
            }
            if ($apk) { break }
        }
    }
    $splitApks = @(Get-ChildItem -LiteralPath $apkOutput -Filter "app-*-$Configuration.apk" -File -ErrorAction SilentlyContinue)
    if (-not $apk -and $splitApks.Count -eq 0 -and (Test-Path -LiteralPath $universalApk)) {
        $apk = $universalApk
    }
    if (Get-Command adb -ErrorAction SilentlyContinue) {
        if (-not $apk) {
            throw "No compatible $Configuration Android APK found for $Device in $apkOutput (ABIs=$($abis -join ', ')). Split APKs exist, so refusing to fall back to a potentially stale universal APK."
        }
        $installOutput = ''
        $installCode = 1
        for ($attempt = 1; $attempt -le 2; $attempt++) {
            $installOutput = (& adb -s $Device install -r $apk 2>&1 | Out-String).Trim()
            $installCode = $LASTEXITCODE
            if ($installCode -eq 0) { break }
            if ($installOutput -notmatch 'INSTALL_FAILED_USER_RESTRICTED' -or $attempt -eq 2) { break }

            Write-Host ''
            Write-Host "Android requires confirmation on $Device." -ForegroundColor Yellow
            Write-Host 'Unlock the phone and accept the installation prompt.' -ForegroundColor Yellow
            Write-Host 'On Xiaomi/Redmi/Poco enable Developer options > Install via USB and USB debugging (Security settings).' -ForegroundColor Yellow
            $null = Read-Host 'Press Enter to retry this APK without rebuilding or resetting data'
        }
        if ($installCode -ne 0) {
            if ($installOutput -match 'INSTALL_FAILED_USER_RESTRICTED') {
                throw "Android blocked or cancelled ADB installation twice on $Device. Keep the phone unlocked and approve the system install prompt. On Xiaomi/Redmi/Poco HyperOS/MIUI enable Developer options > Install via USB and USB debugging (Security settings); wireless debugging alone is insufficient. The APK remains available and can be installed with Use Last. Details: $installOutput"
            }
            throw "ADB install failed with code $installCode on $Device. Details: $installOutput"
        }
        $packagePath = (& adb -s $Device shell pm path com.torca.torca_app 2>&1 | Out-String).Trim()
        if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($packagePath)) {
            throw "ADB installation verification failed for $Device."
        }
        Assert-TorcaAndroidInstalledArtifact -Device $Device -Apk $apk
        return
    }
    Push-Location (Join-Path $RepoRoot 'apps/client/flutter')
    try {
        & flutter install "--$Configuration" -d $Device
        if ($LASTEXITCODE -ne 0) { throw "Install failed with code $LASTEXITCODE." }
    } finally { Pop-Location }
}

function Assert-TorcaAndroidInstalledArtifact {
    param(
        [Parameter(Mandatory = $true)][string]$Device,
        [Parameter(Mandatory = $true)][string]$Apk
    )
    $localHash = Get-TorcaFileSha256 -Path $Apk
    $packagePath = (& adb -s $Device shell pm path com.torca.torca_app 2>&1 | Out-String).Trim()
    $remotePath = ($packagePath -split "`r?`n" |
        Where-Object { $_ -match '^package:' } |
        Select-Object -First 1) -replace '^package:', ''
    if ([string]::IsNullOrWhiteSpace($remotePath)) {
        throw "Android package path could not be read after installation on $Device."
    }
    $remoteHashOutput = (& adb -s $Device shell sha256sum $remotePath 2>&1 | Out-String).Trim()
    $remoteHash = ([Regex]::Match($remoteHashOutput, '(?i)\b[0-9a-f]{64}\b')).Value.ToLowerInvariant()
    if ([string]::IsNullOrWhiteSpace($remoteHash)) {
        throw "Android package hash could not be read after installation on $Device. Details: $remoteHashOutput"
    }
    if ($remoteHash -ne $localHash) {
        throw "Installed Android APK hash mismatch on $Device. Local=$localHash Remote=$remoteHash"
    }
    Write-Host "Android artifact verified on ${Device}: $localHash" -ForegroundColor Green
}

function Get-TorcaFileSha256 {
    param([Parameter(Mandatory = $true)][string]$Path)
    $sha = [System.Security.Cryptography.SHA256]::Create()
    try {
        return ([BitConverter]::ToString($sha.ComputeHash([IO.File]::ReadAllBytes($Path))).Replace('-', '')).ToLowerInvariant()
    } finally { $sha.Dispose() }
}

function Invoke-TorcaClientReleaseDeploy {
    param(
        [string]$RepoRoot,
        [string]$Target,
        [string]$Device,
        [string]$Endpoint,
        [switch]$SkipLaunch
    )
    $old = $env:TORCA_RELAY_ENDPOINT; $oldOrchestrated = $env:TORCA_ORCHESTRATED
    try {
        $env:TORCA_RELAY_ENDPOINT = $Endpoint
        $env:TORCA_ORCHESTRATED = '1'
        $arguments = @{ Target = $Target; Device = $Device; ReuseBuild = $true }
        if ($SkipLaunch) { $arguments.SkipLaunch = $true }
        & (Join-Path $RepoRoot 'scripts/deploy.ps1') @arguments
        if ($LASTEXITCODE -ne 0) { throw "Release deploy failed with code $LASTEXITCODE." }
    } finally { $env:TORCA_RELAY_ENDPOINT = $old; $env:TORCA_ORCHESTRATED = $oldOrchestrated }
}

function Invoke-TorcaClientRun {
    param(
        [string]$RepoRoot,
        [string]$Target,
        [string]$Device,
        [ValidateSet('debug','release')][string]$Configuration = 'debug',
        [switch]$Installed,
        [string]$ExpectedBuildId,
        [switch]$DeferHealthCheck,
        [ValidateRange(5,900)][int]$HealthTimeoutSeconds = 900
    )
    if ($Installed -and $Target -eq 'windows') {
        $directoryName = if ($Configuration -eq 'release') { 'Release' } else { 'Debug' }
        $exe = Join-Path $RepoRoot "apps/client/flutter/build/windows/x64/runner/$directoryName/torca_app.exe"
        if (-not (Test-Path -LiteralPath $exe)) { throw "Built Windows $Configuration client is missing: $exe" }
        $running = @(Get-Process -Name torca_app -ErrorAction SilentlyContinue)
        foreach ($process in $running) {
            if ($process.MainWindowHandle -ne 0) {
                [void]$process.CloseMainWindow()
            }
        }
        if ($running.Count -gt 0) {
            $deadline = [DateTime]::UtcNow.AddSeconds(5)
            do {
                Start-Sleep -Milliseconds 250
                $remaining = @(Get-Process -Name torca_app -ErrorAction SilentlyContinue)
            } while ($remaining.Count -gt 0 -and [DateTime]::UtcNow -lt $deadline)
            if ($remaining.Count -gt 0) {
                Write-Warning 'Torca client did not close gracefully; terminating the existing instance for the requested release restart.'
                foreach ($process in $remaining) {
                    Stop-Process -Id $process.Id -Force -ErrorAction Stop
                }
                $stopDeadline = [DateTime]::UtcNow.AddSeconds(5)
                do {
                    Start-Sleep -Milliseconds 250
                    $remaining = @(Get-Process -Name torca_app -ErrorAction SilentlyContinue)
                } while ($remaining.Count -gt 0 -and [DateTime]::UtcNow -lt $stopDeadline)
                if ($remaining.Count -gt 0) {
                    throw 'Torca client could not be stopped for a release restart.'
                }
            }
        }
        # Keep the exact process returned by Start-Process.  A concurrently
        # running `flutter run -d windows` starts a Debug runner with the same
        # process name.  Looking up any torca_app here used to make the deploy
        # health check accidentally validate that Debug process and its
        # target/debug/torca_native.dll instead of the requested release.
        $started = Start-Process -FilePath $exe -WorkingDirectory (Split-Path $exe) -PassThru
        if ($DeferHealthCheck) {
            return [pscustomobject]@{
                Platform = 'windows'
                Device = $null
                ProcessId = $started.Id
                Executable = $exe
            }
        }
        Wait-TorcaClientLaunch -Platform windows -ExpectedBuildId $ExpectedBuildId -ExpectedWindowsProcessId $started.Id -ExpectedWindowsExecutable $exe -TimeoutSeconds $HealthTimeoutSeconds
        return
    }
    if ($Installed -and $Device -and (Get-Command adb -ErrorAction SilentlyContinue)) {
        & adb -s $Device shell monkey -p com.torca.torca_app 1 | Out-Null
        if ($LASTEXITCODE -ne 0) { throw "ADB launch failed with code $LASTEXITCODE." }
        if ($DeferHealthCheck) {
            return [pscustomobject]@{
                Platform = 'android'
                Device = $Device
                ProcessId = $null
                Executable = $null
            }
        }
        Wait-TorcaClientLaunch -Platform android -Device $Device -ExpectedBuildId $ExpectedBuildId -TimeoutSeconds $HealthTimeoutSeconds
        return
    }
    $args = @{ Target = $Target }
    if ($Device) { $args.Device = $Device }
    if ($Installed) { throw 'Installed Android launch requires an adb device id.' }
    $oldOrchestrated = $env:TORCA_ORCHESTRATED
    try { $env:TORCA_ORCHESTRATED = '1'; & (Join-Path $RepoRoot 'scripts/run.ps1') @args }
    finally { $env:TORCA_ORCHESTRATED = $oldOrchestrated }
    if ($LASTEXITCODE -ne 0) { throw "Run failed with code $LASTEXITCODE." }
}

function Wait-TorcaClientLaunch {
    param(
        [Parameter(Mandatory = $true)][ValidateSet('windows','android')][string]$Platform,
        [string]$Device,
        [string]$ExpectedBuildId,
        [int]$ExpectedWindowsProcessId,
        [string]$ExpectedWindowsExecutable,
        [ValidateRange(5,900)][int]$TimeoutSeconds = 900
    )
    $startedAt = [DateTime]::UtcNow.AddSeconds(-10)
    $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    $lastDetail = ''
    $lastReported = ''
    $processObserved = $false
    # The legacy PowerShell runner must use the same provider-neutral readiness
    # contract as torca-deploy.  NETWORK_READY/TOR_READY are compatibility
    # evidence for Tor only; direct providers publish COMMUNICATION_READY.
    $selectedProvider = [string]$env:TORCA_COMMUNICATION_PROVIDER
    if ([string]::IsNullOrWhiteSpace($selectedProvider)) { $selectedProvider = 'tor' }
    $providerReadyCode = if ($selectedProvider -eq 'tor') { 'NETWORK_READY' } else { 'COMMUNICATION_READY' }
    do {
        if ($Platform -eq 'windows') {
            if ($ExpectedWindowsProcessId) {
                $process = @(Get-Process -Id $ExpectedWindowsProcessId -ErrorAction SilentlyContinue)
            } else {
                $process = @(Get-Process -Name torca_app -ErrorAction SilentlyContinue)
            }
            if ($process.Count -gt 0) {
                $processObserved = $true
                if ($ExpectedWindowsExecutable) {
                    $processInfo = Get-CimInstance Win32_Process -Filter "ProcessId = $($process[0].Id)" -ErrorAction SilentlyContinue
                    $actualExecutable = if ($processInfo) { [string]$processInfo.ExecutablePath } else { '' }
                    # Win32_Process may return a relative dot for a newly
                    # created GUI process while its command metadata is still
                    # settling. Prefer the process object's canonical path
                    # before treating the executable as mismatched.
                    if ([string]::IsNullOrWhiteSpace($actualExecutable) -or $actualExecutable -eq '.') {
                        try { $actualExecutable = [string]$process[0].Path } catch { $actualExecutable = '' }
                    }
                    if ([string]::IsNullOrWhiteSpace($actualExecutable) -or $actualExecutable -eq '.') {
                        try { $actualExecutable = [string]$process[0].MainModule.FileName } catch { $actualExecutable = '' }
                    }
                    $expectedExecutable = [IO.Path]::GetFullPath($ExpectedWindowsExecutable)
                    if ([string]::IsNullOrWhiteSpace($actualExecutable) -or
                        -not [string]::Equals([IO.Path]::GetFullPath($actualExecutable), $expectedExecutable, [StringComparison]::OrdinalIgnoreCase)) {
                        throw "Windows release launch resolved to an unexpected executable. Expected=$expectedExecutable Actual=$actualExecutable. Stop any 'flutter run -d windows' session and retry the release deploy."
                    }

                    $expectedNative = Join-Path (Split-Path $expectedExecutable) 'torca_native.dll'
                    $nativeModule = @(Get-Process -Id $process[0].Id -Module -ErrorAction SilentlyContinue |
                        Where-Object { $_.ModuleName -ieq 'torca_native.dll' } |
                        Select-Object -First 1)
                    if ($nativeModule.Count -gt 0 -and
                        -not [string]::Equals([IO.Path]::GetFullPath([string]$nativeModule[0].FileName), [IO.Path]::GetFullPath($expectedNative), [StringComparison]::OrdinalIgnoreCase)) {
                        throw "Windows release process loaded an unexpected native runtime. Expected=$expectedNative Actual=$($nativeModule[0].FileName). Stop any 'flutter run -d windows' session and retry the release deploy."
                    }
                }
                $logRoot = if ($env:LOCALAPPDATA) { Join-Path $env:LOCALAPPDATA 'Torca/logs/devices' } else { $null }
                $runStart = if ($logRoot -and (Test-Path -LiteralPath $logRoot)) {
                    Get-ChildItem -LiteralPath $logRoot -Recurse -File -Filter 'run.start.json' -ErrorAction SilentlyContinue |
                        Where-Object LastWriteTimeUtc -ge $startedAt |
                        Sort-Object LastWriteTime -Descending | Select-Object -First 1
                } else { $null }
                if ($runStart) {
                    try {
                        $run = Get-Content -LiteralPath $runStart.FullName -Raw | ConvertFrom-Json
                        if ($ExpectedBuildId -and [string]$run.build_id -and [string]$run.build_id -ne $ExpectedBuildId) {
                            throw "Windows runtime build ID mismatch. Expected=$ExpectedBuildId Actual=$($run.build_id). Diagnostic: $($runStart.FullName)"
                        }
                        $bootstrapLog = Join-Path $runStart.DirectoryName 'bootstrap.log'
                        $bootstrap = if (Test-Path -LiteralPath $bootstrapLog) { Get-Content -LiteralPath $bootstrapLog -Tail 80 -ErrorAction SilentlyContinue } else { @() }
                        $events = @($bootstrap | ForEach-Object { try { $_ | ConvertFrom-Json } catch { $null } } | Where-Object { $_ })
                        $failure = $events | Where-Object { $_.code -eq 'RUNTIME_START_FAILED' } | Select-Object -Last 1
                        $providerReady = $events | Where-Object { $_.code -eq $providerReadyCode } | Select-Object -Last 1
                        $localReady = $events | Where-Object { $_.code -in @('LOCAL_READY', 'FLUTTER_GATEWAY_READY') } | Select-Object -Last 1
                        if ($providerReady -or ($selectedProvider -ne 'tor' -and $localReady)) {
                            Write-Host "Windows communication health verified: PID $($process[0].Id), provider=$selectedProvider, build=$($run.build_id), state=$providerReadyCode" -ForegroundColor Green
                            return
                        }
                        if ($failure -and [string]$failure.message -match 'bootstrap Arti client stalled') {
                            throw "Windows provider bootstrap reached a terminal stall. $($failure.message). Restart is required; incident log: $bootstrapLog"
                        }
                        if ($failure -and -not $localReady) { $lastDetail = "Windows $selectedProvider startup: $($failure.message)" }
                        elseif ($localReady) { $lastDetail = "Windows local runtime is ready; waiting for provider=$selectedProvider ($providerReadyCode)" }
                        else { $lastEvent = $events | Select-Object -Last 1; $lastDetail = if ($lastEvent) { "Windows runtime event: $($lastEvent.code)" } else { 'Windows runtime log is initializing' } }
                    } catch {
                        if ($_.Exception.Message -like 'Windows runtime build ID mismatch*' -or $_.Exception.Message -like 'Windows Tor bootstrap reached a terminal stall*') { throw }
                        Write-Warning "Windows process is running but startup metadata could not be read: $($runStart.FullName)"
                    }
                } else { $lastDetail = "Windows process PID $($process[0].Id) is running; waiting for a fresh runtime log" }
            }
            elseif ($processObserved) {
                throw "Windows torca_app exited before provider=$selectedProvider became ready. Collect incident logs with scripts/torca.ps1 -Command collect -Profile incident."
            } elseif ($ExpectedWindowsProcessId) {
                throw "Windows release process PID $ExpectedWindowsProcessId exited before provider=$selectedProvider became ready. Collect incident logs with scripts/torca.ps1 -Command collect -Profile incident."
            } else { $lastDetail = 'Windows torca_app process is not running yet' }
        } else {
            if (-not (Get-Command adb -ErrorAction SilentlyContinue)) { throw 'adb is required for Android launch verification.' }
            $pid = (& adb -s $Device shell pidof com.torca.torca_app 2>&1 | Out-String).Trim()
            if ($LASTEXITCODE -eq 0 -and -not [string]::IsNullOrWhiteSpace($pid)) {
                $processObserved = $true
                $logFiles = (& adb -s $Device shell find /sdcard/Android/data/com.torca.torca_app/files/torca/logs -type f -name bootstrap.log 2>$null | Out-String).Trim() -split "`r?`n" | Where-Object { $_ }
                $remoteLog = $logFiles | Select-Object -Last 1
                $events = if ($remoteLog) { (& adb -s $Device shell tail -n 80 $remoteLog 2>$null | Out-String) } else { '' }
                if ($events -match ('"code"\s*:\s*"' + [Regex]::Escape($providerReadyCode) + '"') -or
                    ($selectedProvider -ne 'tor' -and $events -match '"code"\s*:\s*"(LOCAL_READY|FLUTTER_GATEWAY_READY)"')) {
                    Write-Host "Android communication health verified on ${Device}: PID $pid, provider=$selectedProvider, state=$providerReadyCode" -ForegroundColor Green
                    return
                }
                $failure = [Regex]::Matches($events, '"code"\s*:\s*"RUNTIME_START_FAILED"[^\r\n]*') | Select-Object -Last 1
                $localReady = $events -match '"code"\s*:\s*"(LOCAL_READY|FLUTTER_GATEWAY_READY)"'
                if ($failure -and -not $localReady) {
                    $lastDetail = "Android $selectedProvider startup: $($failure.Value)"
                } elseif ($localReady) {
                    $lastDetail = "Android local runtime is ready; waiting for provider=$selectedProvider ($providerReadyCode)"
                } else {
                    $lastDetail = "Android process PID $pid is running; waiting for provider=$selectedProvider readiness"
                }
            }
            elseif ($processObserved) {
                throw "Android package process exited on $Device before provider=$selectedProvider became ready. Collect incident logs with scripts/torca.ps1 -Command collect -Profile incident -IncludeLogcat."
            } else { $lastDetail = "Android package process is not running on $Device yet" }
        }
        if ($lastDetail -ne $lastReported) {
            Write-TorcaStage -Name "$Platform runtime health" -State 'running' -Detail $lastDetail
            $lastReported = $lastDetail
        }
        Start-Sleep -Milliseconds 500
    } while ([DateTime]::UtcNow -lt $deadline)
    $hint = if ($Platform -eq 'windows') { 'collect logs with scripts/torca.ps1 -Command collect -Profile incident' } else { 'collect logs with scripts/torca.ps1 -Command collect -Profile incident -IncludeLogcat' }
    throw "Runtime launch health check timed out after ${TimeoutSeconds}s for provider=${selectedProvider}: $lastDetail. $hint"
}

function Get-TorcaIrohProfile {
    $value = [string]$env:TORCA_IROH_PROFILE
    if ([string]::IsNullOrWhiteSpace($value)) { return 'always' }
    switch ($value.Trim().ToLowerInvariant()) {
        'always' { return 'always' }
        'always-reachable' { return 'always' }
        'direct' { return 'direct' }
        'direct-only' { return 'direct' }
        'local' { return 'local' }
        'local-only' { return 'local' }
        default { throw "Unsupported TORCA_IROH_PROFILE '$value'. Use always, direct or local." }
    }
}

function Write-TorcaBuildManifest {
    param(
        [pscustomobject]$Paths,
        [string]$Endpoint,
        [string[]]$Targets,
        [string]$Configuration,
        [string]$BuildId,
        [string]$SourceFingerprint
    )
    $manifestTarget = if (@($Targets).Count -eq 1) { @($Targets)[0] } else { ($Targets -join ',') }
    $scopedPaths = Get-TorcaScopedBuildPaths -Paths $Paths -Target $manifestTarget -Configuration $Configuration
    $commit = (& git -C $Paths.RepoRoot rev-parse HEAD 2>$null)
    $release = Get-Content (Join-Path $Paths.RepoRoot 'release/version.json') -Raw | ConvertFrom-Json
    $provider = [string]$env:TORCA_COMMUNICATION_PROVIDER
    if ([string]::IsNullOrWhiteSpace($provider)) { $provider = 'tor' }
    $irohProfile = if ($provider.ToLowerInvariant() -eq 'iroh') { Get-TorcaIrohProfile } else { $null }
    $compiledFeatures = switch ($provider.ToLowerInvariant()) {
        'tor' { 'provider-tor,radio-audio' }
        'iroh' { 'provider-iroh,radio-audio' }
        'webrtc' { 'provider-webrtc,radio-audio' }
        default { throw "Unsupported communication provider '$provider'." }
    }
    $manifest = [pscustomobject]@{
        Schema = 1; Endpoint = $Endpoint; Targets = @($Targets); Configuration = $Configuration
        communicationProvider = $provider; compiledFeatures = $compiledFeatures; irohProfile = $irohProfile
        # These values must be the exact frozen values embedded into the
        # binaries. Recomputing after Flutter/Gradle generated files changed
        # used to produce a manifest for an identity no artifact contained.
        SourceFingerprint = if ($SourceFingerprint) { $SourceFingerprint } else { Get-TorcaBuildSourceFingerprint -RepoRoot $Paths.RepoRoot }
        BuildId = if ($BuildId) { $BuildId } else { Get-TorcaBuildId -RepoRoot $Paths.RepoRoot -Endpoint $Endpoint -Target $manifestTarget -Configuration $Configuration }
        ContractSchema = [int]$release.contractSchema
        Commit = ($commit -join '').Trim(); BuiltAt = [DateTime]::UtcNow.ToString('o')
    }
    $parent = Split-Path -Parent $scopedPaths.ManifestFile
    New-Item -ItemType Directory -Path $parent -Force | Out-Null
    $temporary = "$($scopedPaths.ManifestFile).tmp"
    $manifest | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $temporary -Encoding utf8
    Move-Item -LiteralPath $temporary -Destination $scopedPaths.ManifestFile -Force
}

Export-ModuleMember -Function Get-TorcaBuildSourceFingerprint, Clear-TorcaBuildSourceFingerprintCache, Get-TorcaScopedBuildPaths, Get-TorcaScopedBuildManifest, Get-TorcaExistingBuildManifest, Get-TorcaIrohProfile, Get-TorcaBuildId, Test-TorcaBuildRequired, Invoke-TorcaClientBuild, Invoke-TorcaClientDeploy, Install-TorcaClient, Assert-TorcaAndroidInstalledArtifact, Get-TorcaFileSha256, Invoke-TorcaClientReleaseDeploy, Invoke-TorcaClientRun, Wait-TorcaClientLaunch, Write-TorcaBuildManifest
