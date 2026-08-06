# torca-attachments

## Purpose

Own attachment metadata, lifecycle, limits and message references while delegating bytes, encryption and transfer to ports.

## Owns

- `AttachmentId`;
- media type, original name, byte length and digest metadata;
- preparing, ready, queued, transferring, available and failed states;
- size and type policy;
- attachment reference usable by messaging;
- blob-store, attachment-crypto and transfer ports.

## Does not own

Filesystem paths exposed to UI, concrete encryption, peer streams, image decoding or cache directory implementation.

## 0.1 completion

One bounded encrypted image flow survives interruption, verifies integrity before availability and never stores plaintext bytes in diagnostics.
