# Cleanup 01–40 validation handoff

The cleanup series is intentionally maintained as one commit per numbered point. The source changes, guardrails and validation harnesses live in the repository; validation that requires a Rust/Flutter toolchain or a physical Android device must be executed in that environment and must not be inferred from source inspection.

## Local automated gate

Run the complete source/workspace/package/policy gate:

```powershell
./scripts/Validate-TorcaCleanup.ps1
```

For release-oriented source validation also run the security preflight and a deterministic repeated test pass:

```powershell
./scripts/Validate-TorcaCleanup.ps1 -Security -SoakIterations 25
```

The cleanup-sensitive package runner covers runtime, client engine, native bridge, peer link, pairing coordinator, SQLCipher storage, communication adapters, Tor lifecycle and runtime policy.

## Physical-device evidence

Battery and connectivity checks remain explicit because they depend on Android hardware, radio conditions and OS behavior:

```powershell
./scripts/Run-TorcaBatterySoak.ps1 -DurationMinutes 60
./scripts/Run-TorcaConnectivitySoak.ps1 -Iterations 10
```

For release evidence, extend the battery run to 6–8 hours and preserve the generated `artifacts/soak/` output together with device/build/network metadata.

## Residual items that are not hidden by this cleanup

- A real two-device end-to-end messaging soak over Tor remains stronger evidence than deterministic peer-link tests.
- Battery and connectivity acceptance require physical-device runs; the repository only provides the harness here.
- The security preflight is not a third-party audit or protocol proof.
- The current pairwise-secret design must not be represented as Double Ratchet, forward-secret messaging or post-compromise security until such a protocol is separately designed, implemented and reviewed.
- Relay scalability should be changed only from measured load/latency/resource data, not from cleanup assumptions.

No GitHub Actions or CI workflow is required by this series. All commands are explicit local entrypoints so the same checks can be run on developer/release machines without coupling the architecture to CI configuration.
