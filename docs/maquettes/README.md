# Torca UI maquettes

This directory contains the fast design/reference surface for the Torca `0.3` UX/UI track.

The maquette is intentionally **not** a second production frontend. Rust/domain/protocol code remains the source of truth for product behavior, the Flutter client remains the production UI, and this HTML/CSS/JavaScript application is the source of truth only for proposed visual hierarchy, interaction shape and responsive design while a `0.3` UI change is being designed.

## Open it

Open [`view/index.html`](view/index.html) directly in a browser. There is no npm install, framework, bundler, web server or network dependency. The application uses classic scripts so the normal development path also works from a `file://` URL.

The top development toolbar lets you switch:

- phone/tablet/desktop/wide/fluid viewport frames;
- deterministic application scenarios;
- Modern and Terminal theme variants;
- light/dark mode;
- compact/comfortable density; and
- Polish/English locale.

Press `~` to hide/show the toolbar.

## Structure

```text
view/
  index.html
  bootstrap.js
  core/         application host, router, store and base Screen class
  fixtures/     deterministic in-memory product states/scenarios
  components/   reusable shell/chat/icon/UI primitives
  screens/      class-based screen instances
  styles/       tokens, themes, layout, components and responsive rules
```

Every routed screen is an instance of a class derived from `Torca.core.Screen`. `Torca.core.Router` creates those instances from hash routes. `Torca.core.Store` is the one in-memory state container.

## Current routes

- `#/chats`
- `#/chat/:conversationId`
- `#/contacts`
- `#/contact/:contactId`
- `#/invitations`
- `#/settings`
- `#/diagnostics`
- `#/lab`

The app includes working mock interactions for sending/delivery/read transitions, replies, mock attachments, contact verification, pairing create/join/approval, settings/theme changes and scenario switching.

## Scenario philosophy

A scenario describes presentation state, not protocol behavior. For example the maquette may set a message to `delivered`, but JavaScript is not allowed to define the real production rules that make a message delivered.

Keep fixtures deterministic and add scenarios for visual/flow problems such as long text, offline/reconnect, transfer progress, pairing attention, identity change or empty data. Do not recreate Iroh, storage, pairing crypto, retry policy or durable state machines here.

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
