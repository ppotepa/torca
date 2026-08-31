# Validation evidence archive

Files in this directory are **dated evidence snapshots**, not current project status or release checklists.

Each report should be interpreted only for the source commit, build, environment, target/device, provider/profile, scenario and duration/repetitions it records. A report remains historically useful even when later source changes supersede its conclusions.

## Rules for new reports

A promoted validation report should state, where applicable:

- date and exact source commit/build identity;
- platform, device/emulator/host and relevant environment;
- Iroh profile/network conditions;
- command/scenario and duration/repetition count;
- pass/fail/incomplete verdict criteria;
- important redactions/limitations; and
- links or paths to retained raw artifacts when appropriate.

Do not edit an old report to make it describe a new commit. Add a new dated report instead.

## What reports do not prove

- An old green report does not make current HEAD green.
- Emulator CPU does not prove physical-device battery usage.
- A source test does not prove a platform/device journey.
- A passing security preflight is not an independent security audit.
- A manifest saying what was requested is not the same as evidence that the scenario completed.

Use [`../testing.md`](../testing.md) for evidence terminology and [`../STATUS.md`](../STATUS.md) for the current maturity/release-evidence summary.
