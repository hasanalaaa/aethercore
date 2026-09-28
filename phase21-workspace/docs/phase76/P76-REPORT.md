# P76 report

P76 finished the owner's decisions on P75's open list, the lanes that followed from them, and the
eight findings of the owner's real Windows 11 install. One PR at a time, each merged only on a CI
run whose `head_sha` was the merged head and with `main` green at the previous merge sha. A
statement below without the command or run that produced it is labelled a belief.

`main` at the start: `1f8aff9`, green on CI run `36349909771` (head_sha `1f8aff9`). No push this
session did not make was seen on the repository at any point.

## 1. What merged

| PR | lane | merge sha | PR CI (head) | `main` CI at merge sha | ledger |
|---|---|---|---|---|---|
| #57 | owner-decisions (+ `ea4eb55`, the P75 report's gate table at `771cfb8`) | `5a1f6c8` | `36356157490`, fuzz `36356157486` (`c0dafbf`) | `36359445338` | `DBT-P75-052` closed, `-055` closed, `-077` partly; `DBT-P76-001`, `-002` new |
| #58 | service-early-ipc | `14f2f1b` | `36359499768`, fuzz `36359499833` (`d0b1544`) | `36363800314` | `DBT-P75-078` closed |
| #59 | deps (Dependabot #41–#46; rusqlite dev-dependency) | `be965a2` | `36363900866`, fuzz `36363900849` (`9061e18`); freeze `36359629504` (`53575bc`) | `36370983892` | `DBT-P75-027` closed; `DBT-P70-005` re-measured |
| #60 | fuzz | `5fa4a24` | `36371078846`, fuzz `36371078906` — 12 of 12 targets (`4f286d1`) | `36378861678` | `DBT-P58-004` closed |
| #61 | small-closes | `1581d22` | `36378893648` (`fcb36a5`) | `36386622904` | `DBT-P75-091` closed, `DBT-P75-001` closed |
| #62 | ui-text | `521fbb3` | `36391906783`, fuzz `36391906848` (`4eeeaa4`) | `36398738348` | `DBT-P76-003`, `-004`, `-005`, `-010` closed |
| #63 | care-truth | `ad7a639` | `36399015679`, fuzz `36399015609`; installer `36399011841` (`3f8150c`) | `36465428142` | `DBT-P76-006`, `-008` closed |
| #65 | bootstrap-pwsh | `db54935` | `36465665068`, fuzz `36465665071` (`5b47c93`); freeze `36398701406` (`491a768`) | `36472830347` | `DBT-P76-009` closed |
| #64 | repair-progress | `59cacb0` | `36472889685`, fuzz `36472889676` (`cd2b8ff`) | `36480538760` | `DBT-P76-007` closed |

#65 merged before #64 because it was green first; #64 then merged `main` and re-ran.

Every lane after the first merged `origin/main` before its last CI run; where a squash merge made
`main` textually different but identical in content to what the lane already held (verified with
`git diff` between the lane's base and `main`: empty), the conflict was resolved to the lane's
bytes and the result compared to the lane head (empty diff). Each lane re-sealed after the merge
(`source_seal.py` → `regenerate-source-manifest.py` → `source_seal.py`: OK, both manifests).

## 2. The owner's decisions

### 2.1 `DBT-P75-052` — insights and the assistant in the reader's language (closed)
- `locale = 3` on `AskAssistantRequest` and `RequestInsightRequest` (additive; empty is `en`;
  anything but `en`/`ar` is refused `InvalidRequest`, not answered in English). The desktop, the UI
  (the shell's locale) and `aetherctl --lang` send it.
- Arabic appends a language rule with an Arabic example to both system prompts. Without the
  example the 1.5B model degenerated into a run of digits (measured, this Mac). Tags, `NO EVIDENCE`
  and every gate are the same in both languages.
- The rule engine writes its four summaries in Arabic.
- The insight number gate drops a line that states a number it cannot check in Arabic
  (Arabic-Indic digits, Arabic number words). Red before: "ثلاث مرات" citing a pattern of 5 was kept.
- Real model, this Mac: an Arabic answer in 329 ms, three Arabic insight lines in 1773 ms; the
  English path unchanged. Unix IPC serves `ar`, `en` and empty and refuses `fr`.
- **Measured and recorded, not fixed (`DBT-P76-001`):** the model's Arabic is less faithful than its
  English — one Arabic insight said a completed startup plan "stopped" while citing it. The gates
  check tags and numbers, not meaning, in both languages.
- Backend `detail` strings shown raw are split to `DBT-P76-002` (not in the decision).

### 2.2 `DBT-P75-055` — a privacy counter for hosts, IPs and MACs (closed)
`network_identifier_redactions = 5` on `SupportPrivacyReport` (additive) counts host-name keys, MAC
keys and MAC and IP tokens in text; the account and serial counters no longer include them; the UI
total adds it. The escaped-identifiers test now asserts serial 1, account 0, network 6 (it asserted
serial ≥ 3, which the MACs made true).

### 2.3 `DBT-P75-077` — retired audits (partly; the row stays open)
Moved to `retired-audits/` with a README giving the reason and the checks red at `1f8aff9`, not
deleted and not re-baselined (one import path changed so the moved script runs):
`phase18_1-driver-truth-audit.py`, `phase19-windows-repair-audit.py`, `phase35-gd-proofs.py`. The
same checks fail before and after the move.

**Not moved, because a live gate runs them** — the decision named them as having no CI caller, and
measuring showed they do:
- `scripts/phase35-adversarial-audit.py` is the "Phase 35 release authority and adversarial
  metadata gate" in `release.yml:68`. It is red on `main` (`p35-inherits-p34-930`,
  `p35-version-consistency`, `p35-update-product-match`, `p35-no-secret-regression`), so a release
  run would fail there. Owner: remove that release step, or re-baseline.
- `p27-wirefreeze:*` and `p28-deps` / sealed-snapshot are checks inside `phase27`/`phase28`, which
  the `phase29`…`phase34` chain imports in-process; `test_gate_readers.py` runs `phase29` in CI.

## 3. `DBT-P75-078` — the service answers before the model loads (closed)
The two paths hold clones of one unresolved model (a shared `OnceLock` slot); composition loads it
on its own thread, so the socket binds first. Until the load resolves the engine label reads
`loading`, `RequestInsight` answers `insight.error.modelLoading` (Busy, retryable) and a turn faults
`assistant.fault.modelLoading`; a failed load resolves every clone as before. The UI labels all
three in both languages.

Red before, on the real service binary with the real model beside it (`--features unix-ipc`, this
Mac, debug): the pre-fix tree announced no socket within 30 s; after, the socket at 1252 ms, the
first `ListInsights` label `loading`, and `localModel` at 56479 ms.

## 4. Dependencies (`#59`, `#65`)
#41 actions/download-artifact 4 → 8, #42 reqwest 0.13.5, #45 tauri 2.11.6, #43
@sveltejs/vite-plugin-svelte 7.3.1, #44 @tauri-apps/cli 2.11.5, #46 svelte 5.57.1, one commit
each, plus `rusqlite` as a dev-dependency of `cleaner` and `system-repair` for the six SQL probes P75
could not commit (`DBT-P75-027`, rewritten: the code had not been kept). One freeze over the final
tree, minted by `windows-installer.yml` run `36359629504` at `53575bc` (build, install and the
installed-product smoke green, which also ran download-artifact@v8); the runner's lockfiles were
byte-identical to the branch; `check-dependency-freeze.py`: APPROVED. #41–#46 closed as landed.
#65 changed the hashed root `package.json` (scripts only) and went through its own freeze, run
`36398701406` at `491a768`, APPROVED.

**#20, TypeScript 7 (`DBT-P70-005`), left open:** the newest `svelte-check` (4.7.6) still declares
`typescript` `^5` or `^6` as its peer, so TypeScript 7 needs both compilers and the experimental
`--tsgo` checker as the project's type gate.

## 5. Fuzz (`#60`, `DBT-P58-004` closed)
All twelve sources in `fuzz/fuzz_targets/` are `[[bin]]`, and `fuzz.yml` fuzzes all twelve:
run `36371078906`, 12 of 12 jobs green.

## 6. Small closes (`#61`)
- `DBT-P75-091`: a scan still running headlines "Scanning", not "Healthy" (red before in
  `wire-values.test.ts`).
- `DBT-P75-001`: the Linux telemetry provider is clippy-clean, and the new `ci.yml` job
  `linux-provider-clippy` runs that check on ubuntu.

## 7. The owner's Windows 11 install (`DBT-P76-003` … `-010`)
Found on a real install (i7-14700K) at `be965a2`, installer built locally; the screenshots are on
that PC, not in the repo. One ledger row each, all closed, each with a test that fails on the old
code.

| # | finding | row | PR | what changed | red-before test |
|---|---|---|---|---|---|
| 1 | Hardware Health, AR: the disk description in English | `DBT-P76-003` | #62 | the summary (with its count) and every unreported counter name map to EN/AR keys | `tests/localized-text.test.ts` |
| 2 | Performance: `processTop · NotCollected`, `power.temperature · Degraded` raw | `DBT-P76-004` | #62 | all 12 collector ids and all 10 states labelled in EN/AR (5 were); an unknown id reads "Another collector", never the id | `tests/localized-text.test.ts` |
| 3 | Deep Clean, AR: `WindowsTemp`, `UserTemp`, `WER`, `Files` raw | `DBT-P76-005` | #62 | the five Windows categories and the `Files` kind named in EN/AR, also inside the completion sentence | `tests/localized-text.test.ts` |
| 4 | Timeline empty after a care run that deleted files | `DBT-P76-006` | #63 | cause: `before_sequence = 0` means the newest page, but the service took it as an end index, so the newest page was **always** empty, for every domain. Fixed; care runs are ingested (`care.run:*`) and `start_run` records its run id; every timeline code reads as a sentence in EN/AR; a failed read is shown, not an empty list | composition `a_care_run_and_its_cleanup_are_on_the_newest_timeline_page`; `timeline-text.test.ts`; the installed smoke now checks the care run is on the timeline |
| 5 | System Repair "Assessing…" 10+ minutes | `DBT-P76-007` | #64 | both: legitimately slow (DISM ScanHealth, SFC /verifyonly, CHKDSK /scan in sequence, nothing published until all finished) **and** able to stick (a platform panic left `Scanning` for the life of the service, every later start `Busy`). Each check now lands as it finishes and the running one is named; the page shows it with the count and the start time; `CancelRepairAssessmentRequest` (tag 95, additive) stops it — SFC/CHKDSK killed, DISM through its cancel event, all read-only — keeping what finished; a panic ends `Failed` | coordinator `an_assessment_whose_platform_panics_fails_instead_of_staying_assessing`, `a_running_assessment_shows_its_progress_and_can_be_cancelled`; `repair-progress.test.ts` |
| 6 | Care with the default auto category empty: `chosen=0`, no reason | `DBT-P76-008` | #63 | the panel tells apart no scan, nothing eligible (naming, localized, the categories that need opt-in in Deep Clean), and a plan | `care.test.ts` |
| 7 | Build scripts need PowerShell 7; bootstrap ran 5.1 and never checked | `DBT-P76-009` | #65 | `bootstrap.ps1` (the one entry 5.1 can start) checks for `pwsh`, installs it with `winget install --id Microsoft.PowerShell` when asked, and hands over to it; every other `package.json` script calls `pwsh`; the four 7-only scripts declare `#Requires -Version 7.0` | `scripts/test_bootstrap_pwsh.py` (a CI gate self-test); CI parses `bootstrap.ps1` under Windows PowerShell 5.1 |
| 8 | The assistant drawer (Ctrl+/) never found | `DBT-P76-010` | #62 | the rail's first entry, labelled like the others ("Ask the assistant" / "اسأل المساعد") | `tests/localized-text.test.ts` |

Not measured: which of the two causes of finding 5 applied on the owner's PC (the logs are on that
PC). The Windows cancel path is covered by compilation on the CI Windows job and by the coordinator
tests with a fake platform; it was not run against a real DISM/SFC.

## 8. Final installer run
Dispatched on `main` at the final code sha, after #64:

- run **`36480568322`**, `windows-installer.yml`, head_sha **`59cacb05697f94ab39e42ac8d1849e95b8680712`**,
  conclusion success: `build-unsigned-candidate`, `product-code-probe` and `bundle-log-acl-probe`
  all green. `main` CI at the same sha: run `36480538760`, success.
- Unsigned installer artifact: **`AetherCoreSetup-windows-unsigned-59cacb05697f94ab39e42ac8d1849e95b8680712`**
  (1152345128 bytes). Name and size read from the run's artifact metadata; not downloaded.
- The `bundle-log-acl-probe` smoke, from the job log:
  - service: `STATE : 4 RUNNING`;
  - all 14 `aetherctl` verbs `exit 0 ok=True` (capabilities, engine-source, self-check, doctor,
    perf start/snapshot/report/stop, scan start/status, care status, insights list, timeline
    page/patterns);
  - care end to end: cleanup scan `Ready candidates=1 chosen=1`, preview, run `Completed`
    (`care.summary.completed`, step `VerifiedByDomain`), `timeline entries=8 care run=true
    executions=1`, a grant for a plan never shown refused, a second run `AwaitingConsent`,
    `SMOKE: PASS`;
  - uninstall: `exit 0; ProgramData\AetherCore exists after: False`;
  - `GATE: pass`.

This report's own merge is documentation only (`docs/phase76/`, the seal); no product byte
changed after `59cacb0`.

An earlier dispatch on `main` at `1581d22` (run `36386636531`) was green; the run above, on the
final tree, supersedes it.

## 9. Open, and why
Ledger §1 at the end: 179 rows, 20 open, 1 malformed (`DBT-P63-014`, six cells, reported as
before).

- `DBT-P74-002` (two residuals of the data-root re-own): not attempted in P76; open as the ledger
  states it.
- `DBT-P75-077`, remainder — §2.3; the owner's call.
- `DBT-P70-005` / #20 — §4; upstream.
- `DBT-P75-002` (unix IPC does not forward live events): the fix needs one writer shared by the
  response workers and an event pump, or frames interleave; and no macOS client reads the event
  stream today (the desktop app there sends one-shot requests). Not a small close.
- `DBT-P76-001` (the model's Arabic is less faithful), `DBT-P76-002` (raw backend `detail` strings).
- `DBT-P63-003` (`noRedirectionBitmap`): the row says it is retired only by a Tauri release that
  exposes the field or by building the window in Rust, not by editing configuration.
- The rows that need a machine or a person (§3 of the ledger) are unchanged.

## 10. Cleanup
Remote branches deleted (29), each merged by a PR whose head was its current sha or, for
`lane/windows-smoke-final`, whose only delta is in `main`: the P75 `lane/*` branches and every
`lane/p76-*` after its merge. Kept, not merged: every `probe/*`, `wip/*`, `rescue/fs-acl-p75`,
`lane/cli-trial`, `lane/telemetry-trial`, `lane/windows-smoke`, `lane/service-rollback-before`,
`docs/p75-handoff-2`, and `design/*`. The 21 scratch worktrees of earlier sessions and this
session's lane worktrees were removed (all clean; one untracked P75 report draft copied out first).
Left in place: `aethercore-audit`, `aethercore-design` and `aethercore-review` beside the checkout
(not scratch worktrees).
