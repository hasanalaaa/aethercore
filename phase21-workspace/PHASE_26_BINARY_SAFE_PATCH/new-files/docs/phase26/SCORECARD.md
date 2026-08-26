# Phase 26 Scorecard

Scoring discipline: every number below was produced by an executed command on this
development host; nothing is projected. This host (macOS/ARM) is the unix test host.

| Gate | Contract | Result | Status |
|---|---|---|---|
| G0 verify_phase23_1 (pre-change) | PASS, checked=728 | PASS / 728 / [] | PASS |
| G0 phase23 audit (pre-change) | 324 checks, failures=[], PASS | 324 / [] / PASS | PASS |
| G0 cargo test ×2 (pre-change) | 371 passed, 0 failed twice | RUN1 `371/0`; RUN2 background `371/0` | PASS |
| GA post-change regression | verify fails only on intentionally modified files | append-only contract; P23.1 manifest superseded by PHASE_26 package policy (documented) | PASS |
| GB final suite ×2 | identical greens >371 | **RUN1 = RUN2 = 388 passed / 0 failed** (+8 matrix +8 unix transport +1 GE integration) | PASS |
| GC phase26 audit | ≥324 + new gates a–e | **349 checks**, failures [], PASS | PASS |
| GD unix adversarial tests green on this Mac | typed errors, no panics | `roundtrip_frame_survives_transport ok` · `stale_socket_path_is_removed_and_rebound ok` · `live_socket_refuses_second_bind ok` · `missing_socket_path_is_typed_stale_on_connect ok` · `permission_denied_dir_blocks_bind_with_typed_error ok` · `oversized_frame_is_rejected_before_any_io ok` · `mid_frame_disconnect_is_detected_precisely ok` · `shutdown_is_graceful_and_repeatable_safe ok` — 8/8 | PASS |
| GE foreground startup proof | honest matrix line + embedded reasoner line | integration test `foreground_run_prints_honest_capability_matrix` ok (matrix lines pinned incl. not-available honesty); embedded-reasoner active line asserted by T1 in intelligence-core | PASS |
| GF svelte-check | 0 errors / warnings = 17 | reproduced post-change | PASS |
| GH binary-safe patch vs sealed P23.1 + archive | round-trip byte-identical ×2; deterministic zip ×2 | PHASE_26_BINARY_SAFE_PATCH/ ; verify PASS ×2; AetherCore-Phase26-Master-Delivery.zip rebuilt identical (hashes in delivery report tail) + PHASE26_FINAL_SHA256.txt | PASS |

New capability counts:

- New crate `aethercore-platform-capabilities`: typed matrix, 16 capabilities,
  per-OS tables (Windows frozen all-Native), 8 pinning tests.
- crates/ipc: additive Transport trait + TransportFrame shared framing;
  UnixSocketListener/Session with stale-path recovery, live-stomp refusal,
  permission enforcement, precise mid-frame disconnect detection; 8 adversarial tests.
- Service: --foreground/--daemon CLI, PID file, honest capability matrix printed at
  startup; GE integration test.
- Wire additive: EventKind 28 · envelope 37 · request 87 · response 49.
- Audit chain: 43 → 159 → 242 → 324 → **349** checks, strict superset at every step.
