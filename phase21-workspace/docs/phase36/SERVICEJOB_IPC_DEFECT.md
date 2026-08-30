# P36 — ServiceJob IPC defect: double-connect FIXED, hang NOT resolved

Branch: `fix/servicejob-double-connect`. **Do not merge without owner decision** —
the fix is correct and verified, but Gate 3 does not pass.

`ARCHITECTURAL_DECISION_REQUIRED=True`

## 3a — Discriminator: CONFIRMED

No rebuild, no source change. Service restarted, then run in this exact order as
the real `P36StandardUser` (one-shot Scheduled Task, Password logon,
`RunLevel=Limited`), 15 s watchdog each. Installed binary
`C:\Program Files\AetherCore\aetherctl.exe`.

| # | Verb | Kind | Result | Elapsed | Typed answer |
|---|---|---|---|---|---|
| 1 | `service detect` | OfflineJob, single connect | **RETURNED** | 32 ms | `state Reachable`, `cli.detect.reachable` |
| 2 | `doctor` | ServiceJob, double connect | deadline | 10 027 ms | `cli.windowsLane.request deadline exceeded` |
| 3 | `optimize status` | ServiceJob, double connect | **TIMED OUT** | 15 015 ms | (killed, no output) |

detect passes and doctor fails ⇒ **CONFIRMED**.

`service.jsonl` during the probe: 0 new lines, 0 duplicate-hello.
Historical corroboration in the same log from 2026-08-29, before the probe:

```
{"timestamp":"2026-08-29T18:28:07.008446Z","level":"WARN","fields":{"message":"duplicate client hello ignored","session_id":"1427183c-…"}}
{"timestamp":"2026-08-29T18:38:44.804304Z","level":"WARN","fields":{"message":"duplicate client hello ignored","session_id":"ce5ade80-…"}}
```

Two distinct `session_id` values, two duplicate-hello warnings — the second Hello
per command is real.

## 3b — The fix (one production file)

`apps/aetherctl/src/service_cmds.rs`, `execute()`: the pre-probe
`transport::detect_service(config)` is now `#[cfg(unix)]`.

Mechanism it removes: on Windows `detect_service` **is** a full
`ServiceClient::connect` whose result is discarded via `Ok(_)`
(`apps/aetherctl/src/transport.rs`, `#[cfg(windows)] detect_service`). The
handshake happens inside `SessionClient::connect`, so that discarded connect
already sent a Hello. `ServiceClient` has no `Drop`, and its
`Option<Arc<SessionClient>>` is also held by the session's reader thread, so the
discarded session is never closed — the pipe instance leaks. Every ServiceJob verb
opened two sessions; `service_cmds.rs` then connected a second time.

Reachability is now derived from that single connect: `ServiceClient::connect`
already returns the same typed unreachable errors on Windows.

**Unix is unchanged.** The pre-probe is kept verbatim under `#[cfg(unix)]`
because `detect_service` is the only producer of
`ServiceState::StaleEndpointRecovered` (endpoint exists, no live PID) and of the
`read_live_pid` fallback when the socket file is absent. Neither is
reconstructible from `connect()`. Every unix message key is preserved.

**`offline.rs:140` (`OfflineJob::ServiceDetect`) is NOT affected.** It calls
`transport::detect_service` directly, which is untouched; that path was always a
single connect, which is why `service detect` and `update stage` always worked.

Targeted tests, unix host: `cargo test -p aetherctl --locked` → **28 passed, 0
failed**. No new warnings.
VM rebuild: `cargo build -p aetherctl --release --locked` on ARM64 → Finished in
1 m 31 s, 0 errors. Source synced Mac→VM with SHA-256 verification
(`89F0884187554F6F68960F14FF05D74FD2AD3EEB626ADAD93839B84C10C59320`).

## 3c — Validation: GATE 3 FAILS

Service restarted, then both principals via the established one-shot Scheduled
Task method. Fixed binary SHA-256
`972823D8B63BF1B8E24CF268CE3E5092BD36B9239DFA38DFB5631117C98BA568`.

| Verb | P36StandardUser (Limited) | P36Admin (Highest) |
|---|---|---|
| `doctor` | 10 915 ms — `cli.windowsLane.request deadline exceeded` | **TIMED OUT 15 s** |
| `optimize status` | 10 060 ms — `cli.windowsLane.request deadline exceeded` | 10 127 ms — same |
| `scan status` | 10 033 ms — `cli.windowsLane.request deadline exceeded` | 10 120 ms — same |

- Service stayed `Running` before and after. ✅
- `DUP_HELLO_AFTER_FIX=0`. ✅ **The double connect is gone.**
- Verbs still do not return a result. ❌ **Gate 3 not met.**

The fix changed behaviour measurably — `optimize status` previously blew past the
15 s watchdog and now returns a typed error at its 10 s deadline — but it did not
make the verbs work.

## Root cause is NOT in aetherctl

Decisive probe:

```
aetherctl-fixed.exe --timeout-ms 60000 doctor
→ cli.windowsLane.request deadline exceeded
→ ELAPSED_MS=60222
```

The deadline scales exactly with `--timeout-ms`. So a single, successfully
handshaken session dispatches a Request and **the server never answers it**, for
at least 60 s. `service detect` proves connect + handshake succeed in 32 ms on the
same pipe.

Note on the log: the maintenance service writes **no** line for a healthy session
— `server.rs` logs only `IPC v7 ready`, the session cap, accept failure, session
error, duplicate hello, and request-worker spawn failure. "0 new lines" therefore
proves no *error* occurred; it does not prove the request was processed.

The remaining defect is on the Windows request/response path — the server not
consuming/answering the `Request` frame, or the client's pending-map correlation
(`Arc<Mutex<HashMap<String, mpsc::SyncSender<Response>>>>` in
`crates/ipc/src/windows_impl.rs`) never matching the answer. Both live in
`crates/ipc` or the maintenance service.

Stage 3 authorised **one** production fix in aetherctl and forbade protocol,
installer, and server changes. Closing this requires exactly such a change, so
work stopped here per that rule.

## Decision requested

1. Authorise a bounded investigation of the Windows request/response correlation
   in `crates/ipc/src/windows_impl.rs` and `services/maintenance-service`, or
2. Keep this branch unmerged until that authorisation exists.

The double-connect fix stands on its own merit either way: it is a real defect,
independently evidenced, and it removes one full leaked pipe instance per verb.
