# UI1 parity checklist

This is the manual smoke matrix for the `view/` reference surface. It is intentionally deterministic and does not claim production behavior.

## Navigation and appearance

- [ ] VIEW selector reaches chats, Alice chat, contacts, invitations, settings, diagnostics, UI Lab, bootstrap and profile.
- [ ] PLATFORM selector exposes Desktop, Android and iOS geometry hints.
- [ ] Fluid, phone, tablet, desktop and wide viewport presets render without horizontal overflow.
- [ ] Modern/Terminal, light/dark and comfortable/compact combinations preserve semantic icon meaning.
- [ ] Genome avatar placeholders remain deterministic per contact, legible in every theme and square in Terminal geometry.
- [ ] `~` hides and restores the development toolbar.

## Conversations

- [ ] Search opens from the chat list and conversation header, reports result count, supports clear and selects a result.
- [ ] Right-click/context action menu works for conversations, contacts and messages; compact view presents an action sheet.
- [ ] Reply, reaction, edit, forward, copy, details, retry and delete states are reachable from a message.
- [ ] Older history can be loaded, the timeline preserves its approximate scroll position, and jump-to-latest appears away from the bottom.
- [ ] Composer grows for multiline input, sends on Enter, preserves Shift+Enter, switches to emoji/voice state when empty, and exposes attachment preparation/remove/send.
- [ ] Attachment progress is visible in the message and Transfer Center; pause/resume/cancel/complete states are represented.

## Pairing, identity and diagnostics

- [ ] Create invitation shows QR/link; join supports pasted link and scanner overlay; awaiting pairings support approve/reject.
- [ ] Bootstrap retry/ready and first-profile validation are visible in startup/profile scenarios.
- [ ] Identity-change scenario blocks normal sending until verification.
- [ ] Diagnostics tabs cover battery observation, runtime, bounded logs, pause/filter/clear, self-test, incident and export surfaces.

## Accessibility and responsive audit

- [ ] Dialogs expose `role=dialog`, trap Tab focus, close with Escape and provide labelled controls.
- [ ] Contact rows are keyboard activatable; message/action controls have accessible labels.
- [ ] Reduced motion disables non-essential animations; terminal geometry remains square and modern geometry remains rounded.
- [ ] Empty, offline, long-content, transfer, pairing and identity scenarios remain legible at phone width.
- [ ] Modals, toasts and action sheets stay inside the selected VIEW frame instead of using the host browser viewport.
