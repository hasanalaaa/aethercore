# P79 report (lane 1)

P79 made One-Click Care preparable and understandable: the owner can ask Care to prepare what it could
run, sees what would be deleted and what was left out before approving, and the history of a care run
reads truthfully on the timeline. Six tasks plus the lane's step 0, on one phase branch, verified by the
fast local checks on the owner's PC and by one full CI at the pushed head, then fast-forwarded to `main`.
A statement below without the command or run that produced it is labelled a belief.

The phase PR, its head, its CI run and the merge are recorded in the PR description and in the ledger
(`DBT-P79-002`): a report cannot name the SHA of the commit that contains it.

## 1. Step 0 (its own PR, merged first)

`ci: a push to main skips the windows job when its SHA already passed it as a pull request`
(`DBT-P79-001`): a `tested` job proves, through the Actions API, that a pull request of this repository
already passed `windows` for exactly this SHA; only then a push to `main` skips the job, and the aggregate
`CI required` accepts that one skip. Fail-closed on every doubt. Its PR carries its own evidence, and the
first fast-forward push after it is the verification on `main` (recorded in the ledger row).

## 2. The tasks

| task | commit | what |
|---|---|---|
| P79-01 | `319fe8b` | `main` already fixed the newest-page bug (P76, `DBT-P76-006`); paging pinned by tests |
| P79-02 | `eb13116` | a cancelled care run is neutral; a care run is not a recurring fault; ingestion pinned |
| P79-03 | `4df8ac0` | an unread care plan is `unavailable` in the UI; the over-cap Rust half was done by lane 4 (P87-03) |
| P79-04A | `767cb11`, `611746e` | `PrepareCarePreview` (decision D5), then the three service gates it failed |
| P79-04B | `cc015ff` | the Care button prepares, waits without polling, shows what would be deleted and how much |

### Checked `main` first
- **P79-01** and the first half of **P79-02** were already on `main` (P76 `care-truth`): `before_sequence = 0`
  as the newest page, and `care_run` mapped to a timeline event. Only tests were missing for the first;
  the second had two real gaps, found by writing the acceptance tests first.
- **P79-03**'s core defect (a plan that cannot be built becoming "nothing due") was fixed by lane 4 in
  `P87-03` while this phase ran (`CareError::SourceLimit`); I dropped my Rust change in the rebase.

## 3. Red before, per task (each shown when it was made)

| task | red |
|---|---|
| P79-01 | the three new paging tests fail when `0 => timeline.events.len()` is put back to `0 => 0` |
| P79-02 | on `main`: a cancelled run was `Failed` (`a cancelled run is neutral`), and four failed runs made the pattern `oneClickCare`/`care.run:Failed` |
| P79-03 | `careEmptyReason(scan, 'failed')` answered `noScan`, not `unavailable` |
| P79-04A | three mutations, each caught by a different test: the prepared-once check removed, freshness ignored, candidates needing confirmation prepared (this last one was first not caught, so a candidate selected by default but needing confirmation was added to the test) |
| P79-04B | `afterPrepare`, `eligibleCandidates` and the wiring tests failed before the code existed |

## 4. What failed locally, and why (nothing was weakened)

Three service gates failed on the first `P79-04A` code, found by running every audit script before pushing:
`phase10_service_decomposed` (`dispatch.rs` was 259 lines against a 258 ceiling; the care routes are now one
grouped arm, like the repair routes), `production_rust_has_no_panic_shortcuts` (an `expect` in production
code; the decision now carries the scan it came from) and `service_routes_use_leased_read_start` (a fifth
direct `start_scan_with_lease`; the preview now reuses `start_cleanup_scan`). A `svelte-check` typing error
on a template-literal message key was fixed with a guard helper.

## 5. What changed for the reader

- **Timeline.** A cancelled care run reads neutral under its own code; four failed runs are no longer a
  "recurring problem"; one event per run; another owner's runs never appear.
- **Care panel.** An unread plan says it is unavailable instead of "run a scan first". The button prepares:
  while the local scan runs the page says so and continues when the scan ends (no polling); when a plan is
  ready, the approval dialog lists each candidate with its size, the total, and how many more were found but
  left out because they need the owner's own choice in Deep Clean. Approval still binds to the plan digest.
- **Service.** `PrepareCarePreview` reads and prepares only: a local scan, the same unapproved cleanup plan
  Deep Clean makes, once; it grants no consent and deletes nothing.

## 6. Left out, and not measured

- **Left out:** the space the cleanup actually freed in the care result, and a link from a result to its
  timeline entry (`DBT-P79-003`, open).
- **Not measured:** the prepare journey on a real Windows machine end to end (a real local scan, then the
  review on screen, in Arabic and English); the 15-minute freshness bound; how long a real scan takes.
- **Size:** `P79-04A` is about 170 lines of code and proto and about 330 of tests; the plan estimated 250–350.

## 7. Fast local checks on the PC

Run in my own worktree `C:\dev\lanes\l1` with `CARGO_TARGET_DIR=C:\dev\lanes\l1\target` and
`CARGO_BUILD_JOBS=8`: `cargo fmt --all --check`, `cargo test --locked` for the changed crates and their
dependents, `cargo check -p aethercore-desktop`, the UI type gate, the node UI tests, the UI build,
`static_validate.py` and `source_seal.py`. The results are in the PR description.


## DBT-P79-003 follow-up A: exact domain result (2026-10-02)

The real Cleanup dispatch now carries its terminal domain `reclaimed_bytes` through the Care
executor, typed step report and journal. A value exists only for a completed, verified Cleanup
result. Failed/skipped/unverified and other domain steps carry no measurement; a measured zero
is retained as zero. The core rejects a supplied count outside verified Cleanup evidence.
This is logical deleted-file bytes, not a measurement of physical free-space gain.

`CareStepReport` gains additive wire tags 9 (`has_actual_deleted_bytes`) and 10
(`actual_deleted_bytes`, exact unsigned decimal data). The existing desktop serializes protobuf
messages through JSON; the decimal representation preserves counts above JavaScript's exact
integer range. Contracts are generated by the existing prost build. Migration 0017 stores an
optional decimal TEXT column, preserving old/unmeasured NULL and all u64 values after reopening.
Malformed stored decimal data causes a read failure, never a fabricated zero. Plan digests,
consent, lease, mutation admission and dispatch deadlines retain their existing semantics.

Runtime red: the real composed service fixture reached `VerifiedByDomain` but its wire report
had no `hasActualDeletedBytes` (Null versus true). Green: the same safe fake domain reports
0/1536/9007199254740993 despite preview `expected_bytes=0`; no host files are deleted. Additional
fixtures cover persistence reopen/update through u64::MAX, corrupt decimal data and adversarial
counts on Startup/failed/skipped/unverified outcomes. The full affected-package test run passed
138 tests in 17 suites, zero failures/ignored tests. All-target Clippy passed with warnings denied;
static validation passed 351 checks (`parse_yaml` remains unmeasured).

This source task does not finish the whole debt: localized result rendering, owner-scoped history
restoration after relaunch and the Timeline details link follow in task B. Native Windows fixture
qualification for this exact follow-up is pending the isolated test slot; no hardware or installed
UI acceptance is inferred from these local fixtures.

### Acceptance follow-up B1 — durable result history (2026-10-02)

GetCareStatus restores the latest terminal run for the current owner when no new
prepared plan exists. Historical approval is never session consent. Results keep
the original timestamp and exact measured decimal deletion bytes; non-Cleanup,
unverified, failed and skipped steps do not acquire a measurement. A fresh plan
still returns a preview requiring new approval. Old journals with missing step
records explicitly say that the history is incomplete.

Skipped review/cancelled steps now have durable state and outcome records without
entering Executing or reaching a domain. Final steps_done counts the terminal
records after the final outcome is persisted. The existing review-only adversarial
check now forbids Executing and requires Skipped audit records rather than
forbidding all audit records for review work.

Runtime red traces: restored run ID was empty; final steps_done was zero; skipped
result audit count was zero; legacy missing-step history claimed a complete report.
Green: 152 tests across 17 affected Rust suites, no failures/ignored; four-package
all-target Clippy with -D warnings; UI 88/88 and static 351 checks passed. The UI
runner uses the repository's resolve-ts loader. The new restart test reopens only
a temporary SQLite fixture and asserts owner isolation, no inherited approval,
measured zero, dated history, missing legacy rows and new-plan precedence. Native
Windows fixture qualification and Care result UI/Timeline navigation remain
pending; no installed device mutation was performed.

### Acceptance follow-up B2 — exact result UI and Timeline navigation (2026-10-02)

The real Care panel shows each verified Cleanup step's measured logical deleted
file bytes, including measured zero, or an explicit not-measured label. The
formatter parses the decimal u64 with BigInt, scales/rounds using integer arithmetic,
and localizes the number; the exact decimal remains in a bidi-isolated title.
This describes logical file length, not recovered physical free space. Failed,
skipped, unverified, non-Cleanup and invalid/missing values cannot display a count.
Historical reports retain their date, summary and details action even with zero
restored step rows. Details refreshes the bounded Timeline snapshot and selects
only the exact care-run:<run_id> already in the loaded page. An absent entry is
reported as not loaded, never as a fabricated link; changed session/run invalidates
late replies. This reuses the sealed P87 selection controller without modifying it.

Red: formatter returned null for measured zero; real EN/AR panel lacked result
labels/details; no-op navigation left Overview active. Green: five new real
source-to-render/formatter/controller tests (including u64 maximum, malformed
values, absent source and stale session), UI 93/93, check zero errors/warnings,
build and static 351 checks. The populated result browser fixture passed geometry
8/8 (1280/640 × EN/AR × dark/light, zero horizontal overflow/clips/overlap) and
Arabic clean leak checks 4/4. Browser files are under /private/tmp/p79-result-layout-geometry
and /private/tmp/p79-result-layout. The 640px fixture is not Windows 200% zoom or
Narrator qualification. Native fixture and installed-path evidence remain pending.

### Acceptance follow-up C — malformed composed sources fail closed (2026-10-03)

P79-03's remaining source gap was CarePlan::build falling back to an empty plan;
a malformed source could also disappear while another valid step stayed approved.
Composition now distinguishes an actually empty eligible list from invalid prepared
IDs/digests and a failed non-empty build. The existing typed source-unavailable
error propagates without new wire fields or approval-policy changes. A temporary
DB fixture with a valid source beside an empty ID or malformed digest failed at
runtime before the fix and passes after it. Full service/core verification passed
111 tests, all-target Clippy -D warnings and static 351 checks. The existing true
empty-plan test still passes. No domain execution or owner-device mutation occurs.

### Native source qualification for follow-ups A/B/C (2026-10-03)

Exact native source f0f34683cd5e859f0148fd84386a4cd87461404f was cloned from
an immutable Git bundle into C:\dev\codex-care-fixtures. Windows 11 Pro
10.0.26200/build26200 ran the four-package locked test command with jobs2 and
lane3's own target-p86-codex cache: 154 tests across17 suites passed, no failures
or ignored tests. The same four packages' all-target Clippy -D warnings passed
(exit0, 2m43s). No source overlay was applied. The only untracked Git-root files
were the two receipt logs outside the sealed phase21-workspace. No installed
service/app/model/provider was invoked; test databases/files were fixtures.

Native test receipt SHA256:630e5cf6c73fc6c209f2b8afc228e357e9c7892b2cc8c62a0efdf86c5bea9a37;
Clippy receipt SHA256:ac8cc0a12691f346f2ea2bb3bc422bfb8a737b352536b72bb54b6aea6b4f0f68.
Copies: /private/tmp/p79-care-native-f0f3468.log and
/private/tmp/p79-care-native-clippy-f0f3468.log. This qualifies the native source
fixtures, not installed Care mutation, Narrator, 200% zoom or a hardware matrix.
