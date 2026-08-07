# Torca 0.2 implementation order

Target: **Torca 0.2 — Reliability & Daily-Use UX**

## Rules

- Work lands directly on `main`.
- One batch equals one focused commit.
- `0.2_PROGRESS.md` is the canonical live tracker.
- Flutter remains presentation plus local ephemeral UI state; Rust owns identifiers, timestamps, durable state, networking, security and domain workflows.
- Keep one responsive Flutter client for Windows and Android.
- Do not introduce BLoC, Riverpod or a feature framework.
- Do not add Linux production support, groups, calls, multi-device sync, cloud backup, typing indicators or central presence in 0.2.
- Prefer source-level checks while implementing; owner/platform/E2E validation remains a separate gate.

## Batches

1. `02-00` — freeze the 0.1 baseline, correct stale release documentation, create the 0.2 tracker.
2. `02-01` — normalize active Rust source roots and remove refactor leftovers/orphan source packages where safe.
3. `02-02` — Bridge v11: presentation sends user intent; Rust owns IDs and command timestamps.
4. `02-03` — typed bridge errors and consistent UI error presentation.
5. `02-04` — reusable operation busy state and duplicate-action protection.
6. `02-05` — conversation summaries, unread counters and activity ordering.
7. `02-06` — daily-use composer behavior, keyboard send and scroll-to-latest UX.
8. `02-07` — attachment capabilities and multi-file/desktop-friendly presentation.
9. `02-08` — explicit ephemeral pairing restart/recovery semantics.
10. `02-09` — Diagnostics v2: transition-based events and structured runtime health.
11. `02-10` — local Safety Number verification state.
12. `02-11` — settings/platform QoL with only implemented options.
13. `02-12` — localization foundation and accessibility hardening.
14. `02-13` — split CI/platform matrix definitions.
15. `02-14` — final 0.2 source audit and handoff.

## Dependency order

```text
02-00 -> 02-01 -> 02-02 -> 02-03 -> 02-04 -> 02-05 -> 02-06
                                               |          |
                                               v          v
                                             02-07      02-08
                                               \          /
                                                -> 02-09 -> 02-10 -> 02-11 -> 02-12 -> 02-13 -> 02-14
```

## Completion rule

A batch is source-complete when its intended code and focused tests/contract checks are committed on `main`. Platform and two-client validation may remain open and must not be represented as complete until actually executed.
