# Phase 4 Deliverables — System Repair & Deep Cleanup

Phase 4 extends the verified Phase 0–3 repository with two journaled privileged maintenance domains.

## Added crates

### `crates/system-repair`

- asynchronous read-only integrity assessment;
- fixed direct execution of `%SystemRoot%\System32\dism.exe`, `sfc.exe`, and `chkdsk.exe`;
- Windows Update servicing collision/reboot preflight;
- shared machine-wide mutation serialization;
- durable mutation callback immediately before privileged repair work;
- DISM component-store workflow, SFC repair, optional CHKDSK `/scan`;
- post-repair verification;
- restart recovery that never automatically replays repair commands;
- fake-platform coordinator tests and an ignored elevated Windows live-assessment probe.

### `crates/cleaner`

- explicit Windows cleanup provider allowlist;
- age thresholds and conservative default-selection policy;
- service-minted candidate IDs and immutable exact-file evidence;
- duplicate-selection and file-count safety caps;
- reparse-point ancestor/path rejection;
- frozen final-root identity plus final-path-by-handle containment validation;
- size/modified-time revalidation immediately before deletion;
- deletion through `SetFileInformationByHandle(FileDispositionInfo)`;
- shared machine-wide mutation serialization for the whole cleanup execution;
- partial/locked/changed-file skip accounting;
- restart recovery with no deletion replay;
- fake-platform coordinator/fault tests and an ignored Windows live scan.

## Shared platform changes

- Protocol version 3 and Phase 4 Protobuf request/response types.
- `SystemRepairAction`, `CleanupDeleteAction`, and `CleanupFileEvidence` immutable plan material.
- SQLite migration `0003_phase4.sql` for generic maintenance execution/item journaling.
- Maintenance service handlers and startup recovery for Repair/Cleanup.
- Tauri commands with typed IDs only; no raw paths or command arguments.
- Svelte Repair and Cleanup dashboards with immutable review/UAC flows.
- Cross-phase recovery history remains unified.
- Windows CI and `scripts/verify-phase4.ps1` include the new crates/tests.
- Dependency-light `scripts/static_validate.py` validates Phase 0–4 source/security invariants.

## Explicit non-features

Phase 4 does not implement registry cleaning, WinSxS deletion, Installer cache deletion, Driver Store deletion, Prefetch deletion, arbitrary directory cleaning, browser-session cleaning, CHKDSK `/f`/`/r`/`/spotfix`, forced reboot, Windows Update service disabling, arbitrary DISM sources, arbitrary command execution, Recycle Bin emptying, or automatic replay after interrupted mutation.

## Validation boundary

The source/static validation gate can run outside Windows. Native compilation and live API behavior remain gated by Windows `cargo check --workspace`, tests, Svelte build/check, and opt-in read-only/elevated probes in `verify-phase4.ps1`.

## Authoring-runtime static gate

The final dependency-light source/security validator reports `ok: true` with 25 checks. This includes explicit ordering of the System Repair mutation barrier, stable cleanup-root identity, absence of whole-Recycle-Bin deletion, Protobuf/service/Tauri wiring, migrations, allowlists, and mutation-surface restrictions. Native Windows compilation and live mutation remain acceptance-gated by `scripts\verify-phase4.ps1` and Windows CI.
