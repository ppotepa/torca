# Foundation libraries

Foundation contains stable, dependency-light contracts shared by several Torca mini-domains. It must not become a generic utility package or own product workflows.

## Implemented package

`torca-foundation` currently provides:

- `OpaqueId` as a dependency-free 128-bit identifier representation;
- process identifiers: `CommandId`, `CorrelationId`, `CausationId` and `EventId`;
- bounded millisecond UTC `Timestamp` values;
- `CommandMetadata` and typed `CommandEnvelope<C>`;
- `EventMetadata` and typed `DomainEventEnvelope<E>`;
- safe error classification through `ErrorCode`, `ErrorCategory`, `RetryAdvice` and `ErrorDescriptor`;
- runtime-neutral cooperative cancellation contracts.

Domain-specific identifiers such as `MessageId`, `ContactId` or `ConversationId` remain owned by their domain crates and should be newtypes around `OpaqueId`.

## Explicit exclusions

Foundation does not contain:

- repositories or database abstractions;
- serialization or wire-format models;
- cryptographic randomness or identifier generation;
- async runtime types;
- global service locators;
- Flutter, FFI or platform contracts;
- business state machines.

Identifier generation will be supplied by an application or cryptographic adapter. This keeps deterministic construction and parsing available without coupling every domain to one randomness implementation.
