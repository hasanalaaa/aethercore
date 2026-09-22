# W1 — the first session to run on the Windows machine

Every phase before this one ran on a macOS laptop. Five debt rows have been
`BLOCKED-MACHINE` since Phase 55 because nothing could reach a Windows PC, and
three separate sessions measured that unreachability and recorded it honestly
rather than guessing. You are on that machine. These rows are yours.

Read `docs/LEDGER.md` §1 first — it is the resume point, and each row's evidence
column names the commit or the command that produced it.

---

## Ground rules, same as every phase

* **A claim without a command that produced it is a belief.** Say which it is.
* **If a measurement contradicts this brief, the measurement wins.** Say so and
  record it. Three of the last four phases corrected their own brief this way.
* Stage with **explicit paths**. Never `git add -A`. Run `git status` and
  `git diff --cached --stat` before every commit and read them.
* The order for any commit that changes tracked files:
  `git add <explicit paths>` → `python scripts/regenerate-source-manifest.py`
  → `git add phase21-workspace/MANIFEST.sha256` → `git diff --cached --stat`.
* Never glob `scripts/*.py` when running gates — a previous phase did and it
  executed `_build_p27_*.py`, which rewrote three tracked artifacts.
* You are on `main` with other sessions pushing to it. `git pull --rebase`
  before you push, and if `main` moved under you, re-run the gates.

## Verify before you fix

These rows were written from one machine's state in Phase 55, and that machine
may not be this one. **Reproduce each before treating it as real.** A row that
does not reproduce is a finding worth as much as a fix — say so, and say what
you measured instead. Do not silently "fix" something you never observed.

---

## ITEM 1 — `DBT-P55-008`: the installer is English-only on Arabic Windows

The product's stated invariant is EN/AR parity, enforced by an audit gate. The
gate passes: `scripts/test-phase12-localization.py` reports 34/34 with 1694
EN/AR keys. The installer is outside it.

Confirm this machine's Windows UI language is Arabic (`Get-WinUILanguageOverride`,
`Get-SystemPreferredUILanguage`), then build or obtain the bundle and run it.
Capture what it renders. Phase 55 captured 80 frames, all English — say whether
that reproduces.

Then find where the boundary is: WiX/Burn localization is a `.wxl` file set and a
bundle-level `Language` attribute, and the diagnosis is whether the strings are
missing, or present and never selected. Those have different cures.

## ITEM 2 — `DBT-P55-009`: `/uninstall` presents "Modify Setup" with Repair focused

An unplanned repair ran to completion this way, captured in
`docs/phase55/installer-screens/07-repair-complete-unplanned.png`. A user who
asks to uninstall and gets a repair has been given an operation they did not
request — this is a consent defect, not a cosmetic one.

Reproduce it, then decide whether the fix is the bundle's command-line handling
or the theme's default focus, and say why the other is not the cause.

## ITEM 3 — `DBT-P49-003`: an elevated log is left in `%TEMP%`

After install and after uninstall. The sweep pattern that used to match it
stopped matching — establish what it actually writes, where, and with what ACL,
before changing the sweep. A log written by an elevated process into a
user-writable directory is the part that matters; the tidiness is secondary.

## ITEM 4 — `DBT-P55-004`: the recovery media is absent, not untested

Phase 55 measured **zero** removable volumes carrying `\sources\boot.wim` or
`\bootmgr`. The row's own text is careful about this: it is not "never booted",
it is "not found". Enumerate removable volumes on this machine and say what is
actually there. If the media exists, boot-test it and record the result. If it
does not, that is the finding and the row should say so in this machine's terms.

Related: `DBT-P55-003` says `D:` cannot hold a second full system image beside
the existing one — C: at 571.6 GB used against 410.5 GB free on `D:`. Re-measure
both numbers on this machine before any imaging is attempted.

## ITEM 5 — report

Write `docs/phase71/W1-REPORT.md`. For each item: what you ran, what it produced,
whether the row reproduced, and what you changed. Move each row in
`docs/LEDGER.md` with evidence. A row you could not close stays `OPEN` with the
reason — that has been the standard for seventy phases and it is why the ledger
is worth reading.

---

## Environment notes for this machine

The repository is public: `git clone https://github.com/hasanalaaa/aethercore`.
The workspace root is `phase21-workspace` — every script path in this brief is
relative to it.

What CI installs, and therefore what a full local gate run needs: Rust 1.97.1
(`rust-toolchain.toml` pins it), pnpm 11.22.0, Node 22.16.0, .NET 8, and the
Windows ADK Deployment Tools — `.github/actions/install-windows-adk/action.yml`
carries the exact recipe including two corrections that each cost a CI run to
find. Use it rather than improvising.

You do not need the full gate set for items 1–4, which are installer and media
work. You do need it before any commit that touches tracked source. The local
set is listed at the top of `docs/phase69/P69-REPORT.md`.

`main` is green — run `35484711267` measured `936a26d` with the windows job at
31/31. Keep it that way: verify a green `main` only by a run whose `head_sha`
equals the sha you are claiming, and use `workflow_dispatch` on that sha when the
concurrency group cancels the push-triggered run.
