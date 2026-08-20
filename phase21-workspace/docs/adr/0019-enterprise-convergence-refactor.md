# ADR 0019 — Enterprise Adversarial Convergence Refactor

**Status:** Accepted — Post-Phase-16 Enterprise Convergence

## Context

Phase 16 established the Windows GA qualification and cryptographic release-seal model. A subsequent adversarial audit intentionally ignored the assumption that prior passing gates implied architectural perfection and searched for second-order failure modes across update orchestration, support export, Windows FFI ownership, IPC observability, and the Svelte/TypeScript event/motion layer.

Confirmed issues included a support-bundle preparation quota TOCTOU window, update observer callbacks executed while update-state locks were still held on multiple paths, repeated raw Win32 acquire/release pairs, panic-style production shortcuts, weakly typed UI event payloads, and repeated media-query reads in animation hot paths.

## Decision

AetherCore adopts the following post-Phase-16 enterprise invariants:

1. **Reservation before expensive support-bundle work.** A prepared-bundle slot is reserved atomically per owner and globally before key/archive/signature/file I/O. Reservations count toward the same bounded quota as ready bundles.
2. **No external publication under update-state locks.** Update state may mutate under `states`, but observers are invoked only after a snapshot is cloned and the lock is released.
3. **Common Win32 ownership pairs use RAII.** A narrow `aethercore-windows-foundation` crate owns HANDLE, service-handle, COM apartment, impersonation, and background-thread-mode lifetimes. Domain-specific synchronization/SCM semantics remain explicit.
4. **Production panic shortcuts are rejected.** Provider inconsistencies and impossible-state assumptions fail as typed errors or controlled fail-closed paths rather than `unwrap`, `expect`, or `unreachable!` in production runtime code.
5. **EventBus observability is privacy-scoped.** Global counters may exist internally, while support export receives only metrics for the authenticated owner principal.
6. **UI stream events are exhaustively typed.** Renderer reducers consume a discriminated union and reject schema drift at TypeScript compile time.
7. **Motion preferences are a live singleton service.** OS/accessibility media queries are cached and changes are subscribed to; animation-frame callbacks do not allocate new media-query objects.
8. **IPC backpressure is bounded in work and memory.** Persistent session writes are isolated behind dedicated writer pumps with frame-count and byte budgets; bootstrap enqueue is finite, live saturation fails closed, disconnect notification is at-most-once, and clients enforce the negotiated inflight limit.
9. **Update staging registry locks never own filesystem latency.** Registry coordination is short-lived; each upload has independent mutation locking, one active start reservation per owner, a closed fence, and a service-derived owner/content-addressed final path.
10. **Recovered installer artifacts are cleanup-protected.** Stale cleanup excludes active/staged authority paths and only deletes recognized regular staging files.
11. **Enterprise qualification is additive.** Historical Phase 0–16 gates remain intact. `verify-enterprise.ps1` adds architectural/source/compiler/UI/regression/stress gates, while only `verify-production.ps1` may create the GA seal and now requires all 17 Enterprise targeted regressions in release evidence.

## Consequences

- Burst support requests cannot create unbounded concurrent archive/signing work merely by racing the pre-check.
- Future observer implementations can safely read coordinator state without self-deadlocking on the same mutex.
- Common Win32 cleanup is structurally tied to scope exit and early-return paths.
- Runtime backpressure failures become diagnosable without disclosing other users' activity in support bundles.
- UI event schema drift and interactive `any` escapes are reduced to compile-time failures.
- Accessibility preference changes no longer require the next pointer or navigation event to take effect.
- Enterprise release qualification becomes stricter without retroactively rewriting Phase 16 evidence claims.

## Non-goals

This ADR does not claim that finite static/tests prove absence of all defects or leaks. Windows-native, signed-installer, long-soak, accessibility, physical-DPI, and WebView2 behavior remain execution-qualified by the Phase 16/Enterprise Windows gates.
