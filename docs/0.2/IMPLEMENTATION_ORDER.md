# Torca 0.2 implementation order

Target: **Torca 0.2 — Reliability & Daily-Use UX**

## Rules

- Work lands directly on `main`.
- One batch equals one focused commit.
- `0.2_PROGRESS.md` is the canonical live tracker.
- Flutter remains presentation plus local ephemeral UI state; Rust owns identifiers, timestamps, durable state, networking, security and domain workflows.
- Keep one responsive Flutter client for Windows and Android.
- Do not introduce BLoC, Riverpod or a feature framework.
- Do not add Linux production support, groups, calls, multi-device sync, cloud backup, central presence or an ad-hoc cryptographic ratchet in 0.2.
- Source completion and platform/E2E validation are separate states.

## Batches

1. `02-00` — baseline closure and tracker.
2. `02-01` — repository/source cleanup.
3. `02-02` — Bridge v11 intent boundary; Rust-owned IDs/timestamps.
4. `02-03` — typed bridge errors.
5. `02-04` — operation busy state and duplicate-action protection.
6. `02-05` — conversation summaries/unread/activity ordering.
7. `02-06` — composer/scroll daily-use UX.
8. `02-07` — attachment capabilities and multi-file UX.
9. `02-08` — explicit ephemeral pairing restart semantics.
10. `02-09` — transition-based Diagnostics v2.
11. `02-10` — local Safety Number verification state.
12. `02-11` — real settings/platform QoL.
13. `02-12` — localization foundation/accessibility.
14. `02-13` — split Rust/Flutter/Windows/Android CI matrix.
15. `02-14` — viewport/lifecycle-aware Read semantics and receipt privacy.
16. `02-15` — non-blocking local/native startup and off-UI-isolate FFI mutations.
17. `02-16` — SQLCipher conversation paging/search and lightweight overview snapshots.
18. `02-17` — capture protection, Safety QR and identity-change security policy.
19. `02-18` — canonical source roots, stable runtime paths, readable composition and intent-only public ABI.
20. `02-19` — final source audit, developer policy and validation handoff.

## Dependency order

```text
02-00 -> 02-01 -> 02-02 -> 02-03 -> 02-04 -> 02-05 -> 02-06
                                                  |
                                                  v
02-07 -> 02-08 -> 02-09 -> 02-10 -> 02-11 -> 02-12 -> 02-13
                                                  |
                                                  v
02-14 -> 02-15 -> 02-16 -> 02-17 -> 02-18 -> 02-19
```

## Completion rule

A source batch is complete when its intended code and focused contract/test surface are committed coherently on `main`. Windows/Android/two-client release gates stay open until actually executed; source status must never be used as a substitute for that validation.
