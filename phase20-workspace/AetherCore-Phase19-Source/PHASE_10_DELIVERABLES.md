# Phase 10 Deliverables — Operation Kernel & IPC v7 Evolution

## Milestone outcome

Phase 10 converts the Phase 9 principal-owned request boundary into a persistent, event-driven control plane. The privileged service is decomposed around a single in-process Operation Kernel; machine mutation ownership is centralized; authenticated named-pipe sessions carry RPC responses and live typed events; UI polling is removed; and high-frequency progress telemetry is separated from SQLite safety checkpoints.

Phase 9 security invariants remain authoritative: the UI is untrusted, the service derives the Windows principal from the exact pipe client token, plans are owner-bound and immutable, one-shot consent is consumed atomically, cleanup uses exact handle-level file identity, and dependency/release verification remains fail-closed.

## 1. Operation Kernel

New crate: `crates/operation-kernel`.

The kernel is the service control-plane composition root and contains:

- `AuthorizationManager` — narrow facade over Phase 9 consent-intent authority.
- `StateMachine` — owner-scoped plan/state access without moving durable domain transitions out of `operation-engine`.
- `MutationSupervisor` — one global RAII machine-mutation lease across Driver Install, System Repair, Cleanup, Startup and the reserved Update workload.
- `ReadBudgetManager` — bounded, single-flight expensive read workloads while permitting useful cross-domain read concurrency.
- `RecoverySupervisor` — explicit startup recovery task orchestration.
- `EventBus` — bounded, principal-scoped live event plane with replay and reset semantics.
- `CancellationRegistry` / `RequestContext` — session-scoped cancellation identifiers and request deadlines.
- `ProgressTelemetryStore` — in-memory latest progress with an observer bridge into the Event Bus.

`OperationKernel` is `#![forbid(unsafe_code)]`; Windows FFI remains outside the kernel.

## 2. Service decomposition

`services/maintenance-service/src/main.rs` is now bootstrap/SCM composition rather than the request monolith.

The service is split into:

- `composition.rs` — constructs domain engines and the shared Operation Kernel;
- `server.rs` — authenticated persistent named-pipe session runtime;
- `errors.rs` — typed domain/kernel error classification into `ErrorCode`/`ErrorInfo` without cross-principal detail leakage;
- `router.rs` — admission, principal/deadline checks and domain dispatch only;
- `protocol.rs` — domain-to-Protobuf typed mapping;
- `streaming.rs` — ordered hydration, discovery observation and mutation-terminal watchers.

The Phase 10 source audit rejects a return to a monolithic `main.rs` or `router.rs`.

## 3. Persistent authenticated IPC v7

The production endpoint is `\\.\pipe\AetherCore.Maintenance.v7`.

A desktop connection now performs one `ClientHello` / `ServerHello` handshake and remains open for multiple requests and live events. Each accepted session captures a single immutable Phase 9 `PrincipalContext` from the exact named-pipe client and all RPC/event activity is scoped to its `binding_key()`.

Session protocol capabilities include:

- persistent multiplexed request/response envelopes;
- per-request absolute deadlines;
- session-scoped cancellation IDs;
- duplicate active request-ID rejection on both client and server;
- maximum 32 authenticated sessions, plus a 4-session cap per kernel-observed user SID so one local account cannot monopolize the service;
- maximum 8 in-flight requests per session;
- directional frame bounds: client/control frames are capped at 384 KiB while large server responses/events retain the 8 MiB ceiling;
- explicit `ClientGoodbye` and disconnect cancellation signaling, including one-shot broker client cleanup.

The elevated consent broker retains the fixed-purpose one-shot helper over the same v7 session transport. The desktop uses the persistent `SessionClient`.

## 4. Principal-local event sequence and replay

Event sequence numbers are monotonic **per authenticated principal**, not global across the service. This avoids cross-user activity-pattern leakage and makes sequence discontinuities meaningful to the connected user.

Each principal stream has bounded replay storage. Subscription installation and replay capture happen under one Event Bus lock, closing the replay-to-live race.

If continuity cannot be proven, the server emits an explicit `StreamReset`:

- `ReplayWindowExceeded` — requested cursor is older than retained replay;
- `SubscriberLagged` — bounded subscriber queue overflowed;
- `SequenceReset` — client cursor belongs to a previous service/kernel epoch.

After reset, the server publishes a complete typed hydration image through the **same ordered Event Bus sequence space** before continuing live delivery. A renderer reload on an already-live Tauri/pipe session uses the typed `HydrateSession` RPC after listener registration to request the same ordered hydration image without reintroducing polling. Event loss is never silently hidden.

Subscriber queues are bounded. Queue overflow is promoted to `SubscriberLagged`, stale queued telemetry is drained, and the stream is reset/hydrated rather than silently dropping frames.

## 5. Machine Mutation Supervisor

All mutation entry paths acquire the shared kernel lease before delegating to the domain coordinator:

- Driver Install;
- System Repair;
- Cleanup;
- Startup changes/restores;
- Update is reserved in the enum now so Phase 15 cannot bypass the same machine-wide exclusion rule.

The lease is RAII-backed and is transferred to the mutation watcher after successful start. It is released only when the domain reaches a durable terminal state (or the defined reboot-pending boundary for reboot-aware workloads). Acquisition and release are themselves published as typed principal-scoped events. Cross-principal contention returns a generic busy condition and never discloses the foreign workload, plan ID, or owner; even lease snapshots are owner-scoped by API.

Read-only discovery workloads use the separate `ReadBudgetManager`; they can overlap across domains within the global budget but each expensive kind is single-flight.

## 6. Modular typed Protobuf contracts

The former monolithic contract is split into:

- `common.proto`;
- `operations.proto`;
- `drivers.proto`;
- `repair.proto`;
- `cleanup.proto`;
- `startup.proto`;
- `diagnostics.proto`;
- `events.proto`.

`aethercore.proto` remains only as an import-only compatibility aggregator. `events.proto` is the prost build graph root.

Phase 10 adds typed `ErrorInfo`, `ErrorCode`, `OperationState`, `DiscoveryState`, `RiskLevel`, event kinds, mutation lease state/workload enums, session envelopes and stream-reset reasons. `services/maintenance-service/src/errors.rs` maps kernel/domain failures into typed status/retry semantics. Foreign domain snapshot ownership mismatches are deliberately non-enumerable (`NotFound` + generic state-unavailable detail). Existing display strings are retained where necessary for v7 migration compatibility, but routing/security decisions no longer depend on those strings.

## 7. High-performance telemetry / durable safety split

Driver Install, System Repair, Cleanup and Startup publish fine-grained progress into the shared in-memory `ProgressTelemetryStore`. The kernel observer immediately converts those updates to principal-scoped `ProgressTelemetryEvent` frames.

SQLite remains the crash-safety ledger, not a frame-rate UI transport. Durable writes continue for:

- plan/state transitions;
- one-shot consent consumption;
- exact mutation evidence;
- recovery checkpoints;
- terminal results;
- deliberately coalesced coarse progress where operationally useful.

Driver Install persists WUA progress only on stage/candidate change, >=5 percentage-point movement, or >=2 seconds since the previous durable checkpoint. Transient telemetry is keyed by `(owner_principal_key, plan_id)` and exposes only owner-scoped lookup/clear APIs; live status overlays only telemetry for the authenticated owner newer than the durable record.

Migration `0007_phase10_kernel.sql` adds query indexes required by the kernel while intentionally adding no live-telemetry table.

## 8. UI streaming migration

The Tauri bridge owns one persistent IPC session, serializes reconnects, identity-checks stale-session invalidation so a late failure cannot erase a newer replacement, remembers the last event sequence and emits:

- `aethercore://kernel-event`;
- `aethercore://stream-reset`;
- `aethercore://session-state`.

The Svelte renderer registers event listeners before starting the IPC session. All legacy `setInterval` / `clearInterval` domain polling timers are removed. Initial state and resynchronization arrive through ordered typed hydration events.

## 9. Verification gates

New/updated gates:

- `scripts/phase10-architecture-audit.ps1` — authoritative Phase 10 source/architecture invariant gate;
- `scripts/phase10-kernel-audit.ps1` — compatibility wrapper to the architecture audit;
- `scripts/verify-phase10.ps1` — inherits the complete Phase 9 gate, then tests contracts, kernel, IPC, persistence, Driver Install, System Repair, Cleaner and Startup Manager; checks service/desktop/workspace compilation, Rust formatting, Svelte type/accessibility/build and platform-neutral invariants.
- `scripts/static_validate.py` — Phase 0–10 platform-neutral gate.

The Phase 10 gate specifically verifies per-principal sequence privacy, replay exhaustion/reset behavior, bounded-subscriber lag/reset behavior, replay/subscription race closure, session disconnect cancellation isolation, duplicate request/cancellation rejection, per-user/global session quotas, stale-session reconnect race closure, directional frame limits, machine mutation exclusion, cross-principal busy privacy, owner-scoped lease/telemetry APIs, typed error semantics, read-watcher lease release, bounded read concurrency, modular typed schemas, polling elimination, service decomposition and telemetry/journal separation.

## 10. Authoring-environment result and release boundary

The Linux authoring environment cannot execute Cargo/Rust, PowerShell, Windows named pipes/tokens, SCM/UAC, WiX, Authenticode or the installed desktop session. Therefore Phase 10 does not claim native Windows certification from this environment.

The platform-neutral Phase 0–10 source gate currently passes **149/149** invariants in this delivery. Independent Protobuf import/tag, structured-file and SQLite fresh/Phase 9→10 migration checks also pass. The authoritative Windows command remains:

```powershell
.\scripts\verify-phase10.ps1
```

On the trusted Windows release environment, the approved dependency freeze inherited from Phase 9 remains mandatory and `verify-phase10.ps1` is fail-closed through the inherited `verify-phase9.ps1` gate.
