# AetherCore — Security review of maintenance-service un-gating

## Decision

**VERDICT: NOT SAFE**

Phase 2 is **not entered**. The newly reachable Windows RPC surface contains a
LocalSystem information-disclosure path: `RunSecurityAudit` accepts caller-named
absolute paths, and the router does not enforce the owner-scoped allowlist promised
by the wire contract. No source fix was made in this review.

This is a static, repository-grounded review. The exploit path was traced through
the Windows server and providers; no Windows VM probe was needed to establish the
missing authorization check.

## Scope and evidence

- Required context: `phase21-workspace/docs/phase36/SESSION_CONTEXT.md` (read in full).
- Baseline: `main`, clean and synchronized at `176d4ff` before this review.
- The historical Unix-only split is in `0e30038859b386b49d5f6b38b0b6cbdb4f98bed7`.
- The actual re-inclusion/un-gating is `39ccab04a0df57721d631fb9d6fe7c9da5b37f3c`
  (2026-08-30, “Phase 36 tranche 1 — Windows ARM64 native qualification (in progress)”).
- Live source is authoritative; historical binary-safe copies are not used as evidence.

## UNGATING_COMMITS and mechanism

`0e30038` changed the maintenance-service module declarations to Unix-only as part
of the P31 composition split. In `39ccab0`, the following declarations changed from
`#[cfg(unix)]` to `#[cfg(any(unix, windows))]` in
`services/maintenance-service/src/main.rs`:

`errors`, `performance`, `protocol`, `router`, `streaming`, and `support`.

The same commit removed the Unix gate from `protocol::snapshot` and from
`router::{require_broker, expected_broker_path, require_update_broker,
expected_update_broker_path}`. It also moved the Windows `BOOL` import; that import
fix is unrelated to attack-surface scope.

The commit comments describe these modules as platform-neutral and say that the
Windows composition references the same crate-root modules. Consequently, the
Windows service binary now compiles and dispatches the router and all of its
platform-neutral domains over the named pipe.

## MODULES_BEFORE / MODULES_AFTER

| Module | Before `39ccab0` on Windows | After `39ccab0` on Windows |
|---|---|---|
| `errors` | excluded | compiled |
| `performance` | excluded | compiled |
| `protocol` | excluded | compiled |
| `router` | excluded | compiled |
| `streaming` | excluded | compiled |
| `support` | excluded | compiled |
| `composition`, `scheduler`, `server` | compiled | compiled |
| `care`, `intelligence`, `timeline` | compiled (ungated) | compiled |
| `unix_composition` | excluded | excluded |

The Windows entry point is `main.rs` → `windows_service_host::run()`; the service
then accepts the named pipe and invokes `router::handle_request` for each request.

## NEWLY_REACHABLE_HANDLERS

The complete `Request` oneof is now reachable from the Windows service (protocol
version 7). The message names and tags are defined in
`crates/contracts/proto/operations.proto:21-109`:

- Core/session: `Ping` (10), `GetSnapshot` (11), `HydrateSession` (45).
- Consent: `BeginConsentIntent` (42), `GetConsentIntent` (43),
  `ApproveConsentIntent` (44).
- Driver: `StartDriverScan`, `GetDriverHubSnapshot`, `SetDriverCandidatePolicy`,
  `CreateDriverInstallPlan`, `StartDriverInstall`, `GetDriverInstallStatus`,
  `GetRecoveryHistory` (16–21, 70).
- Repair: `StartRepairAssessment`, `GetRepairAssessment`,
  `CreateSystemRepairPlan`, `StartSystemRepair`, `GetSystemRepairStatus` (22–26).
- Cleanup: `StartCleanupScan`, `GetCleanupSnapshot`, `CreateCleanupPlan`,
  `StartCleanup`, `GetCleanupStatus` (27–31).
- Startup: `StartStartupScan`, `GetStartupSnapshot`, `CreateStartupPlan`,
  `CreateStartupRestorePlan`, `StartStartupChanges`, `GetStartupStatus`,
  `GetStartupHistory` (32–38).
- Diagnostics/deep scan: `StartDiagnosticsScan`, `GetDiagnosticsSnapshot`,
  `GetDiagnosticsHistory`, `StartDeepScan`, `CancelDeepScan`,
  `GetDeepScanSnapshot`, `GetDeepScanHistory`, `SealRemediationPlan` (39–41, 65–69).
- Updates: `CheckForUpdates`, `GetUpdateSnapshot`, `StageUpdate`,
  `GetUpdateCheckDescriptor`, `SubmitUpdateManifest`, stage-upload begin/write/finalize/cancel,
  and install-intent get/claim/complete/cancel (46–64).
- Support: `CreateSupportBundlePreview`, `PrepareSupportBundle`,
  `ReadSupportBundleChunk`, `DiscardSupportBundle`, `MarkSupportBundleExported` (53–57).
- Performance: `StartPerfSampling`, `StopPerfSampling`, `GetPerformanceSnapshot`,
  `GetBottleneckReport`, `CreateOptimizationPlan`, `StartOptimization`,
  `GetOptimizationStatus` (71–77).
- Timeline/care/intelligence: `GetTimelinePage`, `GetRecurrencePatterns` (78–79);
  `StartCareRun`, `GrantCareSessionConsent`, `GetCareStatus`, `CancelCareRun` (80–83);
  `ListInsights`, `RequestInsight`, `DismissInsight` (84–86).
- Platform/enterprise/security: `GetPlatformCapabilities` (87), `GetEngineSource` (88),
  `ExportJournal` (89), and `RunSecurityAudit` (90).

## MUTATING_PATHS and authorization

The request-level guard in `router.rs:104-136` validates protocol, request ID,
deadline/cancellation context, and binds the request to the immutable pipe principal
key. It is not a capability grant by itself.

| Path | Mutation / privilege | Authorization observed |
|---|---|---|
| Driver install, system repair, cleanup, startup changes | Machine-affecting execution | `router.rs:287-312`, `384-408`, `475-500`, `595-620` checkpoint, acquire an owner-keyed `MutationWorkload` lease, then domain `start_with_lease`; domain consumes one-shot owner-bound consent. |
| Care run | May execute safe cleanup/startup plans | `GrantCareSessionConsent` records consent for current principal (`router.rs:1226-1240`); orchestrator requires it and does not auto-run review-only driver/repair work. |
| Update stage upload | Writes bounded staged package data | Owner-bound upload state, bounded chunks/package, signed manifest/hash checks; install claim/complete additionally require update broker. |
| Consent intent read/approval | Authorization state transition | `GetConsentIntent` and `ApproveConsentIntent` call `require_broker` before access (`router.rs:169-185`). Intent is owner-bound and one-shot. |
| `CheckForUpdates`, `StageUpdate` | Legacy service-side download | Explicitly rejected as disabled (`router.rs:771-783`). |
| `StartOptimization` | Potential machine optimization | Explicitly returns forbidden; execution requires consent (`router.rs:1175-1182`). |
| Support bundle | Reads diagnostics/history and creates bounded bundle | Support engine previews, chunks, and discard are owner-scoped; data is sanitized and size-limited. |
| Most scans/snapshots/performance/timeline/insights | Read-only service state/telemetry | Principal key is passed to domain accessors; performance sampling is bounded and owner-tagged. |
| `ExportJournal` | Read-only persistence disclosure | **Defect:** caller-supplied non-empty `owner_principal_key` is used verbatim (`router.rs:1389-1407`); no equality check against `principal_key`. |
| `RunSecurityAudit` | Read-only, but executes as LocalSystem and returns findings | **Defect:** no owner/path authorization; see primary finding below. |

All handler filesystem and domain work runs in the service process after temporary
pipe-client impersonation has been reverted (`server.rs:189-203`; security
inspection reverts at `crates/security/src/lib.rs:92-108`). Thus “read-only” does
not mean caller-permission-limited.

## Primary finding — arbitrary LocalSystem file disclosure (NOT SAFE)

The wire contract says an “Owner-scoped allowlist is enforced by the router”
(`operations.proto:170-174`). The implementation does not enforce it:

1. `router.rs:1515-1562` decodes a non-empty `RunSecurityAuditRequest`, calls
   `validate_targets`, then directly calls `run_audit`.
2. `crates/security-audit/src/lib.rs:75-107` validates only that paths do not
   contain a `..` component. Absolute paths, other users’ profile trees, junctions,
   and protected files are not restricted to an approved root or canonical owner path.
3. `run_audit` dispatches the named paths to providers. `sshd.rs:123-135`,
   `sudoers.rs:114-118`, and `authlog.rs:68-77` call `metadata`/`read_to_string`
   directly. `secrets.rs` recursively scans the supplied directory.
4. A local authenticated user can therefore send a valid v7 request over the
   intentionally accessible Authenticated Users pipe, naming (for example) another
   user’s `SecretsDir` or `AuthLogs` path. The service reads it as LocalSystem and
   returns findings with source locations and evidence.

Disclosure is material even with the scanner’s redaction contract: the secrets
provider retains the first four characters of a detected credential
(`secrets.rs:63-70`), while SSH and sudoers findings carry verbatim matching lines
(`sshd.rs:138-150`, `sudoers.rs:81-103`), and auth-log findings carry verbatim log
lines and source/path metadata (`authlog.rs:110-137`). This is a concrete
privilege-boundary failure introduced when the previously unreachable router became
reachable in the Windows LocalSystem service.

### Secondary disclosure — journal owner override

`ExportJournalRequest.owner_principal_key` documents empty as “calling principal’s
own records” (`operations.proto:196-204`), but the non-empty value is accepted
without checking `owner == principal_key` (`router.rs:1392-1407`). A caller who can
obtain another binding key can request that owner’s execution, support-journal, and
repair-timeline records. This is independent corroboration that newly reachable
read paths need an explicit owner check.

## INPUT_VALIDATION

Present controls are real but insufficient for the security-audit trust boundary:

- Protocol is fixed at v7; request IDs are non-empty, ASCII-safe, and max 128 bytes
  (`router.rs:104-121`; `contracts/src/lib.rs:3-10`).
- Deadlines default to 15 seconds and are capped at 120 seconds; cancellation IDs
  are capped at 256 bytes (`server.rs:389-420`; `contracts/src/lib.rs:15-16`).
- Client/session frames are bounded to 384 KiB and server responses to 8 MiB.
- Audit targets are typed with `deny_unknown_fields`, non-empty at the router, and
  reject only `..` path components. There is no canonicalization, allowed-root
  policy, owner-SID/path relationship, reparse-point defense, or impersonated read.

## RESOURCE_EXHAUSTION

The Windows server has bounded sessions and work: 32 sessions total, four per user
SID, eight in-flight requests per session, 64 globally (`server.rs:25-28`,
`340-367`), with bounded IPC queues. Audit providers cap regular-file parsing at
1 MiB, scans at 4,096 files and two seconds, and findings/evidence at fixed limits
(`security-audit/src/model.rs:20-27`). Update uploads and support bundles are also
size/chunk bounded.

These controls prevent an unbounded allocation, but a local user can still submit
repeated expensive scans of arbitrary readable-by-LocalSystem trees until the
bounded worker/session quotas are occupied. This is a secondary availability risk;
the decisive release blocker is disclosure, not an unbounded-DoS claim.

## BROKER_TRUST_GATES

The broker gates themselves are materially stronger than the audit path:

- `require_broker` and `require_update_broker` are now compiled on Windows and run
  before broker-only operations (`router.rs:1661-1718`).
- `is_expected_broker` requires an elevated token and exact normalized executable
  path. The path is derived from the running service executable, not caller input.
- The named pipe rejects remote clients, claims the first pipe instance, verifies
  the connected endpoint’s service SID/registered running service, and grants AU
  only read plus `FILE_WRITE_DATA` (`windows_impl.rs:267-285`, `758-764`). The
  service captures PID/session and the exact impersonated client token, then always
  reverts before dispatch (`security/src/lib.rs:69-108`).
- Consent and update intent state is additionally bound to the caller’s
  `binding_key` (SID + authentication ID) and consumed one-shot.

The remaining caveat is by design: `is_expected_broker` authenticates elevation and
exact image path, while the router’s intent methods provide owner binding; it does
not independently compare broker SID/session. That does not repair the unrelated
arbitrary-path audit disclosure.

## Required disposition

`NOT SAFE` — do not proceed to the requested Phase 2 branch, Tauri path change,
`engine_source` investigation, or ProgramData rerun. The owner must first decide
the remediation contract for `RunSecurityAudit` (and the journal owner override),
then re-run the security review and the required Windows standard-user checks.

## Review report fields

| Field | Result |
|---|---|
| `UNGATING_COMMITS` | `0e30038` (Unix-only split context); `39ccab0` (actual Windows re-inclusion) |
| `MODULES_BEFORE/AFTER` | Six modules moved from excluded to compiled on Windows: errors, performance, protocol, router, streaming, support |
| `NEWLY_REACHABLE_HANDLERS` | Complete v7 `Request` oneof, tags 10–90, listed above |
| `MUTATING_PATHS` | Driver/repair/cleanup/startup execution and update staging are reachable but consent/lease/broker constrained; care is session-consent constrained |
| `AUTHORIZATION_CHECKS` | Protocol/principal binding globally; owner-keyed leases/one-shot consent; broker gates; missing audit allowlist and journal owner equality |
| `INFORMATION_DISCLOSURE` | **High-severity LocalSystem arbitrary-path audit disclosure;** secondary journal owner override |
| `INPUT_VALIDATION` | Typed requests, frame/deadline/ID limits, only traversal-component check for audit paths |
| `RESOURCE_EXHAUSTION` | Bounded sessions, workers, queues, scan files/time/bytes; repeated arbitrary scans remain an availability concern |
| `BROKER_TRUST_GATES` | Exact elevated image path plus service-owned pipe/SID checks; intent state owner-bound and one-shot |
| `VERDICT` | **NOT SAFE** |
| `PHASE_2` | **Stopped; not entered** |

