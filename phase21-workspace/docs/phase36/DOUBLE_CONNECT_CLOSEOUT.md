# P36 — ServiceJob Double-Connect Closeout

## Defect

Every `ServiceJob` verb in `apps/aetherctl` opened two IPC sessions per
command on Windows: the Windows pre-probe in `detect_service` was itself a
full `ServiceClient::connect` whose result was discarded via `Ok(_)`, then
`execute()` connected again at what is now
[service_cmds.rs:86](../../apps/aetherctl/src/service_cmds.rs#L86). Because
the Hello handshake lives inside `connect`, this emitted two Hellos per verb;
the server logged `"duplicate client hello ignored"` and the discarded
session's reader thread (no `Drop` on `ServiceClient`) kept its pipe instance
alive. `OfflineJob` verbs (e.g. `update stage`) never took this path, which is
the only reason they appeared unaffected.

Fix: `9b9d38c` (`fix(aetherctl): one IPC session per ServiceJob verb on
Windows`) removes the Windows pre-probe entirely — `ServiceClient::connect`
already returns the same typed unreachable errors, so reachability comes from
the single connect. Unix keeps its pre-probe verbatim (`detect_service` there
does not call `ServiceClient::connect`; it derives `ServiceState` from
endpoint/PID inspection that connect() alone can't reconstruct).

## Classified as an efficiency correction, not a correctness one

The transport fix landed first (overlapped named-pipe I/O, `482d480`/`3e0c00f`,
merged to main ahead of this change). Once the transport was overlapped,
the second, discarded Hello no longer blocked the real request — the client
still burned part of its `--timeout-ms` budget on a connection it never used,
but it no longer hung. This defect is therefore closed as a **performance/
resource-leak fix** (halves the IPC round-trips and reader threads per verb),
not as a hang fix — the hang was already resolved by the transport change.

## Verification — one connect confirmed

`apps/aetherctl/src/service_cmds.rs` on `main` (post-merge, commit `fc7665d`):
exactly one `ServiceClient::connect` call, at line 86, inside `execute()`.
The removed Windows pre-probe is documented in the comment block at lines
56–76. The Unix pre-probe at line 79 calls `transport::detect_service`, which
does not call `ServiceClient::connect` — so neither platform performs a
second connect.

## Verb measurements (both actual-token contexts)

Context labels: STD = `P36StandardUser` (`IS_ADMIN=False`), ADMIN = `P36Admin`
(`IS_ADMIN=True`) — both via the established one-shot Scheduled Task probe
method (see `PROGRESS.md`), never the SYSTEM bridge.

| Verb | STD | ADMIN | Notes |
|---|---|---|---|
| detect | 27 ms | 43 ms | |
| doctor | 57 ms | 90 ms | typed `diagnostics.stateUnavailable`, exit 5, matches baseline |
| optimize | 36 ms | 19 ms | |
| scan | 16 ms | 25 ms | |

All four verbs return under both principals. No regression against the
pre-merge baseline (see raw-fact note below).

## Gate 2d — retired

The previous brief asked for a count of distinct `session_id` values per verb
invocation. That is not measurable and the fault was in the brief, not the
work:

- `services/maintenance-service/src/server.rs` contains exactly two `info!`
  calls (`"operation kernel IPC v7 ready"`, `"maintenance service stopped"`)
  and no `debug!`/`trace!` at all.
- The service exposes no log-level knob — no `EnvFilter`, no `RUST_LOG`, no
  config surface.
- The service never logs session creation at any level.
- The only WARN paths that ever produced a session-shaped log line were
  `"duplicate client hello ignored"` and `"IPC session ended"` on error. Both
  are now absent by design: the double-connect fix removed the duplicate
  hello, and clean disconnects return `Ok`.

Zero new log lines after the fix is therefore the **correct** signal, not a
missing instrument. Replacement acceptance criteria (a)–(c) from the brief are
satisfied as recorded above and require no further measurement.

## Raw fact — the VM baseline already had this fix

Before the previous session copied anything, the VM's
`apps/aetherctl/src/service_cmds.rs` already hashed identical to the branch
version: `89f0884…c59320`. This means the 22–81 ms baseline measured on
2026-08-30 was already taken **with** the double-connect fix applied — the
baseline and the "after" measurements above are the same code path, which is
consistent with (c), not a coincidence to be re-explained.

## Merge

`fix/servicejob-double-connect` merged into `main` via `--no-ff` (commit
`fc7665d`), pushed to `origin/main`. No conflicts.
