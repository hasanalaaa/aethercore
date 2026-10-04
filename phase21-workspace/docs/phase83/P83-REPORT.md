# P83 report

P83 reads boots, bounded local network counters/routes and Windows Update history/client errors.
It publishes typed thermal, absolute battery-capacity and network-window observations. The initial
implementation receipts below remain historical; acceptance continuations distinguish implemented
source from exact native evidence and physical qualification. Boot attribution/comparable baseline
and the remaining WU service/System scope stay open until their source and evidence are verified.

## 1. Tasks

| task | commit | what changed |
|---|---|---|
| P83-01A | `2224d58` | recent boots from event 100, by field name, schema-version checked |
| P83-02 | initial WMI receipts below; `7b2c994`, `a6ee7ea` | bounded IP Helper counters/routes, checked monotonic deltas, exact decimal transport and EN/AR display; native7b2c994 tested, later final-head CI and physical matrix pending |
| P83-03A/B | `3162d0c` | the local Windows Update history, and an evidence-only `update-history` assessment step |
| P83-03B follow-up | Codex continuation below | bounded local Operational errors, measured event 25/version 1, with a separate translated assessment check |
| P83-05A | `be6287b`, `c6c9bb0`, `ed89a0a` and P80 | owner-scoped thermal and absolute/relative battery rows with recorded time and distinct missing states |
| P83-05B follow-up | Codex continuation §6 | Startup reuses dated historical boot rows; comparable-type baseline and per-application attribution remain open |
| P83-06A | `9e404fd`; battery continuation below | thermal trip at firmware threshold; absolute capacity interpretation is informational, not a damage verdict |
| P83-06B | `d64484a`; boot pending | measured network error/discard window is informational; reset/down/missing/stale data cannot invent a rate or resolve an old observation |
| P83-04 | — | closed as deferred (D12): nothing built |

Remaining source gaps: P83-01B/05B startup attribution/comparable baseline (DBT-P83-006),
WU service/System/reboot scope (DBT-P83-008, lane2/coordinator continuation), and comparable
boot findings (DBT-P83-005). Counter/default-route and battery/network interpretation source
are implemented by the acceptance continuations; their final native/physical qualification remains
explicitly pending. Operational WU errors alone do not close the remaining update scope.

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


### Exact native P83-02A checkpoint and Task B transport/render (2026-10-03)

Native source was clean detached `7b2c994d9531aba9ea1e95825445b6bd9caacd94`
in the isolated `C:\dev\codex-care-fixtures` checkout, without a source overlay.
Windows 11 Pro build26200, jobs2, lane3-owned target; workspace1615 and GitHub7
source hashes verified. Hardware fixtures40 passed, one live test ignored by the
ordinary suite. The separately authorized existing read-only collector passed1;
all-target Clippy with warnings denied passed. First/second samples each had2
storage devices,1 thermal zone,0 batteries,6 adapters and6 cumulative readings;
deltas0 then6, IPv4 defaults2, IPv6 defaults0, faults0. Only aggregate counts were
printed. No app/service/model action, repair, cleanup or installation was invoked.

Native raw receipts (kept outside the sealed workspace):

| receipt | SHA256 |
|---|---|
| `/private/tmp/p83-network-native-fixtures-7b2c994.log` | `999a11b614b7e8c9e8300893e78a3b663b05b8d383ea5124ce33ba2c5d77adb7` |
| `/private/tmp/p83-network-native-live-7b2c994.log` | `92c0c81709955f8c2870f0f295ab4b330c9c6137a84cc460828fefe6294879e4` |
| `/private/tmp/p83-network-native-clippy-7b2c994.log` | `f072560d65b4ff6c22421e47bec19f73ff4e8a5d8b64b935a76bf59cce746f9b` |

Task B adds NetworkAdapterInfo tags12–23, presence/value flags for admin/APIPA
and IPv4/IPv6 default routes, separate availability, and typed cumulative/delta
messages. All six counters and elapsed milliseconds cross JSON as canonical
unsigned decimal strings; no JavaScript Number conversion occurs. UI BigInt
validation rejects malformed/overflow strings and nonpositive delta intervals.
Measured zero remains measured zero. The service wire fixture preserves u64MAX
and zero; the actual128-adapter JSON wire fixture stays below the existing256KiB
budget, in addition to the original full-domain and measured source-budget tests.

Actual MeasurementRows SSR in EN/AR displays owned cumulative and interval
counts, source and the recorded reading date. The interval is a measured sample
window, not an internet throughput claim. Unknown/failed reads, first samples,
missing routes and invalid data show owned limitations. A missing observation
date is omitted, never replaced by the current time; older dated samples are
historical readings, never asserted current/fresh. An IPv6 default avoids the
no-default label; a disabled/virtual/down adapter or missing default route never
becomes an internet or Critical verdict. Legacy additive fields may be absent.
Native malformed unicast data now propagates its existing failure instead of
silently losing the APIPA flag. No new probe, dependency/version, budget, schema
tag reuse, mutation or gate change was introduced.

Runtime red before implementation: service exact-counter assertion1 failed
(null instead of u64MAX decimal); actual EN/AR SSR4 failed for missing readings
and owned limitations. Both became green. Final UI99/99 passed (including two
recorded-date/unknown regressions); check0 errors/0 warnings and build passed.
Affected Rust162 tests across13 suites passed before the additional actual wire
budget fixture; the new fixture and final service suite passed afterward.
Hardware Windows GNU all-target Clippy and affected host all-target Clippy passed;
static351 passed. Populated Hardware layout8/8 (1280/640, EN/AR, dark/light)
passed with no overflow/clips/overlap. Arabic clean leak4/4 passed; a planted
sentinel remains detectable. The build retains its existing chunk-size warning.

The Task B native final-head test remains distinct from the earlier7b2c994
native receipt: direct SSH currently times out, while root reports the Windows
Actions runner online. Final native source/CI qualification belongs to the
coordinator; no earlier receipt is relabelled as testing this changed head.
Physical IPv6-only/VPN-only/hot-unplug, packet-capture no-probe proof, battery
hardware, Narrator and200% Arabic qualification remain NOT RUN. This closes the
source transport/render portion of DBT-P83-007, not the entire P83 phase.
P83-01B/05B comparable boot baseline and per-entry attribution, P83-03 service/
reboot/System evidence, P83-06 typed network/boot/battery rules and P82 relative
battery units remain separate approved source tasks. A real network finding
requires a valid measured delta and source/time evidence; missing first-sample
or reset data cannot be treated as healthy or a diagnosed fault.


### P83-06B network rule — measured window observation (2026-10-03)

NetworkCounterWindow is an additive typed fact in the existing fact model, not
an opaque payload or a new rule engine. It is normalized only from measured
counter coverage on an enabled, operational adapter, with a valid identity,
source, positive interval and a real observation timestamp no later than now.
First/reset/missing/failed/down/disabled data is absent evidence. The existing
five-minute live-reading window makes old observations stale. Cumulative
lifetime totals, routes, APIPA, cable and virtual status do not enter this rule.

A positive error/discard delta emits NETWORK_COUNTER_ERRORS_OBSERVED at
Informational severity only. Owned EN/AR text carries the exact directional
counts and measured millisecond interval. Source, resource and time are cited;
limits expressly exclude internet outage, component failure or causal diagnosis.
There is no remediation candidate or automatic action. Virtual/VPN readings
are observations with the same limits, never Critical. Transient sampling
windows are excluded from durable machine-state fingerprints.

Resolution requires the same resource, a current valid positive zero-error/
zero-discard window and Completed diagnostics authority. Absence, cancellation,
partial scans and stale/historical zero windows do not resolve the old finding.
A completed scan without comparable readings leaves it NotRechecked. This is
not a general network health verdict or proof of connectivity.

Runtime red: the actual diagnostics-normalize-evaluate path omitted the
measured window finding. Green verification includes normalized source guards,
exact u64MAX finding JSON roundtrip, stale/historical and zero-interval scenarios,
no mutation candidates, persisted resolution controls and the real FindingCard
EN/AR source-to-render path. Targeted pc-intelligence+service tests150 across9
suites passed; all-target Clippy with warnings denied passed after correcting a
new test's unnecessary clone. UI105 passed; static351 passed; check0 errors/0 warnings and build passed.
Populated DeepScan layout8/8 (1280/640, EN/AR, dark/light) passed with the
new exact-count card; Arabic clean leak2/2 passed. A navigation-target disconnect on the initial run was
retested at stable source; no assertions or time budgets changed. Native final-head
verification remains coordinator-owned because direct SSH is unavailable.
Boot same-type baseline/attribution and battery interpretation are separate
approved tasks; no guessed boot classification, impact or health percentage
was added here. WU reboot/source review is now lane2/coordinator-owned.

### General WUA reboot preflight source correction

`ensure_servicing_available` now reads the existing ISystemInformation
RebootRequired property after IsBusy and before the installer-specific preinstall
flag. True preserves the existing RebootPending result; query failure preserves
Wua error/Unknown through existing system-repair callers. Seven actual-function
COM controls changed from three failures to all passing;19 crate tests and host/
Windows GNU all-target Clippy passed. Native/live verification remains pending.
This read-only flag concerns reboot needed to finish update installation or
uninstallation, as specified by [Microsoft](https://learn.microsoft.com/en-us/windows/win32/api/wuapi/nf-wuapi-isysteminformation-get_rebootrequired); it makes no third-party reboot claim.

The integrated Windows CI additionally captures provider event metadata for
Diagnostics-Performance100–103 and Kernel-Boot. This reads publisher schemas,
versions and templates only, with no event-user data, subscription, service
change or product action. Missing providers remain explicitly unmeasured. The
metadata artifact is navigation/semantic evidence for the pending boot work,
not a boot baseline/attribution or full-phase acceptance pass.

### P83-06A battery interpretation and raw-inspector precision proof

BatteryCapacity is an additive typed fact for one measured battery with two
positive absolute mWh capacities. Relative/mixed units, missing/zero capacities,
failed coverage and missing/future time are excluded. The recorded five-minute
live-source freshness window prevents old readings from raising a current
interpretation. Full-charge capacity below design emits only the Informational
BATTERY_CAPACITY_BELOW_DESIGN observation, with the exact two measured values
and an estimated loss truncated to0.1%. u128 arithmetic prevents overflow at
u64MAX; division by zero is excluded. Capacity variation/calibration and unknown
cause are owned EN/AR limits, not a Critical battery alarm, health certificate,
auto action or repair candidate. Relative-unit packs acquire no percentage.

The same resource needs a current valid full/design remeasurement and Completed
diagnostics authority to resolve this capacity observation. Missing, partial,
zero-design or stale evidence is not a resolution proof. This resolves only the
below-design observation; it does not prove overall battery health. Volatile
capacity interpretations do not enter the durable machine-state fingerprint.

Runtime red: the actual normalized absolute capacities produced no capacity
interpretation. Green checks cover relative/mixed/zero/missing/failed/old inputs,
u64MAX exact arithmetic and finding JSON roundtrip, no automatic remediation,
persisted resolution guards and actual FindingCard EN/AR rendering. Targeted
pc-intelligence+service154 tests across9 suites passed, all-target Clippy with
warnings denied passed, UI107 passed, check0 errors/warnings and build passed,
static351 passed, dependency freeze approved. The populated two-observation
DeepScan layout8/8 (1280/640, EN/AR, dark/light) passed. Arabic clean leak2/2 also passed. The first browser run
lost its navigation target; the stable-source retry passed with unchanged
assertions/time limits. Native final-head and real battery qualification remain
NOT RUN; the coordinator's Windows CI is the native route while SSH is down.

Precision review also traced the raw evidence inspector. There is no SystemFact
or FactPayload transport in intelligence.proto or the UI contracts. DeepScan
exposes facts_count and findings; PcMessageArg.value and PcEvidenceRef.
technical_value are strings. The coordinator serializes raw facts only for a
byte-size estimate; persistence stores DeepScanSnapshot without raw fact
payloads. protocol.rs copies argument/evidence strings into the real snapshot
wire. DeepScanPage's inspector joins technicalValue strings without Number
conversion. An additional actual deep_scan_snapshot_proto→JSON test passed,
asserting u64MAX exact strings for interval/error arguments and raw technical
value, and absence of a raw facts/payload field. Final service all-target Clippy
passed after this test. No speculative serde format migration was necessary.

The source battery/network interpretation scope is implemented, while physical
qualification and P83-06B's comparable boot baseline remain distinct pending
work. Boot publisher metadata capture and WU reboot source are coordinator/
lane2-owned; this checkpoint adds neither guessed boot types nor event schemas.

## Measurement boundary corrections — 2026-10-03

Independent review of source head `3ecacc3` found four measurement defects; that head is not
promoted merely because its existing tests pass. Two actual normalization/rule controls first
failed: a missing observation timestamp was treated as current, and firmware readings 3731 < 3732
decikelvin rounded to the same displayed Celsius value and raised a false thermal trip.

The collector now retains both exact firmware readings alongside the existing rounded display.
Normalization requires measured coverage, a positive nonfuture observation, source/zone identity
and plausible exact readings. Legacy rounded-only samples remain displayable but cannot raise or
resolve a thermal finding. The rule and fingerprint compare exact units, with the exact readings
kept in technical evidence. Missing, failed, future and unidentified samples are rejected.

Battery enumeration now accepts only `ERROR_NO_MORE_ITEMS` as a successful end; every other
native error reaches the existing collector-fault path, including errors after partial inventory.
This follows the [SetupDiEnumDeviceInterfaces contract](https://learn.microsoft.com/en-us/windows/win32/api/setupapi/nf-setupapi-setupdienumdeviceinterfaces).
Native boundary controls cover successful continuation, normal end and denial/data/buffer failures.
The network collector records each counter read's own monotonic and wall-clock observation time;
variable delays reading later adapters no longer stretch the first adapter's interval. The actual
shared counter attachment control uses 1000 ms and 1500 ms windows in the same inventory.

Quick source validation: 241 affected Rust tests passed before the final counter control, the
counter controls and host/Windows GNU all-target bindings are checked separately. The fully measured
snapshot still fits its unchanged byte budget. Native Windows execution of this final correction,
physical battery devices, firmware thresholds and no-egress capture remain pending; cross-compilation
is not native qualification. Raw red/green receipts are retained externally under
`/tmp/aethercore-p83-thermal-*`, `measurement-guards-*`, `counter-time-*` and
`measurement-native-cross-clippy.log` with the full `aethercore-p83-` prefix.


## Both update-event channels and dependency state — 2026-10-04

The owner's Windows 11 build 26200 publisher metadata qualifies System event20/versions0–1,
Error level2 with named hexadecimal errorCode, and event21/version0, Informational level4 with
named updatelist. Operational event25/versions0–1 remains discovery-error evidence. The metadata
and event-level probes are read-only; their raw local receipts remain outside the delivered tree.
Unsupported provider/channel/id/version/fields, conflicting duplicates and invalid dates stay unknown.

The existing reader now reads both channels within one five-second deadline and200-record total,
splitting the cap so a busy Operational log cannot starve System. Disabled, denied, unavailable or
late reads explicitly reduce coverage; an available channel retains its own evidence. Historical
restart notifications are dated log observations, never the current WUA reboot-required flag, and
never installation attempts. The evidence-only assessment has no repair fact and the owned EN/AR
summary identifies both channels, incomplete coverage and the historical restart limit.

The same assessment queries state/configuration for wuauserv, BITS and TrustedInstaller without
starting them. A shared existing ProviderSlot bounds each observer, retains the actual worker and
read-budget lease after timeout/cancel, and refuses another service probe while the worker remains.
Only exact SERVICE_STOPPED can feed the existing diagnosis-scoped service-start gate; pending,
paused and unknown states no longer masquerade as stopped. Demand-start idle requires no repair,
disabled policy remains unchanged, and the additional dependency evidence ids cannot propose actions.

Actual System-error parser control failed on the old source before this correction. The old shared
service verdict also reproduced ServiceStopped for a pending state; the corrected controls cover
all six other SCM states. Both packages'73 affected tests, all-target host Clippy and Windows GNU
update bindings passed; the actual two-channel controller has3 positive controls plus2 deliberately
broken controls which fail their intended assertions. Existing7 actual WUA preflight controls pass.
UI119 controls and strict Svelte check (zero errors/warnings) passed before the final source-only
bounded-SCM wiring. Source and cross-bindings checks do not qualify actual final Windows execution.
No update search, service start/configuration, channel activation, dependency or wire change.

Raw receipts: /tmp/aethercore-p83-wu-system-red.log,
/tmp/aethercore-p83-wu-pending-service-runtime-red.log,
/tmp/aethercore-p83-wu-system-services-green.log,
/tmp/aethercore-p83-wu-actual-event-flow.log,
/tmp/aethercore-p83-wu-final-host-clippy.log,
/tmp/aethercore-p83-wu-final-cross-clippy.log,
/tmp/aethercore-integrated-ui-2026-10-04.log.
