# Item 4.C / 4.D — the bundle installed, photographed, and uninstalled

Machine: the physical Windows PC, elevated. Date: 2026-09-12.
Artefact: `AetherCoreSetup-0.1.11-x64.exe`, 1,121,407,585 B, sha256
`747bb6ee3265c4b141bfc58277c4fea71bc8155826b54503f3b7562556b6de3f`, from green
CI run `34683812129`. Pre-action record: `P55-DESTRUCTIVE-ACTION-RECORD.md`.

## Restore point — verified by enumeration

```
restore points before: 5
Checkpoint-Computer returned without error   <- NOT the evidence
restore points after:  6
VERIFIED BY ENUMERATION: True
  SequenceNumber : 8
  Description    : AetherCore P55 Item 4.C - before uninstall 20260912-125620
```

## Starting state — which the brief did not anticipate

AetherCore **0.1.11 was already installed**: P49's build, ARP key
`{0F9F349D-01C8-B3C2-7242-83B5D29047C9}`, `UninstallString` =
`MsiExec.exe /I{0F9F349D…}` — an **MSI-direct** registration, with no bundle
entry. Service `AetherCoreMaintenance` Running as LocalSystem, 16 files in
`C:\Program Files\AetherCore`.

That build predates `ba9973b`, so this machine was running a version where
ephemeral insight state crosses authenticated principals.

Removing it was **baseline preparation, not the measured cycle**:
`msiexec /x … /qn /norestart` → exit 0, then 0 ARP entries, no install
directory, no service.

## The measured cycle

| step | result |
|---|---|
| install via bundle | 16 files, service Running as LocalSystem, **no reboot requested, no reboot pending** |
| ARP after install | **2** entries, but only **1 visible**: the MSI carries `SystemComponent=1` (`Visible="no"` works), the bundle is the single entry a user sees, named **`AetherCore`**, uninstalling through the bundle |
| uninstall via bundle | "Uninstall Successfully Completed", Burn `Exit code: 0x0` |

## Prompts, dialogs and reboot requests

* **No UAC prompt was observed** — and this is a caveat, not a result. The
  bundle was launched from an already-elevated session, which inherits
  elevation. A normal user double-clicking it **would** see a UAC prompt. This
  run cannot measure that, and does not claim to.
* No reboot was requested at any point. `RebootPending` = False after install.
* Dialogs shown: welcome → progress → complete; then Modify Setup → progress →
  complete. All photographed below.

## Item 4.D — the screens

`docs/phase55/installer-screens/`. The owner has never seen this installer.

| file | screen |
|---|---|
| `01-welcome.png` | welcome |
| `02-install-progress.png` | installing |
| `03-install-complete.png` | "Installation Successfully Completed" |
| `04-modify-setup.png` | Modify Setup — Repair / Uninstall / Cancel |
| `05-uninstall-progress.png` | uninstalling |
| `06-uninstall-complete.png` | "Uninstall Successfully Completed" |
| `07-repair-complete-unplanned.png` | "Repair Successfully Completed" — see below |

80 frames were captured in total (75 install, 5 uninstall) and kept under
`out/`, which is not committed. The seven above are one per distinct screen.

**Only one language.** The installer showed English throughout, on a machine
whose Windows UI language is Arabic — the shell's own windows on this box render
Arabic (`من دون عنوان - المفكرة`). The WiX standard bootstrapper ships localised
theme strings, so this is worth a look, but "does it support Arabic" was not
measured here and is not claimed either way. Recorded as `DBT-P55-008`.

### What the screens show about `DBT-P49-002`

| finding | verdict from the pixels |
|---|---|
| "AetherCore Setup Setup" | **FIXED.** Title bar reads **"AetherCore Setup"** |
| stock WiX logo | **FIXED.** The Æ mark renders at 64×64, top-left |
| progress stuck on "Initializing..." | **FIXED.** Reads **"Processing:  AetherCore"** with the bar advancing, on both the install and uninstall paths |
| no version | **NOT FIXED.** See below |
| no licence | unchanged, deliberately — the repository has no product licence text |

### The version: not fixed, and the precise reason

`ShowVersion="yes"` is authored, and it **does** reach the bootstrapper —
`BootstrapperApplicationData.xml` carries `WixStdbaOptions/@ShowVersion = 1`,
and `Registration/@Version` is `0.1.11`. The flag is set and the value is
present.

The reason nothing renders is in the theme: the shipped `thm.xml`, extracted
from the bundle itself, **contains no version control at all** — the only
occurrence of the string "Version" in that file is the XML declaration on
line 1. `ShowVersion` makes a version control visible; the `hyperlinkLicense`
theme in WiX 6.0.2 has none to make visible.

**Fixing it therefore requires shipping a custom theme** (`ThemeFile=`), a
~200-line theme XML the project would then own and maintain, plus a rebuild to
verify. That is a real change with real maintenance cost and it was not made
blind at the end of this pass. The row stays open, but it is now open **with a
cause and a named fix** instead of "shows no version".

## The fourteen-check survivor sweep

**13 of 14 PASS. Check 14 FAILS with three survivors.**

```
 1  INSTALLDIR               False                      PASS
 2  PROGRAMDATA              False                      PASS
 3  SERVICE                  OpenService FAILED 1060    PASS
 4  PIPE_COUNT               0                          PASS
 5  ARP_COUNT                0                          PASS
 6  HKLM_SOFTWARE_AETHER     False                      PASS
 7  HKCU_SOFTWARE_AETHER     False                      PASS
 8  STARTMENU                False                      PASS
 9  SCHEDULED_TASKS          0                          PASS
10  FIREWALL_RULES           0                          PASS
11  HKLM_SERVICES_KEY        False                      PASS
12  EVENTLOG_SOURCE          False                      PASS
13  ALLUSERS_HKCU_MARKERS    0                          PASS
14  FILESYSTEM SWEEP         3 survivors                FAIL
```

### The three survivors, with exact paths — recorded, not deleted

```
C:\Users\husen\AppData\Local\Temp\AetherCore_20260912125833.elevated.log   920 B  install
C:\Users\husen\AppData\Local\Temp\AetherCore_20260912130229.elevated.log   929 B  repair
C:\Users\husen\AppData\Local\Temp\AetherCore_20260912130850.elevated.log   929 B  uninstall
```

This is **`DBT-P49-003`**, reproduced exactly. Per-user, so `MACHINE_WIDE=0`
still holds; but they are product-attributable files the MSI path does not
create.

**The row's filename pattern is now stale and must be updated.** P49 recorded
`AetherCore_Setup_*.elevated.log`. Today's are `AetherCore_*.elevated.log` —
because the bundle `Name` changed from "AetherCore Setup" to "AetherCore", and
Burn derives the log filename from the bundle name. The P49-era files are still
on this machine and still carry the old prefix, which is what makes the change
visible:

```
AetherCore_Setup_20260906011711.elevated.log   926 B   2026-09-06   P49
AetherCore_Setup_20260906012151.elevated.log   935 B   2026-09-06   P49
```

A sweep written against the old pattern would now report zero and be wrong.

### What the raw sweep also hit, and why none of it is a survivor

The filesystem sweep returned 31 name matches. 28 are not survivors of this
cycle, and saying so precisely matters more than the headline count:

* **Pre-existing product data, deliberately kept.**
  `%LOCALAPPDATA%\com.aethercore.desktop` — WebView2 user data, created
  2026-09-06, last written 2026-09-08, i.e. **before** this cycle. Uninstall
  keeps user data by design; `release/UNINSTALL.txt` says so.
* **P49's two `AetherCore_Setup_*` logs**, dated 2026-09-06.
* **Test and development artefacts**, the `DBT-P42-013` family:
  `aethercore-phase4-cleaner-*` ×6, `aethercore-phase4-repair-*` ×4,
  `aethercore-diag-*.db` ×2, `aethercore-ssh-stub-*`,
  `aethercore_elev_probe.txt`. Written by `cargo test`, not by the product.
* **Not the product at all**: four `C--dev-aethercore` cache directories
  belonging to this tooling and named after the repository path, four
  `Recent\*.lnk` shell shortcuts created by opening files, and the user's own
  `OneDrive\…\aethercore-models` folder.

This is the empty-state trap's opposite number: a sweep that greps a name and
reports 31 would be as useless as one that reports 0 because it matched the
wrong pattern.

## An unplanned repair — recorded, not hidden

The bundle was launched with `/uninstall`, and it **still presented the "Modify
Setup" chooser** (Repair / Uninstall / Cancel) rather than proceeding directly
to uninstall. **Repair is the focused default button.** Enter was sent, which
took Repair, and a full repair ran to completion — screen
`07-repair-complete-unplanned.png`.

That was an operator error, and it is recorded because it happened to this
machine: a repair executed that nobody planned. It is **not** offered as
qualification of the repair path. `QD-035-001` lists upgrade and repair as
unqualified and they stay unqualified — a result nobody planned to interpret is
not evidence. If repair is worth qualifying it gets its own item, its own
restore point and its own EXPECTED values.

The uninstall was then re-run and driven with **Alt+U**, the Uninstall button's
own accelerator, which is the correct way to select it.

Worth noting as a usability finding in its own right: on a maintenance
invocation the destructive-sounding chooser defaults to Repair, and `/uninstall`
on the command line did not bypass it. Recorded as `DBT-P55-009`.
