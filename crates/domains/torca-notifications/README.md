# torca-notifications

## Purpose

Own privacy-safe notification intent and display policy independently from operating-system APIs.

## Owns

- notification intent identifiers;
- event-to-notification policy;
- foreground suppression rules;
- privacy modes for title and body content;
- deduplication and replacement semantics;
- notification adapter port.

## Does not own

Android notification channels, Windows toast APIs, message persistence or navigation implementation.

## 0.1 completion

Incoming messages produce at most one policy-compliant notification intent, foreground conversations suppress redundant alerts, and platform adapters receive no unnecessary plaintext.
