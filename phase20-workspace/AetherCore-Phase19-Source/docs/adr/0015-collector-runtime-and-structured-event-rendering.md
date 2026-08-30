# ADR 0015 — Collector Runtime, Bounded Native Parsing, and Structured Event Rendering

## Status

Accepted — Phase 13.

## Context

Native maintenance software cannot assume that WMI, Event Log providers, storage miniports, firmware or vendor drivers are timely, internally consistent or well behaved. Earlier phases bounded several data paths, but storage WMI still contained unbounded enumeration waits, crash diagnostics derived system metadata from rendered XML strings, and direct storage queries lacked a shared containment model for provider hangs and malformed byte ranges.

Reliability failures at these boundaries are not equivalent to negative diagnostic evidence. A timeout, access denial, parser rejection or journal failure must be observable without turning into a false hardware verdict or taking unrelated collectors down with it.

## Decision

1. Introduce `aethercore-collector-runtime` as the common monotonic deadline, hierarchical cancellation, watchdog, panic-containment, quarantine and fault-classification layer.
2. Every long-running collector receives a finite deadline. Enumeration loops derive finite wait slices from that deadline. Parent cancellation propagates down; a timed-out child does not cancel siblings.
3. A provider that outlives its watchdog is quarantined by an `IsolationGate` held by the worker itself. No replacement worker for that gate can launch until the original exits. On success the gate is released before the result becomes visible.
4. WMI storage and System Restore verification must never use `WBEM_INFINITE`. They explicitly interpret `WBEM_S_TIMEDOUT` versus terminal `WBEM_S_FALSE` and reject impossible count/object combinations.
5. Access-denied results are preserved as `PermissionDenied`; they are not collapsed into generic provider failure.
6. Direct ATA/NVMe IOCTLs are supplemental and execute behind an independent short watchdog gate. AetherCore bounds caller latency but does not claim unsafe forced termination of arbitrary blocked vendor-driver execution.
7. Native storage responses are validated as byte buffers before semantic parsing. Offset/length arithmetic is checked, returned-byte claims must fit supplied buffers, ATA requires a complete SMART sector, and NVMe standard fields are read only from the validated standard prefix. Vendor tail data is not interpreted by the standard parser.
8. Windows Event Log metadata is obtained with `EvtCreateRenderContext(EvtRenderContextSystem)` and `EvtRender(EvtRenderEventValues)`, not XML substring parsing. User payload uses a separate user render context.
9. Event render byte sizes and property counts are capped **before allocation**, then revalidated after rendering. Embedded strings must remain inside the aligned render buffer and terminate within a bounded UTF-16 range.
10. Every event handle returned by `EvtNext` is converted immediately to RAII ownership so cancellation, malformed batches and early returns still call `EvtClose`.
11. Hardware and crash providers execute independently. Partial failures produce typed provider-fault evidence and cannot erase successful sibling evidence.
12. Fault details are bounded to 4 KiB at the shared record-construction boundary before journal or IPC transport, and ordinary UI surfaces do not expose raw detail.
13. Diagnostic snapshot serialization/SQLite persistence failure is visible: the scan is downgraded to `Partial` and records a persistence fault rather than silently claiming durable history. Worker-spawn failure becomes `Failed` rather than leaving `Collecting` stuck.
14. Unknown, unavailable or malformed evidence never produces a positive health claim or precise root-cause attribution.

## Consequences

AetherCore can return a partial diagnostic snapshot sooner instead of waiting indefinitely for a provider. Direct vendor evidence may enter quarantine after a timeout while standardized evidence remains usable. Provider faults become first-class support evidence without changing safety conclusions. Structured Event rendering removes dependence on XML formatting, while pre-allocation and pointer bounds reduce parser attack surface.

A bounded caller timeout does not imply that Windows can always abort a synchronous vendor-driver operation; the design deliberately chooses isolation and quarantine over unsafe in-process thread termination. The product may temporarily lose one optional provider until its stuck worker returns, which is preferable to unbounded worker accumulation or process corruption.

## Alternatives rejected

- **Infinite WMI/EventLog waits:** service liveness would depend on external providers.
- **Spawning a new timeout thread on every retry without quarantine:** a hung driver could create an unbounded thread leak.
- **Killing blocked threads:** unsafe in-process termination can corrupt COM, allocator or process state.
- **Continuing to scrape Event XML:** fragile formatting dependence and unnecessary text parsing at a trust boundary.
- **Allocating based only on provider-reported EventLog property counts:** malformed metadata could amplify memory use before validation.
- **Parsing vendor storage structs before validating returned byte ranges:** unsafe under truncated, contradictory or malicious driver responses.
- **Ignoring diagnostic persistence errors:** would misrepresent volatile evidence as durable history.
- **Converting missing provider evidence into a synthetic health score:** violates AetherCore's evidence-first diagnostic contract.
