# Torca 0.3 — architectural boundaries

## Goal

0.3 closes dependency inversion between application and infrastructure/platform, returns use-case
policy and read models to application, and makes the canonical contract generated and typed. It is a
clean pre-release change: no v2 surface, compatibility aliases, or migration path is retained.

## Ordered implementation

1. Enforce the dependency matrix with `cargo metadata`; classify errors with foundation
   `ErrorDescriptor` and convert `requestId` to `CommandEnvelope` metadata at the ABI boundary.
2. Move the concrete Tor runtime driver to `torca-tor`; replace `CommunicationDriver` with narrow
   application-owned ports and map peer-link types only in infrastructure.
3. Centralize deterministic receipt IDs in `torca-receipts`; have application create an atomic
   read-state/outbox commit plan that SQLCipher executes without owning privacy or wire policy.
4. Add `torca-client-application` as the public application façade. It owns readiness, privacy,
   application projections, commands, queries, results, and events; `torca-client-engine` remains
   the transactional single-writer boundary.
5. Reduce `torca-contract` to schema, generated DTOs/codecs, and pure façade mapping. Generate Rust
   and Dart models from the canonical schema. Pairing URI encoding/parsing belongs to
   `torca-pairing-protocol`.
6. Make `torca-native` a single composition/ABI/JNI root that holds the application handle only.
   Activate `torca-presence` and `torca-notifications`; platform hosts consume observations and
   notification intents rather than own their policies.
7. Update Flutter feature controllers to use typed outcomes/events and raw URI submission, never
   snapshot-diff or string-state inference.

## Dependency policy

`domains` and `protocol` may depend only inward. `application` may depend on foundation, domains,
protocol, and application crates. Infrastructure implements application ports. Platform/native is the
only composition root. `torca-contract` is a wire adapter: it may map public façade values but may
not execute use cases or import runtime, bootstrap, probing, security algorithms, or domain internals.

Temporary exceptions in `Torca.ArchitecturePolicy.ps1` exist only for currently identified 0.3 debt
and must be deleted with the corresponding milestone.

## Acceptance evidence

Every milestone needs focused unit/integration tests plus the existing source policy, formatting,
Rust check/clippy/tests, contract generation drift, Flutter analysis/tests, and Windows/Android build
gates. Receipt tests must prove deterministic IDs, no receipt when policy disables it, exactly one
outbox record when enabled, and transaction rollback. Contract tests must prove generated enum and
typed error round trips. Flutter tests must prove pairing returns explicit IDs/events.
