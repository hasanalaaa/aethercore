# P61 — the report

Host: the Mac at `/Users/hasanalaaa/dev/aethercore`. Date: 2026-09-13. Branch
`main` throughout, except the freeze-refresh branch named in ITEM 3. Every figure
below names the command, the commit, or the run id that produced it; where a claim
is reasoning rather than measurement it says so.

The short version. **`ci.yml`'s windows job compiled and ran the test suite for
the first time in this repository's history**, and then stopped on a defect that
is genuinely the repository's: two `aethercore-cleaner` tests open a
`%ProgramData%` lock file that only the installer creates, so `cargo test
--workspace` cannot pass on any Windows machine the product has not been
installed on. **The two definitions of "delivered" are now one**, and the
instrument that held the second one went from unrunnable — killed at forty
minutes — to **20.4 seconds with `target/` at 36 GB in place**. Fixing it also
showed that **eight of its ten gates had not executed since P58**, dying on a
`.github/` path before their first check; they run now and six of them fail 108
of 862 checks, reproduced identically outside the clone. And **the seal P60 built
and proved could fail had no caller anywhere in the repository** — it has one now,
and it passes on Windows, which is the only place ITEM 4's line-ending change
could be checked.

---

## Every CI attempt, in order

| run | at | job | reached | outcome |
|---|---|---|---|---|
| `34752465110` | `f2ddec9` | windows | step 10 of 20 | FAILURE — the P61 baseline, `cargo test` cannot link `dismapi.lib` |
| `34752623583` | `a9f365f` | windows | **step 11 of 21** | FAILURE — `cargo test` COMPILES and RUNS; `DBT-P61-002` |
| `34753171422` | `0db46fe` (branch) | installer | see ITEM 3 | the freeze refresh for the llama pin |
| `34753596499` | `fb16613` | windows | **step 13 of 23** | FAILURE — same defect, two steps later; steps 8 and 12 are new and green |
| `34756098630` | `0b4b7b9` | windows | **step 13 of 23** | FAILURE — same defect; step 9 verifies the freeze re-approved for the llama pin |

No run reached green. `DBT-P61-002` is where it stops, identically in all three
attempts that got past the ADK — 5 test binaries, 17 passed, 2 failed, the same
two names and the same two line numbers — and stopping there is the outcome ITEM 1
asks for. The job went from step 10 of 20 to step 13 of 23, and the three steps
it gained are three assertions the repository was not making at all.

---

## ITEM 1 — `DBT-P60-006`, and what the job now says

### The ADK step: factored, not copied

`.github/actions/install-windows-adk/action.yml`, called by both workflows.
`a9f365f`.

The brief allowed either. Factoring won because the recipe carries two
corrections that cost a CI run each to find — that `/installpath` is invisible to
`KitsRoot10`, and that `3010` is success-pending-reboot — and a second copy is
exactly where those get lost. The pwsh body is `windows-installer.yml:40-83`
byte-for-byte, comments included; verified by `diff` on the two bodies with their
indentation stripped, which reported no differences.

Measured, not assumed:

```
run 34752623583  step 7  Run ./.github/actions/install-windows-adk  SUCCESS  47 s
                         dismapi.lib 30100 bytes  2024-11-16 13:23:08Z
run 34753171422  step 6  the same action, in windows-installer.yml  SUCCESS
```

The step asserts by enumeration rather than by exit code — it prints the library's
size and mtime because a zero exit that produced no file is a failure — and that
line is the evidence it ran.

### The defect it uncovered

`cargo test --locked --workspace` had never compiled on CI. It does now, and it
fails:

```
crates\cleaner\tests\coordinator.rs:188
  cleanup_uses_frozen_candidate_evidence_and_reports_partial_skips  FAILED
  cleanup safety validation failed: The system cannot find the path specified. (0x80070003)
crates\cleaner\tests\coordinator.rs:227
  cleanup_failure_after_deletion_barrier_requires_recovery_review   FAILED
  assertion failed: status.mutation_started
```

Traced to the line rather than guessed. `run_cleanup` (`crates/cleaner/src/lib.rs:814`)
calls the free function `acquire_cleanup_mutation_guard()`, which on Windows
reaches `MachineMutationGuard::try_acquire()` and opens
`%ProgramData%\AetherCore\state\machine-mutation.lock` with `OPEN_EXISTING`
(`crates/windows-foundation/src/lib.rs:142-202`). That file is installer-provisioned.
No runner has one, so `CreateFileW` returns `ERROR_PATH_NOT_FOUND` and the second
test never reaches `mutation_started` at all.

It has been invisible because `#[cfg(not(windows))]` makes the guard a no-op: the
same three tests are **3 of 3 ok in 0.14 s on this Mac**, measured today.

The defect is not the missing file. `CleanupEngine::with_platform` injects a fake
platform for every other effect in these tests, and this one effect bypasses it.
`DBT-P61-002`. Three other crates take the same guard the same way
(`system-repair:210`, `startup-manager:52`, `windows-update:417`); whether their
tests reach it is unmeasured and is not claimed.

### The TRAP — every gate step, whether it executed and what it asserted

A conclusion is not an assertion. This is what each step actually did.

**Steps that executed.** From run `34752623583` and `34753596499`, by the line
each one printed:

| step | what it asserted | evidence in the log |
|---|---|---|
| ADK | `dismapi.lib` exists under `KitsRoot10`, by enumeration | `dismapi.lib 30100 bytes 2024-11-16 13:23:08Z` |
| Delivered source seal | 1,456 tracked files hash to the committed manifest, on a **Windows** working tree | step 8 SUCCESS, `34753596499` |
| Verify approved dependency freeze | lock + manifest baselines, tool pins and freeze metadata all equal the approved set, and `cargo metadata --locked` resolves | `Dependency locks, manifests, tool pins, and freeze metadata match the approved Phase 9 baseline.` |
| Frozen dependency restore | the committed `pnpm-lock.yaml` installs unmodified under `--frozen-lockfile` | `Done in 3.5s using pnpm v11.22.0` |
| Rust formatting | nothing to reformat | no output — `cargo fmt --check` is silent when clean, and non-silent when not |
| Fetch and verify embedded model | the `.gguf` matches its pinned byte count and sha256 | step 12 SUCCESS, `34753596499` |
| Rust unit/integration tests | **5 test binaries, 17 passed, 2 failed** | counted from the log, not from the step's conclusion |

That last row is the point of the TRAP. The step's conclusion is `failure`, which
is honest — but "the tests ran" would not be. `cargo test` stops at the first
failing binary, so **5** of the workspace's binaries reported and the rest were
never built or run. Whatever those hold is still unmeasured.

**Steps that did not execute.** Ten in `34753596499`: `UI accessibility/type
gate`, `UI production build`, `Locked Rust workspace check`, `Platform-neutral
invariants`, `Enterprise convergence`, both Zenith gates, `Supply-chain audit`,
`Build unsigned candidate`, and the artifact upload. `skipped` — asserted nothing.

Two of them were measured on this Mac instead, and pass: `pnpm --dir apps/ui
check` (exit 0, `229 FILES 0 ERRORS 0 WARNINGS`) and `pnpm --dir apps/ui build`
(exit 0). `Platform-neutral invariants` does **not**: `static_validate.py` fails
28 of 347 checks here, and will fail on Windows for the same reasons — see
`DBT-P61-001`.

**A gate that was not running at all.** P60 built `scripts/source_seal.py`, proved
it can fail with ten cases committed failing first, and wired it into
`p30-delivered-source-seal-ok`. Nothing calls that check. `phase30-adversarial-audit.py`
is referenced only by `phase31`, which is referenced only by `phase32`, by
`phase33`, by `phase34` — and **nothing references `phase34`**; `_p33_final_seal.py`
and `_p33_part_a_seal.py` are referenced by nothing either. Grepped across
`scripts/`, `.github/` and `phase21-workspace/.github/`. `release.yml` runs
`phase35-adversarial-audit.py`, which does not chain down to `phase30`.

So the verifier P59/P60 spent a phase making capable of failing had no caller in
any workflow. It has one now — `ci.yml` step 8, first, because everything after it
trusts those bytes — and it is **green on Windows**, run `34753596499`.

---

## ITEM 2 — one definition of delivered

### The decision, and it is committed before the diff

`18f0ab9`, `docs/SOURCE_SEAL.md` § "One definition of delivered". The
implementing diff is `ac823b5`. That order was the requirement and it is what the
history shows.

**The decision: the seal's rule — every git-tracked file under
`phase21-workspace/` — is the project's single definition of delivered, and
`omega-evidence.py` adopts it by importing `source_seal.tracked_files` rather than
restating it.**

Re-measured on today's tree *before* deciding, not taken from P60:

```
walked (omega's rule)  1,469
tracked (the seal)     1,455
walked - tracked          14   exactly the files P60 named
tracked - walked           0
```

Three reasons, in the order they settle it:

1. **Only one of the two can be right about those 14, and the walk is wrong about
   all of them.** Nine Vite build outputs, a 1.1 GB downloaded model, a release
   `.zip`, three runtime SQLite files. The exclusion list that exists to catch
   exactly that caught none of them.
2. **The tracked rule has no list to maintain.** Nothing about `apps/ui/dist/` was
   unusual; it simply arrived after the list was written.
3. **A second implementation of one rule is a second rule.** They did not drift
   because someone changed one — they drifted because there were two. That is why
   both rows close in one commit.

**The difference omega genuinely has, named rather than accommodated.** Per-gate
write detection does not use the delivered set and must not: `run_repo_script`
asks "did this gate write anything", and anything a gate writes is by construction
untracked, so measuring it against the tracked set would answer "no" to every
gate, always. It keeps a full unexcluded walk — of the clone, which is now cheap
for the same reason everything else got cheap.

### What it cost to run, before and after

`/usr/bin/time`, on this machine, `target/` at **36 GB and left in place**:

```
P60   killed after 40 minutes, one clone at 30 GB, 29 GB of free space consumed
      70.7 s only with target/ moved aside
P61   20.409 s, 13 clones, exit 1
```

The fix is a scope rule, not a faster walk: `shutil.copytree(ROOT, clone)` became
`clone_delivered()` — 1,456 files, 55 MB — and the three `tree_state(ROOT)` calls
take that same set.

### `OMEGA-RB-003` is gone, and this time nothing was held aside

```
before (P60)  blockers  OMEGA-RB-002 OMEGA-RB-005 OMEGA-RB-006 ENV-PWSH ENV-WINDOWS
              — and only after substituting a manifest generated under omega's own
                walk rule and moving the three DBT-P60-003 files out of the tree
after  (P61)  blockers  OMEGA-RB-002 OMEGA-RB-005 OMEGA-RB-006 ENV-PWSH ENV-WINDOWS
              source_manifest_before.ok  true
              listed 1,456   deliverable_files 1,456
              source_tree_integrity.ok   true, 1,456 entries
              execution_integrity.ok     true, 13 runs
              clean_source_tree.ok       true
              nothing moved, nothing substituted, git status clean throughout
```

`DBT-P60-003`'s three `C:\ProgramData` SQLite files are still in the tree. They
are untracked, so they are no longer deliverable, so the catch-22 that made
`OMEGA-RB-003` unclearable by any route is gone. The row stays open on the defect
itself — a service writing a literal Windows path on POSIX — narrowed, not closed.

Held by `scripts/test_omega_delivered_set.py`, **5 of 5**: the set IS the seal's
set; build output, the `.gguf` and backslash paths are outside it; the clone is
repository-shaped, carries the seal, and holds the delivered set and nothing else.
Every case fails against the pre-change file, which has neither `delivered_files()`
nor `clone_delivered()`.

One live demonstration rather than an argument: `pnpm --dir apps/ui build` was run
on the delivered tree, exit 0, and `source_seal.py` reads **OK, 1,456 of 1,456**
afterwards. Stated precisely — this particular rebuild was reproducible, so the
old rule would not have broken on it either; what the measurement shows is that
the delivered set is 1,456 before and after while the walk still stands at 1,470.

### What fixing it made visible

The clone was never repository-shaped. P58 moved the workflows to the repository
root (`8888d31`) and `scripts/gate_reader.py` resolves `.github/...` against
`ROOT.parent`; a clone whose parent is a bare temp directory gives every gate that
reads a workflow a `FileNotFoundError` before its first check.

Measured, with the delivered-set fix in and the layout fix not yet:

```
8 of 8 clone-run gates  exit 1,  "no trailing JSON object"
   FileNotFoundError: .../aethercore-sigma-audit-2eknfitc/.github/workflows/ci.yml
```

With the layout fixed they execute, and report:

| gate | checks | failed |
|---|---|---|
| `static_validation` | 347 | **28** |
| `phase15_security` | 106 | **29** |
| `zenith_recursive` | 120 | **26** |
| `enterprise_adversarial` | 88 | **14** |
| `phase13_reliability` | 61 | **6** |
| `phase14_scheduler` | 63 | **5** |
| `zenith_adversarial` | 35 | 0 |
| `phase16_policy` | 42 | 0 |
| | **862** | **108** |

`phase12-localization` also exits 1. **Reproduced outside the clone**, run directly
against the workspace: identical counts for all six, so these are the tree's own
failures and not an artefact of the clone. Samples, to show the kind: `parse_json`
— a UTF-8 BOM in four `docs/phase36/evidence/*.json`; `path_dependencies` — `path =
"../operation-kernel"` inside `PHASE_20_BINARY_SAFE_PATCH/new-files/**`, which has
no sibling to resolve against; `phase4_ui` — the string `DEEP CLEANUP` is missing.

Filed as `DBT-P61-001`, not touched. Nothing regressed: these gates have been
unable to run since P58 and unreachable since before that.

---

## ITEM 3 — the llama-cpp signature: PINNED

### The decision

Pin. `0db46fe`.

Not caution — evidence. The two versions are both in this machine's cargo
registry, so they were diffed rather than reasoned about:

```
llama-cpp-sys-2 0.1.154/llama.cpp  vs  0.1.156/llama.cpp
   139 files differ
    21 files new in 0.1.156   (llama-kv-cache-msa.{cpp,h}, arm64-windows-msvc-cuda.cmake, ...)
     0 files removed
```

`llama_sampler_init_dry` lost an argument as well, and the doc comment on
`penalties` changed from "-1 = context size" to "Negative values are clamped to 0;
they no longer select the context size."

That is not a signature change. It is a different inference engine underneath a
product invariant. `DBT-P56-002` is NARROWED on a **measured** generation result,
and the one thing ITEM 3 rules out is retiring that measurement by assumption.

### The pin set

```
llama-cpp-2      = "=0.1.154"
llama-cpp-sys-2  = "=0.1.154"
```

The second is load-bearing. `llama-cpp-2` 0.1.154's own manifest asks for
`llama-cpp-sys-2 = "0.1.154"` — a caret — so pinning the binding alone leaves the
sys crate free, which is precisely what P60 measured when a from-scratch
resolution took it to 0.1.156 and `llama-cpp-2` 0.1.154 then failed against it
with three errors. A direct `=` requirement makes the two intersect to one
version. No feature change: the sys crate's `default = ["common"]` is already
enabled through `llama-cpp-2`'s own defaults.

`Cargo.lock` moves by **one line** — `llama-cpp-sys-2` joining
`aethercore-intelligence-core`'s dependency list. No version moves; read off the
diff.

### The baseline a future adoption has to re-measure against

Since the brief asks for a generation result if you adopt, here is the one it
would have to beat — measured because pinning is only defensible if the thing
being preserved is known:

```
cargo test --locked -p aethercore-intelligence-core --test embedded_generation
  5 passed; 0 failed   42.46 s   macOS/Metal, against the shipped .gguf
  re-run after the pin: 5 passed; 0 failed   41.95 s
```

Stated plainly: **that is a macOS measurement and this product ships on Windows.**
`ci.yml` did not fetch the model until `fb16613`, so nothing has ever measured
generation on the platform it runs on. An adoption of 0.1.156 needs that first.

### The freeze refresh

`release/dependency-{locks,manifests}.sha256` and `dependency-freeze.json` hash
every `Cargo.toml` in the workspace, so a two-line manifest edit invalidates the
approved freeze and `-VerifyOnly` throws until it is refreshed on a source that
meets `docs/RELEASE_SUPPLY_CHAIN.md`'s three criteria. That is why the pin went to
a branch first: run `34753171422`, dispatched on `p61-llama-pin`, **success**.
`-Refresh` verified the committed lockfiles under `--locked` and
`--frozen-lockfile` rather than re-seeding — `06baeb7`'s state machine holding —
and the freeze uploaded after the build, not before it.

Reviewed rather than taken on trust. Every figure reproduced on this Mac against
the artifact:

```
Cargo.lock      18c82ee5bfe5ca109444d87ada3ec97d9348f891dacc428c99fd1db042d14030  = committed
pnpm-lock.yaml  6bda309c4a483ece97cce68d34eeb281fc6c22fbd85ab235ae21e78faf08b746  = committed
57 of 57 dependency-manifests.sha256 entries recomputed against the committed
   blobs, 0 differing
dependency-locks.sha256's two entries        = the two lockfile digests
dependency-freeze.json's two baseline hashes = sha256 of the two baseline files
crates/intelligence-core/Cargo.toml's entry  = the PINNED file's hash
```

Both lockfiles byte-identical across a Windows runner and this laptop again,
which is `50cc466` still holding. Landed in `0b4b7b9`.

---

## ITEM 4 — `*.cmd` is executed, not captured

`a0cb426`. `.gitattributes` keeps `* -text` and gains `*.cmd text eol=crlf`, with
the distinction written into the comment: a **captured** file keeps the bytes it
was captured with — the 37 CRLF files under `docs/phase36/evidence/` are Windows
console output and rewriting them would falsify a measurement — and an
**executed** file gets the bytes its interpreter needs. The previous comment's
claim that leaving the `.cmd` files alone avoided "picking a side" was wrong: LF
is a side, and `scripts/build-arm64-msi.cmd:91-95` is five lines joined by a `^`
continuation, which is exactly where `cmd.exe` is unreliable with LF.

`eol=crlf` rather than `autocrlf`, and the difference is what keeps the seal
verifiable: the attribute is explicit and platform-**independent**, so every
checkout on every OS produces the same CRLF working tree. `autocrlf` produced CRLF
on Windows and LF elsewhere, which is the defect `50cc466` fixed.

Measured on this Mac:

```
after git add --renormalize + checkout
  scripts/build-arm64-msi.cmd        CRLF in the working tree
  scripts/p36vm/p36_relbuild.cmd     CRLF in the working tree
  index                              still LF, for both
  git status                         clean
  source_seal.py                     FAILED - 2 problems, naming exactly those
                                     two files as `hash`  <- the seal working
  after regeneration                 OK - 1,456 of 1,456
```

**And measured on Windows, which the brief said could not be done from the Mac.**
It is half right — `cmd.exe` still has not been run against either file, and that
remains unverified, on the ARM64 VM `DBT-P55-006` already waits for. But the
*delivered bytes* half is now checked where it matters, because ITEM 1 put the
seal into `ci.yml`:

```
run 34753596499  step 8  Delivered source seal
                 Source seal: OK - 1456 of 1456 tracked files verified
```

The manifest holds the CRLF hashes for those two files. A Windows checkout that
disagreed would fail that step by name. It does not.

What is claimed and what is not: that both files are delivered CRLF on Windows is
**measured**. That `cmd.exe` parses them more reliably for it is **reasoning**
from how `cmd.exe` handles `^` continuations, and it stays unverified until
someone runs them on that VM.

---

## ITEM 5 — Dependabot

**Not attempted.** `ci.yml`'s windows job is not green on `main`: run
`34753596499` stops at step 13 of 23 on `DBT-P61-002`. The condition is not
restated here.

---

## The ledger count, and the rule that produces it

The rule is now written into `docs/LEDGER.md` § 1, with the command that
reproduces it, because P60's report and an independent grep disagreed and neither
had written down what it was counting.

* **Scope**: § 1 only, between the `## §1` heading and the next `## §` heading.
* **A row**: a line starting `| ` that splits into exactly **five** cells whose
  first is a backticked `DBT-` id.
* **Malformed rows are reported, never dropped.**
* **Open**: the status cell, stripped of `*` and whitespace, begins with `OPEN`.
  `OPEN by design` counts. `ACCEPTED` and `CLOSED` do not.

| | rows | open | malformed |
|---|---|---|---|
| P59 end (`8416e89`) | 31 | 17 | 1 |
| P60 end (`f2ddec9`) | 35 | 19 | **3** |
| P61 end | **39** | **17** | 1 |

**Both halves of P60's disagreement are now explained rather than asserted.**
`38` is every `| \`DBT-` line in § 1 regardless of cell count — 35 well-formed
plus **3 malformed**, and P60's own 35 silently dropped two of them. `20` is
`grep -c '^| \`DBT-.*OPEN'`, which counts `DBT-P58-001` as open because the words
`status: OPEN` appear in its **evidence** cell while its status cell reads
`CLOSED`; reproduced, and the id it adds is named.

Two of those three malformed rows were a P60 editing slip — `| done || the
trusted freeze workstation |` and `| any || any |`, a duplicated machine column
that also renders wrong in any markdown viewer. Repaired to five cells.
`DBT-P56-002` stays four-celled, named rather than dropped, as in P58, P59 and
P60.

**Closed this phase:** `DBT-P60-001`, `DBT-P60-004`, `DBT-P60-005`, `DBT-P60-006`.
**Narrowed:** `DBT-P60-003` (it no longer blocks `OMEGA-RB-003`).
**Entered:** `DBT-P61-001` (108 of 862 gate checks fail on the delivered tree),
`DBT-P61-002` (the installer-provisioned lock two tests require).
Neither is a new defect; both were already true and became visible when something
was measured for the first time.

---

## What measurement contradicted in the brief

1. **"`ci.yml` has ten of them and P58 already found ten readers that failed
   open."** The count of gate steps depends on what counts as a gate; more
   usefully, the trap turned out to be worse than stated in one direction and
   better in another. **Better**: every `.ps1` wrapper `ci.yml` calls (`verify-enterprise`,
   both Zenith wrappers, `audit-dependencies`) `throw` on a non-zero child, so
   none of them fails open at the wrapper. **Worse**: the gate P60 built
   specifically so it *could* fail — `p30-delivered-source-seal-ok` — was not
   being run by anything at all, which no reader census would have found because
   the reader was fine and the caller was absent.

2. **"the two `.cmd` files ... cannot be tested from the Mac — say that plainly
   rather than claiming it works."** Half of it can, and now is. The delivered
   bytes are verified on Windows by the seal step, run `34753596499`. Only the
   `cmd.exe` execution remains untestable here, and it is reported as unverified.

3. **`omega-evidence.py`'s scope for `DBT-P60-004`.** The brief left it to
   judgement. Fixing it was not optional in the end: the delivered-set change
   cannot be shown to work without running the thing it changes, and running it
   is what surfaced `DBT-P61-001`. The fix is a scope rule, as the brief required —
   `clone_delivered()`, not a faster walk.

4. **ITEM 3's framing as "the llama-cpp signature".** It is not a signature. 139
   files of the vendored `llama.cpp` differ between 0.1.154 and 0.1.156 and 21 are
   new; `llama_sampler_init_dry` also changed and `penalty_last_n`'s negative
   semantics were redefined. Treating it as a four-argument-to-five-argument edit
   would have swapped the inference engine as a side effect. That measurement is
   why the decision is to pin.

---

## Commits

| commit | item |
|---|---|
| `a9f365f` | ITEM 1 — one ADK recipe, factored, called by both windows jobs. `DBT-P60-006` |
| `18f0ab9` | ITEM 2 — **the decision, before the diff** |
| `ac823b5` | ITEM 2 — one definition of delivered. `DBT-P60-004` + `DBT-P60-005` |
| `a0cb426` | ITEM 4 — `*.cmd text eol=crlf`, and the captured/executed distinction |
| `fb16613` | ITEM 1 — the model the tests need, and the seal nothing was running. `DBT-P61-002` filed |
| `0db46fe` | ITEM 3 — the pin set (on `p61-llama-pin`, cherry-picked to `main`) |
| `0b4b7b9` | ITEM 3 — the freeze re-approved for it. `DBT-P60-001` closes |

`p61-llama-pin` is still on the remote. It exists only to have carried the
dispatch and its content is on `main`; it was not deleted without being asked.

---

## The single next action

Route the machine-mutation guard through `CleanupPlatform` so a test platform can
supply its own, without weakening the real cross-process lease in production —
`DBT-P61-002`. It is the one thing between `ci.yml` and its next ten steps, and
until it moves, every pull request against this repository still fails on `main`'s
reason rather than its own.

Behind it, in the order the job will meet them: whatever the remaining test
binaries hold — only 5 of them have ever run — and then `Platform-neutral
invariants`, where `DBT-P61-001`'s 28 `static_validate.py` failures are waiting.
