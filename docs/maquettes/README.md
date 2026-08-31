# Torca UI maquettes

This directory contains the fast design/reference surface for the Torca `0.3` UX/UI track.

The maquette is intentionally **not** a second production frontend. Rust/domain/protocol code remains the source of truth for product behavior, the Flutter client remains the production UI, and this HTML/CSS/JavaScript application is the source of truth only for proposed visual hierarchy, interaction shape and responsive design while a `0.3` UI change is being designed.

## Open it

Open [`view/index.html`](view/index.html) directly in a browser. There is no npm install, framework, bundler, web server or network dependency. The application uses classic scripts so the normal development path also works from a `file://` URL.

The top development toolbar lets you switch:

- phone/tablet/desktop/wide/fluid viewport frames;
- deterministic application scenarios, including startup and first-profile states;
- Modern and Terminal theme variants, including Gruvbox, Tokyo Night and Catppuccin terminal palettes;
- light/dark mode;
- compact/comfortable density;
- Polish/English locale; and
- **LIVE / PAUSE** mock telemetry mode.

`LIVE` is a maquette-only simulation control. It drives fake LINK/TX/RX activity, bounded diagnostic log events and battery-observation counters. It is not a product networking mode. Press `~` to hide/show the toolbar.

## Structure

```text
view/
  index.html
  bootstrap.js
  core/         application host, router, store and base Screen class
  fixtures/     deterministic in-memory product states/scenarios
  components/   reusable shell/chat/icon/network/UI primitives
  screens/      class-based screen instances
  styles/       tokens, themes, layout, components, parity and responsive rules
```

Every routed screen is an instance of a class derived from `Torca.core.Screen`. `Torca.core.Router` creates those instances from hash routes. `Torca.core.Store` is the one in-memory state container.

## Current routes and surfaces

- `#/bootstrap` — startup/warm-up reference with provider monitor;
- `#/profile` — first local profile setup;
- `#/chats`;
- `#/chat/:conversationId` — conversation, LINK/TX/RX, Instant Contact, Radio, replies and transfers;
- `#/contacts`;
- `#/contact/:contactId` — relationship identity, Safety Number, shared files and contact controls;
- `#/connection/:contactId` — peer quality, RTT, reconnect state and route details;
- `#/invitations` — create/join/approval pairing flows;
- `#/identity` — local installation identity/build-safe presentation;
- `#/settings` — appearance, language, privacy, battery/availability, metered transfer, visual activity, audio and desktop preferences;
- `#/diagnostics` — Battery / Runtime / Logs / Incident engineering cockpit;
- `#/about`;
- `#/lab` — component/state and semantic-icon catalogue.

The normal shell intentionally exposes only Chats, Contacts and Invitations as primary navigation. Transfer Center and the application overflow menu live in the header; Identity, Diagnostics, Settings and About are secondary surfaces, matching the product hierarchy more closely.

## Network monitor

The maquette mirrors the production Ethernet-style transport monitor instead of reducing connectivity to one online dot. Provider and P2P rows each expose:

```text
LINK  TX  RX
```

`LINK` is steady/pulsing according to connection state. `TX` and `RX` are independent short activity flashes. The monitor is global in normal application chrome and is repeated in the conversation header and diagnostic cockpit where the real UI exposes transport context.

## Theme parity

The theme family changes product geometry as well as color:

- **Modern** uses restrained rounding, circular avatars and smooth outline icons;
- **Terminal** uses square avatars/controls/badges/LEDs, zero-radius cards, terminal typography and a more angular icon treatment.

The semantic icon catalogue follows the vocabulary of the production `TorcaIconSet` so a theme may change visual style without changing the meaning of actions or states.

## Diagnostics parity

The diagnostics mock is always reachable in the maquette even though the production app may gate it by build mode. It includes:

- Battery observation start/stop/reset and wake-source counters;
- Runtime provider/route/queue/peer telemetry plus LINK/TX/RX;
- bounded redacted log stream with level filtering and pause/resume;
- route refresh, transfer/build surfaces;
- self-test, incident marking and mock diagnostics export.

All values are fixtures. No diagnostic control invokes native code from the maquette.

## Scenario philosophy

A scenario describes presentation state, not protocol behavior. For example the maquette may set a message to `delivered`, but JavaScript is not allowed to define the real production rules that make a message delivered.

Keep fixtures deterministic and add scenarios for visual/flow problems such as long text, offline/reconnect, transfer progress, pairing attention, identity change, bootstrap, first profile or empty data. Do not recreate Iroh, storage, pairing crypto, retry policy or durable state machines here.

## Mapping to Flutter

Prefer semantic CSS variables (`--page-gutter`, `--message-out`, `--radius-lg`) and semantic icons/components. Approved decisions should map back to `packages/torca_ui` tokens/iconography and the corresponding Flutter screen/widget rather than being copied as arbitrary pixel values.

The `#/lab` screen is the component/state catalogue. It complements the full maquette: the lab is useful for comparing isolated variants; routed screens are useful for validating whole user flows and responsive hierarchy.

## Definition of useful

A maquette change is useful when it makes an implementation decision cheaper. It should answer questions such as:

- what is primary vs secondary information;
- what disappears first when width is constrained;
- where an action lives on touch vs desktop;
- how empty/loading/error/offline states look;
- which design token/icon semantic is required; and
- what should be reproduced in Flutter.

Do not add business behavior merely to make the mock feel more real.
