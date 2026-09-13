# P60 — land the freeze CI already produces, then make the seal mean something

Paste into Claude Code:  `Read and follow phase21-workspace/docs/phase60/P60-FREEZE-AND-SEAL.md and work it end to end`
To resume, paste the same line again and continue from the P60 rows in
`phase21-workspace/docs/LEDGER.md`.

Host: the Mac at `/Users/hasanalaaa/dev/aethercore`. Item 6 needs the physical
Windows PC and is gated on reachability. Never
`~/Documents/ZZ-OLD-AetherCore-DO-NOT-USE`. One session, sequential.

Read `docs/phase59/P59-REPORT.md` first.

---

## RULES

- Every check has an **EXPECTED** value. Observed differs → stop that item, record
  the raw observation verbatim, move to the next independent item.
- Never report a gate passed without numbers, and never trust a gate you have not
  proved can fail. P59 found one that structurally cannot.
- Explicit paths when staging. **Never `git add -A`.**
- Commit and push after every item; move the LEDGER row in the same commit.
- End every commit with `Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>`.
- Check exit codes. `svelte-check` 0/0, `cargo fmt --all -- --check` exit 0.

---

## ITEM 1 — land the freeze. CI has been generating it and throwing it away.

`.github/workflows/windows-installer.yml:110` runs
`./scripts/freeze-dependencies.ps1 -Refresh` — the exact command `OMEGA-RB-001`'s
closure names. Run `34683812129`, step 10 of 12, succeeded in 62 seconds:
`Phase 9 dependency graph frozen`.

Line 118 then uploads `phase21-workspace/out/release/**/*.exe` and **nothing
else**, while the script's own closing line says to commit the lockfiles *and*
every `release/dependency-*` baseline together. The runner is destroyed with the
closure on it.

A release blocker has stood for thirty-nine phases while CI produced its closure
on every run and discarded it.

**1.A — add the freeze outputs to the upload.** Read the script and enumerate
exactly what `-Refresh` writes — `release/dependency-locks.sha256` is one; find
the rest rather than assuming three. Add them as their own artifact, not mixed
into the installer's.

**1.B — run the workflow once**, download the artifact, and commit the lockfiles
and baselines together in one commit that touches nothing else.

**1.C — prove the barrier closes.** Run `scripts/omega-evidence.py` and show
`OMEGA-RB-001` absent from the blocker list, with the before/after output.

EXPECTED: the blocker is gone because the evidence exists, not because the check
was changed. **If you edited `omega-evidence.py` to make it pass, that is a FAIL.**

**1.D — fix the POSIX path defect P59 found by reading**: lines 9–12 build paths
with `\`, so on POSIX the script creates a file literally named
`release\dependency-locks.sha256`. It only ever ran on Windows, so this has never
bitten — fix it before it does.

---

## ITEM 2 — decide whether a hosted runner is a trustworthy freeze source

P59 established that "the trusted dependency-freeze workstation" is **defined
nowhere** — twenty occurrences, no criteria, the only attributes being "Windows"
and "connected". Item 1 closes the barrier using a GitHub-hosted runner. Say
explicitly whether that is acceptable, and why, because the next person will ask.

The evidence that bears on it, and you should weigh it rather than assert:

- `DBT-P55-007` measured that `signatureValidationMode=require` is **inert on a
  developer machine** — a cold restore with deliberately corrupted fingerprints
  still exited 0 — and **enforced on CI**, which is where `NU3034` surfaced. On
  the one control that can be measured, CI is the *more* trustworthy source, not
  the less.
- A hosted runner is ephemeral and its logs are retained; a developer machine is
  persistent and its state is not recorded anywhere.
- Against: a hosted runner is infrastructure you do not control, and provenance
  you cannot inspect after the fact beyond the log.

Write the decision into the repo as the definition that was missing: what makes a
freeze source acceptable for this project, in two or three criteria that can be
checked. A phrase nobody defined is what cost thirty-nine phases.

---

## ITEM 3 — the source seal does not seal

Two rows, one defect:

**`DBT-P59-002`** — `p30-fulltree-spot-hash-ok` **cannot fail**. It regenerates
the record from the same tree it then compares against. It has reported green for
every phase it has ever run in, and that green means nothing.

**`DBT-P59-003`** — `MANIFEST.sha256` lists **551 files against 1,457** in the
tree: 909 unlisted, 200 hash conflicts, 3 listed but missing.

So the seal covers 38% of the source, disagrees with 200 of the files it does
cover, and is verified by an instrument that is structurally incapable of
reporting a problem.

**This is a decision about what the seal is for, not a number to fix.** Answer it
before changing anything:

- **What is it sealing against?** A tampered working tree? A build that does not
  match its source? An auditor asking what shipped? Each implies a different scope.
- **What belongs inside it?** 1,457 files includes vendored trees, generated
  output, and phase archives. Sealing generated output means the seal breaks on
  every build. Name the boundary and defend it.
- **What are the 200 conflicts?** Files that legitimately changed since the
  manifest was written, or genuine drift? Classify them before regenerating —
  regenerating first destroys the evidence of which it was.

Then make the verifier able to fail: it must read the committed manifest, hash the
tree independently, and diff. Prove it with a test that corrupts one byte and
asserts a non-zero exit. Commit that test failing first.

EXPECTED: a seal whose scope is stated, whose coverage is complete within that
scope, and whose verifier fails when a file changes.

---

## ITEM 4 — `DBT-P59-001`, the lockfile the npm ecosystem cannot see

Dependabot's npm ecosystem points at `apps/ui` while `pnpm-lock.yaml` sits a level
above. Reproduced: `ERR_PNPM_OUTDATED_LOCKFILE`, exit 1. Fix the directory, or
state why the layout should change instead.

---

## ITEM 5 — the twelve pull requests

P59 chose to stop Dependabot at the source — `open-pull-requests-limit: 0` on all
three ecosystems with the resumption condition written in the file (`31be244`) —
and rejected "keep them without CI" because a skipped check reports without
measuring, which is the class that has bitten this project four times. That
reasoning stands; do not revisit it.

The twelve existing PRs are still open. `gh pr close` was refused by the sandbox
as an external write. Give the owner the command, once, at the top of your report.

**If Item 1 closes `OMEGA-RB-001`**, reconsider the pause: the resumption
condition in `dependabot.yml` is exactly that. Say whether it is now met.

---

## ITEM 6 — the Windows installer defects. Gated on the machine.

Record reachability and how you measured it. If unreachable, `BLOCKED-MACHINE`
with evidence, and finish everything else.

`DBT-P55-008` English-only installer on an Arabic Windows · `DBT-P55-009`
`/uninstall` shows "Modify Setup" with Repair focused · `DBT-P49-003` elevated
logs survive in `%TEMP%`, and the sweep's pattern stopped matching when Burn
started deriving the log name from the bundle Name.

---

## THE OWNER REGISTER

- **the twelve PRs** — the `gh pr close` loop, at the top of the report
- **`DBT-P55-004`** — recovery media not attached, never boot-tested. Gate 4
  cannot start.
- **Gate 4's device** — `docs/phase55/GATE4-CANDIDATES.md`. Printer, HID or USB
  only.
- **`DBT-P55-003`** — `D:` cannot hold a second image: 571.6 GB used vs 410.5 free.
- **the code-signing certificate** — deferred by explicit decision.

---

## REPORT

- the `gh pr close` command, first line
- what `-Refresh` writes, and the commit that landed it
- `omega-evidence.py` before and after, showing the blocker gone on evidence
- the freeze-source criteria you wrote, and where they live
- the seal's stated scope, the classification of the 200 conflicts, and the exit
  code of the verifier against a one-byte corruption
- the LEDGER's open count before and after
- the single next action, in one sentence
