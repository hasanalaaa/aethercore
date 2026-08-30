# Phase 36 — SESSION CONTEXT (read this file first)

Purpose: this is the project brain for Phase 36 VM qualification. Any new
session reads this file before touching anything else. The progress table at
the bottom is authoritative: resume from it, do not re-derive the world.

Last updated: 2026-08-31

## 1. GIT STATE

- Repo: `/Users/hasanalaaa/Documents/AetherCore 2` (private, `hasanalaaa/aethercore`)
- Branch: `main`
- Session start commit: `218e0d8` (pushed). Contains BOTH Windows IPC fixes:
  the overlapped-IO transport conversion and the single-connect-per-verb
  ServiceJob fix.
- Workspace root: `phase21-workspace/`
- VM source tree is byte-identical to `main`.

## 2. VM AND SNAPSHOTS

VM: Parallels `Windows 11`, UUID `{291d6c17-c344-4498-8be3-3436aab3dcdb}`,
Windows 11 Pro ARM64, build 26200.

| Snapshot | ID | Status |
|---|---|---|
| P36-CLEAN-BASELINE | `{6b721a10-8ac1-4339-9699-bd5145efda0a}` | **FORBIDDEN — never restore** |
| P36-PRE-NATIVE-MUTATION | `{e9434b5f-ba68-452d-ac4c-cbcc863ffb4b}` | **FORBIDDEN — never restore** |
| P36-TRANCHE2-BASELINE | `{357848ae-3a9b-4753-8312-dd944e6b432a}` | **RECOVERY POINT** (current) |

Both FORBIDDEN snapshots predate the install. Restoring either destroys the
working installed product and every tranche-1/2 result.

P36-TRANCHE2-BASELINE is restore-tested and proven: service RUNNING, seven
binaries byte-identical, pipe DACL identical, all verbs returning.

Restore command (recovery only):
```
prlctl snapshot-switch "Windows 11" --id {357848ae-3a9b-4753-8312-dd944e6b432a}
```

## 3. THE DEFECT THIS SESSION EXISTS TO CLOSE

The installed binaries were hand-deployed. The registered MSI
`C:\AetherCore-P36\incoming\AetherCore-0.1.0-arm64.msi`
(6,238,208 B, 2026-08-29) still carries **PRE-FIX** binaries. Any `msiexec /f`
repair or reinstall would silently revert both IPC fixes.

- ProductCode `{FC8A3841-759D-B452-1864-161F84F56C03}`
- UpgradeCode `{45598C77-2C32-5BCE-8510-19C7E51EE3B8}`
- Version 0.1.0

**Byte-equality with the currently installed binaries is NOT a criterion
anywhere in Phase 36.** They are output of an unrecorded ad-hoc invocation and
two earlier sessions were lost chasing them. Correctness is proven
FUNCTIONALLY: the verbs return.

Two legitimate reasons a rebuild differs:
1. cargo unifies features across all packages selected in one invocation, so
   the package set of the invocation changes the emitted code.
2. `aethercore-desktop` is a Tauri app (tauri 2.11.5) embedding the built
   Svelte frontend; its size depends on the `apps/ui` dist state.

## 4. VM OPERATING KNOWLEDGE

- Always use `prlctl exec "Windows 11" cmd.exe /c "..."` or
  `powershell.exe -NoProfile`. The bare form without `cmd.exe` returns EMPTY
  output.
- argv is limited (~16 KB) and nested quoting zsh -> PowerShell is fragile.
  For any real script: base64 it on the Mac, pass ONE argument, decode with
  `[IO.File]::WriteAllBytes(p,[Convert]::FromBase64String('...'))`.
- ARM64 builds: **MSVC is NOT supported**. Known-good recipe is
  `C:\AetherCore-P36\logs\p36_relbuild.cmd` — VsDevCmd arm64 + clang-cl +
  Ninja + libomp. Read and extend it; do not invent an environment. Contents:
  ```
  call "C:\AetherCore-P36\toolchain\vs2022\Common7\Tools\VsDevCmd.bat" -arch=arm64 -host_arch=arm64
  set PATH=C:\AetherCore-P36\toolchain\cmake-4.4.2\bin;C:\AetherCore-P36\toolchain\ninja-1.13.2-arm64;C:\AetherCore-P36\toolchain\llvm-22.1.8\bin;%PATH%
  set LIBCLANG_PATH=C:\AetherCore-P36\toolchain\llvm-22.1.8\bin
  set CC_aarch64_pc_windows_msvc=clang-cl
  set CXX_aarch64_pc_windows_msvc=clang-cl
  set CMAKE_GENERATOR=Ninja
  set CXXFLAGS=/EHsc
  set CXXFLAGS_aarch64_pc_windows_msvc=/EHsc
  set RUSTFLAGS=-C link-arg=libomp.lib
  set LIB=C:\Program Files (x86)\Windows Kits\10\Assessment and Deployment Kit\Deployment Tools\SDKs\DismApi\Lib\arm64;%LIB%
  ```
  Source tree on VM:
  `C:\AetherCore-P36\workspace\AetherCore-Phase35-Master-Delivery`
- Scheduled tasks sit permanently **Queued** unless registered with
  `-AllowStartIfOnBatteries -DontStopIfGoingOnBatteries`.
- Actual-token contexts use the one-shot Scheduled Task method: register as
  the target user with a runtime-generated random password that is never
  printed or persisted. RunLevel `Limited` for `P36StandardUser`, `Highest`
  for `P36Admin`. Drivers exist under `C:\AetherCore-P36\evidence\`.
- Reading the pipe DACL (FileStream and Get-Acl both fail; this works):
  ```powershell
  $fs=[System.IO.File]::Open('\\.\pipe\AetherCore.Maintenance.v7',
      [IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::ReadWrite)
  $fs.GetAccessControl().Sddl; $fs.Close()
  ```
  Expected:
  `O:<service SID> G:SY D:P(A;;FA;;;<service SID>)(A;;FR;;;AU)(A;;DC;;;AU)`
  Some APIs render the AU pair as `(A;;0x12008b;;;AU)`. **SAME DACL**:
  `FR|DC = 0x120089|0x2 = 0x12008b`. Never report that as drift.
- `C:\AetherCore-P36\backup-p36-overlapped` holds **PRE-FIX** binaries. NOT a
  recovery source for the working install.
- Copy every piece of evidence to the Mac BEFORE any step that could need a
  restore. Snapshot restore is all-or-nothing; there is no incremental point.
- **FILE TRANSFER IS SOLVED — do not use base64 chunking.** Parallels shares the
  Mac's Desktop/Documents/Downloads. `prlctl exec` runs as SYSTEM, so the mapped
  drive letters `Y:`/`Z:` are NOT visible, but the UNC path IS:
  `\\Mac\Home\Documents\...`. Staging dir on the Mac:
  `~/Documents/p36-stage/` -> `\\Mac\Home\Documents\p36-stage\`.
  Read AND write both work from the VM. `prlctl exec` round trip is ~0.3 s.
- **Never inline PowerShell inside a cmd string through zsh.** Quoting breaks in
  three layers. Write a `.ps1` (or `.cmd`) into `~/Documents/p36-stage/` and
  invoke it by UNC path. Helper: `~/Documents/p36-stage/vmr <name>` runs
  `<name>.ps1` in the VM.
- Long jobs: launch with `start "" /b cmd /c \\Mac\Home\...\job.cmd`. The
  `prlctl exec` call will still block until its 2 min tool timeout and report a
  timeout — that is expected and harmless; the job keeps running. Poll a
  status file.

## 5. RULES

- Every gate proves the PROPERTY directly. Never accept a proxy measurement.
- STOP RULE: observation differs from expected -> record raw observation +
  expected, stop that gate, move to the next INDEPENDENT gate. Do not
  diagnose, do not theorise, do not fix outside authorization.
- Before any destructive action write ACTION / SNAPSHOT / EXPECTED / RECOVERY
  into this file and commit. If you cannot fill all four, do not run it.
- FORBIDDEN: restoring either forbidden snapshot; ACL mutation outside what an
  MSI action itself performs; Service SID mutation; editing the pipe security
  descriptor; suppressing any ICE finding; deleting any backup or snapshot;
  disabling Defender / UAC / Firewall / SmartScreen; Phase 37; attempting the
  human-gated items.
- Checkpoint: update the progress table below, commit, and push after EVERY
  gate — pass or blocked — never only at end of stage.

## 6. HUMAN-GATED (no agent can do these)

- Desktop launch under a real standard user, and UAC — needs a human at the
  Parallels console. Note `PromptOnSecureDesktop=0` on this VM is a non-default
  deviation, so any UAC finding here carries that caveat.
- Physical x86_64 qualification.
- Authenticode certificate, production update endpoint, production key/HSM,
  dependency-freeze from a trusted workstation.

## 7. PROGRESS TABLE

Legend: PASS / FAIL / BLOCKED / IN-PROGRESS / NOT-STARTED

| Gate | What it proves | Status | Evidence |
|---|---|---|---|
| Stage 0 | Session brain committed | PASS | this file, committed |
| A1 | Recorded canonical build script | PASS | `phase21-workspace/scripts/build-arm64-msi.cmd`, committed |
| A2 | Rebuild artifacts hashed + sized | PASS | `STAGE_A_EVIDENCE.md` A2 table; `evidence/A2-build.log`; old tree preserved at `target/release-preA2-hold` |
| A3 | wix build + validate, zero ICE | PASS | exit 0 both; 0 matches for `ICE\d+` in the log; MSI sha `3f370294…6c94fd34`, 6,205,440 B, copied to Mac |
| A4 | Reinstall-vs-upgrade decision justified | PASS | same ProductCode + same version -> `REINSTALL=ALL REINSTALLMODE=amus`; see `STAGE_A_EVIDENCE.md` A4 |
| A5 | Install from realigned MSI verified | PASS | `amus` refused 1638 (PackagecodeChanging); `vamus` exit 0; all five payload exes replaced with the A2 rebuild; every security property SAME |
| GATE A | Package == installed files, security intact, 8 verb runs | **PASS** | `STAGE_A_EVIDENCE.md`; snapshot P36-MSI-ALIGNED `{7d0696ae-ebc7-4c67-b076-438855d175f3}` |
| B1 | Repair preserves everything | NOT-STARTED | |
| B2 | Uninstall: what goes, what survives | NOT-STARTED | |
| B3 | Clean reinstall on empty box | NOT-STARTED | |
| B4 | Upgrade v1 -> v2 | NOT-STARTED | |
| C1 | Install interrupted mid-transaction | NOT-STARTED | |
| C2 | Service fails to start during install | NOT-STARTED | |
| C3 | Required file missing/corrupt at start | NOT-STARTED | |
| C4 | Rollback via failing custom action | NOT-STARTED | |
| D1 | `aetherctl update stage` real path | BLOCKED-BY-DESIGN | `apps/aetherctl/src/offline.rs:138` — unconditional match arm; see Stage D |
| D2 | stage -> apply -> rollback | NOT-STARTED | |
| D3 | update trust disabled by default, no network | NOT-STARTED | |
| E | Seal, snapshot P36-VM-QUALIFIED | NOT-STARTED | |

## 8. PACE LOG

(one line per ~10 shell commands, naming the current gate)

- cmd ~6 — Stage 0, writing session brain.
- cmd ~20 — Gate A1, authoring canonical ARM64 build recipe.
- cmd ~30 — Gate A2, full rebuild launched in VM background.
- cmd ~50 — Gates A2/A3 PASS, A4 decided. Next: A5 install.
- cmd ~72 — GATE A PASS. Snapshot P36-MSI-ALIGNED taken. Next: Stage B1 repair.

## 9. FACTS ESTABLISHED THIS SESSION (do not re-derive)

### ProductCode is a pure function of version + arch
`scripts/build-installer.ps1` derives it as the first 16 bytes of
`SHA256("AetherCore/MSI/ProductCode/v1" + "AetherCore/<version>/<arch>")`
read as a .NET `Guid`. Verified:

| version | arch | ProductCode |
|---|---|---|
| 0.1.0 | arm64 | `{FC8A3841-759D-B452-1864-161F84F56C03}` <- **the installed one** |
| 0.1.0 | x64   | `{2D5CF2F8-EEFA-65EE-3780-E5A924AB1FB1}` |
| 0.1.1 | arm64 | `{84140FFD-5CBC-175D-928D-E493493F5F51}` |
| 0.2.0 | arm64 | `{4D5C3F4C-9607-B91A-3F9D-687552404614}` |

Consequence (this IS the Gate A4 answer): a rebuild of 0.1.0 for arm64 has the
IDENTICAL ProductCode and the IDENTICAL version, so Windows Installer cannot
major-upgrade it. Realignment must be a same-version reinstall:
`REINSTALL=ALL REINSTALLMODE=amus`. `MajorUpgrade` in Product.wxs does not set
`AllowSameVersionUpgrades`, confirming same-version is not an upgrade path.
Stage B4 bumps to 0.1.1, which yields a new ProductCode under the same
UpgradeCode `{45598C77-2C32-5BCE-8510-19C7E51EE3B8}` and so is a true
major upgrade with `Schedule="afterInstallInitialize"`.

### The MSI payload is SEVEN files, and aetherctl.exe is NOT one of them
`installer/wix/Product.wxs` authors exactly:
`aethercore-desktop.exe`, `aethercore-consent-broker.exe`,
`aethercore-update-broker.exe`, `aethercore-install-hardener.exe`,
`aethercore-maintenance-service.exe`, `libomp140.aarch64.dll`,
`update-trust.json`.

`C:\Program Files\AetherCore` currently holds EIGHT files — those seven plus
`aetherctl.exe` (3,611,136 B). **Observation recorded, not diagnosed:**
`aetherctl.exe` is an unmanaged file hand-placed in INSTALLFOLDER. It is not in
any MSI component, so no MSI action installs, repairs, or removes it. This
matters for Stage A5 ("registered package and installed files agree"), for
Stage B2 (uninstall will leave INSTALLFOLDER non-empty), and for B3 (a clean
MSI-only install will NOT contain aetherctl, so verb exercising must invoke it
from a path outside INSTALLFOLDER).

### Toolchain / path facts
- WiX: `dotnet tool run wix` -> `6.0.2+b3f3403` (pinned, restored).
- libomp source on this VM (the ONLY copy under vs2022):
  `C:\AetherCore-P36\toolchain\vs2022\VC\Redist\MSVC\14.44.35112\debug_nonredist\arm64\Microsoft.VC143.OpenMP.LLVM\libomp140.aarch64.dll`
- `release/update-trust.template.json` is 83 bytes,
  `{"schema":"aethercore.update-trust.v1","enabled":false,"channels":[]}`, and is
  byte-identical to the installed `update-trust.json`. Update trust ships
  DISABLED with ZERO channels — most of Gate D3 is already established.
- `scripts/build-release.ps1` is x64-only and requires the signed dependency
  freeze, the online supply-chain audit and an Authenticode signer. It cannot
  run on this VM. `scripts/build-arm64-msi.cmd` is the ARM64 payload+package
  equivalent.

## 10. GATE-A EXPECTED VALUES (reference baseline, captured pre-realignment)

Source: `evidence/verify-A0-baseline.json`, captured 2026-08-31 against the
working hand-deployed install. Every later verification compares to THIS.

- `sc qc`: `TYPE 10 WIN32_OWN_PROCESS`, `START_TYPE 2 AUTO_START (DELAYED)`,
  `ERROR_CONTROL 1 NORMAL`,
  `BINARY_PATH_NAME "C:\Program Files\AetherCore\aethercore-maintenance-service.exe"`,
  `SERVICE_START_NAME LocalSystem`. `sc query`: `STATE 4 RUNNING`.
- `sc qsidtype`: `SERVICE_SID_TYPE: UNRESTRICTED`.
- Pipe SDDL (read with `NamedPipeClientStream`, see below):
  `O:S-1-5-80-4285065559-3530017622-2858480679-3751456793-1187574229G:SYD:P(A;;0x12008b;;;AU)(A;;FA;;;S-1-5-80-4285065559-3530017622-2858480679-3751456793-1187574229)`
- `icacls "C:\Program Files\AetherCore"`:
  ```
  NT SERVICE\AetherCoreMaintenance:(OI)(CI)(RX)
  BUILTIN\Users:(OI)(CI)(RX)
  BUILTIN\Administrators:(OI)(CI)(F)
  NT AUTHORITY\SYSTEM:(OI)(CI)(F)
  ```
  Users has RX and no write. This is the "unchanged" reference.
- ARP: one entry, key `{FC8A3841-759D-B452-1864-161F84F56C03}`, name `AetherCore`,
  version `0.1.0`. `HKLM\SOFTWARE\AetherCore\InstallVersion = 0.1.0`.
- `C:\ProgramData\AetherCore` exists with 5 files; `state` 4, `logs` 1,
  `support-staging` 0.
- `libomp140.aarch64.dll` present; no `ipc_probe*` anywhere under INSTALLFOLDER.

### CORRECTION to the pipe-DACL recipe in section 4
The `[System.IO.File]::Open` form in the original brief does NOT work on this
box — it goes through `FileStream`, which refuses a non-file device:
`"FileStream was asked to open a device that was not a file."`
The method that DOES work, and that produced the tranche-1/2 evidence, is:
```powershell
$pc = New-Object System.IO.Pipes.NamedPipeClientStream('.','AetherCore.Maintenance.v7',[IO.Pipes.PipeDirection]::In)
$pc.Connect(5000)
$pc.GetAccessControl().GetSecurityDescriptorSddlForm('All')
$pc.Dispose()
```
Implemented in `scripts/p36vm/verify-install.ps1`.

## 11. QUALIFICATION TOOLING (committed, reusable)

- `scripts/build-arm64-msi.cmd` — the canonical ARM64 build recipe.
- `scripts/p36vm/verify-install.ps1 -Label <x>` — the entire Gate-A
  verification list in one read-only pass; writes
  `\\Mac\Home\Documents\p36-stage\out\verify-<x>.json`.
- `scripts/p36vm/verbs-outer.ps1 -Label <x>` — runs the four typed verbs
  (`service detect`, `doctor`, `optimize status`, `scan status`) under BOTH
  actual-token contexts via one-shot Scheduled Tasks; needs
  `C:\AetherCore-P36\tools\aetherctl.exe` and
  `C:\AetherCore-P36\tools\verbs-inner.ps1` staged.
- Mac-side helper `~/Documents/p36-stage/vmr <name>` runs `<name>.ps1` in the VM.

## 12. DESTRUCTIVE ACTION LOG

Every entry is written and committed BEFORE the action runs.

### A5 — install the realigned MSI over the existing install
```
ACTION=    msiexec /i C:\AetherCore-P36\build\out\AetherCore-0.1.0-arm64.msi
           REINSTALL=ALL REINSTALLMODE=amus /qn /l*v C:\AetherCore-P36\logs\a5-install.log
SNAPSHOT=  P36-MSI-BUILT {a2f665b8-0df6-46eb-9842-b7efca505671}
EXPECTED=  msiexec exit 0. Then the full Gate-A list matches section 10:
           service LocalSystem AUTO_START(DELAYED) RUNNING; Service SID
           UNRESTRICTED; pipe SDDL identical; install-dir icacls identical
           (Users RX, no write); libomp140.aarch64.dll present; no ipc_probe;
           ARP still {FC8A3841-...} 0.1.0; and the five payload .exe hashes in
           INSTALLFOLDER now equal the A2 REBUILT hashes, not the old ones.
           Then 8/8 verb runs RETURNED across both actual-token contexts.
           doctor returning diagnostics.stateUnavailable is a PASS.
RECOVERY=  prlctl snapshot-switch "Windows 11" --id {a2f665b8-0df6-46eb-9842-b7efca505671}
```

### Snapshot ledger (append-only)
| name | id | taken before |
|---|---|---|
| P36-TRANCHE2-BASELINE | `{357848ae-3a9b-4753-8312-dd944e6b432a}` | this session |
| P36-MSI-BUILT | `{a2f665b8-0df6-46eb-9842-b7efca505671}` | A5 install |
| P36-MSI-ALIGNED | `{7d0696ae-ebc7-4c67-b076-438855d175f3}` | Stage B — **the lifecycle recovery point** |

### B1 — repair
```
ACTION=    msiexec /f {FC8A3841-759D-B452-1864-161F84F56C03} /qn /l*v
           C:\AetherCore-P36\logs\b1-repair.log
SNAPSHOT=  P36-MSI-ALIGNED {7d0696ae-ebc7-4c67-b076-438855d175f3}
EXPECTED=  msiexec exit 0, then the FULL Gate-A list identical to
           evidence/verify-A5-postinstall.json: service RUNNING LocalSystem
           AUTO_START(DELAYED), SID UNRESTRICTED, pipe SDDL identical,
           install-dir icacls identical, seven payload files with the A2
           rebuild hashes, 8/8 verbs RETURNED.
RECOVERY=  prlctl snapshot-switch "Windows 11" --id {7d0696ae-ebc7-4c67-b076-438855d175f3}
```
