# P82 report

P82 stops the performance path from naming heat without evidence, reads ACPI thermal zones and
batteries, decodes WHEA from its typed record, shows Windows Memory Diagnostic's dated result, and
relates a crash's dump to the System log. Six tasks on `lane2/p82`, stacked on P81. A statement without
the command that produced it is labelled a belief.

## 1. Tasks

| task | commit | what changed |
|---|---|---|
| P82-01 | `ed1413f` | an idle clock is not a throttle; a limit of unknown cause is `FREQUENCY_LIMIT_OBSERVED`, not `THERMAL_CLAMP` |
| P82-02A | `be6287b` | ACPI thermal zones (`MSAcpi_ThermalZoneTemperature`) with plausibility and reasons |
| P82-02B | `c6c9bb0` | batteries via `GUID_DEVICE_BATTERY` and read-only IOCTLs, absolute units only |
| P82-03A | `13c960e` | WHEA category from the CPER record's typed sections, never from translated words |
| P82-03B | `2a11dea` | Windows Memory Diagnostic's last result, dated; "not tested" is not a pass |
| P82-04 | `a058f25`, `0f563c5` | one crash is one incident; `0f563c5` fixes it against the real event shape |

## 2. Evidence

Mac: `cargo clippy -D warnings` clean on the host and on `x86_64-pc-windows-gnu` for every changed
crate, UI unit tests 60, `svelte-check` 0/0, leak gate 9/9 and 5/5 with the plant controls, layout sweep 60/60,
`static_validate.py` no failure (`phase13_structured_whea_classifier` pins a test name and a parameter name:
the name was kept with the new meaning and the gate is unchanged), `source_seal.py` OK.

PC: cumulative `cargo test` (telemetry, engine, crash, performance, bottleneck, pc-intelligence,
service, contracts) exit 0 with `cargo fmt --check` exit 0, `svelte-check` 0/0 and 60 UI tests at `2a11dea`.
Live probes (temporary, run and removed) on the owner's PC: 1 thermal zone read (28 °C, rated critical
105 °C, measured), 0 batteries (a desktop: normal), 4 WER events and 3 minidumps. The probe found what
P82-04 had wrong: the event's first value is `0x0000000a (p1, p2, p3, p4)` (the code is its leading
token) and the dump record holds the file name only while the event holds the full path; before
`0f563c5` all three dumps were unlinked and every code empty, after it all three link with matching codes.

## 3. Red before

| task | red |
|---|---|
| P82-01 | `a_limit_of_unknown_cause_is_not_called_thermal` failed (the old rule reported `THERMAL_CLAMP`); the parser tests are new code |
| P82-02A / 02B | compile-red (`thermal.rs`, `battery.rs`) |
| P82-03A | compile-red (`whea.rs`); the two tests that pinned word matching were replaced by typed ones |
| P82-03B | `wire-values.test.ts` (module missing) |
| P82-04 | compile-red; the follow-up's test uses the real shapes read on the PC |

## 4. What stayed unmeasured

- A machine with several thermal zones, with a real frequency limit, or with a laptop battery: the battery
  IOCTL code (`unsafe`) was type-checked and clippied only, never run against a battery.
- A real WHEA event: the CPER offsets and section GUIDs are from the published specification; that the
  first binary payload of the event is the CPER is inferred (`DBT-P82-004`).
- That Windows Memory Diagnostic's ids 1201/1202 mean "no errors" / "errors" is community-documented, not
  in Microsoft's public documentation (`DBT-P82-005`).
- D7 (fan, VRM, CPU hotspot), D9 (debugger, symbols) and D8 (no launcher) are not built.

## 5. Cumulative continuation verification (2026-10-01)

P81/P82/P83's combined source tree was rerun locally during the P83-03B continuation:
`cargo test --workspace --locked` passed 876 tests across 149 suites (one ignored platform/live test).
The targeted cumulative telemetry/engine/crash/performance/bottleneck/intelligence/service/contracts/
windows-update/system-repair run passed 273 tests across 41 suites (one ignored live test).
UI tests: 62 passed; direct svelte-check: zero errors and warnings; static validation: 349 checks with
zero failures (`parse_yaml` unmeasured); enterprise audit: 88/88; localization self-tests: 34/34.
The P82 `MemoryTestResult` semantic label gap found by localization was fixed using the existing
translated memory-diagnostic title. Source seal verified the complete tracked tree.

These results supplement the original per-task evidence; Mac tests do not execute native Windows
storage/battery/event code. Exact combined-head Windows validation and merge CI remain coordinator
owned. The historical phase30/35 audits still fail their old wire/baseline checks; they were not
weakened or re-baselined. The remaining hardware and accessibility gaps in §4 remain open.


## Acceptance continuation — P82-02B relative capacity (2026-10-03)

A battery reporting BATTERY_CAPACITY_RELATIVE previously lost its measured raw
capacities and was marked unsupported. The typed Battery now retains separate
optional u32 design/full capacities in undefined relative units, with absent
fields omitted and legacy deserialization defaulting to absent. The absolute
mWh fields remain absent for relative packs. A zero design/full remains absent;
two missing capacities remain unsupported. Cycle semantics and native device
acquisition are unchanged; there is no division, guessed percentage or damage
verdict. Absolute batteries preserve their existing capacities and units.

Microsoft BATTERY_INFORMATION specifies relative units when0x40000000 is set;
see https://learn.microsoft.com/en-us/windows/win32/power/battery-information-str.
No conversion into mWh is defined. Additive BatteryInfo tags10–13 preserve
relative values and their presence independently across the actual service JSON
bridge. u32 fits JavaScript's exact integer range; UI still rejects noninteger,
negative, overflow, zero or conflicting absolute/relative capacity payloads.
Owned EN/AR rows show separate design/full relative values, cycle count, source
and recorded time. No battery record is invented on desktops; no batteries are
combined or averaged. The populated Hardware fixture contains both absolute
and relative packs to exercise the actual page.

Runtime red: relative raw capacities had Unsupported instead of Measured.
Actual MeasurementRows EN/AR SSR4 failed for missing relative values and an
ambiguous mixed-unit payload exposing mWh. The implementation made these green.
Persistence JSON roundtrip preserves relative values without acquiring mWh;
legacy payload restores absent values. Real wire conversion verifies values and
both missing presence flags. The affected four-package Rust run passed166 tests
across13 suites; all-target host Clippy and hardware Windows GNU Clippy with
warnings denied passed. UI103 tests passed, check0 errors/0 warnings, build and
static351 passed; dependency freeze approved without manifest/version changes.
The build retains its pre-existing large-chunk warning. Populated Hardware layout8/8
(1280/640, EN/AR, dark/light) and Arabic clean leak4/4 passed. An initial layout
run lost its Chrome target during navigation; the stable-source rerun passed
without changing assertions, gate budgets or tools.

Native final-head execution and physical relative/multiple-battery/tag-change
qualification remain NOT RUN. Current direct SSH is unavailable; Windows-native
integration CI remains coordinator-owned. The earlier7b2c994 read-only collector
receipt observed0 batteries on a desktop, so it does not qualify relative packs.
This closes the approved relative-unit source loss, not all physical P82 claims.
