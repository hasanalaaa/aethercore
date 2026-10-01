# P84 report

P84 made the driver cycle honest about what it knows and careful about what it touches. The page
reads problem codes by their meaning and never calls a guess a Windows fact. It searches online only
when the user confirms it, and a search always ends. An install counts as done only when the
approved driver is what Windows binds afterwards. It starts only on protection that re-proves
itself, and recovery tells the reader what they have and how to go back by hand.

This was lane 3 of three parallel lanes (ASTRA-PLAN §8), on one phase branch, `lane3/p84`. It
started from `e685b14` and merges by fast-forward only. A statement below without the command or
run that produced it is labelled a belief.

**No driver was installed, rolled back or changed on any machine.** Install-path behaviour is proven
with fakes (`crates/driver-install/tests/coordinator.rs`). On the owner's PC I ran only read-only
probes: a local-cache WUA search, the agent's last-search date, and a `pnputil /export-driver` of one
bound package into a temporary folder, which changes nothing on the system.

## 1. Tasks

| task | commits (red → fix → seal) | code lines (+/−, tests included) |
|---|---|---|
| P84-01 problem codes by meaning, title versions named, no downgrade offer | `de06531` → `bcdf30a` → `17794a6` | +144 / −24 |
| P84-02A a scan searches online only when confirmed, for that scan (D14) | `debe056` → `12c1a69` → `7bc58f7` | +105 / −13 |
| P84-02B a bounded, abortable search; the page's own confirmed online button; where a scan looked (D27) | `3b9031c` → `645b6a3`, `68e2d63`, `dc3a740`, `047961a`, `2270286` → `34a4c9e` | +463 / −15 |
| P84-03 verified by the driver bound afterwards | `4f0e40a` → `70422d7` → `3613574` | +186 / −17 |
| P84-04 protection proven, not assumed | `c8aebdf` → `3eb177f` → `65a962f` | +465 / −122 |
| P84-05 recovery says what exists; the Catalog by hand (D17) | `a5c8676` → `98bb8bd`, `46326be` → `2900aed` | +75 / −4 |

Two tasks passed the ~300-line guide:
- **P84-02B:** about half its lines are tests (the bounded runner's three unit tests, a source guard and a live probe) and EN/AR strings.
- **P84-04:** about 280 lines are tests. The fake platform now writes and seals a real export instead of returning made-up evidence, and five existing test literals became `..fake()` with no assertion changed.

Neither split into a third task, because each half is only complete with the other.

Ledger: `DBT-P84-001` to `DBT-P84-005` and `DBT-P84-007` are closed; `DBT-P84-006` is open.

## 2. Red before, per task

Each test was shown failing for its stated reason on the tree before its fix, and passing after.

| task | failing before the fix | why |
|---|---|---|
| P84-01 | driver-hub `p84_01_a_version_read_from_the_offer_title_says_so`, `p84_01_an_offer_not_newer_than_the_installed_driver_is_not_recommended`, `p84_01_a_disabled_device_is_not_urged_to_update`; `wire-values.test.ts` "driver problem codes each read by their meaning" | `"WuaMetadata"` for a title guess; a not-newer offer `recommended`; a code-22 offer `selected_by_default`; `'Problem code 22'` (PC run) |
| P84-02A | driver-hub `only_a_scan_confirmed_online_searches_online_and_only_that_scan` | recorded `[Online]` where the local cache was asked (the scope was threaded through but still ignored, then honoured) |
| P84-02B | `windows-update/tests/search_is_bounded.rs`; `wire-values.test.ts` "driver scan source" | "the driver search does not call search_bounded("; `driverSearchSource is not a function`. The `bounded.rs` unit tests are new with the module. |
| P84-03 | `an_install_is_verified_by_the_driver_bound_after_it_not_by_the_result_code` | "the old driver is still bound: … left Completed right Failed" |
| P84-04 | `protection_that_cannot_be_proven_stops_the_install_before_any_mutation` | a non-new restore point read Completed; with only the freshness fix, a drifted export read Completed |
| P84-05 | `wire-values.test.ts` "driver recovery entries name their restore point, their export, and the manual way back" | `driverRecoveryMeans is not a function` |

Some tests passed on the existing code. They pin behaviour I read but did not change; they are not red-before:
- P84-02A's router mapping test: the proto field was new.
- P84-04's "System Restore turned off" case.
- `an_install_interrupted_after_the_barrier_is_recovery_required_and_not_replayed`: `recover_incomplete` existed with no driver test.

## 3. Fast local check, per task

All of these ran on the PC in `C:\dev\lanes\l3`, with `CARGO_TARGET_DIR` set to that worktree and `CARGO_BUILD_JOBS=8`, unless marked Mac.

- **P84-01, `17794a6`:**
  - `cargo fmt --check` 0.
  - `cargo test --locked -p` driver-authority, driver-hub, driver-install, driver-acquisition, pc-intelligence and maintenance-service: all ok.
  - svelte-check 0; UI unit tests 56/56; `static_validate.py` ok (349); `source_seal.py` OK.
  - Mac leak gate: clean 9/9, inject 5/5, plants text/attr/arg found, sweep 60/60, drivers en/ar 4/4.
- **P84-02A, `7bc58f7`:**
  - fmt 0; the crates above plus contracts and windows-update: ok.
  - `cargo check -p aethercore-desktop` ok; `SetCanAutomaticallyUpgradeService` compiles on Windows.
- **P84-02B, `34a4c9e` (and `dc3a740` for the Rust crates):**
  - fmt 0; windows-update tests ok.
  - Live local-cache probe (`live_wua.rs --ignored`): 3 offers, 0 warnings, 2243 ms; Windows last searched online on 2026-09-30.
  - UI unit tests 57/57; seal OK; static ok (Mac).
  - Mac: `cargo check`/`clippy --target x86_64-pc-windows-gnu -p aethercore-windows-update` clean while the PC's SSH was down.
  - Mac leak gate: clean, inject and plants pass; sweep 60/60.
  - Browser (fixture, Arabic): the confirmation renders, and Cancel closes it with no call.
- **P84-03, `3613574`:** fmt 0; the driver crates and the service: 0 failures; UI unit tests 57/57; seal OK.
- **P84-04, `65a962f`:**
  - fmt 0; driver-backup, restore-point, driver-install, driver-hub, maintenance-service and pc-intelligence: 0 failures.
  - Live `pnputil` export probe: `oem10.inf`, 3 files, 7,351,873 bytes, verified on re-read.
  - UI unit tests 57/57; seal OK.
- **P84-05, `46326be`:**
  - svelte-check 0; UI unit tests 58/58; seal OK.
  - `static_validate.py` ok after it caught an `https://` in the UI (`phase7_no_remote_ui_assets`); the address now carries no scheme.
  - Mac leak gate as above, plus activity and drivers 2/2.
  - Browser: the Arabic recovery row reads restore point #43, the export folder, and the manual route; a cleanup entry shows none of it.

## 4. Decisions applied

### Execution recovery: preserve the original backup seal

The recovery review found that rewriting an exported file and its hash inside the mutable
manifest could replace the original protection evidence. The regression
`rewriting_the_export_and_its_manifest_does_not_reseal_the_original_evidence` failed on the
original implementation (one failed test, exit 101). `seal_export` now captures SHA-256 of the
exact manifest bytes in its returned evidence; `verify_export` compares those bytes with that
captured hash before parsing or checking the directory. Older evidence without the hash
deserializes but fails closed. The existing unchanged-export test also checks that legacy case.
The install coordinator receives this evidence directly from the platform export and verifies
it before its mutation barrier. An independent read-only review traced that caller and the
checkpoint serialization; no new wire field, dependency or live driver install was needed.
The five-package Mac check (`driver-backup`, `driver-install`, `driver-hub`,
`maintenance-service`, `pc-intelligence`, locked, jobs=2) passed after the fix.
Full CI at the integration head remains the merge gate; its receipt belongs to the PR.

- **D2 and D14.** The empty request reads the local cache. Online requires a confirmation that names Microsoft's service or the managed server, and holds for one scan. The router binds the scope to the caller's own request, so no confirmation is stored or reused. The install-time WUA access after approval is unchanged.
- **D13.** No `ProblemStatus` wire field was needed.
- **D15.** A downgrade is never recommended or selectable. The consent path for a deliberate downgrade was **not built**: it is closed as deferred, because no recommendation makes it part of this release.
- **D16.** No safety tier changed and there is no bypass.
- **D17.** No automatic rollback and no Catalog importer. Recovery is guidance plus the preserved evidence. Both are closed as deferred.
- **D27.** `DriverHubSnapshot` gained `search_scope` (14) and `windows_last_online_search` (15), both additive, as the scan's coverage indicator.
- **D0.** Additive fields only, on reviewed tags, bindings through `build.rs`. No oneof tag was needed from the 150–179 range.

## 5. What stayed unmeasured

- **The abortable search job in the service's context (LocalSystem).** From an SSH session the agent refuses `BeginSearch`: `0x80070005` with or without process COM security at impersonate level, and `0x80004003` with a null callback. The plain search runs instead. In the service, which sets the same COM security at startup, I believe the job runs, but I did not measure it without touching the installed service (`DBT-P84-006`).
- **A free-disk-space check before the export.** There is none; a full disk surfaces as a `pnputil` or file-system error and stops the install. Nor did I check the reparse state of the folders above the backup root (the root and its tree are checked).
- **Any real install, verification or recovery.** Proven with fakes only, by the lane's rule.
- **The pages with Narrator, on a real install.**

## 6. Merge

Before the pull request, the phase branch is rebased on the latest `main` and re-sealed. It merges by
fast-forward only, once full CI is green at the exact head, so the `main` sha is this report's own
commit. The PR records the run.
