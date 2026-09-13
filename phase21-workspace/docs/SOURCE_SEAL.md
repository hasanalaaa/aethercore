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

The verifier requires `git` and says so rather than falling back to a directory
walk. A tree with no git metadata cannot be asked which files were delivered,
only which happen to be on disk, and the difference between those two questions
is the entire 914-file hole this replaced.
