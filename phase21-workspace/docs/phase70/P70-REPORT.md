# P70 — FREEZE: refreshed on a qualifying source, reproduced on a second machine

2026-09-21. Branch `main` at `936a26dc79c5e55e89fe94072d6f152a525aabf6`, plus
one Dependabot branch. No rewrite of `docs/RELEASE_SUPPLY_CHAIN.md` was needed
and none was made: the document already decided, at `:80`, that a GitHub-hosted
runner qualifies, and already names `windows-installer.yml` as what satisfies
criterion 3. This phase executed that decision rather than re-litigating it.

The phase produced one thing it did not set out to produce: a measurement that
says refreshing the freeze on `main` cannot unblock a single dependency bump.
That is §3, and it is the finding that outranks the rest.

---

## 1. What was run, and what each run is evidence of

| run | ref | head_sha | verdict | what it establishes |
|---|---|---|---|---|
| `35618987104` | `main` | `936a26d` | **success** | **criterion 2** for `main`'s freeze — a durable third-party record naming the exact source commit |
| `35619604606` | `…/uuid-1.26.1` (#19) | `70cdde6` | **success** | PR #19's freeze candidate, minted, reviewed and reproduced here |
| `35620937336` | `…/svelte-check-4.7.6` (#14) | `7cc81a5` | **success** | npm freeze candidate |
| `35620941644` | `…/svelte-5.57.0` (#15) | `1ddbd32` | **success** | npm freeze candidate |
| `35620946028` | `…/vite-8.3.0` (#17) | `6bb0375` | **success** | npm freeze candidate |
| `35620950722` | `…/typescript-7.0.2` (#20) | `1c372fd` | **failure at step 11** | `DBT-P70-005` — TS 7 is not a compatible bump, and criterion 3 caught it |
| `35484711267` | `main` (push) | `936a26d` | **success** | §6 — the verdict P69's own commit could not cite |

Every dispatch was `workflow_dispatch`, and each run's `head_sha` was read back
from the API and checked against the sha claimed for it before any result was
used. `windows-installer.yml` declares no `concurrency` group — only `ci.yml`
does, keyed on `github.ref` — so six dispatches ran in parallel without
cancelling one another.

## 2. The three criteria, measured for this freeze

### Criterion 1 — reproducible on an independent machine

`docs/RELEASE_SUPPLY_CHAIN.md` says a developer workstation *"becomes acceptable
the moment it is the second machine"*. This Mac was made the second machine.
**Every artifact reproduced.** This is criterion 1 measured for this freeze, not
inherited from 2026-09-13.

| measurement | result |
|---|---|
| `cargo metadata --locked` on the committed `Cargo.lock` | **exit 0 in 1.2 s**, lock bytes unchanged |
| `pnpm --dir apps/ui install --lockfile-only --frozen-lockfile` | **exit 0**, `pnpm-lock.yaml` unchanged |
| `release/dependency-locks.sha256` — 2 entries recomputed | **2/2 match** |
| `release/dependency-manifests.sha256` — 57 entries recomputed | **57/57 match** |
| manifest **file set** rediscovered from the script's own globs | **identical**, 57 = 57, no file found that is not listed and none listed that is not found |
| `release/dependency-manifests.sha256` self-hash vs `dependency-freeze.json` | `58462aa3…64d71a` **match** |
| `release/dependency-locks.sha256` self-hash vs `dependency-freeze.json` | `9ddfb1b8…6816b0` **match** |
| pinned tools | `rustc`/`cargo` **1.97.1** = `rust-toolchain.toml`; `pnpm` **11.22.0** = `package.json`'s `packageManager` |

Reproduced on **aarch64-apple-darwin** against a freeze minted on
**windows-2025** — different architecture, different OS, different filesystem
case-sensitivity, and (per `DBT-P58-001`) different line-ending handling, which
`.gitattributes`' `* -text` is what makes the byte comparison meaningful at all.

**One experiment was run and is reported as the wrong experiment**, because
getting this wrong is how a phase manufactures a false finding. Deleting
`Cargo.lock` in a scratch copy and running `cargo generate-lockfile` produced
`4c2f8e4d…` against the committed `18c82ee5…` — **117 differing package
blocks**. That is not host divergence, it is **index drift over time**:
`bitflags` 2.13.1→2.13.2, `camino` 1.2.5→1.2.6, `cc` 1.4.4→1.4.7, `cfg-if`
1.0.4→1.0.5, plus new `base64` 0.23.1, `core_detect` and `miniz_oxide` 0.9.1
nodes. Every directly-pinned dependency held its version — `llama-cpp-2` stayed
0.1.154, `criterion` 0.5.1, `windows-core` 0.62.2 — which is what proves the
diff is the registry moving under a from-scratch resolution and not the
manifests resolving differently here. **A from-scratch resolution is not a
reproducibility test and must never be cited as one**; the test is whether the
same committed inputs yield the same evidence, and they do, 59 hashes out of 59.

### Criterion 2 — a durable third-party record naming the exact source commit

Run **`35618987104`**, `workflow_dispatch` on `main`, `head_sha`
**`936a26dc79c5e55e89fe94072d6f152a525aabf6`**. Step 10 *Create candidate
dependency evidence* — the `-Refresh` — **success**.

### Criterion 3 — the graph is compiled on the machine that produced it, before the output leaves it

Satisfied by construction in `windows-installer.yml`, unchanged by this phase.
Step 10 runs `-Refresh`, step 11 builds, step 12 uploads. The handoff phrased
this as *"let `-Refresh` run after the build step"*; the file does the opposite
and the document is right, not the handoff — what must follow the build is the
**upload**, and it does. The workflow's own comment records why: run
`34745535685` uploaded a freeze and then failed to compile the tree that freeze
described.

## 3. The finding: refreshing the freeze on `main` unblocks nothing

**What P69 already knew, credited so this does not read as new**: its §6.2 says
*"the approved dependency freeze blocks every cargo and npm bump at step 11, and
no amount of re-sealing helps"*, and gives the reason —
`release/dependency-locks.sha256` hashes the two lockfiles directly. This
section adds the half that was not known: **refreshing the freeze does not help
either, unless it is refreshed on the bump's own branch**, and the mechanism is
in the script.

**`-Refresh` does not regenerate lockfiles when lockfiles are committed.**
`scripts/freeze-dependencies.ps1:99-112` branches: if `Cargo.lock` and
`pnpm-lock.yaml` both exist it **verifies** them (`cargo metadata --locked`,
`pnpm install --lockfile-only --frozen-lockfile`) and re-derives the baseline
hash files from what is already on disk. It resolves from scratch **only in the
seed state**, which is the correction `06baeb7` made after `DBT-P60-001`.

Two consequences follow, and the second is the one that matters.

1. **The refresh on `main` is a content no-op.** It re-derives the three
   `release/` files from lockfiles that did not change, so it reproduces bytes
   that are already committed. Its entire value is criterion 2 — it is the
   durable third-party record that `main`'s freeze was minted on a qualifying
   source, replacing a provenance inherited from the 2026-09-13 workstation run.
   That is worth having and it is all it is.

   **Confirmed empirically, not merely predicted from the source.** Run
   `35618987104`'s uploaded artifact was downloaded and compared against the
   committed tree: **all five files byte-identical** — `Cargo.lock`,
   `pnpm-lock.yaml`, `release/dependency-locks.sha256`,
   `release/dependency-manifests.sha256`, `release/dependency-freeze.json`. A
   refresh on `main` reproduces `main`. That also makes this freeze
   independently reproduced **three times** now: the 2026-09-13 run, this Mac
   today, and a fresh hosted runner today.


2. **It does nothing for any open bump.** PR #19 changes `Cargo.lock`
   (`uuid` 1.25.0 → 1.26.1). Its lock hash therefore differs from the approved
   `release/dependency-locks.sha256` **regardless of how recently `main`'s
   freeze was refreshed**, because the baseline is a hash of `main`'s lock, not
   a property of the resolver. The same holds for #14/#15/#17/#20, which move
   `pnpm-lock.yaml` — also a baselined file.

**So the freeze must be refreshed on each bump's own branch, and that refresh
must be committed there.** The loop P69 documented is therefore not four steps
but six:

```
1. bring the bump branch up to date with main
2. python3 scripts/source_seal.py --json      # read BEFORE re-sealing
3. dispatch windows-installer.yml on the BUMP BRANCH; download the freeze set
4. review and commit release/dependency-{locks,manifests}.sha256 + dependency-freeze.json
5. python3 scripts/regenerate-source-manifest.py   # re-seal LAST: step 4 changed tracked files
6. CI green -> merge
```

Step 5 must come after step 4, not before it as P69's two-command form implies,
because the three `release/` files are git-tracked and the seal covers every
git-tracked file under `phase21-workspace/`. A branch re-sealed before its
freeze is committed is red again the moment it is.

## 4. #19 — who is allowed to commit a freeze

**PR #19's only failure was step 11.** Run `35484566311` at `70cdde6`: step 10
*Delivered source seal* **success** (P69's loop did its job), step 11 *Verify
approved dependency freeze* **failure**, steps 12–26 **skipped**. `deny-check`
and all five `fuzz` legs pass on that head.

This phase first stopped here, reading `docs/RELEASE_SUPPLY_CHAIN.md` as
forbidding the commit that would clear step 11. **That reading was wrong, and
the document is what disproves it.** The sentence is at **`:33`**, not `:37`,
and it scopes its constraint to **workflows** — `ci.yml` and `release.yml` are
verify-only, `windows-installer.yml` runs `-Refresh`, a refresh mints a
candidate and never an approved graph, approval is the commit, and no workflow
in this repository can approve a dependency graph *because no workflow can
commit one*.

The clause carries its own reason, and the reason is mechanical: a CI job cannot
commit. It is a statement about what a workflow cannot do, not a prohibition on
tooling in general. The paraphrase this phase started from — "no automation may
commit this" — is broader than the text, and broad enough to forbid the thing
the repository already does.

**The precedent settles it, and it was verified rather than taken on trust.**
`d53ec168dc5f`, the commit that closed `OMEGA-RB-001`, is authored and committed
by `hasanalaaa`, with the message *"feat(p60): land the Phase 9 dependency
freeze — OMEGA-RB-001 closes on evidence"*, and carries exactly five files:

```
phase21-workspace/release/dependency-freeze.json
phase21-workspace/release/dependency-locks.sha256
phase21-workspace/release/dependency-manifests.sha256
phase21-workspace/release/dependency-freeze.blocker.json   (removed)
phase21-workspace/MANIFEST.sha256
```

That is the commit `docs/RELEASE_SUPPLY_CHAIN.md` was written around, and
`DBT-P58-001` cites it as the evidence that `-VerifyOnly` passes for the first
time in this repository's history. Writing these bytes is a mechanical act the
project has already performed in exactly this shape.

**Where approval actually sits is the merge.** The owner reads the PR diff and
merges it; that is the risk acceptance the document describes when it says
whether the project accepts these dependencies belongs to the owner and happens
when he commits the files. Minting bytes is not accepting them. **So this phase
commits the freeze on each bump's own branch and merges nothing.**

### What was done to #19

1. **Brought up to date with `main`.** `70cdde6` was one commit behind
   (`936a26d`); head is now **`2b67644`**. Checked before relying on the run:
   `936a26d` touches `.github/dependabot.yml`, `MANIFEST.sha256`,
   `docs/SOURCE_SEAL.md` and two new phase reports — **no lockfile, and none of
   the 57 manifest-baseline files** — so the freeze minted on the pre-merge tree
   is byte-valid on the merged one.
2. **The merged `MANIFEST.sha256` was verified, not assumed.** Both sides of the
   merge edited it — `main` added the P69 documentation rows, #19 changed the
   `Cargo.lock` row — and git auto-merged them textually. Regenerating the
   manifest independently over the merged tree at `2b67644` (1,491 tracked
   files, 0 missing) reproduces the auto-merged file **byte-identically**. A
   textual three-way merge of a hash manifest is exactly the kind of thing that
   is silently wrong, so it was measured rather than trusted.
3. **The freeze candidate** was minted by run **`35619604606`** at `70cdde6`.
4. **Committed, then verified on CI** — verdict below.

### Regenerating the manifest without `git`

`regenerate-source-manifest.py` calls `source_seal.tracked_files()`, which shells
out to `git ls-files`, and `git` is dead on this host (§7). The delivered set was
taken from the **GitHub git-trees API** instead — the same question (*what does
this commit's tree contain?*) asked of the same source of truth — and the
manifest was rebuilt with the script's own rule: `sorted(paths)`, one
`f"{sha256}  {rel}\n"` line each, LF endings, UTF-8, manifest excluding itself.

**The workaround was proven against a known-good artifact before being used on
anything.** Regenerating `main`'s manifest at `936a26d` by this method yields
**1,491 lines, byte-identical to the committed `MANIFEST.sha256`** except for
exactly the two lines this phase edited on purpose (`docs/LEDGER.md`,
`scripts/verify-enterprise.ps1`). Path set, ordering, formatting and hashing all
reproduce. A workaround that has not reproduced a known artifact is a guess, and
this one is not.

### The commit, and the verdict

Committed as **`87a425a5`** on `dependabot/cargo/phase21-workspace/uuid-1.26.1`.
Three files, and the diff is as small as the change permits:

| file | change |
|---|---|
| `release/dependency-locks.sha256` | +1 / −1 — the `Cargo.lock` row |
| `release/dependency-freeze.json` | +2 / −2 — `cargo_lock_sha256`, `lock_baseline_sha256` |
| `MANIFEST.sha256` | +2 / −2 — the two rows above |

`release/dependency-manifests.sha256` is **not** in the commit: the bump touches
no manifest file, so its bytes are unchanged and including it would have been a
no-op entry in a commit that should show only what moved.

**Run `35627361780` at `87a425a5`:**

| step | at `70cdde6` | at `87a425a5` |
|---|---|---|
| 10 `Delivered source seal` | success | **success** |
| 11 `Verify approved dependency freeze` | **failure** | **success** |
| 12 `Frozen dependency restore` | skipped | **success** |
| 13 `Restore Rust build cache` | skipped | **success** |

**Step 11 is green.** The wall that has blocked every cargo and npm bump in this
repository is cleared on #19, and steps past 11 ran on a dependency-bump PR for
the first time.

Step 10 deserves its own line, because it independently validates the workaround
in the previous subsection: `source_seal.py` ran on the runner, with a working
`git`, against a `MANIFEST.sha256` this host built from the **GitHub trees API**
because it has no `git` — and it passed. The authoritative tool verified the
substitute's output.

**#19 is not merged.** It is green and it is the owner's to merge.

## 5. `--keep-going`, and what it buys

`scripts/verify-enterprise.ps1` line **23** now reads:

```powershell
& cargo clippy --workspace --all-targets --locked --keep-going -- -D warnings
```

The handoff said `:24`; line 24 is the `throw` that reads `$LASTEXITCODE`. The
flag belongs on the invocation, and it must precede `--` because it is a cargo
flag, not a rustc one. `cargo clippy --help` does not list it — clippy forwards
to `cargo check`, which does — and it is accepted.

**Measured on this tree, same working copy, both runs to completion:**

| | crates reported failing | errors reported | exit | wall |
|---|---|---|---|---|
| without `--keep-going` | **4** | **20** | 101 | 12.4 s |
| with `--keep-going` | **9** | **52** | 101 | 38.2 s |

The five crates the plain gate never reaches — `aethercore-cleaner`,
`aethercore-consent-broker`, `aethercore-desktop`,
`aethercore-install-hardener`, `aethercore-update-broker` — are not discovered
until the four ahead of them are fixed. Cargo already parallelises within a
wave, which is why the plain run reports 4 and not 1; what `--keep-going` adds
is that a failed crate stops being a scheduling barrier. Clearing this set costs
**one run with the flag against 2–6 runs without it** (at least one new crate
surfaces per run, so five remaining crates take two to six rounds). At the
~50-minute Windows round trip P69 measures, that is **50 minutes against 100–300
minutes**, and it is the same wall P68 spent eight round trips on.

**The qualification, stated because the numbers above are this host's and not
CI's:** all 52 errors are macOS-only and the Windows clippy gate is genuinely
green — run `35484711267` step 21 **success**. Most are dead-code and
unused-import errors on Windows-path symbols that `cfg(windows)` excludes here
(`sid_text_from_bytes`, `ATA_SMART_SECTOR_BYTES`, `SERVICE_SDDL`,
`PRODUCT_NAME`), and the four that are real lints rather than dead code
(`manual_checked_ops`, two `useless_conversion`, `unnecessary_cast`) are all in
`crates/performance-telemetry/src/macos_impl.rs`, a file the runner never
compiles. **What transfers is the flag's fan-out behaviour, which is a property
of cargo's scheduler and not of the platform. What does not transfer is the
count.** The flag is not being added because this tree is red; it is being added
so that the next time the Windows gate is red, it is red all at once.

**The change itself is unmeasured on the runner, and stays that way.** The edit
lives in the working tree, not in any of this phase's four commits: those went
to PR branches and carried freeze and seal files only, while
`verify-enterprise.ps1` belongs on `main`, which this phase was not authorised
to commit to (§9). Step 21 did run green on all four bumps — but on the
*unmodified* script, so what CI has confirmed is that the gate passes, not that
`--keep-going` behaves there as it does here.

## 6. The verdict P69 could not cite

P69's own commit created `936a26d`, and the CI run for `936a26d` begins when
that commit lands — so the report inside it could not name its own result.

**Run `35484711267`, `push`, `head_sha` `936a26d`: `success`.**

| job | result | duration |
|---|---|---|
| `deny-check` | **success** | 2m 18s |
| `windows` | **success** | 1h 03m 44s |

The `windows` job is **29 success / 2 skipped across its 31 steps**, and the
two skips are both cache *saves*, which skip on a cache hit — step 9 `Save
Windows ADK Deployment Tools` and step 27 `Save Rust build cache`. **No gate was
skipped: every step that asserts something about the source ran, and every one
passed.** Specifically:
step 10 `Delivered source seal` **success**, step 11 `Verify approved dependency
freeze` **success**, step 21 `Enterprise convergence master source/native
qualification gate` **success** — the clippy, rustfmt and full-workspace-test
wall P68 spent eight round trips on — step 24 `Supply-chain audit` **success**,
step 25 `Build unsigned production packaging candidate` **success**.

It is **not** a first — and the draft of this report claimed it was, which is
why it was checked. `ci.yml` on `main` has the identical 29-success/2-skipped
shape at `d0bb66d` (`35479177815`), `cb9607d` (`35476215704`) and `d7b087a`
(`35471181801`), the three pushes before it. `936a26d` is the **fourth
consecutive** fully-green run on `main`, after a wall of eleven failures and
three cancellations running back to 2026-09-14. That is the more useful fact:
`main` is not green once, it is stably green.

It also corroborates §5's qualification independently:
step 21 runs `verify-enterprise.ps1`, whose clippy leg is green on Windows while
52 errors sit in front of it on macOS.

## 7. Environment — worked around, and said so

**`/usr/bin/git` is unusable on this host** and it is the only `git`:

```
You have not agreed to the Xcode license agreements.
```

`scripts/source_seal.py:88` and `scripts/regenerate-source-manifest.py` both
invoke `git` by name, so **neither could be run this phase**. Every git
operation here went through the **GitHub REST API via `gh` 2.96.0** instead —
run dispatch, run and job status, step conclusions, PR state, tree listings.
That is a workaround, it is named here rather than left implicit, and it has a
real limit: it reads the *remote*, so it cannot substitute for the seal, which
reads the working tree.

**`cc` exits 69, so no cargo build script links.** Re-measured this phase rather
than inherited: `cargo clippy -p aethercore-contracts` appeared to pass, and
that was a stale `target/` artifact — touching `crates/contracts/build.rs`
(mtime only, contents unchanged, so the seal is unaffected) forced the rebuild
and reproduced `error: linking with 'cc' failed: exit status: 69` exactly as
P69 §5 recorded.

**What that does not block — and this corrects P69 §6.1**, which says this host
*"cannot compile, test or lint any Rust in this workspace"*: `cargo clippy` on
library targets does not link, so the clippy gate itself **does** run here and
produces real lint output. That is what made §5's measurement possible at all.
The boundary is build scripts and binaries, not linting.

## 8. The four npm bumps — three landed green, one is not a bump

An npm bump breaks **both** baselines, not one. It moves `pnpm-lock.yaml`
(hashed by `release/dependency-locks.sha256`) and `apps/ui/package.json`, which
is one of the 57 files hashed by `release/dependency-manifests.sha256`. So each
of these commits carries three `release/` files where #19 carried two, and
re-seals **five** manifest rows rather than two: the two the bump touched, plus
the three the refresh rewrote.

Each was put through §3's loop: branch brought up to date with `main` (all four
were 4 commits behind), freeze minted by a `workflow_dispatch` on the branch
itself, artifact downloaded and verified here, then committed and re-sealed.

| PR | bump | freeze run | commit | CI |
|---|---|---|---|---|
| #14 | `svelte-check` 4.7.5 → 4.7.6 | `35620937336` | **`17dc649f`** | **`35628508932` success** |
| #15 | `svelte` 5.56.9 → 5.57.0 | `35620941644` | **`d5571c10`** | **`35628525906` success** |
| #17 | `vite` 8.2.1 → 8.3.0 | `35620946028` | **`a1b2a89a`** | **`35628532938` success** |
| #20 | `typescript` 6.0.3 → 7.0.2 | `35620950722` **failed at step 11** | — | — |

**Verified before each commit, on this machine, never assumed:** for all three,
the artifact's `Cargo.lock` and `pnpm-lock.yaml` are byte-identical to the
branch's committed lockfiles; all 2 lock hashes and all 57 manifest hashes
recompute (**59/59** each); `dependency-freeze.json`'s four self-hashes match
and its `rust_toolchain` 1.97.1 / `pnpm` 11.22.0 match the pins. Nothing was
staged that had not changed, and the re-seal ran **after** the freeze files.

**All four PRs are now fully green** — `deny-check`, `windows` and all five
`fuzz` legs — and **none is merged.**

| PR | CI | fuzz | merged |
|---|---|---|---|
| #19 `87a425a5` | success (30 steps green, 1 skipped) | `35627361849` success | **no — owner's** |
| #14 `17dc649f` | success (29 green, 2 skipped) | `35628508976` success | **no — owner's** |
| #15 `d5571c10` | success (29 green, 2 skipped) | `35628526096` success | **no — owner's** |
| #17 `a1b2a89a` | success (29 green, 2 skipped) | `35628532963` success | **no — owner's** |

### #20 is `DBT-P70-005`, and it proves criterion 3 works

The brief grouped #20 with #14/#15/#17 as a compatible npm bump. It is not.
Its freeze run failed at step 11 with `svelte-check` refusing outright:

> TypeScript 7 support currently requires both TypeScript 7 and TypeScript 6
> installed in your project, and requires using the `--tsgo` or
> `--tsgo-experimental-api` flag.

The wall is `scripts/build-release.ps1:64`, *Svelte/TypeScript validation
failed.* Adopting TS 7 is three coupled changes — both compilers installed via
an npm alias, `--tsgo` passed to `svelte-check`, and a decision about whether
the project wants the experimental Go type-checker at all — so it belongs in the
ledger beside #16/#18/#21/#22, not in a merge queue.

**And the run is the first live evidence that criterion 3 does what the document
claims.** Step 10 `-Refresh` **succeeded** and minted a candidate; step 11
failed; step 12 `Upload dependency freeze set` was **skipped**. A freeze
describing a tree that does not build never reached the artifact store. That is
exactly the failure of run `34745535685` which the upload-after-build ordering
was introduced to prevent — now observed working, rather than argued for.

## 9. What moved, and what is open

**The seal was honoured without being runnable.** `source_seal.py` needs `git`
and `git` is dead here (§7), so the check P69 §3.4 requires *before* any re-seal
was performed by hand against `MANIFEST.sha256` itself, which lists every path:
`shasum -a 256 -c` over all **1,491** entries gives **1,489 OK and exactly 2
FAILED** — `docs/LEDGER.md` and `scripts/verify-enterprise.ps1`, the two files
this phase edited on purpose. No `unlisted`, no `missing`, nothing else drifted,
and `crates/contracts/build.rs` — touched in §7 to force a build-script rebuild
— hashes **identical** to its manifest entry, confirming that touch moved mtime
and not bytes. P69's rule holds: every failure is a `hash` on a path the change
was supposed to touch.

**Committed to PR branches — four commits, nothing on `main`, nothing merged:**

`87a425a5` (#19), `17dc649f` (#14), `d5571c10` (#15), `a1b2a89a` (#17). Each
carries only the freeze files that actually changed plus a re-sealed
`MANIFEST.sha256`, and each is green on CI and fuzz.

**Left in the working tree, uncommitted, for the owner:**

* `scripts/verify-enterprise.ps1` — `--keep-going` on line 23.
* `docs/LEDGER.md` — five new `§1` rows, `DBT-P70-001` … `DBT-P70-005`.
  Count **69 → 74**, all five-celled.
* `docs/phase70/P70-REPORT.md` — this file.

These belong on `main`, and this phase was authorised to commit to PR branches
only. **`main` moved under the phase while it ran**: another session landed
`664a778a` (*"fix(p71): the five BLOCKED-MACHINE rows, measured on the Windows
PC"*) at 16:00Z, editing `docs/LEDGER.md`, `MANIFEST.sha256`,
`scripts/gate4-preconditions.ps1` and adding `docs/phase71/W1-REPORT.md`. The
five P70 rows were **rebased onto that new base** rather than onto the stale
one, so they apply cleanly and P71's edits to `DBT-P49-003`, `DBT-P55-003/004/
008/009` are preserved. Whoever commits these must re-seal after, and this
phase's `MANIFEST.sha256` work does **not** cover them.

**Opened as ledger rows rather than merged**, because each is a decision:

| PR | bump | why it is a decision |
|---|---|---|
| #16 | `reqwest` 0.12.28 → 0.13.4 | pinned `default-features = false` + `native-tls` + `blocking`; the risk is which TLS backend that feature string still selects, not the five call sites |
| #18 | `rand` 0.9.5 → 0.10.2 | the four call sites sit on exactly the names 0.9 renamed (`rng()`, `random_range`); one seeds `[u8; 32]` in the support-bundle path |
| #20 | `typescript` 6.0.3 → 7.0.2 | **measured this phase**: `svelte-check` refuses TS 7 without a dual install and `--tsgo` (§8) |
| #21 | `windows-core` 0.62.2 → 0.100.0 | the dependency is declared and never `use`d — see below |
| #22 | `criterion` 0.5.1 → 0.8.2 | MSRV against `1.97.1` is the question; no CI job runs the four bench targets |

**#21 needs its correction stated here too.** The brief calls `windows-core`
*"the crate whose missing `Send` impl on `HANDLE` P62 measured and built
`Box<dyn Any>` around"*. `docs/phase62/P62-REPORT.md:55` measures that
constraint against **`windows` 0.62.2** — a different crate, pinned separately
at `Cargo.toml:87`, and **not** what #21 bumps. Measured here:
`windows-core = "0.62.2"` is declared once, at
`crates/windows-update/Cargo.toml:17`, under
`[target.'cfg(windows)'.dependencies]`, and
`grep -rn 'windows_core' --include='*.rs'` over `crates apps services` returns
**zero hits**. `Cargo.lock` already carries two windows-core nodes (0.61.2 and
0.62.2) pulled transitively. So #21 bumps a direct dependency no source file
imports and does not touch the constraint it appears to touch; the question
upstream of the version number is whether the declaration should exist. **Not
verified here**: whether removing it still builds — `cc` exits 69 on this host,
so that check belongs on a machine that can compile.

**`OMEGA-RB-001`'s criteria now hold for `main` on measurement rather than
inheritance** — 1 reproduced on this Mac today, 2 as run `35618987104`, 3 by
construction and observed live on #20. And the thing that was not understood
before this phase is that none of it carries over to a branch that changes a
lock: **the freeze is per-tree, so every bump needs its own, and §3's loop is
how.** Four of them now exist and are green.
