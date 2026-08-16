# Background thread ownership

This inventory is normative for process-owned Torca background work. A worker must have exactly one owner and an explicit shutdown/stale-result policy.

| Worker | Owner | Lifetime | Shutdown | Stale-result protection |
|---|---|---|---|---|
| client engine actor | `ClientEngineActor` | process/application runtime | sends `Shutdown`, joins | mailbox disconnect; no detached result |
| application runtime actor | `RuntimeOwner` | process runtime | sends `RuntimeCommand::Shutdown`, joins | actor is sole state owner |
| relay health | `RelayHealthWorker` | `RuntimeOwner` | sends worker shutdown, joins | one long-lived command lane |
| pairing supervisor | `PairingWorkerDriver` | runtime composition | sends worker shutdown, joins | one long-lived command lane |
| Tor recovery | `OwnedTorDriver` / `TorBootstrapWorker` | one recovery generation | receiver may be dropped without join | `RecoveryEpoch`; stale generations are discarded |
| onion publisher | `OwnedTorDriver` / `OnionPublisher` | current Tor client generation | explicit shutdown command, joins | owner replaces/stops publisher before recovery |
| native process actor | process registry `RuntimeHandleInner` | native process runtime | `ActorMessage::Shutdown`; runtime state closes before loop exits | one process registry owner |
| native host startup | `TorcaRuntime` | one host-start attempt | receiver ownership; successful orphan explicitly shuts its `RuntimeOwner` down | only the current `host_start` receiver may adopt a result |
| attachment maintenance job | `TorcaCommunicationDriver` | one bounded maintenance pass | one-shot; owner observes active/result slots | `attachment_job_active` prevents overlap; completion is consumed once |
| peer listener/accept path | Tor/peer-link owner | Tor runtime generation | listener/peer-link shutdown | route change invalidates sessions and reconnect state |

## Rules

1. `JoinHandle` belongs to the struct that starts a long-lived worker whenever deterministic shutdown is required.
2. Detached one-shot work is allowed only when its result is generation/epoch scoped or when an orphaned successful result is explicitly shut down.
3. No worker may mutate a newer runtime generation through a captured reference.
4. Network I/O may run on worker threads, never while a process registry mutex is held.
5. Worker completion wakeups use a non-blocking callback/mailbox edge and never call the callback while the callback-slot mutex is held.
6. Every named worker must have a stable `torca-*` thread name; the naming pass is tracked separately.
7. A new worker must be added to this table when introduced.

## Intentional detach: Tor recovery

`TorBootstrapWorker` is the only intentionally non-joined recovery worker in this inventory. Joining it in `OwnedTorDriver::shutdown()` could block the process on an Arti bootstrap. Correctness is instead provided by `RecoveryEpoch`: shutdown advances the epoch and late results are ignored/dropped.
