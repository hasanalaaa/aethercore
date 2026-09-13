# P61 — a green windows job, and one definition of "delivered"

P60 closed `OMEGA-RB-001` on evidence and gave the repository a source seal that
can fail. What it also did was make `ci.yml`'s windows job real enough to fail
honestly. This phase is about getting it to green, and about removing the two
places where the repository disagrees with itself.

Standing rules, unchanged:

* Stage with explicit paths. Never `git add -A`. Run `git status` before staging
  and read what you are about to commit.
* A claim without a command that produced it is a belief. Say which it is.
* If a measurement contradicts this brief, the measurement wins. Say so in the
  report and record it.

---

## ITEM 1 — `DBT-P60-006`: the ADK step, then the job

`ci.yml`'s windows job has no ADK step. `windows-installer.yml:37-80` has one
that works, and `crates/system-repair/build.rs` locates `dismapi.lib` from
`KitsRoot10`, so the tests cannot link without it.

Do not write a second ADK step. Two copies of an install recipe drift, and the
one in `windows-installer.yml` already carries two hard-won corrections in its
comments (the `/installpath` that `KitsRoot10` never sees, and `3010` being
success-pending-reboot). Either factor it into something both workflows call, or
copy it byte-for-byte including the comments and say in the commit message which
you chose and why.

Then push the job forward **one step at a time**. After each green run, report
the run id and the step number reached. Stop at the first failure that is a real
defect in the repository rather than a missing tool on the runner, file it, and
say so — reaching a real defect is a successful outcome for this item.

TRAP: a job that goes green because a gate stopped running is not a green job.
For every gate step, record whether it executed and what it asserted, not only
its conclusion. `ci.yml` has ten of them and P58 already found ten readers that
failed open.

## ITEM 2 — `DBT-P60-004` + `DBT-P60-005`: one definition of delivered

`source_seal.tracked_files()` and `omega-evidence.py` disagree about which files
are delivered — 14 of them. Two definitions means at least one gate is measuring
the wrong set, and neither can be trusted until they agree.

Decide which definition is correct **before** changing either side, and write
the decision down with its reason. `git-tracked under the sealed root` is
already stated and defended in `docs/SOURCE_SEAL.md`; if `omega-evidence.py`
needs a different set, that difference is a requirement and must be named, not
accommodated silently.

Then close both rows together. Closing one while the other still holds the old
definition is how the disagreement comes back.

Note also that `omega-evidence.py` is currently unrunnable on a built workspace:
13 full copies of `target/`, 36 GB, killed at 40 minutes against 70.7 s clean.
Whether that is in scope for this item is your call — but if you fix it, the fix
is a scope rule, not a faster walk.

## ITEM 3 — `DBT-P60-001`: the llama-cpp signature

`-Refresh` re-seeded the graph and the result did not compile. Two options:

* adopt the `0.1.156` signature and **re-measure** `tests/embedded_generation.rs`
  against it, or
* pin the chain whole.

Either is defensible. What is not defensible is adopting the new signature and
assuming the test still means what it meant — the embedded model is a product
invariant, and a test that compiles is not a test that still measures generation.
If you adopt, the report must carry an actual generation result, not a build.

## ITEM 4 — `.gitattributes`: `*.cmd` is executed, not captured

`50cc466` set `* -text` and it is the right root-cause fix for the freeze. But
it also leaves the two `.cmd` files LF on Windows, where `core.autocrlf` used to
deliver them CRLF:

* `scripts/build-arm64-msi.cmd` — line continuation `^` at lines 91-95
* `scripts/p36vm/p36_relbuild.cmd`

`cmd.exe` is unreliable with LF-only line endings around multi-line constructs,
and `^` continuation is one of them. The file's own comment says leaving them
alone avoids "picking a side" — but LF **is** a side, and it is the wrong one
for a file that is executed rather than captured. Captured console output under
`docs/phase36/evidence/` keeps its CRLF because the CRLF is the evidence; a
`.cmd` has no such claim.

Add `*.cmd text eol=crlf` and state the distinction in the comment. These are
the ARM64 VM reproduction recipes (`DBT-P55-006`), so this cannot be tested from
the Mac — say that plainly rather than claiming it works.

## ITEM 5 — Dependabot, only if main is green

The pause condition was restated once already: restore the limit to 5 when
`ci.yml`'s windows job is green on main. Do not restate it a second time. If
ITEM 1 does not reach green, this item is `not attempted`, and that is the
correct outcome — not a failure.

---

## What the report must contain

1. The run id and step reached for every CI attempt, including the failures.
2. For ITEM 2, the decision and its reason **before** the diff that implements it.
3. For ITEM 3, a generation result if you adopted, or the pin set if you pinned.
4. The ledger count before and after, with the counting rule stated — P60's
   report and an independent grep disagreed (35/19 vs 38/20) because the rule was
   not written down. Write it down.
5. Anything in this brief that measurement contradicted.
