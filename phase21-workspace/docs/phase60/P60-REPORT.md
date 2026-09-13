# P60 — the report

Host: the Mac at `/Users/hasanalaaa/dev/aethercore`. Date: 2026-09-13.
Ledger rows moved in `docs/LEDGER.md`; every figure below names the command, the
commit or the run id that produces it.

**The twelve Dependabot PRs are already closed.** The owner closed all twelve
today, `#1`–`#12`, between 07:18:27Z and 07:42:15Z, none merged — `closed by
hasanalaaa` on every one, read from `repos/hasanalaaa/aethercore/issues/N/events`.
The `gh pr close` loop P59 handed over has been run and is not repeated here.

The short version. **The freeze that CI has been generating for thirty-nine
phases does not build**, and it took landing it to find out. `-Refresh` ran
`cargo generate-lockfile`, which discards the reviewed lock and re-resolves from
scratch, and today's re-resolution walked into an upstream API break: the tree it
produced fails to compile on macOS and on Windows, at the same file and line.
**And even a freeze that builds could not be committed**, because the Windows
runner checks out CRLF, so the freeze recorded hashes of bytes that exist on no
other machine. Both are fixed at the cause rather than worked around. **The
source seal has never described this tree** — not merely since it went stale, but
since the repository's first commit, where 23 of its entries already disagreed
with the files they named. It now covers 1,451 of 1,451 delivered files and has a
verifier that can fail, proved by ten cases committed failing first.

---

## ITEM 1 — landing the freeze

### 1.A — what `-Refresh` writes

Enumerated from the script, not assumed. Five files written, one deleted:

| path | how | line |
|---|---|---|
| `Cargo.lock` | `cargo generate-lockfile` | `:87` |
| `pnpm-lock.yaml` | `pnpm --dir apps/ui install --lockfile-only` | `:89` |
| `release/dependency-locks.sha256` | `Set-Content` | `:95` |
| `release/dependency-manifests.sha256` | `Set-Content` | `:96` |
| `release/dependency-freeze.json` | `Set-Content` | `:115` |
| `release/dependency-freeze.blocker.json` | **deleted** | `:116` |

They upload as their own artifact rather than mixed into the installer's: this
set is reviewed and committed, the `.exe` is not. `71cc061`.

One thing worth knowing about `upload-artifact`: `if-no-files-found: error` fires
only when **nothing at all** matches, so with a five-path list a partial freeze
would have uploaded quietly and looked green. The freeze step now asserts all
five exist and the blocker is gone before the upload runs.

### 1.B — the freeze does not build

The first dispatch, `34745535685` at `71cc061`, landed the set exactly as
intended: step 11, all five files, 47,569 bytes. Then step 12 failed.

```
crates\intelligence-core\src\llama.rs:367
error[E0061]: this function takes 5 arguments but 4 arguments were supplied
LlamaSampler::penalties(Self::PENALTY_WINDOW_TOKENS, Self::PENALTY_REPEAT, 0.0, 0.0)
```

Reproduced independently on this Mac: `cargo check --workspace --locked`, exit
101, same file, same line. `llama-cpp-2` 0.1.156 added an `n_vocab` parameter to
`LlamaSampler::penalties` and redefined what a negative `penalty_last_n` means.
`llama-cpp-2 = "0.1.154"` is a caret requirement, so a from-scratch resolution
took it.

**This corrects P59.** P59 predicted the commit would be "three new files plus one
deletion, with `Cargo.lock` and `pnpm-lock.yaml` unchanged", on the strength of
`cargo update --dry-run --workspace` → `Locking 0 packages`. `--workspace`
restricts that probe to workspace members, and `-Refresh` does not run `cargo
update` at all — it runs `cargo generate-lockfile`, which throws the lock away.
What it actually produced moved **48 crates** and added **6 transitive names**
(`core_detect`, `multiversion`, `multiversion-macros`, `multiversion_no_op`,
`simdutf8`, `zlib-rs`), including `llama-cpp-2`/`-sys` 0.1.154 → 0.1.156 and
`ed25519-dalek`, `reqwest`, `uuid`, `smallvec` and forty-four others.

Pinning `llama-cpp-2 = "=0.1.154"` was tried and is **not sufficient**:
`llama-cpp-sys-2` floats independently inside llama-cpp-2's own manifest, still
resolved to 0.1.156, and `llama-cpp-2` 0.1.154 then failed to compile against it
with 3 errors. That is recorded as `DBT-P60-001` rather than half-fixed.

So the defect is not the version. `docs/RELEASE_SUPPLY_CHAIN.md` already states
the state machine — lockfiles are resolved from scratch **only in the seed
state**, "when it has never been dependency-frozen" — and the script never
implemented it. `-Refresh` re-seeded unconditionally, which is why no two freezes
were ever the same and why the graph was never reviewable. `06baeb7` makes it
verify the committed lockfiles under `--locked` and `--frozen-lockfile`, and
re-seed only when there is no lockfile to verify. The same commit moves the
artifact upload to **after** the build step: a graph that does not compile on the
machine that produced it must not sit in the artifact store looking like a
candidate, which is exactly what `34745535685` left behind.

### 1.B, second stop — the runner checks out CRLF

`34746478525` at `06baeb7` went green end to end and uploaded the freeze set
after the build. It still could not be committed:

```
Cargo.lock      committed  89340cba6d5f8665b9d0ec73dd7f26c22680e735af44152bfcef240248c66519
                artifact   0d0ba17b6a67919c6ddcd3068022f00a96dec882cdeb77411ff8c7a4f614d9d4
pnpm-lock.yaml  committed  6bda309c4a483ece97cce68d34eeb281fc6c22fbd85ab235ae21e78faf08b746
                artifact   c06924a60e23328375649b5d86af225558d2ada256f4cd6eff55f58cc7ec1933
```

Measured, not guessed: sha256 of the committed bytes with every `LF` replaced by
`CRLF` equals the artifact hash, **for both files, exactly**. GitHub's Windows
runners set `core.autocrlf=true`, so `actions/checkout` rewrites every text file
on the way out and the freeze hashed a working tree that exists only on Windows.

This was invisible until `06baeb7`. While `-Refresh` re-seeded, `cargo
generate-lockfile` overwrote the checked-out file with LF bytes of its own —
which is why the *first* artifact's `Cargo.lock` was LF and byte-identical to the
one this Mac produced. Stopping the re-seed removed the accident that had been
hiding it. `MANIFEST.sha256` has the identical defect and nobody had hit it: a
Windows checkout fails the source seal on every text file it contains.

`.gitattributes` with `* -text` (`50cc466`) — no conversion in either direction,
so the working tree is the committed bytes on every platform. Chosen over
`text=auto eol=lf` because `-text` renormalizes nothing: `git add --renormalize .`
under it changes **zero** files, where `eol=lf` would have rewritten 37 that are
committed with CRLF, most of them captured Windows console output under
`docs/phase36/evidence/`. The CRLF in captured evidence is part of what was
captured.

### 1.C — the barrier, before and after

`34749242668` at `50cc466` went green end to end, and the set it produced is the
graph this repository has been building for forty phases:

```
Cargo.lock      89340cba6d5f8665b9d0ec73dd7f26c22680e735af44152bfcef240248c66519   = committed
pnpm-lock.yaml  6bda309c4a483ece97cce68d34eeb281fc6c22fbd85ab235ae21e78faf08b746   = committed
```

Three new files and one deletion, with the lockfiles untouched — which is what
P59 predicted, for a reason P59 did not have. It is true because `-Refresh`
stopped re-seeding and the runner stopped rewriting line endings, not because the
graph happened not to move. Landed in `d53ec16`.

Reviewed rather than taken on trust. Every figure independently reproduced on
this Mac against the CI artifact: both lockfiles byte-identical to the committed
files; all **57** `dependency-manifests.sha256` entries recomputed locally, **0
differing**; `dependency-locks.sha256`'s two entries equal to the two lockfile
digests; `dependency-freeze.json` referencing exactly those four digests.

**The gate `OMEGA-RB-001` reads, before and after:**

```
$ python3 scripts/check-dependency-freeze.py --json
  before   exit 2   approved false   blocker_present true
           missing: release/dependency-locks.sha256, dependency-manifests.sha256, dependency-freeze.json
  after    exit 0   approved true    8 of 8 checks true
```

**`omega-evidence.py`, before and after, unmodified:**

```
before  ['OMEGA-RB-001', 'OMEGA-RB-002', 'OMEGA-RB-005', 'OMEGA-RB-006', 'OMEGA-ENV-PWSH', 'OMEGA-ENV-WINDOWS']
after   [                'OMEGA-RB-002', 'OMEGA-RB-005', 'OMEGA-RB-006', 'OMEGA-ENV-PWSH', 'OMEGA-ENV-WINDOWS']
```

`OMEGA-RB-001` is gone because the evidence exists. The file that decides it was
not edited: `scripts/omega-evidence.py` is sha256
`8d8938821b979ef171e59e78868241beac887c439071e90b481fc0e1515d94a3` in the working
tree, at `HEAD`, and at P59's final commit `8416e89` — the same bytes in all
three.

One honesty note on how those two runs were obtained. `omega-evidence.py` aborts
before every other gate when its manifest check fails, and it and the seal
disagree about 14 untracked files (`DBT-P60-005`), so it would never have reached
the freeze check at all. For each run the delivered manifest was held, replaced
with one generated under **omega's own walk rule** by a script that lives outside
the repository, and restored afterwards — `git status` clean both times. The
three `DBT-P60-003` files were moved aside for the same reason and moved back.
Nothing in `scripts/` was changed to produce these two lists.

**And `ci.yml` passes its first gate for the first time.**

```
run 34749206113 at 50cc466   step 7 Verify approved dependency freeze   FAILURE, nine gates skipped
run 34751272638 at d53ec16   step 7 Verify approved dependency freeze   SUCCESS
                             step 8 Frozen dependency restore           SUCCESS
                             step 9 Rust formatting                     SUCCESS
                             step 10 Rust unit/integration tests        FAILURE  <- DBT-P60-006
```

No workflow in this repository had ever passed `-VerifyOnly`. One has now.

### 1.D — the POSIX path defect

`freeze-dependencies.ps1:9-12` built its four output paths with `\`. On POSIX a
backslash is an ordinary filename character, so `-Refresh` would have created a
file literally named `release\dependency-locks.sha256` in the workspace root.
Fixed to forward slashes in `71cc061`; .NET accepts either on Windows.

---

## ITEM 2 — is a hosted runner a trustworthy freeze source

Yes, and the reason is not the runner.

The criteria are written into `docs/RELEASE_SUPPLY_CHAIN.md` as the definition
that was missing, and the phrase "the trusted dependency-freeze workstation" is
retired from the live code with them:

1. **The resolution reproduces on an independent machine.**
2. **The run leaves a durable third-party record naming the exact source commit.**
3. **The graph it produces is compiled on the machine that produced it, before
   the output leaves that machine.**

Criterion 1 is measured rather than asserted. A GitHub-hosted `windows-2025`
runner and this macOS laptop each resolved the same manifests and produced
`Cargo.lock` with sha256
`c95e3c79872d731d777c1baaaa2944d8820ebecf9cea7f3c97de29ba7e0cbb50` —
**byte-identical, across two operating systems**. That is what makes the host
replaceable: reproduction on a second unrelated machine removes the need to trust
the first, and it costs about a minute.

Criterion 3 is the one with teeth, and this phase exists because nothing enforced
it. Neither host's identity nor the runner's ephemerality would have caught a
graph that does not compile. Compiling it did — on both hosts, identically.

**A hosted runner qualifies.** It meets 1 as measured, meets 2 by construction,
and meets 3 now that the upload follows the build. Its ephemerality is a real
advantage: a fresh VM cannot carry a developer's accumulated cargo or npm state
into the resolution. And on the one supply-chain control anyone has actually
measured on both, `DBT-P55-007`, CI is the *stronger* source —
`signatureValidationMode=require` is inert on the developer machine (corrupted
`trustedSigners` fingerprints, restore still exit 0) and enforced on CI, which is
where `NU3034` surfaced.

The argument against is not dismissed: the runner is infrastructure this project
does not control and leaves nothing to inspect afterwards but the log. Criterion 1
exists precisely for that.

**A developer workstation does not qualify on its own** — it fails criterion 2
outright. It becomes valuable the moment it is the *second* machine.

The document's own contradiction is resolved rather than left standing. It said
CI is verify-only while `windows-installer.yml` runs `-Refresh`: a refresh mints
a **candidate**; approval is the commit, and no workflow can commit.

---

## ITEM 3 — the seal

### The decision, before any number was changed

`docs/SOURCE_SEAL.md`.

**What it seals against**: one question — are the bytes in this tree the bytes the
project sealed at this commit. Not build provenance, not the dependency graph,
not verification hermeticity. Each of those has its own instrument and the
document names which.

**What is inside it**: every git-tracked file under `phase21-workspace/`, and
nothing else. Tracked is the definition of delivered, and it is the one rule that
cannot drift. The directory walk it replaces swept in **14 files that are not
source, and its exclusion list caught none of them**:

| files | what they are |
|---|---|
| 9 | `apps/ui/dist/**` — Vite output, rewritten by every `pnpm build` |
| 1 | `assets/models/*.gguf` — downloaded, 1.1 GB |
| 1 | `release/phase35/*.zip` — a build artifact |
| 3 | `services/maintenance-service/C:\ProgramData/.../aethercore.db{,-shm,-wal}` |

In the other direction, **tracked minus walked = 0**: the new scope loses no
delivered file. Committed generated output (`SBOM.cdx.json`, the icon set) and the
phase archives stay **inside** — they are delivered bytes and an auditor must be
able to verify them.

### The 200 conflicts, classified before regenerating

Actually 202 on today's tree. `MANIFEST.sha256` was written exactly once, in
`b024df8`, which is this repository's **root commit**, and never touched again
until now. Every conflicting file was compared against that commit's blob:

| count | what it means |
|---|---|
| **179** | correct at `b024df8`, changed by a later commit. Legitimate staleness across forty phases. |
| **23** | did not match the file's own bytes **in the commit that created the manifest**. `b024df8` has no parent, so no earlier state exists that those hashes could describe. |
| **0** | unrecorded drift. `git status` was clean throughout, so every file's bytes are a committed state. |

The 23 — `Cargo.toml`, `QUALIFICATION_DEBT.json`, `scripts/static_validate.py`,
five `apps/ui/src` files, ten `crates/*/src` files and the rest — are the files
Phase 19 was still editing when it was committed. The manifest was generated
partway through and never regenerated. **The seal has never described this tree.**

### A verifier that can fail

`scripts/source_seal.py` reads the committed manifest, hashes the tree
independently, and diffs. It writes nothing.

`scripts/test_source_seal.py` was **committed failing in `c094614`** —
**1 of 10 pass** with the verifier not yet written — and passes 10 of 10 after
`10734d8`:

```
PASS  pristine sealed tree verifies                       exit=0 verified=4
PASS  one flipped byte is reported as a hash mismatch     exit=1 src/a.txt hash
PASS  a listed file that is gone is reported as missing   exit=1 src/b.txt missing
PASS  a tracked file the manifest never listed            exit=1 src/c.txt unlisted
PASS  rebuilding ignored output does not break the seal   exit=0
PASS  a manifest covering part of a tree cannot report ok exit=1 three unlisted
PASS  the delivered workspace verifies                    exit=0 1451/1451
PASS  one flipped byte in the delivered tree fails        exit=1 README.md hash
PASS  the corruption case leaves the tree clean           git status: ''
PASS  the run left the delivered manifest untouched
```

**The exit code against a one-byte corruption is 1**, and the failure names the
file with both the expected and the actual digest. The last two cases are not
padding: the first draft of this test called the generator with a `--root` it did
not accept, argparse being absent there, so the generator ignored argv and
rewrote the delivered tree's own manifest. The case that catches that stays.

`p30-fulltree-spot-hash-ok` becomes `p30-delivered-source-seal-ok`. The old check
loaded the ledger, compared the tree to it, **discarded that result**, ran
`_build_p30_patch.py` to regenerate the ledger from the same tree, and compared
against that. Restoring its honest form was considered and rejected with numbers:
`PHASE_30_EXPECTED_FULL_SHA256.json` describes the **Phase 32** tree — 759
entries match, 245 diverge, 7 name files that are gone — so at Phase 60 it asks
whether today's source equals a snapshot from 28 phases ago, and the correct
answer is *no*. There is no version of that check that is both honest and
meaningful. Its origin is on the record: the regeneration entered in Phase 31
(`0e30038`) to silence a `.DS_Store` divergence, and silenced the instrument.
Removing it also removes a tracked-file write from an audit — the run no longer
rewrites four files under `PHASE_30_BINARY_SAFE_PATCH/`.

### What the seal still does not cover

Two gaps, both recorded rather than half-closed.

`DBT-P60-002`: the seal's root is `phase21-workspace/`, so the four files at the
repository root that build and gate the product — `.github/workflows/{ci,fuzz,release}.yml`
and `.github/dependabot.yml` — are sealed by nothing. Three of them are exactly
the dangling `missing` entries `DBT-P59-003` recorded. Widening is not a manifest
change alone: `omega-evidence.py` rejects any manifest path resolving outside the
workspace (`path_escape`), and its `tree_state`, `path_within_root` and
disposable-clone logic all key on the same root.

`DBT-P60-005`: `omega-evidence.py` defines the deliverable set with the same
directory walk this phase replaced, so it and the seal disagree over those 14
untracked files and `OMEGA-RB-003` stays raised — now on **14** problems instead
of 1,114, all of one kind and all named. `omega-evidence.py` was **not** edited
this phase.

---

## ITEM 4 and ITEM 5 — Dependabot

### ITEM 4 — `DBT-P59-001`, fixed

`.github/dependabot.yml`'s npm `directory` moves from
`/phase21-workspace/apps/ui` to `/phase21-workspace`. `apps/ui` holds only
`package.json`; `pnpm-lock.yaml` and `pnpm-workspace.yaml` are one level up, so
Dependabot bumped a manifest it could not lock and `pnpm --dir apps/ui install
--frozen-lockfile` then failed with `ERR_PNPM_OUTDATED_LOCKFILE` — reproduced by
P59, exit 1, and carried by PRs `#6`, `#7`, `#10` and `#12`.

The layout is not what should change. The pnpm workspace root is where a
workspace lockfile belongs and where every other tool already looks —
`pnpm-workspace.yaml` declares `packages: ['apps/ui']` from there, `ci.yml` and
`windows-installer.yml` both run `pnpm --dir apps/ui` from there, and the freeze
hashes `apps/ui/package.json` from there. Dependabot was the only thing pointed
somewhere else.

Stated plainly: this is **not verified end to end.** No Dependabot run can
evaluate it while `open-pull-requests-limit: 0` holds. It is landed now rather
than left for the resumption change because the alternative is that restoring the
limits brings the four broken npm PRs straight back.

### ITEM 5 — the twelve, and the pause

The twelve are closed already; see the top of this report.

**Is the resumption condition met? The literal condition yes, the thing it stood
for no — so the pause holds.**

The condition in the file read: restore the limits "once `OMEGA-RB-001` is closed
— the freeze set committed and `ci.yml`'s windows job reaching step 2". The
freeze set is committed, `OMEGA-RB-001` is closed, and the windows job now
reaches step 10 of 20. By the letter, met twice over.

By intent, not. P59 paused because "every pull request against this repository
fails identically before its diff is ever compiled", and that is still true — at
step 10 instead of step 1. `ci.yml`'s windows job does not install the Windows
ADK Deployment Tools, which `windows-installer.yml` does, so
`crates/system-repair/build.rs:40` panics on `dismapi.lib` and everything from
the Rust tests onward is unreachable. `DBT-P60-006`. Resuming now would reopen
twelve pull requests to collect twelve copies of a failure that belongs to `main`.

So the condition is restated in the file, once, to say what it always meant:
**restore the three limits to 5 when `ci.yml`'s windows job is green on `main`.**

Two things land with the pause so that resuming is a one-line change rather than
a new investigation: the npm directory above, and an `ignore` for `llama-cpp-2`
and `llama-cpp-sys-2` on the cargo ecosystem. PR `#11` was
`bump llama-cpp-2 from 0.1.154 to 0.1.156` — the exact bump measured in ITEM 1 as
not compiling. Dependabot was right to notice it and would be right to propose it
again; the reason not to take it is `DBT-P56-002`'s generation measurement, which
the new sampler signature would invalidate. That is a decision with its own
re-measurement, and the `ignore` entry names the condition that removes it.

---

## ITEM 6 — `BLOCKED-MACHINE`

Re-measured today, not inherited:

| check | result |
|---|---|
| `~/.ssh/config` | **does not exist** |
| `~/.ssh/known_hosts` | one entry, `[192.168.68.114]:8022` |
| `arp -a` | **5** hosts on `192.168.68.0/24` — `.1`, `.100`, `.106`, `.109`, `.110`; **`.114` is not among them** |
| `mount` | no SMB, AFP or CIFS mount |
| grep over `docs/` | no hostname, address or credential for the PC |

P59 saw 16 ARP entries and no `.114` either. The table ages out; the absence does
not. The two direct probes P59 attempted were refused by the sandbox — TCP
connect (*Credential Exploration*) and `ping` (*Containment Escape*) — and were
deliberately **not re-attempted**: re-running a refused probe produces a second
refusal, not new evidence.

`DBT-P55-008`, `DBT-P55-009` and `DBT-P49-003` stay `BLOCKED-MACHINE`. None was
attempted and none is claimed.

---

## Two things found while measuring

**`omega-evidence.py` is unrunnable on a workspace that has been built.** It takes
**13** disposable clones — `shutil.copytree` of the whole of `ROOT`, then
`tree_state` over the copy twice, with no exclusions at all. `target/` on this
machine is **36 GB**, so that is roughly 470 GB copied and 950 GB hashed per run.
The attempt was killed after forty minutes with one clone at 30 GB and 29 GB of
free space consumed. With `target/` moved aside — 1.2 GB of workspace — the same
run completes. This is the same root defect as the seal's: a deliverable set
defined as a directory walk that includes build output. `DBT-P60-004`.

**A Windows path leaked into the POSIX tree, and it makes `OMEGA-RB-003`
unclearable.** `services/maintenance-service/C:\ProgramData/AetherCore/state/`
holds three SQLite files, created 2026-09-01 by running the maintenance service on
this Mac. `omega-evidence.py`'s walk says they are deliverable; its own
`validate_manifest_relpath` rejects any manifest path containing a backslash as
`non_portable_path`. So they cannot be listed and cannot be omitted — with those
files present, that gate cannot reach a clean verdict on this machine by any
route. They are untracked and gitignored; they were moved aside for the
measurements below and moved back. `DBT-P60-003`.

---

## The ledger count

Counted by reading the status cell of every five-cell `§1` row, not by arithmetic
on a previous figure.

| | rows | status begins `OPEN` |
|---|---|---|
| before (`8416e89`) | 31 | **17** |
| after | 35 | **19** |

Four closed: `DBT-P58-001` (the freeze), `DBT-P59-001` (the Dependabot
directory), `DBT-P59-002` (the verifier that could not fail), `DBT-P59-003` (the
seal that described nothing).

Six entered, none of them a new defect — all six were already true and became
visible because something was measured for the first time:

| id | what became visible |
|---|---|
| `DBT-P60-001` | upstream broke its API inside `llama-cpp-2`'s 0.1.x line; a re-resolved lock does not compile |
| `DBT-P60-002` | the seal's root is the workspace, so the four files at the repository root that build and gate the product are sealed by nothing |
| `DBT-P60-003` | a Windows path leaked into the POSIX tree, and `omega-evidence.py` can neither list nor omit the files |
| `DBT-P60-004` | `omega-evidence.py` copies and hashes the whole workspace 13 times; with a built `target/` that is ~470 GB copied |
| `DBT-P60-005` | the seal and `omega-evidence.py` disagree about what is delivered, over 14 files |
| `DBT-P60-006` | `ci.yml` never installs the Windows ADK, which is what now stops it on `main` |

The `DBT-P56-002` row is still malformed — four cells rather than five — and is
excluded from both figures, as in P58 and P59. Pre-existing, and left alone.

---

## Commits

| commit | item |
|---|---|
| `71cc061` | ITEM 1.A + 1.D — the freeze outputs upload; the POSIX paths are fixed |
| `c094614` | ITEM 3 — ten seal cases, **committed failing**, 1 of 10 pass |
| `10734d8` | ITEM 3 — a verifier that can fail; the P30 tautology removed |
| `06baeb7` | ITEM 1.B — `-Refresh` stops re-seeding; the upload follows the build |
| `fccfb36` | ITEM 2 + ITEM 3 — the freeze-source criteria, the seal's scope, the manifest |
| `3ce0624` | ITEM 6 — `BLOCKED-MACHINE`, re-measured |
| `50cc466` | ITEM 1.B — `.gitattributes`, so the working tree is the committed bytes |
| `d53ec16` | ITEM 1.B/1.C — **the freeze lands; `OMEGA-RB-001` closes** |

---

## The single next action

Add the Windows ADK Deployment Tools step to `ci.yml`'s windows job — the same
`adksetup.exe /quiet /norestart /features OptionId.DeploymentTools` that
`windows-installer.yml` already runs — so the gate that now reaches step 10 of 20
can show what the other ten steps say, and Dependabot can be resumed against a
`main` that is green.
