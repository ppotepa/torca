# Mini-domain libraries

Each directory below represents a planned independent domain library.

- [`torca-identity`](torca-identity/README.md)
- [`torca-contacts`](torca-contacts/README.md)
- [`torca-pairing`](torca-pairing/README.md)
- [`torca-conversations`](torca-conversations/README.md)
- [`torca-messaging`](torca-messaging/README.md)
- [`torca-receipts`](torca-receipts/README.md)
- [`torca-attachments`](torca-attachments/README.md)
- [`torca-presence`](torca-presence/README.md)
- [`torca-notifications`](torca-notifications/README.md)

A domain crate owns vocabulary, invariants, transitions, commands, events, errors and required ports. It must remain testable without SQLite, Tor, Flutter or operating-system services.
