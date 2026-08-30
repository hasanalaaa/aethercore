# AetherCore Native Engine Reliability Contract — Phase 13

## Purpose

Phase 13 treats Windows collectors, event providers, firmware and vendor-facing FFI as hostile reliability boundaries. A provider may stall, return a truncated buffer, expose vendor-specific data, panic inside a parser, disappear between calls, contradict its own return count, or return structurally valid data that is insufficient for a diagnosis. None of those conditions may hang the service, trigger an unsafe read, corrupt durable evidence, create an unbounded population of stuck workers, or prevent unrelated collectors from completing.

## Collector runtime

`aethercore-collector-runtime` is the shared reliability layer. `CollectorControl` carries a monotonic deadline and hierarchical cooperative `CancellationToken`. Parent cancellation propagates into every child provider; a child timeout or local cancellation does not poison siblings. Platform loops call `checkpoint` and derive finite platform waits from the remaining deadline.

`run_isolated` contains panics and adds a watchdog. `run_isolated_gated` additionally holds an `IsolationGate` in the worker itself. If a platform call outlives the caller deadline, the caller returns a typed timeout and cancels the child token, but the provider gate remains occupied until the real worker exits. A retry therefore fails fast as unavailable/quarantined rather than creating another potentially stuck thread. On normal success, the worker releases the gate **before** publishing its result so a visible success cannot race with a spurious immediately-following quarantine.

The fault taxonomy is `Timeout`, `Cancelled`, `Unavailable`, `PermissionDenied`, `MalformedResponse`, `ProviderFailure`, `Io`, and `Internal`. Provider detail is technical evidence only and is bounded to 4 KiB on a valid UTF-8 boundary before it can enter a snapshot, SQLite journal or IPC contract. The UI does not render raw provider detail in ordinary product surfaces.

## WMI discipline

Storage WMI enumeration never uses `WBEM_INFINITE`. Enumeration uses a finite slice derived from the collector deadline and has a hard 256-object inventory cap. For the one-object semisynchronous request used by AetherCore:

- `WBEM_S_TIMEDOUT` means the enumeration is still active and may legitimately carry a partial object;
- `WBEM_S_FALSE` is terminal and must not contradict the returned object slot/count;
- a success status with an impossible returned count, `returned > 1`, `returned == 1` with no object, or `returned == 0` with an object is classified as malformed provider behavior.

`E_ACCESSDENIED` / WMI access-denied results become `PermissionDenied`, not a generic provider failure. The storage collector is also isolated behind a watchdog so a hang in COM connection/query setup cannot hold the diagnostic engine indefinitely. Memory telemetry has an independent isolation boundary.

System Restore verification follows the same finite-wait and return-consistency discipline; it no longer performs an unbounded verification enumeration.

Phase 13 deliberately prefers a bounded partial snapshot to waiting without limit for a semisynchronous provider.

## Direct storage IOCTLs

NVMe and ATA direct queries are optional evidence supplements. They run behind a separate two-second gated watchdog, so one malfunctioning storage driver cannot repeatedly accumulate blocked IOCTL workers. The caller deadline is strict, but AetherCore does **not** claim that an arbitrary synchronous vendor driver can always be force-cancelled safely. If kernel I/O outlives the watchdog and ignores cooperative cancellation, the worker remains isolated and quarantined; no replacement direct-IOCTL worker is launched until the original exits.

FFI acquires bytes; pure Rust parsers decide whether those bytes are trustworthy. ATA validates the driver-reported byte count, response header, checked data offset, declared `cBufferSize`, and complete 512-byte SMART sector before parsing. NVMe validates descriptor/protocol metadata, checked offset/length arithmetic, containment in the driver-reported byte count, header overlap, and the required standard health-log prefix. Standard fields are decoded from bounded byte ranges instead of trusting native struct layout. Vendor tail bytes are tolerated but ignored by the standard parser. Vendor-defined ATA raw values stay raw evidence and never become universal health claims.

## Structured Event Log and WHEA rendering

Crash diagnostics no longer scrape rendered Event XML. The collector creates an `EvtRenderContextSystem` for system metadata and an `EvtRenderContextUser` for provider payload values, then calls `EvtRender(..., EvtRenderEventValues, ...)`. Provider, Event ID and timestamp are obtained from structured system-property positions; classification receives bounded typed/rendered values rather than XML substrings.

Event enumeration uses finite `EvtNext` slices and a maximum of 128 records. Every non-null event handle returned by a batch is immediately converted to RAII ownership **before** cancellation checks or rendering. Sparse, trailing, zero or count-inconsistent handle batches are rejected, but all retrieved handles are still closed through `EvtClose` on every exit path.

All render allocations are bounded. Before allocation, the first `EvtRender` size probe is rejected if it reports more than 256 properties or more than 64 KiB for system values / 256 KiB for user values. The second render call is revalidated against the allocation and property cap. Individual UTF-16 strings are capped at 16K code units, embedded pointers must remain inside the validated aligned render buffer, and a terminating NUL must be found within that bound. Binary, SID, GUID and array payloads are deliberately ignored by the lightweight classifier rather than guessed at.

Malformed events fail independently and become bounded `MalformedResponse` provider evidence; valid neighboring events remain usable. Event Log access denied is explicitly `PermissionDenied`.

## Minidumps

The minidump path is metadata-only and bounded to the 64 newest `.dmp` files. Only the header bytes needed for the supported dump-header layouts are read. Header structs are accessed only after signature and exact-length validation. Unknown or truncated formats remain `MetadataOnly`; they do not become invented bugcheck values or driver attribution.

Directory, metadata and per-file parse failures do not erase valid sibling dumps. A bounded number of per-file faults is retained, and `std::io::ErrorKind::PermissionDenied` is preserved as `PermissionDenied`.

## Fault isolation, durability and diagnostic state

Hardware and crash providers fan out concurrently under independent watchdog boundaries and child cancellation tokens. A panic, timeout, access denial or malformed response in one provider cannot suppress successful sibling evidence. Partial sub-provider faults from WMI, direct storage, Event Log and minidumps are preserved as typed `ProviderFault` records.

`DiagnosticsSnapshot.providerFaults` is transported through Protobuf and persisted inside the diagnostic snapshot JSON. Persistence is part of reliability: serialization or SQLite snapshot-write failure is no longer ignored. A scan that otherwise would be `Ready` is downgraded to `Partial`, receives a bounded persistence fault and localized warning, and publishes the in-memory evidence without claiming durable history was successfully recorded. Worker spawn failure likewise transitions the scan to `Failed` instead of leaving it stuck in `Collecting`.

Diagnostic cards keep the evidence-first rule: **unknown remains unknown**; unavailable or malformed evidence never becomes a fabricated positive health verdict.

## Verification

`verify-phase13.ps1` inherits the complete Phase 0–12 gate without allowing packaging to run early. It then requires the Phase 13 reliability source audit, a locked Windows Rust workspace compile, targeted reliability tests, deterministic fault injection and the aggregate Phase 0–13 static gate. Only after those pass may reproducibility, release packaging, signing and installer lifecycle verification execute.

`phase13-fault-injection.ps1` exercises watchdog timeout/cancellation, hierarchical cancellation, quarantine after a hung worker, release-before-result ordering, panic containment, fault-detail bounding, malformed/truncated ATA/NVMe buffers, NVMe vendor-tail handling, pathological EventLog property counts, structured EventLog classification and provider-fault preservation. `-LiveReadOnly` additionally runs ignored read-only probes against real Windows storage, WMI, Event Log and minidump providers on a lab machine.
