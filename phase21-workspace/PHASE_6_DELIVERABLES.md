# AetherCore Phase 6 Deliverables

## Milestone

**Hardware Telemetry & Crash Diagnostics Engine**

Phase 6 adds a read-only, evidence-first diagnostic subsystem to the existing AetherCore service/UI architecture.

## New native crates

### `crates/hardware-telemetry`

- `MSFT_PhysicalDisk` inventory through Windows Storage WMI.
- `MSFT_StorageReliabilityCounter` standardized temperature/wear/error/latency telemetry when exposed.
- direct NVMe SMART/Health log query via `IOCTL_STORAGE_QUERY_PROPERTY` / `StorageDeviceProtocolSpecificProperty`.
- read-only ATA/SATA SMART attribute-table retrieval via `SMART_RCV_DRIVE_DATA`; raw vendor-defined values are preserved but never converted into universal AetherCore health claims.
- exact decimal preservation of NVMe 128-bit counters.
- `GlobalMemoryStatusEx` current physical-memory pressure.
- optional metric model; no synthetic health percentage.
- storage evidence classifier with backup-first critical guidance.
- unit tests + ignored read-only live Windows collector test.

### `crates/crash-diagnostics`

- newest-first System Event Log collection through `EvtQuery`/`EvtNext`/`EvtRender`, bounded to the newest 30 days and at most 128 matching events.
- WHEA, Kernel-Power and WER SystemErrorReporting evidence.
- evidence categories/confidence that do not overstate root cause.
- bounded `%SystemRoot%\Minidump` enumeration.
- lightweight bugcheck metadata parser using the Windows `DUMP_HEADER64` and `DUMP_HEADER32` layouts; unsupported layouts remain metadata-only.
- file-name-only dump exposure; no arbitrary path or dump upload.
- unit tests + ignored read-only live Windows collector test.

### `crates/diagnostic-engine`

- asynchronous `Idle -> Collecting -> Ready | Partial | Failed` scan state.
- independent collector failure isolation.
- evidence/confidence diagnostic cards and safe guided actions.
- crash/WHEA temporal correlation labeled non-causal.
- WER-without-dump and Kernel-Power-without-dump triage paths.
- SQLite snapshot/history persistence with bounded retention.
- unit tests for honest wording, critical-storage guidance, partial scans, and persistence.

## Persistence

Migration `0005_diagnostics.sql` adds `diagnostic_snapshots` with timestamp index.

The database keeps the newest 50 diagnostic snapshots. Phase 6 history is observational; it does not create or replay mutation checkpoints.

## Contracts and IPC

Protocol upgraded to **v6**.

New typed requests:

- `StartDiagnosticsScanRequest`
- `GetDiagnosticsSnapshotRequest`
- `GetDiagnosticsHistoryRequest`

New response/model families expose structured:

- storage reliability;
- storage device telemetry;
- current memory telemetry;
- diagnostic events;
- minidump/crash metadata;
- diagnostic cards;
- bounded diagnostic history.

Optional metrics use explicit presence booleans in Protobuf so missing values cannot be confused with zero.

No Phase 6 request accepts a path, WMI/Event Log query, command, IOCTL request, or mutation intent.

## Maintenance service

The service owns one `DiagnosticEngine` instance and serves Phase 6 scan/snapshot/history endpoints over the existing authenticated named pipe.

A scan is read-only and requires no UAC authorization. The latest durable diagnostic snapshot is restored into the engine view after service restart.

## Tauri + Svelte UI

The previously deferred **Hardware** and **Crash history** navigation pages are now live.

Hardware dashboard:

- storage device cards;
- reported vs unavailable SMART/reliability metrics;
- current memory pressure;
- evidence-based storage/memory cards;
- explicit diagnostic honesty note.

Crash dashboard:

- WHEA/Kernel-Power/WER evidence;
- minidump metadata + bugcheck header when available;
- confidence labels;
- guided next actions;
- explicit root-cause-unknown behavior.

No UI surface displays a fabricated health score.

## Safety invariants

1. Phase 6 collection is read-only.
2. Missing telemetry remains missing; no zero substitution.
3. No synthetic whole-PC/drive/RAM health percentage.
4. Current memory pressure is not a hardware-health verdict.
5. “No WHEA memory event found” is not “RAM healthy.”
6. Kernel-Power Event 41 does not identify root cause.
7. Minidump filename/header does not identify a culprit driver/module.
8. Temporal WHEA/crash proximity is correlation, not causation.
9. Raw event XML, arbitrary dump paths, and arbitrary collector queries never cross the UI contract.
10. A failed collector produces `Partial` when other evidence remains available.

## Verification

Primary Windows gate:

```powershell
.\scripts\verify-phase6.ps1
```

Optional read-only physical probe:

```powershell
.\scripts\verify-phase6.ps1 -LiveTelemetry
```

Optional Phase 5 inventory in the same validation session:

```powershell
.\scripts\verify-phase6.ps1 -LiveTelemetry -IncludePhase5Inventory
```

The live Phase 6 probes intentionally perform no repair or system mutation.

The source-delivery static validator checks workspace membership, Protocol v6, diagnostic migration, collector API markers, ATA read-only-only behavior, the explicit 30-day/128-event bound, official dump-header layouts, honest-reporting markers, read-only surface, service/Tauri/UI wiring, and Windows-gate presence. Final source result: **49/49 checks successful**.
