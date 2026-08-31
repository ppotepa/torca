# Torca threat model

Torca is alpha software and has not received an independent production security audit. This model covers the active Iroh-only production architecture and identifies what Torca attempts to protect, what it cannot protect and when the model must be revisited.

## Scope

In scope:

- Windows/Android client hosts and the Flutter/native boundary;
- Rust application/runtime and durable state;
- protected secret storage and SQLCipher-backed structured storage;
- invitation/pairing and contact verification;
- Iroh direct/relay transport and provider route state;
- authenticated peer sessions and application protocols;
- messages, receipts, attachments and Radio Mode;
- lifecycle/background execution, notifications and diagnostics; and
- pairing-service protocol/client exchange used during commissioning.

Out of scope as guaranteed protection boundaries:

- a fully compromised OS/device/user account;
- a malicious authenticated recipient copying content;
- global anonymity against powerful traffic analysis;
- supply-chain/security properties not established by the checked-in build/dependency process; and
- guaranteed availability under network/provider/platform denial of service.

## Assets

High-value assets include:

- identity private keys and protected installation identity state;
- relationship/peer secrets and credentials;
- database keys;
- invitation/pairing capabilities before expiry/use;
- message, attachment and Radio plaintext;
- encrypted durable history and delivery state;
- contact verification decisions; and
- network metadata such as addresses, timing, traffic volume and route/reachability state.

## Adversaries

The model considers:

- an unauthenticated remote party sending malformed/bounded external inputs;
- a network observer or active network attacker;
- transport/relay/pairing infrastructure that can observe metadata, fail or deny service;
- a malicious or compromised paired peer;
- an attacker with partial local filesystem access but not all protected secrets; and
- a fully compromised local endpoint, which exceeds the application's confidentiality boundary.

## Trust boundaries

### Flutter and native/application boundary

Flutter is presentation and does not own durable/security state. The generated contract must contain presentation-safe projections and typed intent only. Secret key material and provider-private route bytes should not be exposed merely for rendering.

A native library/build mismatch must fail explicitly; silently substituting another runtime would cross the trust boundary and invalidate security assumptions.

### Local storage and platform secrets

SQLCipher protects structured local storage using a database key kept outside ordinary presentation state. Platform-protected secret stores reduce exposure of private keys/secrets at rest where the OS provides those facilities.

These controls do not protect plaintext after the process/OS is compromised or while the application legitimately decrypts data for use.

### Iroh transport

Iroh provides network reachability and transport security properties, but an endpoint/relay route is not Torca contact identity. Torca peer authentication and application-layer encryption remain authoritative above transport.

Direct paths can expose peer network-location metadata; relay paths change who observes endpoints but do not provide Tor anonymity. Iroh/relay/network infrastructure can deny service and observe timing/volume/routing metadata.

### Pairing/commissioning

Invitation/bootstrap material is a bounded capability. Parsing must be bounded and invalid/stale capabilities must fail. The creator explicitly approves/rejects before a durable relationship is established.

Pairing-service/commissioning infrastructure is not trusted as peer identity and must not become a long-term message mailbox/account authority.

### Remote paired peer

An approved peer is trusted to possess relationship credentials, not to obey local retention preferences. It can copy, export, screenshot or record received plaintext/media.

Contact verification helps users detect identity changes but cannot make a compromised remote endpoint trustworthy.

## Security controls/invariants

- Explicit approval before durable relationship creation.
- Peer handshake/authentication bound to approved relationship credentials/capability context.
- Application-layer authenticated encryption for peer payloads with bounded framing/input validation.
- Durable retry/deduplication/replay handling so duplicate/late outcomes are safe.
- Provider route generations/freshness handling so stale reachability data is not silently promoted.
- SQLCipher-backed structured storage and platform-protected secrets where supported.
- Security-sensitive identifiers/state generated/owned in Rust rather than Flutter.
- Diagnostics/logging designed to exclude payload plaintext and reusable secrets.
- Source/architecture policy that keeps concrete Iroh/provider code below neutral application boundaries.

## Key compromise and cryptographic limits

The current relationship-key design does not claim Signal-style forward secrecy or post-compromise security. Compromise of a long-lived relationship secret can therefore affect more than one message/session.

Any future ratchet/key-evolution design is a cryptographic architecture change requiring explicit design review, compatibility planning, negative tests and documentation updates; it must not be introduced as an incidental feature refactor.

## Availability and resource abuse

Externally supplied frames/invitations/attachments must be bounded. Retry/connect/probe behavior must be bounded and event/deadline driven to avoid remote or stale-state amplification into local CPU/network/battery exhaustion.

Healthy idle contacts must not cause unconditional reconnect/probe loops. Durable work survives transient provider failure; transport failure remains distinct from local data availability.

## Privacy/metadata threats

Application encryption does not hide traffic timing, traffic volume, peer reachability or direct network location. Diagnostics/performance traces can also leak operational metadata even when payloads are redacted.

Privacy reviews must therefore consider both content confidentiality and metadata exposure. See [`../../PRIVACY.md`](../../PRIVACY.md).

## Review triggers

Revisit this threat model when changing:

- provider or Iroh profile/reachability semantics;
- invitation/pairing/credential/verification formats;
- cryptographic algorithms/key derivation/key lifetime;
- wire/protocol compatibility or input bounds;
- storage epoch/migration/protected secret ownership;
- notification/capture/diagnostic behavior;
- attachment/Radio media protection;
- new server/account/discovery/backup/analytics services; or
- platform/native/generated boundaries.

A future production provider must document its metadata exposure and pass provider conformance plus required platform/network evidence before production composition.
