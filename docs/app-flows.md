# Application flows

This page describes current user/runtime journeys in terms of ownership. Exact DTO fields, screen names and protocol bytes remain source contracts.

![Torca application flows](diagrams/app-flows.svg)

## Startup

```text
Flutter process starts
  -> load presentation preferences
  -> open FfiEngineGateway
  -> initialize torca-native / process-owned Rust runtime
  -> open/validate local durable state
  -> compose the production Iroh provider
  -> build the first application projection
  -> Flutter decodes successful initialization
  -> send lifecycle: flutter_gateway_ready
  -> attach deep links + platform lifecycle/notifications/desktop integration
  -> render application UI
```

`flutter_gateway_ready` is the application-level readiness boundary used by the client host. Local/application readiness and network reachability are deliberately different states: usable encrypted local history must not disappear behind Iroh reachability warm-up or temporary network degradation.

If native initialization fails, the client shows an explicit retryable startup failure. It does not silently substitute an in-memory business implementation.

## Create an invitation

```text
user chooses create invitation
  -> Rust application creates pairing session
  -> Iroh provider publishes bounded bootstrap/route material
  -> UI renders supported QR/full-link invitation
  -> remote joins using the invitation capability
  -> encrypted pairing exchange authenticates relationship material
  -> creator receives explicit approval/rejection decision
  -> accept -> durable relationship/contact persisted
  -> reject/cancel/expire -> no relationship
```

Invitation/bootstrap material is a commissioning capability, not the durable identity of a contact. Expired/stale route material must not be treated as a successfully paired peer.

## Join an invitation

The add-contact action and a pairing deep link converge on the same application-owned join flow.

```text
user opens add contact OR app receives pairing link
  -> shared join UI parses bounded invitation input
  -> Rust application starts/joins pairing session
  -> Iroh establishes commissioning/bootstrap transport
  -> encrypted pairing exchange
  -> remote creator makes explicit decision
  -> accepted relationship/contact persisted
  -> application projection changes
  -> Flutter renders the durable result
```

Flutter does not infer pairing success because a dialog closed or a network request returned. The persisted relationship projected by Rust is authoritative.

## Open a conversation

Navigation carries a conversation identifier. Flutter asks the Rust facade for paged conversation state/history and search results; it does not load and locally filter the entire durable history as the normal path.

## Send a message

![Message delivery ownership](diagrams/message-delivery.svg)

```text
composer intent
  -> typed EngineGateway command
  -> application validates intent
  -> durable message/outbox state is persisted
  -> runtime delivery owns retry/demand
  -> authenticated peer session + application-layer encryption
  -> provider-neutral peer byte stream
  -> Iroh direct/relay transport
  -> remote validates/decrypts/deduplicates/persists
  -> acknowledgement/receipt returns
  -> local durable state + projection update
  -> Flutter renders status
```

Network failure leaves retry ownership in Rust. Flutter is never the durable outbox.

## Read receipts

Read-receipt preference is synchronized with the runtime. When enabled, application-owned read state creates protocol/control work as required. Flutter renders the resulting projection; it does not fabricate delivered/read success from UI visibility alone.

## Attachments

```text
user selects source file
  -> Flutter submits explicit path/user intent
  -> Rust validates capability/limits and imports application-controlled state
  -> encrypted/resumable transfer uses the paired peer boundary
  -> progress/cancel/resume remain runtime-owned
  -> recipient persists transfer state
  -> explicit user open/export creates user-visible output
```

Generated/runtime capabilities are authoritative for limits and supported operations; Flutter must not hard-code a second protocol capability model.

## Radio Mode

Radio Mode is experimental, mutual-consent and half-duplex.

```text
user enables/accepts Radio with contact
  -> application owns consent/session/floor state
  -> platform obtains microphone permission before capture
  -> application owns burst/floor rules and key derivation
  -> Iroh-backed media transport carries encrypted media
  -> release/background/session close stops capture/transmission as required
```

Radio is not a separate contact identity or provider. It runs within the approved relationship and shared runtime lifecycle.

## Notifications and deep links

Runtime events are exposed through the narrow generated/event boundary. Android notification routing, deep links and Windows host interactions translate OS-originated actions into the same application intents/navigation requests used inside the client. Platform code does not become a second message/contact database.

## Background and connectivity lifecycle

Platform lifecycle changes feed the Rust runtime. Runtime policy combines lifecycle, durable demand, communication evidence and deadlines to decide what work should remain active. Idle presentation polling is not a correctness dependency.

A route/network change can degrade or refresh Iroh reachability without invalidating the durable Torca relationship. Stale provider routes are not trusted as peer identity; authenticated peer/session checks remain authoritative.

## Settings

Presentation-only preferences stay in Flutter/platform preferences where appropriate. Settings that affect runtime/security/background behavior are synchronized through runtime commands and reflected back through authoritative projections.

## Diagnostics

Diagnostics expose bounded operational state such as provider health, queues, runtime/power counters and redacted errors. They must not intentionally contain message/attachment plaintext, Radio audio, private identity keys, database keys, relationship secrets or reusable pairing capabilities.

See [`operations.md`](operations.md) for incident/recovery workflow.
