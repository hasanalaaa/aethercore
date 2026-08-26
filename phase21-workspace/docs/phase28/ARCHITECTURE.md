# Phase 28 — Headless Command Surface (aetherctl + Service Lifecycle)

Status: complete on this host (macOS/ARM, Apple Silicon). Windows behavior FROZEN — the
cfg(windows) CLI lane reuses the existing named-pipe `SessionClient` unchanged and is
honestly deferred for runtime proof (QD-028-001).

## 1. Dual-mode design (CX-3: one truth surface)

`aetherctl` is a new binary crate (`apps/aetherctl`) with two command families:

- **Embedded/offline** (about/version/capabilities/engine-source/telemetry-once/
  self-check/service detect/service units): strictly read-only. They consume the same
  library truth as the service — `platform-capabilities` static matrix, the REAL
  cfg-selected `PerfPlatform` provider, the pinned model manifest + streaming sha256 —
  and contain ZERO mutation-capable symbols (audit-enforced symbol scan). Anti-snake-oil:
  offline never fabricates; degraded collectors are reported as typed CollectorFaults;
  refused probes are typed answers.
- **Service-backed** (doctor/perf/optimize/timeline/care/insights/scan): plain v7 RPC over
  the shared transport. The CLI constructs `Request`s from `aethercore-contracts`,
  frames them via `aethercore-ipc`, and maps non-zero service status codes to typed
  rejections carrying the SERVICE's own message_key. Mutations can NEVER bypass service
  governance because there is no local mutation code path at all.

Transport reuse (T2):
- cfg(unix): `aethercore_ipc::unix_impl::UnixSocketSession` — identical framing/auth
  contract (4-byte LE length prefix + protobuf body; 0700 dir / 0600 socket boundary is
  listener-side). Additive this phase: `set_io_timeouts()` so a CLI can never hang beyond
  `--timeout-ms`.
- cfg(windows): the frozen named-pipe client `aethercore_ipc::SessionClient`, reused
  verbatim; no Windows file was modified.

Hand-rolled arg parser (no clap): global flags BEFORE the command only; every parse
failure is a typed Usage error (exit 2) with a stable message_key.

Versioned machine envelope on EVERY JSON output:
`{"schema":"aethercore.aetherctl.v1","command":"<cmd>","ok":true,"data":{…}}` or
`{"schema":"…","ok":false,"error":{"kind":…,"message_key":…,"detail"?}}`. The envelope is
parse-backed through a `deny_unknown_fields` reader before it reaches stdout, so output
can never drift from its own schema. Text mode renders aligned ASCII tables derived from
the SAME JSON projection — the two modes cannot disagree.

## 2. Service detection

Socket probe + PID-file liveness cross-check → three typed states:

| State | Condition |
|---|---|
| `Reachable { pid }` | endpoint answers; pid reported only when `<data-root>/state/aethercore.pid` holds a live process (kill(pid,0), cfg(unix) libc, waiver-gated) |
| `Offline` | no endpoint and no live PID record |
| `StaleEndpointRecovered` | endpoint file exists but refuses connection while no live daemon holds the PID record (SIGKILL leftover); the DAEMON's stale-recovery rebinds it on next start — proven live |

PID-file resolution mirrors the daemon exactly ($AETHERCORE_PID_FILE override →
$AETHERCORE_DATA_DIR/state/aethercore.pid). Foreground daemons intentionally write no PID
file (Phase 26 contract: --daemon = foreground + logs + PID file); detection stays honest
and reports Reachable{pid:null} there.

## 3. Security posture

Nothing privileged is added. The CLI inherits the service's safety wholesale:

- Same IPC auth boundary as every other client (CX-4): 0700 rendezvous dir + 0600 socket,
  peer bound to the socket-owning user service-side. No new endpoints, ports, daemons, or
  setuid bits; no network I/O dependencies (audit-enforced allowlist).
- Embedded commands are read-only by construction AND by audit (mutation-guard symbol scan).
- Consent discipline: care start requires interactive digest-typed confirmation in text
  mode; default NO; wrong digest refused before any grant RPC; grant+start ride ONE
  principal connection so the server-side session registry binds them; JSON mode and
  --non-interactive always refuse with exit 6.
- Refused operations always print WHY: typed kind + stable message_key (+ technical
  detail from the service when present).
- Lifecycle artifacts are ARTIFACTS ONLY (CX-5): `service units --print` emits the launchd
  plist (plutil-lint validated) and systemd unit to stdout; nothing installs anything.

## 4. Daemon lifecycle (QD-026-003 closure)

- Graceful signals (unix, unix-ipc build): SIGTERM/SIGINT flip the stop flag; the accept
  loop exits immediately; connected sessions get a bounded 5 s GRACEFUL_DRAIN_WINDOW to
  finish the single in-flight frame; then the socket file is removed, the PID file is
  removed, and the process exits 0. Proven by a REAL signal integration test
  (`sigterm_drains_connected_session_then_exits_zero_cleaning_files`).
- Size-capped rotating daemon logs: `--daemon` now writes structured JSONL through
  `diagnostics::init_json_file_rotated` — cap constant 5 MiB, keep-last-N generations
  (.1, .2), deterministic names, runtime rolling. Windows' `init_json_file` behavior is
  untouched (frozen).
- launchd user-agent plist validated LIVE via `plutil -lint` (= OK).
- systemd unit statically asserted (Type/ExecStart --daemon/Restart/hardening keys);
  `systemd-analyze verify` honestly NOT_EXECUTED without a Linux host (QD-028-002).

## 5. Proof suite (this Mac, real spawned daemon)

`apps/aetherctl/tests/phase28_cli_matrix.rs` (feature `e2e`; run with
`cargo test --workspace --features aethercore-maintenance-service/unix-ipc,aetherctl/e2e`):

offline_surface_succeeds_with_no_daemon_present ·
service_backed_command_matrix_round_trips_over_real_uds ·
kill9_stale_endpoint_detected_then_next_daemon_rebinds ·
sigterm_drains_connected_session_then_exits_zero_cleaning_files ·
consent_adversarial_refusals_are_typed_and_grant_is_visible ·
service_set_fails_fast_offline_within_timeout.

## Deliverables

1. apps/aetherctl — parser/envelope/exit registry/detection/session client/offline
   commands/service lanes/lifecycle artifact printer; zero clippy findings; envelope
   schema pinned by unit tests.
2. Daemon lifecycle closure (drain + cleanup + rotated logs) in maintenance-service +
   diagnostics (additive; Windows paths untouched).
3. packaging/launchd + packaging/systemd artifacts (+ README honesty note).
4. docs/phase28 full set; QUALIFICATION_DEBT.json closes QD-026-003 with evidence and
   carries all prior items; QD-028-001..004 added.
5. Audit chain extended 421 → strict superset (exact count in SCORECARD.md), including
   renderer-untouched byte-comparison against the sealed P27 snapshot.
