# AetherCore — Phase 27 Master Delivery Report
# Native Unix Providers & Composition Wiring

Seal: P27 · Base: sealed P26 tree · Host: macOS (unix test host, Apple Silicon)
Windows behavior: FROZEN — zero regression tolerance maintained.

## Baseline gates (pre-change, verbatim)

```
$ python3 PHASE_26_BINARY_SAFE_PATCH/verify_phase26.py .
{"status": "PASS", "checked": 749, "problems": []}
$ python3 scripts/phase26-adversarial-audit.py .
checks: 349, failures: [], status PASS
$ cargo test --workspace --jobs 2   (×2)
passed: 388 failed: 0    |    passed: 388 failed: 0   (104 suites per run)
```

## T1 — Unix transport composition + live proof

`services/maintenance-service/src/unix_composition.rs` binds the SAME router logic that
serves named pipes on Windows to a `UnixSocketListener`: one wire contract, byte-identical
framing (4-byte LE length prefix + protobuf v7). Contract enforcement unchanged:
0700 dir / 0600 socket (`ensure_private_dir`), stale-path recovery after a crashed prior
run, live-stomp refusal. Principal binding = the socket-owning OS user
(`socket_owner_principal`, CX-4/QD-026-001; SO_PEERCRED deepening stays deferred and is
NOT claimed).

GD integration proof (real binary over the real socket):
`services/maintenance-service/tests/phase27_unix_ipc.rs`
- v7 handshake Hello → ServerHello (protocol_version pinned),
- Ping round trip → `PongResponse{service_version}`,
- GetPlatformCapabilities → platform="macos", telemetryCpu=native,
  driverServicing=notAvailable (honesty asserted in both directions),
- GetEngineSource → source="native", platform="macos",
- permission contract asserted on the real bound socket (dir 0700 / socket 0600),
- kill without cleanup → stale socket file left behind → second lifecycle REBINDS the
  same path and serves a fresh handshake (stale-path recovery proven in practice).

Verbatim GetPlatformCapabilities response captured through the REAL unix socket
(hex, prost-encoded; decoded rows match the matrix below):

```
12d6060a1608071212 756e697175652d66726573682d69642d4141 8a03ba06 0a05 6d61636f73 ...
platform="macos" capabilities=[telemetryCpu:native, telemetryMemory:native,
telemetryStorage:native, telemetryGpu:degraded(cap.note.macosGpuLimited),
thermalPowerClamp:degraded(cap.note.macosThermalViaNq), driverServicing:notAvailable(...
macosNoDriverStore), systemRepairDism/Sfc/Wua:notAvailable(...macosNoDismSfcWua),
processGovernorEcoQos:notAvailable(...macosNoEcoQos), gameModeProfile:notAvailable(
...windowsOnlyApi), restorePoints:notAvailable(...macosRestoreNotAvailable),
windowsUpdate:notAvailable(...macosNoWinUpdate), timelineIntelligence:native,
localIntelligence:native, careOrchestration:native]
```

## T2/T3 — Native providers

- `crates/performance-telemetry/src/macos_impl.rs`: libc-only syscalls — mach
  `host_statistics64` tick deltas, `sysconf(_SC_PAGESIZE)` + `HOST_VM_INFO64` +
  `sysctl(VM_SWAPUSAGE)` memory, `getloadavg`, `statfs` storage capacity/queue proxies,
  libproc `proc_pidinfo(PROC_TASKINFO)` process-CPU top. No subprocesses.
- `crates/performance-telemetry/src/linux_impl.rs`: typed `/proc/stat`, `/proc/meminfo`,
  `/proc/loadavg`, `/proc/diskstats`, `/sys/class/thermal_zone*/temp` parsers. Absent or
  malformed sources degrade to typed `CollectorFault`; nothing invented.
- Every Phase 20 discipline reused unchanged: MIN_INTERVAL_MS=250 clamp,
  `normalized()` clamping of hostile values, CollectorFault isolation, RAII.
- GPU stays Degraded; ThermalPowerClamp stays Degraded — no SMC/IOKit claims.
- `libc` declared strictly under
  `[target.'cfg(any(target_os = "macos", target_os = "linux"))'.dependencies]`
  WITH the owner-review waiver comment.
- Tests: tick-delta math vs injected counters; hostile counters clamp; every fallible
  sub-collector degrades to CollectorFault; Linux parser fixture tests (embedded texts;
  malformed lines skipped typed); live macOS integration.

## T4 — Selection + real-sample proof

`select_platform()`: cfg-selected WindowsPerfPlatform / MacosPerfPlatform /
LinuxPerfPlatform; SyntheticPerfPlatform retained for `--synthetic`, audits, tests and
offline UI. Capability matrix truth-upgrade: macOS cpu/memory/storage → Native with
code-backing grep-enforced by the audit.

GE real-sample test (`tests/phase27_real_sample.rs::real_sampler_aggregate_analyze_
pipeline_is_wellformed`): sampler at the 250 ms floor for ~12 ticks on the REAL
MacosPerfPlatform → ring.aggregate() → bottleneck::analyze(); asserts sample_count ≥ 10,
window_ms > 0, busy-bp ≤ 10_000, 64-hex digest, well-formed findings, no panic. GREEN.
Determinism remains pinned ONLY on the synthetic platform (real clocks are not
byte-deterministic — see docs/phase27/ARCHITECTURE.md §4).

## T5 — Renderer surface

- New AboutPanel (About/Diagnostics): live PlatformCapabilities matrix with typed chips
  Native/Degraded/NotAvailable + reason/note keys via catalogs; raw key shown when
  untranslated so missing i18n can never fake a state.
- PerformancePage shows the serving engine source honestly (native/synthetic).
- New additive wire ops: request tag 88 `GetEngineSourceRequest`, response tag 50
  `EngineSourceResponse`; desktop commands `get_platform_capabilities`,
  `get_engine_source`.
- i18n EN/AR parity gate extended to all new `about.*` keys.

## T6 — Audit

`scripts/phase27-adversarial-audit.py`: strict superset importing phase26's audit
in-process. New gates: honesty code-backing for every Native cell; provider conformance
(impl PerfPlatform, 250 ms floor, normalized(), CollectorFault, no-subprocess);
libc cfg-gating with waiver; unix-ipc feature gating; cfg(windows) freeze scan outside an
explicit allowlist; wire-freeze (no tags beyond 88/50 this phase; EventKind/envelope
ranges hold); composition wiring markers; i18n parity; GD-test presence incl. stale-path
recovery and permission assertions.

## T7 — Docs & ledger

docs/phase27/ARCHITECTURE.md (provider architecture, syscall tables, honest limits,
real-vs-synthetic determinism statement) · QUALIFICATION_DEBT.json append-only
(+QD-027-001 macOS Apple Silicon/Intel validation spread, +QD-027-002 Linux kernel-version
spread; QD-026-002 notes refined in docs — nothing closed without evidence) · this report.

## Gate tails

GB. `cargo test --workspace --jobs 2` ×2 — identical greens; exact counts recorded in the
standardized status report (run output /tmp/p27_ga_run1.txt, /tmp/p27_ga_run2.txt).
GC. phase27-adversarial-audit → PASS with exact check count (>349).
GG/GH. PHASE_27_BINARY_SAFE_PATCH built from the sealed-snapshot diff; apply → verify
round-trip byte-identical ×2; deterministic master archive rebuilt twice with identical
SHA-256 (+PHASE27_FINAL_SHA256.txt).
