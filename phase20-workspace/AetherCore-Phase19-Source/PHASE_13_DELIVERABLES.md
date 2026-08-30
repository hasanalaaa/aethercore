# Phase 13 Deliverables — Engine Reliability Evolution

## Milestone status

Phase 13 hardens AetherCore's native collector and diagnostic boundaries without changing the Phase 9 principal/consent security contract, the Phase 10 persistent IPC v7 architecture, or the Phase 12 localization/bidi contract.

## Delivered architecture

### Shared collector runtime

- New `aethercore-collector-runtime` workspace crate.
- Monotonic per-collector deadlines.
- Hierarchical cooperative cancellation with parent-to-child propagation and sibling isolation.
- Watchdog execution with panic containment.
- `IsolationGate` quarantine preventing retry storms while a timed-out worker is still alive.
- Gate release before successful result publication, removing the immediate-retry race.
- Typed fault taxonomy: timeout, cancelled, unavailable, permission denied, malformed response, provider failure, I/O and internal.
- 4 KiB UTF-8-safe provider-fault detail bound before snapshot/journal/IPC transport.

### WMI and collector timeout discipline

- Removed unbounded WMI enumeration from storage telemetry and System Restore verification.
- Finite semisynchronous WMI `Next` slices controlled by the collector deadline.
- Explicit `WBEM_S_TIMEDOUT` versus terminal `WBEM_S_FALSE` semantics.
- Fail-closed validation for impossible returned-count/object combinations.
- 256-object storage inventory cap.
- Independent storage and memory isolation gates.
- Explicit WMI/Win32 access-denied classification.

### Storage / IOCTL resilience

- Separate gated direct-storage watchdog for optional NVMe/ATA evidence.
- Pure byte-level ATA SMART response parser.
- Pure byte-level NVMe health-log parser.
- Checked protocol offsets/lengths, response-byte counts and header overlap.
- Complete ATA 512-byte sector requirement before interpretation.
- Standard NVMe prefix-only parsing; vendor tail tolerated but not interpreted.
- Malformed, forged and truncated response regressions.
- Explicit non-claim: the caller timeout does not forcibly terminate arbitrary blocked vendor-driver execution; the provider stays quarantined instead.

### Structured Event Log / WHEA

- Removed Event XML substring parsing from crash diagnostics.
- `EvtCreateRenderContext` system/user contexts plus `EvtRenderEventValues`.
- 64 KiB system and 256 KiB user render caps.
- 256-property pre-allocation cap and post-render revalidation.
- Bounded UTF-16 string traversal with pointer containment and alignment checks.
- Immediate RAII ownership for every non-null `EvtNext` event handle.
- Sparse/trailing/null/count-inconsistent event batches fail closed while still closing retrieved handles.
- Finite `EvtNext` slices, 128-event scan cap, typed malformed/access-denied faults.

### Minidump and diagnostic isolation

- Metadata-only minidump path remains bounded to the 64 newest dumps.
- Per-file enumeration/metadata/parse failures preserve successful sibling evidence.
- Permission denied remains a distinct fault class.
- Hardware and crash providers fan out behind independent watchdog gates.
- Nested provider faults are preserved in the diagnostic snapshot.
- Diagnostic snapshot serialization/SQLite persistence failure is visible and downgrades `Ready` to `Partial`.
- Diagnostic worker spawn failure transitions to `Failed` rather than leaving a scan stuck in `Collecting`.

### Typed transport and UI evidence

- `ProviderFaultKind` and `ProviderFaultInfo` added to the diagnostic Protobuf contract.
- Maintenance service maps typed faults without flattening them to display prose.
- Hardware and Crash feature surfaces expose a localized provider-fault panel.
- Provider/operation identifiers are isolated as technical text.
- Raw fault detail is deliberately not rendered in normal UI surfaces.

## Verification deliverables

- `scripts/phase13-reliability-audit.py`
- `scripts/phase13-reliability-audit.ps1`
- `scripts/phase13-fault-injection.ps1`
- `scripts/verify-phase13.ps1`
- `docs/ENGINE_RELIABILITY.md`
- `docs/adr/0015-collector-runtime-and-structured-event-rendering.md`
- CI and signed-release workflows routed through Phase 13.

The Windows gate inherits Phase 0–12 without early packaging, then requires the Phase 13 source audit, locked Rust workspace compilation, reliability tests, deterministic fault injection and the aggregate static gate before reproducibility, packaging, signing or installer lifecycle verification may run.

## Qualification boundary

The Linux authoring environment can validate source invariants, localization parity, TypeScript/Svelte/CSS syntax and deterministic non-Windows logic, but cannot substitute for Windows SDK compilation or execute real WMI, EventLog, vendor-driver IOCTL, SCM, UAC, WiX or Authenticode behavior. `scripts/verify-phase13.ps1` on the trusted Windows build/release host remains the authoritative native gate. `-LiveReadOnlyFaultInjection` is intended for a disposable/lab Windows machine and performs read-only collector probes.
