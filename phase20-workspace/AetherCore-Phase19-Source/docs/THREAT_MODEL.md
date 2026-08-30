# AetherCore Threat Model — Zenith Recursive Baseline

## Trust boundaries

**Low trust:** Svelte/WebView UI, non-elevated desktop host, other local named-pipe callers, display strings/metadata returned by Windows APIs, user-controlled files inside approved cleanup roots, Event Log payload text, storage-reported metadata, and minidump files on disk.

**Privileged:** `AetherCoreMaintenance` running as LocalSystem in installed mode with an unrestricted service-specific SID, or elevated console mode during development.

**Consent boundary:** the one-shot elevated consent broker. Every named-pipe connection is bound to a service-derived Windows principal context (user SID + logon AuthenticationId + Windows session ID). The service obtains SID/AuthenticationId/elevation by impersonating the exact connected pipe client and reading the impersonated thread token, then calls `RevertToSelf` before request dispatch; PID is not an ownership authority. The non-elevated shell can request a short-lived, non-secret consent intent for one owned immutable plan. The elevated broker receives only that intent ID; the service validates broker elevation, exact installed image path, and matching logon principal before showing the service-sourced plan summary and accepting one approval.

**Platform authorities:** PnP/Configuration Manager, Windows Update Agent, System Restore, fixed Windows servicing executables, NTFS handle semantics, SCM/Task Scheduler/registry APIs, Windows Storage WMI, storage IOCTLs, Windows Event Log, and OS memory-status APIs.

## Mutation threats retained from earlier phases

### UI compromise becomes arbitrary LocalSystem execution

Control: privileged operations are typed Protobuf requests. Mutation endpoints accept service-issued IDs/bounded decisions, never arbitrary commands, registry paths, service names, filesystem roots, driver packages, or caller-selected executables.

### Forged/replayed/cross-user UAC consent

Control: Phase 9 removes command-line challenges and reusable grants. A service-minted consent intent is short-lived, bound to immutable plan ID + SHA-256 digest + owner principal, can be approved exactly once by the validated elevated broker, and is atomically consumed in the same SQLite transaction that advances the owned plan from `AwaitingAuthorization` to `Preflight`. An approved or consumed intent cannot be re-presented or re-approved.

### Driver/repair/cleanup/startup TOCTOU

Control: each subsystem freezes service-side evidence and revalidates native state at the last responsible moment. Drift fails closed or becomes a skip/recovery record instead of silently changing a different target.

### Servicing/mutation collision

Control: Driver Install, System Repair, Cleanup, and Startup/Service mutation share a machine-wide mutex. Repair additionally checks Windows Update busy/pending-reboot state before crossing its mutation barrier.

### Broad cleanup, shell execution, or unsafe service control

Control: fixed command arrays and allowlisted cleanup providers only. No shell endpoints. Phase 5 never stops managed services or sets `SERVICE_DISABLED`; protected targets fail closed.

### Passive-default violation

Control: `Unreviewed`/`KeepEnabled` cannot produce plan actions. Restore is an explicit, newly authorized plan. Restart recovery never replays a startup/service mutation.

## Phase 6 threats and controls

### Synthetic health score creates false confidence or fear

Threat: sparse or vendor-dependent SMART/WMI evidence is collapsed into a “97% healthy” score that appears more authoritative than the underlying data.

Control: there is no health-score field in Phase 6. Metrics are individually optional and source-attributed. Missing values remain missing. Classification escalates only explicit Windows/device-reported warning/error evidence.

### Missing telemetry is interpreted as zero or healthy

Threat: unsupported SMART/reliability counters are deserialized as zero, making an unsupported device appear perfect.

Control: Rust uses `Option<T>` and Protobuf uses explicit presence booleans for optional numeric metrics. UI renders `Not reported`. A device with insufficient standardized evidence may remain `Unknown` even if no reported counter is bad.

### Vendor SMART semantics are over-generalized

Threat: AetherCore applies undocumented/raw ATA vendor attribute thresholds universally and produces false diagnoses.

Control: Phase 6 uses standardized Windows reliability counters plus defined NVMe SMART/Health fields for automated severity. It may read the ATA SMART attribute table through the documented read-only path, but preserves only ID/current/worst/raw evidence and never assigns universal vendor-specific meanings, units, or thresholds to raw ATA values. It also does not invent universal temperature thresholds.

### Malformed/hostile storage IOCTL output causes memory unsafety

Threat: a device/driver returns a short or malicious protocol-data descriptor/offset.

Control: direct NVMe parsing uses bounded aligned buffers, validates returned descriptor length/version/size, checks protocol data length, validates computed offset against actual bytes returned, and only then performs an unaligned struct read.

### WHEA event text is treated as deterministic failed-component identification

Threat: keyword-based event summarization identifies a specific DIMM/CPU/device without enough evidence.

Control: WHEA cards are explicitly `LoggedHardwareEvidence`/root-cause-unknown. Memory-related evidence does not name a DIMM. Generic WHEA evidence is retained without forced component attribution.

### Kernel-Power Event 41 is blamed on PSU/driver/hardware

Control: Event 41 is modeled only as proof of an unclean shutdown/restart (`EventHigh/CauseLow`). Guidance asks the user to inspect power, thermal, WHEA and dump evidence rather than naming a cause.

### Minidump metadata is used to blame a driver

Threat: the most visible filename/module or a lightweight header is promoted to a culprit.

Control: Phase 6 parses only bounded metadata and the official `DUMP_HEADER64`/`DUMP_HEADER32` bugcheck fields. Dump filename/header never produces driver attribution. UI states that module-level attribution may require matching binaries and symbols.

### Incorrect dump-layout parsing

Control: recognized kernel dump signatures are mapped to the official Windows `DUMP_HEADER64`/`DUMP_HEADER32` structures rather than handwritten offsets. Other layouts remain metadata-only.

### Event/minidump data leaks through UI/IPC

Threat: raw Event Log XML or arbitrary dump paths expose more local data than necessary.

Control: the UI receives structured evidence only. Raw XML remains collector-local. Minidumps are exposed by file name/size/timestamp and bounded header metadata, not arbitrary caller-selected paths or file contents.

### Diagnostic scan performs mutation through a “guided action”

Control: Phase 6 contracts contain scan/snapshot/history only. Guided actions are text. The Phase 6 crates contain no file deletion, service configuration, update installation, restore-point or servicing mutation surface.

### Collector failure is hidden as healthy

Control: hardware and crash collectors fail independently. One failure yields `Partial` plus a warning while preserving successful evidence; failure of both yields `Failed`. Missing evidence is not converted into a healthy card.

### Crash/WHEA temporal proximity is mistaken for causation

Control: correlation within the diagnostic window is phrased explicitly as temporal correlation, not proof of causation.

### Diagnostic history grows without bound

Control: persistence retains only the newest 50 full diagnostic snapshots. API history requests are additionally bounded.

## Explicitly excluded dangerous/deceptive behavior

Through Phase 9 AetherCore does not implement shell-command endpoints, arbitrary INF installation, display/firmware one-click installation, registry cleaning, broad filesystem heuristics, WinSxS/Installer/Driver Store/Prefetch deletion, whole-Recycle-Bin emptying, CHKDSK repair switches, service stop/delete/`SERVICE_DISABLED`, automatic startup changes, forced restart, mutation replay after ambiguous interruption, synthetic hardware-health percentages, arbitrary vendor SMART scoring, automatic “RAM healthy” claims, Event 41 root-cause claims, or driver blame from lightweight dump metadata.

## Residual risks before GA

- Windows Storage WMI availability and reliability-counter coverage vary across controllers, OEM drivers, RAID layers, SATA/NVMe bridges, virtual disks, and enterprise storage stacks.
- Direct NVMe protocol queries require validation across a broad controller/firmware matrix and against malformed/partial driver responses.
- WHEA event wording/payload differs by hardware and Windows versions; Phase 6 intentionally keeps classification conservative.
- Lightweight minidump parsing does not replace debugger/symbol-backed analysis and should be tested against a curated crash corpus before expanding supported layouts.
- Event Log access/retention policies may limit evidence windows.
- Signed packaging, SBOM, IPC fuzzing, storage-driver fuzz/fault injection, large OEM hardware matrix, physical memory-fault test systems, crash corpus review, and external security assessment remain production gates.


### Cross-user confused deputy

Control: the service never accepts an owner SID/key from web content or Protobuf request fields. It derives the connected client's principal from the named pipe/client token, computes an internal binding key, and scopes plan reads, snapshots, diagnostic history, execution status, startup history, creation, authorization, and mutation start to that owner. Driver, repair, cleanup, and startup in-memory snapshots also carry owner state, so a second interactive user cannot reuse another user's prior discovery evidence.

### Cleanup path replacement / TOCTOU

Control: cleanup scan evidence freezes the final root plus the target volume serial and 128-bit file identifier obtained from an opened handle. Before deletion, AetherCore reopens without following reparses, revalidates the final path, size/time evidence and exact volume/file ID, and deletes through the validated handle. A same-path replacement is rejected.

## Phase 10 — Persistent-session and Operation Kernel threats

### Cross-user event sequence leakage

Threat: a global stream sequence lets one authenticated local user infer that another user's session is active even when payloads are filtered.

Control: Event Bus state and monotonic sequence numbers are partitioned by the Phase 9 `owner_principal_key`. Replay, current sequence, replay floor, subscribers and hydration are all principal-scoped.

### Replay/live race and silent event loss

Threat: an event emitted between replay capture and live-subscription installation can disappear, or a slow renderer can overflow a queue without noticing.

Control: replay capture and subscriber installation occur under one Event Bus lock. Subscriber queues are bounded. Overflow marks the subscriber lagged; the server emits an explicit `StreamReset` and republishes a complete typed current-state image through the same ordered stream. A cursor older than retained replay or newer than the current service epoch is also reset explicitly.

### Persistent-session resource exhaustion

Threat: a local client holds many named-pipe sessions or concurrently issues enough requests to exhaust service threads/resources.

Control: the service caps authenticated sessions at 32 globally and 4 per kernel-observed user SID, so one local account cannot monopolize every persistent slot. Active requests are capped at 8 per session. Client/control frames are capped at 384 KiB while only server responses/events retain the 8 MiB ceiling required for large inventories. Deadlines are capped; cancellation IDs and request IDs are bounded; duplicate active request/cancellation identifiers are rejected. Read-only expensive work uses a separate global budget and single-flight-per-kind policy.

### Request correlation/cancellation rebinding

Threat: duplicate IDs cause one request to receive another response or allow a cancellation token to be rebound.

Control: the desktop rejects duplicate in-flight request IDs, the service rejects duplicate active request IDs per session, and `CancellationRegistry::try_register` rejects a duplicate scoped cancellation ID. Cancellation keys are prefixed by the server-generated session ID.

### Stale reconnect invalidation race

Threat: a request running on a dead/obsolete desktop session fails after another thread has already installed a replacement session, then clears the shared session slot and tears down the healthy replacement.

Control: the Tauri bridge invalidates a failed session only when `Arc::ptr_eq` proves the failed object is still the session stored in the shared slot. Connection creation remains serialized and rechecked under `CONNECT_LOCK`.

### Client disconnect during privileged work

Threat: treating UI disconnect as authority to terminate an already-started system mutation can interrupt Windows servicing at an ambiguous unsafe point.

Control: disconnect signals only request-scoped cancellation tokens. Once a durable mutation has crossed its start/safety boundary, ownership transfers to the service/kernel mutation lease and continues to a durable terminal/reboot boundary. The UI can reconnect and hydrate/replay state.

### Concurrent machine mutations

Threat: Driver Install, DISM/SFC repair, cleanup, startup mutation or future application update run concurrently and invalidate each other's assumptions or recovery evidence.

Control: all mutation start routes acquire one shared `MutationSupervisor` RAII lease. The workload/plan/owner is observable in typed lease events, and release occurs only at a durable terminal/reboot boundary. Update is reserved in the supervisor now even though its engine arrives later.

### Telemetry durability amplification

Threat: frame-rate progress updates force `synchronous=FULL` SQLite writes, creating avoidable I/O latency and pressure that can degrade servicing or UI responsiveness.

Control: `ProgressTelemetryStore` is the high-frequency in-memory plane and emits live principal-scoped events. SQLite remains the safety ledger for state transitions, mutation evidence, recovery and coarse checkpoints. The migration intentionally creates no live telemetry table.

## Zenith UI agency / accidental activation

Threat: pointer capture can keep a button as the event target after a user drags outside the control to cancel, causing visible disarm and native activation to disagree.

Control: `fluidPress` tracks an armed hysteresis state and installs a one-shot capture-phase activation fence when pointer-up occurs while disarmed. Keyboard activation is explicitly left enabled. This is UI defense-in-depth only; it does not replace service-side authorization or consent validation.

Zenith makes no change to privileged trust boundaries, cryptographic keys, update authority, support-bundle disclosure, IPC principal derivation, or mutation ownership.


## Zenith Recursive hardening

### Persisted plan summary diverges from immutable execution material

Threat: SQLite contains duplicated index/presentation columns (`id`, `title`, `risk`, `created_unix_ms`, owner) alongside the hashed immutable JSON. Corruption that changes only the duplicated columns can make the consent surface describe a different risk/title than the actions whose JSON digest is being approved.

Control: `OperationEngine::material_from` now treats `immutable_json` as the authoritative consent/execution material, verifies its SHA-256 digest, and then requires every duplicated immutable column to exactly match the parsed material. `updated_unix_ms` must not predate creation. Any mismatch is `IntegrityMismatch`; plan display, consent creation, and execution all fail closed. `PlanView` is populated from the verified material instead of the duplicate presentation columns.

Trust limit: SHA-256 stored beside the material is corruption detection and consent binding, not a secret MAC against an already-privileged local administrator who can rewrite both database and process state. Windows installation ACLs, service isolation, code signing, and the OS privilege boundary remain the protection against untrusted writers.

### Principal replay registry grows across long-running logon churn

Threat: each distinct owner binding key can allocate replay history for the service lifetime. Repeated Windows logon identities could accumulate stale streams even after all subscribers disappear.

Control: inactive owner streams are retained in a bounded LRU-style registry (`MAX_OWNER_STREAMS=128`). Streams with live subscribers are never eviction candidates. Returning owners whose replay state was evicted enter the existing explicit `StreamReset` + typed hydration path; continuity is never fabricated.

### Update state persists across historical principal churn

Threat: each distinct owner binding can retain an in-memory update snapshot and release map for the process lifetime even after that Windows logon disappears.

Control: `UpdateCoordinator` bounds historical owner-state retention to 128 entries by retiring the oldest quiescent cache entry before admitting a new owner. States representing staging, a staged artifact, pending consent, or active installation are never eviction candidates. If every retained state is mutation-critical, correctness takes precedence and the cap is allowed to soften temporarily rather than discard live authority or recovery context.

### Session churn accumulates stuck native request workers

Threat: the per-session limit of eight requests does not bound detached request workers across disconnect/reconnect cycles if a native Windows call does not reach a cooperative cancellation checkpoint.

Control: request admission also acquires a process-global RAII worker slot (`MAX_GLOBAL_INFLIGHT_REQUESTS=64`). Exhaustion returns typed `Busy`/503 and starts no worker. The slot is owned by the worker thread and is released only when that worker exits, so a truly stuck call consumes capacity instead of enabling unbounded thread growth.

### Synchronous named-pipe writer teardown

Control: outbound producers are byte/frame bounded and fail closed. The transport registers least-rights thread handles for blocking reader/writer pumps and requests `CancelSynchronousIo` across directions on backpressure, disconnect, and teardown; the client pump fences new queued writes after shutdown. Residual: Windows documents cancellation as a request rather than a wait-for-completion guarantee, so slow-peer/service-stop/repeated-cancel timing and handle/thread retirement remain native qualification. Overlapped I/O is required only if that evidence cannot prove acceptable deterministic teardown.

### Mutation lease released by observer failure

Threat: a machine mutation starts successfully, but its progress-watcher thread cannot be created. If the `MutationSupervisor` lease is owned by that watcher, thread creation failure drops the guard while the real mutation continues, permitting an overlapping machine mutation.

Control: driver install, system repair, cleanup, and startup mutation coordinators accept an exact `MutationLease` handoff and move that lease into the actual execution worker. `MutationLease::matches` binds workload, plan ID, and principal key. Streaming watcher signatures contain no mutation lease. Failure of observer creation can reduce telemetry but cannot release mutation authority.

### Expensive read budget released by observer failure

Threat: WUA/WMI/PnP/filesystem/diagnostic work starts, then its progress watcher fails and drops the only read-budget guard. A new expensive request can then be admitted while the first worker is still consuming resources.

Control: driver discovery, repair assessment, cleanup discovery, startup discovery, and diagnostics use leased start APIs. The exact `ReadBudgetLease` is moved into the actual scan worker and remains held until that worker exits. Progress watchers own no read budget.

### Thread creation failure after work admission

Threat: OS thread exhaustion occurs after counters, cancellation IDs, process state, or child-process pipes have been prepared. Panic-based `thread::spawn` semantics can terminate the request path or leave state inconsistent.

Control: critical runtime thread creation uses named `thread::Builder` calls with explicit failure handling. Service session admission restores its global counter; request-worker failure removes the pre-registered cancellation ID and returns typed Busy/503; scan workers mark their scan failed if execution never started; IPC client reader creation propagates `IpcError::Io`; repair stdout/stderr reader failure kills/waits the child; the desktop reconnect helper fails setup explicitly. Streaming watchers use a non-panicking helper because they are observers, not authority holders.

### Watcher owns mutation/read authority

Threat: a status watcher holds the machine-wide `MutationLease` or expensive-read `ReadBudgetLease` while a separate worker performs the protected operation. Watcher creation, panic, early terminal observation, scheduler delay, or polling failure can then make authority lifetime diverge from work lifetime.

Control: production routes pass exact-identity leases into the actual workers. `MutationLease::matches` binds workload + plan + owner; `ReadBudgetLease::matches` binds workload. Driver install, system repair, cleanup and startup mutation workers retain their mutation lease until worker exit. Driver discovery, repair assessment, cleanup discovery, startup discovery and diagnostics retain their read budget in the collector worker. Stream watchers own no lease.

### Runtime thread creation fails after authority/state acquisition

Threat: the OS refuses a thread after a session slot, cancellation token, one-shot consent, execution journal, child process or active-operation flag has already been acquired.

Control: affected runtime paths use named fallible `thread::Builder`. Failure restores session accounting, removes request cancellation registrations, returns typed request/service errors, records mutation failure, clears telemetry/running state, or kills/waits the repair child and joins surviving pipe readers as appropriate. Stream watcher creation failure is logged as observation loss; it cannot release worker-owned authority and current state remains recoverable through hydration.

### Fixed-name pipe endpoint is squatted before service startup

Threat: a standard-user process creates `\\.\pipe\AetherCore.Maintenance.v7` before the LocalSystem service, or a client treats an opened hostile endpoint as a transient failure and later retries into a different instance.

Control: the service's startup listener uses `FILE_FLAG_FIRST_PIPE_INSTANCE`, making a pre-existing namespace object a startup failure rather than a silent later instance. Listener lifetime is separate from session lifetime: after an accept, the service creates the successor listener synchronously before session admission or worker handoff, so a normal disconnect cannot create a zero-instance namespace gap. If accept fails and consumes the listener, recovery reclaims with FIRST_INSTANCE rather than joining an unknown namespace. A production client verifies the connected pipe object's owner SID is the service-specific `NT SERVICE\AetherCoreMaintenance` SID and requires the registered `AetherCoreMaintenance` service to be running as an own-process service before sending `ClientHello`. Only `ERROR_FILE_NOT_FOUND` and `ERROR_PIPE_BUSY` are treated as transient open states; access-denied, invalid-access, and all other open failures are terminal. Once an endpoint opens, failed identity verification is also terminal. Debug console mode uses a strict fresh 128-bit suffix and is compiled out of release behavior.

### Authenticated user creates an additional trusted-name server instance

Threat: named-pipe `FILE_GENERIC_WRITE` contains the access bit also used as `FILE_CREATE_PIPE_INSTANCE`. Granting generic write to Authenticated Users lets a standard user request another server instance of the existing pipe object.

Control: the production pipe DACL grants AU only `FILE_READ_DATA | FILE_WRITE_DATA | READ_CONTROL | SYNCHRONIZE` (`0x00120003`) and never grants generic write. The client opens with the same specific mask. The production DACL grants `GENERIC_ALL` server authority only to the service-specific SID; this preserves service-scoped endpoint authority without granting server authority to Builtin Administrators. The service SID type is intentionally `UNRESTRICTED` because the same LocalSystem process must perform broad, typed maintenance mutations against Windows resources with heterogeneous ACLs. The native verification script requires service-SID ownership/create-instance authority and rejects create-instance on AU or any other allow trustee.

### Pipe server impersonates the non-elevated desktop

Threat: default named-pipe SQOS can permit a server to impersonate the connected desktop user more strongly than required for AetherCore's authorization model.

Control: production client opens specify `SECURITY_IDENTIFICATION`. The service can inspect the client's token identity/groups for its principal binding, but the pipe transport does not grant resource impersonation/delegation authority. The native verification probe opens with the same SQOS.

Trust limit: endpoint authentication is designed to exclude standard-user namespace squatters and configuration drift. A compromised LocalSystem context or administrator capable of rewriting service configuration/security descriptors is already inside the privileged OS trust boundary and is not converted into an unprivileged threat by this check.

### Write-restricted token conflicts with the privileged maintenance executor

Threat: applying `SERVICE_SID_TYPE_RESTRICTED` to the LocalSystem executor is treated as a universal hardening win even though AetherCore must write to registry, service, update, repair, driver and approved filesystem objects whose DACLs are outside product control. Legitimate typed mutations can then fail solely because the product SID is absent from an external resource ACL.

Control: the installed executor uses `SERVICE_SID_TYPE_UNRESTRICTED`. Its service SID remains enabled and owner-capable and is used for AetherCore-owned ACLs and named-pipe ownership; a native process-token probe requires zero restricting SIDs. The design does **not** call this a sandbox. The high-privilege boundary is constrained by typed Protobuf operations, service-derived principal binding, immutable one-shot consent, global mutation arbitration, reparse/path validation and durable audit/recovery state.

Future direction: if stronger OS-token isolation is required, split a restricted coordination/read broker from narrowly typed privileged mutation executors and qualify each operation, rather than write-restricting the current all-purpose mutation process.

### Structural audit tokens survive malformed source

Threat: a string/token-based audit continues to find every expected invariant even though patch corruption has made a Rust declaration syntactically invalid.

Control: the recursive gate includes a meta-regression rejecting repeated public-function declarations and pins the expected diagnostic refresh signature count. This is defense-in-depth only; Rust compilation/clippy on the qualified toolchain remains the authoritative syntax/type/lint gate.
