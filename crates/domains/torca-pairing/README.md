# torca-pairing

## Purpose

Own invitation and pairing-session state machines that transform two untrusted rendezvous participants into an explicitly approved verified relationship result.

## Owns

- invitation identifiers, codes and expiry;
- host and joiner roles;
- session states and allowed transitions;
- local and remote approval state;
- cancellation, rejection, expiry and completion;
- pairing transcript facts required for verification;
- pairing repository, rendezvous and pairing-crypto ports.

## Does not own

Relay process implementation, contact persistence, conversation creation, peer sockets or Flutter screens.

## Planned commands

`CreateInvitation`, `JoinInvitation`, `ApprovePairing`, `RejectPairing`, `CancelPairing`, `ExpirePairing`.

## Completion event

`PairingCompleted` exposes verified public identity, peer endpoint and authorized capability material to an application handler. It does not directly create other domain aggregates.

## 0.1 completion

Two independent engines reach the same completed result after explicit approval, while retries, duplicate relay frames, rejection and expiry remain deterministic.
