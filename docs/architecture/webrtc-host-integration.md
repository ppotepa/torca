# WebRTC host integration

The Rust runtime intentionally has no dependency on an SDP, ICE, STUN, TURN,
or platform WebRTC SDK. Android and Windows provide those details through the
`WebRtcHostBridge` and the provider-neutral traits in
`torca-transport-api`.

## Ownership

The platform host owns:

1. the WebRTC factory and peer connections;
2. signaling transport and authentication to the signaling service;
3. ICE gathering, ICE restart, STUN/TURN configuration, and network callbacks;
4. the concrete `RTCDataChannel` implementation;
5. microphone/media APIs (if a future media provider is enabled).

The Rust runtime owns:

1. Torca identity and pairing protocol state;
2. framing, acknowledgements, retries, and durable delivery;
3. provider-neutral route/readiness projection;
4. runtime wake-up and lifecycle policy;
5. closing stale channels after a route generation changes.

## Required adapter methods

Implement `WebRtcDataChannel` for every negotiated reliable, ordered
DataChannel. `send` must apply the SDK's binary send operation, while
`try_receive` must be non-blocking. The SDK's `onMessage`, `onOpen`,
`onClose`, and `onError` callbacks must wake the registered callback from
`set_waker`.

Implement `WebRtcSessionProvider` on one process-wide host bridge:

* `local_endpoint_hint()` returns a bounded, opaque signaling descriptor;
* `connect(contact)` returns only an already negotiated channel;
* `accept()` drains channels reported by the SDK's incoming callback;
* `commissioning()` reports local runtime, incoming reachability, and route
  state from the current ICE generation;
* `refresh_route()` starts an ICE restart or a new offer/answer exchange;
* `set_waker()` must be called before starting the runtime.

Construct the bridge with `with_signaling_reconnect(...)` when the platform
SDK owns a reconnectable WebSocket/HTTPS signaling session. The callback is
invoked by the common pairing transport; it must only reopen signaling and
must not claim a fresh route until the SDK callback has supplied new ICE
candidates through `set_commissioning`.

The existing `WebRtcHostBridge` provides the registry, stale-route transition,
bounded incoming queue, channel cleanup, and waker propagation. A concrete SDK
adapter should call `bind_contact` after a contact-specific DataChannel is
open and `push_incoming` for an unsolicited authenticated incoming channel.
Call `unbind_contact` on terminal close. Do not leave closed SDK channels in the
registry.

## Route and commissioning state machine

```text
Starting
  -> Gathering
  -> Connecting
  -> Connected
  -> Ready

Ready -- network change / refresh_route --> RouteStale
RouteStale -- SDK callback with new ICE generation --> Gathering
RouteStale -- timeout/error --> Degraded or Failed
```

The host must never publish `Fresh` for the old generation after
`refresh_route()` returns. The bridge marks the route `Stale` and closes old
channels before the SDK starts renegotiation. Once the SDK has a usable
DataChannel and current candidates, call `set_commissioning` with a new
generation and `ProviderRouteState::Fresh`.

## Signaling

`WebRtcSignalingProvider::exchange` carries bounded opaque bytes only. The
host may use WebSocket, HTTPS, or another signaling service. The Rust pairing
protocol must not parse SDP or ICE candidates. `reconnect()` should reopen the
signaling session; it must not fabricate a ready route. If signaling is not
registered, native composition fails explicitly and never falls back to Tor.

## Failure and retry rules

* DataChannel close: call `unbind_contact`, publish `Degraded`, and wake Rust.
* ICE failure: publish `RouteStale` or `Failed` with a retryable diagnostic.
* Network callback: call `refresh_route`; do not reconnect every contact in a
  loop.
* Backpressure: return an error from `send`; the common delivery layer owns
  retry and durable queueing.
* Runtime teardown: call `clear_channels` before releasing the SDK peer
  connection objects.

## Validation checklist

Before enabling a WebRTC deployment profile, the host adapter must prove:

* an offer/answer exchange completes on two processes;
* direct ICE works on the local test network;
* TURN fallback works when direct UDP is blocked;
* a network change invalidates the old route and recovers one generation;
* incoming and outgoing channels wake the Rust runtime;
* a closed channel cannot be returned by a later `connect` call;
* pairing, text, attachments, and acknowledgements use the same channel;
* no WebRTC build links Tor-only crates or expects an onion endpoint.
