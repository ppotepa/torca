# torca-file-storage

## Purpose

Implement encrypted local blob and cache ports for attachments and exported diagnostics.

## Owns

- private application data directories;
- atomic temporary-file promotion;
- bounded cache accounting and cleanup;
- encrypted blob naming and lookup;
- integrity verification before promotion;
- safe deletion and orphan cleanup.

## Does not own

Attachment domain policy, encryption algorithm selection, image decoding or OS share-sheet behavior.

## 0.1 completion

Interrupted writes do not expose partial blobs, cache cleanup respects active references and plaintext attachment data is not persisted outside explicitly controlled temporary scope.
