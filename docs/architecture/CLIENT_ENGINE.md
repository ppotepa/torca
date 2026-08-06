# ClientEngine architecture

The ClientEngine is the client-side composition and coordination boundary. It is implemented as a single-writer actor.

## Responsibilities

- receive typed commands from the bridge and internal workers;
- serialize state-changing operations;
- invoke application workflows and domain APIs;
- coordinate storage transactions through ports;
- dispatch durable and in-memory follow-up work;
- own retry scheduling and cancellation scopes;
- consume peer and platform lifecycle events;
- publish immutable UI projections and health snapshots;
- enforce one active engine instance per client data directory.

## Non-responsibilities

The engine must not contain:

- domain invariants that belong in mini-domain crates;
- raw SQL or row mapping;
- cryptographic algorithm implementation;
- wire serialization details;
- Flutter widgets or navigation;
- direct operating-system notification calls.

## Actor model

```text
producers
  UI commands
  peer events
  timers
  platform lifecycle
        |
        v
bounded engine mailbox
        |
        v
single command/event loop
        |
        +--> domain/application handlers
        +--> storage transactions
        +--> durable jobs
        +--> projection publication
```

Long-running I/O is started outside the actor and reports a typed completion event back to the mailbox. The actor must not block on unbounded network work.

## Snapshots

The engine publishes immutable snapshots with a monotonic revision. Flutter can discard older revisions and rebuild from the latest complete snapshot. Snapshot models are generated bridge contracts, not database entities.

## Shutdown and recovery

Shutdown stops accepting external mutations, persists required state, cancels workers, closes peer listeners and Tor processes, then closes storage. Startup repairs or resumes durable work before reporting ready.
