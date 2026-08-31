# Torca project status

Torca is security-sensitive **alpha** software under active development. The current source tree has one production communication composition: Iroh. Memory is retained only for deterministic tests.

The authoritative product/build/compatibility metadata is [`../release/version.json`](../release/version.json). A version string is not a release-readiness claim.

## Supported product composition

| Area | Status | Notes |
| --- | --- | --- |
| Windows | alpha-supported composition | shared Flutter/Rust client |
| Android | alpha-supported composition | shared Flutter/Rust client with Android lifecycle/background host integration |
| Iroh | production provider | direct or Iroh relay reachability according to profile/runtime conditions |
| Memory provider | test-only | deterministic application/provider-conformance testing |
| Tor | retired | not part of the active production graph |
| WebRTC | retired unfinished adapter | not part of the active production graph |
| Linux client | not supported for production | no supported production composition |

## Implementation maturity

| Capability | Current state |
| --- | --- |
| local identity / SQLCipher storage / protected secrets | implemented; source-tested paths exist |
| pairing / explicit approval / persisted relationship | implemented; deterministic/integration coverage exists; physical-device evidence remains a separate gate |
| authenticated peer messaging / durable retry / receipts | implemented; runtime/integration coverage exists |
| paged history and search | implemented |
| attachments | implemented with encrypted/resumable runtime-owned transfer state |
| contact verification / identity-change protection | implemented security-sensitive workflow |
| notifications / deep links / lifecycle integration | implemented with platform-specific host adapters |
| Radio Mode | experimental alpha capability; not a general voice-call system |
| provider abstraction | Iroh production + Memory test composition behind provider-neutral application ports |
| deploy / diagnostics / soak tooling | implemented developer tooling; individual run results must be cited separately |

“Implemented” means the source/composition exists. It does not mean the feature has been independently audited, physically soak-validated on every target or proven release-ready.

## Current release evidence still required

Before treating the project as a public production-quality release, the repository still needs evidence and release work beyond ordinary source checks, including:

- repeatable real-device Windows/Android peer journeys covering pairing, restart without re-pairing, route/network changes, background/foreground delivery, receipts and attachments;
- long-running physical Android power/background measurements with controlled device/network conditions;
- independent security review appropriate to the claims being made; and
- production Android signing/provenance. The current Android release build still uses the debug signing configuration and must not be represented as a production-signed artifact.

These are release-evidence requirements, not statements that the underlying feature is absent.

## Security/privacy position

Torca provides authenticated application-layer protection for paired traffic and encrypted local structured storage, but it does **not** claim:

- Tor-style anonymity from Iroh direct/relay transport;
- Signal-style forward secrecy or post-compromise security in the current relationship-key design;
- protection after full endpoint/OS compromise;
- control over content copied/recorded by a paired recipient; or
- guaranteed availability under suspension, network failure or denial of service.

See [`../SECURITY.md`](../SECURITY.md), [`security/THREAT-MODEL.md`](security/THREAT-MODEL.md) and [`../PRIVACY.md`](../PRIVACY.md).

## Validation records

Automated tests, builds and soaks answer different questions. Use the evidence terms defined in [`TESTING.md`](TESTING.md).
