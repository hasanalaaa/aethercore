# AetherCore Enterprise Adversarial Audit & Convergence Report

## Status

This document records the post-Phase-16 recursive enterprise audit of AetherCore. The audit deliberately treats the Phase 0–16 verification history as a baseline, not as proof that no additional defects exist.

The convergence cycle used four engineering passes over the same source tree:

1. **Adversarial Auditor & Critic** — searched for second-order failure modes that static phase gates do not necessarily expose: reentrancy under locks, TOCTOU quota races, repeated raw Win32 ownership pairs, panic shortcuts, schema drift, renderer hot-path allocations, and observability blind spots.
2. **Chief Enterprise Architect** — converted confirmed findings into explicit invariants and ownership boundaries.
3. **Systems Innovator** — introduced bounded, privacy-safe runtime telemetry and reduced hot-path work while preserving privilege separation and zero renderer polling.
4. **Senior Staff Implementer** — implemented the refactors, added regressions, and created a new Enterprise master gate that cannot confer GA by itself.

The cycle repeated until all confirmed defects in the scope below had an implementation and a verification hook. This is not a mathematical claim that the program is defect-free. Native Windows FFI behavior, WebView2 composition, SCM/UAC, signed installer lifecycle, and long-running leak behavior still require execution on qualified Windows hosts.

## Enterprise invariants after convergence

### Privilege and authority

- The non-elevated desktop remains separated from the LocalSystem maintenance service.
- No new arbitrary URL, path, command, executable, or installer-argument authority crosses the privileged boundary.
- Update installation remains under the fixed Burn/MSI path and `MutationSupervisor::Update`.
- Autonomous maintenance remains read-only and commit-fenced.

### Concurrency and publication

- Update snapshot observers are called only after the update-state mutex has been released.
- Support-bundle preparation reserves global/per-owner capacity before archive/signature I/O begins.
- Support-bundle filesystem deletion is performed outside quota-state locks; failed deletion keeps state retained rather than silently freeing quota.
- EventBus lag/disconnect telemetry is bounded and publication remains non-blocking.
- EventBus subscriptions unregister immediately on drop; cleanup does not depend on a future event.
- IPC outbound queues are bounded by both frame count and resident byte budget; reader/writer disconnect notification is at-most-once.
- Desktop inflight admission cannot exceed the server-advertised session limit.
- Update upload filesystem I/O is isolated per upload; final staged artifacts are owner-scoped and content-addressed.

### Windows FFI ownership

A new `aethercore-windows-foundation` crate owns common lexical resource pairs through RAII:

- Win32 `HANDLE` / `CloseHandle`
- SCM `SC_HANDLE` / `CloseServiceHandle`
- COM MTA initialization / `CoUninitialize`
- thread impersonation / `RevertToSelf`
- background thread mode begin / end

The abstraction is intentionally narrow. Higher-level guards whose release semantics carry domain meaning remain local rather than being hidden behind a generic wrapper.

### Privacy and diagnostics

- EventBus support evidence is owner-scoped. Global owner/subscriber counts are never serialized into a user-owned support bundle.
- Installation-local support proofs and independent fingerprint validation from Phase 15 are unchanged.
- No raw minidump or raw EventLog export authority was introduced.

### UI and motion

- IPC events consumed by the UI are represented as a TypeScript discriminated union with an exhaustive reducer.
- The remaining interactive `any` escape in driver scan state mapping was removed.
- Motion/accessibility media queries are cached by a singleton preference observer instead of being recreated from animation-frame callbacks.
- Reduced-motion/transparency/contrast changes can propagate immediately to subscribed primitives without renderer polling.

## Confirmed defects closed

The complete defect/root-cause/refactoring matrix is maintained in `ENTERPRISE_TRANSFORMATION_MATRIX.md`. The highest-impact closures are:

1. Support-bundle quota reservation linearization before expensive work.
2. Update observer publication outside all state-lock paths, including direct submit/stage/finalize paths.
3. Centralized RAII for repeated Win32 ownership pairs on privilege-sensitive paths.
4. Typed/fail-closed replacements for production panic shortcuts and impossible WMI branches.
5. Owner-scoped EventBus backpressure telemetry for diagnosability without cross-principal leakage.
6. Exhaustively typed UI kernel events and a cached, live accessibility preference runtime.
7. Bounded IPC v7 client/server writer queues so slow pipe peers cannot indefinitely block request/event producers; queues are bounded by both frame count and bytes, live sends fail closed on backpressure, and bootstrap/replay has a finite enqueue deadline.
8. Immediate EventBus unsubscribe on subscription drop, at-most-once reader/writer disconnect notification, and client-side enforcement of the server-advertised inflight limit.
9. Update staging registry/I/O separation with per-upload locks, owner-scoped start reservations, late-chunk fences, and owner/content-addressed service paths.
10. Restart-safe stale staging cleanup that cannot delete the active durable-execution artifact and refuses non-owned/non-regular staging entries.
11. Common Win32 HANDLE closure centralized in `windows-foundation`, including desktop broker process handles and semantic mutation guards.
12. Enterprise verification that makes these architectural invariants part of CI/release qualification and binds all 17 targeted regressions into GA runtime evidence.

## Sustained resource-growth qualification

The Enterprise release stress profile adds `enterprise-soak-analyze.py` after the Phase 16 24-hour soak. It evaluates the complete resource sample series rather than only baseline/final endpoints:

- positive least-squares slope per hour for Private Bytes, handles and threads;
- first-quartile versus last-quartile median growth;
- minimum duration/sample-count requirements;
- fail-closed version/profile binding to the same installed release.

`phase16-seal-release.ps1` now requires both `aethercore.enterprise-stress-evidence.v1` and `aethercore.enterprise-resource-trend.v1` from the **release** profile before a GA seal can be created. These bounds materially strengthen leak detection, but remain empirical qualification rather than a mathematical proof of leak impossibility.

## Verification boundary

The Enterprise source gate verifies source architecture, static invariants, exact targeted-test selectors, and integration with Phase 16 release orchestration. On Windows, `verify-enterprise.ps1` additionally runs Cargo formatting/clippy/tests, strict UI checking/building, and optionally the enterprise stress matrix.

The following cannot be honestly proven in this Linux authoring environment:

- compilation against the Windows SDK and live Win32/COM/WMI/SCM semantics;
- UAC token/elevation behavior and Authenticode chain validation;
- Tauri/WebView2 visual composition, physical display DPI and input hardware behavior;
- signed Burn/MSI install/repair/uninstall lifecycle;
- 24-hour resource-growth behavior or the absence of every possible memory/handle leak.

`verify-production.ps1` remains the only GA sealing path and now requires the Enterprise source audit in addition to the Phase 16 production evidence model.
