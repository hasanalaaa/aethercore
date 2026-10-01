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
