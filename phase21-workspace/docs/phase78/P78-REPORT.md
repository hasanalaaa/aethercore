# P78 report

P78 made the repair assessment a bounded, honest operation: it does not search Microsoft Update on its
own, a check that never returns ends as an unknown check, the owner can name what to cancel, and the
screen is not left on "Assessing" by a late answer. Six PRs (P78-02 was split in two), one at a time,
each from the latest `main`, each merged pinned to a head whose CI run was green. A statement below
without the command or run that produced it is labelled a belief.

`main` at the start: `e63bf7b`, green on CI run `36610657856` (head_sha `e63bf7b`, windows on
`aether-win`). No push this session did not make was seen: `git log e63bf7b..origin/main` holds only the
merge commits below, all by `hasanalaaa`.

## 1. What merged

| task | PR | merge sha | PR CI (head) | `main` CI at merge sha |
|---|---|---|---|---|
| P78-01 the update health probe is local; the completion time has its test | #75 | `729a5b6` | `36624578040`, fuzz `36624578038` (`b06317d`) | `36629293943` success |
| P78-02A a check runs under its own deadline and slot (`bounded`) | #76 | `04a96fd` | `36634431168` attempt 2, fuzz `36634431169` (`bc9aa29`) | `36642598308` success |
| P78-02B the four long checks use it | #77 | `df28423` | `36642732805` attempt 3, fuzz `36642732744` (`76f9a06`) | `36649046176` success |
| P78-03 cancel verbs carry an id (decision D3) | #78 | `fca43da` | `36649220657`, fuzz `36649220634` (`b6a1fef`) | `36653984224` success |
| P78-04 the assessment screen | #79 | `e685b14` | `36654400180`, fuzz `36654400190` (`c5cca5a`) | `36658277708` success (windows on `aether-win`, `02:06:26Z`–`02:48:35Z`) |

Every PR's windows job ran on the owner's PC (`aether-win` or `aether-win-2`, both with the
`aether-win` label). Every lane merged `origin/main` before its last CI run and re-sealed
(`source_seal.py` → `regenerate-source-manifest.py` → `source_seal.py`: OK, both manifests).

## 2. The plan was partly stale, so the work was smaller than §2 said

The plan's "MEASURED" lines described `main` before P76's `DBT-P76-007`. Checked first, per task:

- **P78-01:** the assessment worker already turned a panic into a stored `Failed` assessment with the same
  id (`an_assessment_whose_platform_panics_fails_instead_of_staying_assessing`) and already took `completed`
  after the checks. Only the probe (`SetOnline(VARIANT_BOOL(-1))`, online) and a test for the timestamp were
  missing.
- **P78-02:** per-check progress and the owner's cancel already existed; the missing part was a deadline and
  a slot per provider.
- **P78-04:** the page already showed the running check, the count finished, the start time, a slow-scan
  note, a stop button and an indeterminate bar.

## 3. Red before, per task

Shown at the time, by the command in the PR, not replayed at the end (unlike P77): each test fails for the
stated reason against the old behaviour.

| task | red | why it fails |
|---|---|---|
| P78-01 | `health_probe_is_local.rs` on the parent: `the health probe asks for an online search: VARIANT_BOOL(-1)`; `an_assessment_completes_after_its_measurement_ended` green on `main`, red with the old `completed = started` (`completed 1790712388913 is before the measurement ended 1790712388996`); the `wire-values.test.ts` test on the parent | the probe searched Microsoft Update; the timestamp had no test; the probe's `format!` sentence had no Arabic |
| P78-02A | the three `tests/bounded.rs` tests with `run_check` running its work inline, as `main` does: `the assessment waited for the stuck check`, the cancel test waits its full 5 s, the panic test propagates | no per-check deadline or slot |
| P78-02B | `assess_is_bounded.rs` against `main`'s `windows_impl.rs`: `DISM_SLOT is not passed to run_check: its check runs unbounded` | the four long checks were plain calls |
| P78-03 | first, the tests did not compile (`cancel_assessment` took one argument, no `cancel_repair`); then each of three mutations of the finished code failed exactly one test: a stale assessment id cancels anything (`a stale id cancelled the current assessment`), the barrier ignores the cancel, any id cancels the running plan; the UI test without its row | no id on the cancel, no repair cancel |
| P78-04 | `repair-progress.test.ts`: the module did not exist; with the answer always winning: `the event stream already said Ready` | a late answer overwrote a finished assessment |

## 4. What went red, and why

- **#75, run `36624335128`, head `292f459`.** `windows` failed at "Delivered source seal" (`listed=1554
  verified=1554 failed=1`) and `gate-self-tests` at the same check: I regenerated the manifest before the
  new test file was tracked, so it was unlisted. Fixed at `b06317d` (regenerate after `git add`).
- **#76, run `36629411289`, head `2210918`.** `windows` failed at "UI unit tests": the P77-03 scan, which
  reads Rust sources, found three new sentences in `bounded.rs` without Arabic. I had planned their keys for
  02B and the split moved the sentences into 02A. Fixed at `bc9aa29`.
- **#76, run `36634431168` attempt 1; #77, run `36642732805` attempts 1 and 2; #78, run `36644042355`
  attempt 1: runner drops.** Each ended exactly 10:00 after it started with no step logs and the annotation
  "The self-hosted runner lost communication with the server" (`21:36:51Z`→`21:46:51Z`,
  `22:59:36Z`→`23:09:36Z`, `23:10:52Z`→`23:20:52Z`, `23:20:55Z`→`23:30:55Z`). The planning session read the
  runner's `_diag` log: after a finished job the listener on `aether-win` acknowledges the next one and
  cancels its worker, so GitHub times the job out at 10:00. A stuck listener, not the code and not load. I
  reran #76 once; on the planning session's instruction I did not count the later drops against my
  one-rerun rule; it restarted `aether-win` and #77's third attempt passed; I reran #78 once after that.
  #78's own rerun was superseded by a push (conflict with `main`), and its new head passed.
- **A gate I met before pushing, not in CI.** My first P78-04 had a ticking elapsed-time clock
  (`setInterval`). `static_validate.py` (`phase10_ui_polling_eliminated`, `phase11_no_renderer_polling`) and
  `zenith-adversarial-audit.py` (`ui_zero_interval_polling`) failed. I removed the clock; the gates were not
  touched.
- Nothing else failed. `0xC0000005` never appeared.

## 5. What changed

- **P78-01.** `probe_update_health` searches the agent's local cache (`SetOnline(VARIANT_FALSE)`; D2: no online
  default, no replacement online path). Its detail says it answered from the local cache and does not show that
  Windows is up to date. The result code stays `UpdateHealthy` when the agent answered (three consumers read it
  as agent health); the sentence carries the limit. The previous healthy sentence was composed with `format!`, so
  the P77-03 scan could not see it: it had no Arabic and read "Details unavailable" on `main`.
- **P78-02.** `system-repair::bounded` (`ProviderSlot`, `run_check`): a check runs on its own thread under its
  own deadline; past it, it is told to stop and reads `Unknown` (`CheckTimedOut`); a still-running earlier attempt
  is not started again (`CheckStillRunning`); a panic reads `ProbeUnavailable` and frees the slot; the owner's
  cancel returns within 50 ms; a late result goes to a channel nobody reads. DISM ScanHealth, SFC `/verifyonly`
  and the chkdsk online scan (20 minutes each) and the Windows Update probe (2 minutes) use it. A timed-out check
  keeps the id of the check it replaces, so the diagnosis reads it as `Unknown`.
- **P78-03 (D3 applied exactly).** `CancelRepairAssessmentRequest.assessment_id` (field 1; empty still means the
  current one); new verb `CancelSystemRepairRequest { plan_id }` (`Request` tag 96). Before the mutation barrier
  a cancelled plan ends `Failed`, `FailedBeforeMutation`, nothing to recover, no command run; after it, the
  request stops nothing. Another owner's or an unknown plan is refused; an older plan's id does not touch the
  running one; repeating is safe. `CancelRequest` in `events.proto` is not reinterpreted.
- **P78-04.** `settleAssessment` keeps a finished assessment when a late `start`/`cancel` answer repeats
  `Scanning`; the cancel names the assessment on screen (the desktop command takes `assessment_id`); offline the
  page says the connection was lost and that the assessment may still be running, and offers no stop button.

## 6. Left out on purpose

- **D3 covers cancel verbs only, and D0 wants owner acceptance for any other wire change**, so P78-03 adds no
  `cancelRequested` field to the repair status and no `Cancelled` plan state.
- **A repair cancel button**, and any cancel after the barrier, wait for P85 (a real stop of the running tool).
  The residual is a new ledger row, `DBT-P78-002`.
- **An elapsed-time clock and a last-update time.** The clock needs `setInterval`, which the no-renderer-polling
  gates refuse; the last-update time needs a wire field (D0). The page keeps showing the start time.
- **`collector-runtime` was not reused** for the deadline mechanism (`run_isolated_gated_with_token` has the same
  idea): adding it as a dependency changes `system-repair`'s hashed `Cargo.toml` and `Cargo.lock`, a
  dependency-freeze run for about a hundred lines. Noted in the module and the ledger.

## 7. Not measured

- **The deadlines are first bounds, not measurements**: how long DISM, SFC and chkdsk take on a real machine,
  and whether 20 minutes is right, needs the PC. A slower real scan reads unknown and can be run again.
- **A network capture on Windows** proving the update probe makes no connection.
- **`system-repair` cannot be cross-compiled on a Mac** (`libsqlite3-sys` needs a MinGW compiler). The rewritten
  `assess` was type-checked and run verbatim against native stubs (7 checks, 14 progress events); the `windows`
  job compiled the real one.
- **The screen on the owner's Windows 11 with Narrator and Arabic.**
- `cargo test --workspace` on the Mac fails only in `intelligence-core` tests that need the 1.1 GB embedded model
  (not on that machine); on the PC's runner they pass.

## 8. Diffs, and one thing that did not fit the budget

The plan's ≤ ~300 lines per task held for code, not always with tests: P78-02 came to about 550 with its tests
and was split in two (A about 400 with 250 of tests, B about 115); P78-03 is about 130 of code and proto and about
290 of tests, P78-04 about 90. The red-first rule keeps each test with its fix.

## 9. The ledger

Closed in P78: `DBT-P78-001`. Opened: `DBT-P78-002` (repair cancel after the barrier: P85, and D0 for a status
field and a plan state). Recount on `main` at `e685b14` (a script over rows that begin `` | `DBT- ``): 187 rows, of
which 160 `CLOSED`, 21 `OPEN`, 3 `ACCEPTED`, and 3 with a status of their own.

## 10. Cleanup

The merged branches (`lane/p78-01-local-health-probe`, `lane/p78-02-bounded-assessment`,
`lane/p78-02b-bounded-checks`, `lane/p78-03-cancel-verbs`, `lane/p78-04-progress-screen`, `docs/p78-report`) are
deleted only after each head is confirmed equal to its PR head; the local worktrees and the unpushed
`lane/p78-02b-wire-deadlines` are removed. Other branches were not touched.
