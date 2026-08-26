# Phase 26 — Universal Platform Foundation

Status: complete on this host (macOS/ARM). Windows behavior FROZEN — zero regression
tolerance enforced by gate P26-c. All new code is cfg(unix)-additive or platform-neutral.

## CX Decisions (owner directives, verbatim as ADRs)

**CX-1. Windows behavior is FROZEN.** Zero regression tolerance; every existing
cfg(windows) block is byte-frozen relative to sealed P23.1 (gate P26-c scans all
non-allowlisted .rs files). The named-pipe implementation remains the Windows transport.

**CX-2. Capability honesty is contractual.** `NotAvailable` is a first-class typed
answer carrying a closed-vocabulary reason key. The product NEVER simulates an
unavailable capability on any platform.

**CX-3. One wire contract everywhere.** Unix domain sockets carry byte-identical
framing to the Windows named pipe: 4-byte little-endian length prefix + protobuf body.
Selection happens at composition time via cfg — no runtime transport negotiation.

**CX-4. macOS IPC authentication is dir+socket permissions today.** The private
0700 directory + 0600 socket file is the Phase 26 auth boundary; SO_PEERCRED does not
exist on macOS and peer-cred depth is honestly deferred (QD-026-001). Linux gets the
same boundary now with SO_PEERCRED deepening later.

**CX-5. Daemon lifecycle managers are out of scope.** `--foreground` runs the
identical service loop attached to console; `--daemon` = foreground + structured file
logs + PID file. launchd/systemd installation arrives in Phase 28.

**CX-6. The capability matrix is data, not behavior gates.** Phase 26 only REPORTS;
no unix capability is claimed beyond what exists (telemetry/timeline/intelligence/care).

**CX-7. Every cfg(unix) test must actually run here.** This Mac is the unix test host;
the 8 unix transport tests and 8 matrix tests execute natively in CI-equivalent runs.

## Honest capability matrix (as printed at service start)

| Capability | Windows | macOS | Linux |
|---|---|---|---|
| telemetryCpu | native | native | native |
| telemetryMemory | native | native | native |
| telemetryStorage | native | native | degraded (distro variance) |
| telemetryGpu | native | degraded (macOS GPU limited) | degraded (distro variance) |
| thermalPowerClamp | native | degraded (via NQ) | degraded (distro variance) |
| driverServicing | native | not-available (no driver store) | not-available |
| systemRepairDism/Sfc/Wua | native ×3 | not-available ×3 | not-available ×3 |
| processGovernorEcoQos | native | not-available (no EcoQoS) | not-available |
| gameModeProfile | native | not-available (Windows-only API) | not-available |
| restorePoints | native | not-available | not-available |
| windowsUpdate | native | not-available | not-available (Windows-only API) |
| timelineIntelligence | native | native | native |
| localIntelligence | native | **native** (embedded Qwen2.5-1.5B, Metal) | native |
| careOrchestration | native | native | native |

## Deliverables

1. `crates/platform-capabilities` — typed matrix, per-OS tables, 8 pinning tests.
2. `crates/ipc` — additive `Transport` trait + shared `TransportFrame`; new
   `unix_impl.rs` (`UnixSocketListener/Session`) with stale-path recovery, live-socket
   stomp refusal, permission enforcement, precise mid-frame disconnect detection;
   8 adversarial tests green on this Mac.
3. Service CLI: `--foreground` / `--daemon` (+ PID file); startup prints the full
   honest matrix line-by-line. GE integration test pins the output.
4. Wire additive: EventKind 28 · envelope 37 · request 87 · response 49
   (`GetPlatformCapabilities` surface for UI/CLI in a later phase).
5. Audit chain extended: 43→159→242→324→**349** checks, strict superset.
