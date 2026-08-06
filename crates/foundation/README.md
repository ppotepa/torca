# Foundation libraries

Foundation libraries contain stable, dependency-light primitives used by multiple domains.

Expected capabilities include opaque identifiers, bounded timestamps, command metadata, correlation identifiers, redacted diagnostic identifiers and cancellation-neutral error categories.

Foundation must not contain product workflows, repositories, global service locators or convenience wrappers for one consumer. A type belongs here only when its semantics are genuinely shared and stable.
