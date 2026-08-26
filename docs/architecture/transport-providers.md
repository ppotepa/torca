# Interchangeable communication providers

Torca has one application protocol and one active transport provider per
deployment. The provider owns commissioning, pairing reachability and
authenticated peer bytes; handshake, encryption, delivery, attachments,
Radio and persistence remain provider-neutral.

## Selection invariant

`torca-deploy` stores one `CommunicationProvider` in the deployment plan and
artifact manifest. The value is compiled into the native client through an
explicit Cargo feature (`provider-tor`, `provider-iroh` or
`provider-webrtc`) and is also passed to Flutter as a build define. The
environment value is only a consistency check; it cannot link another
provider. Artifact manifests must record the exact feature set, and a missing
or mismatched feature fingerprint is rejected before install. This prevents
old multi-provider binaries from being reused accidentally.

Provider labels, readiness and deployment requirements are defined once as
`TransportKind::deployment_profile()` in `torca-transport-api`. Its single
`ProviderCommissioningService` declaration expresses whether a provider has no
commissioning service, uses a managed rendezvous, or expects external
rendezvous/signaling. Only a provider whose service requires one receives
`TORCA_PROVIDER_ENDPOINT` or requires an endpoint match when an artifact is
reused. A direct provider does not inherit a Tor relay configuration merely
because Tor was the first implementation.

The same profile owns commissioning budgets. Tor keeps a 15-minute native
bootstrap budget for slow directory startup, whereas Iroh uses a 45-second
budget and WebRTC 60 seconds. Deploy health validation uses the provider's
service budget as well, so an Iroh/local runtime never waits through a
Tor-sized warm-up timeout.

The native startup gate reads this same profile. A provider cannot be accepted
by the deploy wizard yet silently rejected (or substituted with Tor) by the
client process.

## Iroh power and network policy

Iroh owns one endpoint per application. The runtime sends provider-neutral
`network_changed` events to that endpoint so reachability evidence is reset and
the provider can migrate its sockets after Wi-Fi/LTE changes. The Iroh online
probe is demand-driven, single-flight, bounded to three attempts per network
generation and uses exponential backoff; after the third failure it waits for
a new platform event or a new reachability lease. Endpoint construction does
not start the probe, and it is never a startup gate for the local shell or
invitation creation.

`set_dormant(true)` suppresses new reachability probes and reports incoming
reachability as degraded while keeping the local runtime usable. The runtime
also exposes an explicit reachability-demand hint; Automatic/BatterySaver
background idle clears that hint, while foreground, AlwaysAvailable and
durable work may acquire it. Direct/local
profiles do not even start an `Endpoint::online()` task: they have no relay or
address-lookup service, so waiting for a home relay would be an infinite false
warm-up and an unnecessary battery wake source. Because Iroh 1.x does not
expose a mutable equivalent of Tor's `SoftDormant`, the provider owns a
replaceable endpoint slot. The relay-backed `always` profile closes the
endpoint during dormancy and rebinds it from the same protected secret on
resume; direct/local keep their already-cheap UDP listener bound so the opaque
endpoint route stored in contacts does not change merely because the app was
backgrounded. Existing relay-backed sessions are therefore closed deliberately,
while direct routes remain stable across the local sleep transition.

The Iroh Radio connector does not advertise datagrams until a datagram lane is
actually implemented. It also opts out of a duplicate application heartbeat;
QUIC/Iroh owns transport liveness. Legacy stream providers retain the bounded
application keep-alive.

Provider diagnostics expose a redaction-safe `energyClass` label. Iroh
`direct/local` reports `low`, Iroh `always` reports `medium`, and the Tor
and WebRTC lifecycles report their corresponding conservative class. This is
only a policy/soak label, not a physical battery measurement; mAh and charge
delta must still come from identical platform A/B runs.

Durable text delivery uses a provider-neutral batch boundary. The delivery
worker can claim several messages, groups them by contact, and submits one
batch to the authenticated peer session. Iroh writes all frames under one
stream lock and flushes once; envelope IDs and ACKs remain independent, so a
partial failure is retried through the existing durable outbox. Providers
without a native batching primitive use the compatibility default of one send
per frame.

For local battery/soak profiles `torca-deploy` accepts the provider-owned
profile directly, for example:

```powershell
cargo run -p torca-deploy -- deploy --communication-provider iroh --provider-profile direct
```

The accepted values are `always`, `direct` or `local` (with the legacy explicit
switches `TORCA_IROH_DISABLE_RELAY`, `TORCA_IROH_DISABLE_DISCOVERY` and
`TORCA_IROH_LOCAL_ONLY` still supported). These switches are opt-in because disabling relay/discovery also
disables reliable background reachability; direct/local are intended for
foreground LAN/controlled-network use and must be re-paired or refreshed after
an external address/network change if the persisted endpoint hint is no longer
dialable. The provider diagnostics expose `routeGeneration` separately from
endpoint identity, together with a typed `routeState` (`fresh`, `stale` or
`unavailable`), so a consumer can distinguish migration from a missing route
instead of silently retrying an old address. Production profiles should use a
Torca-owned relay/discovery configuration instead of the public N0 defaults
when privacy and operational control require it.

After authentication, a provider may send a provider-neutral `Route` frame
containing its current generation and opaque endpoint bytes. The peer link
validates the selected provider, rejects stale generations, and atomically
updates the contact route without interpreting those bytes. Iroh keeps QUIC
sessions alive during network migration and advertises the refreshed route once
the endpoint reports a fresh address; providers without migratable sessions
retain close-and-reconnect behavior.

While Iroh is migrating, `PeerTransportFactory::local_route_state()` returns
`ProviderRouteState::Stale` (the older `local_route_is_fresh()` boolean remains
available for compatibility). The generic peer link returns a retryable
`NotReady` result before
constructing an outgoing transport, and the provider wake retries after the
new route generation is available. This prevents stale local addresses from
being dialled during Wi-Fi/LTE transitions. Pairing bootstrap, pairing dials,
and Radio dials apply the same check, including a second check inside an
outgoing peer transport to cover the race between preflight and QUIC connect.
The pairing route source preserves this distinction: `Ok(None)` means that a
provider has not produced any route yet, while a stale route returns the
provider-neutral `runtime.route_refresh_required` error. UI and deployment
code expose that as an explicit provider route refresh or re-pair action
rather than keeping an invitation in an indistinguishable pending state. The
`provider.route.refresh` command is provider-neutral and is handled by the
selected lifecycle implementation.
The decoded inbound frame queue is bounded; overflow closes that transport
generation and leaves durable delivery to replay instead of allowing an idle
or backgrounded client to grow without limit.

The relay-backed profile accepts provider-owned service values at build time:

```powershell
$env:TORCA_IROH_RELAY_URLS = "https://relay.example/,https://relay-2.example/"
$env:TORCA_IROH_PKARR_URL = "https://lookup.example/pkarr"
cargo run -p torca-deploy -- build --communication-provider iroh --provider-profile always
```

`TORCA_IROH_RELAY_URLS` is a comma-separated list and
`TORCA_IROH_PKARR_URL` configures the matching publisher and resolver. These
values are embedded as non-secret build configuration by
`torca-transport-iroh/build.rs`; the deployer forwards them into every native
Cargo invocation. Credentials are intentionally not supported there.
When either custom value is present, the endpoint starts from the minimal
Iroh preset and never silently retains an unrelated N0 service. Direct/local
builds ignore both variables by contract and remain relay/discovery-free.

Session capability reporting is path-aware. Iroh reports `IrohDirect` or
`IrohRelay` from the currently selected QUIC path, rather than claiming that
every `always` session is relay-backed. A direct path obtained after hole
punching therefore lowers the observed transport cost without changing the
deployment profile.

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

Native packages are physically isolated. An Iroh APK/DLL is built with
`--no-default-features --features provider-iroh,radio-audio`, so it contains
no Arti/Tor implementation, onion service or Tor relay adapter. A Tor package
uses the corresponding `provider-tor` feature and does not link Iroh or
WebRTC. The deployer itself depends only on provider-neutral contracts and
does not link any concrete transport. `scripts/Verify-TorcaProviderIsolation.ps1`
and CI enforce these dependency-graph invariants. Native Cargo outputs are
kept under `target/providers/<provider>` so switching providers cannot reuse
or overwrite a library produced for another deployment.

An Iroh deployment has no Torca relay phase: the planner skips relay build,
compose, endpoint acquisition and onion readiness. Iroh may still use its own
QUIC relay fallback as part of the Iroh provider; that is separate from the
Tor-specific `services/relay` image. The relay image is therefore built only
for a Tor deployment and is not shipped in either client package.

The deployment plan itself is provider-neutral: it carries
`provider_service_build`, `provider_maintenance`, `provider_endpoint` and an
opaque `provider_profile`. The selected provider validates that profile (Iroh
currently uses `always`, `direct` or `local`) before any artifact is built.
Native metadata also exposes the canonical `providerProfile` for Iroh builds,
so diagnostics and the Flutter build-info surface can detect an artifact that
was compiled for a different reachability/energy policy than the deploy plan.
Those fields describe a managed provider service without requiring the runtime
or a future provider to inherit relay/onion semantics. Existing checkpoints
using `relay_build`, `onion` and `relay_endpoint` remain readable during the
migration; current plans serialize only the neutral names. The CLI keeps the
old flags as visible aliases, so automation can migrate deliberately instead
of breaking at the same time as the transport boundary changes.

The runtime never starts two providers for one session. A future fallback is
an ordered close-then-open operation, not a parallel connection.

The headless `torca-lab-peer` uses the same compile-time boundary as a client:
the soak orchestrator builds it with exactly one forwarded feature
(`provider-tor` or `provider-iroh`, plus `radio-audio`). A bare
`cargo build -p torca-lab-peer` keeps the Tor compatibility default, while
provider-specific soak runs never reuse that artifact.

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

The common media worker owns only protocol liveness. It schedules a heartbeat
using the provider's advertised `max_idle_interval_ms` (with a safety margin),
and each received `KeepAlive(sequence)` is answered once with the same
sequence. The response is idempotent, so it proves that both workers are
running without creating a ping-pong loop. A provider must still bound its
connect, read, write and flush operations; a socket that can block forever is
not a valid `RadioMediaStream` implementation. Provider disconnects are
reported as generic `RadioTransportFailure` values and never as Tor/onion
states. Media events use a dedicated Radio runtime wake source; they must not
be routed through text-delivery or peer-maintenance wakeups. This guarantees
that `Ready`, `FloorGranted`, `FloorDenied` and `Interrupted` transitions are
drained by `RadioCoordinator::maintain` immediately after provider activity.

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

The shared `torca-radio-adapters` crate intentionally contains no Tor
dependency. The onion/TCP implementation lives in `torca-radio-tor`; Iroh
uses its own factory in `torca-transport-iroh`, and future providers follow
the same split. This keeps provider selection from pulling unrelated stacks
into every client build.

Radio recovery is provider-neutral as well. `RadioMediaConnector` owns only
provider I/O and advertises bounded read/write and idle capabilities;
`RadioMediaAdapter` owns the worker, handshake, floor retransmission and
media event queue; `RadioCoordinator` owns consent, session generations and
the product-level floor guard. A provider must therefore map a closed stream
to a fatal read error (not a temporary timeout), wake the media worker when an
incoming stream is queued, and report whether its worker is alive. The
coordinator retries only failed session starts; an already established
generation is reconnected by the media worker, avoiding duplicate
`SessionOpen` races. A floor request can never remain in `RequestingFloor`
indefinitely: the provider timeout and the coordinator watchdog both produce
a terminal transition. `RequestFloor` is idempotent for the same operation ID
while a handshake or floor grant is pending; a replay cannot turn a valid
request into a false denial. If the provider worker dies, the shared adapter
publishes `WorkerUnavailable`, clears local floor/capture state and schedules
the same bounded session-recovery path for every provider.

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
