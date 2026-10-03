# P83 report

P83 adds boots, network adapters, Windows Update history and client error events, and one thermal finding. What the plan
lists beyond that is recorded open in the ledger, not built. Lane 2's work is P80 to P83; a statement
without the command that produced it is labelled a belief.

## 1. Tasks

| task | commit | what changed |
|---|---|---|
| P83-01A | `2224d58` | recent boots from event 100, by field name, schema-version checked |
| P83-02 | `a836ff1`, `ec80c55`, `8703278` | network adapters as Windows reports them (local WMI, no packet) with additive wire fields; `8703278` fixes the query found invalid on the PC |
| P83-03A/B | `3162d0c` | the local Windows Update history, and an evidence-only `update-history` assessment step |
| P83-03B follow-up | Codex continuation below | bounded local Operational errors, measured event 25/version 1, with a separate translated assessment check |
| P83-05A | `be6287b`, `c6c9bb0` and the shared P80 contract | thermal zones and batteries reach the Hardware rows via the owner-scoped snapshot |
| P83-05B follow-up | Codex continuation §6 | Startup reuses dated historical boot rows; comparable-type baseline and per-application attribution remain open |
| P83-06A | `9e404fd` | `THERMAL_TRIP_EXCEEDED` for a zone at its own rated critical trip point |
| P83-04 | — | closed as deferred (D12): nothing built |

Not done and recorded open: P83-01B/05B (startup attribution and the boot baseline, `DBT-P83-006`),
the adapter counters and default routes (`DBT-P83-007`), System-channel/reboot event evidence and
update-service configuration (`DBT-P83-008`), and the battery, boot and network findings of P83-06
(`DBT-P83-005`, by design). The Operational error subset of DBT-P83-008 is implemented; that does
not close the remaining service/reboot scope.

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

## 5. Codex continuation — P83-03B (2026-10-01)

The interrupted `windows-update/src/lib.rs` parser and three tests were preserved and completed.
The native reader queries only the local Operational channel, verifies that it is enabled, reads
newest-first Error events within 30 days, and caps the subset at 200 events, each XML at 64 KiB,
and the iteration at five seconds. A separate assessment slot retains the existing bounded-worker
contract. Missing, disabled, denied, timed-out or unreadable logs are unknown, never an empty success.

Only the machine-probed provider/channel/event 25/version 1 with a named hexadecimal `errorCode`
and UTC `TimeCreated` is recognized. Other event schemas increment an unsupported count. Native
`EvtRender` double quotes and the original single-quoted machine fixture both work. Identical
(event, code, time) records are deduplicated; only four distinct newest error codes are displayed.
The small deduplication scan is quadratic under the fixed 200-event cap.

`windows-update-client-events` is separate from the installation history and has no diagnosis fact:
it proposes no repair and does not add events to installation-attempt failure counts. The existing
Repair page renders it through the existing check contract, with EN/AR title, progress label, detail
and channel label; no new wire field, online query, service start or dependency is introduced.

Red-before evidence: the quote/schema regression failed with a double-quoted record parsed as None;
the assessment test was compile-red for the missing check; the Arabic test failed on the untranslated
fallback. Fast checks: Rust system-repair/windows-update suites, 62 UI tests, native Windows GNU
cross-clippy, direct svelte-check (0 errors/warnings), static validation (349 checks), localization
self-tests (34/34). The localization self-test also found the P82 `MemoryTestResult` label gap; the
existing `hardware.memtest.title` is now reused by the semantic mapper. No gate was changed.

The pnpm wrapper refused to reinstall a node_modules symlink outside this worktree. Its existing
svelte-check binary was run directly with the same arguments; shared dependencies were preserved.
Full workspace testing initially failed four intelligence-core tests because this worktree lacked the
ignored shipped GGUF. The already-present artifact in the primary checkout was linked read-only
for the retry; no model was downloaded and no test, limit or gate was weakened.

Fresh cumulative Mac validation completed for P81/P82/P83 at the same source tree:

- `CARGO_BUILD_JOBS=4 CARGO_TARGET_DIR=../target-l2 cargo test --workspace --locked`: exit 0,
  876 passing tests across 149 suites, one ignored platform/live test. The shipped GGUF retry passed.
- `cargo test --locked` with packages hardware-telemetry, diagnostic-engine, crash-diagnostics,
  performance-telemetry, performance-bottleneck, pc-intelligence, maintenance-service, contracts,
  windows-update and system-repair: exit 0, 273 passing tests across 41 suites, one ignored live test.
- `cargo clippy -p aethercore-windows-update -p aethercore-system-repair --all-targets --locked -- -D warnings`:
  exit 0. Windows GNU cross-clippy for windows-update all targets: exit 0.
- `cargo fmt --all -- --check`, 62 UI unit tests, direct svelte-check with warning threshold and
  fail-on-warnings: exit 0, zero Svelte diagnostics.
- `static_validate.py`: 349 checks, zero failures (`parse_yaml` remains explicitly unmeasured);
  `phase12-localization-audit.py`: PASS; localization self-tests: 34/34;
  `enterprise-adversarial-audit.py`: 88 checks, zero failures; source seal: 1580 workspace files,
  7 .github files, all verified.

The old phase30/phase35 audit scripts were also executed. They fail the historical wire-freeze and
Phase35 baseline/contract checks; those phase-scoped audits are not acceptance authority for this
additive P83 tree. Their failures remain explicit and neither gates nor baselines were modified.
The inherited/inactive phase-gate archive is a coordinator-owned lane. The full Windows phase gate remains coordinator-owned. The exact-source native quick/live run
completed later, as recorded in §7.

The continuation exceeds the plan's approximate 300-line guide when parser tests, native resource
bounds, assessment wiring and translations are counted together; the native module alone is 154
lines. No safety or unknown-state handling was removed to meet a size estimate.
Not yet measured: a disabled channel on a real machine, an unsupported real schema, and a reboot
client event. System-channel evidence and service configuration remain open. Native parsing uses
Windows-rendered XML and is intentionally not a general-purpose external XML parser.

## 6. Startup history display — P83-05B continuation

The Startup page now reuses `bootRows` and `MeasurementRows` with the already owner-scoped shared
diagnostic snapshot. It shows each recorded duration with its date, source and measurement state,
plus the event window, sample count and last scan time. It explicitly calls this historical evidence:
boot type and application delays are unmeasured, so no baseline, regression or per-application time
saving is inferred. With no boot record, it says no evidence is available instead of inventing zero.
No collector, backend call, polling, dependency, protocol field or startup action was added.

The context-label/row test was red for the missing label, then green. Fast verification: 63 UI tests,
svelte-check 0 errors/warnings, static validation 349 checks with zero failures, localization 34/34.
The populated Startup fixture was visually inspected in Arabic; its historical 41.5-second sample
and scan date render. The page passed all 12 layout combinations (1280/1024/960, EN/AR, dark/light)
and its Arabic clean leak check. The first dev-server run used the default mode and showed an empty
transport; it was stopped and rerun in the same `--mode fixture` used by CI before these claims.

This closes only the history-display part of DBT-P83-006. Boot type, measured per-entry delay,
like-type baseline, startup attribution and a real-machine Narrator pass remain open.

The full CI-equivalent UI phase checks were then rerun at this UI tree: 60/60 layout combinations
(1280/960/720, EN/AR, dark/light over deepScan/repair/cleanup/startup/hardware), 9/9 clean Arabic
leak pages, 5/5 injected-sentinel pages. Startup's sample text and duration were inspected visually.
Backend source remains the fully tested P83-03B commit; this follow-up changes only UI presentation,
translations, the UI check and phase records. Final Windows full-phase/merge CI remains root-owned.

## 7. Native Windows continuation verification

At exact source commit `599e054557266355ca62750c3fde95a369610a06`, the clean detached Windows
checkout at `C:\dev\lanes\l2` was fetched from the coordinator-pushed branch. With
`CARGO_BUILD_JOBS=2` and a separate `CARGO_TARGET_DIR=C:\dev\lanes\l2\target-codex-l2`:

- `cargo fmt --all -- --check`: exit 0.
- `cargo test -p aethercore-windows-update -p aethercore-system-repair --locked`: exit 0,
  41 passed, three live tests correctly ignored (online discovery and elevated full assessment were
  not invoked). This compiled and ran the Windows-native assessment integration.
- `cargo test -p aethercore-windows-update --test live_wua --locked reads_live_client_errors_without_an_online_search -- --ignored --exact --nocapture`:
  exit 0, one local native reader smoke passed. It checks a successful bounded read, recognized event
  schemas and the 200-event bound. It does not prove a specific failure was logged or that updates
  are compliant; the test intentionally prints no personal event contents.
- `python scripts/source_seal.py`: OK, 1580/1580 workspace files and 7/7 .github files.

The final marker was `P83_WINDOWS_QUICK_PASS 599e054557266355ca62750c3fde95a369610a06`.
No full remote workspace test or invasive DISM/SFC/CHKDSK assessment ran alongside the shared CI.
The native test finished before any further source checkout change. The UI-only follow-up is
`24c4602121b5c7c5d999fde2f0599e4b90cb848e`; backend source is unchanged from the native-tested commit.
The full final-head Windows phase check and integration CI remain coordinator-owned.

Full UI negative controls also passed: planted text, attribute and sentence-argument leaks were
all detected (one positive detection each). The final tracked source tree was sealed again after
these records; no baseline, gate, timeout, dependency or pipe DACL was weakened.

### Acceptance follow-up P83-02A — bounded native counters/routes (2026-10-03)

The optional WMI adapter inventory is replaced by local GetAdaptersAddresses,
GetIfEntry2 and GetIpForwardTable2. SDK bindings were checked against the installed
pinned windows0.62.2 and Microsoft's APIs. The existing crate enables only
IpHelper/Ndis/WinSock features; package versions and Cargo.lock remain unchanged.
The approved manifest freeze hashes are refreshed, not the lock/version pins.

Acquisition retries buffer growth at most three times under1MiB, enumerates at
most128 adapters and256 unicast records each, and bounds route traversal at4096.
Bounds/cycles/errors are failures rather than invented empty success. Buffer
pointers are checked before reading; the route allocation is freed through RAII.
Only the IPv4 link-local flag is retained, not addresses/MAC/SSID. Virtual,
disconnected, disabled and no-default-route states imply no internet diagnosis.
No probe/resolve/socket/connect or adapter mutation API is called.

Cumulative six-direction counters preserve u64 and measured zero. The bounded
session cache keys GUID plus LUID, prunes disappeared/failed-read adapters, and
publishes checked deltas only across a positive monotonic interval. First samples,
non-increasing clocks, decreased/reset/wrapped counters and changed identity
produce no delta. Separate compact counter/route availability uses the common
reading timestamp/source. Unknown/absent additive data is omitted on storage
serialization and restores as unknown, not healthy.

Runtime red: measured zero traffic had no delta. A genuine existing payload gate
also failed at296060 bytes after duplicating Coverage objects; compact availability
and absent-value omission fixed it without changing the256KiB limit or original
padded fixture. The additional measured128-adapter fixture uses u64MAX and256
CJK alias characters, stays in the same budget and roundtrips exact values.
Mac66 tests (hardware+diagnostic), host all-target Clippy, Windows GNU all-target
Clippy, static351 and dependency-freeze check passed. Windows-only buffer/APIPA
fixtures compile through the cross check. Native execution, wire/render exposure
(P83-02B), capture proving no outgoing probes, IPv6/VPN/hot-unplug physical
qualification and network findings remain pending; this source checkpoint does
not close DBT-P83-007 or the phase.

Primary API sources: https://learn.microsoft.com/en-us/windows/win32/api/iphlpapi/nf-iphlpapi-getadaptersaddresses;
https://learn.microsoft.com/en-us/windows/win32/api/netioapi/nf-netioapi-getifentry2;
https://learn.microsoft.com/en-us/windows/win32/api/netioapi/nf-netioapi-getipforwardtable2.
