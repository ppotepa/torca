# Torca documentation

Torca keeps a deliberately small set of maintained documentation because the implementation is changing quickly. The purpose of these documents is to preserve stable product intent, ownership and trust boundaries without duplicating the source tree.

## Maintained documents

- [`../README.md`](../README.md) — product overview, system shape and development entrypoints.
- [`../ARCHITECTURE.md`](../ARCHITECTURE.md) — medium-depth architecture and ownership model.
- [`../SECURITY.md`](../SECURITY.md) — current security posture and explicit non-guarantees.
- [`security/threat-model.md`](security/threat-model.md) — assets, trust boundaries, threats and review triggers.
- [`../CONTRIBUTING.md`](../CONTRIBUTING.md) — placement rules, workflows and documentation policy.
- [`../ROADMAP.md`](../ROADMAP.md) — product/engineering direction rather than release bookkeeping.

Short README files at major repository roots (`apps`, `crates`, `services`, `tests`) are navigation aids only. They should point toward the same central model rather than introduce alternative architecture descriptions.

## What belongs in documentation

Document stable facts such as:

- what Torca is trying to become;
- which layer owns a responsibility;
- which trust boundary is crossed;
- why the client/relay split exists;
- what a contributor should use as a public workflow;
- security guarantees and non-guarantees;
- long-lived product/non-goal direction.

## What should stay in code/history

Avoid maintaining prose copies of:

- exact contract or wire version numbers;
- exact database migration counts;
- timeout/retry constants;
- generated command/DTO field lists;
- individual class/method inventories;
- temporary refactor/batch names;
- implementation progress percentages;
- release-specific source audits after their work is merged.

These details are better represented by source, generated schemas, tests, release metadata and Git history.

## History

Older version plans, batch trackers, source audits and ADRs were useful while their changes were being implemented but became misleading once the architecture moved on. They are retained by Git history rather than kept in the active tree as competing sources of truth.

If historical reasoning becomes relevant again, recover it from the commit that introduced the behavior and rewrite the still-valid principle into the appropriate evergreen document.