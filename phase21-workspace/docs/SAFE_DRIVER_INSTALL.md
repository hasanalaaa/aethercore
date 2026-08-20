# Safe Driver Installation — Phase 3

## Objective

Phase 3 converts Phase 2's read-only Windows driver candidates into a narrowly constrained, recoverable installation workflow. The privileged service is the sole mutation authority. The desktop reviews and requests intent; it never constructs the installation operation.

## End-to-end flow

```text
Phase 2 Ready snapshot
  -> selected service-issued candidate IDs
  -> immutable service plan (AwaitingAuthorization)
  -> UAC digest authorization
  -> Preflight
       PnP identity/binding revalidation
       WUA Update ID + revision revalidation
       busy / reboot checks
  -> asynchronous WUA download
  -> final WUA busy / reboot check
  -> protection barrier
       BEGIN_SYSTEM_CHANGE
       verify fresh restore point
       export applicable current OEM packages
       hash + manifest backups
       durable Protected checkpoint
       durable mutation_started + Executing transition
  -> BeginInstall / EndInstall
  -> close restore transaction
  -> Verifying
       PnP re-interrogation
       WUA per-item result checks
  -> Completed | RebootPending | Failed/RecoveryRequired
```

The ordering matters: packages may be downloaded before the restore point exists because downloading is not a driver mutation. The restore point/backup window is therefore kept close to `BeginInstall`, while the durable barrier guarantees the service cannot invoke installation unless protection evidence is committed.

## Immutable plan material

Each `InstallWindowsDriver` action captures:

- Phase 2 candidate ID;
- WUA Update ID and revision;
- PnP Device Instance ID;
- matched Hardware ID/Compatible ID evidence and match quality;
- current provider/version/INF/date evidence;
- current Device Manager problem state;
- update title/provider/class and size range.

The plan's immutable JSON is SHA-256 hashed. Persistence re-computes that digest when loading the plan; corruption/tampering fails closed.

## Preflight

Before any mutation, the coordinator:

1. fetches every target Device Instance ID again;
2. requires the same present device and class;
3. requires the immutable matched ID still to exist in the device's Hardware IDs or Compatible IDs;
4. requires the current bound driver evidence to still match provider, version, INF, and date;
5. requires the problem state/code to match the reviewed scan;
6. rejects display and firmware classes again;
7. WUA re-searches the exact Update ID + revision and requires a complete search result;
8. rejects already-installed/no-longer-applicable offers and unaccepted EULAs;
9. checks WUA busy/reboot state.

Any mismatch forces a new Phase 2 scan/review rather than silently adapting the authorized plan.

## WUA download and installation

The WUA layer uses the official object model:

- `IUpdateDownloader::BeginDownload/EndDownload` for asynchronous download;
- `IDownloadJob` progress for real percentage and byte telemetry;
- `IUpdateInstaller::BeginInstall/EndInstall` for asynchronous installation;
- `IInstallationJob` progress for installation telemetry;
- `IUpdateInstaller2::ForceQuiet=true` where supported so no privileged interactive UI appears;
- `IsForced=false`, so AetherCore never overrides Windows applicability;
- `AllowSourcePrompts=false`, so the service cannot block on source UI;
- no automatic EULA acceptance;
- no forced restart.

A machine-wide AetherCore mutex serializes its own WUA mutation operations. `IsBusy` and `RebootRequiredBeforeInstallation` are checked before download and again immediately before mutation.

## System Restore transaction

The restore-point crate dynamically loads `srclient.dll` and resolves `SRSetRestorePointW`. It pairs:

- `BEGIN_SYSTEM_CHANGE` + `DEVICE_DRIVER_INSTALL` before mutation; and
- `END_SYSTEM_CHANGE` + `DEVICE_DRIVER_INSTALL` after completion, or `CANCELLED_OPERATION` when no mutation occurred.

Windows may suppress a new restore point under frequency policy while returning an earlier sequence. AetherCore therefore queries `ROOT\\DEFAULT:SystemRestore` and accepts the protection only if the exact returned sequence has AetherCore's unique description. Failure to prove freshness blocks installation.

## Driver-package backup

For each target with a currently bound third-party OEM INF:

```text
%SystemRoot%\System32\pnputil.exe /export-driver oemNN.inf <service-owned candidate directory>
```

The executable/path/verb are fixed. No shell is involved. The candidate directory must be empty and non-reparse; output is bounded, hashed with SHA-256, and recorded in `aethercore-backup.json`.

If the target has no bound driver or an in-box/non-OEM INF, backup is explicitly recorded as not applicable. If an applicable OEM export fails, mutation is blocked.

## Durable journal and recovery

Phase 3 adds:

- `plan_executions` — current execution state/telemetry/protection evidence;
- `driver_install_items` — per-device/update result and before/after evidence;
- `execution_checkpoints` — append-only checkpoints around safety boundaries;
- `recovery_records` — operator-visible recovery evidence.

`mutation_started=true` and the `Protected -> Executing` transition are committed immediately before WUA `BeginInstall`.

### Reboot

If WUA reports `RebootRequired`:

- AetherCore records a boot marker and enters `RebootPending`;
- it never restarts Windows itself;
- after service startup, a changed boot marker allows `Resuming -> Verifying`;
- only device verification is repeated; installation is never replayed.

### Unexpected restart during mutation

If the journal says `Executing` or `Verifying` when the service restarts, AetherCore marks the plan failed/recovery-required, writes a recovery record, and does not call WUA installation again. Restore-point and backup evidence remain visible in Activity & recovery.

## UI behavior

The Drivers page adds:

- immutable plan review with plan digest/action count;
- explicit UAC authorization;
- separate Start installation action after the plan's one-shot consent intent has been atomically approved;
- WUA download/install stage and actual progress;
- bytes downloaded/known total when supplied by WUA;
- per-device result, HRESULT, before/after version, verification state, and backup path;
- restore-point and mutation-barrier status;
- non-forcing restart notification;
- recovery history that survives app/service restarts.

A pending driver plan is restored from the service journal when the UI starts. Driver plans use only the production typed operation state machine; no demo transition endpoint exists.
