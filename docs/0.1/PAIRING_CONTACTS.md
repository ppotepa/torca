# Pairing, contacts and conversations — Batch 07

Pairing is an explicit, expiring two-sided approval state machine. Completion yields a public peer proposal; only the application engine creates the verified contact and direct conversation.

Implemented domains:

- `torca-pairing` — codes, roles, expiry, local/remote approval, rejection, cancellation and completion;
- `torca-contacts` — verified identity, onion route, capability handle, block/unblock/remove transitions;
- `torca-conversations` — one direct conversation per contact with archive/restore lifecycle.

No domain crate accesses SQLite, relay networking, Tor sockets or Flutter state.
