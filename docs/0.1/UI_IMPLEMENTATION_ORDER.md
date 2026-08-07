# Torca 0.1 UI Productization

This document is the dependency-ordered implementation plan for the Torca 0.1 product/UI pass. It starts after the original 20 source batches. Those batches remain historically complete; this track productizes the shared client without reopening the core architecture.

## Rules

- Work lands directly on `main` as one focused commit per UI batch.
- `0.1_PROGRESS.md` is the canonical live tracker.
- Allowed states: `NOT STARTED`, `IN PROGRESS`, `SOURCE DONE / VALIDATION OPEN`, `BLOCKED`, `DONE`.
- A batch may be source-complete while owner/platform validation remains open.
- Flutter remains presentation only: rendering, responsive layout, navigation, input/presentation validation, local view state, themes, menus, focus and platform UI integration.
- Rust/runtime remains owner of domain state, identifiers and durable workflows, Tor, pairing, peer sessions, retries, persistence, cryptography, probing/PeerHealth, attachment transfer and diagnostics data.
- Do not add BLoC, Riverpod, feature-framework layers or separate desktop/mobile business implementations for this pass.
- Do not add notification message previews in 0.1. The Android service projection remains metadata-only and Windows notifications remain body-free.
- Connection quality is `PeerHealth`/`ConnectionQuality`, not radio signal strength. Probe traffic reuses the authenticated peer session; it must not create a fresh Tor connection per probe.

## UI-00 — Plan + tracking

Create this plan, register the track in `ROADMAP.md`, and add the live tracker to `0.1_PROGRESS.md`.

## UI-01 — Boundary + preferences

Wire `LocalPreferences` into startup and app composition. Keep only real presentation preferences for 0.1 (`themeMode`, `notificationsEnabled`). Remove temporary-export maintenance from Flutter and verify runtime ownership. Record bridge debt where Flutter currently supplies domain identifiers/timestamps; do not perform a speculative contract rewrite yet.

## UI-02 — Theme system

Add `theme/app_theme.dart`, `app_theme_mode.dart`, and semantic presentation colors. Support System/Light/Dark only. `MaterialApp` must use light/dark themes and the persisted theme mode.

## UI-03 — Settings + app shell

Add a small Settings screen with Appearance/Theme and Notifications/Enable notifications. Add one application overflow menu exposing New pairing, Your identity, Diagnostics, Settings and About Torca. Do not add empty future settings sections.

## UI-04 — Connection primitives

Extract reusable Tor/peer connection presenters and indicators from screen-private widgets. All screens must map the existing runtime connection states consistently without creating a Flutter state machine.

## UI-05 — PeerHealth backend

Implement runtime-owned peer probing over an already-authenticated peer session and aggregate a `PeerHealthSnapshot` containing state, quality, optional RTT, last successful probe, consecutive failures and reconnect attempt. Quality must consider session/reconnect/failure freshness in addition to RTT.

## UI-06 — Bridge v10

Bump the bridge contract to v10 and expose `PeerHealthDto`, preferably nested under `ContactDto`. Update schema, generated Rust/Dart projections, FFI/native handling, memory gateway and tests. Flutter must only render health computed by Rust.

## UI-07 — Connection UI/header

Add Connection Details and a reusable conversation header showing peer identity and connection quality. Contact Details and Tor status should reuse the same presentation primitives. Do not invent central presence/`last seen` semantics.

## UI-08 — Message bubbles

Replace raw conversation `ListTile` rendering with inbound/outbound message bubbles, reply quote, timestamp and compact lifecycle icons. Message lifecycle semantics remain runtime-owned.

## UI-09 — Context/actions

Introduce one reusable message action model. Touch uses long-press/bottom-sheet presentation; desktop uses right-click context menus. Add equivalent conversation/contact actions while preserving confirmations for destructive operations.

## UI-10 — Attachments

Polish attachment presentation with file icon, human-readable size/transferred bytes, progress and existing Open/Save/Retry/Cancel actions. Do not add image thumbnail processing in 0.1.

## UI-11 — Pairing polish

Keep the existing pairing workflow and improve its presentation with clear staged progress. Do not add a second pairing state machine in Flutter.

## UI-12 — Diagnostics

Place a human-readable health dashboard above developer/raw diagnostics. Preserve redaction and the existing export/self-test capabilities.

## UI-13 — Notifications/tray

Make `notificationsEnabled` control real Windows/Android notification behavior. Android native/background ownership must not depend on a live Flutter Activity. Expand the Windows tray with status and New pairing where supported. Keep notification bodies private in 0.1.

## UI-14 — Desktop/accessibility

Add desktop shortcuts, right-click behavior, focus traversal/indicators, Escape dismissal behavior, semantics, tooltips, text scaling and minimum interactive targets. Keep one responsive widget tree.

## UI-15 — Final audit

Perform a no-new-features final audit: presentation/runtime boundary, no duplicate desktop/mobile business implementation, no dead/stale bridge references, generated contract consistency, status mapping reuse and validation handoff. Update the canonical 0.1 release tracker.

## Dependency order

```text
UI-00
  -> UI-01
  -> UI-02 -> UI-03 -> UI-04
                         -> UI-05
                         -> UI-06
                         -> UI-07
                         -> UI-08 -> UI-09 -> UI-10
                                      -> UI-11 -> UI-12 -> UI-13
                                                          -> UI-14
                                                          -> UI-15
```

## Completion rule

Each UI batch lands as one commit on `main` and updates `0.1_PROGRESS.md` in that same commit. Source changes include focused tests where the repository has an executable test surface. If the environment cannot perform platform validation, mark the batch `SOURCE DONE / VALIDATION OPEN` rather than claiming `DONE`.
