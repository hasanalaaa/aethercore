# P36 — Windows overlapped IPC: validation record

**Result: PASS.** The Windows named-pipe request/response path works. Before this
change no request/response round trip had ever completed on Windows.

## Root cause

`try_clone()` is `DuplicateHandle`: a second HANDLE onto the *same* kernel
FILE_OBJECT. Neither `CreateNamedPipeW` nor the client's `CreateFileW` set
`FILE_FLAG_OVERLAPPED`, so the object carried `FO_SYNCHRONOUS_IO` and the I/O
manager serialized every operation on it. A `WriteFile` from the writer thread
blocked behind the outstanding `ReadFile` of the reader thread.

The handshake succeeded only because the `ServerHello` writes precede the session
loop's first `read()`. That is why `service detect` passed while every
request/response verb deadlocked until its 10 s client deadline.

`update stage` was never counter-evidence: it is an `OfflineJob` and never
touched IPC.

## Fix

`crates/ipc/src/windows_impl.rs`, both sides, one change: `FILE_FLAG_OVERLAPPED`
plus a per-direction OVERLAPPED and auto-reset event, `Read`/`Write` implemented
over `ReadFile`/`WriteFile` + `GetOverlappedResult`. Cancellation moved from
`CancelSynchronousIo` to `CancelIoEx`, deleting `SyncIoCancellation`,
`writer_thread_id`, `cancel_registered_io`, `bind_reader_to_current_thread` and
its `server.rs` call site. Production code is a **net deletion of 6 lines**.
The frame codec, the wire protocol, `unix_impl.rs`, the accept loop, and the pipe
security descriptor are untouched.

## Gate 1 — build and test

| Check | Result |
|---|---|
| ipc unit tests, branch, Windows ARM64 | 17 passed |
| `windows_roundtrip` on branch | green, 0.30 s |
| `windows_roundtrip` on `main` | hangs; killed at the 600 s bound |

The round-trip test is new: `crates/ipc/tests/windows_roundtrip.rs`. Before it,
`crates/ipc/tests/` held a single `#![cfg(unix)]` file — the Windows IPC path had
no integration coverage at all, which is how this survived to Phase 36.

## Gate 2 — deploy

Binaries backed up to `C:\AetherCore-P36\backup-p36-overlapped` (service exe and
`aetherctl.exe`) before replacement. `libomp140.aarch64.dll` untouched. Two
earlier deploy attempts aborted cleanly on toolchain gaps (`LIBCLANG_PATH` for
bindgen, `cmake` absent from PATH; both under `C:\AetherCore-P36\toolchain`)
without mutating the installed product.

## Gate 3 — runtime, actual-token contexts

One-shot Scheduled Task method per the established P36-D009 procedure.
Measured twice independently, by the execution session and again in review.

| Verb | P36StandardUser (`IS_ADMIN=False`) | P36Admin (`IS_ADMIN=True`) |
|---|---|---|
| `service detect` | RETURNED 24–81 ms | RETURNED 25–74 ms |
| `doctor` | RETURNED 61–77 ms | RETURNED 46–79 ms |
| `optimize status` | RETURNED 32–52 ms | RETURNED 23–43 ms |
| `scan status` | RETURNED 22–52 ms | RETURNED 22–42 ms |

Before the fix these same verbs returned `cli.windowsLane.request deadline
exceeded` at ~10 000 ms, or were killed at a 15 s watchdog.

`doctor` returns the typed product rejection `diagnostics.stateUnavailable` — a
business-logic answer from the service, not a transport failure. `perf snapshot`
returns live host telemetry, proving the full stack answers.

| Other Gate 3 checks | Result |
|---|---|
| Three concurrent clients | all returned real payloads |
| Pipe DACL before vs after | byte-identical: `O:<service SID> G:SY D:P(A;;0x12008b;;;AU)(A;;FA;;;<service SID>)` |
| Service state | RUNNING throughout |

## Open question resolved

The unresolved D-transport / D-handler sub-split resolves to **D-transport**: the
response was being produced and blocked on the file-object lock. The transport
fix was sufficient; no handler defect exists.

## Not covered here

- `fix/servicejob-double-connect` remains unmerged. It removes a genuine double
  IPC connect per ServiceJob verb (`detect_service` opens a full session and
  discards it, then `execute` connects again). It touches only
  `apps/aetherctl/src/service_cmds.rs` — zero file overlap with this change. With
  the transport fixed it is now an efficiency correction rather than a
  correctness one, and needs its own rebuild and re-validation before merge.
- `ipc_probe.exe` is still present in `C:\Program Files\AetherCore` (DBT-P36-008).
  It must not reach release packaging.
- Physical x86_64 qualification remains required (P36-D004 / P36-D015).
