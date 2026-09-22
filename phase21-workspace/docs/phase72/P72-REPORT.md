# P72 — landing P70's outputs, and what the freeze loop actually costs

Five items. Two of them (4 and 5) were questions the brief asked me to *decide*,
and in both cases the decision came out against the framing the brief inherited:
one row's premise is a false negative, and one row's feared trade-off does not
exist. Item 3's framing was also wrong, and the measurement says so.

Ground rules as always: a claim without a command that produced it is a belief,
and where a measurement contradicts the brief the measurement wins. Three
sections below do exactly that.

---

## 0. What this host can and cannot do

Unchanged from P69/P70 and re-confirmed here rather than assumed:

* `DEVELOPER_DIR=/Library/Developer/CommandLineTools` makes `git` work.
* `cargo` runs: `cargo 1.97.1 (c980f4866 2026-06-30)`, `clippy 0.1.97`. Argument
  parsing, metadata and clippy-on-libraries work. **Nothing that links runs** —
  the unaccepted Xcode licence makes `cc` exit 69 — so no verdict here rests on
  a build.
* `pnpm 11.22.0` is present and **exactly matches** `package.json`'s
  `packageManager` pin, which matters for Item 2.
* `python3` runs the gates. Run them **by name**; never glob `scripts/*.py`.

---

## 1. Item 1 — P70's outputs, landed

Commit `40120d9` on `main`, plus `dde8b4f` (see §1.2).

### The delta was verified, not assumed

The brief said "+5 rows, −0 … verify that rather than assume it". The first
`git diff --stat` against local `HEAD` showed **12 insertions, 7 deletions**,
which looks like a contradiction and is not: local `main` was two commits behind
(`936a26d`), so the diff was carrying `664a778`'s own LEDGER edit as well. After
`git merge --ff-only origin/main` the delta is exact:

```
phase21-workspace/docs/LEDGER.md                | 5 +++++
phase21-workspace/scripts/verify-enterprise.ps1 | 2 +-
```

`git diff -- docs/LEDGER.md | grep -c '^+|'` → **5**. The companion
`grep -c '^-'` → 1, and that one match is the `--- a/…` diff header, not a
deleted row. So **+5 rows, −0** holds.

The five rows are `DBT-P70-001`…`005`, all `OPEN`, all deliberately unmerged by
P70. `--keep-going` lands at `scripts/verify-enterprise.ps1:23`.

### Ordering, and the seal

Staged with explicit paths, then sealed — in that order, because
`regenerate-source-manifest.py` defines the delivered set as **git-tracked
files** (`tracked_files()`; the docstring records `DBT-P59-003`, where a
directory walk swept in build outputs and a `.gguf`). Sealing before staging a
new file would miss it.

```
MANIFEST.sha256 regenerated over 1493 tracked files
Source seal: OK - 1493 of 1493 tracked files verified
```

`MANIFEST.sha256` moves exactly three rows: `docs/LEDGER.md`,
`scripts/verify-enterprise.ps1`, and the new `docs/phase70/P70-REPORT.md`.

### 1.2 One thing the brief did not mention

`docs/phase71/W1-WINDOWS-FIRST-SESSION.md` was **untracked**. It is the brief
that produced `W1-REPORT.md`, which `664a778` committed. Untracked means
unsealed — the manifest is tracked-files-only by construction — so it was
invisible to the seal and would have shown as `git status` noise for every later
session.

Committed separately as `dde8b4f`, deliberately **not** folded into the P70
commit: the brief enumerated three things for Item 1 and that commit contains
exactly those three. No content change; the file is committed as written.
`Source seal: OK - 1494 of 1494`.

---

## 2. Item 2 — three merges, measured

The brief asked for two numbers nobody had: how long a full cycle takes
wall-clock, and how many runner-hours three merges cost. Both are below. The
more useful finding is a third one nobody asked for: **the freeze set is
reproducible off-runner to the byte, so the runner time is not buying the
freeze — it is buying criterion 3.**

### The loop, and why the shortcut was rejected

A freeze set is a pure function of the committed tree. `-Refresh` has not
re-seeded since P60 (`freeze-dependencies.ps1`): it verifies the committed locks
resolve, then writes baselines derived from committed bytes. Everything it
writes can be computed on this Mac, and was.

`docs/RELEASE_SUPPLY_CHAIN.md` nevertheless rules the shortcut out, and it is
right to. A developer workstation **fails criterion 2** (no durable third-party
record naming the source commit) and **cannot satisfy criterion 3 at all** —
"the graph it produces is compiled on the machine that produced it", and nothing
compiles here (`cc` exits 69). Local computation is therefore only ever
**criterion-1 reproduction**, never minting. The dispatch loop stays; what the
reproduction removes is the need to *trust* the runner.

### Byte-exact reproduction, twice

For each PR the minted artifact was downloaded and compared against the set
computed here. All five files — `Cargo.lock`, `pnpm-lock.yaml`, and the three
`release/dependency-*` files — were **byte-identical**, both times:

| | mint run | artifact | result |
|---|---|---|---|
| #14 | `35669406613` | `dependency-freeze-7ba4679…` | **5/5 identical** |
| #15 | `35673467332` | `dependency-freeze-3d60dc42…` | **5/5 identical** |
| #17 | `35676496648` | `dependency-freeze-cd14d2ef…` | **5/5 identical** |

**#15 and #17 are the stronger results.** #14's `pnpm-lock.yaml` came from
Dependabot unchanged, so its match proves only that nothing corrupted it. #15's
and #17's were **regenerated here on macOS** by
`pnpm --dir apps/ui install --lockfile-only` under the pinned pnpm 11.22.0 — and
`windows-2025` produced the same bytes both times. pnpm resolution is
deterministic across the two hosts under the pin. That is criterion 1 in its
strongest form: two hosts sharing only the pinned manifests, same bytes — and
**15 of 15 files across three independent mints.**

The byte format had to be got right first, and was proven rather than assumed:
`.gitattributes` sets `* -text`, and these files are committed **CRLF** because
PowerShell `Set-Content` wrote them. Regenerating main's own
`dependency-locks.sha256` from main's lockfile hashes with CRLF reproduces
`83b2912a…` exactly — main's committed `lock_baseline_sha256`.

### Wall-clock per cycle

| | #14 | #15 | #17 |
|---|---|---|---|
| local merge, resolve, seal | ~9 min | ~3 min | ~3 min |
| PR CI run | 48.5 min | 46.8 min | 50.8 min |
| PR `windows` job | 48m17s | 46m33s | 50m45s |
| mint run — **parallel** | 33.4 min | 35.6 min | 30.9 min |
| freeze reproduced locally | 5/5 | 5/5 | 5/5 |
| push → CI green | 48.5 min | 46.8 min | 50.9 min |
| push → merged | **52.9 min** | **48.4 min** | 71.2 min\* |

\* #17's 71.2 min is **my latency, not the pipeline's**: CI went green at
02:29:09 and I merged at 02:49:23, having been held up by a tool-classifier
outage and a monitor re-arm. The honest cycle number is *push → CI green*, and
across three cycles that is **48.5 / 46.8 / 50.9 min — a mean of 48.7.**

**The mint is free in wall-clock.** Both it and the PR's CI trigger off the same
push — they started one second apart — and the mint (~34 min) finishes well
inside the CI window (~47 min). The binding constraint is a single ~47-minute
Windows CI job. A cycle is `max(mint, CI)`, not their sum, so the brief's
implicit serial model over-estimates by about 40%.

### Where the runner time actually goes

The mint run's own step breakdown is the most useful number in this section:

| mint step | time |
|---|---|
| 10 `Create candidate dependency evidence` — **the freeze itself** | **0.8 min** |
| 11 `Build unsigned Windows Setup candidate` | **29.9 min** |

**The freeze computation is 48 seconds. The 30-minute build around it is the
criterion-3 tax.** That is not waste — criterion 3 exists because run
`34745535685` once uploaded a freeze describing a tree that did not compile —
but it is the number that decides the automation question. You are not paying
for the freeze; you are paying to prove a machine compiled the tree it
describes.

### Serialising is what costs, and it costs twice

**1. Each merge re-conflicts the rest, and escalates the conflict.** All four
PRs rewrite `lock_baseline_sha256`, so every merge puts the others into
CONFLICTING. Measured, not predicted: merging #14 took #15 from **3 conflicts to
5**, adding `pnpm-lock.yaml` and `dependency-manifests.sha256` — because all
three npm bumps edit the same `apps/ui/package.json` block. A derived-file
conflict becomes a real lockfile conflict, resolvable only by re-running pnpm.

**2. A merge can cancel the previous merge's main CI.** `ci.yml` sets
`concurrency: group: ci-${{ github.ref }}` with `cancel-in-progress: true` —
added deliberately in P65 and correct, since an obsolete run's verdict is
worthless. #14's main run (`35673142914`) was **cancelled at 53.4 minutes** when
#15 merged.

**I predicted #15's would be cancelled too, and it was not** — `35676426143`
completed `success` in 52.9 min, because cycle 3 ran long enough for it to
finish first. The rule is therefore narrower than it first looked: a main run is
lost only when the next merge lands **inside** the previous run's ~50-minute
window. At this cadence that is roughly a coin-flip per merge, and it gets worse
the more the loop is optimised — the faster the cycles, the more main verdicts
are thrown away.

Nothing shipped unverified by this: each PR's own CI passed on the exact merged
tree, and squash preserves that tree byte-for-byte — verified, `origin/main`'s
tree equalled the staged branch's tree both times (`117722e1`, `967ffc49`).
It is the *main-branch* verdict that is lost, which is what P65's comment
already warns about.

**3. Squash orphans a staged chain.** The repo's convention is squash (#13,
#19). Because a squash commit is not a descendant of the PR head, each
subsequent branch had to be re-staged against the new `main`. That was cheap
here only because the squashed tree was identical to the staged one, so every
computed freeze value stayed valid and was reused rather than recomputed. With
N bumps that re-staging happens N−1 times.

### Runner-hours for three merges

The second number the brief asked for. Windows minutes only — the `deny-check`
and five `cargo-fuzz` jobs are Linux and total roughly 45 min across everything
below, at a 1× multiplier.

| what | #14 | #15 | #17 | total |
|---|---|---|---|---|
| PR CI `windows` job | 48.3 | 46.6 | 50.8 | **145.7** |
| mint run | 33.4 | 35.6 | 30.9 | **99.9** |
| main CI after merge | 53.4 *(cancelled)* | 52.9 | ~50 *(in flight)* | **~156.3** |
| | | | | **≈ 402 min** |

**≈ 6.7 Windows-runner hours for three dependency bumps.** GitHub bills
Windows at a 2× multiplier, so ≈ **13.4 billable hours**.

Read that against what was actually being decided: three version strings in one
`package.json`. And note where it goes — only **1.7%** of it (3× 0.8 min) is the
freeze; **39%** is main-branch CI re-verifying, once per merge, a tree each PR's
own CI had already verified.

### Does this need automating before the next ten bumps?

**Yes, but not the part that looks expensive.** Automating the *dispatch* is
pointless — it is two commands and it runs in parallel with CI anyway. The cost
is structural:

* **Batch, don't serialise.** All three bumps merged into one integration branch
  in ~15 minutes of local work, `package.json` auto-merging every time because
  the version lines do not collide. One integration branch means **one mint and
  one CI** instead of three, no re-conflicts, no cancelled main runs, and no
  re-staging. For ten bumps the saving is not linear — it is the difference
  between ten cycles and one.
* The per-PR work that remains is mechanical and identical every time: merge,
  regenerate the lock with the pinned pnpm, recompute four hashes, reseal. That
  is worth a script; the numbers above are what justify writing one.

---

## 3. Item 3 — `--keep-going` on the runner: the brief's framing is wrong

> "P70's 4→9 crates / 20→52 errors was measured on macOS and all 52 are outside
> the Windows tree, so the count does not transfer; the scheduling behaviour
> does. Say which you observed."

**I observed neither, and the reason is that on a green tree there is nothing to
observe.** That is the finding, not an evasion.

### What the flag is

`--keep-going` is a real `cargo build` flag, documented as:

```
--keep-going    Do not abort the build as soon as there is an error
```

It is absent from `cargo clippy --help` but accepted by it — `cargo clippy -p
__no_such_crate__ --keep-going` fails on *package resolution*, which happens
after argument parsing, so the flag parsed. The runner agrees: step 21 ran
`cargo clippy --workspace --all-targets --locked --keep-going -- -D warnings`
and **succeeded**, which it could not have done had cargo rejected the argument.

### Why the scheduling behaviour does not transfer either

The flag's entire semantic is conditional on failure: it changes what cargo does
*when a unit errors*. With zero errors, cargo builds every unit either way, so
`--keep-going` and its absence are observationally identical.

**The Windows tree has zero.** Measured from run `35668706898`'s log, step 21
isolated (10 461 lines):

```
warning: lines: 0
error:   lines: 0
...
AetherCore Enterprise convergence gate passed.
```

Clippy compiled the whole workspace — `Compiling aethercore-windows-update`,
`aethercore-restore-point`, and the rest — and emitted **not one diagnostic**.
So there was no error to keep going past.

The brief assumed the count would not transfer but the scheduling would; in fact
both are unobservable on a green tree, and only the first half of that sentence
was right for the stated reason.

### The timing, and the false attribution I am not making

| step | run `35657020319` (ef0c361, **without**) | run `35668706898` (dde8b4f, **with**) |
|---|---|---|
| 13 Restore Rust build cache | 1.4 min | **3.8 min** |
| 21 Enterprise convergence gate | **20.1 min** | **11.2 min** |

Step 21 nearly halved. **That is not `--keep-going`, and claiming it would be
wrong.** The cause is visible one row up: `ef0c361` is the uuid bump, so it
changed `Cargo.lock`, which changed the Rust build-cache key — that run restored
a smaller cache (1.4 min) and clippy rebuilt more. `dde8b4f` changes only
documentation and one `.ps1`, so it restored a fuller cache (3.8 min) and clippy
had far less to do. The step-21 difference is cache warmth.

**What is established:** the flag is valid, accepted on the pinned 1.97.1
toolchain on `windows-2025`, and inert on a green tree. It costs nothing today
and pays only on the first run that actually has errors — which is the right
time to have added it, but it means this phase cannot show a number.

---

## 4. Item 4 — `DBT-P70-003`: keep `windows-core`, and do not merge #21

**Verdict: the row's premise is a false negative. The declaration is
load-bearing. Remove nothing; #21 must not be merged.**

### The grep is true and the conclusion drawn from it is false

Both halves of the row's measurement reproduce:

```
crates/windows-update/Cargo.toml:17:windows-core = "0.62.2"    # under [target.'cfg(windows)'.dependencies]
grep -rn windows_core --include='*.rs'  ->  0
```

The grep returns zero because **the dependency is consumed by proc-macro
expansion, which never appears in source text.** No `use` statement will ever
name it.

`crates/windows-update/src/execution_windows.rs` carries **four**
`#[windows::core::implement(...)]` attributes — lines 45, 57, 69, 81, on the WUA
download/installation callbacks. `windows-implement` **0.60.2** — the exact
version this `Cargo.lock` resolves — emits **50 absolute `::windows_core::`
paths** in the code it generates (48 in `src/gen.rs`, 2 in `src/lib.rs`):
`::windows_core::Interface`, `::windows_core::ComObject`,
`::windows_core::IUnknown`, `::windows_core::imp::WeakRefCount`, and so on. A
`#[proc_macro_attribute]` cannot use `$crate`, so it hardcodes the absolute
path, and that path resolves **only if the consuming crate has `windows_core` in
its extern prelude** — i.e. as a direct dependency. There is no override:
`pub fn implement` takes no crate-path argument, and the macro's own doc example
opens with `use windows_core::*;`.

### The control case, which is why this is measured rather than argued

`crates/restore-point/src/windows_impl.rs` uses `windows::core::w!` (`:178`) and
`windows::core::s!` (`:181`) and declares **only** `windows.workspace = true` —
no `windows-core`. It compiles. `w!`/`s!` are `macro_rules!`, they resolve
through `$crate`, and they need no direct dependency.

Repo-wide, **exactly two** crates use `windows::core::` macros and **exactly
one** declares `windows-core` — and it is the one using the *proc*-macro. That
correlation is the explanation, not a coincidence.

### Why #21 would break rather than merely churn

`Cargo.lock` resolves `windows 0.62.2 -> windows-core 0.62.2`, the same node the
direct declaration resolves to. So today `windows-update` has exactly one
`windows-core` and the macro expansion's types are the types `windows::core`
re-exports.

Taking the direct declaration to **0.100.0** while `windows` stays **0.62.2**
puts two `windows-core` versions in one crate. `#[implement]` would generate
impls of **0.100.0** traits for WUA interfaces
(`IDownloadProgressChangedCallback` and the other three) that come from
**0.62.2** — a type mismatch, not a warning.

**Falsifiable, and not yet falsified by a build.** #21's own CI (run
`35474426178`) died at **1m28s** on step 10 `Delivered source seal` and never
reached compilation, so this is reasoned from the pinned sources rather than
observed. It does not need a build to act on, because **the verdict is to change
nothing** — the risk of the recommended action is zero, which is exactly when a
source-level argument is sufficient.

P70's correction of its own brief stands and is re-confirmed: P62's
`Send`/`HANDLE` constraint lives in the `windows` crate (`Cargo.toml:87`), not
`windows-core`, so #21 does not touch the constraint it appears to touch.

---

## 5. Item 5 — `DBT-P49-003`: the `<Log>` escape route works, and the trade-off is false

**Verdict: the mechanism is proven and `Disable` is not needed. One on-machine
measurement is outstanding, so the row stays OPEN. `Bundle.wxs` is unchanged.**

P71 left this row OPEN because one escape route was undisproved — Burn's `<Log>`
element on `<Bundle>` — and framed it as a trade: disabling installer logging
costs the support-export story. I tested it the only way this host can, against
the **pinned WiX v6.0.2 source**, which is the method P71 itself used for the
Repair button.

### `Prefix` relocates the directory — it is not just a filename prefix

`src/burn/engine/logging.cpp` at tag `v6.0.2`, inside `LoggingOpen`: the prefix
is first run through `VariableFormatString` (`:163`), then branched on
`PathSkipPastRoot` (`:170`) under WiX's own comment —

> `// If the log path is rooted and has a file component, then use that path as is.`

A **rooted** prefix is split by `PathGetDirectory` into the logging base folder
(`:175`) and `PathFile` into the filename (`:178`). Only the `else` falls back to
`GetNonSessionSpecificTempFolder` (`:181`) — which is the `%TEMP%` the row is
about.

### The elevated child reaches the same code

`LoggingOpen` sets `wzPostfix = L".elevated"` for `BURN_MODE_ELEVATED` at
`:77-78` and is otherwise the identical path. So relocating the parent's log
relocates the elevated child's log with it — which is the whole point, since the
elevated write is the defect.

### The directory is created, so there is no first-install problem

With an `Extension` set, `LogOpen` takes the `PathCreateTimeBasedTempFile`
branch (`logutil.cpp`) rather than the branch that calls `DirEnsureExists`
directly — and that looked at first like a gap. It is not:
`PathCreateTimeBasedTempFile` (`pathutil.cpp`) calls
`DirEnsureExists(sczPrefixFolder)` itself before creating the file.

### Cross-check that this is the code that produced P71's evidence

The same function builds the name as
`%ls_%04u%02u%02u%02u%02u%02u%ls%ls%ls` = `<prefix>_YYYYMMDDhhmmss<postfix>.<ext>`.
With prefix `AetherCore`, postfix `.elevated`, extension `log`, that is exactly
`AetherCore_20260912125833.elevated.log` — the file P71 measured. The reading is
of the code that produced the evidence, not of a plausible neighbour.

### So the weighing the brief asked for resolves without a trade

```xml
<Log Prefix="[ProgramData]\AetherCore\logs\AetherCore" Extension="log" />
```

moves both the parent and the elevated log out of the unelevated user's `%TEMP%`
while leaving logging **fully enabled**. The support-export story is preserved,
not sacrificed. `Disable` is not on the table and never needed to be.

### Two honest limits — why the row stays OPEN and nothing was changed

1. **The DACL is unmeasured.** `DirEnsureExists` passes `NULL` security
   attributes, so the new folder inherits `%ProgramData%`'s DACL. That default
   gives non-admin `Users` read/execute plus create-file, rather than the
   `FullControl` they inherit in `%TEMP%` — which is what would remove
   `WRITE_DAC`/`WRITE_OWNER`/`DELETE` over an elevated-written file. **But this
   row's entire subject is the ACL, so an unmeasured ACL cannot close it.**
2. **Failure is silent.** If `LogOpen` fails, Burn calls `LogDisable()` and sets
   `BURN_LOGGING_STATE_DISABLED` with `hr = S_OK` (`logging.cpp:186-193`). A bad
   path therefore degrades to precisely the outcome the support-export story
   cannot accept, with no error.

`installer/wix/Bundle.wxs` is **unchanged**. This Mac cannot build or run the
bundle, and an installer edit that cannot be verified is what §6 forbids. The
row moves from *escape route undisproved* to **mechanism proven, one measurement
outstanding**: make the `<Log>` change on the Windows PC and measure the
resulting file DACL.

Source read: `logging.cpp`, `logutil.cpp`, `pathutil.cpp`, all at tag `v6.0.2`,
matching the `wix 6.0.2` pin in `.config/dotnet-tools.json`.
