# AetherCore Zenith Recursive Final Adversarial Review

## Attack the consent boundary

**Attack:** change only duplicated SQLite `title`, `risk`, owner, ID or creation time while preserving `immutable_json` and its digest.

**Result:** the material digest is verified and every duplicated immutable field must equal parsed material. Display, consent and execution fail with `IntegrityMismatch` before a misleading plan view can be produced.

**Remaining assumption:** an already-privileged local administrator who can rewrite both material and digest is outside this hash's cryptographic guarantee. OS ACL/service isolation/code signing remain that boundary.

## Attack mutation single-flight

**Attack:** cause a status watcher to fail or terminate while a driver install/repair/cleanup/startup mutation continues.

**Before:** watcher lifetime held `MutationLease`, so observation and authority were coupled.

**Result after fix:** the exact workload/plan/owner lease is moved into the actual worker closure. Watchers receive no lease. Watcher failure cannot release mutation authority while the worker is alive.

**Attack:** hand a lease for one workload/plan/owner to another worker.

**Result:** `MutationLease::matches` rejects the mismatched identity. A regression covers workload, plan and owner mismatch.

## Attack expensive read budgeting

**Attack:** kill a scan watcher while WUA/WMI/PnP/filesystem diagnostic work continues.

**Result after fix:** the actual collector owns `ReadBudgetLease`; the watcher owns none. Production routes use leased start methods for all five expensive read domains.

## Attack thread creation

**Attack:** exhaust OS thread resources after one-shot consent is consumed or after a request cancellation token is registered.

**Result after fix:** worker creation is fallible. Mutation coordinators perform durable failure cleanup and clear runtime state; request-worker failure removes the cancellation registration and returns typed Busy/503; session admission restores its counter when session-thread creation fails.

**Attack:** fail a stream watcher after backend work already starts.

**Result:** the watcher failure is logged. It does not release a mutation/read lease, and reconnect hydration can reconstruct current typed state.

**Attack:** fail one repair stdout/stderr reader or hit the command timeout.

**Result:** child termination/wait is explicit and any successfully created reader is joined before returning the error.

## Attack principal retention

**Attack:** rotate through more than 128 Windows logon principals and connect/disconnect each.

**Result:** EventBus evicts only the oldest inactive owner streams. Live subscribers are never eviction candidates. Returning old cursors receive explicit replay reset/hydration.

**Attack:** rotate principals through update checks/states.

**Result:** oldest quiescent update state is retired at the target cap. Staging, staged, consent-pending and installing owners are never selected. If every entry is critical, correctness wins and the cap softens rather than deleting authority/recovery state.

## Attack request admission

**Attack:** repeatedly open sessions, start long native calls, disconnect and reconnect.

**Result:** every worker owns one of 64 process-global slots in addition to the 8-per-session cap. Once exhausted, the service refuses new request work. A permanently blocked worker reduces capacity by one but cannot be multiplied into unbounded accepted workers.

## Attack the coordinator API surface

**Attack:** bypass the maintenance-service router and call a legacy public mutation/read `start*` method directly from a new internal consumer.

**Before final closure:** production routes were leased, but public unleased compatibility entry points still existed.

**Result after fix:** all public unleased mutation and expensive-read entry points are removed. Private coordinator boundaries require concrete, non-optional leases, and tests exercise the leased path rather than a privileged shortcut. The recursive gate rejects public unleased starts and optional authority guards.

## Attack the named-pipe endpoint

**Attack:** create the fixed production pipe before the service and wait for the desktop to send its first protocol bytes.

**Result:** the service's startup listener uses `FILE_FLAG_FIRST_PIPE_INSTANCE`, so it will not silently become a later instance of an attacker-created object. After each accepted connection, it creates the successor listener before session admission/handoff, preserving namespace ownership across normal churn; failed accepts reclaim only with FIRST_INSTANCE. Independently, the production client authenticates the opened kernel object's owner as the exact `NT SERVICE\AetherCoreMaintenance` SID and requires `AetherCoreMaintenance` to be a running own-process service before `ClientHello`; an opened-but-untrusted endpoint is terminal, not retried.

**Attack:** grant Authenticated Users generic write and create a second server instance after the legitimate service owns the pipe.

**Result:** generic write is no longer granted. AU receives only `0x00120003`, which includes data read/write, DACL/owner read and synchronization but excludes bit `0x4` (`FILE_CREATE_PIPE_INSTANCE`). The client requests those same specific rights. The installed-service probe rejects any non-service allow ACE carrying create-instance, including generic-write/all aliases, and rejects any Builtin Administrators allow ACE.

**Attack:** stand up a malicious pipe server and use the desktop client's token to access local resources.

**Result:** the client open explicitly requests `SECURITY_IDENTIFICATION`; the server can inspect identity for authorization but cannot use the client's security context for resource access. The production client intentionally does not rely on `GetNamedPipeServerProcessId` from a `CreateFile` handle because that is not the documented API contract.

**Residual:** kernel owner/DACL and first-instance behavior must be captured on qualified Windows. A process already executing as LocalSystem or an administrator able to alter service/pipe security is outside this standard-user squatting boundary and remains governed by OS/service/code-signing trust.

**Attack:** widen the installed AU ACL with an unrelated extra allow right, add an AU deny ACE, remove DACL protection, add an unexpected allow trustee, or inject an ACE type the parser does not recognize while preserving the originally checked required bits.

**Observed during self-attack:** the first native probe would still have accepted some such policy supersets.

**Result after fix:** the evidence probe requires exact AU allow rights, zero AU deny, a protected DACL, exactly one AU allow plus one trusted-service allow, and the expected allow-trustee set only. It rejects every deny ACE, callback/object/opaque/flagged ACE shapes, and any generic-right create-instance authority outside the service SID. The release artifact can no longer turn a widened or semantically different ACL into a green result simply because required bits are present. The runtime also resolves the service SID through bounded, aligned storage, checks the returned byte count, validates it with `IsValidSid`, and caches only successful resolution.

**Attack:** harden the LocalSystem maintenance executor with `SERVICE_SID_TYPE_RESTRICTED` and assume that is always safer.

**Result:** rejected as incompatible with the approved mutation mission. The service must write to heterogeneous Windows and third-party objects that cannot safely be re-ACL'd for AetherCore. Installation uses `SERVICE_SID_TYPE_UNRESTRICTED`; a live token probe requires the service SID to remain enabled and owner-capable while the restricted-SID list stays empty. Isolation is enforced at typed IPC, principal/consent, mutation-supervisor and product-resource ACL boundaries rather than by silently breaking valid maintenance writes.

## Attack the interaction layer

**Attack:** pointer-down, drag/cancel/lost capture, then synthesize click.

**Result:** press feedback begins on pointer-down; pointer cancel/lost capture arms a one-shot click fence; direct transform motion remains spring/presentation-value driven and the fixed press-state transition is removed.

## Attack the audit itself

**Attack:** create malformed Rust that still contains every expected string token.

**Observed during this pass:** a duplicated `pub fn passive_hardware_refresh` declaration demonstrated this class of false structural assurance.

**Correction:** fix the source and add a meta-regression rejecting repeated public-function declarations, duplicate top-level method names within the same `impl` (including private methods), plus an exact diagnostic signature cardinality check. Structural PASS is still subordinate to the real Rust compiler on Windows CI.

## Corruption / hostile input / million-operation questions

- Plan duplicate/material divergence is fail-closed before consent.
- Replay memory is bounded by retained owners × replay capacity rather than all historical principals.
- Update metadata is bounded for quiescent owners without erasing mutation-critical state.
- Accepted request workers are globally bounded across session churn.
- Machine mutation and expensive-read leases follow work lifetime, not observer lifetime.
- Thread creation failure has explicit cleanup after partial state acquisition.
- Event sequence saturation at `u64::MAX` remains a theoretical horizon; saturation would stop strict monotonic growth and should be treated as epoch-reset territory rather than wraparound.
- SQLite durable history remains governed by existing domain pruning/recovery policy; this pass does not invent destructive journal truncation without recovery evidence.

## Attack synchronous pipe teardown

**Attack:** let a peer stop reading until the dedicated writer is blocked, then trigger backpressure, disconnect, or client/service teardown.

**Correction:** each blocking IPC direction now has a registered real thread handle opened with the access right required by `CancelSynchronousIo`. Teardown and backpressure request cancellation, reader/writer directions cross-cancel, the server reader is bound to its actual session worker, and the client writer refuses to start additional queued writes after shutdown. The cancellation registry lock is released before crossing the Win32 API boundary.

**Residual:** this is deliberately not labeled deterministic from source inspection. `CancelSynchronousIo` requests cancellation and does not wait for completion; a narrow timing race can exist around an operation that begins immediately as cancellation is requested. Native slow-peer/service-stop/disconnect/repeated-cancel testing must decide whether this synchronous design is sufficient or whether an overlapped-I/O migration is justified.

## Attack pipe-open retry classification

**Attack:** make the trusted-name pipe open fail with an authorization/configuration error rather than namespace absence or normal busy state, and observe whether the desktop repeatedly treats it as benign startup churn.

**Correction:** the retry classifier accepts only `ERROR_FILE_NOT_FOUND` and `ERROR_PIPE_BUSY`. Every other open failure is terminal immediately, while an opened endpoint that fails owner/service authentication remains `UntrustedPeer` and is never retried. This prevents access-policy drift from being normalized into a generic retry loop.

## Final decision

No confirmed source finding from this recursive pass is silently left as PASS. ZR-001 through ZR-019 plus ZR-G01/ZR-G02 are fixed and structurally regression-gated. ZR-R01 through ZR-R03 remain explicit qualification/release blockers. The transformed source is ready for qualified Windows compilation and adversarial native testing; it is not declared signed GA from this environment.
