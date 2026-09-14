# P65 — the report

Host: the Mac at `/Users/hasanalaaa/dev/aethercore`. Date: 2026-09-14. Branch
`main` throughout. Every figure names the command or the run that produced it.
Where a claim is reasoning rather than measurement it says so, and this time
there are two.

The short version. **The runner is back and it told us we were wrong.** Run
`34827684392` executed all 23 steps' worth of job and stopped at **step 18 in
`phase11-design-audit.ps1:40`** — not at clippy, which **never ran at all**.
`DBT-P63-004` is confirmed closed *on the platform*: the runner printed
`main.rs=153 lines; router.rs=116 lines + 18 domain modules`. Four more walls
fell after it, each found by the one before. `ci.yml` now caches, and the
pre-wall half of the job went from **20m59s to 10m57s**. Dependabot stays paused.

---

## The correction that comes first

The brief said run `34827684392` "executed 25 windows steps". When this session
opened it, the run was **`in_progress`, 12 steps done, step 13 building**. The
brief also said the three P64 attempts "died at 0 steps in 3 seconds" — that is
right, and `34820243631` and `34819921518` are both in the list. The billing
block is genuinely gone. But the verdict did not exist yet; it arrived at
09:53:50 UTC, thirty minutes into this session. Everything below is from the
finished run.

---

## ITEM 1 — where the windows job actually stops, and what each step asserted

Run `34827684392`, `workflow_dispatch` at `117f46b`. **windows: failure. deny-check:
success.** Steps 1–17 green, step 18 red at 6m10s, steps 19–23 skipped.

The instruction was to record what each step **asserted**, not only its verdict,
because a step that measures nothing and exits 0 has been this project's
recurring failure. Each row below names the assertion and the evidence line from
the job log.

| # | step | what it ASSERTED | evidence |
|---|---|---|---|
| 1–7 | setup: checkout, rustup 1.97.1, node 22.16.0, pnpm 11.22.0, dotnet 8, ADK | the ADK asserts by ENUMERATION, not exit code — `dismapi.lib` must exist at the `KitsRoot10` default path or it throws | `info: downloading 5 components` (minimal ×3 + rustfmt + clippy); ADK 38s |
| 8 | Delivered source seal | every git-tracked file under `phase21-workspace` is listed in `MANIFEST.sha256` **and** its hash matches; also the only place `.gitattributes` eol rules are checked on Windows, because it hashes the WORKING TREE | `Source seal: OK - 1486 of 1486 tracked files verified` |
| 9 | Verify approved dependency freeze | `Cargo.lock` + `pnpm-lock.yaml` hashes equal `release/dependency-locks.sha256`, and 57 manifest files equal `release/dependency-manifests.sha256` | `Dependency locks, manifests, tool pins, and freeze metadata match the approved Phase 9 baseline.` |
| 10 | Frozen dependency restore | the lockfile is not stale against `package.json`, and **85 lockfile entries** pass pnpm's supply-chain policy | `Verifying lockfile against supply-chain policies (85 entries)` — the number that later exposed `DBT-P65-005` |
| 11 | Rust formatting | `cargo fmt --all -- --check`: zero diff | silent, exit 0, 3s |
| 12 | fetch-embedded-model | the downloaded `.gguf` matches the SHA-256 in `assets/models/models.manifest.json` | hash compared against the manifest entry |
| 13 | Rust unit/integration tests | **135 binaries, 660 passed, 0 failed, 11 ignored** | summed from 135 `test result:` lines, 14m03s |
| 14 | UI accessibility/type gate | `svelte-check --threshold warning --fail-on-warnings` | `svelte-check found 0 errors and 0 warnings` |
| 15 | UI production build | `vite build` emits; 213 modules | `✓ built in 1.47s` |
| 16 | Locked Rust workspace check | `cargo check --workspace --locked` — the lockfile is not mutated by the build | exit 0, 6m23s |
| 17 | Platform-neutral invariants | `static_validate.py` | `"ok": true, "checks": 347, "failed": []` |
| 18 | Enterprise convergence gate | see the chain below | **FAILURE**, 6m10s |

### What step 18 asserted, in order, before it threw

`verify-enterprise.ps1 -SkipOnlineSupplyChain` runs `verify-phase16.ps1` **first**,
and that recurses down the whole phase chain. From the log's own narration:

```
Phase 6  hardware/crash diagnostic rule tests            passed
Phase 7  strict Svelte a11y/type diagnostics + build     passed
Phase 7  design/accessibility/localization invariants    passed
Phase 8  security boundary audit + hardened IPC tests    passed
Phase 8  full locked workspace                           passed
Phase 9  dependency freeze; principal/consent/identity   passed
Phase 9  authorization + filesystem regression tests     passed
Phase 9  locked workspace after production sanitation    passed
Phase 9  platform-neutral Phase 0-9 invariants           passed
Phase 10 Operation Kernel decomposition                  passed
Phase 10 modular typed Protobuf contracts                passed
Phase 10 persistent authenticated IPC v7                 passed
Phase 10 machine mutation supervisor integration         passed
Phase 10 transient telemetry / durable journal split     passed
Phase 10 renderer polling elimination + reconnect order  passed
Phase 10 service decomposition and production sanitation passed
  -> "Phase 10 architecture/source audit passed.
      main.rs=153 lines; router.rs=116 lines + 18 domain modules."
Phase 10 kernel/transport/contract/journal tests         passed
Phase 10 platform-neutral Phase 0-10 invariants          passed (347 checks)
Phase 11 design-system / fluid-interaction source audit  THREW
```

**`DBT-P63-004` is closed on the platform, not only on the Mac.** That printed
line is the runner's own arithmetic and it equals this host's hand-read exactly.

### Clippy never ran, and that is measured, not inferred

P64's single next action named `verify-enterprise.ps1:23` —
`cargo clippy --workspace --all-targets --locked -- -D warnings` — as "almost
certainly" the next wall, and labelled it a prediction. It is wrong, and in the
same way P63's was: it names a line that sits *behind* the real one.

```
grep -ci clippy  <the whole 13,889-line job log>   ->  2
```

Both occurrences are the `rustup toolchain install ... --component rustfmt,clippy`
line in step 3. `verify-enterprise.ps1:15` runs `verify-phase16.ps1`; line 23 is
the clippy call; the chain threw inside line 15. **`DBT-P63-010`'s 30 clippy
findings are still unmeasured on Windows** — three phases of predictions, zero
measurements.

---

## ITEM 2 — `DBT-P64-001`, split in two, and two more defects behind it

### (a) the determinism check stopped repairing its own subject

P64 filed this as an audit that "quietly corrects" the SBOM. It is worse than
quiet. Measured on a clean tree at `117f46b`:

```
run 1  ->  p29-sbom-count-consistency  FAILS  (600 vs 604)
run 2  ->  p29-sbom-count-consistency  PASSES
```

The gate **repaired its own subject**, so the staleness could never survive to a
second observation. Both generator runs now go to a `TemporaryDirectory` and the
delivered file is only ever read. `generate_sbom.py` gained `--out`; the default
is unchanged, so regenerating the delivered inventory is still the bare call.

Three consequences, each measured:

* `p30-delivered-source-seal-ok` no longer reports `hash:SBOM.cdx.json`. That
  seal had been measuring the audit's own side effect, exactly as P64 read it.
* `p29-sbom-generator-deterministic` is **new**. The generator's docstring has
  always claimed byte-identical regeneration and nothing asserted it — the old
  check compared component **counts**, which two different inventories can share.
* When the generator exits non-zero, both downstream gates are recorded **FAILED,
  not skipped**. A check that disappears with its input reads as a pass.

Negative controls run for all three: a timestamp injected into the output fires
`-deterministic`; a forced `return 9` fires all three; both revert clean.

### (b) the regeneration, in its own commit, manifest after

`b7d0194` — SBOM only, then `MANIFEST.sha256`, in that order. `DBT-P63-001` is
the commit that got that order backwards and cost run `34768701075` at step 8.

And then the regeneration itself turned out to be wrong twice.

**`DBT-P65-001`** — `generate_sbom.py` read `apps/ui/pnpm-lock.yaml`.
`git log --all -- apps/ui/pnpm-lock.yaml` is **empty**: that path has never
existed in any commit. The lockfile is at the workspace root. The missing-file
branch returned `[]`, so every inventory ever generated carried **zero** npm
components. `.github/dependabot.yml` already records the same fact for a
different consumer — "this pointed at `/phase21-workspace/apps/ui` ...
`pnpm-lock.yaml` and `pnpm-workspace.yaml` live one level up", `DBT-P59-001`.
The generator was never corrected alongside it.

**`DBT-P65-005`** — with the path fixed, npm came to 45. Step 10 of the runner
says **85**. The name class in the pnpm key pattern was `[^@'\s]+`, which
excludes `@`; a scoped package is `@scope/name@version`, so all 40 scoped
packages failed to match and were dropped. The old pattern matched exactly the
45 unscoped entries — the coincidence that made 45 look like a whole number.

```
packages: total = 85   scoped (@...) = 40   unscoped = 45
matched by the old regex: 45
```

Final: **604 cargo + 85 npm = 689 components**, and 85 now equals what pnpm
itself reports. The committed SBOM had been stale by eleven patch versions on 49
crates, missing 4 crates, and missing all 85 npm packages.

The general lesson, and the only reason the second one was caught: **an
inventory nothing cross-checks is a number, not a measurement.** The runner
printed an independent count and no one had ever compared the two.

---

## ITEM 3 — caching and concurrency, and what they were actually worth

`ci.yml` had no cache of any kind. `package-manager-cache: false` was its only
cache line and it *disables* one. No concurrency group either.

The design was decided by measurement, not by convention. Run `34827684392`:

```
steps 1-12 (checkout, toolchains, ADK, seal, freeze, pnpm, fmt)   184s total
  step 7   install-windows-adk                                     38s
  step 10  pnpm install --frozen-lockfile                           3s
  step 13  cargo test                                           14m03s
  step 16  cargo check                                           6m23s
```

So the cargo cache is the whole game, the ADK cache is 38 seconds, and **pnpm is
deliberately not cached** — 3s is less than a cache round-trip, and recording
that is cheaper than re-deriving it next phase.

**`restore` + `save` split, not `actions/cache`.** This job has failed at step 18
or later in every run since P59, and a plain `cache` step saves from a post-step
declared `post-if: success()` — it would have cached nothing, ever. Split, with
`if: always()` on a save placed last, the tree built by a run that later goes red
is kept.

**I got that right for cargo and then failed to apply it to the ADK**, which
shipped as a plain `actions/cache@v4` in `d1af6df`. Two runs proved it:

```
34831042672  key Windows-adk-deploytools-e76dcfae...  "Cache not found", install 50s
34834327695  same key, unchanged action.yml           "Cache not found", install 55s
```

Asked twice with the same key, missed twice — never written. Fixed in `c4e46e9`
with the same split, but **without** `always()`: the save sits immediately after
the install, so a plain step is the correct guard in both directions — skipped if
the install failed, and already finished before step 20 can fail.

### What it bought, cold run against warm run

| step | cold `34831042672` | warm `34834327695` |
|---|---|---|
| Restore Rust build cache | 0m01s (miss) | 1m32s (hit) |
| Rust unit/integration tests | 12m17s | **5m48s** |
| Locked Rust workspace check | 5m02s | **0m10s** |
| **steps 1–19, to the wall** | **20m59s** | **10m57s** |

**10m02s saved per run, 48%,** and that is net of the 1m32s restore. The cache is
**3.45 GB** (`Sent 3703740510 of 3703740510 (100.0%)`), well under the 10 GB
budget — the one thing `d1af6df` flagged as unmeasured is now measured. On the
warm run the save step correctly **skipped** on an exact key hit.

Two consequences stated rather than discovered later:

* `cancel-in-progress: true` applies on `main`. A push can cancel a run before it
  reaches the wall. For a verdict on one SHA use `workflow_dispatch` — which is
  what `34827684392` was. This session held its pushes for exactly that reason.
* **The step numbers moved.** "Step 18" has meant `verify-enterprise.ps1` for four
  phases. From `d1af6df` that is **step 20**, and `static_validate.py` is 18.
  Every earlier report's numbers refer to the old layout.

---

## The walls after the wall

Each fix exposed the next, and every one was found on this Mac after the runner
named the file.

**`DBT-P65-002` — `phase11-design-audit.ps1:40`.** The `<button>` sweep ran over
`$Text`, which includes `.css`. A `<button>` ELEMENT cannot be authored in a
stylesheet, so every match there is prose — and four were, all in comments, all
written **2026-09-05 in P48**: `feature-layout.css:952,953,954` and
`motion.css:8`. Seventeen phases old; nothing had ever reached it. Scoped to the
file types that can hold markup; the four checks above it keep `.css` because
`!important`, fixed-duration transitions and sub-12px type are CSS defects.
Confirmed fixed on the runner: `Phase 11 design-system source audit passed.`

**`DBT-P65-003`, filed not fixed.** P64 read `phase11-design-audit.ps1` as
"41 assertions, 0 failed, 0 unmeasured" and called it measured. That is true and
it is not the whole script. The 41 are helper calls; the **8 inline
`if (...) { throw }`** at `:28 :29 :30 :34 :35 :36 :37 :52` are counted nowhere —
not as assertions, not as unmeasured, and `skipped_constructs` is `[]`. P64's own
contract is "an assertion this tool skips is UNMEASURED, never passing"; inline
throws are skipped and reported as **nothing at all**. P64 hand-read phase10's
inline throws and trusted the scanner for phase11's. That is the entire gap.

**`DBT-P65-004` — `test-phase11-motion.ps1:12`,** two defects in one line, and
neither fix works alone. Measured with tsc 6.0.3, the version the lockfile pins:

```
--lib ES2022,DOM     (one argument)   -> compiles, emits both files
--lib "ES2022 DOM"   (one argument)   -> error TS6046, exactly CI's
--lib ES2022 DOM     (two arguments)  -> error TS6231, NOT CI's
```

So Windows delivered one argument with a space where the comma was. Quoting fixes
it. **Reasoning, not measurement, and it does not fully close:** `ci.yml` step 3
passes `--component rustfmt,clippy` through pwsh and rustup reports "downloading
5 components", which is correct. I cannot reconcile that with the reading above
and am not going to invent a mechanism that covers both. The input/output table
is what is measured.

The second half is `--ignoreConfig`. tsc 6 stops at option parsing, so **TS6046
hid TS5112** — "tsconfig.json is present but will not be loaded if files are
specified on commandline" — which this invocation earns by naming files
explicitly. The Windows log holds 1 TS6046 and 0 TS5112 for that reason. Fixing
only the quoting would have moved the wall by one error and cost another round
trip. Verified end to end here: tsc exit 0, both files emitted,
`phase11-motion-tests.cjs` prints `PASS` twice.

**The current wall: `phase12-localization-audit.ps1:12`** — run `34834327695`,
`test-phase12-localization.py`, 30/32 checks:

```
no_hardcoded_visible_english: apps/ui/src/components/NavigationRail.svelte: 'Ctrl /'
backend_literal_owned_text_coverage: 20 untyped ('detail'/'result_code') strings
```

**It reproduces identically on this Mac on a bare `python3` call.** It needed
zero CI round trips to find and no phase had ever run it. **Not fixed here, and
deliberately**: both failures are user-visible product text in an EN/AR catalog
of 1,678 keys. That is a content decision, not a mechanical defect, and it is not
one to take unilaterally at the end of a session.

### The map that makes the next phase cheap

Every Python gate the PowerShell chain invokes, run here:

| gate | here |
|---|---|
| `phase12-localization-audit.py` | PASS |
| `phase13-reliability-audit.py` | PASS |
| `phase14-scheduler-audit.py` | PASS |
| `phase15-security-audit.py` | PASS |
| `phase16-ga-audit.py` | PASS |
| `phase18-driver-authority-audit.py` | PASS |
| `static_validate.py` | PASS, 347 checks |
| **`test-phase12-localization.py`** | **FAIL 30/32 — the current wall** |
| `check-pipe-teardown-qualification.py` | BLOCKED — needs native teardown evidence from a Windows run |
| `sigma-evidence-integrity-test.py` | **UNMEASURED** — still running after ~25 min here; not a verdict |
| `sigma-master-full-app-ui.py` | **UNMEASURED** — not reached behind the above |

Two UNMEASURED rows, named as such rather than counted as passes.

---

## The pre-push scan, and the rows that are not FAIL

`python3 scripts/ps_marker_scan.py` before every push: **234 assertions, 0
failed, 3 unmeasured**, unchanged throughout. The three were read by hand rather
than reported as a number.

* **`$lockBaseline`, `$manifestBaseline`** (`freeze-dependencies.ps1`) resolve to
  `release/dependency-locks.sha256` and `release/dependency-manifests.sha256`.
  Replayed in Python against the committed baselines: **lock 2/2 match;
  manifest 57/57 match**, order-independent — the only difference is that
  PowerShell's `Sort-Object` is culture-aware and Python's is ordinal. None of
  P65's changed files is in either set. CI step 9 passed this at `117f46b` in 56s.
* **`$MutationLock`** (`verify-installer-security.ps1`) is
  `%ProgramData%\AetherCore\state\machine-mutation.lock` — an ACL and contention
  check on an **installed machine**, unmeasurable from source on any host. Read
  the call chain rather than assumed: `verify-enterprise` → `verify-phase16` →
  `verify-phase15:72`, gated behind `$InstallerLifecycle`, which `ci.yml` never
  passes. **Unreachable in CI.**

**phase10's inline throws, hand-read 2026-09-14.** P64 read four; there are
**seven**, and the one it omitted is the renderer sweep at `:154`.

```
:127  MutationWorkload::   DriverInstall SystemRepair Cleanup Startup            4/4 ok
:129  durable_mutation_released                                                        ok
:131  ReadWorkload::  DriverDiscovery RepairAssessment CleanupDiscovery
                      StartupDiscovery Diagnostics                              5/5 ok
:154  setInterval/clearInterval across apps/ui/src         0 hits               ok
:168  main.rs        = 153   (throws at >=220)                                  ok
:170  router.rs      = 116   (throws at >=220)                                  ok
:174  router/*.rs    =  18 modules (throws at <2)                               ok
:177  largest module = dispatch.rs at 258 (throws at >=260)                      ok
```

115 scanned by the tool, **7 hand-read** (not 4), and reading phase11's eight the
same way is what found `DBT-P65-002`.

---

## ITEM 4 — Dependabot

**Not resumed.** The condition in `.github/dependabot.yml` is *"RESTORE THE THREE
LIMITS TO 5 WHEN `ci.yml`'s WINDOWS JOB IS GREEN ON `main`."* It is not green.
Three runs this session, all failure:

```
34827684392  117f46b  step 18  phase11-design-audit.ps1:40
34831042672  ca5b25d  step 20  test-phase11-motion.ps1:12
34834327695  5da01e2  step 20  phase12-localization-audit.ps1:12
```

The wall moved three times and is still standing. P59, P60, P63 and P64 each
declined for the same reason and each was right.

---

## The trap that nearly caught this report

`DBT-P63-001` is the commit that regenerated `MANIFEST.sha256` **before** writing
the file it was adding, so the new file was never listed and run `34768701075`
died at step 8. Writing this report reproduced it exactly. The manifest is
generated from `git ls-files --cached`, so a new file that is not yet staged is
invisible to it:

```
python3 scripts/regenerate-source-manifest.py   -> 1486 tracked files
git add docs/phase65/P65-REPORT.md
python3 scripts/source_seal.py
  Source seal: FAILED - 1486 of 1487 tracked files verified, 1 problems
    unlisted     docs/phase65/P65-REPORT.md
```

Staged first, then regenerated: **1487 of 1487, OK.** The lesson `DBT-P63-001`
paid a CI round trip for is that the order is `git add` → regenerate → `git add`
the manifest, and it is worth writing down because knowing the rule was not
enough to avoid it.

## The ledger

`DBT-P64-001` **CLOSED**, both halves. `DBT-P63-004` **confirmed closed on the
runner**, which no previous phase could claim. `§3` row 6 — the billing block —
**CLOSED**; the repository is public and jobs start.

**Entered:** `DBT-P65-001` (SBOM lockfile path, CLOSED), `DBT-P65-002` (phase11
CSS-comment false positive, CLOSED), `DBT-P65-003` (`ps_marker_scan.py` does not
count inline throws, **OPEN**), `DBT-P65-004` (tsc `--lib` quoting + hidden
TS5112, CLOSED), `DBT-P65-005` (scoped npm packages dropped, CLOSED).

`DBT-P63-010` **still unmeasured** — clippy has now failed to run on Windows for
three consecutive phases that predicted it would be the wall.

---

## Commits

| commit | item |
|---|---|
| `a4060b2` | ITEM 2(a) — the determinism check writes to a temp path; `-deterministic` added; `DBT-P65-001` |
| `b7d0194` | ITEM 2(b) — the SBOM regenerated deliberately, manifest after |
| `d1af6df` | ITEM 3 — cargo + ADK cache, concurrency group |
| `ca5b25d` | `DBT-P65-002` — step 18's real wall, and the ITEM 1 measurement |
| `5da01e2` | `DBT-P65-004` — the comma and the error hiding behind it |
| `c4e46e9` | the ADK cache could never populate — my own reasoning, not applied twice |
| `04307f5` | `DBT-P65-005` — 40 scoped npm packages restored |
| this one | the report |

---

## The single next action

**`test-phase12-localization.py`, and it does not need CI.** It fails here,
identically, on `python3 scripts/test-phase12-localization.py`: 30/32, the
`'Ctrl /'` text node in `NavigationRail.svelte` and 20 untyped backend
`detail`/`result_code` strings. Both are product-text decisions — a keyboard hint
that may belong in the catalog or may belong outside it, and backend prose that
must either get typed message keys or a documented exemption. Decide those two,
then the chain moves again.

Two things to carry in:

* **The gates behind the wall are mostly Python and they run on this Mac.** The
  table above is the map. P63 and P64 each spent a phase per defect on ~50-minute
  round trips for failures that a bare `python3` call reproduces in seconds.
* **A gate's helper calls are not its assertions.** `ps_marker_scan.py` counts
  `Require-*` calls and says nothing about inline `if (...) { throw }`, which is
  where three of this phase's five defects lived. Until `DBT-P65-003` is fixed,
  reading a PowerShell gate means reading its throws by hand — and saying how
  many there were, because P64 said four where there were seven.
