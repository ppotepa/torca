# Interchangeable communication providers

Torca has one application protocol and one active transport provider per
deployment. The provider owns commissioning, pairing reachability and
authenticated peer bytes; handshake, encryption, delivery, attachments,
Radio and persistence remain provider-neutral.

## Selection invariant

`torca-deploy` stores one `CommunicationProvider` in the deployment plan and
artifact manifest. The value is compiled into the native client through
`TORCA_COMMUNICATION_PROVIDER` and is also passed to Flutter as a build
define. A missing value is treated as `tor` for backwards-compatible
artifacts. A manifest built for another provider is rejected before install.

Provider labels, readiness and deployment requirements are defined once as
`TransportKind::deployment_profile()` in `torca-transport-api`. Its single
`ProviderCommissioningService` declaration expresses whether a provider has no
commissioning service, uses a managed rendezvous, or expects external
rendezvous/signaling. Only a provider whose service requires one receives
`TORCA_PROVIDER_ENDPOINT` or requires an endpoint match when an artifact is
reused. A direct provider does not inherit a Tor relay configuration merely
because Tor was the first implementation.

The native startup gate reads this same profile. A provider cannot be accepted
by the deploy wizard yet silently rejected (or substituted with Tor) by the
client process.

`PlatformServices` intentionally contains no relay or onion endpoint. Native
platform adapters provide only paths, protected stores, lifecycle facts and
optional platform-owned bridges. Provider-specific deployment configuration is
read at the composition boundary: currently only `provider_composition::tor`
interprets `TORCA_PROVIDER_ENDPOINT` as a v3 onion rendezvous address. This prevents a Windows or Android host from
accidentally carrying Tor configuration into Iroh or WebRTC.

Every deployment therefore has exactly one selected provider. The CLI/TUI
default is Tor. A provider becomes selectable only after its complete native
composition is registered. Tor and Iroh are selectable; WebRTC remains hidden
until its host session and signaling bridges are implemented. Real-device
validation is a separate release gate and is intentionally not performed by
the provider-neutral contract tests.

The deployment plan itself is provider-neutral: it carries
`provider_service_build`, `provider_maintenance` and `provider_endpoint`.
Those fields describe a managed provider service without requiring the runtime
or a future provider to inherit relay/onion semantics. Existing checkpoints
using `relay_build`, `onion` and `relay_endpoint` remain readable during the
migration; current plans serialize only the neutral names. The CLI keeps the
old flags as visible aliases, so automation can migrate deliberately instead
of breaking at the same time as the transport boundary changes.

The runtime never starts two providers for one session. A future fallback is
an ordered close-then-open operation, not a parallel connection.

## Provider boundary

`torca-transport-api` defines stable provider identity (`TransportKind`),
path/capability metadata, byte-stream/factory contracts and a provider-owned
`ProviderCommissioning` snapshot. Its stages are generic: local runtime,
incoming reachability and pairing rendezvous. A non-Tor provider therefore
never has to fabricate an onion-service state.
`torca-peer-link` owns the authenticated session lifecycle and accepts a
`PeerTransportFactory`; it no longer needs to know how a transport connects or
accepts an incoming stream. This is the extension point for Iroh, WebRTC and
deterministic test transports.

Native bootstrap selects exactly once through
`provider_composition::compose_selected_provider`; it does not construct Arti,
an onion listener, a relay health probe or a pairing driver itself. Tor owns
those details in `torca-native/src/provider_composition/tor.rs` and returns
generic runtime parts: lifecycle, peer factory, pairing factory, optional
rendezvous probe and radio-media factory. This is the required shape for each
future provider module. It prevents a selected provider from inheriting Tor
startup as an accidental side effect.

Radio is composed through `RadioMediaSystemFactory`. Common communication
assembly receives the factory, never a `TorServiceHandle`; Tor supplies its
onion/TCP implementation from the Tor provider module. An Iroh or WebRTC
provider must supply an equivalent media factory before Radio is enabled for
that provider. This keeps the radio protocol, audio pipeline and coordinator
shared while correctly making media reachability provider-owned.

Provider bootstrap callbacks are converted to the transport API's
`CommissioningEvent` before they reach the native host. The host therefore
renders `LocalRuntime`, `IncomingReachability` and `PairingRendezvous`, not
`TorBootstrapStage`; a provider can use a different implementation without a
second warming-up flow.

The native/Flutter snapshot exposes `communicationProvider`,
`communicationState`, `endpointSummary` and `transport.communication`.
`torState`, `onionAddress` and `transport.tor` are compatibility projections
for existing Tor clients only and must not be used by new UI or policy.
The same rule applies to contacts: `ContactRoute` stores opaque provider
endpoints in a provider-keyed map, while its legacy onion address is optional
and populated only for Tor relationships. Direct-provider contacts therefore
never carry a fabricated empty onion value through SQLite or Flutter.
Connectivity events likewise use `TransportLayer::Communication` and
`TransportLayer::PairingService`; the legacy `TransportLayer::Tor` and
`TransportLayer::Relay` map to those ledgers solely to preserve old probes.
Runtime probes use the same neutral vocabulary: `Communication`,
`IncomingReachability` and `PairingService`. Historical `Tor`, `OnionService`
and `Relay` probe targets are accepted solely for older diagnostic snapshots.

### Central composition contract

The native host has one provider-selection call:

```text
TransportKind + ProviderCompositionInputs
    -> ProviderComponents
```

`ProviderComponents` contains only these neutral ports:

```text
CommunicationLifecycle
PeerTransportFactory
ProviderPairingFactory
optional RendezvousProbe
RadioMediaSystemFactory
```

`ProviderPairingFactory` receives host-owned identity approval, peer-secret
storage, engine and connectivity observer through `ProviderPairingInputs`, and
returns a `PairingDriver`. It is therefore the provider—not `RuntimeOwner`—
that decides whether pairing uses Tor rendezvous, WebRTC signaling, Iroh
discovery or another approved mechanism. `RuntimeOwner` only schedules the
resulting generic ports.

Inside the pairing coordinator the exchange boundary is named
`PairingSessionServicePort`, not a relay port. It retains the small
`open/join/push/poll/ack/close` state machine while allowing a provider-owned
discovery or signaling implementation to satisfy it. The Tor adapter is only
one implementation of that port.

The exchanged frame is correspondingly `PairingSessionDelivery`. The old
`PairingRelayDelivery` Rust alias is retained only for source compatibility
with the existing Tor test adapter; it is not part of new provider code.

Pairing QR URIs have a compatibility-preserving bootstrap envelope. A normal
managed-session invitation remains `v=2`. A provider that must establish an
initial direct or signaling session uses `v=3` with a bounded
`PairingBootstrapDescriptor { provider, payload }`. That descriptor is
short-lived discovery material only: it never replaces the encrypted pairing
offer, which remains the source of truth for the durable peer route,
identity and authorization. This is the path reserved for Iroh direct-QR and
WebRTC external-signaling bootstrap.

The shared `torca-rendezvous-client` remains protocol- and transport-neutral:
it contains the pairing service client, generic `PairingServiceTransport` port
and framed request exchange only. `RelayTransport` remains only as a source
compatibility alias. The Arti/onion stream implementation is isolated in
`torca-rendezvous-tor`. A future direct-provider pairing adapter can therefore
reuse the coordinator and relay protocol without pulling Tor into its crate
graph.

`ProviderCompositionInputs::rendezvous_endpoint` is deliberately optional.
It carries a deployment-configured rendezvous endpoint for providers that need
one; direct providers can ignore it and obtain their own signaling settings
from their platform adapter. New code must not introduce Tor/onion arguments
into this contract.

The adapter crates currently present in the source tree are Tor, Iroh, WebRTC
and a deterministic memory adapter:

* `torca-transport-tor` exposes the existing onion stream as a provider;
* the Tor composition uses the same provider metadata and factory;
* `torca-transport-iroh` uses persisted endpoint addresses;
* `torca-transport-memory` is available for contract/SOAK tests.

The provider-neutral Iroh adapter is implemented in
`torca-transport-iroh`. It owns stable endpoint binding (`bind_endpoint`), a
bound-endpoint lifecycle and one QUIC
bidirectional stream: it reports local endpoint readiness immediately, reports
incoming reachability once Iroh becomes online, preserves bounded Torca payload
framing, wakes the runtime on inbound data and supports an incoming/outgoing
factory. The reusable `IrohComposition::bind` helper now owns persistent
endpoint identity, lifecycle and peer-factory construction. The
`IrohPairingService` and `IrohPairingServiceTransport` now provide the bounded
direct-QR slot service over the shared pairing coordinator port. Native Iroh
composition wires that service into `ProviderPairingFactory`; its endpoint
router also owns a dedicated `torca/radio/1` ALPN and supplies the common
Radio media worker over QUIC.
The deployment gate is open for the Rust composition and direct-QR contract;
real-device validation remains a release task and does not change the
provider-neutral runtime boundary.

Iroh commissioning deliberately separates local invitation creation from
incoming reachability.  A creator can generate and display a direct-QR
invitation as soon as the local Iroh endpoint is bound; discovery publication
is not a startup or pairing-creation gate.  A join operation performs the
authoritative network attempt and reports terminal input errors (missing or
mismatched bootstrap) without placing them in the retry queue.  This prevents
the old “saved locally until warming up” loop for a short code that cannot be
resolved by a direct provider.
Pairing offers carry only the provider wire name and
an opaque endpoint blob; version 3 is the current bootstrap envelope and
version 2 remains the managed-session compatibility format.
Pairing/contact SQLite persistence retains that endpoint map.

Creator pairing sessions also persist the short-lived slot reconstruction
metadata (code, expiry, ticket and creator public blob) inside the protected
pairing state. On restart the coordinator asks the selected provider to
restore the slot through `PairingSessionServicePort::restore_creator`. Iroh
recreates its process-local slot; Tor keeps the default no-op because its
authoritative slot lives in the managed rendezvous service. A provider that
cannot restore a local slot must fail the session explicitly rather than
reporting a healthy but unusable invitation.

`torca-transport-webrtc` provides the matching adapter over an already
negotiated reliable/ordered DataChannel. Android and desktop platform code
only needs to implement its small `WebRtcDataChannel` bridge; SDP/ICE/TURN
signalling remains platform-owned, while the negotiated opaque session hint is
carried by the pairing protocol and persisted with the contact.

Its `WebRtcLifecycle` and `WebRtcTransportFactory` are the corresponding
provider lifecycle/composition and `PeerLink` boundaries. The
platform provider owns negotiation and returns one channel per contact; the
factory guarantees that the peer link owns exactly one channel/provider at a
time.

`torca-transport-webrtc::WebRtcHostBridge` is the reference host adapter. A
platform SDK registers its opaque local hint, negotiated channels and a
bounded signaling callback on this bridge, then supplies the same `Arc` to
`with_webrtc_provider` and `with_webrtc_signaling_provider`. The bridge owns
only queues and lifecycle hand-off; it does not implement SDP/ICE/TURN or
pretend that an unnegotiated channel is ready.
Native hosts with one bridge owner may instead call
`torca_native::register_webrtc_host_bridge` once for both ports.

WebRTC selection is represented in deployment planning and pairing offers, but
is not deployment-ready. The native composition boundary is wired through two
explicit host ports: `PlatformServices::webrtc_session_provider` (or
`torca_native::register_webrtc_session_provider`) for negotiated DataChannels
and `PlatformServices::webrtc_signaling_provider` (or
`torca_native::register_webrtc_signaling_provider`) for the external signaling
exchange. The signaling adapter is translated into
the common `RendezvousClient`/`PairingSessionServicePort`; SDP, ICE, STUN/TURN
and platform callbacks remain outside the runtime. Both bridges must provide a
bounded opaque local signaling hint and negotiated channels. If either bridge
is not registered, native startup fails explicitly; it never silently
substitutes Tor while another provider is selected. The remaining work is
platform implementation and commissioning/real-device validation before
opening the deployment gate.

The native registry is scoped to a runtime generation: startup clears stale
bridges before registering host services, and shutdown clears them again. This
prevents an Android activity/process restart from reusing a dead DataChannel or
signaling object.

## Adding a provider

1. Implement `ProviderTransport` and a `PeerTransportFactory` in a dedicated
   infrastructure crate.
2. Implement a provider-owned `CommunicationLifecycle` that emits generic
   commissioning stages. Do not translate its state into onion terminology.
3. Implement `ProviderPairingFactory`; it must supply pairing route data as
   opaque provider endpoint bytes and own its rendezvous/signaling client.
4. Supply a `RadioMediaSystemFactory`, or explicitly declare Radio unsupported
   until an equivalent provider media route exists. Iroh uses its dedicated
   QUIC media ALPN; WebRTC remains unsupported until its negotiated media
   channel is available.
5. Add the provider to `compose_selected_provider` only after its platform
   adapter supplies required signaling/configuration.
6. Keep peer protocol, encryption, delivery, attachments, Radio coordinator,
   persistence and Flutter pages unchanged.
7. Add a manifest/build test, a commissioning test and a one-provider
   integration test before opening the deploy gate.
