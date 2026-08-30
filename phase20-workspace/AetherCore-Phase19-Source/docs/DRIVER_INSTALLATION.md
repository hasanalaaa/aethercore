# Safe Driver Installation — Phase 3 Design

## Transaction overview

Phase 3 turns a Phase 2 selection into a privileged mutation without turning the service into a generic installer.

```text
Ready Phase-2 snapshot
  -> service resolves candidate IDs
  -> immutable Amber plan / AwaitingAuthorization
  -> UAC broker approval of a service-minted, principal-bound consent intent for the exact plan digest
  -> Preflight
     - fresh PnP identity/class/driver/problem-state check
     - fresh complete WUA applicability search
     - active-WUA/reboot-before-install checks
  -> WUA asynchronous download (still no driver mutation)
  -> protection barrier
     - verified fresh System Restore BEGIN
     - export applicable bound OEM INF packages
     - persist backup manifests/checkpoints
     - Preflight -> Protected
     - durable mutation-started checkpoint
     - Protected -> Executing
  -> WUA BeginInstall / EndInstall
  -> Executing -> Verifying
  -> paired System Restore END
  -> fresh devnode verification
     -> Completed
     -> RebootPending -> restart -> Resuming -> Verifying -> Completed/Failed
     -> Failed + RecoveryRequired
```

The ordering is intentional: potentially long network download work occurs before opening a restore transaction, while every protection step remains immediately before `BeginInstall`.

## Service-owned plan material

The UI supplies only:

- `scan_id`;
- `inventory_epoch`;
- selected service-generated candidate IDs.

The service resolves and freezes:

- WUA update GUID + revision;
- device instance ID;
- matched Hardware/Compatible ID evidence;
- device class;
- current provider/version/INF/date;
- current devnode problem code;
- offer title/provider/class/size range.

That canonical material is SHA-256 digested by the existing operation engine. The elevated broker authorizes that digest, not a UI command.

## Preflight and TOCTOU defense

Immediately before execution, every planned target is re-interrogated through PnP/Configuration Manager. AetherCore rejects the plan when a device disappeared, its class changed, the matched ID is gone, its current driver differs, or its problem state changed.

WUA then runs a new online search and requires `orcSucceeded`. Every exact update ID/revision must still be present, uninstalled, EULA-accepted, and non-display/non-firmware. No version-number ranking substitutes for WUA applicability.

## Windows Update execution

`aethercore-windows-update` uses the supported asynchronous interfaces:

- `IUpdateDownloader::BeginDownload` / `EndDownload`;
- `IDownloadJob::GetProgress` for real percentage and byte telemetry;
- `IUpdateInstaller::BeginInstall` / `EndInstall`;
- `IInstallationJob::GetProgress` for real install telemetry;
- explicit two-hour worker timeouts with `RequestAbort` on timeout;
- `IsBusy` and `RebootRequiredBeforeInstallation` before download and immediately before mutation;
- `IsForced=false` and no silent `AcceptEula`;
- `IUpdateInstaller2::ForceQuiet=true` only to suppress Windows UI/source prompts, never to override applicability.

One WUA update that services multiple selected device instances executes once; its result is fanned back to each immutable device action.

## System Restore

The restore wrapper deliberately follows the legacy native contract rather than assuming a successful Boolean means a new restore point exists:

1. initialize process-wide COM security before WUA/WMI use;
2. initialize COM on each worker thread;
3. dynamically load `%SystemRoot%`'s `srclient.dll` through the system loader and resolve `SRSetRestorePointW`;
4. call `BEGIN_SYSTEM_CHANGE` with `DEVICE_DRIVER_INSTALL` and a unique service-generated description;
5. query `ROOT\DEFAULT` `SystemRestore` for the exact returned sequence + description;
6. if freshness cannot be proven, pair the begin with `END_SYSTEM_CHANGE/CANCELLED_OPERATION` and reject installation;
7. after WUA execution, pair the begin with matching `END_SYSTEM_CHANGE`;
8. if END/CANCEL fails, preserve a recovery warning rather than hiding the incomplete protection transaction.

## Driver backup

For a currently bound third-party Driver Store package whose observed INF is `oem<digits>.inf`, AetherCore executes only:

```text
%SystemRoot%\System32\pnputil.exe /export-driver oem#.inf <service-generated-directory>
```

No command shell is involved. Candidate/plan directory names are service UUID-like components; exported reparse points are rejected; each file receives a SHA-256 hash in `aethercore-backup.json`.

When there is no current driver or the current driver is not an OEM INF, backup is explicitly journaled as not applicable. It is not represented as a successful export.

## Durable execution journal

Migration `0002_driver_install.sql` adds:

- `plan_executions` — global stage/progress/restore/backup/reboot/recovery state;
- `driver_install_items` — per-device WUA and post-PnP result;
- `execution_checkpoints` — append-only barrier/protection/recovery evidence;
- `recovery_records` — user-visible recovery history.

Progress telemetry is rate-limited before durable writes so `synchronous=FULL` is not turned into a high-frequency progress database.

## Post-install verification

After WUA returns, the service re-opens current PnP state for every target. An item is verified only when:

- WUA's exact per-update result is `orcSucceeded`;
- the device is still present;
- Configuration Manager reports no current devnode problem.

The before/after driver version and problem code are journaled for display. The target version is not used as the authority for success.

## Reboot and recovery

A WUA/item reboot signal results in `RebootPending`; AetherCore never forces restart. On service startup, a reboot marker is compared before moving to `Resuming -> Verifying`.

If the service restarts while a plan is `Executing` or `Verifying`, it does **not** replay WUA installation. It marks the plan failed/recovery-required and preserves restore/backup evidence. Preflight/protected interruptions before the mutation barrier cancel a fresh restore transaction when possible and fail safely.

## Phase 3 exclusions

- display/GPU installation;
- firmware installation;
- arbitrary INF/vendor installer installation;
- automatic rollback;
- forced reboot;
- DISM/SFC/cleanup/startup changes;
- third-party/proprietary driver catalogs.
