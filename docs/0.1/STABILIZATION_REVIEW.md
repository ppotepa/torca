# Cross-cutting stabilization review

The final source review found and fixed these issues before handoff:

- signing and attachment-encryption keys shared one semantic type; split into `SigningSecretKey` and `SealingKey`;
- memory storage mutated schema state before commit; pending state now commits or rolls back atomically;
- connection PRAGMAs were modeled inside a transaction; bootstrap now executes them connection-scoped;
- outbound message and outbox inserts were combined in a bind-unfriendly SQL batch; statements are separate and composed by a transaction;
- due outbox work was selected without atomically claiming it; claim uses `UPDATE ... RETURNING`;
- stale outbox claims had no recovery path; recovery and dead-letter operations were added;
- failed messages could start sending without explicit retry; only queued messages can begin a send;
- duplicate receipts replaced the original immutable fact; duplicates are now no-ops;
- pairing completion did not preflight duplicate conversation IDs; engine preflight now covers contact, conversation ID and contact ownership;
- peer responder ACK could be sent without matching the validated inbound challenge; remote challenge binding is now mandatory;
- Tor paths with spaces were not quoted; torrc rendering now escapes and quotes paths;
- generated-contract argument parsing ignored a single explicit output path; parsing is now deterministic.

These fixes are statically reviewed but still require owner-run compilation and tests.
