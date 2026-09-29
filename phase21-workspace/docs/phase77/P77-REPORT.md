# P77 report

P77 made the Arabic UI stop printing identifiers and backend English, and put a gate in CI that
keeps it from coming back. Five tasks, one PR each, merged one at a time from the latest `main`.
A statement below without the command or run that produced it is labelled a belief.

`main` at the start: `ab7eadb` (P76's #67), green on CI run `36533258810` (head_sha `ab7eadb`).
The windows job ran on the owner's PC (`runner_name=aether-win`) in every PR run below; each
merge waited for a run whose `head_sha` was the merged head. No push this session did not make was
seen: `git log ab7eadb..origin/main` holds only the six commits below, all by `hasanalaaa`.

## 1. What merged

| task | PR | merge sha | PR CI (head) | `main` CI at merge sha |
|---|---|---|---|---|
| step 0: the plan and decisions D0–D31 | #68 | `3aa42eb` | (`2e9772d`) | `36556583828` success |
| P77-01 the display boundary fails closed | #69 | `c3a4b5b` | `36556698130`, fuzz `36556698202` (`fef0780`) | `36564794351` success |
| P77-02 collectors and providers are named | #70 | `8c50ea1` | `36565025421` **failed**, then `36570056447`, fuzz `36570056446` (`5810ed3`) | `36574768117` success |
| P77-03 the rendered-DOM leak gate | #71 | `ef264bf` | `36575126979`, fuzz `36575126877` (`ce687bc`) | `36583855405` success |
| P77-04A service errors as keys | #72 | `9f2cffd` | `36584055584`, fuzz `36584055580` (`6d41679`) | `36593169078` success |
| P77-04B Fleet errors in the reader's language | #73 | `38adffb` | `36593335726`, fuzz `36593335422` (`3dbf3e7`) | `36601943250` success (windows on `aether-win`, `17:01:17Z`–`17:36:08Z`) |

Every lane after the first merged `origin/main` before its last CI run and re-sealed
(`source_seal.py` → `regenerate-source-manifest.py` → `source_seal.py`: OK, both manifests). The
lanes were stacked while I built them, so each merge conflicted with the squash of the ones before
it in context lines only; the diff of each lane against `main` after the merge was exactly its own
task (P77-02 +108/−6, P77-03 +338/−25 including the ledger and seal, P77-04A +103/−1 in code,
P77-04B +189/−14 in code).

## 2. Red before, per task

The new tests of each PR, run against the tree of that PR's parent (a scratch worktree per task,
tests copied in from the merge commit). Each fails for the stated reason; the other tests pass.

| task | failing tests on the parent | why they fail |
|---|---|---|
| P77-01 | `an unknown enum value is never shown as itself, in either language`; `backend English prose is never shown in Arabic, not even inside an Arabic sentence`; `a caller that shows data opts out of the fallback, and only then` | every `localize*` helper returned the unknown value as it came, and unknown prose was pasted into Arabic sentences |
| P77-02 | `every deep-scan collector id the coordinator emits is named in both languages`; `every provider that reports a fault is named in both languages`; `the deep-scan and provider-fault views do not print collector or provider ids` | the ids come from `coordinator.rs` and four crates; none had a name, and two pages printed the id |
| P77-03 | `every sentence the UI-feeding crates write for the reader has an Arabic rendering`; `an identifier stays data, a bare word does not, and a dropped sentence is reported once` | 17 sentence-like Rust literals had no Arabic; no dropped-sentence log |
| P77-04A | `every message key the service emits resolves in both catalogs`; `a transport failure is classified, a known key is kept, and raw text is never the answer` | 14 keys the service emits had no label in either catalog; no `serviceErrorKey` |
| P77-04B | `every error the fleet crate can raise reads in Arabic without its English or its payload`; `the sentences the desktop hands the fleet page read in Arabic`; `the fleet page shows no error as the sentence it was thrown with` | no Fleet pattern table; `FleetPage` rendered `String(error)` and a raw `result.detail` |

`disk activity a provider says it did not measure reads unmeasured, not 0%` also fails in those
scratch worktrees. That is an artefact of the replay (`Cannot find package 'svelte'`: the scratch
worktrees have no `node_modules`), not a red-before; it passes in CI and locally.

P77-03's gate was also red on wire-true fixture data before it was fixed: it found 13 fixture
sentences I had invented (corrected to the sentences the code emits), and the Rust scan found the 17
untranslated real sentences. Its three `--plant text|attr|arg` controls must each be detected, and
were, locally and in the `ui-leak-gate` job of run `36575126979`.

P77-02's guards were made red on purpose too: a planted bare `{fault.operation}`,
`{fault.provider}` or `<TechnicalText` fails `phase13_fault_ids_not_user_visible`
(`static_validate.py`) and `provider_fault_localized_ui` (`phase13-reliability-audit.py`); restored,
both pass.

## 3. What went red, and why

- **#70, run `36565025421`, head `c0145c2`.** The windows job failed at the enterprise gate:
  `phase13-reliability-audit.py`'s `provider_fault_localized_ui` demanded the old markup that
  printed the provider and operation ids, which is what P77-02 removed. I had run `static_validate.py`
  and the phase 12 audits but not this one. The failure was explained and was not a crash (no
  `0xC0000005`), so I did not rerun. I changed the check to pin the new invariant, tightened both
  guards (my first version missed a bare `{fault.operation}`; the negative control showed it), ran
  every audit script the windows job invokes (ten, all green) before the next push, and merged on the
  new run. From then on I ran that whole set on every branch before pushing.
- Nothing else failed. No runner drop, no `0xC0000005`, no rerun.

## 4. What changed for the reader

- **P77-01.** An unknown enum value reads "Unknown" in both languages; unknown backend prose reads
  "Details unavailable" in Arabic. Bare technical codes (HRESULT, numbers, UPPER_SNAKE, dotted or
  underscored names) stay data. `LocalizedOwnedText` has `data` for what the machine wrote.
- **P77-02.** Deep-scan collectors and fault providers have names (`localizeScanCollector`,
  `localizeFaultProvider`); an unnamed id reads "Another data source". The fault panel no longer
  prints the operation id.
- **P77-03.** New job `ui-leak-gate` (hosted `macos-latest`, about 2 minutes; see §6). It fails on a
  sentinel or the fallback text in any text node, `aria-label`, `title`, `alt` or `placeholder` of
  an Arabic page, and requires its own three plant controls to be detected. The boundary logs each
  dropped sentence once (`console.warn`, bounded to 200). 17 real sentences gained Arabic
  (`tech.*`, EN/AR parity).
- **P77-04A.** 14 service message keys gained EN/AR labels; `serviceInvoke` rejects with
  `serviceErrorKey(error)`: a known key is kept, a transport or OS failure reads
  `service.error.unreachable|timeout|denied|failed`, and the raw text goes to `console.error`
  only. `Error` instances pass through unchanged (`setError` and the pages' `.includes(...)`
  checks depend on them).
- **P77-04B.** One pattern table `fleetOwnedText` (38 rows, 38 `fleet.err*` keys) covers every
  `#[error]` in `crates/fleet/src` and the desktop's literals. The desktop's `detail` stays a
  string: no published contract changed, so no decision from the owner's list was needed.

## 5. Regression window (a fact about `main`, not a belief)

P77-01 turns any real product sentence that has no Arabic rendering from raw English into "Details
unavailable". The 17 such sentences were found by P77-03. So from the merge of #69 (`c3a4b5b`,
`2026-09-29T11:55:40Z`) until the merge of #71 (`ef264bf`, `2026-09-29T14:35:49Z`), 2 h 40 min,
`main` showed "Details unavailable" for those 17 real sentences in Arabic where it had shown
English. No release was cut in that window: `gh release list` and `git tag` show the newest release is `v0.1.11-rc.1`, published `2026-09-09`.

## 6. Not measured, and open

- **The Rust sentence scan undercounts.** It reads sentence-like string literals (17 of 117 had no
  Arabic); it does not see sentences built with `format!`. The fixture may not carry every sentence
  the device sends. What I have is: every sentence in the scan and in the wire-true fixture reads in
  Arabic.
- **Names the device sends** (a friendly name, a publisher, a service name) are data and are shown
  as sent, in whatever language the device wrote them. That is deliberate and tested
  (`LEAKDATA Samsung 980` must survive).
- **Correlation ids** on screen were not audited.
- **A real Windows 11 install** was not run for P77; the evidence is the fixture in Chrome plus the
  tests. The Arabic Windows 11 review at 100–200 % DPI with Narrator is still the manual step the
  phase 12 gate names.
- **`ui-leak-gate` runs on GitHub's hosted macOS runner**, not on the owner's PC (the owner asked
  for checks to run on the PC rather than GitHub's servers). It needs Chrome and takes about two
  minutes; `gate-self-tests`, `deny-check` and `linux-provider-clippy` are hosted the same way.
  Moving it to the PC is one `runs-on` change plus Chrome on the runner; I did not do it without
  the owner's word because it would add to the one-at-a-time windows queue.
- **The PC queue is the limit on throughput.** One windows job at a time, 35–39 minutes, plus one
  on `main` per merge; each task took 1–2 hours end to end. A merge can cancel the previous `main`
  run (concurrency is `ci-${{ github.ref }}` with cancel-in-progress); all `main` runs above
  completed.

## 7. The ledger

Closed in P77: `DBT-P77-001` (the roadmap is in the repository), `DBT-P77-002` (the Arabic UI's
identifiers and backend English) and `DBT-P76-002` (Fleet results and service errors reach the UI
raw). Recount on `main` at `38adffb` (a script over rows that begin `` | `DBT- ``): 185 rows, of
which 159 `CLOSED`, 20 `OPEN`, 3 `ACCEPTED`, and 3 with a status of their own (split, partly fixed,
reclassified). I did not re-audit the 20 open rows for staleness in P77.

## 8. Cleanup

The merged branches `lane/p77-01-owned-text`, `-02-provider-names`, `-03-leak-gate`,
`-04a-service-errors`, `-04b-fleet-errors` and `docs/p77-roadmap` are deleted only after each head
is confirmed equal to its PR head; the local scratch worktrees are removed. The repository's other
branches (`wip/*`, `probe/*`, `lane/cli-trial`, …) predate P77 and were not touched.
