# Phase 27 — Architecture: Native Unix Providers & Composition Wiring

Status: SEALED · Supersedes nothing · Extends docs/phase26/ARCHITECTURE.md (CX-1…CX-7 remain binding)

## 1. Provider architecture

`crates/performance-telemetry` selects its platform implementation at compile time
(`select_platform()` in src/lib.rs):

| cfg | provider | engine source label |
|---|---|---|
| `windows` | `WindowsPerfPlatform` (frozen, P20) | native |
| `target_os = "macos"` | `MacosPerfPlatform` (P27) | native |
| `target_os = "linux"` | `LinuxPerfPlatform` (P27) | native |
| `--synthetic` / audits / tests | `SyntheticPerfPlatform` | synthetic |

The synthetic platform remains a first-class, always-available selection for audits,
determinism tests and offline UI. It is never silently substituted for a native provider.

### 1.1 macOS provider (`macos_impl.rs`) — libc-backed syscalls ONLY

| Collector | Syscall/libc symbol | Notes |
|---|---|---|
| CPU ticks | `host_statistics64(HOST_CPU_LOAD_INFO)` via `host_processor_info`-style counts | user/system/idle/nice ticks; delta between ticks |
| Page size | `sysconf(_SC_PAGESIZE)` | libc 0.2 exposes no `host_page_size`; sysconf is the documented equivalent |
| Memory | `sysctl(CTL_VM: VM_SWAPUSAGE)` + `HOST_VM_INFO64` (`vm_statistics64`) | active/wired/compressed/swap usage |
| Load average | `getloadavg` | 1-minute figure |
| Storage capacity/queue | `statfs` (f_bavail/f_blocks), iostat proxy from disk counters via `sysctl` | active-capacity + queue-depth proxies |
| Process CPU top | `proc_pidinfo(PROC_TASKINFO)` over pid enumeration | per-process CPU deltas |

No subprocesses anywhere in the provider (`std::process::Command` is audit-banned).
GPU stays **Degraded** (`cap.note.macosGpuLimited`) and thermal/power stays **Degraded**
(`cap.note.macosThermalViaNq`): no SMC/IOKit user-client access is implemented or claimed.
The `libc` dependency is declared strictly under
`[target.'cfg(any(target_os = "macos", target_os = "linux"))'.dependencies]` with the
owner-review waiver comment required by the audit allowlist.

### 1.2 Linux provider (`linux_impl.rs`) — typed `/proc` parsers

| Source | Parser | Failure semantics |
|---|---|---|
| `/proc/stat` | `parse_proc_stat_cpu` → tick delta across calls (first tick: in-tick double read) | absent/garbled → `CollectorFault`, never invention |
| `/proc/meminfo` | `parse_proc_meminfo` (MemTotal/MemAvailable/MemFree…) | missing keys → fault |
| `/proc/loadavg` | `parse_loadavg` | malformed line → fault |
| `/proc/diskstats` | `parse_diskstats` (queue/io-tick proxies; layout varies) | distro variance stays honestly Degraded (QD-026-002 unchanged) |
| `/sys/class/thermal_zone*/temp` | present-zone scan | absence → `CollectorFault` |

Every discipline from Phase 20 is reused unchanged: `MIN_INTERVAL_MS = 250` clamp,
`PerfSnapshot::normalized()` clamping of hostile values, `CollectorFault` isolation
(one failing collector never fails the snapshot), RAII resource handling.

## 2. Unix transport composition (T1)

On unix with feature `unix-ipc`, `services/maintenance-service` binds the SAME router
logic that serves named pipes on Windows to a `UnixSocketListener`:

- rendezvous: `default_socket_dir()` = `${AETHERCORE_IPC_ROOT:-/tmp}/aethercore-ipc/v7`
  (short by construction — macOS `sun_path` is 104 bytes);
- contract: dir 0700 / socket 0600 via the existing `ensure_private_dir`; stale-path
  recovery after a crashed prior run; live-stomp refusal ("already served by a live
  process");
- principal binding (CX-4/QD-026-001): the connecting peer is bound to the socket-owning
  user (`socket_owner_principal`). SO_PEERCRED deepening stays deferred and no deeper
  authentication is claimed;
- framing: byte-identical 4-byte LE length prefix + protobuf v7 handshake
  (`ClientHello`/`ServerHello`), one wire contract across transports;
- shutdown: SIGINT/SIGTERM flip the same flag as the Windows SCM Stop control; the accept
  loop polls in bounded windows so the flag is honored promptly;
- apps/desktop mirrors the same cfg split: on unix it connects through
  `UnixSocketSession` to the identical path.

Integration proof (this Mac, GD gate): real binary spawn → handshake →
`GetPlatformCapabilities` + `Ping` + `GetEngineSource` round trips verbatim → kill →
restart rebinds the stale socket path and serves again (services/maintenance-service/
tests/phase27_unix_ipc.rs).

## 3. Honest capability matrix upgrade (T4)

macOS rows now backed by code: `telemetryCpu`, `telemetryMemory` (native via §1.1),
`telemetryStorage` (statfs). Linux rows: cpu/memory native via §1.2 parsers; storage
keeps the distro-variance note. GPU/thermal stay Degraded; Windows-only rows stay
NotAvailable. The phase27 audit honesty gate grep-verifies every Native claim against
provider symbols; a claim without code fails the audit.

## 4. Real vs synthetic determinism statement

Determinism tests REMAIN on the synthetic platform. Real-clock sampling is wall-clock
driven: tick boundaries jitter, counter deltas vary run-to-run, and macOS/Linux expose
no deterministic tick source at the 250 ms floor. Byte-deterministic replay is therefore
defined ONLY over `SyntheticPerfPlatform`. Real-sample tests assert structural validity
(sample_count, digest format, well-formed findings) — never exact values
(tests/phase27_real_sample.rs).

## 5. Renderer surface (T5)

- `apps/ui/src/components/AboutPanel.svelte`: live PlatformCapabilities matrix with typed
  availability chips (Native/Degraded/NotAvailable) + reason/note keys resolved through
  the message catalogs; raw key shown when untranslated (a missing translation can never
  masquerade as a different state).
- PerformancePage shows the serving engine source honestly (native/synthetic chip).
- New Tauri commands `get_platform_capabilities` / `get_engine_source` forward the
  additive wire operations to the service.
- i18n: every new `about.*` key exists in BOTH catalogs (EN/AR parity gate extended).

## 6. Wire changes (additive only)

- operations.proto Request oneof: `GetEngineSourceRequest get_engine_source = 88`.
- operations.proto Response oneof: `EngineSourceResponse engine_source_response = 50`
  (+ capabilities.proto gains the shared `EngineSourceResponse` message).
- NO other tags allocated; EventKind/envelope ranges unchanged (audit-enforced).

## 7. Honest limits

- macOS principal depth = directory-permission boundary (documented; QD-026-001 open).
- Linux storage queue metrics are proxies; layouts vary across kernels/distros
  (QD-026-002 notes refined, not closed).
- macOS provider validated on Apple Silicon (this host); Intel spread tracked in
  QD-027-001. Linux kernel-version spread tracked in QD-027-002.
