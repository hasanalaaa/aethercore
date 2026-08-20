# AetherCore Phase 3 Deliverables

## Scope delivered

Phase 3 implements the first production-oriented privileged mutation path in AetherCore: **safe installation of eligible Windows Update Agent driver offers** selected from the current Smart Driver Hub snapshot.

### 1. System protection

New crate: `crates/restore-point`

- native `SRSetRestorePointW` resolved dynamically from `srclient.dll`;
- paired `BEGIN_SYSTEM_CHANGE` / `END_SYSTEM_CHANGE` flow for `DEVICE_DRIVER_INSTALL`;
- explicit `CANCELLED_OPERATION` path when protection must be abandoned before mutation;
- process-wide COM security initialization plus worker-thread COM apartment initialization;
- WMI `ROOT\DEFAULT:SystemRestore` verification of exact sequence + unique AetherCore description;
- rejection when Windows returns success but a fresh matching restore point cannot be proven;
- ignored elevated live integration probe that creates/verifies/cancels a real restore point.

### 2. Pre-install driver backup

New crate: `crates/driver-backup`

- accepts only service-observed `oem<digits>.inf` names;
- uses fixed `%SystemRoot%\System32\pnputil.exe` with `/export-driver` only;
- service-generated plan/candidate destination roots;
- rejects reparse points/junctions/symlinks inside the exported tree;
- bounds exported file count;
- SHA-256 manifest for every exported file;
- records explicit `BackupNotApplicable` when no OEM package can be exported;
- ignored elevated read-only live export integration probe.

### 3. Immutable plan + UAC gating

Updated `operation-engine`, `driver-hub`, contracts, service, desktop, and UI:

- Phase 2 candidate IDs are resolved against exact current `scan_id` + `inventory_epoch`;
- service freezes WUA identity/revision, devnode identity/class, match evidence, current driver, and problem state;
- resulting Amber plan starts in `AwaitingAuthorization`;
- existing single-use digest-bound UAC broker grant is mandatory before `Preflight`;
- protocol version is 2 and IPC remains typed/bounded;
- no raw INF path, update URL, command line, registry expression, or executable command is accepted from the UI.

### 4. WUA asynchronous execution

Updated crate: `crates/windows-update`

- complete online revalidation of exact WUA update ID/revision before execution;
- EULA must already be accepted; AetherCore does not silently call `AcceptEula`;
- display/firmware class rejected again at WUA execution boundary;
- AetherCore machine-wide mutation mutex;
- `IUpdateInstaller::IsBusy` and `RebootRequiredBeforeInstallation` checks before download and immediately before mutation;
- `IUpdateDownloader::BeginDownload/EndDownload` with polled `IDownloadJob` progress and byte telemetry;
- `IUpdateInstaller::BeginInstall/EndInstall` with `IInstallationJob` progress;
- two-hour bounded async operation timeouts with abort requests;
- `IsForced=false`, source prompts disabled, quiet WUA UI;
- one WUA identity executes once even when it services multiple selected devnodes.

### 5. Transaction coordinator

New crate: `crates/driver-install`

Execution ordering:

```text
Authorization
  -> PnP preflight
  -> fresh complete WUA revalidation
  -> asynchronous download
  -> fresh verified Restore Point
  -> applicable OEM package exports + SHA-256 manifests
  -> durable Protected checkpoint
  -> durable mutation barrier / Executing
  -> WUA BeginInstall
  -> WUA EndInstall
  -> Verifying
  -> paired Restore END
  -> fresh PnP verification
  -> Completed | Failed | RebootPending
```

The protection callback is the only path through which WUA can reach `BeginInstall`.

### 6. Durable journal and recovery

Migration: `crates/persistence/migrations/0002_driver_install.sql`

Adds:

- `plan_executions`;
- `driver_install_items`;
- `execution_checkpoints`;
- `recovery_records`.

Persisted evidence includes progress, WUA result/HRESULT, restore sequence, backup root, before/after driver data and problem codes, reboot requirement, mutation-barrier status, and recovery flags.

Service startup runs `recover_incomplete()`:

- `RebootPending` resumes **verification only** after a reboot is detected;
- interrupted `Executing`/`Verifying` is never replayed and becomes recovery-required;
- pre-mutation `Preflight`/`Protected` interruption fails safely and cancels a fresh restore transaction when possible;
- restore END/CANCEL failure is itself journaled as recovery evidence.

### 7. Post-install verification

`windows-pnp` now exposes target-instance re-interrogation used for both preflight and post-install checks.

A device is marked verified only when the exact WUA item result is `orcSucceeded`, the current device remains present, and Configuration Manager reports no active devnode problem. Reboot-required plans remain pending until post-reboot verification.

### 8. UI integration

The Svelte Drivers workflow now includes:

- **Review & install** plan creation;
- immutable plan action count, risk, epoch, and digest review;
- UAC authorization trigger;
- live WUA percentage and byte telemetry;
- restore-point, backup, and mutation-gate indicators;
- per-device WUA result/HRESULT and before/after version/problem evidence;
- non-forced reboot-pending banner;
- Activity & Recovery history with restore sequence and backup evidence.

### 9. Tests and validation assets

- operation-engine authorization/immutability/transition tests;
- persistence durability + fault-injection tests;
- restore/backup validation tests;
- WUA result/timeout/barrier source invariants;
- `driver-install/tests/coordinator.rs` fake-platform integration test proving:
  - WUA download precedes protection;
  - restore precedes backup;
  - backup precedes mutation;
  - WUA mutation executes once;
  - restore END precedes final PnP verification;
  - an injected OEM-backup failure cancels protection, never crosses the mutation barrier, and never reaches WUA install;
  - an injected restore-END failure after mutation can never report `Completed` and requires recovery review;
  - successful evidence reaches `Completed`;
- `verify-phase3.ps1` deterministic native Windows gate;
- optional `-LiveDiscovery` and elevated `-LiveProtection` probes;
- dependency-light `scripts/static_validate.py` source/contract/security gate.

## Safety exclusions retained

Phase 3 does **not** implement:

- NVIDIA/AMD/Intel or other display driver installation;
- firmware installation;
- arbitrary INF/vendor package installation;
- automatic rollback;
- forced reboot;
- DISM/SFC repair;
- cleaner/startup/service optimization;
- proprietary driver catalog/backend.

## Exit-gate status

The source-level Phase 3 implementation and static consistency/security gate are complete in this deliverable. Native Windows compilation, live System Restore/PnPUtil/WUA behavior, reboot recovery, and physical-driver mutation remain authoritative Windows/hardware gates. See `docs/VALIDATION.md` and `scripts/verify-phase3.ps1`.
