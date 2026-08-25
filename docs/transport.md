# Communication providers

Torca separates the application protocol from the network that carries authenticated peer bytes. A deployment selects exactly one communication provider; application/domain behavior remains shared.

## Current provider matrix

| Provider | Deploy selector | Commissioning | Pairing bootstrap | Messages | Attachments | Radio | Direct path |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Tor | selectable/default | managed rendezvous | managed session; QR/full link/short code | yes | yes | yes | no |
| Iroh | selectable | none | direct QR/full link | yes | yes | yes | yes |
| WebRTC | hidden | external signaling | external signaling | contract yes | contract yes | no | yes |
| Memory | hidden/test | none | test memory | yes | yes | no | yes |

The matrix is derived from `TransportKind::deployment_profile()` and `ProviderFeatures` in `torca-transport-api`. Normal deployment exposes only Tor and Iroh. WebRTC remains hidden until the host session/signaling bridge is ready; memory is reserved for simulated runtimes.

## Selection invariant

`torca-deploy` records the provider in the deployment plan/artifact metadata and compiles the matching provider selection into the native client. Native bootstrap checks the same provider profile used by deployment. A client must not silently start Tor when Iroh/WebRTC was selected, and the runtime never owns two active providers for one session.

Provider-specific deployment configuration is read only at provider composition. In particular, an Iroh deployment does not inherit Tor's rendezvous/onion endpoint just because Tor is the default provider.

## Provider-neutral contract

`torca-transport-api` owns:

- stable provider identity (`TransportKind`);
- deployment/capability metadata;
- `PeerTransport` / `PeerTransportFactory` byte-stream boundaries;
- commissioning stages/events; and
- provider bootstrap descriptors.

`torca-native::provider_composition` is the single concrete selection point. Each provider returns neutral runtime components:

```text
CommunicationLifecycle
PeerTransportFactory
ProviderPairingFactory
optional RendezvousProbe
RadioMediaSystemFactory
```

Everything above this point consumes those ports, not Arti, QUIC or DataChannel types.

## Shared behavior above transport

The selected provider does not redefine:

- contact/domain identities;
- peer authentication;
- application-layer payload encryption;
- durable message/control queues;
- receipts and deduplication;
- attachment state and encrypted local storage;
- conversation persistence/read models;
- contact verification; or
- Flutter navigation/product pages.

A provider changes reachability and commissioning, not the Torca relationship or message semantics.

## Tor

Tor uses embedded Arti, onion peer endpoints and the managed rendezvous service for pairing. The rendezvous service is ephemeral and untrusted; it does not become a normal message mailbox or durable conversation store.

Tor advertises QR/full-link/short-code pairing, incoming sessions, messages, attachments and Radio. It is the default provider for backward-compatible plans/artifacts.

## Iroh

Iroh uses a persisted endpoint identity and QUIC. Its composition owns endpoint binding, incoming/outgoing streams, direct pairing bootstrap and a provider-specific Radio media route.

Iroh does not require the managed Tor rendezvous service. Invitation creation can proceed when the local endpoint is bound; the join side performs the authoritative network attempt. Short-code-only pairing is not advertised because direct providers need bootstrap material carried by QR/full-link invitations.

Iroh is currently marked deployment-ready/selectable. This is a composition/deploy fact, not a claim that its network privacy is the same as Tor or that every real-device scenario has equal evidence.

## WebRTC

The WebRTC transport adapter consumes a reliable/ordered DataChannel that has already been negotiated by the host. SDP/ICE/STUN/TURN/signaling stays outside the shared runtime.

Native composition has explicit host ports for session channels and signaling. Until Windows/Android host implementations register both parts, normal deployment keeps WebRTC hidden and startup fails explicitly if a WebRTC composition is attempted without the required bridges.

Radio is not advertised for WebRTC in the current provider feature profile.

## Memory

The memory adapter provides deterministic/simulated provider behavior for tests. Native production composition rejects it.

## Pairing bootstrap versions

Provider bootstrap is separate from the encrypted relationship exchange. Managed-session invitations can use the compatibility format, while direct/signaling providers can carry a bounded provider bootstrap descriptor. The descriptor is short-lived route/discovery material; it does not replace the encrypted pairing offer or explicit approval as the durable source of truth.

## Provider-neutral UI/status

New UI and policy should use provider-neutral fields such as communication provider/state, endpoint summary and communication transport projections. Tor-specific state/onion compatibility fields may exist for older clients but must not become a new product dependency.

## Adding or opening a provider

Before exposing a provider in normal deployment:

1. implement the byte-stream/factory adapter;
2. provide lifecycle/commissioning events using neutral stages;
3. provide pairing bootstrap/rendezvous/signaling through the provider-owned pairing factory;
4. provide Radio media or explicitly advertise Radio unsupported;
5. wire native/platform composition without leaking provider-specific types upward;
6. add provider manifest/composition/pairing tests; and
7. complete the platform/device evidence appropriate for the release claim.

See [`ARCHITECTURE.md`](../ARCHITECTURE.md) for layer ownership and [`testing.md`](testing.md) for evidence language.