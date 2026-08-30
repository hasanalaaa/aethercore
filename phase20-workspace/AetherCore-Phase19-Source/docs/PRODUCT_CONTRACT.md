# AetherCore Product Contract — Through Phase 6

## Product promise

AetherCore is a conservative Windows maintenance and diagnostic orchestrator, not a fear-based “PC optimizer.” Windows remains authoritative for Windows-managed driver applicability; the privileged maintenance service remains authoritative for native state and mutation; the user remains authoritative for consent.

AetherCore prefers an explicit unknown/unsupported result over a fabricated diagnosis, score, or optimization claim.

## Capabilities through Phase 2 — Driver inventory and discovery

AetherCore may enumerate present PnP devices, read bound-driver and Configuration Manager state, discover applicable Windows driver offers through Windows Update Agent, deterministically attach offers using normalized Hardware IDs/Compatible IDs, and expose service-issued scan/candidate IDs to the non-elevated UI.

All display-class devices and firmware offers remain protected/manual-managed by default.

## Phase 3 — Safe driver installation

For user-selected, service-issued, eligible Windows driver candidates, AetherCore may:

- create immutable Amber-risk plans containing update/device/match/current-driver evidence;
- require digest-bound one-shot UAC consent;
- revalidate PnP and WUA evidence immediately before mutation;
- coordinate with Windows Update activity and pending-reboot state;
- download selected WUA updates asynchronously and report native progress;
- require and independently verify a System Restore point before installation;
- export the currently bound OEM driver package before change;
- durably journal the mutation barrier before WUA installation;
- install only the exact WUA-selected update collection, without forcing applicability;
- re-interrogate devnodes after installation and after a required reboot;
- persist reboot/recovery/backup/restore evidence without automatically replaying a failed installation.

## Phase 4 — System Repair

AetherCore may:

- run read-only integrity assessment with fixed System32 paths/arguments;
- run DISM `/CheckHealth` and `/ScanHealth` before the durable mutation barrier;
- run only DISM `/RestoreHealth` and SFC `/scannow` as repair mutations;
- verify with DISM `/CheckHealth` and SFC `/verifyonly`;
- use CHKDSK `/scan` only as a read-only disk diagnostic;
- serialize servicing through the shared machine-wide mutation lock;
- reject mutation when servicing contention/pending reboot makes execution unsafe;
- preserve durable result/recovery evidence and never replay an interrupted post-barrier repair automatically.

Phase 4 does not expose arbitrary command execution, caller-supplied DISM sources/offline images, CHKDSK repair switches, forced dismount, or forced restart.

## Phase 4 — Deep Cleanup

AetherCore may scan only built-in allowlisted providers. Cleanup plans freeze exact file evidence inside the privileged service. Execution reopens and revalidates the provider root and exact target file through handle-resolved final paths, rejects reparse/junction escape, and skips changed/missing/locked/ambiguous files rather than widening scope.

The UI receives category/count/byte summaries rather than arbitrary personal file paths. Recycle Bin whole-bin emptying remains excluded because it cannot preserve the exact frozen-target invariant.

## Phase 5 — Startup & Background Services Manager

AetherCore may inspect:

- HKCU/HKLM/HKU `Run` and `RunOnce` values, including 32/64-bit views where applicable;
- user/common Startup folders;
- boot/logon scheduled tasks outside protected Microsoft task namespaces;
- eligible third-party automatic own-process services.

The passive-default rule is part of the domain model:

- every item begins `Unreviewed`;
- `Unreviewed` and `KeepEnabled` produce no mutation and are omitted from the plan;
- only explicit `Disable` can become an immutable service-generated plan action;
- protected Windows/security/network/VPN/storage/input/accessibility/dependency-sensitive targets fail closed;
- service changes require an additional explicit confirmation;
- services are not stopped and are not set `SERVICE_DISABLED`; eligible automatic services move to Demand/Manual for future starts;
- exact original state is durably journaled before mutation;
- Restore is a new immutable UAC-authorized plan linked to the original `change_id`;
- restart recovery reconciles native state and never blindly replays a mutation.

AetherCore does not fabricate boot-impact rankings when attributable evidence is unavailable.

## Phase 6 — Hardware Telemetry & Crash Diagnostics

Phase 6 is read-only and may:

### Storage

- enumerate `MSFT_PhysicalDisk` metadata/Windows health state;
- read `MSFT_StorageReliabilityCounter` metrics when a device/driver exposes them;
- read NVMe SMART/Health log page `02h` through `IOCTL_STORAGE_QUERY_PROPERTY` for eligible Windows-reported NVMe devices;
- read ATA/SATA SMART attribute tables through the read-only `SMART_RCV_DRIVE_DATA` path where supported, preserving raw values without assigning universal vendor-specific meanings or thresholds;
- expose temperature, device-reported maximum temperature, wear, power-on hours, errors, latency counters, NVMe critical-warning/spare/percentage-used and 128-bit SMART counters when available;
- classify directly supportable evidence without creating a synthetic health percentage.

Missing metrics remain absent and must be rendered as `Not reported`/unavailable, not as zero.

### Memory

- read current physical-memory pressure using Windows memory-status APIs;
- classify memory-related WHEA events as logged hardware evidence;
- guide users toward Windows Memory Diagnostic/offline DIMM testing when warranted.

Current memory pressure is not a RAM hardware-health test. Absence of matching WHEA events is not proof that RAM is fault-free.

### Crash and BSOD evidence

- query the newest 30 days of System Event Log evidence from WHEA-Logger, Kernel-Power, and WER SystemErrorReporting, capped at 128 matching events per scan;
- enumerate bounded recent `%SystemRoot%\Minidump` files;
- parse only recognized `DUMP_HEADER64`/`DUMP_HEADER32` bugcheck metadata such as bugcheck code/parameters;
- create confidence-labelled diagnostic cards and safe next steps;
- report temporal WHEA/crash proximity only as correlation;
- persist bounded local diagnostic snapshots/history in SQLite.

Kernel-Power Event 41 is unexpected-shutdown evidence only. Minidump metadata alone does not justify assigning a driver/module culprit; module-level attribution may require matching symbols/binaries.

## Explicitly forbidden through Phase 6

AetherCore does not:

- expose arbitrary privileged command/shell endpoints;
- accept caller-supplied executable paths/arguments, arbitrary registry paths, service names, scheduled-task paths, cleanup roots, WMI/Event Log queries, IOCTL payloads, or diagnostic dump paths for privileged execution;
- install arbitrary INF/EXE driver packages or silently install display/firmware updates;
- disable Windows Update services;
- delete WinSxS, Windows Installer cache, Driver Store, Prefetch, browser profiles/sessions, restore points, or heuristic “registry junk”;
- execute CHKDSK repair switches or force restart;
- stop a Phase 5 managed service or set it `SERVICE_DISABLED`;
- auto-disable an `Unreviewed` startup/service item;
- replay interrupted privileged mutations automatically;
- compute or display a fabricated whole-PC/drive/RAM health percentage;
- convert missing SMART/reliability telemetry into zero/healthy;
- apply universal health meanings or failure thresholds to vendor-defined ATA SMART raw attribute values;
- state that RAM is healthy merely because no WHEA memory event was found;
- state that Kernel-Power Event 41 identifies a crash/power root cause;
- infer a culprit driver from a minidump filename/header alone;
- upload minidumps or raw diagnostic telemetry to a product backend in Phase 6.

## Cross-phase safety invariants

1. The UI cannot create arbitrary privileged operations; it references service-issued IDs and bounded typed intent.
2. Mutation plans are immutable and digest-bound before UAC authorization.
3. The service validates caller identity and broker identity at the named-pipe boundary.
4. Plan transitions and recovery checkpoints are durable before the next side effect.
5. Driver/Repair/Cleanup/Startup mutations share one machine-wide mutation lock.
6. No ignored recommendation or `Unreviewed` item is consent.
7. Cleanup never expands beyond exact approved file evidence.
8. Interrupted mutation is not automatically replayed; ambiguous native state becomes recovery evidence.
9. Phase 6 collection never crosses a mutation barrier because it is observational only.
10. Unsupported diagnostic evidence remains unknown/unavailable.
11. Evidence, interpretation, confidence, and guided next action are represented separately.
12. No UI progress percentage is invented when the native platform does not expose meaningful progress.

## Release boundary

A successful source/static gate is not a signed production release. Windows-native `cargo check/test`, physical hardware validation, diverse NVMe/SATA/SAS/RAID/OEM behavior, enterprise policy testing, signed WiX packaging, Authenticode/timestamping, SBOM/dependency audit, IPC fuzzing, adversarial filesystem tests, crash corpus validation, and external security review remain required before public production distribution.

## Zenith interaction invariants

- Visible pointer-down state and native activation must agree: dragging a pressed control outside its hysteresis before release cancels activation, not only its appearance.
- Pointer/system cancellation is not treated as an intentional momentum release.
- Dialog close completion is exactly-once even when reduced motion hard-synchronizes presentation state.
- Toggle selection, mutually exclusive startup decisions, and bounded progress are exposed semantically to assistive technology, not only through color/shape.
- Spatial cues mirror under RTL while technical identifiers remain directionally isolated.
- These renderer invariants do not confer authority: every privileged operation still requires the existing service-side identity, consent, plan and mutation checks.
