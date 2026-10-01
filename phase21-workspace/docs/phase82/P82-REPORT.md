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
