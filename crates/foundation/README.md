# Foundation libraries

Foundation libraries contain stable, dependency-light primitives used by multiple domains.

## Current crate

```text
torca-foundation
```

Batch 01 creates the buildable crate boundary without introducing shared product types prematurely. Batch 02 adds opaque identifiers, timestamps, command metadata, correlation identifiers and event envelopes.

Foundation must not contain product workflows, repositories, global service locators or convenience wrappers for one consumer. A type belongs here only when its semantics are genuinely shared and stable.
