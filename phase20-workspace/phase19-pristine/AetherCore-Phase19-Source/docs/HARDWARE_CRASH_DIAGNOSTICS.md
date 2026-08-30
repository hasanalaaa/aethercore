# Hardware Telemetry & Crash Diagnostics — Phase 6

## Purpose

Phase 6 adds a read-only evidence pipeline for physical-storage telemetry, system-memory pressure, Windows Hardware Error Architecture (WHEA) evidence, unexpected-shutdown/crash events, and lightweight minidump metadata. It deliberately does **not** turn those signals into a synthetic PC health percentage or a deterministic failed-component verdict.

The subsystem is split into three crates:

- `aethercore-hardware-telemetry` — storage + current memory telemetry.
- `aethercore-crash-diagnostics` — System Event Log + `%SystemRoot%\Minidump` metadata.
- `aethercore-diagnostic-engine` — orchestration, evidence cards, confidence labels, snapshot persistence, and history.

All Phase 6 collection is read-only and uses the existing service/Protobuf boundary. No UAC grant or maintenance mutation plan is required for a diagnostic scan.

## Storage telemetry

### Standardized Windows reliability source

`hardware-telemetry` queries `ROOT\Microsoft\Windows\Storage` for:

- `MSFT_PhysicalDisk`
  - device ID
  - friendly name
  - firmware and serial metadata when exposed
  - bus/media type
  - size
  - Windows-reported health state
- `MSFT_StorageReliabilityCounter`
  - current/max temperature when exposed
  - device wear estimate when exposed
  - power-on hours
  - total/uncorrected read/write errors
  - maximum read/write/flush latency counters

A reliability provider failure does not erase the physical-disk inventory. Devices remain visible, their reliability fields remain unset, and source notes state that standardized reliability counters were unavailable.

### Direct NVMe SMART/Health

For Windows-reported NVMe physical disks, the collector additionally opens the corresponding physical-drive device read-only and issues `IOCTL_STORAGE_QUERY_PROPERTY` with `StorageDeviceProtocolSpecificProperty` / `ProtocolTypeNvme` for SMART/Health log page `02h`.

The parser validates:

1. returned `STORAGE_PROTOCOL_DATA_DESCRIPTOR` length/version/size;
2. reported protocol-data length;
3. reported data offset against the actual returned IOCTL buffer;
4. `NVME_HEALTH_INFO_LOG` size before reading the log.

Captured NVMe evidence includes:

- critical-warning flags;
- composite temperature;
- available spare;
- percentage used;
- unsafe-shutdown count;
- media/data-integrity error count;
- error-information log-entry count.

NVMe 128-bit counters are represented as decimal strings so the UI/protocol cannot truncate them to 64 bits.


### Read-only ATA/SATA SMART attributes

For Windows-reported ATA/SATA physical disks, the collector may also issue the documented read-only `SMART_RCV_DRIVE_DATA` control code with the ATA `READ_ATTRIBUTES` command. AetherCore records only the SMART attribute ID, normalized current/worst bytes, and the exact six-byte raw value (decimal + hexadecimal).

ATA SMART raw attribute semantics and units are vendor-defined. Phase 6 therefore does **not** assign universal names, units, failure thresholds, or health conclusions to raw ATA attributes. They are displayed as supplemental device evidence only; automated storage severity remains based on standardized Windows reliability counters and defined NVMe health fields.

### Honest storage classification

The model contains **no `health_score` field**. Each measurement is optional. If Windows/the device does not report a value, it remains absent and the UI says `Not reported` rather than substituting zero.

AetherCore escalates only directly supportable evidence, for example:

- Windows reports the disk `Unhealthy`;
- non-zero uncorrected read/write errors;
- NVMe critical-warning bits;
- non-zero NVMe media/data-integrity errors;
- reported wear at/above the device wear estimate limit;
- current temperature at/above a device/Windows-reported maximum when both are available.

No arbitrary universal temperature or SMART-vendor threshold table is invented in Phase 6.

Critical storage guidance is deliberately backup-first: back up important data, avoid unnecessary heavy writes, use vendor diagnostics/firmware guidance, and replace the device if reliability errors persist or increase.

## Memory telemetry and WHEA

Current memory pressure comes from `GlobalMemoryStatusEx`:

- total physical memory;
- available physical memory;
- current physical-memory load percentage.

This is a resource-pressure metric, **not** a RAM hardware test.

Hardware memory evidence comes separately from WHEA events in the Windows System log. A memory-related WHEA event may justify an `Attention` card and guidance to use Windows Memory Diagnostic/offline DIMM testing, but Phase 6 does not identify a particular DIMM unless the underlying evidence explicitly supports that conclusion.

If no memory-related WHEA events are found in the queried window, the card wording is exactly scoped to the log evidence: **“No logged memory hardware errors were found.”** It explicitly states that this does not prove RAM is fault-free.

## Event Log collection

`crash-diagnostics` uses Windows Event Log APIs (`EvtQuery`, `EvtNext`, `EvtRender`) against the `System` channel, newest first, with an explicit 30-day `TimeCreated[timediff(@SystemTime) <= ...]` window and a hard cap of 128 matching records per scan.

The current provider set is:

- `Microsoft-Windows-WHEA-Logger` — hardware-error evidence;
- `Microsoft-Windows-Kernel-Power` — Event 41 unexpected-shutdown evidence;
- `Microsoft-Windows-WER-SystemErrorReporting` — crash/bugcheck evidence.

The collector preserves provider, event ID, event timestamp, bounded derived category, confidence, summary, and detail. Raw XML is used locally for classification but is not exported wholesale through the UI contract.

### Non-exaggerated event semantics

- Kernel-Power Event 41 confirms an unclean shutdown/restart; it does not identify the cause.
- WHEA confirms Windows recorded hardware-error evidence; a summarized event is not automatically proof of a failed CPU/RAM/PCIe component.
- WER bugcheck evidence confirms a system crash report; precise module attribution may require a dump plus symbols.

## Minidump metadata

The collector enumerates only `%SystemRoot%\Minidump\*.dmp`, newest first, with a hard count cap. It exposes the file name, size, file timestamp, and lightweight kernel-header metadata.

Phase 6 recognizes the Windows kernel dump signatures and reads bugcheck metadata through the official `DUMP_HEADER64` or `DUMP_HEADER32` layouts supplied by the Windows bindings. Unsupported/unrecognized layouts remain metadata-only rather than being interpreted through guessed offsets.

The service does **not** upload dumps and does not expose arbitrary dump paths. Full driver/module attribution is explicitly outside the lightweight parser because meaningful dump debugging can require matching binaries and symbol files.

## Correlation and diagnostic cards

`diagnostic-engine` combines the independent collectors into cards with explicit evidence classes such as:

- `ReportedMetricEvidence`
- `LoggedHardwareEvidence`
- `CurrentOSMetric`
- `HeaderEvidence`
- `EventHigh/CauseLow`
- `LogWindowOnly`

If a recent dump and WHEA event timestamps fall within ±10 minutes, the crash card reports the temporal correlation and explicitly says that correlation is **not proof of causation**.

If WER records a bugcheck but no minidump is available, the engine produces a crash-evidence card instead of pretending no crash happened. If only Kernel-Power Event 41 exists, it reports an unexpected shutdown with low root-cause confidence.

## Partial collection and history

A diagnostic scan has the states:

`Idle -> Collecting -> Ready | Partial | Failed`

Storage/memory and crash/event collectors fail independently. If one collector fails, the successful evidence remains useful and the snapshot becomes `Partial` with an explicit warning. Only failure of both collector families becomes `Failed`.

Completed snapshots are serialized into SQLite table `diagnostic_snapshots`. The service retains the newest 50 snapshots per owner and exposes bounded, principal-scoped history metadata through the current Protocol v7 contract. This is diagnostic history, not a privileged mutation journal.

## IPC and privacy

Phase 6 introduced the typed diagnostic operations; Protocol v7 retains them and adds Phase 9 principal ownership at the service boundary:

- `StartDiagnosticsScan`
- `GetDiagnosticsSnapshot`
- `GetDiagnosticsHistory`

No Phase 6 message accepts a filesystem path, event-log query, WMI query, IOCTL buffer, command line, or remediation command from the UI.

The UI receives structured telemetry/evidence only. It does not receive full minidump paths or raw Event Log XML. All Phase 6 collection happens inside the native service boundary.

## UI behavior

The Hardware page presents:

- per-disk source and identity metadata;
- available temperatures, wear, power-on hours, errors, NVMe evidence, and latency counters;
- explicit `Not reported` states;
- current memory pressure;
- evidence-based diagnostic cards.

The Crash History page presents:

- recent minidump metadata;
- WHEA/Kernel-Power/WER evidence from the explicit 30-day/128-event query window;
- confidence-scored triage cards;
- safe guided next actions.

The interface never labels an unsupported device “healthy” merely because counters are missing and never assigns a percentage health score.

## Validation boundary

The authoring environment cannot execute Windows WMI, NVMe IOCTLs, Event Log APIs, or native minidump collection. The authoritative gate is therefore Windows:

```powershell
.\scripts\verify-phase6.ps1
```

Optional read-only live probes:

```powershell
.\scripts\verify-phase6.ps1 -LiveTelemetry
```

Those probes collect telemetry/events/minidump metadata only. They do not repair, delete, install, disable, or schedule anything.
