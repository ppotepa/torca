# Communication transport

Torca separates **product/application protocols** from **network reachability**. Application/domain code consumes provider-neutral routing and peer byte-stream contracts; infrastructure owns the concrete provider.

Iroh is the sole production communication provider. Memory is a deterministic test implementation. Tor and the unfinished WebRTC adapter are retired from the active product graph.

## Provider matrix

| Provider | Production | Pairing/bootstrap | Peer traffic | Purpose |
| --- | --- | --- | --- | --- |
| Iroh | yes | QR/full-link commissioning using Iroh route material | messages, controls, attachments and Radio | production reachability |
| Memory | no | deterministic fixture | test-only | unit/integration/conformance tests |

There is no runtime provider picker in the product UI and no dynamic plugin loader. Production native composition builds the Iroh implementation behind neutral ports.

## Provider-neutral application contract

Upper layers work with concepts such as:

- stable `ProviderId` identity;
- opaque provider route/routing metadata;
- provider lifecycle/capability information; and
- peer byte-stream/transport factories used by the authenticated peer layer.

Upper layers must not depend on Iroh endpoint types, QUIC objects, relay protocol details, socket addresses or provider-specific serialized route formats.

Provider routes are reachability data, not Torca contact identity. Peer authentication/relationship capability remains authoritative after a transport connects.

## Responsibility split

| Responsibility | Owner |
| --- | --- |
| contact/relationship identity and approval | Torca domain/application |
| pairing cryptography and peer credential semantics | Torca application/crypto/protocol layers |
| durable delivery/retry/receipts/attachments | Torca application + repositories |
| authenticated peer-session/application framing | peer-link/application protocol stack |
| endpoint identity, provider route material, dial/accept | Iroh infrastructure |
| direct vs Iroh relay path selection | Iroh/provider configuration/runtime behavior |
| platform lifecycle and final composition | native/platform layer |

This split prevents a transport change from redefining the meaning of a contact or message.

## Iroh lifecycle

The Iroh component owns a persisted endpoint identity, publishes route/bootstrap material needed for commissioning, accepts incoming streams and dials persisted peer routes.

Route generations/freshness prevent stale discovery data from being silently treated as current. Network/provider route state can change while a durable relationship remains valid. Successful provider connection alone is not proof of the expected Torca peer; the authenticated peer layer verifies the relationship.

Runtime waiting is demand/event/deadline driven. The application should not add a parallel fixed keepalive/reconnect loop simply because Iroh is present.

## Iroh profiles

Current tooling recognizes Iroh profiles including:

- `always` — normal reachability profile used where broader incoming reachability/relay services are desired;
- `direct` — direct-oriented profile used when relay/discovery overhead is intentionally avoided and a usable direct route exists; and
- `local` — lab-only/local validation profile.

Profiles are reachability/deployment choices, not distinct Torca product providers. Exact service configuration is part of build/deployment metadata and should be read from source/tool help for the current implementation.

## Network privacy

Torca application-layer encryption protects content independently of the selected Iroh path, but Iroh is **not an anonymity network**.

A direct path can reveal network-location metadata to the paired peer and network observers. Iroh relay use changes which parties see particular network endpoints but does not create Tor-style anonymity or remove timing/traffic-analysis risk.

Security/privacy documentation must therefore distinguish:

- who can read/authenticate Torca content; from
- who can observe network location, timing, volume and reachability metadata.

See [`security/threat-model.md`](security/threat-model.md).

## Pairing/bootstrap

Pairing uses bounded invitation/bootstrap capabilities. The UI renders the invitation forms supported by the active Iroh composition; current production flow is QR/full-link based rather than a Tor-era onion/managed-relay model.

Pairing-service protocol/client code, where used by commissioning, is not the normal conversation message path and must not become a central account, presence or mailbox service.

## Adding a future provider

A provider is not production-ready until it:

1. implements the neutral provider/peer transport contracts;
2. owns its concrete routing/endpoint types entirely below the application boundary;
3. supports bounded commissioning/lifecycle behavior;
4. defines capability and network-metadata/privacy semantics;
5. passes `torca-provider-conformance` and relevant integration tests;
6. is wired through native/platform composition without provider branches in Flutter; and
7. has platform/network/device evidence appropriate to the release claim.

Until those conditions are met, Iroh remains the sole production provider and Memory remains test-only.
