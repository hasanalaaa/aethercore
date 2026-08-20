# ADR 0012 — Operation Kernel and Persistent IPC v7

- Status: Accepted
- Phase: 10

## Context

Through Phase 9, the service had strong principal ownership and one-shot consent, but the control plane still reflected an earlier connection-per-request architecture. The service composition/router had accumulated cross-domain coordination responsibilities, the renderer polled multiple domains independently, mutation exclusion remained distributed, and frequent progress updates risked coupling UI responsiveness to durable SQLite writes.

AetherCore needs one control plane that preserves the Phase 9 trust boundary while supporting live state, bounded concurrency and future Update orchestration.

## Decision

### One in-process Operation Kernel

Keep the maintenance service as a modular monolith. Do not create additional privileged microservices. Introduce `aethercore-operation-kernel` as the in-process authority for service-level authorization facade, state-machine facade, global mutation leasing, read-work budgets, recovery orchestration, session cancellation, live events and transient progress telemetry.

Domain crates retain their specialized Windows behavior and durable domain transactions. `operation-engine` remains the durable plan/state authority.

### One global machine mutation lease

Exactly one mutating workload may own the machine at a time. Driver Install, System Repair, Cleanup, Startup and Update share one RAII supervisor. A lease is transferred from request admission to a watcher only after the domain start succeeds and is released at a durable terminal/reboot boundary.

Read-only discovery uses a distinct bounded budget and may overlap across domains.

### Persistent principal-bound IPC v7 sessions

A named-pipe connection begins with `ClientHello`, captures the exact Phase 9 Windows principal once, and remains open for multiple requests and server events. Requests carry deadlines and session-scoped cancellation IDs. Request IDs cannot be simultaneously active twice.

A disconnect cancels request-scoped tokens for that session only. It does not blindly abort a durable mutation that has already crossed its safety boundary.

### Principal-local event sequence space

Do not use a global sequence counter. Each authenticated principal has an independent monotonic sequence and bounded replay window. This prevents another user's activity from being observable as unexplained sequence gaps.

Replay capture and subscription installation are atomic. If continuity is impossible, send an explicit `StreamReset`; never silently skip events. Rehydrate current typed state by publishing it through the same sequence-numbered Event Bus.

### Transient telemetry is not the safety ledger

Fine-grained progress is in-memory and streamed immediately. SQLite persists safety-critical transitions/evidence and intentionally coalesced progress only. No Phase 10 migration creates a frame-rate telemetry table.

### Modular schemas, stable package

Split the Protobuf source by domain while retaining package `aethercore.v1` and compatible existing field numbers. Add typed states/errors/session/event envelopes. Keep the old aggregate `.proto` as imports only during the v7 migration.

## Consequences

Positive:

- the renderer no longer needs domain polling timers;
- cross-domain mutation exclusion has one authority;
- stream continuity is explicit and testable;
- reconnect and subscriber lag have deterministic resynchronization;
- high-frequency UI progress no longer implies high-frequency `synchronous=FULL` writes;
- future update orchestration must use the same mutation lease;
- service bootstrap/router are materially smaller and easier to audit.

Tradeoffs:

- the service now owns long-lived session threads and bounded subscriber queues;
- existing domain crates without native event emitters require short-lived internal observation bridges for coarse state changes;
- cancellation is request-scoped unless a domain explicitly defines safe cooperative cancellation; durable mutations are intentionally not killed on UI disconnect;
- sequence/replay is service-epoch in-memory state, so service restart correctly requires reset/hydration rather than pretending continuity.

## Rejected alternatives

- Multiple privileged services: increases attack surface, installation complexity and distributed-state failure modes.
- Global event sequence: leaks cross-user activity patterns and creates ambiguous gaps.
- Unbounded queues: converts a slow renderer into service memory exhaustion.
- Persist every progress event: turns SQLite durability into a UI frame transport and creates unnecessary fsync pressure.
- Kill mutation on client disconnect/deadline: can interrupt Windows servicing at an unsafe point and violate Phase 3/4/5 recovery invariants.
