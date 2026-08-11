# Foundation

`torca-foundation` contains stable, dependency-light primitives shared across Torca: identifiers, time values, cancellation, command/event helpers and classified errors.

Foundation is intentionally product-agnostic. It should not accumulate messaging workflows, storage code, protocol codecs, platform APIs or generic helper dumping grounds.

If a concept has product meaning, it normally belongs in a domain or application crate instead.

See [`../../ARCHITECTURE.md`](../../ARCHITECTURE.md) for the layer model.