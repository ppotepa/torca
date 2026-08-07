# Packaged Tor runtime

Torca 0.1 owns a dedicated Tor child process. Production builds never search `PATH`, Tor Browser, or another application's Tor installation.

Before a Windows or Android build, place reviewed Tor distribution artifacts at exactly these paths:

```text
vendor/tor/
├── windows/
│   ├── tor.exe
│   └── <all runtime DLL/data files required by that Tor distribution>
└── android/
    ├── arm64-v8a/
    │   └── libtor.so
    └── x86_64/
        └── libtor.so
```

`libtor.so` is the packaged executable PIE used as the owned Android Tor process; its name intentionally follows Android native-library packaging rules so the APK extracts it into `applicationInfo.nativeLibraryDir`.

The public `build.ps1`, `run.ps1`, and `deploy.ps1` entrypoints stage these artifacts into the generated Flutter platform scaffold. Missing artifacts are a hard error. The repository does not silently download or substitute a Tor binary.

## Relay endpoint

Set one canonical relay onion endpoint before a real platform build:

```powershell
$env:TORCA_RELAY_ENDPOINT = '<56-char-v3-onion>.onion:443'
```

Alternatively create an untracked `release/relay_endpoint.txt` containing exactly the same `host.onion:port` value. Build tooling validates the v3 onion hostname and port and packages the value as:

- Windows: `relay_endpoint.txt` beside the executable;
- Android: `assets/torca/relay_endpoint.txt`.

The relay is used only for short-lived pairing rendezvous. Text, receipts, and attachments use direct authenticated P2P onion-service sessions after pairing.

## Release provenance

For an actual 0.1 release, record alongside the release artifacts:

- Tor upstream version;
- upstream download/source location;
- SHA-256 of every packaged Tor artifact;
- target ABI;
- build/reproducibility notes;
- reviewer/date approving the bundled version.

Do not store private keys, relay administrative capabilities, peer capabilities, database keys, or pairwise secrets under `vendor/`.
