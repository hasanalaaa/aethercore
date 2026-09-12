# P58 — make the gates real, then pay the ledger down

Paste into Claude Code:  `Read and follow phase21-workspace/docs/phase58/P58-GATES-AND-DEBT.md and work it end to end`
To resume after any interruption, paste the same line again and continue from the
P58 rows in `phase21-workspace/docs/LEDGER.md`.

Host: the Mac at `/Users/hasanalaaa/dev/aethercore`. Items 5–7 need the physical
Windows PC and are gated on it being reachable. Never
`~/Documents/ZZ-OLD-AetherCore-DO-NOT-USE`. One session, sequential.

P56 and P57 closed the design and the assistant. Seventeen debt rows remain, and
the first two of them mean this project's CI story is half-built.

---

## RULES

- Every check has an **EXPECTED** value. Observed differs → stop that item, record
  the raw observation verbatim, move to the next independent item. Do not
  theorise, do not redefine the criterion, do not work around it.
- Never report a gate passed without numbers.
- Explicit paths when staging. **Never `git add -A`.**
- Commit and push after every item; move the LEDGER row in the same commit.
- End every commit with `Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>`.
- **Do not report a build clean by reading a piped tail.** P56 read
  `cargo check | tail -3`, saw warnings, called it clean, and had committed a
  service that did not build. Check the exit code.
- `svelte-check` stays at zero errors and zero warnings. That is the baseline now.
- No new colour literals. The icon is settled — the Æ mark.

---

## ITEM 1 — `PageId` says "assistant" and the assistant is not a page

`apps/ui/src/lib/navigation.ts:4` lists `'assistant'` in `PageId`, the union whose
own comment calls it "stable semantic route identifiers". The drawer is not a
route: `DIRECTION.md` rejected a dedicated screen explicitly, and
`NavigationRail.svelte` carries the comment "the drawer is not a screen and must
not read as one".

A union that claims to enumerate routes, containing something that is not one, is
an invitation for the next reader to give it a nav entry.

Remove it from `PageId`. `IconName` keeps `'assistant'` — the drawer does have an
icon, and that union is about icons. If something genuinely depends on the page
membership, say what and why it should stay instead.

EXPECTED: build clean, `svelte-check` 0/0, and the drawer still opens on every
screen with `Ctrl+/`.

---

## ITEM 2 — the CI gates are fiction. Make them real.

Two rows that only make sense read together:

- **`DBT-P55-001`** — `phase21-workspace/.github/workflows/{ci,fuzz,release}.yml`
  have **never been dispatched**. Not failed: never run.
- **`DBT-P55-002`** — `cargo fmt --all -- --check` fails on **1,247 files**, and
  `ci.yml`'s formatting gate would have caught it.

So the project carries three workflows nobody has ever executed, and the first
thing one of them would do is fail on 1,247 files. Every "the gates are green"
statement in this repo's history refers to gates that were run by hand.

**2.A — find out why they never dispatched.** Path filters, branch filters, a
workflow file outside `.github/` at the repo root, a disabled workflow — measure
it, do not guess. Note `windows-installer.yml` lives at the **repo root**
`.github/`, and these three live under `phase21-workspace/.github/`. GitHub only
reads `.github/workflows/` at the repository root. If that is the cause, say so
plainly — it means these three files have never been workflows at all.

**2.B — fix the formatting.** `cargo fmt --all` and commit it as its own commit,
touching nothing else, so the diff is reviewable as pure formatting. Record the
file count before and after.

**2.C — make them run.** Whatever 2.A found, correct it so all three dispatch.
EXPECTED: a green run id for each, recorded. A workflow that passes because its
failing step was removed, skipped, or marked `continue-on-error` is a **FAIL** —
this project has been bitten four times by an instrument that lies.

If a workflow genuinely cannot run on a hosted runner, say so with the evidence
and propose what would.

---

## ITEM 3 — the icon cannot be regenerated

`DBT-P55-005`: `apps/desktop/icons/SOURCE.json` names
`design/icon/aethercore-mark.svg` as the source of the shipped icon set, and that
file **is not in the tree**. The PNGs match their recorded hashes, so the artwork
is authentic — it simply cannot be rebuilt from source.

The file exists in the design worktree at
`/Users/hasanalaaa/dev/aethercore-design/design/icon/`. Restore it, verify the
pipeline regenerates the set byte-identically to the committed PNGs, and close the
row. If the regenerated set differs, **stop** — that is a finding about the
pipeline, not a reason to overwrite the shipped artwork.

---

## ITEM 4 — two product decisions, taken rather than deferred again

Both have been open since P47 and both are decisions, not defects.

**`DBT-P47-003`** — `perProcessorBusyBp` is never populated on Windows. Decide:
implement it, or record it as deliberately out of scope with the reason. If you
implement, state the observer-effect and cadence cost explicitly rather than
choosing it silently.

**`DBT-P47-004`** — GPU adapter identity and VRAM stay empty; §41.16 showed the
data exists via WMI while PDH gives engines only. Same choice, same requirement:
if you wire it, name the source and prove the number traces to it.

Empty is honest and staying empty is a legitimate answer. Guessing is not.

---

## ITEMS 5–7 — the Windows PC. Check reachability first and record it.

If the machine is unreachable, mark these `BLOCKED-MACHINE`, say so plainly, and
finish everything else. Do not wait on it.

**ITEM 5 — `DBT-P55-008`, the installer renders English only** on a machine whose
Windows UI language is Arabic. This product ships EN/AR as equals and its owner's
own machine is Arabic. The installer is the first thing a user sees and it does
not speak to half its audience. Localise the Burn UI, or record precisely what
WiX makes impossible.

**ITEM 6 — `DBT-P55-009`, `/uninstall` still shows the "Modify Setup" chooser
with Repair as the focused default.** A user pressing Enter to uninstall repairs
instead — a P55 session did exactly that and ran an unplanned repair. Make
`/uninstall` uninstall.

**ITEM 7 — `DBT-P49-003`**, the bundle leaves an elevated log in `%TEMP%` after
install and uninstall — three survivors, measured. Note P55 found the filename
pattern had changed from `AetherCore_Setup_*` to `AetherCore_*` because Burn
derives it from the bundle Name, **so a sweep written against the old pattern
reports zero and is wrong**. Fix the survivor, and fix the sweep's pattern so it
cannot silently pass again.

---

## THE OWNER REGISTER — record, do not attempt

- **`DBT-P55-004`** — the recovery media is **not attached**; zero removable
  volumes carry `\sources\boot.wim`. It has never been boot-tested. Gate 4 cannot
  start until it is. Two steps: attach it, boot from it once and confirm the
  recovery environment sees the system disk and `D:\WindowsImageBackup`.
- **Gate 4's device** — Windows Update offers zero drivers; the candidate list is
  in `docs/phase55/GATE4-CANDIDATES.md`. Printer, HID or USB class only. Never
  storage, chipset, GPU or anything in the boot path.
- **`DBT-P55-003`** — `D:` cannot hold a second full image beside the existing
  one: C: uses 571.6 GB against 410.5 GB free. A second target or a deletion is
  the owner's call.
- **`DBT-P55-007`** — `signatureValidationMode=require` is not enforced on a
  developer machine; a cold restore with corrupted fingerprints still succeeded.
  It **is** enforced on CI. Record the consequence plainly: a developer-machine
  build is not evidence that packaging dependencies were signature-checked, which
  makes a green CI the only trustworthy build path.
- **the code-signing certificate** — deferred by explicit decision.

---

## REPORT

- why the three workflows never dispatched, and the green run id for each
- the `cargo fmt` file count before and after
- whether the icon set regenerates byte-identically
- the two P47 decisions, each with its reason
- what the Windows items found, or `BLOCKED-MACHINE` with the evidence
- the LEDGER's open count before and after
- the single next action, in one sentence
