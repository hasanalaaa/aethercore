# P75 handoff — state at the stop (2026-09-25)

The lead session stopped at its usage limit in the middle of Wave 1. Nothing was merged except the
plan. This file records exactly where every lane is, so the next engineer can continue without
re-deriving anything. The plan is `AMBITION.md`; the binding per-lane rules are `LANE_CONTRACT.md`
(both in this directory).

## Ground truth

* `main` = `f22d285` (the plan commit), **green**: CI run `35962879005`, head_sha `f22d285`.
  The previous head `9747fe3` is green on run `35938070817`.
* Ledger §1: 76 rows, 23 open, 3 malformed (`DBT-P56-002`, `DBT-P63-010`, `DBT-P63-014`). No row
  has moved yet in P75.
* The local Mac: workspace clippy `-D warnings` fails on `main` (driver-backup dead code +
  4 lints in `performance-telemetry/src/macos_impl.rs`); PR #26 fixes it. `cargo fmt --check` and
  `cargo test --workspace --locked` pass on `main`.
* Disk on the Mac was the binding constraint (~80 GB free). Every cargo command used
  `CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0`.

## Branch convention used at the stop

* `lane/<name>` — the lane's committed, sealed work (what a PR is opened from).
* `wip/<name>` — `lane/<name>` **plus one final commit titled `WIP(p75/<name>)`** holding the
  uncommitted edits at the moment of the stop. That commit is **not sealed, not verified**. Finish
  it, split it into proper commits (one concern each, message = what/why/proof), seal, and fold it
  into `lane/<name>`. Delete `wip/<name>` when done.

## Lane status

| lane | branch head(s) | PR | what is done | what is next |
|---|---|---|---|---|
| `mac-clippy` | `lane/mac-clippy` `753cfd6` | **#26, CI green** (`36058189476` at `753cfd6`) | whole-workspace macOS clippy clean | **merge first** (protocol below) |
| `no-egress` | `lane/no-egress` `12efc77` | **#25, CI green** (`36058095459` at `12efc77`) | passive driver scan is LocalCacheOnly; interactive is Online; type-level scope | merge second |
| `service-rollback` | `lane/service-rollback` `bf16bf6`; `wip/service-rollback` `22c3de2` (adds the lane doc draft); `lane/service-rollback-before` `d209712` (gate-only proof branch) | none | rollback CA + hardener service-config verb; probe gates rewritten (state sampled before stop, RUNNING + UNRESTRICTED + <60 s); purge condition `NOT UPGRADINGPRODUCTCODE`; ProductCode probe job | Probe run `36057858803` on the gate-only branch **failed** (the intended red-before — confirm from its log). Probe run `36057863308` on `bf16bf6` **also failed** in `bundle-log-acl-probe`; read its evidence artifact (`bundle-log-acl-evidence-bf16bf6…`, artifact id 10834284814) and the `GATE:` lines to find which gate, fix, re-dispatch `windows-installer.yml` on the branch. Then open the PR. DBT-P74-002 (goal 3) not started. |
| `insight-model` | `lane/insight-model` `02a90ba`; `wip/insight-model` `9812648` | none | 2 commits; the llama.rs rewrite (shared decode loop, grammar, insight parsing) is in the WIP commit | finish llama.rs, the selector fallback-after-gate fix, one shared model, the timeline-pattern mislabel fix, tests (real-model insight test printing wall time); PR; quote Windows wall times from CI log (DBT-P62-004) |
| `plan-journal` | `lane/plan-journal` = `main`'s old base; `wip/plan-journal` `77e1bb1` | none | atomic `transition_plan_with_journal` + callers + streaming watcher changes, all in the WIP commit; the agent was starting the full workspace test | run the red-before trigger tests, the full test, split into commits, seal, PR |
| `keygen-rng` | `lane/keygen-rng` `599d646`; `wip/keygen-rng` `5a74e33` (adds the lane doc) | none | 4 commits: OS CSPRNG (BCryptGenRandom via local extern), refuse overwrite, honest permissions, tests; red-before measured | verify the full workspace build/test, fold the lane doc in, PR |
| `fs-acl` | `lane/fs-acl` `23f7169`; `wip/fs-acl` `ee842f1` | none | red-before **measured on Windows**: CI run `36057784591` at `23f7169` fails the 3 new tests in `tests/filesystem_posture.rs` (as intended); the ACL implementation in `filesystem.rs` is in the WIP commit | finish the DACL walk, make the 3 tests pass, CVE not-available, firewall registry, PR |
| `cli-truth` | `lane/cli-truth` `56141ec`; `wip/cli-truth` `f279adf` | none | 6 commits (export signer fingerprint/trust, update verify, vulndb atomic, --format both, care prefix, …); the `--lang ar` wiring is in the WIP commit | finish i18n/render wiring (English output byte-identical), tests, PR |
| `telemetry-windows` | `lane/telemetry-windows` `15fbaed`; `wip/telemetry-windows` `4482cab` | none | 1 commit; sampler stop/start double-thread fix red test + lib.rs fix in the WIP commit | the rest of the lane list (DBT-P49-004, DBT-P47-003, GPU per-adapter, fault counters, throttle, disk capacity, % idle) |
| `diagnosis-evidence` | `lane/diagnosis-evidence` `0fd2af9` | none | 1 commit: fixes 1+4 (failed Event Log read → unavailable card; Ready vs faults) | fixes 2, 3, 5, 6, 7 with red tests; PR |

Not started (Wave 2, see `AMBITION.md` §3): `installer-ux`, `care-consent`, `gate-honesty`,
`ui-truth`, `service-host`, `update-trust`, `db-diagnostics`, `fuzz`, `seal-root` (last). Wave 3 in
§4. `installer-ux` must wait for `service-rollback` (same files); `care-consent` for `plan-journal`
and `insight-model`; `ui-truth` for `insight-model` (catalogs); `gate-honesty` after Wave 1.

Open, not ours: PR #20 (Dependabot TypeScript 7) — belongs to the dependency lane (`DBT-P70-005`).

## Merge protocol (one PR at a time; `main` is never red)

For the next PR in line:
1. `git checkout lane/<name> && git merge origin/main`. `MANIFEST.sha256` will conflict — never
   hand-merge it: take either side, then re-seal (step 3).
2. Move the lane's ledger rows in `docs/LEDGER.md` on this branch (status cell + evidence with run
   ids; new rows get ids `DBT-P75-001`, `-002`, … in merge order), and bump "Last moved". Use the
   lane's `docs/phase75/lanes/<name>.md` as the source.
3. `python3 scripts/source_seal.py --json` (every failing path must be one this merge changed) →
   `git add <explicit paths>` → `python3 scripts/regenerate-source-manifest.py` →
   `git add MANIFEST.sha256` → `python3 scripts/source_seal.py` prints OK → commit, push.
4. Wait for `ci.yml` green on the PR with head_sha == the pushed head (use `workflow_dispatch` on
   that sha if a run was cancelled by concurrency).
5. Squash-merge. Confirm the `main` push run is green at the merge sha before merging the next PR.

## Rules that must survive the handoff

Everything in `LANE_CONTRACT.md` (invariants, traps, seal, local proof, CI), and from the P75 brief:
never widen `ASSISTANT_DEADLINE` (20 s); the pipe DACL `(A;;FR;;;AU)(A;;0x00000002;;;AU)` is
deliberate and hex `0x00120003` is forbidden; the hardener re-owns `C:\ProgramData\AetherCore` to
SYSTEM; never weaken a gate or edit an expected value; lockfile changes only in the dependency lane
with the freeze minted by `windows-installer.yml` on that branch (`docs/phase70/P70-REPORT.md` §3);
never `git add -A`, never force-push, never merge red. Windows behaviour is proven on the
`windows-2025` runner, never asserted. At the end: `docs/phase75/P75-REPORT.md` per `AMBITION.md` §6.

---

# Continuation — state at 2026-09-26 (second lead session)

One lane at a time locally; no subagents. `main` = `a013de5` (#34 merged) and **RED** on
`t4`/`t5` in `intelligence-core/tests/adversarial.rs` (run `36206362980`): with the real model the
2-vCPU runner is too slow for t4's "finish in 10 s" and t5 overran by one token step. #36 fixed
the 1.4 s prefill overrun but predicted each step from the previous one only. **PR #38
(`lane/insight-budget`) is the fix-forward — merge it before anything else.** Real-model timing on
the runner is the weak spot of this phase: if #38 is not enough, the next step is a per-phase
maximum step time, not a wider deadline.

**Ledger merges:** every lane edits `docs/LEDGER.md`, so `git merge origin/main` conflicts on the
"Last moved" line AND, when two lanes add rows at the same place, on the rows. Never resolve by
replacing hunks: rebuild from `origin/main`'s file plus exactly the rows the lane changed since the
merge base (the script used is reproduced at the end of this file).

## Merge train (one PR at a time; each needs `main` merged in, which conflicts only on
`docs/LEDGER.md` "Last moved" and `MANIFEST.sha256`, then a fresh CI at the new head)

| PR | lane | head at writing | PR CI | notes |
|---|---|---|---|---|
| #25, #26, #27, #29 | no-egress, mac-clippy, keygen-rng, insight-model | merged | green | #29's main run `36192104951` |
| #28 | fs-acl | merged `9e0b755` | `36192190898` green | fixture made user-owned (runner user is RID 500, elevated → owner is Administrators; `DBT-P75-008` opened); icacls order proven in throwaway run `36185665074`; firewall test moved behind `#[cfg(test)]` for `enterprise-adversarial-audit` |
| #30 | cli-truth | `d4ffac8` | `36206404288` green (needs main again after #38) | exit-code change 5→8 flagged for the owner (`DBT-P75-016`) |
| #31 | diagnosis-evidence | `6f47c6c` | green pre-#29 (`36187290070`) | Kernel-Power XPath measured in throwaway run `36186229655` |
| #32 | plan-journal | `f017f7c` | green pre-#29 (`36187928668`) | replayed onto main; `DBT-P75-027` open (needs a dev-dependency) |
| #33 | telemetry-windows | `8d6b46d` | `36189280509` | only lifecycle + watchdog; the Windows counter items are not started |
| #34 | service-rollback | merged `a013de5` | `36202794187` green | installer probe red `36184401229` / green `36184392149`; `DBT-P74-002` not started |
| #35 | db-diagnostics (Wave 2) | `707d735` | failed only on t5 | 7 red-before tests |
| #36 | insight-deadline | merged `883e0f4` | `36198857596` green | fixed the prefill overrun |
| #37 | update-trust (release-authority part) | `lane/update-trust` | pending | DBT-P75-035..037 |
| #38 | insight-budget | `lane/insight-budget` | pending | **fixes main red on t4/t5; merge first** |

Ledger ids assigned: `DBT-P75-006`…`034` (see each lane doc under `docs/phase75/lanes/`).

## Measurements that change earlier beliefs

* Windows insight generation does not fit 10 s on the 2-vCPU runner: `deadline exceeded after
  768 of 927 prompt token(s)` at 11.1–11.4 s. The budget overshoots by up to 1.4 s because the
  deadline is checked between prompt chunks (`DBT-P62-004`, OPEN). Answers are badged
  `ruleFallback`.
* The pwsh COM wrapper in `windows-installer.yml` was the cause of the masked upgrade gate, not
  the product; the harness now runs a standalone script (proven under pwsh 7.6.6 and 5.1).
* Throwaway branch `probe/p75-com` (push-triggered, ~2 min on windows-2025) is the cheap way to
  measure a Windows fact before spending an 80-minute CI run. Delete it when P75 closes.

## Not started

Wave 2: installer-ux (after service-rollback), care-consent (after plan-journal), gate-honesty,
ui-truth, service-host, update-trust, fuzz, seal-root (last). `P75-REPORT.md` not written.

## Ledger merge resolver (run from `phase21-workspace/` during the merge; arg = lane name)

```python
import re, subprocess, sys
lane = sys.argv[1]
P = 'phase21-workspace/docs/LEDGER.md'
show = lambda rev: subprocess.run(['git', 'show', f'{rev}:{P}'], capture_output=True, text=True, check=True).stdout
base_rev = subprocess.run(['git', 'merge-base', 'HEAD', 'origin/main'], capture_output=True, text=True, check=True).stdout.strip()
base, ours, main = show(base_rev), show('HEAD'), show('origin/main')
rid = lambda line: line.split('|')[1].strip() if line.startswith('| `DBT-') else None
rows = lambda text: {rid(l): l for l in text.split('\n') if rid(l)}
b, o = rows(base), rows(ours)
changed = [i for i in o if b.get(i) != o[i]]
out = main.split('\n')
for i in changed:
    k = next((n for n, l in enumerate(out) if rid(l) == i), None)
    if k is not None:
        out[k] = o[i]
        continue
    before = [n for n, l in enumerate(out) if rid(l) and rid(l).startswith('`DBT-P75-') and rid(l) < i]
    out.insert((before[-1] if before else max(n for n, l in enumerate(out) if rid(l))) + 1, o[i])
text = '\n'.join(out)
text, n = re.subn(r"Last moved: P75 \(2026-09-\d\d\), lane `[^`]+`\.", f"Last moved: P75 (2026-09-26), lane `{lane}`.", text)
assert n == 1, n
open('docs/LEDGER.md', 'w').write(text)
```
Then `git checkout --theirs MANIFEST.sha256`, `source_seal.py --json` (every failing path must be
the lane's own or the ledger), add them, regenerate, seal OK, commit, push.
