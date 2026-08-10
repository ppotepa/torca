# Contributing to Torca

Torca is under active development. The goal of the contribution rules is to keep one understandable cross-platform product rather than accumulate parallel implementations and documentation that describes old snapshots of the codebase.

## Source of truth

Use this order when information conflicts:

1. active code and generated schemas in `main`;
2. enforced source/architecture policies under `scripts/modules`;
3. the evergreen root documentation (`README`, `ARCHITECTURE`, `SECURITY`);
4. Git history for old implementation plans and architectural context.

Do not treat old commit messages, historical release plans or deleted batch trackers as current requirements.

## One-client rule

Torca has one shared Flutter application. Windows and Android are hosts/targets of that application, not independent products.

Do not introduce separate business workflows for a platform because a UI or lifecycle detail differs. Platform-specific code is appropriate for actual OS capabilities such as protected secrets, lifecycle, notifications, secure-window behavior, installation/device information and host integration.

## Where code belongs

### Foundation

Put only stable, dependency-light primitives in `crates/foundation`.

### Domains

Put product invariants and domain state in `crates/domains`. Domain code must not know about SQLCipher, Arti, Flutter, JNI, Win32 or other infrastructure/platform details.

### Protocols

Put bounded wire formats, framing and protocol validation in `crates/protocol`. Network DTOs are contracts, not substitutes for domain entities.

### Application

Put use-case coordination, policy, background orchestration interfaces and presentation-facing read models in `crates/application`.

The public presentation-facing application boundary is `torca-client-application`. New presentation workflows should go through it rather than reaching directly into storage or infrastructure.

The single-writer engine remains the consistency boundary for durable domain transitions. Runtime/background work belongs behind application-defined drivers and handles.

### Infrastructure

Put concrete SQLCipher, cryptography, Tor, file and network implementations in `crates/infrastructure`. Infrastructure may implement application/domain ports; application/domain code must not import infrastructure implementations.

Arti belongs only in `torca-tor`. SQL belongs in storage infrastructure. Security primitives belong in the crypto/security owners rather than presentation or contract code.

### Platform

Put contract serialization, native ABI composition and operating-system integration in `crates/platform`.

Platform conditionals should remain inside this boundary. The presentation contract may translate application commands/read models but must not duplicate application/security policy.

## Flutter boundary

Flutter should send user intent and render application read models. Keep the UI free of:

- database access or SQL;
- private/peer secret material;
- retry/outbox ownership;
- Tor or peer session ownership;
- duplicated application state machines;
- platform detection outside `lib/platform`;
- direct dynamic-library handling outside the FFI gateway/worker boundary.

Local presentation state is fine: focus, selection, dialog state, text controllers, responsive layout and local UI preferences do not require a global state framework.

## Reliability and privacy rules

Torca operates over a network that is expected to be slow, unavailable and reconnecting. New workflows should therefore have explicit durability/idempotency semantics when data must survive failure.

Observability should be payload-free or redacted by default. Avoid moving message bodies, onion addresses, Safety Numbers, capabilities or secret values into logs/metrics merely to make debugging easier.

Commands crossing the Flutter/native boundary should represent intent. Rust/application code owns security-sensitive identifiers, timestamps and durable state transitions.

## Public developer workflows

Use the root scripts rather than inventing new public build procedures:

```powershell
./scripts/build.ps1 -Target check
./scripts/run.ps1 -Target windows
./scripts/run.ps1 -Target android
./scripts/deploy.ps1 -Target windows
./scripts/deploy.ps1 -Target android
```

Build details belong in the script modules/tools behind these entrypoints.

## Validation

The normal validation path includes source architecture policy, generated contract consistency, Rust formatting/check/lints/tests, Flutter formatting/analysis/tests and platform builds where applicable.

A change should add focused tests at the lowest useful layer. Prefer deterministic application/integration tests for failure/retry/state-machine behavior and reserve real platform/Tor end-to-end validation for behavior that cannot be represented by a fake/controlled adapter.

Do not report a platform or release gate as validated unless it was actually executed.

## Documentation policy

Documentation should explain **why the system is shaped this way, who owns what, and how to work with it**. It should not mirror every source type.

Update documentation when:

- a layer or owner changes;
- trust/security boundaries change;
- a public workflow changes;
- product direction/non-goals change;
- a new deployable/service changes the system shape.

Do not create permanent documentation for temporary batch numbers, exact timeout values, current migration counts, generated field lists or short-lived refactor names. Code, tests, schemas and Git history are better sources for those details.

Per-crate public APIs should normally be documented with Rustdoc/source comments rather than parallel README files that drift independently.

## Change style

Prefer small coherent changes over framework-building. Introduce an abstraction when it clarifies ownership, isolates volatility or enables testing; do not introduce one only to make the folder tree look more architectural.

Before adding a new crate, ask whether it has a distinct owner, dependency direction and independent reason to change. Avoid generic `common`, `helpers`, `misc` or manager-style dumping grounds.

Security-sensitive or architecture-wide changes should update the relevant central document in the same change.