# P59 — the readers that fail open, the PR flood, and whether the freeze is reachable

Paste into Claude Code:  `Read and follow phase21-workspace/docs/phase59/P59-READERS-AND-FREEZE.md and work it end to end`
To resume, paste the same line again and continue from the P59 rows in
`phase21-workspace/docs/LEDGER.md`.

Host: the Mac at `/Users/hasanalaaa/dev/aethercore`. Item 4 needs the physical
Windows PC and is gated on reachability. Never
`~/Documents/ZZ-OLD-AetherCore-DO-NOT-USE`. One session, sequential.

Read `docs/phase58/P58-REPORT.md` first. P58 made the gates real for the first
time — `actions/workflows` went from 1 to 5, because three of them were never
workflows at all. What it found while doing that is why this phase exists.

---

## RULES

- Every check has an **EXPECTED** value. Observed differs → stop that item, record
  the raw observation verbatim, move to the next independent item. Do not
  theorise, do not redefine the criterion, do not work around it.
- Never report a gate passed without numbers, and **never report a pass from a
  reader you have not proved opens its file**. That is this phase's subject.
- Explicit paths when staging. **Never `git add -A`.**
- Commit and push after every item; move the LEDGER row in the same commit.
- End every commit with `Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>`.
- Check exit codes. Do not read a piped tail and call it clean.
- `svelte-check` stays at 0 errors / 0 warnings. `cargo fmt --all -- --check`
  stays at exit 0 — P58 got it there from 1,254 hunks across 128 files.

---

## ITEM 1 — `DBT-P58-005`: ten readers that fail open

P58 moved three workflow files and broke ten gate scripts. One,
`static_validate.py`, crashed visibly. **The other eight would have completed
their report while reading `""`** — so any check that asserts the *absence* of
something would have passed on a file it never opened. That defect was about to
ship inside the commit whose purpose was fixing exactly this class.

Eight of ten were repaired in `a8946bb`. The class is still open in ten files.

**1.A — enumerate every reader.** Across `scripts/`, `tools/` and anything a
workflow invokes: every place that reads a file, a command's output, or an API
response and continues on failure — `open(...)` inside a bare `try`,
`.read_text()` with a swallowed exception, `except Exception: pass`,
`-ErrorAction SilentlyContinue` feeding a comparison, `|| true`, `2>/dev/null`
whose result is then tested, `.unwrap_or_default()` on a read.

For each, classify:
  **A** — failing open is correct here, and say why
  **B** — a failure would silently produce a pass. **Must be fixed.**
  **C** — cannot tell from the code

Publish the table with `file:line`. **Commit the census before any fix** — it is
the deliverable even if nothing is repaired yet.

**1.B — fix every B at the read, not at the call site.** A reader that cannot
distinguish "absent" from "unreadable" is the defect; a guard at each caller is
nine guards and a tenth caller waiting to be written. One helper that returns a
result rather than a string, and callers that must handle it.

**1.C — prove it.** For each fixed reader, a test that drives the unreadable path
and asserts the gate **fails**. Commit them failing first, as this project does.

EXPECTED: a gate script pointed at a missing file exits non-zero and says which
file. Today, eight of them would have said PASS.

---

## ITEM 2 — twelve Dependabot PRs that cannot pass

Enabling `dependabot.yml` — which had also never been read, for the same
root-directory reason — opened **12 pull requests**. Every one changes a lockfile,
and every lockfile change fails `OMEGA-RB-001` by definition. They are CI load and
noise that hides real failures.

Decide and act, recording the reason:

- **pause Dependabot** until the freeze question in Item 3 is settled, or
- **keep it and stop it running CI** (a path filter, a label, or `paths-ignore`),
  so the PRs accumulate as information without burning runs, or
- **close them** and re-enable when the freeze story is real

Do not leave twelve doomed runs cycling. If the sandbox refuses the API calls —
P58 was refused twice — say exactly which call was refused and give the owner the
one-line `gh` command to run himself.

---

## ITEM 3 — is `OMEGA-RB-001` reachable at all?

`scripts/omega-evidence.py:469` raises it when `dependency_freeze.approved` is
false, and its stated closure is:

> Run `scripts/freeze-dependencies.ps1 -Refresh` on the trusted
> dependency-freeze workstation, review the graph, commit the generated
> locks/baselines, then rerun `-VerifyOnly` and all `--locked` gates.

It has blocked release since Phase 19 — **thirty-nine phases** — and the owner
register has carried "a dependency freeze from a trusted workstation" as
outstanding that whole time. A barrier whose closure has never once been
attempted is not evidence of caution; nobody has established it is *possible*.

**Do not run the freeze.** Establish whether it can be run:

**3.A — read `freeze-dependencies.ps1` in full.** What does `-Refresh` actually
require? Network access, a specific OS, a signing key, a registry mirror, an
approval file, a human review step? Name each requirement and whether it exists
here.

**3.B — what does "the trusted dependency-freeze workstation" mean in this repo?**
Search for its definition. If it is defined, say where and what makes a
workstation trusted. **If it is not defined anywhere, that is the finding** — the
closure names a precondition the project never specified, which is why it has
never been met.

**3.C — which of the five files does it name, and which exist?** P58 recorded
3 of 5 missing. List them, and say for each whether it is meant to be generated by
the freeze or to pre-exist it.

**3.D — state plainly what the owner would have to do.** Concretely: a machine, a
network state, a command, an approval. If the honest answer is "this can be run on
the Mac, offline, in ten minutes", say that — the barrier may be inherited
language rather than a real constraint. If it genuinely needs something he does
not have, name it and put it in the owner register with what it would cost.

Either answer is worth the session. What is not acceptable is a fortieth phase
where it stays open because nobody looked.

---

## ITEM 4 — the Windows installer defects. Gated on the machine.

Check reachability and record how. If unreachable, mark `BLOCKED-MACHINE` with
the evidence and finish everything else — do not wait.

- **`DBT-P55-008`** — the installer renders **English only** on a machine whose
  Windows UI language is Arabic. This product ships EN/AR as equals and its
  owner's own machine is Arabic. Localise the Burn UI, or record precisely what
  WiX makes impossible.
- **`DBT-P55-009`** — launched with `/uninstall`, the bundle still shows the
  "Modify Setup" chooser with **Repair as the focused default**. A P55 session
  pressed Enter to uninstall and ran a full repair. Make `/uninstall` uninstall.
- **`DBT-P49-003`** — the bundle leaves elevated logs in `%TEMP%`. Fix the
  survivor, and fix the sweep's pattern: it was written against
  `AetherCore_Setup_*` and Burn now derives `AetherCore_*` from the bundle Name,
  so the sweep reports zero and is wrong.

---

## THE OWNER REGISTER

- **`DBT-P55-004`** — the recovery media is not attached and has never been
  boot-tested. Gate 4 cannot start. Attach it, boot from it once, confirm the
  recovery environment sees the system disk and `D:\WindowsImageBackup`.
- **Gate 4's device** — candidates in `docs/phase55/GATE4-CANDIDATES.md`.
  Printer, HID or USB class only.
- **`DBT-P55-003`** — `D:` cannot hold a second image: C: uses 571.6 GB against
  410.5 GB free.
- **`DBT-P55-007`** — signature validation is inert on a developer machine and
  enforced only on CI, so a green CI is the only trustworthy build path.
- **the code-signing certificate** — deferred by explicit decision.
- **the dependency freeze** — Item 3 will say what it actually needs.

---

## REPORT

- the A/B/C reader census with `file:line`, and how many B existed
- proof that a fixed reader fails on a missing file, with the exit code
- what you did about the twelve PRs, and any API call the sandbox refused
- whether `OMEGA-RB-001` is reachable, and exactly what the owner must do
- the Windows items, or `BLOCKED-MACHINE` with evidence
- the LEDGER's open count before and after
- the single next action, in one sentence
