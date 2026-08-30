# ADR 0021 — Recursive Adversarial Hardening and Worker-Owned Authority

## Status

Accepted for source. Qualified Windows compilation, native fault injection, signing and release evidence are still required.

## Context

A later recursive review of the Zenith tree found assumptions that historical static gates did not prove: plan presentation duplicated immutable data outside the digest; per-principal replay/update caches had no retirement; per-session request admission did not bound workers across reconnect churn; and, most importantly, machine mutation/read-budget leases were owned by status watchers instead of the threads doing the protected work.

A watcher is an observation mechanism. It must never be the lifetime authority for privileged work. Thread-creation failure also has to be treated as a normal resource-failure path rather than a panic after consent/state has already advanced.

## Decision

1. `immutable_json` is the authoritative immutable consent/execution representation. After SHA-256 verification, every duplicated immutable query/presentation column must match it. `PlanView` renders verified material.
2. EventBus retains at most 128 inactive owner streams under normal operation. Only streams without subscribers are eviction candidates; replay loss is represented through the existing reset/hydration contract.
3. Update owner state uses oldest-quiescent retention at 128. Staging, staged, awaiting-consent and installing states are non-evictable; correctness may soften the numeric cap.
4. Every accepted request worker consumes a process-global atomic RAII slot in addition to per-session admission. New work fails closed when 64 workers are active.
5. `MutationLease` is matched against exact workload/plan/owner identity, unleased mutation start entry points are removed, and the lease is moved into the actual driver/repair/cleanup/startup worker. Status watchers hold no mutation authority. No public unleased mutation start API remains, and the private worker boundary requires a concrete non-optional lease.
6. `ReadBudgetLease` is matched against exact workload identity, unbudgeted scan/assessment entry points are removed, and the lease is moved into the actual expensive collector worker. Status watchers hold no read-budget authority. No public unleased expensive-read start API remains, and the private worker boundary requires a concrete non-optional lease.
7. Runtime threads created after authority/state changes use named fallible `thread::Builder`; failure restores counters/tokens/state or records a terminal failure as appropriate.
8. Watcher creation failure is observation loss, not permission to release worker authority. Hydration/reconnect remains the recovery mechanism.
9. Direct press feedback remains immediate and presentation-value spring driven; pointer cancellation fences a later activation.
10. Recursive source validation includes a meta-regression against malformed duplicated Rust declarations so token-presence checks cannot masquerade as compilation.
11. The production IPC client authenticates the opened pipe object before protocol exchange by verifying service-specific SID ownership and a running `AetherCoreMaintenance` own-process service. It does not use `GetNamedPipeServerProcessId` from the client handle because that is outside the documented `CreateNamedPipe` handle contract. Debug development uses a per-launch 128-bit rendezvous suffix instead of weakening production endpoint verification.
12. Service startup claims the fixed namespace with `FILE_FLAG_FIRST_PIPE_INSTANCE`. Client retries are limited to `ERROR_FILE_NOT_FOUND` and `ERROR_PIPE_BUSY`; every other open error is terminal, and an opened endpoint that fails trust verification is terminal.
13. Authenticated Users receive only exact named-pipe client rights `0x00120003` (`READ_DATA | WRITE_DATA | READ_CONTROL | SYNCHRONIZE`). Generic write is forbidden because its append/create-instance bit can authorize additional server instances.
14. Production named-pipe clients set `SECURITY_IDENTIFICATION` SQOS so the service can identify the user for authorization without receiving resource-impersonation/delegation authority.
15. Native IPC release evidence must match the intended DACL, not merely contain minimum rights: AU allow rights are exact, the DACL is protected, exactly one AU allow and one trusted-service allow are permitted, every deny ACE is rejected, callback/object/opaque/flagged ACE shapes fail closed, unexpected allow trustees are rejected, and generic write/all masks are treated as create-instance authority.
16. Listener ownership is independent of connected-session ownership. After every successful accept, the service creates the successor listener before session admission/handoff; after an accept failure it reclaims with FIRST_INSTANCE. Successor-listener creation failure is a terminal fail-closed service error.
17. Pipe-open retry classification is fail-closed: only documented absence/busy namespace states are retried; authorization/configuration failures are surfaced immediately.
18. Machine-wide mutation arbitration uses an installer-provisioned protected ProgramData lock file and fail-immediate `LockFileEx`, not a predictable Global named mutex. Runtime opens the authority with `OPEN_EXISTING` and never creates it.
19. Production pipe ownership and server-instance authority are scoped to `NT SERVICE\AetherCoreMaintenance`. The service SID is the sole server-authority ACE; AU retains exact client rights and Builtin Administrators receive no pipe allow ACE. Service-SID resolution uses a bounded aligned buffer, validates the returned length and `IsValidSid`, and caches only successful resolution.
20. The privileged maintenance executor uses `SERVICE_SID_TYPE_UNRESTRICTED`, not `RESTRICTED`. The service SID remains an ACL identity, but the executor must mutate heterogeneous Windows/third-party resources that cannot safely be pre-ACL'd for the product SID. Native qualification reads the live service process token and requires the service SID to be access-check-enabled, enabled-by-default/owner-capable, with no restricting SID list. Stronger future isolation should split a restricted broker from the privileged mutation executor instead of write-restricting the executor itself.

## Consequences

- Consent cannot display immutable summary fields that disagree with the frozen action material without an integrity failure.
- Historical principal caches are bounded without deleting active subscription or update-authorization state.
- Session churn cannot amplify accepted request workers beyond the global process budget.
- Mutation single-flight and read-budget guarantees now follow the work that needs them, not an observer polling that work.
- OS thread exhaustion degrades availability in a typed fail-closed way rather than silently releasing authority or panicking after authorization.
- SHA-256 stored beside immutable material remains corruption/consent binding, not a MAC against a privileged administrator.
- Synchronous named-pipe teardown now requests cross-thread `CancelSynchronousIo` in both directions and fences post-shutdown client writes. This mitigation is source-complete but not runtime-proven here; Windows slow-peer/cancellation/handle-lifetime evidence decides whether synchronous I/O remains acceptable or must be replaced by overlapped I/O.
- Production IPC no longer depends on a client-side server-PID query with an unsupported handle contract; it combines service-SID ownership, service-state verification and first-instance namespace ownership before protocol exchange.
- AU cannot create new server instances through generic-write aliasing, while legitimate non-elevated clients retain only the data/control-read/synchronization rights required for endpoint verification and transport.
- Client SQOS is explicit and least-privilege: identity inspection remains available to the service, resource impersonation does not.
- `verify-maintenance-service-token.ps1` is mandatory installed-service evidence that the executor has the intended unrestricted service-SID token shape; `verify-ipc-pipe-security.ps1` separately proves live owner/DACL/service/SQOS invariants. Neither native probe is claimed executed in this environment.
- Native IPC evidence is exact-policy evidence: wrong service SID type/owner, missing service create-instance authority, Administrator server authority, widened AU rights, AU deny drift, unprotected DACL inheritance, unexpected allow trustees, or unrecognized ACE encodings fail qualification.

- Named-pipe namespace ownership now remains continuous across normal session churn; accepted-session teardown cannot intentionally precede creation of the successor listener. Native rapid-disconnect/squatter-race evidence remains required.
