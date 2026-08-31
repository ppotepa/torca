# Torca 0.3 roadmap — UX/UI stabilization and polish

`0.3` is the release where Torca should stop feeling like a collection of implemented features and start feeling like one intentionally designed product.

The current product version remains whatever is declared by `release/version.json`; this roadmap does **not** itself bump the release version. The intended next product line is `0.3.x` once the release/versioning process reaches that point.

## Release theme

**Make the existing product coherent, calm, responsive and trustworthy.**

The priority is not adding large new features. It is fixing broken or confusing flows, reducing visual noise, making the conversation experience pleasant, aligning every surface with the selected theme, eliminating layout failures, and establishing enough visual regression coverage that later UI work stops re-breaking unrelated screens.

A useful shorthand for every `0.3` decision:

> primary content first; secondary state on demand; technical detail only when it helps the current task.

## Scope and non-goals

### In scope

- conversation/chat visual redesign;
- navigation, home, contacts, invitations and settings information architecture;
- iconography and theme consistency;
- responsive behavior across Android and Windows;
- broken, awkward or misleading UI flows;
- loading, empty, offline, error and recovery states;
- localization completeness and microcopy;
- accessibility, keyboard/focus behavior and reduced motion;
- visual/perceptual performance problems;
- UI regression tests and a deterministic component/state catalogue;
- small backend/runtime fixes only when they are required to make an existing UI flow correct.

### Out of scope unless needed to repair an existing flow

- groups;
- calls/video calls;
- multi-device synchronization;
- a second production communication provider;
- major cryptographic/protocol redesign;
- cloud backup/public discovery;
- large new social/product features.

## Priority model

| Priority | Meaning for 0.3 |
| --- | --- |
| **P0 — release blocker** | broken core flow, data/security ambiguity, red screen/overflow on a supported viewport, unusable control, invisible required state, or regression that prevents normal messaging/pairing/settings use |
| **P1 — required polish** | inconsistent hierarchy/theme/iconography, confusing state, awkward responsive behavior, missing localization/accessibility, visual clutter, or noticeably poor interaction quality |
| **P2 — delighter** | tasteful micro-interactions and convenience improvements that can be dropped without compromising release quality |

Do not spend P2 time while known P0 work remains.

---

# 0.3.0 milestones

## M0 — UI foundation and regression harness

**Goal:** make later visual work cheap and safe instead of fixing every screen independently.

Primary areas: `packages/torca_ui`, `apps/client/flutter/lib/theme`, shared widgets, Flutter tests.

### M0.1 — Define one responsive vocabulary — P0

Replace ad-hoc breakpoint decisions with shared layout classes such as:

```text
compact     phone / narrow window
medium      tablet / small desktop window
expanded    normal desktop
wide        desktop with optional context panel
```

The exact pixel thresholds remain implementation details, but they should be defined once and consumed by shell, home, conversations, pairing and settings.

Acceptance:

- resizing a Windows window never produces overlapping controls or clipped primary actions;
- Android portrait remains the first compact target;
- medium widths do not receive a desktop layout merely because they are wider than a phone;
- pane widths have meaningful minimums and degrade by removing secondary panels before crushing primary content.

Likely anchors:

- `apps/client/flutter/lib/widgets/adaptive_app_shell.dart`
- `apps/client/flutter/lib/screens/home_screen.dart`
- `apps/client/flutter/lib/screens/conversation_screen.dart`

### M0.2 — Expand design tokens — P1

`TorcaTokens` currently contains a small radius/spacing/list-height set while many screens still hard-code `8`, `12`, `16`, `24`, fixed widths and component-specific geometry.

Add semantic tokens where they remove repeated visual decisions:

- page/section/list gutters;
- control heights and compact/comfortable target sizes;
- conversation bubble radius/padding/gap/max width;
- composer geometry;
- pane widths/minimums;
- header height;
- state emphasis/opacity;
- animation duration/curve groups.

Avoid turning the token package into a dump of every number. A token should express a design rule shared by more than one surface.

### M0.3 — Iconography audit — P0/P1

Make `TorcaIconSet` the semantic icon contract and audit every user-facing icon.

Known issues from the current code:

- pinned conversations currently use the archive icon;
- muted state uses the normal notification icon rather than a muted/off semantic;
- terminal `invitations` maps to a camera;
- terminal `emoji` maps to a generic message icon;
- terminal `forward` currently reuses reply;
- PDF/text/document mappings are only loosely semantic;
- navigation expansion can show an ellipsis-style `more` icon rather than a clear sidebar expand/collapse affordance;
- the build footer uses an identity/shield-style icon for Torca itself.

Required result:

- semantic names such as `pin`, `muted`, `unread`, `verified`, `copyLink`, `scanQr`, `sidebarExpand`, `sidebarCollapse`, `microphone`, `voiceClip`, `radio`, `retry` and `offline` have intentional modern and terminal mappings;
- the same semantic state never changes meaning between themes;
- a theme family may change visual style, not product meaning;
- screens/widgets should not reach directly into unrelated icon families to patch missing semantics.

Anchor: `packages/torca_ui/lib/src/icon_set.dart`.

### M0.4 — Motion grammar — P1

The theme currently disables page transitions globally. Replace the binary "all transitions off" behavior with a small motion grammar:

- short fade/slide for route/context changes in normal mode;
- subtle state transitions for appearance, selection and status;
- no decorative continuous motion when hidden/backgrounded;
- `reduceMotion` disables non-essential transitions;
- terminal themes may use more discrete/stepped transitions without becoming visually noisy.

### M0.5 — UI Lab / deterministic state catalogue — P0

Create a development/test-only catalogue that renders important widgets without a real network/runtime state. It can be a test harness or dev-only route, but must not become a parallel business implementation.

Minimum catalogue states:

- chat bubble: inbound/outbound, grouped, long, failed, queued, delivered/read, reply, reaction, attachment, voice;
- conversation list item: unread, pinned, muted, draft, offline, blocked, radio-active;
- empty/loading/error/offline sections;
- pairing create/join/approval/expired/error;
- contact details/verification warning;
- settings controls;
- modals/context menus/snackbars.

The catalogue is the fastest place to visually compare Modern vs Terminal, light/dark and density variants.

### M0.6 — Golden/visual regression matrix — P0

Add deterministic visual coverage for high-value surfaces. Do not attempt to golden-test the entire app in every combination.

Baseline viewport set:

- narrow Android-like portrait;
- normal Android portrait;
- tablet/medium landscape;
- small desktop window;
- normal 1440-class desktop.

Baseline appearance set:

- Modern light;
- Modern dark;
- representative Terminal dark;
- compact density smoke coverage.

Every theme variant still needs a lightweight contrast/layout smoke test even if it does not get a full golden matrix.

---

## M1 — Conversation redesign

**Goal:** make the conversation the strongest-looking and easiest-to-use surface in Torca.

Primary anchors:

- `apps/client/flutter/lib/widgets/message_bubble.dart`
- `apps/client/flutter/lib/screens/conversation_screen.dart`
- `apps/client/flutter/lib/screens/conversation_widgets.dart`
- `apps/client/flutter/lib/widgets/conversation_header.dart`
- attachment/voice/reply/status widgets.

### M1.1 — Rebuild message bubble hierarchy — P0

The current one-to-one bubble renders a sender label (`You` / contact) inside effectively every bubble, adds relatively large vertical chrome and carries lifecycle information as its own mini-row. This makes ordinary text messages feel like cards rather than conversation.

Target behavior:

- no repeated `You`/contact label in normal 1:1 bubbles;
- visually group consecutive messages from the same direction when close in time;
- use tail/corner treatment only where grouping needs it;
- message text is the dominant element;
- timestamp + delivery/read state forms a quiet baseline/footer rather than a second content row;
- outbound/inbound colors remain readable in every palette without looking like two unrelated components;
- max width changes by responsive class, not a single `84%` rule;
- short messages remain compact instead of becoming wide cards;
- very long text, URLs, emoji-only messages and large font scaling do not break geometry.

### M1.2 — Message status language — P0/P1

Use one compact status grammar for queued/sending/sent/delivered/read/failed/cancelled.

- normal success states should be visually quiet;
- failed/retry states must be obvious and actionable;
- a tooltip/details surface can expose exact lifecycle timestamps on desktop instead of permanently displaying them in every bubble;
- delivery state must never be represented by an ambiguous icon borrowed from another semantic.

### M1.3 — Reply and reaction treatment — P1

- reply quote becomes a compact inset strip, not another large card inside the bubble;
- tapping the quote should jump to the referenced message where possible;
- unavailable original has an intentional muted state;
- reaction chips should visually attach to the bubble edge and occupy less vertical space;
- reaction layout must remain stable with many emoji and narrow widths.

### M1.4 — Attachments and voice as first-class message content — P0/P1

Unify attachment/voice message geometry with text bubbles:

- image/video preview gets media-first treatment;
- generic file rows share type icon, filename, size, progress and primary action hierarchy;
- transfer progress/error/retry is readable without opening diagnostics;
- voice clip play/progress/duration controls align with bubble theme and density;
- attachment-only messages do not reserve an empty text region;
- captions, if present in the current contract, should read as part of the message rather than metadata.

### M1.5 — Composer redesign — P0

The current composer can place emoji, attachment, text field, send and voice/Radio controls in one row. On compact widths this competes for space and makes the primary action unclear.

Target:

- text field is always the dominant width owner;
- use one compact leading `+`/attachment action or overflow cluster on mobile;
- show **Send** when there is sendable text/attachments;
- show voice/Radio primary action when the composer is otherwise empty;
- desktop may expose more actions directly when space permits;
- multiline input grows within a bounded height and then scrolls;
- Enter/Shift+Enter behavior is explicit and platform-appropriate;
- disabled/busy state does not shift the entire layout;
- attachment preparation has a compact preview tray with clear remove/retry state;
- keyboard/IME appearance never hides the primary composer action.

### M1.6 — Timeline behavior — P0

- preserve viewport when older history is prepended;
- show a floating `jump to latest` affordance only when useful;
- include unread/new count in that affordance when the user is away from the bottom;
- unread separator appears once at the correct boundary and remains stable while history loads;
- date separators have a subtle capsule/label treatment rather than reading like arbitrary body text;
- opening a conversation with unread content lands predictably;
- new inbound messages do not forcibly steal scroll position while the user reads older history.

### M1.7 — Search redesign — P1

The current search is essentially a field plus close button and can disable unrelated composer actions.

Add:

- result count/current result;
- previous/next navigation;
- highlighted result anchor;
- clear query action;
- sane empty/no-results state;
- closing search returns to the previous timeline position when possible.

### M1.8 — Conversation header — P1

Make the header about the person/conversation, not transport diagnostics:

- avatar + display name are primary;
- availability is a short human state;
- verification/identity-change warning is prominent only when necessary;
- connection/transport technical details move behind the info/details affordance;
- actions collapse into overflow as width shrinks;
- Radio state is visible when active without permanently dominating the header.

### M1.9 — Conversation gestures and desktop affordances — P2

After P0/P1:

- swipe-to-reply on Android;
- hover quick actions on desktop;
- double-click/select/copy behavior that does not fight context actions;
- optional subtle haptic confirmation for send/reaction/record boundaries on Android.

---

## M2 — Home, chat list and navigation

### M2.1 — Simplify conversation list items — P0/P1

The current row can simultaneously show timestamp, pin, mute, draft, Radio, connection indicator, contact info and unread badge. Reduce this to a predictable hierarchy.

Recommended hierarchy:

```text
avatar | name                         time
       | last message / draft         unread
```

Secondary states:

- pin/mute become small intentional badges near name/time only when active;
- online/offline should be expressed primarily through avatar/presence treatment;
- remove the always-visible info button from each row; info remains available through context menu/header/details;
- Radio only surfaces when it is actually active/attention-worthy;
- selected desktop row has a clear theme-aware selection state;
- draft should use text emphasis (`Draft:`) and optional icon, not three simultaneous signals.

Anchor: `apps/client/flutter/lib/widgets/conversation_summary_tile.dart`.

### M2.2 — Empty states that teach the next action — P1

Chats, contacts and invitations each need an intentional empty state with:

- one illustration/icon treatment from the active theme;
- one sentence explaining why the screen is empty;
- exactly one primary next action where appropriate;
- no technical provider terminology.

### M2.3 — Navigation shell cleanup — P0/P1

Current shell issues include a persistent build/service footer on mobile and ambiguous rail expand/collapse iconography.

For product builds:

- move build/service details to Diagnostics/About rather than consuming navigation chrome;
- retain concise dev metadata only when explicitly useful in development builds;
- use explicit sidebar expand/collapse icons;
- selected destination should be unambiguous in both Modern and Terminal themes;
- badges must not collide with icons at compact density or high text scaling;
- global runtime status becomes a small exceptional-state indicator, not constant technical telemetry.

Anchor: `apps/client/flutter/lib/widgets/adaptive_app_shell.dart`.

### M2.4 — Desktop master/detail behavior — P0

- remember useful pane widths locally;
- double-click divider or explicit action resets to sensible default;
- when width contracts, hide context panel before shrinking conversation below usable width;
- blank conversation pane gets a designed placeholder rather than dead space;
- selection survives resizing between narrow route mode and wide split mode;
- focus/keyboard traversal remains logical across list, conversation and context panel.

### M2.5 — Home information hierarchy — P1

The authenticated shell title currently often becomes the user's display name. Re-evaluate header hierarchy:

- product/location title should tell the user where they are;
- own profile identity belongs in profile/settings or a deliberate account affordance;
- transfer center/settings/diagnostics actions should not compete equally with primary navigation.

---

## M3 — Pairing, invitations and contacts

### M3.1 — Pairing flow becomes a guided flow — P0

The existing implementation has robust state handling but presents much of it through large modal surfaces.

Create/join should read as a short guided journey:

```text
Create: preparing -> invitation ready -> peer joined -> approval -> success
Join:   paste/scan -> validating -> waiting/approval -> success
```

Each step should answer:

- what is happening;
- whether the user needs to act;
- whether it is safe to close;
- how to recover if it fails.

### M3.2 — Invitation card redesign — P0/P1

For an Iroh invitation:

- QR is the hero element when available;
- copy/share full invitation are primary secondary actions;
- do not visually imply that a short code is independently usable when the active provider requires bootstrap material;
- show expiration as human relative time/countdown without a high-frequency timer;
- route-refresh-required state should have one clear recovery action;
- success should be visible long enough to understand what happened before a modal disappears.

### M3.3 — Scanner — P1

- full-screen/near-full-screen scanner on mobile rather than a small fixed dialog camera viewport;
- clear framing guide and instruction;
- permission denied/restricted states with recovery guidance;
- flashlight action when supported;
- desktop gracefully prefers paste/manual input rather than pretending scanning is equal on every host.

### M3.4 — Incoming approval — P0

Incoming contact approval must have one canonical visual surface and must never appear twice due to navigation/snapshot races.

The approval UI should prioritize:

- remote display identity;
- safety/verification context available at that stage;
- clear Accept / Reject actions;
- expiry/error state;
- no low-level route/provider bytes.

### M3.5 — Contact list cleanup — P1

- reduce Card-inside-list visual weight;
- use consistent row geometry with chat list;
- search/filter contacts when the list grows;
- blocked/pending/new states use badges/secondary text, not several competing icons;
- context actions are identical semantically on touch and desktop.

### M3.6 — Contact details and trust — P0/P1

Reorder contact details around user intent:

1. identity/avatar/name;
2. verification/Safety Number state;
3. messaging/Radio availability;
4. shared media/files;
5. connection details (advanced);
6. destructive actions.

Identity changed/unverified/verified states must be visually distinct and understandable without reading protocol terminology. Block/remove operations require explicit, differentiated destructive confirmation.

---

## M4 — Settings, diagnostics and microcopy

### M4.1 — Settings information architecture — P0/P1

The current settings page is one long list mixing appearance, battery, language, privacy, notifications, audio and desktop behavior.

Target categories:

- Appearance;
- Notifications & privacy;
- Battery & availability;
- Audio / Radio (where supported);
- Language;
- Desktop/platform behavior;
- Advanced / Diagnostics / About.

Compact layout can use a list of categories leading to sub-pages. Expanded desktop can use a two-pane settings layout.

### M4.2 — Remove hard-coded user-facing English — P0/P1

Current examples include navigation labels, appearance `Variant`, battery/availability copy and several build/diagnostic labels.

For `0.3`:

- every normal user-facing string goes through the localization system;
- technical raw strings may remain only where they are intentionally shown as diagnostics/protocol identifiers;
- Polish and English should both be complete enough that switching language does not leave mixed-language primary UI.

### M4.3 — Appearance settings — P1

- replace the large live preview with a compact representative preview or thumbnails that do not dominate settings;
- show theme family + palette/variant + light/dark/system + density as a comprehensible hierarchy;
- ensure Terminal is treated as a complete family, not just "different font + square corners";
- preview message bubble, controls, icon set and selection state, not only a contact row.

### M4.4 — Battery settings wording — P0

Battery/availability options affect actual reachability expectations. Make trade-offs explicit in user language:

- Automatic — recommended balance;
- Always available — higher background cost;
- Battery saver — background delivery may be delayed.

Do not require the user to infer the relationship between multiple switches/dropdowns. Disable or explain combinations that cannot have the requested effect.

### M4.5 — Diagnostics hierarchy — P1

Separate **user-facing connection health** from **engineering diagnostics**.

Normal users need short states such as:

- Connected;
- Connecting;
- Offline;
- Background limited;
- Action required.

Advanced diagnostics may expose provider profile, route generation, build IDs, counters and hashes behind an explicit details surface with copy/export actions. Redaction rules remain unchanged.

### M4.6 — Feedback grammar — P1

Standardize how Torca communicates outcomes:

- snackbar: small completed action (`Copied`, `Saved`, `Invitation copied`);
- inline state: recoverable form/flow error;
- modal confirmation: destructive or security-sensitive decision;
- banner: persistent app-level condition needing attention;
- progress indicator: only when work is actually ongoing.

Avoid snackbars for state that the user must remember after it disappears.

---

## M5 — Responsive, accessibility and platform polish

### M5.1 — Supported viewport audit — P0

Every primary flow must be manually and automatically exercised at the baseline widths from M0.

A Flutter overflow warning/red-yellow stripe on a supported viewport is a `0.3` release blocker.

Audit especially:

- conversation composer with keyboard visible;
- long contact names;
- long translated labels;
- settings segmented controls/dropdowns;
- pairing QR/modal/scanner;
- message action menus;
- attachment tray;
- desktop split panes;
- notification/runtime banners.

### M5.2 — Text scaling — P0

Core flows must remain usable at large text scale (target at least 200% for primary screens). Do not solve overflow by globally disabling text scaling or shrinking important text below readable size.

### M5.3 — Touch targets and compact density — P0/P1

Compact density may reduce whitespace, but primary touch controls on Android still need usable targets. The current theme can shrink Material tap targets globally; audit touch behavior separately from desktop compact density.

### M5.4 — Keyboard/focus/shortcuts — P1

Windows target:

- visible focus indication;
- predictable tab order;
- Escape closes transient surfaces before merely unfocusing arbitrary controls;
- conversation send/newline shortcut documented and tested;
- search shortcut;
- navigation shortcuts do not fire while typing where inappropriate;
- context menus work with keyboard as well as mouse.

### M5.5 — Accessibility semantics — P0/P1

- icon-only actions have useful labels/tooltips;
- message semantics do not redundantly read decorative status icons;
- unread/verified/blocked/offline state is not color-only;
- QR/invitation screen provides a non-visual copy/share path;
- progress semantics include meaningful state;
- decorative avatar animation does not pollute screen-reader output.

### M5.6 — Contrast audit — P0

Check all Modern and Terminal variants in light/dark for:

- body text;
- muted metadata;
- inbound/outbound bubbles;
- selected rows;
- disabled controls;
- error/warning/success;
- badges and focus indication.

A palette may be attractive and still be rejected if semantic contrast is insufficient.

### M5.7 — Platform-specific polish — P1/P2

Android:

- system bars and keyboard insets match theme;
- scanner/microphone permission flows are native-feeling;
- back gesture/back button closes the correct layer;
- optional haptics only for meaningful action boundaries.

Windows:

- minimum/resized window behavior;
- hover/focus/context menus;
- tray labels and icons use current product vocabulary;
- no development metadata in normal navigation chrome;
- window restoration preserves reasonable layout state.

---

## M6 — Broken-flow sweep and release polish

### M6.1 — State matrix walk-through — P0

For every primary surface, explicitly test:

```text
empty
loading
normal
busy
success
offline/degraded
recoverable error
terminal error
blocked/restricted
```

If a state currently falls through to a blank widget, stale snapshot, generic exception string or disabled control with no explanation, it is part of `0.3`.

### M6.2 — Core journey acceptance — P0

Minimum user journeys:

1. first launch / profile setup;
2. create invitation;
3. scan/paste and join invitation;
4. incoming approval/rejection;
5. contact appears without manual refresh;
6. open chat;
7. send text and observe queued/sent/delivered/read states;
8. reply, search and jump around history;
9. send/open/cancel/retry attachment;
10. record/play voice clip and use Radio where enabled;
11. offline -> reconnect -> queued work resumes;
12. block/unblock/remove contact;
13. change theme/density/language;
14. change battery/availability mode with understandable consequences;
15. minimize/background/restore on supported platforms;
16. resize Windows through compact/medium/expanded/wide states without losing selection or corrupting layout.

### M6.3 — Copy and terminology pass — P1

Remove implementation vocabulary from normal UI where it does not help the user:

- provider service;
- commissioning;
- route generation;
- endpoint hashes;
- runtime terms;
- internal status enum wording.

Keep it in Advanced Diagnostics where it is useful.

Prefer short verbs and state language:

- `Try again` instead of implementation-specific retry text;
- `Reconnect` only when the user action actually reconnects;
- `Copy invitation` / `Share invitation` rather than generic `Copy` near several values;
- `Waiting for approval` rather than a raw pairing state.

### M6.4 — Perceptual performance — P0/P1

A visually polished release must also feel calm:

- avoid rebuilding large home/conversation trees for unchanged snapshots;
- avatar/visual activity remains suspended when invisible/backgrounded according to policy;
- large image previews use appropriately sized thumbnails rather than decoding huge originals into small cells;
- opening Settings/theme preview should not produce visible stalls;
- long chat/contact lists remain virtualized;
- no spinner should run indefinitely for an operation that has already failed or become idle.

This milestone complements, not replaces, the runtime/battery work documented elsewhere.

### M6.5 — Final visual QA — P0

Before calling `0.3` UI-complete:

- capture the golden baseline from the final intended styles;
- run the baseline viewport/theme matrix;
- perform one Windows and one physical Android manual pass;
- check Polish and English;
- check light/dark and Modern/Terminal;
- check compact/comfortable density;
- check large text scale;
- verify no primary icon changes semantic meaning between themes;
- verify screenshots do not expose development-only metadata in product chrome.

---

# P2 delighters — only after the release is clean

These are intentionally optional. They are candidates, not release requirements.

- subtle generated chat background texture/pattern derived from the active theme, with a plain-background option and zero external assets/networking;
- hover-revealed message actions on desktop;
- Android swipe-to-reply;
- small send/reaction haptics;
- smooth avatar/status transition rather than abrupt presence dot changes;
- command/search shortcut overlay on desktop;
- drag-and-drop attachment affordance on Windows;
- paste-image-to-attach flow;
- tasteful skeleton placeholders for short local loading windows where they improve perceived continuity;
- conversation selection animation in expanded desktop layout;
- theme-aware QR framing and scanner overlay;
- an About surface with version/build/license information moved out of navigation chrome.

A delighter is rejected if it increases idle work, harms privacy, makes Terminal and Modern behavior diverge semantically, or creates a new durable state machine in Flutter.

---

# Suggested implementation batches

The roadmap is designed to be executed in reviewable batches rather than one visual rewrite.

| Batch | Focus | Expected result |
| --- | --- | --- |
| **0.3-A** | M0 foundation | responsive classes, icon contract, tokens, UI Lab/golden harness |
| **0.3-B** | M1 conversations | bubbles, composer, header, timeline, search, attachments/voice |
| **0.3-C** | M2 navigation/home | simplified chat list, shell, master/detail, empty states |
| **0.3-D** | M3 pairing/contacts | guided invitations, scanner, approvals, trust/details hierarchy |
| **0.3-E** | M4 settings/diagnostics | category IA, localization, battery wording, feedback grammar |
| **0.3-F** | M5 responsive/accessibility | overflow, text scale, keyboard/focus, semantics, contrast, platform polish |
| **0.3-G** | M6 release sweep | state matrix, broken flows, perceptual performance, final visual QA |

If an early batch exposes a genuine functional bug, fix it in that batch instead of documenting around it.

# Definition of done for 0.3

`0.3` is ready for release-candidate versioning only when all of the following are true:

- no known P0 item in this roadmap remains open;
- core messaging and pairing journeys are usable without technical knowledge of Iroh/runtime internals;
- chat bubbles/composer/list are visually coherent in Modern and Terminal themes;
- no supported baseline viewport has a known overflow or inaccessible primary control;
- primary UI is fully localized in English and Polish;
- primary flows remain usable with large text scaling and keyboard/touch input appropriate to platform;
- semantic icon meanings are consistent across theme families;
- deterministic visual regression coverage exists for the highest-risk surfaces;
- error/offline/retry states are intentionally designed rather than incidental Flutter defaults;
- Windows and physical Android receive a final manual UI pass;
- `CHANGELOG.md` is updated with the actual implemented subset, and release metadata is bumped only through the rules in `versioning-and-releases.md`.

This file is the single maintained release roadmap. Detailed temporary implementation checklists should live in issues/commits and be removed/closed when finished rather than creating additional competing roadmap files.
