# The delivered-source seal

`MANIFEST.sha256` is the seal over the delivered source tree.
`scripts/regenerate-source-manifest.py` writes it; `scripts/source_seal.py`
verifies it; `scripts/test_source_seal.py` proves the verifier can fail.

Written in P60 because the seal did not have a stated purpose, and everything
wrong with it followed from that. It listed 551 files against a tree of 1,462,
disagreed with 202 of the files it did list, and the check that reported on it —
`p30-fulltree-spot-hash-ok` — regenerated its own expected values from the tree
it then compared against, so it had never been able to report a problem.

## What it seals against

One question: **are the bytes in this tree the bytes the project sealed at this
commit?**

That is a transport-and-tamper check on delivered source. It is deliberately not
three other things, each of which has its own instrument:

| question | instrument |
|---|---|
| does this source match the bytes we sealed? | **this seal** |
| does the dependency graph match an approved one? | `release/dependency-freeze.json` + `freeze-dependencies.ps1 -VerifyOnly` |
| does a binary match the source it claims to come from? | the release pipeline's SBOM and reproducibility controls |
| did verification modify the tree while running? | `omega-evidence.py`'s `source_tree_integrity` before/after digests |

Keeping those apart is what makes the scope answerable. A seal that also tried to
cover build provenance would have to cover build output, and would then break on
every build.

## What is inside it

**Every git-tracked file under `phase21-workspace/`, and nothing else.**

Tracked is the definition of delivered. Whatever git carries is what an auditor
receives; whatever it does not carry, they never see. The rule needs no
maintenance and cannot drift, which is the property the previous rule lacked.

The previous rule was a directory walk minus a hardcoded exclusion list
(`.git`, `target`, `node_modules`, `out`, `__pycache__`, `*.pyc`, `*.pyo`).
Measured against the tracked set in P60, that list missed **14 files** and caught
none of them:

| files | what they are |
|---|---|
| 9 | `apps/ui/dist/**` — Vite build output, rewritten by every `pnpm build` |
| 1 | `assets/models/qwen2.5-1.5b-instruct-q4_k_m.gguf` — downloaded, 1.1 GB |
| 1 | `release/phase35/AetherCore-0.1.0-windows-x86_64-offline.zip` — a build artifact |
| 3 | `services/maintenance-service/C:\ProgramData/AetherCore/state/aethercore.db{,-shm,-wal}` — runtime SQLite state, under a directory whose literal name is a leaked Windows path |

Sealing those is what "the seal breaks on every build" means concretely. In the
other direction the tracked set contains nothing the walk excluded: **tracked
minus walked = 0**, so the new scope loses no delivered file.

Three boundary calls that follow from the same rule, stated so nobody has to
re-derive them:

- **Committed generated output is inside.** `SBOM.cdx.json`, the rendered icon
  set, the phase patch archives. They are delivered bytes; an auditor holding
  them must be able to verify them. That they were produced by a tool is
  irrelevant — that they are shipped is not.
- **Phase archives and vendored trees are inside.** 165 files under
  `PHASE_*_BINARY_SAFE_PATCH/new-files/` alone. Excluding them would recreate
  the hole this document exists to close.
- **Untracked files are outside, including ignored build output.**
  `scripts/test_source_seal.py` holds this: rebuilding ignored output, and adding
  more of it, does not disturb the seal.

## One definition of delivered

`DBT-P60-004` + `DBT-P60-005`. Two instruments were deciding what "delivered"
means and they disagreed over 14 files: this seal asked git, and
`scripts/omega-evidence.py` walked the directory tree minus a hardcoded
exclusion list. At least one of the two was measuring the wrong set, so neither
verdict could be trusted.

**The decision: the seal's rule is the project's single definition of delivered,
and `omega-evidence.py` adopts it by importing `source_seal.tracked_files`
rather than restating it.**

Three reasons, in the order they settle the question:

1. **Only one of the two can be right about the 14 files, and the walk is wrong
   about all 14.** They are nine Vite build outputs, a 1.1 GB downloaded model,
   a release `.zip` and three runtime SQLite files. None is delivered; none was
   caught by the exclusion list that exists to catch exactly them.
2. **The tracked rule has no list to maintain.** The walk's rule is a list, and a
   list is only ever as current as the last person who remembered it. Nothing
   about `apps/ui/dist/` or `assets/models/*.gguf` was unusual — they simply
   arrived after the list was written.
3. **A second implementation of one rule is a second rule.** The two sets did not
   drift because someone changed one of them; they drifted because there were two
   of them. Importing is what closes that, and it is why both rows close in the
   same commit: closing one and leaving the other holding a private copy is how
   the disagreement comes back.

### The one place omega-evidence needs a different set, named

Per-gate write detection does **not** use the delivered set, and must not.
`run_repo_script` asks "did running this gate write anything", and anything a
gate writes is by construction not tracked — measuring that against the tracked
set would answer "no" to every gate, always. It keeps a full unexcluded walk of
the disposable clone. That is now both complete and cheap for the same reason:
the clone is built from the delivered set, so there is no `target/` in it to
walk.

Everything else in `omega-evidence.py` asks this seal's question and takes this
seal's set: `verify_manifest`'s deliverable inventory, `cleanliness`, and the
`source_tree_integrity` before/after digests.

### What that changes, stated rather than discovered later

- **`SIGMA-RB-001` now means what it says.** Its condition is "verification
  changed delivered source bytes"; it is now measured over delivered bytes. An
  untracked file appearing in the workspace is no longer a whole-tree integrity
  failure — and a gate that writes one is still caught, inside the clone, which
  is where a gate's writes actually happen.
- **`cleanliness` no longer reports untracked junk.** A `.DS_Store` that is not
  tracked does not ship, so it is not a delivery defect. A tracked one still is.
- **`DBT-P60-003` stops blocking `OMEGA-RB-003`.** The three SQLite files under
  the leaked `C:\ProgramData` directory are untracked, so they leave the
  deliverable set entirely and the gate can reach a verdict on this machine. The
  underlying defect — a service writing a literal Windows path on POSIX — is
  untouched and the row stays open on that.

## The known gap

The seal covers `phase21-workspace/`. It does **not** cover the repository root,
where P58 moved `.github/workflows/{ci,fuzz,release}.yml` and
`.github/dependabot.yml` so GitHub would register them. Those four files gate and
build the product and are sealed by nothing.

This is why three manifest entries dangled for two phases:
`.github/dependabot.yml`, `.github/workflows/ci.yml` and
`.github/workflows/release.yml` were listed at their old workspace-relative paths
and the files were gone.

Widening the seal to the repository root is not a manifest change alone:
`omega-evidence.py` resolves every manifest path under the workspace and rejects
anything that escapes it (`path_escape`), and its `tree_state`, `path_within_root`
and disposable-clone logic all key on the same root. Recorded as a ledger row
rather than half-done here.

## What the 202 conflicts were

Classified in P60 **before** the manifest was regenerated, because regenerating
first destroys the evidence of which kind they were.

`MANIFEST.sha256` was written exactly once, in `b024df8` — the **root commit** of
this repository — and never touched again until P60. Every conflicting file's
bytes were compared against that commit's blob:

| count | what it means |
|---|---|
| **179** | the manifest was correct at `b024df8` and the file changed in a later commit. Legitimate staleness across forty phases. |
| **23** | the manifest's hash did not match the file's own bytes **in the commit that created the manifest**. `b024df8` has no parent, so there is no earlier state in this repository those hashes could describe. |
| **0** | unrecorded drift. `git status` was clean throughout, so every file's bytes are exactly a committed state. |

The 23 are `Cargo.toml`, `QUALIFICATION_DEBT.json`, `scripts/static_validate.py`,
four `crates/*/src` files each in `pc-intelligence`, `system-repair` and
`windows-update`, five `apps/ui/src` files, and eight more. They are the files
Phase 19 was still editing when it was committed: the manifest was generated
partway through, the work continued, and it was never regenerated.

So the seal has never described this tree. Not since P58 moved the workflows, not
since the manifest went stale — since the first commit.

## Using it

```bash
python3 scripts/source_seal.py                  # verify; exit 0 = sealed
python3 scripts/source_seal.py --json           # the full diff, machine-readable
python3 scripts/regenerate-source-manifest.py   # re-seal after deliberate edits
python3 scripts/test_source_seal.py             # prove the verifier can still fail
```

`git add` the new files **before** regenerating. Untracked is out of scope for
both the generator and the verifier, so a file you forgot to add is not sealed —
and the moment you do add it, the verifier reports it `unlisted`. That is the
seal working, not a bug.

### Dependency bumps: re-seal on the bump's own branch

A Dependabot PR edits `Cargo.lock`, `Cargo.toml`, `package.json` or
`pnpm-lock.yaml`. All four are git-tracked under the sealed root, so all four are
sealed, and Dependabot has no step that regenerates `MANIFEST.sha256`. `Delivered
source seal` is step 10 of `ci.yml`'s windows job, so a bump dies there and the
seventeen gates after it never run. Measured on PR #19, uuid 1.25.0 -> 1.26.1,
run 35474420447: `FAILED - 1488 of 1489 tracked files verified, 1 problems`,
`hash Cargo.lock`, steps 11-26 skipped.

Lockfiles are **not** excluded from the seal to make that stop. The rule is
*every tracked file, no list*, for the reason all of **What is inside it** gives;
and a dependency lock is the most tamper-relevant delivered file in the tree, so
an exclusion would put the hole exactly where it pays to have one. The person
merging the bump re-seals it, in two commands whose order is the whole point:

```bash
python3 scripts/source_seal.py --json          # read this BEFORE re-sealing
python3 scripts/regenerate-source-manifest.py  # then re-seal; commit the manifest
```

1. **Read `--json` first.** Every object in `failed[]` must have
   `"reason": "hash"` and a `"path"` the bump was supposed to touch. A `hash` on
   anything else, or any `unlisted`, `missing`, `not_tracked` or `symlink` entry,
   means something other than a version bump rode in on the branch. Regenerating
   first destroys that evidence: the new manifest describes whatever is on disk,
   including whatever you did not look at. This is the same rule that made the
   202 conflicts classifiable in P60 and it fails the same way if skipped.
2. **Then re-seal on the bump's branch, in the same PR.** The generator hashes
   the working tree, so it has to run where the bumped lockfile is. Merging a
   bump and re-sealing on `main` afterwards leaves `main` red at step 10 in
   between, which fails every other open PR too.

If `MANIFEST.sha256` conflicts when a bump branch is brought up to date with
`main`, resolve it by rerunning the generator, never by merging hash lines by
hand. A hand-merged manifest is a manifest nothing produced.

**Re-sealing clears step 10 and stops there.** Step 11 is
`freeze-dependencies.ps1 -VerifyOnly`, which is the *second* row of the table in
**What it seals against** and asks a different question — is this dependency
graph an approved one. A bump changes the answer to both, and only the seal's
answer is mechanical. Measured on PR #19 at `70cdde6`, run `35484566311`: step
10 `Delivered source seal` **success**, step 11 throws at
`freeze-dependencies.ps1:43`, `Dependency lock hashes differ from the approved
freeze baseline`. Refreshing the freeze is an owner action with three criteria
in `docs/RELEASE_SUPPLY_CHAIN.md`, and that document is explicit that no
workflow can approve a graph because no workflow can commit one. So: re-seal as
part of taking the bump; do **not** refresh the freeze as part of taking the
bump.

The verifier requires `git` and says so rather than falling back to a directory
walk. A tree with no git metadata cannot be asked which files were delivered,
only which happen to be on disk, and the difference between those two questions
is the entire 914-file hole this replaced.
