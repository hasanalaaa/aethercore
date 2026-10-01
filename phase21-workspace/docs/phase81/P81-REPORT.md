# P81 report

P81 makes the storage evidence say which disk it belongs to and what it does not know. Four tasks on
`lane2/p81`, stacked on P80. A statement without the command that produced it is labelled a belief.
The full-CI run and the merge sha are in the PR and in the planning session's status.

## 1. Tasks

| task | commit | what changed |
|---|---|---|
| P81-01 | `62f0779` | a `PhysicalDrive` handle is used only when it reports the disk WMI described (serial, bus, length) |
| P81-02 | `b7372a8` | the NVMe health log read for what it says: spare threshold, critical-warning bits, wear above 100, 128-bit counters |
| P81-03 | `331bcbf` | coverage by bus, the drive's own failure prediction, counters compared only with the same disk's past |
| P81-04 | `2b8f6d0` | each disk is its own verdict on the Hardware page; a backup line first; what THIS disk did not report |

## 2. Evidence

Mac: UI unit tests 58, `svelte-check` 0/0, leak gate 9/9 clean and 5/5 injected with the three plant
controls, layout sweep 60/60; `cargo clippy` clean on the host and on `x86_64-pc-windows-gnu`;
`static_validate.py` no failure (one gate, `phase6_ata_is_observational_only`, first failed on a
draft of P81-03 that read the raw ATA table in the classifier; the classifier now reads a coverage flag
and the gate is unchanged); `source_seal.py` OK after every task.

PC (`aether-win`, `C:\dev\lanes\l2`): P81-01 head: `cargo fmt --check` exit 0, `cargo test` of the
telemetry crate 19 passed, the read-only live storage test passed; a temporary probe (run and removed)
showed the NVMe disk bound and its health log read (available spare 100) and the USB disk not queried
and claiming nothing. Cumulative runs at the P82 and P83 heads (which contain P81) are in the P82 and
P83 reports: `cargo test` exit 0.

## 3. Red before

| task | red |
|---|---|
| P81-01 | compile-red: `bind_handle`, `DiskIdentity` did not exist |
| P81-02 | compile-red: `spare_threshold` did not exist |
| P81-03 | compile-red: `compare_counters`, `parse_predict_failure`; the engine test `a_second_scan_says_what_changed_for_the_same_disk` |
| P81-04 | `wire-values.test.ts`: `disk-facts.ts` missing |

These are compile-reds, not behaviour-reds: the logic is new code with no earlier behaviour to fail.
`a_disk_that_stops_answering...` (P80) was green before its change.

## 4. What stayed unmeasured

- SATA behind AHCI or RAID, Storage Spaces, USB bridges and a hot-unplug during a read: no such device
  on the PC, so the binding refusal paths ran only in unit tests.
- A real SATA disk with a real predicted failure, and the previous-scan comparison across a service restart.
- The Hardware page on the owner's Windows 11 with Narrator, at 200% zoom, in Arabic.
- D7: no vendor threshold table was added (`DBT-P81-003`).

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
