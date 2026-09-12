# P55 — fix CI, restore the progress ledger, prepare Gate 4

Paste into Claude Code:  `Read and follow phase21-workspace/docs/phase55/P55-CI-LEDGER-GATE4.md and work it end to end`
To resume after any interruption, paste the same line again and continue from the
progress table you build in Item 2.

**Runs on the physical Windows PC at `C:\dev\aethercore`, elevated.**

    ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole(544)

EXPECTED `True`. If `False`, stop and say so — do not spawn `-Verb RunAs` children.
Launch with `claude.cmd`, not `claude`: `ExecutionPolicy` blocks the `.ps1`
wrapper, and the Store-packaged build cannot inherit elevation at all.

`core.autocrlf` MUST stay `false`. Verify before your first commit.

---

## RULES

- Every check has an **EXPECTED** value. Observed differs → stop that item, record
  the raw observation verbatim, move to the next independent item. Do not
  theorise, do not redefine the criterion, do not work around it.
- Never report a gate passed without numbers.
- Explicit paths when staging. **Never `git add -A`.** `git status` before, not after.
- Commit and push after every item, and move the progress row in the same commit
  as the work.
- End every commit message with `Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>`.
  The last five commits on this repo dropped that and every other convention.
- A security regression stops everything immediately.
- Never disable Defender, UAC, Firewall or SmartScreen.
- **This machine has no snapshots.** System Restore is the only rollback and a bad
  driver can leave it unbootable. Before any destructive step: write
  **ACTION / SNAPSHOT / EXPECTED / RECOVERY**, commit it, create a named restore
  point, and verify it by enumeration — `Checkpoint-Computer` returning OK is not
  proof, `Get-ComputerRestorePoint` listing it is.

### The five patterns this codebase produces

Duplicated derivation · belief presented as measurement · tests that never touch
the real path · the measuring instrument itself lying · the empty-state trap.

---

## ITEM 1 — the CI has never succeeded

`.github/workflows/windows-installer.yml` has run three times and failed three
times. The last attempt, `52e6228` at 2026-09-09 18:42 UTC, failed at the step
**Install Windows ADK Deployment Tools**.

This is not a new problem wearing a new hat. P49 measured the same root cause from
the other side: the first x64 build died with

    LNK1181: cannot open input file 'DismApi.lib'

because `crates/system-repair/src/dism_api.rs` declares `#[link(name = "DismApi")]`
with no build-script search path, so `LIB` must carry the ADK's
`…\SDKs\DismApi\Lib\amd64` directory — **`amd64`, not `x64`**.
`scripts/build-arm64-msi.cmd` sets it. `scripts/build-release.ps1`, the production
pipeline, sets nothing and depends on the invoking shell already being right.

**This machine has a working ADK.** Use it as the reference: measure exactly what
is installed, where, and which files the link step actually needs, then make CI
reproduce that — or remove the dependency on a hosted-runner ADK install.

Decide and argue for one, in the progress record, before implementing:

  **(a) fix the ADK install step** on the runner
  **(b) give the crate a build script** that locates `DismApi.lib` itself, so no
    caller has to set `LIB` — this removes the trap for CI *and* for every human
  **(c) vendor the import library** if licensing permits, with the provenance
    recorded and hash-checked the way `DBT-P42-012` handles `vcomp140.dll`

(b) is the only one that also fixes `build-release.ps1`'s silent environment
dependency. Say why if you choose otherwise.

EXPECTED: a green run of the workflow, its run id recorded. **A workflow that
passes because the failing step was removed or `continue-on-error`'d is a FAIL** —
this project has been bitten four times by a measuring instrument that lies.

If the runner genuinely cannot host what the build needs, say so plainly with the
evidence and propose what would: a self-hosted runner already exists in
`release.yml`'s design (`[self-hosted, windows, x64, aethercore-signing]`), though
`actions/runners` currently reports zero.

---

## ITEM 2 — restore the progress ledger, better than it was

Phases 46–53 were resumable because `SESSION_CONTEXT.md` carried a §NN progress
table: every item a row, every row moved in the same commit as its work. Four
sessions died at usage limits and lost nothing because of it.

That stopped. There is no §54. The last five commits carry no phase tag, no
`Co-Authored-By`, and no ledger row. The audit that produced them went to
`docs/phase54/ARCHITECTURE_AUDIT.md` instead — which is **better** as a document
than another section in a 14,512-line file, but it is not a resume mechanism.

Build the replacement. It must:

- live in **one file** that is the single authoritative status of the project —
  not a section of `SESSION_CONTEXT.md`, which is now too large to be read by a
  cold session
- carry one row per open item: id, what it is, status, evidence (a commit or a
  measurement), and which machine it needs
- carry the **debt ledger** — 24 items currently read as open. Verify each against
  the code before copying it forward; P48 found seven rows that were wrong, and a
  ledger reporting closed work as open sends the next session to redo it
- state, at the top, exactly how a cold session resumes: read this file, find the
  first row not closed, continue
- record what is **owner-gated** and what each blocker unblocks, so their absence
  is never mistaken for missing work

Then make `SESSION_CONTEXT.md` point at it in one line at the top, and leave the
history where it is. Do not rewrite 14,512 lines.

**Commit this before Item 3.** Everything after it depends on being resumable.

---

## ITEM 3 — re-establish recovery. Gate 4 cannot start without it.

Gate 4's failure mode is a machine that will not boot. The recovery posture is
currently **worse** than when Gate 4 was first deferred:

- P49 measured `Gate 0f` as regressed: **there is no `D:` and no
  `D:\WindowsImageBackup`**. The 521 GB verified image is gone with the detached
  drive.
- The recovery media was created but **has never been boot-tested**, and P47
  measured it detached as well.

So today there is no proven way back from an unbootable machine. Driver work in
that state is not a calculated risk, it is an uncalculated one.

**3.A — enumerate what is actually attached.** Disks, volumes, by FriendlyName and
size, not by letter. Report what recovery artefacts exist and what does not.

**3.B — if the external drive is re-attached**, re-create the full system image
and verify it by **listing**, not by exit code:

    wbadmin start backup -backupTarget:<L>: -include:C: -allCritical -quiet
    wbadmin get versions -backupTarget:<L>:

EXPECTED: a version dated today. An empty listing is a FAIL regardless of the exit
code — P41 recorded `wbadmin` returning 0 having done nothing.

Enumerate the target's contents with `-Force` before writing to it, and **do not
format anything**.

**3.C — if it is not attached**, stop Item 3, record it, and do Item 5's read-only
preparation anyway. Do not start Gate 4. Say plainly that recovery is unproven.

---

## ITEM 4 — build the installer the owner actually ships

`AetherCoreSetup.exe` exists: P49 built it (`d0398765…`, 1,128,354,997 B) and
installed it for the first time anyone ever had. Two things are wrong with that
artefact today.

**It predates phases 50–54.** Everything since is missing from it — the rebuilt
Overview, the Settings screen, the local AI chat, the sparkline wiring, the
storage-capacity provider, and `ba9973b`'s fix for ephemeral insight state
crossing authenticated principals. That last one is a cross-user data exposure;
shipping an installer without it is not an option.

**Its first-run experience is broken.** `DBT-P49-002` recorded, by observation:

    title bar reads "AetherCore Setup Setup"
    the logo is WiX's stock placeholder, not the Æ mark §48.2 made deliberate
    the welcome screen shows no version and no license
    progress sits on "Initializing..." for the entire 30-second install

This is the first thing any user sees, before the product they installed. The app
behind it is careful about every number it shows; its installer is not.

**4.A — build both artefacts from the current source.**

    MSI    → wix build → wix msi validate → check-msi-payload
    Bundle → wix build Bundle.wxs → AetherCoreSetup.exe

EXPECTED: every exit code 0, `wix msi validate` output **EMPTY** (zero ICE, no
suppression), payload check PASS, 16 file rows. Record both sha256s and byte
sizes. Read the Burn chain out of its own manifest with WiX's extractor, not with
a string scan. EXPECTED: `VCRedist` → `WebView2` → `AetherCoreMsi`.

If Item 1 produced a green CI run, build through that path so the artefact is
reproducible rather than a local one-off. Say which path you used.

**4.B — fix `DBT-P49-002`.** The bundle's name, its branding, its welcome text and
its progress reporting. Use the Æ mark already in the icon pipeline. The version
comes from the same single decider `product-identity` became in P46 — do not add
a second literal. If the license text does not exist yet, say so rather than
inventing one.

Progress sitting on "Initializing..." for the whole install is a measuring
instrument that lies, in the place a user is most likely to believe it. Make it
report the chained packages as they execute, or say precisely why Burn cannot.

**4.C — install it, then uninstall it through the bundle.** Restore point first,
verified by enumeration. EXPECTED: the result is indistinguishable from an MSI
install on every property Gate 5 checks, and uninstall leaves **zero survivors on
all fourteen**. Record every prompt, dialog and reboot request it produces.

**4.D — screenshot every screen of the installer**, in both languages if it
supports them, and put them where the owner can see them. He has never seen it.

## ITEM 5 — prepare Gate 4 completely, then stop at the owner line

Gate 4 needs two things no session can supply. Prepare everything else so that
when the owner supplies them, the gate runs in one pass.

**5.A — nominate candidate devices, with evidence.** Windows Update offers this
machine zero drivers, so there is no candidate yet. Enumerate what is attached and
propose candidates the owner can choose from:

- printer-class, HID-class, or USB-peripheral devices only
- **never** storage, chipset, GPU, network or anything in the boot path
- for each: the device, its current driver version and date, whether a different
  version is obtainable at all, and what rollback would restore

Present them as a short list with the risk of each stated. The Intel Arc display
driver (31.0.101.5007, 2023-11-18) is **not** a candidate — a display driver is
exactly the class that must not be the safe test device. Say so if it appears.

**5.B — write the Gate 4 runbook** as an executable script plus a checklist: the
restore point, the pre-mutation record, the install, the verification, the
rollback, and the post-rollback verification. Every step with its EXPECTED value.
Dry-run everything that can be dry-run.

**5.C — verify the preconditions mechanically** and state which hold:
recovery point · disk image · recovery media present · recovery media boot-tested
· candidate device chosen · driver obtainable.

**Then stop.** Do not install a driver. The two owner actions are:

1. **Boot-test the recovery media.** Creating it is not testing it. Boot from it
   once and confirm the recovery environment can see the system disk and the image.
2. **Choose a device from 4.A.**

Record both as blocking, with the exact steps the owner takes.

---

## REPORT

- the CI decision, the run id of the green run, and what the failing step actually
  needed — or why the runner cannot host it
- the new ledger's path, and the count of debt rows you verified versus copied
- the recovery posture, measured: what exists, what does not, what is proven
- both artefact sha256s, and whether they came from CI or a local build
- the installer screenshots, and what `DBT-P49-002` looks like fixed
- the candidate device list with the risk of each
- exactly what the owner must do for Gate 4 to run, in order
- anything recorded rather than worked around, with its id
