# P83 report

P83 adds boots, network adapters, the Windows Update history and one thermal finding. What the plan
lists beyond that is recorded open in the ledger, not built. Lane 2's work is P80 to P83; a statement
without the command that produced it is labelled a belief.

## 1. Tasks

| task | commit | what changed |
|---|---|---|
| P83-01A | `2224d58` | recent boots from event 100, by field name, schema-version checked |
| P83-02 | `a836ff1`, `ec80c55`, `8703278` | network adapters as Windows reports them (local WMI, no packet) with additive wire fields; `8703278` fixes the query found invalid on the PC |
| P83-03A/B | `3162d0c` | the local Windows Update history, and an evidence-only `update-history` assessment step |
| P83-06A | `9e404fd` | `THERMAL_TRIP_EXCEEDED` for a zone at its own rated critical trip point |
| P83-04 | — | closed as deferred (D12): nothing built |

Not done and recorded open: P83-01B/05B (startup attribution and the boot baseline, `DBT-P83-006`),
the adapter counters and default routes (`DBT-P83-007`), the `WindowsUpdateClient` event evidence
(`DBT-P83-008`), and the battery, boot and network findings of P83-06 (`DBT-P83-005`, by design).

## 2. Evidence

Mac: clippy clean on the host and on `x86_64-pc-windows-gnu` for the crates that cross-compile
(`system-repair` cannot: `libsqlite3-sys` needs a MinGW compiler), UI unit tests 61, `svelte-check` 0/0,
`static_validate.py` no failure (`phase12_arabic_windows_update_localized` first failed on my Arabic wording;
the wording was fixed, the gate unchanged), `source_seal.py` OK.
PC at `3162d0c`: `cargo fmt --check` exit 0 and `cargo test` of telemetry, engine, crash, performance,
bottleneck, pc-intelligence, service, contracts, windows-update and system-repair exit 0 (this compiles the
`system-repair` Windows code). Live on the owner's PC: 9 boots read (the newest 25 414 ms, matching
`wevtutil`), and after `0f563c5` the three real dumps link to their WER events.

Live probes on the owner's PC (temporary, run and removed): after `8703278` the network collector reads
5 adapters (Ethernet up at 1 Gbit/s with media connected, Wi-Fi down, a virtual Bluetooth adapter, two not
present); before it, 0 adapters and no fault, because the query selected two properties the class does not have
and the error was swallowed as "class not published". The local Windows Update history read: 7 entries, none
truncated, 1 unresolved failure, 0 repeated. 1 thermal zone, 0 batteries (a desktop).

## 3. Red before

Compile-red for the new modules (`boot_from_fields`, `adapter_from_wmi`, `analyze_history`,
`update_history_check`); the engine, service and UI tests were written first for their conversions. The
thermal finding's tests were written with the implementation in one step and not observed red.

## 4. What stayed unmeasured

- Network: a network capture proving no probe is sent was not made (the read is a local WMI query: belief).
- A machine with the boot channel disabled, a failed-then-succeeded update, or a real thermal trip.
- The Hardware and Repair pages on the owner's Windows 11 with Narrator and Arabic.
