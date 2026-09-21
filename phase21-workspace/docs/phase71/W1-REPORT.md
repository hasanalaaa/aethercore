# P71 W1 — the five BLOCKED-MACHINE rows, measured on the Windows PC

Host: the physical Windows PC, `C:\dev\aethercore`, Windows 11 Pro 10.0.26200.
Date: 2026-09-21. Branch `main` from `936a26d`. Session **not elevated**
(Medium integrity; `BUILTIN\Administrators` present but deny-only).

This is the first session in the project to run on Windows. Five rows had sat
`BLOCKED-MACHINE` since P55 because three sessions on the Mac measured the PC as
unreachable and said so instead of guessing. Those measurements were correct and
are now moot: this is that PC.

**Every figure below names the command that produced it.** Where I could not
measure something, the row says so and claims nothing.

The short version. **Four of the five rows reproduce; one does not, and its
non-reproduction is the finding.** `DBT-P55-008` asserts a machine whose Windows
UI language is Arabic — this machine's UI language is `en-US`, measured four
ways, so the row's precondition does not hold here even though the underlying
defect is real and is now proven from source. `DBT-P55-009` reproduced exactly,
with the focused control read out of the live UI rather than eyeballed, and the
cause is **not** the command-line handling. `DBT-P49-003` reproduced with **four**
survivors, not three, and the ACL is the part that matters. `DBT-P55-004`
reproduced verbatim: the recovery media is absent. Two things had to be fixed
before any of it could be trusted, and one of them means **the brief's own
mandated push sequence would have damaged the repository** if run in order.

---

## 0. Two things that had to be true first

### 0.1 The working tree did not match the committed bytes, and `git status` hid it

`python scripts/source_seal.py` on a clean checkout of green `main`:

```
Source seal: FAILED - 1379 of 1491 tracked files verified, 112 problems
```

All 112 were `hash`. The cause is line endings. `.editorconfig` on disk was 198
bytes with CRLF; the committed blob is 183 bytes with LF:

```
$ od -c phase21-workspace/.editorconfig | head -2
0000000   r   o   o   t       =       t   r   u   e  \r  \n  \r  \n   [
$ git show HEAD:phase21-workspace/.editorconfig | od -c | head -2
0000000   r   o   o   t       =       t   r   u   e  \n  \n   [   *   ]
```

This clone was made at `48ff76e`, which predates the root `.gitattributes` and
its `* -text`. That file exists precisely to stop this (its own header cites
P60 run `34746478525`), but it cannot retroactively rewrite a working tree that
was already on disk, and git's stat cache reported the tree clean throughout —
`git check-attr text eol` correctly resolves `text: unset`, so git was applying
no conversion and simply never re-read the files.

Repaired by restoring the tree from the index:

```bash
git ls-files -z | xargs -0 rm -f
git checkout-index -f -a
```

(`git checkout-index -f -a` alone is not enough — it silently skips entries it
believes current. The files must be removed first.) After:

```
Source seal: OK - 1491 of 1491 tracked files verified
```

**This contradicts the brief and the measurement wins.** The brief says to run
`regenerate-source-manifest.py` after the rebase *always, whether or not there
was a conflict*, and then to check the seal. Run in the state this machine was
in, that order rewrites `MANIFEST.sha256` with the CRLF hashes of 112 tracked
files — a manifest that verifies on this one working tree and nowhere else,
including CI. **Regeneration must follow a verified tree, not precede it.** The
safe order on any Windows checkout is: verify the seal first; if it fails on
hashes, repair the tree; regenerate only once `source_seal.py` is green for
reasons that have nothing to do with your own edits.

### 0.2 `git status` is not a safe pre-commit check on this host

After the repair, `git status --short` prints 1515 ` M` entries while
`git diff --name-only` prints **0**, and the blob hashes agree exactly:

```
$ git hash-object .gitignore     -> 81e330a78181f58295c9b0b1ebee277bb38da26a
$ git ls-files -s .gitignore     -> 100644 81e330a7...26a 0  .gitignore
```

The ` M` is a stale stat cache that survives `git update-index --really-refresh`
on git 2.54.0.windows.1. The standing rule says to read `git status` before
every commit; on this host that reading is noise. `git diff --cached --stat` —
which the same rule also requires — is index-versus-HEAD and is unaffected, and
it is what I used.

It is not merely cosmetic: `git pull --rebase` refuses to run against it —

```
error: cannot pull with rebase: You have unstaged changes.
```

— while `git diff --name-only` reports zero files. The cure is to rebuild the
index, which is safe only once both diffs are empty:

```bash
git diff --name-only          # must be 0
git diff --cached --name-only # must be 0
rm .git/index && git reset
```

After that `git status` reports 0 entries and the rebase proceeds. Anyone
working this repository from a Windows clone should expect to do this once,
after the §0.1 repair.

### 0.3 Python was absent

Only the Microsoft Store stub (`python.exe`, version `0.0.0.0`, exits silently)
was on `PATH`. The seal, the manifest regeneration and the whole python gate set
need a real interpreter, and the brief's push sequence calls two of them. I
installed one per-user, no elevation:

```
winget install --id Python.Python.3.13 --exact --source winget --scope user --silent
-> Python 3.13.15, C:\Users\husen\AppData\Local\Programs\Python\Python313
```

**This host otherwise already carries the full CI toolchain**, which is worth
recording because it retires the largest drag P69 named:

| tool | here | CI pin | |
|---|---|---|---|
| rustc / cargo | 1.97.1 | 1.97.1 | matches |
| pnpm | 11.22.0 | 11.22.0 | matches |
| node | v22.23.2 | 22.16.0 | **drift** |
| .NET | 8.0.424 | 8 | matches |
| wix | 6.0.2 | 6.0.2 | matches |

**P69 §6 item 1 does not apply to this machine.** The Mac could not compile any
Rust in this workspace, so every Rust verdict cost a ~50-minute CI round trip.
This host compiles. Whoever works here next should stop paying that toll.

---

## 1. `DBT-P55-008` — the installer renders English only

### What I ran, and what it produced

```
Get-WinUILanguageOverride              -> (empty: no override set)
Get-SystemPreferredUILanguage          -> en-US
Get-UICulture                          -> en-US
Get-WinSystemLocale                    -> en-US
HKLM\SYSTEM\CurrentControlSet\Control\MUI\UILanguages -> en-US   (only)
Get-WinUserLanguageList                -> ar-IQ            (single entry)
Get-WinHomeLocation                    -> GeoId 121, Iraq
```

### Does the row reproduce? **No — and that is the finding**

The row says "on a machine whose Windows UI language is Arabic". **This
machine's Windows UI language is `en-US`.** What is Arabic is the *user
preferred language list* (`ar-IQ`), which is a different setting: with no
Arabic MUI language pack installed — and `UILanguages` holds `en-US` alone —
Windows itself renders in English, and `GetUserDefaultUILanguage()` returns
1033.

So P55's "80 captured frames, all English" **cannot be reproduced as a defect on
this machine**, because on an `en-US` UI an English installer is the correct
rendering. I did not capture 80 frames to prove a tautology. What I did instead
is settle the diagnosis the row actually asks for, which does not depend on the
machine at all — plus one runtime confirmation from this host.

I did not install an Arabic language pack. That is a multi-hundred-megabyte
system change requiring elevation and a sign-out, it alters the owner's daily
environment, and it is not needed to answer the question.

### MISSING, or PRESENT AND NEVER SELECTED? **MISSING.** Four independent proofs

**1. Nothing to select is authored.**

```
$ git ls-files | grep -i wxl
(no output)
```

Zero `.wxl` files are tracked anywhere in the repository.

**2. The build never asks for one.** `scripts/build-installer.ps1:225`:

```powershell
& dotnet tool run wix build installer\wix\Bundle.wxs -arch x64 `
    -ext WixToolset.Util.wixext -ext WixToolset.BootstrapperApplications.wixext `
    -o $bundle -d "ProductVersion=$Version" ...
```

No `-culture`. No `-loc`. `installer/wix/Product.wxs:10` hardcodes
`Language="1033"` for the MSI as well.

**3. The shipped artefact carries exactly one, and it is WiX's own.** Extracted
from the byte-verified P55 artefact — `AetherCoreSetup-0.1.11-x64.exe`,
1,121,407,585 B, sha256 `747bb6ee3265c4b141bfc58277c4fea71bc8155826b54503f3b7562556b6de3f`,
which is the value `P55-ITEM4C-LIFECYCLE.md:4-6` records from green CI run
`34683812129`:

```
$ dotnet tool run wix burn extract <exe> -o ... -oba ...
BootstrapperApplicationData.xml  BootstrapperExtensionData.xml  logo.png
manifest.xml  thm.wxl  thm.xml  wixstdba.exe
numeric (lcid) subdirectories? NONE
```

`thm.wxl` is `<WixLocalization Culture="en-us" Language="1033">`, copyright
".NET Foundation" — the stock file that ships inside `wixstdba`, not a project
file.

**4. That is exactly where a localized bundle would have put them.**
`LocProbeForFile` (`src/libs/dutil/WixToolset.DUtil/locutil.cpp`, tag `v6.0.2`,
the version the bundle reports: `Burn x64 v6.0.2`) probes, in order:

* `<base>\<language>\thm.wxl` if a language was passed;
* `<base>\<langid>\thm.wxl` for each entry from `GetThreadPreferredUILanguages`
  with `MUI_MERGE_USER_FALLBACK | MUI_MERGE_SYSTEM_FALLBACK`;
* `<base>\<langid>\thm.wxl` for `GetUserDefaultUILanguage()`;
* the same for its default-sublang form;
* the same for `GetSystemDefaultUILanguage()`;
* and only then falls back to `<base>\thm.wxl`.

A localized bundle ships `<lcid>\thm.wxl` payload subdirectories. **This one has
none**, so every probe misses and the fallback is the only file there is. The
selection logic is not broken; there is nothing for it to select.

**Runtime confirmation, from this host.** In the Item 2 run below, Burn logged:

```
i000: Setting numeric variable 'WixStdBALanguageId' to value 1033
```

`WixStdBALanguageId` is set to the language id of the `.wxl` that was actually
loaded (`WixStandardBootstrapperApplication.cpp:2989`). It loaded 1033.

### The cure, and the second fact that has to go with it

The cure for MISSING is to author the strings and ship them as `<lcid>\thm.wxl`
payloads, plus an RTL theme — `thm.xml` positions every control with absolute
coordinates and negative right-anchors and is not mirror-aware, so an Arabic
`.wxl` alone would produce Arabic text in a left-to-right layout. This is a real
localization deliverable, not a build-flag change, and it is the same enabling
step Item 2's second cure needs (see §2).

The second fact must be recorded with it, or the next session will ship the
strings and conclude they did not work: **on this machine a correctly localized
bundle would still render English**, because `GetUserDefaultUILanguage()` is
1033 here and no Arabic MUI pack is installed. Testing an Arabic installer needs
a machine whose UI language is Arabic. That is not this one, as measured.

**Row: OPEN.** Precondition corrected, cause proven, cure named.

---

## 2. `DBT-P55-009` — `/uninstall` presents the chooser with Repair focused

### Establishing that the test was safe, before running it

The row describes an unplanned repair that ran to completion. Repeating the
experiment blind on the owner's machine was not acceptable, and this session's
UAC configuration removes the obvious abort point:

```
EnableLUA = 1   ConsentPromptBehaviorAdmin = 0   PromptOnSecureDesktop = 0
```

`ConsentPromptBehaviorAdmin = 0` is *elevate without prompting*, so an elevation
request from this Medium-integrity session would be granted silently — there
would be no UAC dialog to decline.

So I settled it from the pinned source first.
`WixStandardBootstrapperApplication.cpp` at tag `v6.0.2`, `OnDetectComplete`:

```cpp
BOOL fSkipToPlan = SUCCEEDED(hrStatus) &&
                   (BOOTSTRAPPER_DISPLAY_FULL > m_commandDisplay ||
                    BOOTSTRAPPER_ACTION_LAYOUT == m_commandAction ||
                    BOOTSTRAPPER_RESUME_TYPE_REBOOT == m_commandResumeType);
```

With `/uninstall` and no `/quiet` or `/passive` the display **is**
`BOOTSTRAPPER_DISPLAY_FULL`, so the first clause is false; the action is
UNINSTALL, not LAYOUT; and this is not a reboot resume. `fSkipToPlan` is
`FALSE`, so nothing is planned and nothing is applied until a control is
clicked. The source's own comment says it plainly: *"If we're requiring user
input (which currently means Install, Repair, or Uninstall)"*.

That turned a destructive experiment into a safe one. Recovery was staged anyway:
the byte-verified artefact above is on disk at
`out/ci-artifact/.../artifacts/AetherCoreSetup-0.1.11-x64.exe`.

### ACTION / SNAPSHOT / EXPECTED / RECOVERY

**SNAPSHOT (before).** 2 ARP entries (`{0F9F349D-…}` MSI, `{660370F6-…}` bundle),
16 files in `C:\Program Files\AetherCore`, service `AetherCoreMaintenance`
Running, both Package Cache directories present, 4 `AetherCore_*` logs in
`%TEMP%`.

**ACTION.** The exact string Add/Remove Programs invokes, taken from the
bundle's own `UninstallString`, unelevated, full display:

```
"C:\ProgramData\Package Cache\{660370F6-37BC-4E8A-8C62-B1CB1F5D834D}\AetherCoreSetup-0.1.11-x64.exe" /uninstall /log <scratch>\item2-burn.log
```

**EXPECTED.** Loading page, then the Modify chooser, with Repair focused; no
plan, no apply, no elevation, no change. Process killed without clicking.

**RECOVERY.** Reinstall from the verified artefact. Not needed.

### What it produced

Focus was read out of the live UI with UI Automation
(`AutomationElement::FocusedElement`) rather than judged from a screenshot, at
t = 3, 6, 10 and 15 seconds. Every probe, identically:

```
FOCUSED ELEMENT: name='Repair' type=ControlType.Pane
```

(`ControlType.Pane` because the WiX theme engine owner-draws its buttons; a
`ControlType.Button` query returns 0 on this window.)

The Burn log for the same run:

```
i009: Command Line: '/uninstall /log C:\...\item2-burn.log'
i101: Detected package: AetherCoreMsi, state: Present, cached: Yes, install registration state: Present
i199: Detect complete, result: 0x0, registration state: Full, cached: Yes, eligible for cleanup: No
i052: Condition 'WixStdBAUpdateAvailable' evaluates to false.
```

and then **it ends**. No `Plan begin`. No `Apply begin`. The post-action
snapshot is identical to the pre-action snapshot in all five fields, and no new
elevated log appeared in `%TEMP%`.

### Does the row reproduce? **Yes, exactly**

Launched with `/uninstall`, the bundle presents the maintenance chooser and the
focused default is Repair. A user who asks Add/Remove Programs to uninstall
AetherCore and presses Enter runs a repair. That is a consent defect, and the
row is right to call it one rather than a cosmetic one.

### Which cause — and why the other is not

**It is not the bundle's command-line handling.** The argument arrived intact —
`i009: Command Line: '/uninstall …'` is Burn echoing what it parsed — and Burn
acted on it as UNINSTALL, reaching `Detect complete … registration state: Full`
for an uninstall. Had parsing been at fault the log would show a different
action or a dropped switch. It shows neither. Nothing downstream of `i009` is a
parsing question.

**The chooser appears because the BA maps UNINSTALL to it on purpose.**
`DeterminePageId`, at `BOOTSTRAPPER_DISPLAY_FULL` and `WIXSTDBA_STATE_DETECTED`:

```cpp
case BOOTSTRAPPER_ACTION_MODIFY:    __fallthrough;
case BOOTSTRAPPER_ACTION_REPAIR:    __fallthrough;
case BOOTSTRAPPER_ACTION_UNINSTALL:
    *pdwPageId = m_rgdwPageIds[WIXSTDBA_PAGE_MODIFY];
    break;
```

Uninstall falls through to the Modify page. This is upstream WiX behaviour, not
a defect this repository introduced.

**Repair is focused because of the theme's control order.** `thm.xml`, the
`Modify` page, declares four `TabStop="yes"` controls in this order:

| order | control | |
|---|---|---|
| 1 | `ModifyUpdateButton` | `EnableCondition="WixStdBAUpdateAvailable"`, `HideWhenDisabled="yes"` |
| 2 | `RepairButton` | `HideWhenDisabled="yes"` |
| 3 | `UninstallButton` | |
| 4 | `ModifyCancelButton` | |

The log shows `WixStdBAUpdateAvailable` false, so control 1 is hidden and
**`RepairButton` is the first visible tab stop**, which is what takes initial
focus. Measured as `name='Repair'`, four times.

So: the command line is handled correctly; the consent defect lives entirely in
the BA layer — the page choice in `wixstdba`, the focus in the theme.

### What I changed: nothing, and why

Two cures exist and both are product decisions I should not take alone at the
end of a pass.

**1. `SuppressRepair="yes"`** on `bal:WixStandardBootstrapperApplication`. One
attribute. `wixstdba` does
`ThemeControlEnable(m_pControlRepairButton, !m_fSuppressRepair)` and `thm.xml:67`
marks `RepairButton` `HideWhenDisabled="yes"`, so Repair disappears and
Uninstall becomes the first visible tab stop — the consent defect is gone.
**But** `/repair` takes the same `DeterminePageId` fallthrough, so this also
removes the only UI route to repair, and `scripts/static_validate.py`'s
`phase8_msi_repair_not_disabled` records the opposite intent in as many words:
*"MSI repair remains available so the deferred hardener can restore service/ACL
policy drift."* Applying it would contradict a documented product decision.

**2. A project-owned theme** whose Modify page declares `UninstallButton` before
`RepairButton`. Keeps repair, removes the hazard, and is **the same enabling
change Item 1 needs** — once the bundle owns its `thm.xml`/`thm.wxl` instead of
using `wixstdba`'s stock pair, `<lcid>\thm.wxl` payloads become possible. It
costs a forked theme in the repository and the maintenance that implies.

`QD-035-001` lists repair as unqualified. Changing which destructive operation
an installer offers by default, under a live qualification regime, is an owner's
call — and it is the same call P55 declined to make blind at the end of its own
pass, for the same reason.

**Row: OPEN.** Reproduced, cause proven and separated from the innocent one,
both cures costed. The row moves from "usability" to what it is: a consent
defect with a known mechanism.

---

## 3. `DBT-P49-003` — the elevated log left in `%TEMP%`

### Does the row reproduce? **Yes, and the count is four, not three**

```
AetherCore_20260912125833.elevated.log   920 B   13:01:13   install
AetherCore_20260912130229.elevated.log   929 B   13:08:23   repair
AetherCore_20260912130850.elevated.log   929 B   13:12:45   uninstall
AetherCore_20260912131651.elevated.log   920 B   13:17:28   restoration install
```

The first three are the survivors P55 named. **The fourth is P55's own
restoration install** — the separately-reported install at `3c96251` that left
the machine carrying the fixed build. It was recorded as an action and never
counted as a survivor, so the sweep's headline of three has been one short since
the day it was written.

The names confirm the correction already in the row: they are `AetherCore_*`,
not P49's `AetherCore_Setup_*`. A sweep on the old pattern still reports zero
and is still wrong.

### What it actually writes, where, and with what ACL

The brief asked for this before changing the sweep, and it is the part that
matters.

**Where, and by whom.** The elevated child runs from the protected system temp
and writes its log into the *unelevated user's* temp. From the log's own body:

```
i001: Burn x64 v6.0.2 ... path: C:\WINDOWS\TEMP\{118AFD98-...}\.be\AetherCoreSetup-0.1.11-x64.exe
i009: Command Line: '-q -burn.elevated ***** ***** *****'
i000: Setting string variable 'WixBundleLog' to value
      'C:\Users\husen\AppData\Local\Temp\AetherCore_20260912125833.elevated.log'
```

**The ACL**, identical on all four files:

```
Owner : BUILTIN\Administrators
SDDL  : O:BAG:S-1-5-21-...-1001D:(A;ID;FA;;;SY)(A;ID;FA;;;BA)(A;ID;FA;;;S-1-5-21-...-1001)
        [Allow] NT AUTHORITY\SYSTEM     FullControl  (inherited=True)
        [Allow] BUILTIN\Administrators  FullControl  (inherited=True)
        [Allow] Hussein\husen           FullControl  (inherited=True)
```

Every ACE carries `ID`. The DACL is **wholly inherited** from `%TEMP%` and the
file asserts no protected DACL of its own; `%TEMP%`'s own DACL is
`D:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;FA;;;<user>)`. So a file **owned by
`BUILTIN\Administrators`, written by an elevated process**, ends up **writable
by the unelevated user** — `FullControl` includes `WRITE_DAC`, `WRITE_OWNER` and
`DELETE`, not merely read.

**That is the defect, and it is the write path, not the contents.** The contents
are benign and Burn redacts the sensitive part itself: the elevated command line
renders as `-q -burn.elevated ***** ***** *****`, and the body is four variable
initialisations plus `Exit code: 0x0`. Nothing privileged is disclosed.

**Bounding the severity honestly.** `%TEMP%` is writable only by that user,
SYSTEM and Administrators, and on this machine the user who owns `%TEMP%` is the
same principal who consented to the elevation. So this is not a cross-user
escalation here. It is the classic elevated-write-into-a-user-controlled-path
pattern — a predictably named file an unelevated principal can pre-create,
redirect or rewrite — and it would matter on a machine where those principals
differ. Tidiness is the least of it, exactly as the row's framing implies.

**One new datum.** My Item 2 run reached `Detect complete` and stopped. It
created **no** elevated log, because the elevated child is only spawned at
Apply. The survivors are therefore one per *applied* operation, which is what
makes the count of four match the four applies this machine has seen.

### What I changed: nothing, and the escape route I did not disprove

Burn derives the log name from the bundle `Name` and writes it unconditionally;
the bundle cannot relocate the elevated child's log by changing `Name` without
re-breaking the pattern again. The untested escape route is Burn's `<Log>`
element on `<Bundle>` (`Disable`, `Prefix`, `Extension`). I did not test it, and
I am not recommending it blind: disabling installer logging trades directly
against this product's support-export story. It is named here so the next
session does not have to find it.

The row's status rule is explicit — `ACCEPTED` means escape routes **disproved**.
I have not disproved that one. **Row: OPEN**, with the ACL measured, the count
corrected to four, the content cleared, and the one remaining option named.

---

## 4. `DBT-P55-004` and `DBT-P55-003` — recovery media and disk

### `DBT-P55-004`: the media is ABSENT. Reproduced verbatim

The row's named evidence, re-run today — `scripts/gate4-preconditions.ps1`,
condition 3:

```
recovery media present  FAILS  no removable volume carries \sources\boot.wim or
                               \bootmgr (0 removable volume(s) attached)
```

Word for word what P55 recorded. Confirmed independently:

```
Get-Volume          -> C: (Fixed), D: "SD" (Fixed), one 0.9 GB unlettered (Fixed)
Win32_LogicalDisk   -> C: DriveType 3, D: DriveType 3     (2 = removable: none)
Get-PhysicalDisk    -> 0  NVMe Micron_2500_MTFDKBA1T0QGN  953.9 GB
                       1  HIKSEMI  USB  SSD               953.9 GB
```

Physical disk 1 is a USB-attached SSD, and it is `D:` — but it presents as a
**Fixed** volume, so it is neither a removable volume nor recovery media. There
are **zero removable volumes on this machine**.

The row's careful distinction holds and was worth making: the media is not
untested, it is **absent**, and it cannot be boot-tested until it exists. There
is nothing here to fix and no boot test to run. **That is the finding.**
**Row: OPEN**, now verified on the machine it names rather than from a distance.

### `DBT-P55-003`: re-measured on this machine, and it still holds

| | P55 | today (2026-09-21) | |
|---|---|---|---|
| `C:` used | 571.6 GB | **574.1 GB** | 616,404,086,784 B |
| `D:` free | 410.5 GB | **410.5 GB** | 440,769,040,384 B |

`D:` free is unchanged to the tenth of a gigabyte; `C:` used has grown 2.5 GB in
nine days. The deficit is **163.6 GB**. `D:` cannot hold a second full image of
`C:` beside the existing one, so writing one would still destroy the only
verified copy — which is exactly the owner decision recorded at P55-3.
**Row: OPEN**, numbers refreshed.

**What I could not measure, and am not claiming.** `D:\WindowsImageBackup`
exists and is dated 2026-09-02, but is ACL-denied to this unelevated session:

```
Get-Acl  D:\WindowsImageBackup -> UnauthorizedAccessException
Get-ChildItem D:\WindowsImageBackup -Force -> Access to the path ... is denied
```

My first reading of "0 files" was an **access artefact, not absence**. I am not
claiming the existing image's version count, age or integrity in either
direction. Re-run elevated to establish it.

### The change I did make: a gate that accused the machine of what it could not see

Running `gate4-preconditions.ps1` unelevated produced `0 of 6 conditions hold`,
and two of those six were false accusations:

```
recovery point  FAILS  Get-ComputerRestorePoint threw: Access denied
disk image      FAILS  WindowsImageBackup exists on D: but wbadmin lists zero versions
```

Both instruments require elevation. Unelevated, `Get-ComputerRestorePoint`
throws and `wbadmin` prints an access error that parses to zero versions — and
"zero versions" is indistinguishable, to the regex, from "no image", which is
not the same claim at all. The script reported them as the machine failing,
when what actually happened is that the instrument refused to look.

The file already states the correct rule, in its own condition 6:

> *"A thrown search that leaves `$found` null renders as '0 offered' if the
> caller is careless. Say UNKNOWN instead -- this is the instrument lying."*

Conditions 1 and 2 violated the rule their own file had written down. This is
the same species as the three false accusations P69 fixed in the phase 26/27
audits: the source — here, the machine — was innocent and the gate convicted it.

Fixed by measuring elevation once and reporting `UNKNOWN` where the instrument
cannot see. **No verdict is loosened**: the summary counts `State -eq 'HOLDS'`
and the blocking list is `State -ne 'HOLDS'`, so `UNKNOWN` still blocks Gate 4
and the exit code is unchanged at 1. After:

```
recovery point  UNKNOWN  Get-ComputerRestorePoint threw: Access denied (session is
                         not elevated; this cmdlet requires it) -- not measured, not claimed
disk image      UNKNOWN  WindowsImageBackup exists on D: but wbadmin needs elevation;
                         version count is UNKNOWN, not zero -- re-run elevated
recovery media present  FAILS  ... (0 removable volume(s) attached)
```

The genuine failures, including `DBT-P55-004`'s, are untouched — `Get-Volume`
needs no elevation, which is why condition 3 was trustworthy from this session
and conditions 1 and 2 were not.

---

## 5. Gates run for this commit

The only tracked source this commit touches is
`scripts/gate4-preconditions.ps1` and the ledger and docs.

| gate | result |
|---|---|
| `python scripts/source_seal.py` | **OK - 1491 of 1491 tracked files verified** |
| `python scripts/static_validate.py` | **ok: true, checks 347, failed: []** |
| `python scripts/ps_marker_scan.py` | **81 scripts, total assertions=234, failed=0**, 3 unmeasured (pre-existing) |
| `python scripts/test_windows_server_support.py` | **PASS** |
| `scripts/gate4-preconditions.ps1` | runs; exit 1 unchanged; two conditions now UNKNOWN |

Scripts were named explicitly and never globbed.

**What I did not run, and why.** The Rust suite, the UI gates and
`verify-enterprise.ps1` were not run locally. Nothing in this commit touches
Rust, the UI, the installer sources or the dependency graph — the one code
change is to a standalone precondition helper that no build step consumes — and
`verify-enterprise.ps1` re-runs the whole Rust suite five times (P69 §1). CI
covers them on push. This host *can* run them, which is new and is recorded in
§0.3; it was not a good use of this pass.

---

## 6. Row dispositions

| row | before | after | why |
|---|---|---|---|
| `DBT-P55-008` | OPEN, BLOCKED-MACHINE (P60) | **OPEN** | Precondition **does not hold here** — UI language is `en-US`, not Arabic. Defect diagnosed anyway: strings are **MISSING**, proven four ways |
| `DBT-P55-009` | OPEN, BLOCKED-MACHINE (P60) | **OPEN** | **Reproduced.** Not command-line handling; `wixstdba` page policy + theme control order. Two cures, both owner decisions |
| `DBT-P49-003` | OPEN, BLOCKED-MACHINE (P60) | **OPEN** | **Reproduced, four survivors not three.** ACL measured; content cleared; `<Log>` escape route named but not disproved |
| `DBT-P55-004` | OPEN | **OPEN** | **Reproduced verbatim.** Zero removable volumes. Media absent, so unfixable and unboot-testable here |
| `DBT-P55-003` | OPEN | **OPEN** | **Re-measured.** C: 574.1 GB used vs D: 410.5 GB free; deficit 163.6 GB. Claim holds |

No row closed. Four reproduced and are now backed by measurements taken on the
machine they name instead of inference from a machine that could not reach it;
the fifth is corrected. The `BLOCKED-MACHINE` qualifier is retired from all
three rows that carried it — the block was real and is now spent.

## 7. Open after P71

1. **`DBT-P55-008` and `DBT-P55-009` share one enabling change.** Both are
   solved by the bundle owning its `thm.xml`/`thm.wxl` instead of using
   `wixstdba`'s stock pair: it is the prerequisite for `<lcid>\thm.wxl`
   payloads *and* the way to reorder the Modify page without suppressing
   repair. Whoever does one should do both.
2. **An Arabic installer cannot be tested on this machine** until an Arabic MUI
   pack is installed and the UI language is switched. Shipping the strings and
   testing here would show English and look like a failure.
3. **`D:\WindowsImageBackup` is unread.** Its version count, age and integrity
   are unknown, not absent. One elevated `wbadmin get versions -backupTarget:D:`
   settles it and decides how much `DBT-P55-003` actually matters.
4. **Any Windows clone older than the root `.gitattributes` carries the §0.1
   defect silently.** The seal catches it; `git status` does not. Verify the
   seal before regenerating the manifest, never after.
5. **Node drift**: this host has v22.23.2, CI installs 22.16.0. Harmless for
   this commit, which touches no UI, but it is a difference between what this
   machine verifies and what CI verifies.
