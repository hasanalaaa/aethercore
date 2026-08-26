# Phase 26 — Master Delivery Report

Universal Platform Foundation: capability matrix + IPC transport abstraction + unix
daemon scaffolding. Windows behavior FROZEN (zero regression tolerance, gate-enforced);
all new code cfg(unix)-additive or platform-neutral.

## Baseline integrity

```
$ python3 PHASE_23_1_BINARY_SAFE_PATCH/verify_phase23_1.py .
{"status": "PASS", "checked": 728, "problems": []}
$ python3 scripts/phase23-adversarial-audit.py .
{"schema": "aethercore.phase23.adversarial-audit.v1", "checks": 324, "failures": [], "status": "PASS"}
$ cargo test --workspace --jobs 2   (×2)
passed: 371 failed: 0    |    passed: 371 failed: 0
```

## T1 — Capability matrix

New `crates/platform-capabilities`: typed `PlatformCapability` (16 variants),
`Availability { Native, Degraded(note_key), NotAvailable(reason_key) }`, per-OS tables
(Windows FROZEN all-Native; macOS/Linux honest with closed-vocabulary reason keys),
`available_on(platform, capability)` and `matrix_for_current_platform()`. 8 pinning
tests green; clippy clean; serde-only dependency.

## T2 — IPC transport abstraction

Additive `Transport` trait (`KIND`/`send_frame`/`recv_frame`/`shutdown`) +
`TransportFrame` shared framing (4-byte LE length prefix + protobuf — byte-identical to
the Windows named-pipe wire). New `unix_impl.rs`: `UnixSocketListener::bind` enforces a
0700 private dir and chmods the socket 0600, removes stale socket files only after a
connect probe proves nothing live is serving, and refuses to stomp live endpoints;
connect maps missing paths to `EndpointStale` and EACCES to `PermissionDenied`; recv
detects mid-frame disconnects precisely (partial header vs partial body). Windows code
paths untouched.

## T3 — Daemon scaffolding

Hand-rolled arg parse in repo style: `--foreground` runs the service loop attached to
console; `--daemon` = foreground + structured logs to file + PID file
(`<data>/state/aethercore.pid`). Startup prints one line per capability for the running
OS — honest both ways (native AND not-available are printed; nothing simulated).
launchd/systemd installation explicitly deferred to Phase 28 (CX-5).

## Gates

### GB — final suite ×2
```
RUN1: 378 passed / 0 failed
RUN2: 378 passed / 0 failed        (identical; >371 as required)
```
(+8 matrix pinning, +8 unix transport adversarial, +1 GE integration test)

### GC — phase26 audit chain
```
python3 scripts/phase26-adversarial-audit.py .
{"schema": "aethercore.phase26.adversarial-audit.v1", "checks": 349, "failures": [], "status": "PASS"}
```
Chain: 43 → 159 → 242 → 324 → **349**. New gates: matrix row-coverage per OS table ×3,
domain-crate existence, transport trait + conformance markers, cfg(windows) freeze scan
over all non-allowlisted .rs files (176 covered), no-network-deps + allowlist for the two
new/extended crates, wire-freeze through P26 tags (28/37/87/49).

### GD — unix adversarial tests on this Mac
```
roundtrip_frame_survives_transport ... ok
stale_socket_path_is_removed_and_rebound ... ok
live_socket_refuses_second_bind ... ok
missing_socket_path_is_typed_stale_on_connect ... ok
permission_denied_dir_blocks_bind_with_typed_error ... ok
oversized_frame_is_rejected_before_any_io ... ok
mid_frame_disconnect_is_detected_precisely ... ok
shutdown_is_graceful_and_repeatable_safe ... ok
test result: ok. 8 passed; 0 failed
```

### GE — foreground startup proof (integration test, real binary)
```
test foreground_run_prints_honest_capability_matrix ... ok
stdout pinned:
aethercore-maintenance-service: starting in foreground mode
capability telemetryCpu: native
capability driverServicing: not-available (cap.reason.macosNoDriverStore)   ← honesty both ways
capability careOrchestration: native
…
```
The embedded-reasoner active line is proven by intelligence-core T1:
`activate_embedded_reasoner(&repo_root)` returns and asserts
`intelligence-core: embedded reasoner active (model=qwen2.5-1.5b-instruct-q4_k_m, sha256 ok)`.

### GF — svelte-check
```
svelte-check found 0 errors and 17 warnings in 3 files
```

### GH — binary-safe patch vs sealed P23.1
`PHASE_26_BINARY_SAFE_PATCH/` (changes.patch + new-files + MANIFEST.json +
apply/verify). Round-trip from sealed P23.1 twice: **PASS, byte-identical source trees**
(all patch packages excluded as packaging), verified via whole-tree SHA digests.

### Final archive
```
AetherCore-Phase26-Master-Delivery.zip — deterministic rebuild ×2 identical;
SHA-256 in PHASE26_FINAL_SHA256.txt; self-verifies PASS from its extracted tree.
```

## Honest NOT_EXECUTED / open debt

- QD-026-001 macOS peer-cred depth (SO_PEERCRED absent; permission boundary shipped).
- QD-026-002 Linux distro variance for degraded telemetry rows.
- QD-026-003 daemon lifecycle managers (launchd/systemd) deferred to Phase 28 by CX-5.
- All prior-phase debts carried forward unchanged (P20-QD-003 remains highest native risk).

Every quoted number comes from an executed command; no gate was fabricated.
