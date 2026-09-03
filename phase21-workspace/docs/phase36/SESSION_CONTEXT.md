# Phase 36 — SESSION CONTEXT (read this file first)

Purpose: this is the project brain for Phase 36 VM qualification. Any new
session reads this file before touching anything else. The progress table at
the bottom is authoritative: resume from it, do not re-derive the world.

Last updated: 2026-08-31

## 1. GIT STATE

- Repo: `/Users/hasanalaaa/dev/aethercore` (private, `hasanalaaa/aethercore`)
  **RELOCATED 2026-08-31** from `/Users/hasanalaaa/Documents/AetherCore 2`, which was
  inside the iCloud CloudDocs container. See section 14.
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
  `~/dev/p36-stage/` -> `\\Mac\dev\p36-stage\`.  (was `~/Documents/p36-stage`
  -> `\\Mac\Home\Documents\p36-stage\` before the 2026-08-31 move; see section 14)
  Read AND write both work from the VM. `prlctl exec` round trip is ~0.3 s.
- **Never inline PowerShell inside a cmd string through zsh.** Quoting breaks in
  three layers. Write a `.ps1` (or `.cmd`) into `~/dev/p36-stage/` and
  invoke it by UNC path. Helper: `~/dev/p36-stage/vmr <name>` runs
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
| B1 | Repair preserves everything | PASS | `msiexec /f` exit 0; zero differing fields vs A5 state; 8/8 verbs — `STAGE_B_EVIDENCE.md` |
| B2 | Uninstall: what goes, what survives | PASS | exit 0; service+pipe+ARP+HKLM+StartMenu all gone; ProgramData and unmanaged aetherctl.exe survive by design — `STAGE_B_EVIDENCE.md` |
| B3 | Clean reinstall on empty box | PASS | plain `msiexec /i` exit 0, zero differing fields vs Gate A, 8/8 verbs; note aetherctl.exe survived B2 rather than being installed |
| B4 | Upgrade v1 -> v2 | PASS | RemoveExistingProducts ran; single ARP entry `{84140FFD-…}` 0.1.1; only differing field is InstallVersion; 8/8 verbs |
| C1 | Install interrupted mid-transaction | RECORDED (PASS: outcome + recovery) | killed both msiexec mid-FileCopy: NO rollback ran, 4 orphaned files, no service, no pipe, no ARP; a later failed install rolled back and cleaned them; recovered exit 0 |
| C2 | Service fails to start during install | PASS | injection package 0.9.2 (corrupt libomp): exit 1603 Error 1920 after the full 32 s service wait; rollback complete, every field back to pre-injection |
| C3 | Required file missing/corrupt at start | RECORDED (PASS: outcome + recovery) | start fails 1053 / event 7000+7009, service STOPPED, pipe count 0, machine usable; `msiexec /f` restores and 8/8 verbs return |
| C4 | Rollback via failing custom action | PASS | injection package 0.9.4 (hardener rejects `apply`): exit 1603 in 2.0 s at the custom action, before StartServices; rollback complete |
| D1 | `aetherctl update stage` real path | BLOCKED-BY-DESIGN | unconditional match arm `apps/aetherctl/src/offline.rs:138`; all 7 update verbs return CapabilityUnavailable on the VM — `STAGE_D_EVIDENCE.md` |
| D2 | stage -> apply -> rollback | BLOCKED | consequence of D1: nothing stageable, so nothing to apply or roll back |
| D3 | update trust disabled by default, no network | PASS | installed update-trust.json `enabled:false, channels:[]`; typed refusal precedes any transport; MSI is self-contained (EmbedCab) |
| E | Seal, snapshot P36-VM-QUALIFIED | PASS | `STAGE_E_SEAL.md`; snapshot `{a38386fa-15f9-4f86-a231-5de585ff3cd7}`; final state has ZERO differing fields vs Gate A and 8/8 verbs |

## 8. PACE LOG

(one line per ~10 shell commands, naming the current gate)

- cmd ~6 — Stage 0, writing session brain.
- cmd ~20 — Gate A1, authoring canonical ARM64 build recipe.
- cmd ~30 — Gate A2, full rebuild launched in VM background.
- cmd ~50 — Gates A2/A3 PASS, A4 decided. Next: A5 install.
- cmd ~72 — GATE A PASS. Snapshot P36-MSI-ALIGNED taken. Next: Stage B1 repair.
- cmd ~100 — B1, B2 PASS; Stage D closed from source + VM probe.
- cmd ~125 — GATE B PASS (B1-B4). Host disk exhausted; no new snapshots. Next: Stage C.
- cmd ~165 — C3 and C1 recorded and recovered. Next: C2 and C4 injection packages.
- cmd ~188 — GATE C complete: all four injections recorded, machine recovered with a plain msiexec /i (exit 0). Next: Stage E seal.
- cmd ~198 — SESSION COMPLETE. All gates A1-A5, B1-B4, C1-C4, D1-D3, E closed. See STAGE_E_SEAL.md.

## 9. FACTS ESTABLISHED THIS SESSION (do not re-derive)

## 10. P37 SERVER-BRANCH VM ACTION RECORD

- ACTION: build and validate the revised Server MSI, then rerun the ARM64 client gate.
- SNAPSHOT: preserve current VM state before mutation; recovery point remains P37-SHIPPING-QUALIFIED `{a1696567-7528-4136-a445-848dccd3d2c1}`.
- EXPECTED: build exit 0, `wix msi validate` exit 0 with zero ICE findings; client gate at 0.1.6 baseline (16 files, service RUNNING, matching ACLs, 18/18 verbs in both token contexts).
- RECOVERY: `prlctl snapshot-switch "Windows 11" --id {a1696567-7528-4136-a445-848dccd3d2c1}`.

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
  `\\Mac\dev\p36-stage\out\verify-<x>.json`.
- `scripts/p36vm/verbs-outer.ps1 -Label <x>` — runs the four typed verbs
  (`service detect`, `doctor`, `optimize status`, `scan status`) under BOTH
  actual-token contexts via one-shot Scheduled Tasks; needs
  `C:\AetherCore-P36\tools\aetherctl.exe` and
  `C:\AetherCore-P36\tools\verbs-inner.ps1` staged.
- Mac-side helper `~/dev/p36-stage/vmr <name>` runs `<name>.ps1` in the VM.

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
| P36-PRE-UNINSTALL | `{b92fa0f0-3150-468f-814e-d0cc8538cb7c}` | B2 uninstall |
| P36-POST-UNINSTALL | `{86fc5a29-34fc-4e46-8c91-43b151082246}` | B3 clean reinstall |
| P36-VM-QUALIFIED | `{a38386fa-15f9-4f86-a231-5de585ff3cd7}` | **the seal** — Stage C freed enough guest space for this one to succeed |

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

### B2 — uninstall
```
ACTION=    msiexec /x {FC8A3841-759D-B452-1864-161F84F56C03} /qn /l*v
           C:\AetherCore-P36\logs\b2-uninstall.log
SNAPSHOT=  P36-PRE-UNINSTALL {b92fa0f0-3150-468f-814e-d0cc8538cb7c}
EXPECTED=  msiexec exit 0. Service AetherCoreMaintenance stopped and REMOVED
           (sc query -> 1060 service does not exist). The named pipe gone.
           The seven MSI-authored files gone from INSTALLFOLDER. ARP entry gone.
           HKLM\SOFTWARE\AetherCore\InstallVersion gone. Start Menu folder gone.
           SURVIVING, and to be recorded either way: C:\ProgramData\AetherCore
           and its state/logs/support-staging contents (no RemoveFile authored
           for them), and the unmanaged aetherctl.exe, which no MSI component
           owns and which will therefore keep INSTALLFOLDER alive.
RECOVERY=  prlctl snapshot-switch "Windows 11" --id {b92fa0f0-3150-468f-814e-d0cc8538cb7c}
```

### B3 — clean reinstall on the emptied box
```
ACTION=    msiexec /i C:\AetherCore-P36\build\out\AetherCore-0.1.0-arm64.msi /qn /l*v
SNAPSHOT=  P36-POST-UNINSTALL (taken after B2, before B3)
EXPECTED=  exit 0, then the full Gate-A list matches verify-A5-postinstall.json
           except that aetherctl.exe is absent unless it survived B2.
RECOVERY=  restore P36-PRE-UNINSTALL {b92fa0f0-3150-468f-814e-d0cc8538cb7c}
```

### B4 — major upgrade 0.1.0 -> 0.1.1
```
ACTION=    msiexec /i C:\AetherCore-P36\build\out\AetherCore-0.1.1-arm64.msi /qn /l*v
SNAPSHOT=  P36-PRE-UPGRADE (taken after B3, before B4)
EXPECTED=  exit 0. RemoveExistingProducts runs (MajorUpgrade
           Schedule="afterInstallInitialize"). Exactly ONE ARP entry afterwards,
           key {84140FFD-5CBC-175D-928D-E493493F5F51}, version 0.1.1 — no mixed
           state, the 0.1.0 key {FC8A3841-...} gone. HKLM InstallVersion 0.1.1.
           All security properties unchanged, 8/8 verbs RETURNED.
RECOVERY=  restore P36-PRE-UPGRADE
```

### v2 package (built ahead, while the box was stable)
`AetherCore-0.1.1-arm64.msi`, 6,209,536 B,
SHA-256 `ad3df284b4b7e9072df3134a1bf67c4591a18d63a50e5264c0ee2abc029a2620`,
ProductCode `{84140FFD-5CBC-175D-928D-E493493F5F51}` — exactly the value the
deterministic scheme predicts for 0.1.1/arm64. Zero ICE. Same UpgradeCode.
Payload is the same set of binaries; only the package version differs.
`aethercore-desktop.exe` re-linked to `c351ccf0…` (same size) because a Tauri
rebuild is not byte-reproducible — expected, not a criterion.

## 13. HOST DISK EXHAUSTION — NO NEW SNAPSHOTS CAN BE TAKEN

Observed 2026-08-31 while attempting `P36-PRE-UPGRADE`:

```
Unable to create the snapshot. There is not enough free space on the physical disk.
```

Host state: `/System/Volumes/Data` 926 Gi total, 892 Gi used, **6.8 Gi free
(100% capacity)**. `~/Parallels/Windows 11.pvm` is 118 G; `~/Downloads` is
275 G and `~/Library/Caches` is 12 G.

**Not remediated, deliberately.** Deleting any snapshot or backup is forbidden
by this brief, and `~/Downloads` / `~/Library/Caches` are the user's data, not
this session's to delete. Freeing host space is a HUMAN action.

### Consequence and the adaptation used

Every snapshot taken up to `P36-POST-UNINSTALL` still exists and still
restores. From B4 onward the pre-action snapshot is an EXISTING snapshot plus,
where needed, a recorded automated replay:

- B4 recovery = restore `P36-POST-UNINSTALL {86fc5a29-34fc-4e46-8c91-43b151082246}`
  then re-run `\\Mac\Home\Documents\p36-stage\b3.cmd` (the exact clean-install
  step that already passed B3 with exit 0).
- Stage C recovery = restore `P36-MSI-ALIGNED {7d0696ae-ebc7-4c67-b076-438855d175f3}`,
  which is a fully verified working install, before each injection.

The four-line rule is still satisfiable: ACTION / SNAPSHOT / EXPECTED /
RECOVERY all have real values. What is lost is a fresh per-step restore point,
so each recovery costs one extra automated step instead of one restore.

### B4 — major upgrade 0.1.0 -> 0.1.1 (revised record)
```
ACTION=    msiexec /i C:\AetherCore-P36\build\out\AetherCore-0.1.1-arm64.msi /qn /l*v
           C:\AetherCore-P36\logs\b4-upgrade.log
SNAPSHOT=  P36-POST-UNINSTALL {86fc5a29-34fc-4e46-8c91-43b151082246}
           (no fresher snapshot is possible — see host disk exhaustion above)
EXPECTED=  exit 0; RemoveExistingProducts runs; exactly ONE ARP entry afterwards,
           {84140FFD-5CBC-175D-928D-E493493F5F51} version 0.1.1, with the 0.1.0
           key {FC8A3841-...} gone; HKLM InstallVersion 0.1.1; every security
           property unchanged; 8/8 verbs RETURNED.
RECOVERY=  prlctl snapshot-switch "Windows 11" --id {86fc5a29-34fc-4e46-8c91-43b151082246}
           then: prlctl exec "Windows 11" cmd.exe /c "\\Mac\Home\Documents\p36-stage\b3.cmd"
```

## 14. STAGE C PLAN (ordered to need zero snapshot restores if all goes well)

Order chosen so each injection either self-recovers or lands on a bare box:
C3 (post-install fault, recovered by the authored repair contract) -> uninstall
-> C1 (kill msiexec mid-copy on a bare box) -> C2 (poisoned-payload package:
service cannot start) -> C4 (poisoned-payload package: custom action fails).

The C2/C4 injections are done by building an INJECTION PACKAGE with one payload
file deliberately broken. Nothing on the machine is mutated to cause them — no
ACL change, no Service SID change, no pipe descriptor edit.

### C3 — required file missing at service start
```
ACTION=    stop AetherCoreMaintenance; rename libomp140.aarch64.dll to .missing;
           attempt sc start; record; then recover with msiexec /f
SNAPSHOT=  P36-MSI-ALIGNED {7d0696ae-ebc7-4c67-b076-438855d175f3}
           (no fresher snapshot is possible - host disk exhausted, see §13)
EXPECTED=  service start FAILS. Machine still usable. Then msiexec /f restores
           the file and the service starts again, 4 verbs return.
RECOVERY=  msiexec /f {84140FFD-5CBC-175D-928D-E493493F5F51} /qn
           If that fails: prlctl snapshot-switch "Windows 11" --id {7d0696ae-ebc7-4c67-b076-438855d175f3}
```

### C1 — install interrupted mid-transaction
```
ACTION=    uninstall v2; then start msiexec /i v1 and Stop-Process -Force every
           msiexec.exe the instant the verbose log reaches file copy
SNAPSHOT=  P36-POST-UNINSTALL {86fc5a29-34fc-4e46-8c91-43b151082246}
           (no fresher snapshot is possible - host disk exhausted, see §13)
EXPECTED=  unknown by design - this is the measurement. Record: does Windows
           Installer roll back cleanly, is the machine usable, is the service
           orphaned, is the pipe dangling, is C:\Windows\Installer consistent.
RECOVERY=  \\Mac\Home\Documents\p36-stage\b3.cmd   (plain msiexec /i v1, already
           proven exit 0 in B3). If that fails:
           prlctl snapshot-switch "Windows 11" --id {86fc5a29-34fc-4e46-8c91-43b151082246}
```

### C2 and C4 — injection packages (no machine mutation used to cause them)
```
ACTION=    uninstall the good v1; install AetherCore-0.9.2-arm64.msi (payload's
           libomp140.aarch64.dll replaced with 599,504 random bytes) -> expect
           the service to fail to start during install. Survey. Then install
           AetherCore-0.9.4-arm64.msi (payload's aethercore-install-hardener.exe
           replaced with System32\whoami.exe, which rejects the literal argument
           "apply" and exits non-zero) -> expect the deferred custom action
           HardenInstalledSecurity, authored Return="check", to fail and trigger
           rollback. Survey.
SNAPSHOT=  P36-POST-UNINSTALL {86fc5a29-34fc-4e46-8c91-43b151082246}
           (no fresher snapshot is possible - host disk exhausted, see §13)
EXPECTED=  both installs FAIL and roll back to "not installed": no service, no
           pipe, no ARP entry, INSTALLFOLDER holding only the unmanaged
           aetherctl.exe, machine usable.
RECOVERY=  \\Mac\Home\Documents\p36-stage\b3.cmd  (plain msiexec /i v1, proven
           exit 0 in B3 and again after C1). If that fails:
           prlctl snapshot-switch "Windows 11" --id {7d0696ae-ebc7-4c67-b076-438855d175f3}
```
Injection packages built (zero machine mutation, no ACL/SID/pipe edit):
- `AetherCore-0.9.2-arm64.msi` ProductCode `{4944D099-6844-678B-B98B-77B34955CD8A}`
- `AetherCore-0.9.4-arm64.msi` ProductCode `{C2FB7D6F-7B7F-315C-E5D0-24FE6C901809}`

## 15. INTERACTIVE DESKTOP QUALIFICATION (2026-08-31)

Configuration change carried into this gate: `Add-LocalGroupMember -Group Users -Member P36StandardUser`.
Immediately before planning, `qwinsta` measured `P36StandardUser` in session 6
(`Disc`) and `hasanalaaa` in session 7 (`Active`). `tscon 6 /dest:console`
reconnected the existing session without credentials; subsequent `qwinsta`
showed session 6 `P36StandardUser Active` and session 7 disconnected.

The prior standard-user denials were re-run after the Users-group change using a
Limited, Interactive scheduled task. All remain denied: service-exe write,
update-trust.json write, create-file in INSTALLFOLDER, mutation-lock write,
`sc stop AetherCoreMaintenance`, and `sc config AetherCoreMaintenance` (all
file operations Win32 access denied / 0x5; SCM operations exit 5).
Raw transcript: `~/dev/p36-stage/out/denials-reverified.txt`.

Desktop qualification evidence: an Interactive/ Limited scheduled task launched
`C:\Program Files\AetherCore\aethercore-desktop.exe` in session 6; process was
responsive under `HASANALAAA3A44\P36StandardUser`. Screenshots are retained at
`~/dev/p36-stage/out/desktop-launch.png` (shell Overview). Verb transcript
`~/dev/p36-stage/out/verbs-desktop-STD.txt` proves service verbs returned,
`diagnostics.stateUnavailable` was typed for doctor, and JSON insights reported
`engineLabel":"localModel"`.

Token proof: `~/dev/p36-stage/out/token-probe.txt` records
`IsInRole(Administrator)=False`, BUILTIN\\Users membership, and Medium Mandatory
Level (S-1-16-8192), establishing a genuine non-elevated standard-user token.

UAC consent behavior was not exercised because the remaining step requires a
human-operated elevation request at the console. Any future UAC finding must
carry the caveat that this VM has non-default `PromptOnSecureDesktop=0`; do not
change that setting. No credentials were entered, requested, generated, or
handled by the operator.

---

# PHASE 37 — SHIPPING READINESS SESSION (started 2026-08-31)

New brief: Stage 0 disk reclamation, Stage 1 MSI authoring defect, Stage 2
complete uninstall, Stage 3 terminal-first CLI distribution, Stage 4 server
and fleet capability.

This brief EXPLICITLY AUTHORIZES snapshot deletion (Stage 0), which the Phase
36 rules forbade. The Phase 36 prohibition is superseded for snapshots only.
Everything else in section 5 still stands.

## S0 — HOST DISK SURVEY (observed 2026-08-31, before any deletion)

```
/System/Volumes/Data   926 Gi total   891 Gi used   7.9 Gi free   100%
```

IMPORTANT MEASUREMENT FACT: this volume is APFS and the project tree is full of
**clones** (copy-on-write files sharing blocks). `ls -l` apparent size wildly
overstates disk cost. All sizes below are `du` unique-block figures.

| Consumer | Apparent | Unique blocks | Category |
|---|---|---|---|
| `~/Downloads` | — | 275 G | USER DATA — out of scope, not this session's to delete |
| `~/Library/Application Support/iMobie` | — | 87 G | USER DATA (device backups) — out of scope |
| `~/Parallels/Windows 11.pvm` | — | 128 G | snapshot chain, see S0.3 |
| `phase21-workspace/target` | 17 G | 17 G | REBUILDABLE |
| `~/Library/Caches` | 12 G | 12 G | REBUILDABLE |
| all 18 delivery `.zip` (root + `_archive`) | 34 G | **12 G** | needs tag proof |
| `_graphify` | 1.1 G | **40 K** | clone of the model; deleting frees ~nothing |
| all 3 `*.gguf` copies | 3.2 G | **1.07 G** | APFS clones of ONE file; deleting 2 frees ~0 |
| `node_modules` (4 dirs) | — | 0 B | already empty/cloned |

### S0.1 — the model "duplicates" are not duplicates on disk
Three paths hold `qwen2.5-1.5b-instruct-q4_k_m.gguf`, each 1,117,320,736 B,
inodes 64124843 / 67395891 / 64186585. `du` reports the 2nd and 3rd as **0 B**:
they are APFS clones sharing every block with the canonical copy. Deleting them
reclaims essentially nothing. Canonical (the one the build consumes):
`phase21-workspace/assets/models/qwen2.5-1.5b-instruct-q4_k_m.gguf`.
The other two are kept or removed on tidiness grounds only, not space grounds.

### S0.2 — archive-vs-tag coverage
Tags present on `origin`: p19 p20 p21 p22 p23 p23.1 p31 p32 p33 p34 p35
p36-tranche1. **No tag exists for p26, p27, p28, p29, p30.**
Therefore `AetherCore-Phase26/27/28/29-Master-Delivery.zip` (in `_archive`) and
`AetherCore-Phase30-Master-Delivery.zip` (root) are NOT represented by a tag and
are **KEPT** regardless of space pressure.
Every `PHASE*_FINAL_SHA256.txt` is kept unconditionally (105 B each).

### S0.3 — Parallels snapshot chain (linear, oldest first)

| # | Name | ID | Disk delta | .mem | Age |
|---|---|---|---|---|---|
| 1 | P36-CLEAN-BASELINE | `{6b721a10}` | 44.47 GB | 3.78 GB | 08-28 23:26 |
| 2 | P36-PRE-NATIVE-MUTATION | `{e9434b5f}` | 25.43 GB | 4.39 GB | 08-29 16:08 |
| 3 | P36-TRANCHE2-BASELINE | `{357848ae}` | 16.44 GB | 3.89 GB | 08-30 23:24 |
| 4 | P36-MSI-BUILT | `{a2f665b8}` | 13.25 GB | 3.13 GB | 08-31 02:36 |
| 5 | P36-MSI-ALIGNED | `{7d0696ae}` | 0.49 GB | 3.16 GB | 08-31 02:40 |
| 6 | P36-PRE-UNINSTALL | `{b92fa0f0}` | 0.70 GB | 3.21 GB | 08-31 02:51 |
| 7 | P36-POST-UNINSTALL | `{86fc5a29}` | 0.62 GB | 3.35 GB | 08-31 02:54 |
| 8 | **P36-VM-QUALIFIED** | `{a38386fa}` | 6.15 GB | 4.65 GB | 08-31 03:15 |
| — | live delta (current run) | `{5fbaabe3}` | 0.64 GB | — | 08-31 03:48 |

**Redundancy statement.** Snapshots 1 and 2 are marked FORBIDDEN-TO-RESTORE by
the Phase 36 brief: restoring either destroys the working install. A snapshot
that may never be restored has zero remaining recovery value. Snapshots 3–7 are
intermediate checkpoints of a qualification run that is now COMPLETE and sealed
(`STAGE_E_SEAL.md`); every state they capture is superseded by snapshot 8, which
is the verified end state (install present, service RUNNING, 8/8 verbs, zero
differing fields vs Gate A). **Snapshots 1–7 are therefore redundant.**
Snapshot 8 P36-VM-QUALIFIED is the sole recovery point for Stages 1–4 and is
NEVER deleted.

## S0.4 — DESTRUCTIVE ACTION RECORD: reclaim rebuildable trees

```
ACTION=    rm -rf "phase21-workspace/target"
           rm -rf ~/Library/Caches/*
SNAPSHOT=  none needed — neither path is a unique artifact.
EXPECTED=  ~29 GB reclaimed. Free space rises from 7.9 GiB to ~37 GiB.
           `cargo build` can regenerate target/ from the committed source at
           HEAD; every OS/tool cache regenerates on next use.
RECOVERY=  rebuild. target/ is derived output of committed source; no evidence
           file lives under it (all Phase 36 evidence is under
           phase21-workspace/docs/phase36/ and evidence/, both committed).
```

## S0.5 — DESTRUCTIVE ACTION RECORD: delete redundant snapshots 1-7

```
ACTION=    prlctl snapshot-delete "Windows 11" --id <id>  for each of
           {86fc5a29} {b92fa0f0} {7d0696ae} {a2f665b8} {357848ae}
           {e9434b5f} {6b721a10}     (leaf-inward order)
SNAPSHOT=  P36-VM-QUALIFIED {a38386fa-15f9-4f86-a231-5de585ff3cd7} is RETAINED
           and is the recovery point. It is never an argument to this command.
EXPECTED=  each call exits 0; `prlctl snapshot-list "Windows 11"` afterwards
           shows exactly ONE snapshot, {a38386fa}, still marked current.
           Each delete frees its .mem file outright (3.1-4.4 GB each, ~24 GB
           total) plus whatever the delta merge collapses.
RECOVERY=  NONE for snapshots 1-7 — deletion is final and that is the point;
           they are declared redundant in S0.3 above. Recovery for the VM
           itself remains `prlctl snapshot-switch "Windows 11" --id {a38386fa}`.
           Leaf-inward order means P36-VM-QUALIFIED is never the merge target
           of a failed operation.
```

## GATE 0 — RESULT: **PASS** (2026-08-31)

Free space on `/System/Volumes/Data`:

| | bytes | GB (decimal) | GiB |
|---|---|---|---|
| before | 8,482,394,112 | 8.48 | 7.90 |
| after | 100,453,310,464 | **100.45** | 93.55 |
| reclaimed | 91,970,916,352 | **91.97** | 85.65 |

**Caveat stated plainly:** the gate says "at least 100 GB free". That is met on
the decimal reading (100.45 GB), which is how `df` and Apple report capacity. On
the binary reading it is 93.55 GiB, NOT 100 GiB. Reaching 100 GiB would require
deleting user data (see "kept" below), which is outside this brief.

### What was deleted

| Item | Reclaimed | Why it was safe |
|---|---|---|
| `phase21-workspace/target` | 16.1 GiB | cargo build output. `CACHEDIR.TAG` present, `git ls-files` returns zero tracked paths under it, no Phase 36 evidence lives there (all evidence is under `docs/phase36/` and `evidence/`, both committed). Regenerated by `cargo build`. |
| `~/Library/Caches/*` | 12.0 GiB | OS and tool caches (Chrome 5.4 G, playwright 1.1 G, Homebrew, Yarn, pnpm, pip, node-gyp, updater downloads). Every one regenerates on next use. |
| Parallels snapshots 1–7 | ~46 GiB | See S0.3. Snapshots 1–2 were FORBIDDEN-TO-RESTORE and so had zero recovery value; 3–7 were intermediate checkpoints of a run that is complete and sealed, every state superseded by P36-VM-QUALIFIED. Deleted leaf-inward, each `exit 0`. |
| `AetherCore-Phase32/33/34/35-Master-Delivery.zip` | 8.78 GB | Proven redundant, see below. |
| `~/Library/Developer/Xcode/DerivedData` | 1.2 GB | Xcode build cache. |
| `~/Library/pnpm/store` | 1.1 GB | pnpm content-addressed store; `store` was its only child (no global binaries). Regenerated by `pnpm install`. |
| `_graphify/` | 15 MB | derived source index. Its 1.1 GB `.gguf` was an APFS clone costing 0 blocks. |
| scratchpad extractions | 4.0 GB | this session's own temp files. |

### The archive proof (Gate 0's "requires proof" clause)

`unzip` fails on these files: they are **gzip**, not zip, despite the `.zip`
extension (`head -c4` = `1f 8b 08 00`). Listed with `tar tzf`.

Test applied to every file inside each archive:
1. is it byte-identical to the same path at `HEAD`? (`git hash-object` vs
   `git rev-parse HEAD:<path>`), else
2. does its exact blob exist in the git object database? (`git cat-file -e`)

A file passing either test is recoverable from the repo. Results:

| Archive | files | not recoverable from git |
|---|---|---|
| Phase32 | 1014 | **0** |
| Phase33 | 1035 | **0** |
| Phase34 | 1069 | **0** |
| Phase35 | 1124 | **1** |
| Phase32-pre-provenance-fix | 1014 | **9** |

The single Phase 35 exception is the signed offline release zip (89,047 B, blob
`d1911445e3cb2bf887b3e618ee3ecfbcf3344acf`), which appears at two paths inside
the archive with identical content and differs from the untracked copy on disk.
It was committed to `_archive-preserved/phase35/` and pushed BEFORE any archive
was deleted.

Note the tag-only test would have given the wrong answer: `git ls-tree -r p35`
holds 937 workspace files while `HEAD` holds 1237, because ~300 files
(`scripts/`, `services/maintenance-service/src/`, `tools/`) were untracked when
p35 was cut and added to git later. Coverage had to be proven against the whole
object database, not against the tag.

### What was KEPT, and why

| Item | Size | Reason |
|---|---|---|
| `_archive/AetherCore-Phase32-…-pre-provenance-fix-…zip` | 3.9 GB | 9 files absent from git; one is a 4.12 GB `PHASE_31_BINARY_SAFE_PATCH/changes.patch`. Extracting it to preserve it would cost 4.12 GB to free 4.18 GB. Retained whole. |
| `AetherCore-Phase26/27/28/29/30-Master-Delivery.zip` | 0 local blocks | no `p26`–`p30` tag exists, so not proven. Also iCloud-**dataless** — they cost nothing locally. |
| all 15 `PHASE*_FINAL_SHA256.txt` | 1.6 KB | seal record, kept unconditionally per the brief. |
| canonical `qwen2.5-1.5b-instruct-q4_k_m.gguf` | 1.07 GB | the file the build consumes. |
| the two "duplicate" `.gguf` | **0 bytes** | APFS clones of the canonical (see S0.1). Deleting them reclaims nothing, so they were left alone. |
| `P36-VM-QUALIFIED` `{a38386fa}` | 78.9 GB + 4.65 GB mem | the sole recovery point for Stages 1–4. |
| `~/Downloads` | 275 GB | **user data.** 267 GB of it is one folder of personal files. Out of scope. |
| `~/Library/Application Support/iMobie` | 87 GB | **user data** (device backups). Out of scope. |
| `~/Library/Developer/CoreSimulator/Devices` | 11 GB | rebuildable in principle, but it is simulator state for unrelated projects. Available as a further lever if the user wants it; not taken unilaterally. |

### Post-deletion VM health check

After all seven snapshot merges: `sc query AetherCoreMaintenance` -> `STATE 4
RUNNING`, and `C:\Program Files\AetherCore` still lists all eight files. The
merges did not damage the guest.

### Measurement lesson for future sessions (do not re-derive)

`~/Documents` is **iCloud Drive-synced**, and most large files there are
`compressed,dataless` — evicted placeholders with `st_blocks = 0`. `ls -l`
apparent size is meaningless for disk planning on this host; only `du` /
`stat -f %b` tell the truth. Deleting a dataless file frees ZERO local space
while permanently removing it from iCloud — strictly worse than useless.
`brctl status` also shows iCloud actively syncing `.git/objects`, which is why
`git ls-tree` on an old tag can take minutes (object hydration).

## BLOCKER RAISED AT GATE 0 — CONCURRENT SESSION IN THIS WORKING TREE

At 04:07–04:09 local, while this session was mid-Stage-0, ANOTHER agent
committed four UI/icon commits into this same working tree on a new branch
`codex/design-elevation` (created from `b88d4bc`):

```
0676c8e docs(ui): record design elevation handoff
043ccb1 docs(phase36): close historical icon placeholder debt
ffdf134 feat(ui): codify evidence-first design language and brand mark
9f07df5 feat(desktop): ship AetherCore evidence shield icon set
```

Consequence observed: the working tree was silently moved off `main`, so this
session's `_archive-preserved` commit initially landed on THEIR branch. It was
re-created on `main` via plumbing (`commit-tree` + `update-ref`) without
checking out, so their checkout was never disturbed. `main` is now `22b93a1`
and pushed.

Stages 1–4 are NOT started. They edit `installer/wix/Product.wxs`, rebuild the
workspace, and mutate the VM — none of which can produce attributable evidence
while a second agent is committing to the same tree. **Human decision required
before Stage 1.**

---

# 14. REPOSITORY RELOCATED OUT OF iCLOUD (2026-08-31)

## Why

`/Users/hasanalaaa/Documents/AetherCore 2` was inside the iCloud CloudDocs
container. Proof (not inference): `~/Documents/AetherCore 2` and
`~/Library/Mobile Documents/com~apple~CloudDocs/Documents/AetherCore 2` are the
**same inode, 63148279**. `~/dev` is not in the container — `brctl status` has
zero references to it.

iCloud evicts file contents and leaves `compressed,dataless` placeholders.
Reading one blocks on a network fetch. Measured cost: **2.5 seconds per git
object** (100 loose objects took 251 s). At the worst point **823 of 2501
objects in `.git` were dataless.**

Three failures this session traced to exactly this, and to nothing else:

1. `pnpm --dir apps/ui build` failed with
   `ETIMEDOUT: connection timed out, read` inside
   `node_modules/.pnpm/aria-query@5.3.1/...`. 1062 of 1565 files under
   `node_modules` were dataless. **This was previously reported as a broken
   symlink for `@jridgewell/remapping`. That diagnosis was wrong** — there were
   zero dangling symlinks tree-wide and `@jridgewell/remapping@2.3.5` was
   present the whole time.
2. `git worktree add` stalled at 22% (603/2651 files) after 25 minutes.
3. `git ls-tree -r p35` took minutes instead of milliseconds.

## What was done

Everything was pushed to `origin` first, so the new location was created by a
fresh `git clone` rather than by copying iCloud stubs. Nothing depended on
iCloud returning an object.

| | old | new |
|---|---|---|
| repo | `~/Documents/AetherCore 2` | **`~/dev/aethercore`** |
| design worktree | (stalled, never completed) | **`~/dev/aethercore-design`** |
| VM staging | `~/Documents/p36-stage` | **`~/dev/p36-stage`** |
| VM-visible repo | `\\Mac\Home\Documents\AetherCore 2\` | **`\\Mac\dev\aethercore\`** |
| VM-visible staging | `\\Mac\Home\Documents\p36-stage\` | **`\\Mac\dev\p36-stage\`** |

Verification, each measured rather than assumed:

- `git fsck --full` on the new clone: **exit 0, no output.**
- All four branches resolve; all **12 tags resolve to the same commits** as the
  old repo (`p19 p20 p21 p22 p23 p23.1 p31 p32 p33 p34 p35 p36-tranche1`).
- **0 dataless files** in the new `.git`.
- Worktree created in **0.315 s** (against 25 min stalled at 22% in iCloud).
  `.git` file inside it reads
  `gitdir: /Users/hasanalaaa/dev/aethercore/.git/worktrees/aethercore-design`,
  and `git rev-parse`/`git log`/`git status` all run inside it.
- Object-read throughput, full read of every object in the repo:

  | | rate |
  |---|---|
  | old repo, cold iCloud objects | **0.4 objects/sec** (100 objects / 251 s) |
  | new repo | **21,169 objects/sec** (2,498 objects / 118 ms) |

## Parallels share — CHANGED, and this is the part that will bite a new session

`prlctl list -i` reported `Host defined sharing: Off`. The VM only ever saw
Desktop / Documents / Downloads through **Shared Profile**, which is why
`\\Mac\Home` lists exactly those three and why `\\Mac\Home\dev` did not exist.

A host shared folder was added:

```
prlctl set "Windows 11" --shf-host-add dev --path /Users/hasanalaaa/dev
```

Shared Profile was left **on**, so `\\Mac\Home\Documents\...` still works for
anything historical. Both directions proven on the new path:

- VM reads the Mac: `type \\Mac\dev\p36-stage\PROBE.txt` returned the token
  written on the Mac; `dir \\Mac\dev\aethercore\phase21-workspace\installer\wix`
  listed `Product.wxs`, `Bundle.wxs`, `README.md`.
- VM writes the Mac: a file written by the guest to
  `\\Mac\dev\p36-stage\out\VM_WRITE_TEST.txt` read back identically on the Mac.

## Absolute paths: what was rewritten and what deliberately was NOT

30 tracked files contain the old absolute path. They are **not** all the same
kind of thing, and rewriting all of them would corrupt evidence.

**Rewritten — operational, would send a future session or script to the wrong
place:**

- `docs/phase36/SESSION_CONTEXT.md` — the repo path, the staging dir, the UNC
  path, and the `vmr` helper path.
- 9 scripts that hard-coded the root as a fallback
  (`_build_p27/p28/p29/p30/p31/p32_archive.py`, `_build_p27_patch.py`,
  `_p27_roundtrip.py`, `_p33_part_a_seal.py`). These were **not** repointed at
  the new absolute path, which would only rot again. They now derive it:
  `Path(__file__).resolve().parents[1]` for the workspace and `parents[2]` for
  the repo root. Verified to resolve to
  `/Users/hasanalaaa/dev/aethercore/phase21-workspace` and
  `/Users/hasanalaaa/dev/aethercore`; all 9 pass `py_compile`.

**Deliberately NOT rewritten — historical records whose bytes are the
evidence:**

- `PHASE20_FINAL_SHA256.txt` and the other seal pointer files.
- `PHASE_2x/3x_BINARY_SAFE_PATCH/changes.patch` — patch bodies covered by a
  `MANIFEST.json` SHA; editing them invalidates the manifest.
- `docs/phase2x/MASTER_DELIVERY_REPORT.md`, `docs/phase32/HYGIENE.md`,
  `docs/phase32/ISSUES.json`.
- `_handoff/p36-codex-to-hermes/*` — `CODEX_SESSION.jsonl` is sealed by
  `CODEX_SESSION_SHA256.txt`.
- The destructive-action records in section 12 above. They record the command
  that was ACTUALLY RUN at the time, from the path that existed then. A
  recovery command in a historical record is a fact, not an instruction — if you
  need to re-run one, translate `\\Mac\Home\Documents\p36-stage\` to
  `\\Mac\dev\p36-stage\` yourself.

## What stayed behind in iCloud, on purpose

`~/Documents/AetherCore 2` still exists and was NOT deleted. It holds the
delivery archives that Gate 0 kept: the Phase 26–30 zips and
`_archive/AetherCore-Phase32-…-pre-provenance-fix-…zip`. Most are dataless and
cost zero local bytes; materialising ~13 GB out of iCloud purely to relocate
files that are fine where they are would have been pointless. `~/Parallels` was
not touched.

## CORRECTION to the Gate 0 record

Gate 0 above states the two duplicate `.gguf` copies are **APFS clones**. That
was wrong. They — and the canonical copy — are **`compressed,dataless` iCloud
placeholders**. `du` reports 0 blocks for both reasons, which is why the two are
indistinguishable by size alone.

The Gate 0 conclusion is unchanged and still correct: deleting the duplicates
reclaims no local space. The reason differs, and the difference matters —
**the canonical model was never on this disk.** It is 1,117,320,736 B,
SHA-256 `6a1a2eb6d15622bf3c96857206351ba97e1af16c30d7a74ee38970e434e9407e`
(pinned in `assets/models/models.manifest.json`, fail-closed loader). Its
materialisation out of iCloud is tracked separately; the desktop build cannot
run without it.

---

# 15. GITIGNORED-BUT-REQUIRED AUDIT (2026-08-31)

The relocation in section 14 was done by `git clone`. That is complete for
tracked content and **silently drops everything `.gitignore` excludes.** Two
files that the product needs were not in git and would have been lost with the
old directory. This section is the full audit so it cannot happen again.

## 15.1 The embedded model — RECOVERED

`assets/models/qwen2.5-1.5b-instruct-q4_k_m.gguf` is excluded by `*.gguf`. It is
**not in git and not on the remote.** Without it,
`EMBEDDED_MODEL_RELATIVE_PATH` (`crates/intelligence-core/src/llama.rs:47`) does
not resolve and the fail-closed loader refuses to start — the local intelligence
core, the core of the product, does not run.

Recovery, each step measured:

| step | result |
|---|---|
| materialise from iCloud (`dd if=... of=/dev/null bs=1m`) | **1,117,320,736 bytes in 753.5 s (1.48 MB/s)**, flags went `compressed,dataless` -> `-`, blocks 0 -> 2,182,272 |
| SHA-256 of the materialised source | `6a1a2eb6…9407e` |
| pinned in `llama.rs:54` | `6a1a2eb6…9407e` — **MATCH** |
| pinned in `assets/models/models.manifest.json` | `6a1a2eb6…9407e` — **MATCH** |
| copy to `~/dev/aethercore/…` (same relative path) | exit 0 in **0.374 s** (local->local) |
| SHA-256 after the copy | `6a1a2eb6…9407e` — **MATCH**, size 1,117,320,736 |

**Why `brctl download` and two `cp` attempts appeared to fail.** They did not.
macOS materialises a dataless file into the page cache and only publishes the
allocated blocks at completion, so `stat -f %b` reads **0 for the entire
download** and then jumps to the full size. Polling block count is not a
progress indicator. `dd` with a byte counter is. Do not kill a materialisation
because block count is not moving — the first two attempts were killed for
exactly that wrong reason and wasted ~35 minutes.

## 15.2 Gate A2 evidence log — RECOVERED, and the rule that ate it is fixed

`docs/phase36/evidence/A2-build.log` (62,257 B) is cited in the Gate A2 row of
the progress table. `phase21-workspace/.gitignore` had a bare `*.log`, so it was
**the only one of the 34 files in `docs/phase36/evidence/` that was not
tracked** — 33 of 34 were. It was dataless and not in git in any form.

Materialised, copied, verified byte-identical
(`337f42dbba8a6996…`), and the ignore rule was given a negation so this class
cannot recur:

```
*.log
# Phase evidence logs are sealed records, not build noise. Never let *.log eat them.
!docs/phase*/evidence/*.log
```

## 15.3 Full sweep — every ignore rule, and whether anything it excludes is needed

| rule | excludes | needed to build or run? |
|---|---|---|
| `*.gguf` | the embedded model | **YES — §15.1. Recovered.** |
| `*.log` | `docs/phase36/evidence/A2-build.log` | **YES (as evidence) — §15.2. Recovered, rule fixed.** |
| `target/`, `/target/` | cargo output | no — it is the output |
| `node_modules/`, `/apps/ui/node_modules/` | pnpm tree | no — `pnpm install --frozen-lockfile` rebuilds it in 495 ms |
| `dist/`, `/apps/ui/dist/` | vite output | no — `pnpm build` rebuilds it in 884 ms |
| `*.zip` | delivery archives; `release/phase35/…offline.zip`; `PHASE_35_…/BINARY_ARTIFACTS/…offline.zip` | not a build input. The release zip's blob is `d1911445…`, already in git via `_archive-preserved/`. Restored to its natural path anyway. |
| `**/BINARY_ARTIFACTS/` | 7 Phase 35 signed release files | no — all 7 blobs verified present in the object DB (`eb1cd21e`, `faacc9b6`, `17f8dc17`, `f69f440f`, `de65c2f5`, `823bb0d5`, `d1911445`) |
| `*.db`, `*.db-shm`, `*.db-wal` | `phase21-workspace/state/aethercore.db*` and two `C:\ProgramData\…` trees | no — runtime state written by a service run on 2026-08-24. `git grep "state/aethercore.db"` over `*.rs *.toml *.ps1 *.py` returns nothing: no fixture, no build or test dependency. |
| `_archive/`, `_graphify/` | historical archives, derived index | no |
| `.DS_Store`, `.vscode/`, `.idea/`, `__pycache__/`, `*.pyc` | editor/OS/python noise | no |
| `/.devdata/`, `/out/` | not present on disk | n/a |

**Result: exactly two required files were outside git. Both are recovered and
verified. Nothing else the product needs is excluded.**

## 15.4 Standing rule

`git clone` is not a backup of this project. Anything matched by `.gitignore`
must be carried separately and verified by hash. Before removing any working
copy, run:

```
git ls-files --others --ignored --exclude-standard
```

and justify every entry.

---

# 16. PHASE 37 — SHIPPING-READINESS BRIEF (v2), started 2026-08-31 14:00

The v2 brief renumbers the stages. Its **Stage 0 is "merge the design work"**,
not the disk reclamation recorded in section 13/Gate 0 above. To avoid
collision the v2 gates are written here as `S0`..`S4`.

| Gate | What it proves | Status | Evidence |
|---|---|---|---|
| S0 | design/shell-v2 merged, checks green, dev-only files absent from bundle | **PASS** | §16.1 below; merge `1cf86be`, icon cherry-pick `e8170dd` |
| S1 | aetherctl authored in Product.wxs; full file audit; clean-box install proves every file and every verb | **PASS** | §16.4; 15 files, engineLabel=localModel, 18/18 verbs, zero ICE |
| S2 | uninstall leaves zero product trace, user-chosen exports kept, idempotent | **PASS (re-gated)** | §16.6 then §16.10 — S4 found a survivor the first sweep missed; fixed, sweep hardened, re-run clean on 0.1.6 |
| S3 | terminal-first CLI install on a clean machine, one documented command | **PASS** | §16.7; 1.9 MB archive, Expand-Archive, 8 verbs, every exit code as documented |
| S4 | server-readiness assessment with evidence per claim | **PASS** | `docs/SERVER_READINESS.md`; §16.9; 3 fleet defects found and fixed |

## 16.1 GATE S0 — RESULT: **PASS** (2026-08-31)

Merged `origin/design/shell-v2` (`836906f`) into `main` with `--no-ff` as
`1cf86be`. Automatic merge, zero conflicts.

Verification, run against the MERGED tree (stronger than verifying the branch
alone), all on the Mac at `~/dev/aethercore`:

| Check | Expected | Observed | Result |
|---|---|---|---|
| `pnpm --dir apps/ui build` | exit 0 | exit 0, 196 modules, built in 695 ms | PASS |
| svelte-check errors | 0 | **0** | PASS |
| svelte-check warnings | not above 17 | **17** (all `css_unused_selector`, 3 files) | PASS (at the ceiling) |
| EN/AR catalogue key parity | delta 0 | `en=1549 ar=1549 delta=0` | PASS |
| `node tests/transport-env.test.mjs` | pass | 1/1 pass | PASS |

Parity was measured with the key extractor already in the repo
(`scripts/phase18-driver-authority-audit.py:53`, check `P18-I18N-001`), not a
new one.

### Dev-only files confirmed absent from the production bundle

`layout-fixture.html`, `layout-sweep.html` and `src/dev/layout-fixture.ts` are
verification harnesses. Three independent facts, each measured:

1. `dist/` after a clean `rm -rf dist && pnpm build` contains exactly
   `index.html`, `assets/index-BKWlQf78.css`, `assets/index-CZwBFtmd.js`.
2. `grep -rl "layout-fixture\|layout-sweep\|layoutFixture" dist/` → **no match**.
3. `grep -rn "layout-fixture" apps/ui/src apps/ui/index.html apps/ui/vite.config.*`
   → **no match**. Nothing in the entry graph reaches them.

`apps/desktop/tauri.conf.json` sets `"frontendDist": "../ui/dist"`, so the
Tauri bundle takes `dist/` only; the two `.html` files sit at the `apps/ui`
root, outside it.

### OBSERVED ≠ EXPECTED — recorded, per the STOP rule

The brief states `design/shell-v2` "carries the real app icon". **It does
not.** `git diff --stat main origin/design/shell-v2` touches 25 files, none of
them under `apps/desktop/icons/` and not `tauri.conf.json`.

The icon set is on `origin/codex/design-elevation` only, in commit
`9f07df5 feat(desktop): ship AetherCore evidence shield icon set` — 19 icon
files plus a `tauri.conf.json` change adding `icons/icon.ico` and
`icons/icon.icns` to the `bundle.icon` list (previously `icon.png` alone, so
the MSI would otherwise carry no `.ico`). That branch was cut from the older
`b88d4bc` and its other commits are rebased duplicates of what
`design/shell-v2` already contains, so merging it whole would have re-litigated
merged content.

Action taken: the single self-contained icon commit was cherry-picked onto
`main` as `e8170dd` (`git cherry-pick -x`), exit 0, no conflicts. The rest of
`codex/design-elevation` was NOT merged and is superseded by `design/shell-v2`.

### Concurrency note (the §"BLOCKER RAISED AT GATE 0" above)

`git worktree list` shows `~/dev/aethercore-design` checked out at
`design/shell-v2`, last commit 13:26, i.e. the design track was active ~40 min
before this session started. This session did **not** check out that branch in
the main tree and did not write into that worktree; it merged the branch by
reference and verified in `~/dev/aethercore`. The design track can keep working
on `design/shell-v2`.

## 16.2 DESTRUCTIVE ACTION RECORD — S1: refresh the VM source tree and rebuild

```
ACTION=    (a) prlctl snapshot "Windows 11" -n P37-PRE-STAGE1
           (b) overwrite 66 source files in
               C:\AetherCore-P36\workspace\AetherCore-Phase35-Master-Delivery
               from \\Mac\dev\aethercore\phase21-workspace (the non-docs delta
               218e0d8..HEAD: the design merge, the icon set, and the Stage 1
               MSI authoring + asset-resolution fixes)
           (c) run scripts\build-arm64-msi.cmd 0.1.2 to produce a NEW MSI whose
               payload includes aetherctl.exe and whose package authors the
               assets tree
SNAPSHOT=  P37-PRE-STAGE1 (taken by step (a) — host now has 90 GiB free, so a
           fresh restore point is possible again; §13's exhaustion is resolved).
           Fallback if the snapshot cannot be taken: P36-VM-QUALIFIED
           {a38386fa-15f9-4f86-a231-5de585ff3cd7}, the Phase 36 seal.
EXPECTED=  (b) every copied file's SHA-256 in the guest equals the Mac's.
           (c) build exits 0; wix msi validate exits 0 with ZERO ICE matches;
               the payload dir contains SIX .exe (was five) plus
               libomp140.aarch64.dll and update-trust.json; the MSI is roughly
               1.1 GB larger than the 6.2 MB Phase 36 MSI because the 1.07 GB
               model is now inside it.
           Nothing on the machine is installed or uninstalled by this step.
RECOVERY=  prlctl snapshot-switch "Windows 11" --id <P37-PRE-STAGE1 id>
           The installed product is NOT touched by (b) or (c); the running
           service keeps its current binaries either way.
```

## 16.3 DESTRUCTIVE ACTION RECORD — GATE S1: clean-box install of 0.1.2

The 0.1.2 build is done: exit 0, `wix msi validate` exit 0, **zero `ICE\d+`
matches** in the log, MSI `872d6997…c95b9e`, **1,099,640,832 B** (was 6,205,440 B
for 0.1.0 — the difference is the 1.07 GB model that is now authored).
ProductCode `{2D97C23D-D2A1-83FE-3675-90F95D55540B}`, a new code under the same
UpgradeCode, so this is a true major upgrade path and a legitimate clean install.
Payload now holds SIX .exe: `aetherctl.exe b8a29c92…4350d8` joins the five.

```
ACTION=    (a) msiexec /x {FC8A3841-759D-B452-1864-161F84F56C03} /qn   (the 0.1.0 install)
           (b) DELETE the leftovers the gate requires to be absent:
               C:\Program Files\AetherCore (in full — today only the unmanaged
               aetherctl.exe survives an uninstall), C:\ProgramData\AetherCore,
               HKLM\SOFTWARE\AetherCore, HKCU\...\Software\AetherCore.
               This is done BY HAND here on purpose: Stage 2 is the gate that
               makes the UNINSTALLER do it. Gate 1 only needs a bare box.
           (c) sweep and record that the box is bare
           (d) msiexec /i AetherCore-0.1.2-arm64.msi /qn  (plain install, no flags)
SNAPSHOT=  P37-PRE-STAGE1 {e94d539e-8046-443b-871c-9d6711c34fd2}
EXPECTED=  (a) exit 0. (c) zero AetherCore files, no service, no pipe, no ARP
           entry, no HKLM/HKCU key. (d) exit 0, then:
             - INSTALLFOLDER holds FIFTEEN files: six .exe,
               libomp140.aarch64.dll, update-trust.json,
               assets\models\{qwen2.5-1.5b-instruct-q4_k_m.gguf,models.manifest.json},
               assets\models\licenses\{Apache-2.0.txt,Qwen-GGUF-NOTICE.txt},
               assets\vulndb\{vulndb.json,vulndb.manifest.json,cis_map.json}
             - the gguf is 1,117,320,736 B with sha256 6a1a2eb6…9407e
             - service LocalSystem AUTO_START(DELAYED) RUNNING, SID UNRESTRICTED,
               pipe SDDL and install-dir icacls identical to §10
             - ARP one entry {2D97C23D-…} version 0.1.2
             - every verb RETURNS under BOTH token contexts, and `insights list`
               reports engineLabel **localModel**, not ruleFallback
RECOVERY=  prlctl snapshot-switch "Windows 11" --id {e94d539e-8046-443b-871c-9d6711c34fd2}
```

## 16.4 GATE S1 — RESULT: **PASS** (2026-08-31)

Bare-box install of `AetherCore-0.1.2-arm64.msi`
(`872d6997…c95b9e`, 1,099,640,832 B), built with zero `ICE\d+` matches and
`wix msi validate` exit 0.

### The box really was bare first

`msiexec /x {FC8A3841-…}` exit 0, then the leftovers were deleted by hand (Stage
2 is the gate that makes the UNINSTALLER do that; Gate 1 only needs a clean
machine). Sweep before installing:

```
INSTALLDIR_EXISTS=False   PROGRAMDATA_EXISTS=False
HKLM_KEY=False            HKCU_KEY=False
SERVICE=OpenService FAILED 1060: the specified service does not exist
PIPE_COUNT=0              ARP_COUNT=0
STARTMENU=False           TASKS=0            FIREWALL_RULES=0
```

### Install: `msiexec /i … /qn`, exit 0 in 59 s

`C:\Program Files\AetherCore` now holds **FIFTEEN** files (it held eight, and
only seven of those were owned by the package):

| bytes | path |
|---|---|
| 586,240 | `aethercore-consent-broker.exe` |
| 6,380,544 | `aethercore-desktop.exe` |
| 246,784 | `aethercore-install-hardener.exe` |
| 9,811,968 | `aethercore-maintenance-service.exe` |
| 672,768 | `aethercore-update-broker.exe` |
| **3,959,808** | **`aetherctl.exe`** — the named defect, now MSI-owned |
| 11,358 | `assets\models\licenses\Apache-2.0.txt` |
| 11,343 | `assets\models\licenses\Qwen-GGUF-NOTICE.txt` |
| 898 | `assets\models\models.manifest.json` |
| **1,117,320,736** | **`assets\models\qwen2.5-1.5b-instruct-q4_k_m.gguf`** |
| 3,103 | `assets\vulndb\cis_map.json` |
| 6,704 | `assets\vulndb\vulndb.json` |
| 142 | `assets\vulndb\vulndb.manifest.json` |
| 599,504 | `libomp140.aarch64.dll` |
| 83 | `update-trust.json` |

Installed gguf sha256 `6a1a2eb6…9407e` = the pin compiled into
`llama.rs` and the pin in `models.manifest.json`.

### Security properties: ZERO differing fields vs the §10 baseline

- `sc qc`: TYPE 10, START_TYPE 2 AUTO_START (DELAYED), ERROR_CONTROL 1 NORMAL,
  BINARY_PATH_NAME the installed service, SERVICE_START_NAME LocalSystem;
  `sc query` STATE 4 RUNNING.
- `sc qsidtype`: UNRESTRICTED.
- pipe SDDL: `O:S-1-5-80-4285065559-…-1187574229G:SYD:P(A;;0x12008b;;;AU)(A;;FA;;;S-1-5-80-…)`
  — identical to §10.
- `icacls`: `NT SERVICE\AetherCoreMaintenance:(OI)(CI)(RX)`,
  `BUILTIN\Users:(OI)(CI)(RX)`, `BUILTIN\Administrators:(OI)(CI)(F)`,
  `NT AUTHORITY\SYSTEM:(OI)(CI)(F)`. Users still RX with no write.
- ARP: exactly one entry `{2D97C23D-D2A1-83FE-3675-90F95D55540B}` 0.1.2;
  `HKLM\SOFTWARE\AetherCore\InstallVersion = 0.1.2`.
- `libomp140.aarch64.dll` present; no `ipc_probe*` anywhere.

### Every verb returns, from the INSTALLED aetherctl, under both token contexts

`verbs-outer.ps1 -Label S1` ran nine verbs as `p36standarduser`
(IS_ELEVATED_ADMIN=False) and as `p36admin` (True) via one-shot Scheduled Tasks.
**18/18 RETURNED.** Both transcripts record
`CTL_PATH=C:\Program Files\AetherCore\aetherctl.exe` and
`CTL_SHA256=b8a29c92…4350d8`, which is the payload hash from the build manifest —
so the binary exercised is the one the MSI installed, not a leftover.

The three decisive outputs:

1. **`insights list` → `{"engineLabel":"localModel", …}`**
   This is the direct proof the model landed. `engine_label()` returns
   `localModel` only when `EMBEDDED_ENGINE_ACTIVE` was set by a successful
   `verify_model_hash` + `load` at service start. Before this change the
   artifact was not installed at all, so every installed instance reported
   `ruleFallback`.
2. **`self-check`** → `manifestValid true`,
   `modelsDir C:\Program Files\AetherCore\assets\models`, artifact
   `1117320736` bytes, `sha256Match true`.
   (`loaded:false` is CORRECT and not a failure: `aetherctl`'s
   `embedded-model` feature is off by default and `--load-model` was not passed,
   so the probe reports the honest not-available answer.)
3. **`sec audit --profile cis-l1`** produced a scored `aethercore.compliance.v1`
   report on a machine that is not the build machine — the
   `env!("CARGO_MANIFEST_DIR")` path could never have resolved there.

### The vulndb fix, proven from a foreign working directory

```
CWD=C:\Windows\Temp
CWD_HAS_ASSETS_VULNDB=False
LANE=cve       STATUS={"count":0,"kind":"ok"}
LANE=firewall  STATUS={"count":1,"kind":"ok"}
```
`kind:"ok"` means `vulndb::load_verified` opened and hash-verified the DB. With
the old CWD-relative resolution this lane could only have been
`NotAvailable(vulndbIntegrity: …db file missing…)`.

### Findings recorded during this gate, NOT fixed here

- **`aetherctl --help` and `-h` are unknown commands.** They print
  `cli.usage.unknownCommand` to stderr and exit non-zero, then dump usage. Only
  the bare word `help` is a real verb. This is a Stage 3 (S3) defect; measured,
  not assumed.
- **`platform_tag()` returns `"other"` on Windows.**
  `crates/security-audit/src/lib.rs:110` tests only macos and linux, so every
  compliance report generated on the product's PRIMARY platform carries
  `host_fingerprint = "other:<digest>"`. Recorded for S4; not touched here
  because it changes the digest of every previously issued report.
- **`verbs-inner.ps1` records `EXIT_CODE=` (empty).**
  `Start-Process -PassThru` + `WaitForExit(ms)` does not populate `ExitCode` on
  this PowerShell. `RESULT=RETURNED` and the captured stdout are unaffected and
  are what the gate asserts. Harness gap; fix before S3 needs exit codes.

## 16.5 DESTRUCTIVE ACTION RECORD — GATE S2: prove uninstall leaves no trace

```
ACTION=    (a) build 0.1.3 with the Stage 2 authoring (RemoveFolderEx,
               ForceDeleteOnUninstall x2, UNINSTALL.txt, ARPCOMMENTS)
           (b) uninstall 0.1.2 and clear leftovers -> bare box
           (c) install 0.1.3 clean
           (d) CREATE REAL STATE: run the mutating/reading verbs so the service
               fills C:\ProgramData\AetherCore, and plant one deliberately DEEP
               path under recovery\driver-backups\ (declared synthetic - a real
               driver install is out of scope on this VM) to prove the removal
               reaches unknown names at unknown depth
           (e) record the full ProgramData tree and the registry footprint
           (f) msiexec /x <0.1.3 ProductCode> /qn
           (g) sweep the machine for ANY remaining trace, and report every
               survivor
           (h) run the uninstall a SECOND time to prove idempotence
SNAPSHOT=  P37-PRE-STAGE1 {e94d539e-8046-443b-871c-9d6711c34fd2}
EXPECTED=  (f) exit 0. (g) ZERO product traces: no C:\Program Files\AetherCore,
           no C:\ProgramData\AetherCore, no AetherCoreMaintenance service, no
           AetherCore pipe, no ARP entry, no HKLM\SOFTWARE\AetherCore, no
           HKCU\Software\AetherCore, no Start Menu folder, no scheduled task,
           no firewall rule. (h) the second uninstall does NOT fail on things
           already gone.
RECOVERY=  prlctl snapshot-switch "Windows 11" --id {e94d539e-8046-443b-871c-9d6711c34fd2}
```

## 16.6 GATE S2 — RESULT: **PASS** (2026-08-31)

Package `AetherCore-0.1.5-arm64.msi`, ProductCode
`{02F801D6-C117-CBB4-09A0-B51CB9E455C3}`, built exit 0 with **zero `ICE\d+`
matches** and `wix msi validate` exit 0.

### What the product actually leaves at rest — measured before authoring anything

- files: `C:\Program Files\AetherCore` (MSI-owned) and
  `C:\ProgramData\AetherCore` (written by the SERVICE at runtime)
- service `AetherCoreMaintenance` and its named pipe (the pipe exists only
  while the service runs)
- registry: `HKLM\SOFTWARE\AetherCore`, the ARP entry,
  `HKCU\Software\AetherCore`
- Start Menu folder
- **no scheduled tasks** — there is no `RegisterTaskDefinition` anywhere in the
  workspace; `crates/startup-manager`'s `ITaskService` use reads and toggles the
  USER's existing items. `crates/fleet`'s scheduler is in-process, persisted in
  the product database.
- **no firewall rules** — `crates/security-audit/src/firewall.rs` is a
  config-file-presence reader. It creates nothing.

Both negatives were then confirmed empirically on a bare box: `TASKS=0`,
`FIREWALL_RULES=0`.

### The first implementation FAILED this gate, and that is the useful part

`util:RemoveFolderEx` was the obvious declarative answer and it is wrong here.
It enumerates the target tree and injects `RemoveFile` rows **before
CostInitialize**, and Windows Installer then hard-fails at `InstallValidate` if
any file in that snapshot has since disappeared. The maintenance service is
still RUNNING at that point — `StopServices` is in the execute sequence, later —
and is still appending to `logs\service.jsonl`. The snapshot therefore races a
live writer. Raw observation from `s2C-uninstall.log`:

```
DEBUG: Error 2318:  File does not exist: C:\ProgramData\AetherCore\logs\service.jsonl
Action ended 18:45:44: InstallValidate. Return value 3.
Removal success or error status: 1603.
```

and the sweep after it found the ENTIRE product still installed: five payload
exes, the ARP entry, both registry keys, `aethercore.db`. Strictly worse than
the surviving ProgramData it was meant to fix. Case A had passed on an earlier
build purely on timing.

**Remedy:** a deferred custom action `PurgeMachineData`, scheduled
`After="DeleteServices"` under `REMOVE="ALL"`, running a new `purge-data` verb
on `aethercore-install-hardener.exe` — the binary that already holds deferred
LocalSystem authority in this package. Deleting after the service is gone is
the only ordering that cannot race. The action keeps that binary's posture: it
takes NO path from its command line, derives the directory from `%ProgramData%`
through the same trusted-path helpers as `apply`, and refuses a reparse point
anywhere in the tree rather than following it, so a planted junction cannot turn
an uninstall into a recursive delete somewhere else. Idempotent by
construction — an absent directory is success — with one 1.5 s retry for a
handle still closing behind the just-deleted service. This also removed the
`WixToolset.Util.wixext` dependency from the Product.wxs build again.

### The three cases, all on 0.1.5

| case | what was done first | uninstall exit | survivors |
|---|---|---|---|
| **A** normal | install, `scan start`, `perf start`, `perf snapshot`, plus a 5-level-deep file at `recovery\driver-backups\p37-depth-probe\level2\level3\backup-blob.bin` | **0** | **none** |
| **B** registry gone | `reg delete HKLM\SOFTWARE\AetherCore` before uninstalling | **0** | **none** |
| **C** pieces gone | service stopped AND deleted, `ProgramData\AetherCore` deleted, Start Menu folder deleted, `aetherctl.exe` and `UNINSTALL.txt` deleted, HKLM key deleted | **0** (was **1603**) | **none** |

The depth probe matters: it is a path no authored `RemoveFile` could name, at a
depth wildcards cannot reach. It is declared **synthetic** — a real driver
install was not performed on this VM — but the removal problem it stands for is
the real one.

### The sweep, run after every case — 14 checks, zero survivors each time

```
1  INSTALLDIR              False
2  PROGRAMDATA             False
3  SERVICE                 OpenService FAILED 1060: does not exist
4  PIPE_COUNT              0
5  ARP_COUNT               0
6  HKLM_SOFTWARE_AETHER    False
7  HKCU_SOFTWARE_AETHER    False
8  STARTMENU               False
9  SCHEDULED_TASKS         0
10 FIREWALL_RULES          0
11 HKLM_SERVICES_KEY       False
12 EVENTLOG_SOURCE         False
13 ALL-USERS HKCU MARKERS  (none in any hive under HKEY_USERS)
14 FILESYSTEM SWEEP        (no hit under Program Files, Program Files (x86),
                            ProgramData, or any user profile root/AppData)
```

### Idempotence — and the one honest asterisk

Uninstalling a machine whose service, data directory, Start Menu folder,
payload files and registry key were already gone exits **0**: case C. That is
the requirement, and it is met.

Running `msiexec /x <ProductCode>` a **second** time, against a product that is
no longer installed, returns **1605**. Recorded rather than argued away: 1605
is `ERROR_UNKNOWN_PRODUCT`, Windows Installer's answer to "that product is not
installed". It is not a cleanup failure and there is nothing left behind to
clean — the sweep after it is identical. Making it return 0 would mean
suppressing the OS's own not-installed signal, which would be worse.

### What uninstall keeps, and where the user is told

`release/UNINSTALL.txt` is installed beside the product and states in plain
language what goes and what stays; `ARPCOMMENTS` carries the summary into
Settings > Apps and Programs and Features, which is the uninstall UX the user
actually sees. Verified installed and readable from the ARP key:

```
UNINSTALL_TXT=True
KEY={02F801D6-...} VERSION=0.1.5
COMMENTS=Uninstall removes the program files, the AetherCore Maintenance
         service, and ALL machine data under C:\ProgramData\AetherCore ...
         Reports and diagnostic bundles you exported to a location YOU chose
         are not touched. See UNINSTALL.txt in the install folder.
```

The "kept" claim was checked, not assumed. A signed diagnostic bundle stays
verifiable after the signing key under `state\` is deleted:
`support_bundle::verify_archive(bytes, expected_public_key_fingerprint_sha256)`
takes the archive plus the fingerprint the user was shown at export, never the
local key file, and `crates/support-bundle` carries an explicit
`embedded_key_is_not_a_root_of_trust` test.

### Stated limit

The HKCU marker is per-user and Windows Installer runs the uninstall in one
account's context; it cannot enumerate other users' hives. On a machine where
several people used AetherCore, one integer value may remain in each of their
hives. Check 13 sweeps `HKEY_USERS` for exactly this and found none here.
UNINSTALL.txt states it rather than hiding it.

## 16.7 GATE S3 — RESULT: **PASS** (2026-08-31)

Run on the machine left bare by Gate S2, so "clean machine" is not a claim, it
is the previous gate's measured end state:

```
INSTALLDIR=False  PROGRAMDATA=False  ARP=0
SERVICE=OpenService FAILED 1060: does not exist
CLIDIR_BEFORE=False
```

### The artifact

`scripts/build-cli-archive.ps1 -Version 0.1.5 -Arch arm64` →
`aetherctl-0.1.5-windows-arm64.zip`, **1,892,681 B**, sha256
`05743e86040eb6293946e46fab5370d245ab9a8d75a6b515240a47ac95ce1d83`, recorded in
`out/cli/SHA256SUMS.txt`. Verified on the target: the published sum and the
on-disk sum are identical. (For scale: the full MSI is 1,099,640,832 B.)

### One documented command, and it is the whole install

```powershell
Expand-Archive aetherctl-0.1.5-windows-arm64.zip -DestinationPath $env:ProgramFiles\AetherCLI -Force
```

Five files land, nothing else happens — no service, no registry, no ARP entry,
nothing written outside the folder chosen:

```
 3,815,424  aetherctl.exe
     1,522  README.txt
     3,103  assets\vulndb\cis_map.json
     6,704  assets\vulndb\vulndb.json
       142  assets\vulndb\vulndb.manifest.json
```

### Real verbs return, and every exit code matches the documented table

| verb | exit | result |
|---|---|---|
| `--output json service detect` | **0** | `{"ok":true,"data":{"state":"Offline",…}}` — the README's first command |
| `--output json about` | 0 | product/platform/protocolVersion 7 |
| `--output json capabilities` | 0 | 16 native capabilities enumerated |
| `sec audit --profile cis-l1 --out … --format json` | **0** | a full scored `aethercore.compliance.v1` report **on a machine with no AetherCore installed** |
| `--output json sec audit --firewall` | 0 | `lane=cve status={"kind":"ok"}` |
| `--output json self-check` | **8** | `LocalIo / cli.selfCheck.modelsDirNotFound` |
| `--output json doctor` | **3** | `ServiceUnreachable` |
| `--help` | 0 | the usage block, on stdout |

Two of those are the interesting ones:

- **`sec audit --profile cis-l1` returning a scored report here** is the Stage 1
  `env!("CARGO_MANIFEST_DIR")` fix proven end to end. This machine is not the
  build machine and has no `assets/compliance` anywhere; the profile is inside
  the binary. And `lane=cve kind:"ok"` means the vulndb travelled in the archive
  and hash-verified beside the executable — the other Stage 1 fix, in the
  CLI-only shape.
- **`self-check` exiting 8 with `modelsDirNotFound` is a PASS, not a failure.**
  The 1.07 GB model belongs to the full product; a CLI-only install does not
  have it and says so with a typed error instead of pretending. The README
  states this before the user runs it.
- **`doctor` exiting 3 with `ServiceUnreachable`** is the honest answer for a
  service verb with no service: a typed envelope, immediately, not a hang.

Headless throughout: every command above ran through `prlctl exec` as SYSTEM in
session 0, with no interactive desktop session and no GUI.

### Findings recorded during this gate, NOT fixed here

- **`aetherctl about` reports `"version":"0.1.0"` from an archive built as
  0.1.5.** The CLI reports `CARGO_PKG_VERSION`, which is the workspace crate
  version and does not track the MSI/product version passed to the build. Anyone
  scripting a version check gets the wrong number. Recorded for a later stage.
- **`platform` is `"other"` on Windows** — second sighting, see §16.4. It
  appears in the `sec audit` JSON and in every compliance report's
  `host_fingerprint`.

## 16.8 DESTRUCTIVE ACTION RECORD — S2 RE-GATE after the Gate S4 finding

```
ACTION=    (a) build 0.1.6 carrying the fixed aetherctl (fleet state under
               %ProgramData%\AetherCore, schedule round-trip repaired)
           (b) DELETE the stale survivor left by the superseded build:
               C:\WINDOWS\system32\config\systemprofile\AppData\Roaming\aethercore
               (585 + 256 + 0 bytes; fleet inventory, schedules, empty
               known_hosts). It is orphaned state from a version that no longer
               exists and there is no installed product that reads it.
           (c) install 0.1.6, create fleet state, confirm it lands under
               %ProgramData%\AetherCore
           (d) uninstall, then re-run the HARDENED sweep (which now also walks
               the SYSTEM, SysWOW64, LocalService and NetworkService profiles)
           (e) reinstall so the VM is left with a working product
SNAPSHOT=  P37-PRE-STAGE1 {e94d539e-8046-443b-871c-9d6711c34fd2}
EXPECTED=  (c) C:\ProgramData\AetherCore\fleet\ holds inventory.json,
           schedules.json and trust\known_hosts, and
           %APPDATA%\aethercore does NOT reappear.
           (d) uninstall exit 0 and the hardened sweep reports ZERO hits under
           every profile root, not just under C:\Users.
RECOVERY=  prlctl snapshot-switch "Windows 11" --id {e94d539e-8046-443b-871c-9d6711c34fd2}
```

## 16.9 GATE S4 — RESULT: **PASS** (2026-08-31)

The deliverable is `docs/SERVER_READINESS.md`. It names what works, what does
not, and what remains unqualified, with evidence per claim. Summary of the
evidence gathered here:

**4a — Server SKUs.** The blocking finding, read out of the BUILT package's
`LaunchCondition` table rather than the source:
`VersionNT64 AND MsiNTProductType = 1 AND OSCURRENTBUILD >= 22621`.
`MsiNTProductType = 1` is **workstation only** (member server 3, domain
controller 2), so the installer refuses every Windows Server SKU before copying
a file. `Bundle.wxs:13` carries the same gate. Windows Server 2025 clears the
build floor (26100 ≥ 22621) and is still rejected on product type alone. Server
Core additionally cannot host `aethercore-desktop.exe` (WebView2 + a shell), and
the MSI installs that binary unconditionally with no feature to omit it. DISM,
`ITaskService` and `SHGetKnownFolderPath` are all present on Server, so those
are not client-only dependencies. **No Server SKU was executed against** — every
Server statement is derived, and is labelled as such.

**4b — Headless.** Proven for session 0: service `SESSION_ID=0`, clients
`nt authority\system` in session 0, and the whole-machine process list showing
exactly ONE AetherCore process (the service) with the desktop never started.
All of Gates S1/S2/S3 were produced this way. Stated limit: a console session
exists on this VM (nothing depended on it), so *zero-session* operation is not
proven, only *session-0-only* operation.

**4c — Fleet, more than one target.** Two host records driven for real:
`fleet add` ×2, `fleet list` (both, `trusted:false`), `fleet show`,
`fleet probe --host h1 --host h2` and `fleet audit --profile cis-l1` over both →
per-host `outcome:"not_verified"`, *"host is not authorized in the AetherCore
trust store"*. Scope resolution, per-host isolation and fail-closed trust are
proven; **no real SSH audit against a second machine is claimed** — none exists.
`ssh.exe` is present (`OpenSSH_for_Windows_9.5p2`), so the gap is a target, not
a capability. Hermetically, `crates/fleet` passes **59 tests** (47 unit + 12
integration) including `gd4_multi_host_orchestration_hermetic`.

**4d — Resource cost, measured at 1 Hz against the live service with the model
active (`engineLabel: localModel`):**

| phase | CPU over 60 s | working set avg/max | private commit | handles | threads |
|---|---|---|---|---|---|
| idle | 0.297 s = **0.50 % of one core** | 68.8 / 69.2 MB | 54.5 MB | 185 | 8 |
| scanning (205 facts, 61 findings) | 0.766 s = **1.28 % of one core** | 80.7 / 81.2 MB | 57.7 MB | 335 | 14 |

The number that matters for a server: `PeakWorkingSet64 = 1,132,965,888`
(**1.13 GB**) at model load, falling back to ~82 MB resident. Private commit
stays at 58 MB against a 5.7 GB address space, so the model is mapped rather
than committed and is reclaimable — but the transient happens at every service
start, i.e. every boot, and nothing lets an operator run the service without the
model. Disk: install dir 1,139,623,893 B; ProgramData after a scan 794,976 B.

### Three fleet defects found by running the surface, all fixed here

1. **The scheduler could not read back what it wrote.** `fleet schedule add`
   → exit 0; `fleet schedule run-due` → **exit 5,
   `fleet.schedulesInvalid: missing field \`schema\``**. The writer hand-built
   its JSON and dropped `schema`, which `FleetSchedule` requires under
   `deny_unknown_fields` with no default. Scheduled compliance — the point of
   the fleet surface — was broken end to end on a real install while 8
   `scheduler_runner` tests passed, because they build `FleetSchedule` directly
   and never traverse the CLI writer. Differential evidence, the persisted files
   verbatim:

   ```
   before: { "cadence":…, "enabled":true, "nextRunUnixMs":…, "profileId":"cis-l1",
             "scheduleId":"s1", "scope":["h1","h2"] }              <- no schema
   after:  { …, "scheduleId":"s1", "schema":"aethercore.fleet.schedule.v1", … }
   ```
   `run-due` then returned `{"ran":0,"runs":[]}` with **exit 0**.

2. **Fleet state was per-user and survived uninstall.** `%APPDATA%\aethercore`
   resolved to
   `C:\WINDOWS\system32\config\systemprofile\AppData\Roaming\aethercore` for the
   service account, holding `fleet\inventory.json` (585 B),
   `fleet\schedules.json` (256 B) and `fleet\trust\known_hosts`. After
   `msiexec /x` returned **0**, with `Program Files\AetherCore` and
   `ProgramData\AetherCore` both gone, **all three were still there**. Two
   defects in one: an administrator's fleet is invisible to a scheduled run
   under another account, and the SSH trust store outlives the product. Moved
   under `%ProgramData%\AetherCore`, inside what `purge-data` already removes.

3. **The whole fleet surface was missing from `aetherctl --help`**, so the
   server capability was undiscoverable from the tool. Added, with the
   fail-closed trust rule stated.

### This invalidated part of Gate S2, and that is recorded, not buried

Gate S2's sweep sampled `C:\Users\*` and its AppData roots but **not** the
SYSTEM, SysWOW64, LocalService or NetworkService profiles, which is exactly
where the surviving fleet state was. The sweep script has been hardened to walk
all of them, and Gate S2 is re-run against 0.1.6 in §16.10.

## 16.10 GATE S2 — RE-GATED: **PASS** (2026-08-31)

Package `AetherCore-0.1.6-arm64.msi`, ProductCode
`{863BF31B-B840-63F9-5B30-35528662C54F}`, exit 0, **zero `ICE\d+`**, validate
exit 0. It carries the aetherctl with the Gate S4 fixes.

The stale `%APPDATA%\aethercore` from the superseded build was deleted first
(§16.8 record). Then:

**Fleet state is machine state now, and the surface still works from there**

```
PROGRAMDATA_FLEET=True
   585  C:\ProgramData\AetherCore\fleet\inventory.json
   256  C:\ProgramData\AetherCore\fleet\schedules.json
     0  C:\ProgramData\AetherCore\fleet\trust\known_hosts
SYSTEM_APPDATA_AETHERCORE=False        <- does not reappear
fleet list        -> ok:true, both hosts
fleet schedule run-due -> ok:true, {"ran":0,"runs":[]}   exit 0
```

Full ProgramData tree before uninstalling — 8 files including the fleet trust
store and a live scan's WAL:

```
   585  fleet\inventory.json          4,096  state\aethercore.db
   256  fleet\schedules.json         32,768  state\aethercore.db-shm
     0  fleet\trust\known_hosts     671,592  state\aethercore.db-wal
     0  logs\service.jsonl                0  state\machine-mutation.lock
```

**Uninstall: exit 0. The HARDENED sweep — now walking the SYSTEM, SysWOW64,
LocalService and NetworkService profiles as well as `C:\Users` — reports ZERO
survivors on all 14 checks**, including check 14, which is the one that missed
the fleet state the first time.

### Final state of the machine

Reinstalled 0.1.6, exit 0. `C:\Program Files\AetherCore` holds **SIXTEEN** files
(the fifteen from §16.4 plus `UNINSTALL.txt`, 3,206 B). Service RUNNING, pipe
SDDL and install-dir ACLs identical to the §10 baseline, gguf sha256
`6a1a2eb6…9407e`.

`verbs-outer.ps1 -Label FINAL`: **18/18 RETURNED** across both token contexts,
`insights list` reports `engineLabel: "localModel"` in both, and `--help` now
prints the usage block to **stdout** from the MSI-installed binary — the Stage 3
fix proven on the shipped payload, not just on a dev build.

Snapshot `P37-SHIPPING-QUALIFIED {a1696567-7528-4136-a445-848dccd3d2c1}`.

### Snapshot ledger (Phase 37 additions)

| name | id | taken before |
|---|---|---|
| P37-PRE-STAGE1 | `{e94d539e-8046-443b-871c-9d6711c34fd2}` | the Stage 1 source refresh and rebuild |
| P37-SHIPPING-QUALIFIED | `{a1696567-7528-4136-a445-848dccd3d2c1}` | **the Phase 37 seal** |

### Package ledger (Phase 37)

| version | ProductCode | why it exists |
|---|---|---|
| 0.1.2 | `{2D97C23D-D2A1-83FE-3675-90F95D55540B}` | Gate S1: aetherctl + assets authored |
| 0.1.3 | `{76F8CD00-C012-423D-0890-DD8F311608F0}` | Stage 2 first attempt (`util:RemoveFolderEx`) — **superseded, do not ship** |
| 0.1.4 | `{ABF18F00-3B3F-601A-8ACE-E1F7F25077DF}` | RemoveFolderEx + registry fallback — **superseded, failed case C with 1603** |
| 0.1.5 | `{02F801D6-C117-CBB4-09A0-B51CB9E455C3}` | deferred `purge-data`; Gates S2 and S3 |
| **0.1.6** | `{863BF31B-B840-63F9-5B30-35528662C54F}` | **the current package**: adds the three Gate S4 fleet fixes |

---

# 17. PHASE 38 — MERGE AND CLOSE TWO RECORDED DEFECTS (started 2026-08-31)

Brief: merge two validated branches, fix `platform_tag()` returning `"other"`
on Windows, give the product version a single source of truth, then sweep for
every other instance of those two defect CLASSES and harvest the
"recorded, not fixed" backlog.

## 17.1 PROGRESS TABLE (append one row per closed item)

| # | Item | Status | Evidence |
|---|---|---|---|
| 1 | Merge `feat/desktop-qualification` + `feat/windows-server` into `main` | **PASS** | `e14b843`, `ca1eab5`, both `--no-ff`, zero conflicts, pushed |
| 2 | `platform_tag()` fixed at source + regression test | **PASS** | `66a0f5b`; test `platform_tag_names_the_host_and_never_falls_back_to_other`; negative control fails with `left: "other"` |
| 3 | Version single source of truth | **PASS** | §17.4, §17.8, §17.11 — 0.1.7 MSI built zero-ICE, installed, `about` == MSI == ARP == Cargo.toml |
| 4 | Step 4a platform-detection class sweep | **PASS** | §17.7 — one derivation, four label sites, `static_validate::platform_identity_single_source` |
| 5 | Step 4b version-derivation class sweep | **PASS** | §17.7 — six derived surfaces, zero independent deciders, `static_validate::version_single_source_of_truth` |
| 6 | Step 4c other single-truth values | **PASS (reported)** | §17.7 — pipe name not a defect; `engine_source` and ProgramData resolution recorded with reasons |
| 7 | Desktop app did not compile on macOS (icon.png not RGBA) | **FIXED** | §17.6, `4a4b5f0` — isolated as pre-existing, `cargo check -p aethercore-desktop` clean |
| 8 | Untranslated AR string introduced by the merge | **FIXED** | `4a4b5f0` — bisected to `ca1eab5`, gate `phase12_arabic_windows_update_localized` green |
| 9 | Windows SKU detection returned Unknown on every Windows host | **FIXED** | §17.12, `2450ffb` — verified on Windows: `platform=windows`, 16/16 native |
| 10 | Step 5 consolidated recorded-but-not-fixed harvest | **PASS** | §17.10 — 27 items, owner/hardware-gated separated |
| 11 | DBT-P36-001 / DBT-P36-002 closed | **PASS** | `477a052` — probes deleted, `pub mod probe` removed, ipc 8+8 green |
| 12 | Step 6 verification | **PASS** | §17.13 — parity 1554/1554, svelte 0/17, local-only proven to fail when violated |

## 17.2 CORRECTION TO THE BRIEF — branch contents

The brief describes `feat/desktop-qualification` as "documentation and evidence
only, no product code". **It is not.** `git log 2942aa0..origin/feat/desktop-qualification`:

```
dd4183c docs: keep screenshot claim precise
be43e0a docs: record interactive desktop qualification
e23377d feat(installer): admit Windows Server and omit Core desktop
63edc11 feat(server): make capability reporting SKU-aware
```

The two branches SHARE `63edc11` and `e23377d`; `feat/windows-server` adds only
`04d0bf4` on top. So merging desktop-qualification first brought in the Windows
Server product code, and the windows-server merge contributed one commit.
Recorded because the brief's characterisation would mislead a later reader.

## 17.3 platform_tag ROOT CAUSE (measured, not inferred)

`crates/security-audit/src/lib.rs:110` carried its OWN `cfg!` ladder:

```rust
if cfg!(target_os = "macos") { "macos" }
else if cfg!(target_os = "linux") { "linux" }
else { "other" }
```

Windows was never a branch, so it fell to `else`. Meanwhile the rest of the
product derived platform identity from `Platform::current()` in
`crates/platform-capabilities`. **Two independent derivations, and the copies
disagreed** — that is the class, not the instance.

Already fixed at the source by `63edc11` on the server branch, which routes
`platform_tag()` and three other call sites through
`aethercore_platform_capabilities::current_platform_name()`. It shipped with
**no test**; this session adds the regression test.

### Fingerprint impact — STATED, not hidden

`host_fingerprint` is built as `"{platform}:{digest}"`
(`apps/aetherctl/src/sec.rs:107`) and is covered by the report digest, which is
covered by the signature. A report generated before this fix and one generated
after, on the same machine in the same state, **will not compare equal**.

**Nothing in the product treats an older report's fingerprint as
authoritative.** `verify_compliance_report()` (`apps/aetherctl/src/sec.rs:160`)
recomputes the digest from the report's OWN bytes and verifies the signature
over that; it takes no external expected fingerprint. There is no baseline
store, no pinned fingerprint, no cross-report comparison — `host_fingerprint`'s
only non-test consumers are construction (`sec.rs:107`), storage
(`compliance.rs:403`) and HTML rendering (`compliance.rs:489,492`). Previously
issued reports remain verifiable and are NOT invalidated. What changes is that
an external diff of a pre-fix against a post-fix report shows a fingerprint and
digest difference that is not a configuration change.

## 17.4 VERSION — the mechanism and the reason

**Single source of truth: `[workspace.package].version` in
`phase21-workspace/Cargo.toml`.**

Reason, and why nothing new was invented: every crate already carries
`version.workspace = true`, so every binary already reported that value through
`CARGO_PKG_VERSION` (~20 call sites, `aetherctl about` among them), and
`scripts/build-release.ps1` — the production x64 pipeline — already derived from
it. It was the single source for everything *except the surfaces that ship*.
The fix was to stop three other surfaces deciding for themselves, not to add a
version file.

`scripts/Get-ProductVersion.ps1` is new and holds the derivation ONCE;
`build-release.ps1`'s private copy of the regex now calls it, so the change
removes a duplicate rather than adding three.

| surface | before | after |
|---|---|---|
| every Rust binary (`about`, service, desktop) | `CARGO_PKG_VERSION` | unchanged — already canonical |
| `scripts/build-arm64-msi.cmd` | arbitrary `%1`, default hard-coded `0.1.0` | derives from Cargo.toml; a disagreeing argument is a hard error |
| `apps/desktop/tauri.conf.json` | `"version": "0.1.0"` | field REMOVED; tauri-utils 2.9.3 documents that with it absent the Cargo.toml version is used (read from the pinned source, not memory) |
| `scripts/build-installer.ps1` | mandatory `-Version`, no cross-check | optional, derived, disagreement is an error |
| `scripts/build-cli-archive.ps1` | mandatory `-Version`, no cross-check | optional, derived, disagreement is an error |
| ARP entry | MSI `ProductVersion` | follows the MSI, so derives |

## 17.5 DESTRUCTIVE ACTION RECORD — sync 9 files to the VM and build 0.1.7

```
ACTION=    (a) prlctl snapshot "Windows 11" -n P38-PRE-VERSION-BUILD
           (b) copy the NINE files of commit 66a0f5b from
               \\Mac\dev\aethercore\phase21-workspace into
               C:\AetherCore-P36\workspace\AetherCore-Phase35-Master-Delivery
               (Cargo.lock, Cargo.toml, apps/desktop/tauri.conf.json,
                crates/security-audit/src/lib.rs, scripts/Get-ProductVersion.ps1,
                scripts/build-arm64-msi.cmd, scripts/build-cli-archive.ps1,
                scripts/build-installer.ps1, scripts/build-release.ps1)
           (c) run scripts\build-arm64-msi.cmd  (NO argument -- the version now
               comes from Cargo.toml) to produce AetherCore-0.1.7-arm64.msi
SNAPSHOT=  P38-PRE-VERSION-BUILD, taken by step (a).
           Fallback: P37-SHIPPING-QUALIFIED {a1696567-7528-4136-a445-848dccd3d2c1}.
EXPECTED=  (b) each copied file's SHA-256 in the guest equals the Mac's.
           (c) the script prints
               `Version=0.1.7  (derived from Cargo.toml [workspace.package].version)`;
               build exits 0; `wix msi validate` exits 0 with ZERO `ICE\d+`
               matches; the MSI is named AetherCore-0.1.7-arm64.msi and is
               roughly 1.1 GB.
           The VM probe confirms the guest source tree already carries BOTH
           merged branches' product code (Product.wxs Server admission,
           current_platform_name in security-audit), so these nine files are
           the whole delta between the guest tree and merged main.
           Nothing is installed or uninstalled by this step; the running
           service keeps its current binaries.
RECOVERY=  prlctl snapshot-switch "Windows 11" --id <P38-PRE-VERSION-BUILD id>
```

## 17.6 FINDING — the desktop app does not compile on macOS (PRE-EXISTING)

Discovered while checking that removing the `version` field from
tauri.conf.json was safe. `cargo check -p aethercore-desktop` on the Mac fails:

```
error: proc macro panicked
   --> apps/desktop/src/main.rs:2934:14
    |    .run(tauri::generate_context!());
    = help: message: icon .../apps/desktop/icons/icon.png is not RGBA
```

`file` reports `PNG image data, 512 x 512, 8-bit/color RGB` and `sips` reports
`hasAlpha: no`. Tauri's `generate_context!` requires RGBA.

**Isolated as PRE-EXISTING, not caused by this session:** restoring the
pre-change `tauri.conf.json` from `ca1eab5` and re-running produces the
identical error. It arrived with the icon set cherry-picked as `e8170dd`
(Phase 37 §16.1). Windows builds are unaffected — they use `icon.ico` — which
is why 0.1.2 through 0.1.6 all built exit 0 on the VM.

Consequence: the product documents Windows/macOS/Linux support and the desktop
app currently cannot be compiled on macOS or Linux.

## 17.7 STEP 4 — THE CLASS, NOT THE INSTANCE

### 4a — PLATFORM DETECTION: how many places decide?

**One derivation, four label sites.** They agree by construction.

| | site | role |
|---|---|---|
| derivation | `crates/platform-capabilities/src/lib.rs:196` `Platform::current()` | the only OS decision |
| derivation | same file `:375` `current_platform_name()` | the only wire LABEL, SKU-aware |
| label site | `crates/security-audit/src/lib.rs:111` `platform_tag()` | delegates |
| label site | `apps/aetherctl/src/offline.rs:158` `platform_str()` | delegates |
| label site | `services/maintenance-service/src/router.rs:1377` | delegates |
| label site | `services/maintenance-service/src/router.rs:1385` | delegates |

Before `63edc11`, `platform_tag()` had its own `cfg!` ladder — that is what
made it possible for the copies to disagree. Now every label routes through one
function, so agreement is structural rather than coincidental, and
`scripts/static_validate.py::platform_identity_single_source` fails the gate if
a second derivation reappears.

Every other `cfg!(target_os = …)` in the tree was inspected individually and is
a **behaviour branch, not an identity derivation** — legitimate and unavoidable:

| site | what it branches on | verdict |
|---|---|---|
| `security-audit/src/lib.rs:157` + `password.rs:60` | macOS keeps password policy in OpenDirectory, not `login.defs` | correct, honest NotAvailable |
| `security-audit/src/census.rs:274` | which package-manager lanes exist | correct in shape, **but see item 18** |
| `aetherctl/src/offline.rs:163` and `maintenance-service/src/performance.rs:317` `engine_source()` | native vs synthetic perf engine | duplicated — see 4c |

### 4b — VERSION DERIVATION: how many places decide?

Enumerated in full. **One canonical source, six derived surfaces, zero
independent deciders remaining.**

| surface | before | now |
|---|---|---|
| `[workspace.package].version` in `Cargo.toml` | canonical | **canonical** |
| ~20 Rust `env!("CARGO_PKG_VERSION")` sites (incl. `aetherctl about`) | derived | derived |
| `apps/desktop/tauri.conf.json` | **independent** `"0.1.0"` | field removed, inherits |
| `scripts/build-arm64-msi.cmd` | **independent** arg, default `0.1.0` | derives; disagreement is an error |
| `scripts/build-installer.ps1` | **independent** mandatory arg | derives via helper |
| `scripts/build-cli-archive.ps1` | **independent** mandatory arg | derives via helper |
| `scripts/build-release.ps1` | derived, but its own private regex | uses the shared helper |
| `package.json`, `apps/ui/package.json` | **independent** `"0.1.0"`, script-readable | field removed (both `private: true`) |
| MSI `ProductVersion`, ARP entry, `HKLM InstallVersion` | follow the MSI | follow the MSI |

Deliberately NOT unified, because they are different truths that only share the
word "version": `PROTOCOL_VERSION` (7), `REPORT_SCHEMA_VERSION`,
`UPDATE_CONTRACT_VERSION`, `REMOTE_CONTRACT_VERSION`, `PLANNER_VERSION`,
`RULE_ENGINE_VERSION`, `POLICY_VERSION`. Collapsing these into the product
version would be a defect, not a fix.

### 4c — OTHER VALUES THAT SHOULD BE ONE TRUTH

| value | finding | verdict |
|---|---|---|
| **pipe name** | `crates/ipc/src/lib.rs:9` `PIPE_NAME` is the single product constant. Two PowerShell files repeat the literal (`scripts/verify-ipc-pipe-security.ps1`, `scripts/p36vm/verify-install.ps1`). | **Not a defect.** Those are verification harnesses. A harness that read the expected value out of the code under test would assert nothing. Correct as-is. |
| **install path** | zero hard-coded `Program Files\AetherCore` literals in Rust. | Clean. |
| **`engine_source()`** | Implemented TWICE — `aetherctl/src/offline.rs:163` and `maintenance-service/src/performance.rs:317` — kept in sync by a comment ("Mirrors …(parity gate)") and a static check. | **Real instance of the class**, but NOT contained: the service copy has a `force-synthetic-perf` feature gate the CLI copy does not, so they are not actually identical and merging them naively would change behaviour. **RECORDED, not fixed.** (Both also have a dead `else` arm: `cfg!(windows) \|\| macos \|\| linux` is true on every supported target, so `"synthetic"` is unreachable there.) |
| **product data root** | `crates/windows-foundation/src/lib.rs:137` states the rule explicitly — *"Resolve the machine-wide data root from Windows rather than trusting an inherited environment variable"* — and uses `SHGetKnownFolderPath(FOLDERID_ProgramData)`. It is the ONLY site that follows it. `apps/install-hardener/src/main.rs:56` (`purge_data`, the deferred LocalSystem uninstall delete), `apps/aetherctl/src/fleet.rs:79`, `transport.rs:86/123`, `crates/security/src/lib.rs:487` and three `apps/desktop` sites all resolve it from `%ProgramData%` or an inherited root. | **RECORDED, not fixed.** Checked for a security consequence first and there is none: `purge_data` runs as LocalSystem under msiexec, where changing the machine `ProgramData` variable already requires admin, and it is guarded by `validate_absolute_no_parent` (absolute, disk-prefixed, no `..`) plus `reject_reparse_tree`. The real risk is **correctness**: on a machine with a relocated ProgramData, install and uninstall could disagree and the purge would miss the real directory. Aligning `purge_data` with the documented rule is a one-function change but it sits in the LocalSystem recursive-delete path, so it needs Gate S2 re-run to keep that evidence honest. Not "obviously safe", so it is recorded. |
| **product name** | the literal `"AetherCore"` appears at ~10 production sites (path joins, `release-authority` product-id checks). | Cosmetic. A shared constant would be tidier; no observed defect. Recorded only. |
| **icon assets** | `icon.png` fixed (§17.6). The other 16 PNGs under `apps/desktop/icons/` are also 8-bit RGB with no alpha. | Not required by `generate_context!`, and they are design-owned. **Recorded, not rewritten.** |

## 17.8 GATE — 0.1.7 BUILD RESULT: **PASS**

`scripts\build-arm64-msi.cmd` invoked with **NO argument**; the version came
from Cargo.toml. Build ran 23:27:16 -> 23:39:09 (~12 min), all six steps.

```
Version=0.1.7  (derived from Cargo.toml [workspace.package].version)
ProductCode={5DE146C7-DF44-4F60-7D38-552B4FC48736}
=== BUILD OK: C:\AetherCore-P36\build\out\AetherCore-0.1.7-arm64.msi
```

| property | value |
|---|---|
| MSI | `AetherCore-0.1.7-arm64.msi` |
| bytes | 1,099,649,024 |
| sha256 | `65d506834eb2196be640313a399acd834bfaf05f671f54e819fc7404f5c16a68` |
| ICE findings (`ICE\d+` over the whole log) | **0** |
| `wix msi validate` | exit 0 (step [6/6] ran; the script is `|| exit /b 1`, and BUILD OK followed) |

Read back out of the BUILT package's Property table, not from source:

```
MSI_ProductVersion=0.1.7
MSI_ProductCode={5DE146C7-DF44-4F60-7D38-552B4FC48736}
MSI_UpgradeCode={45598C77-2C32-5BCE-8510-19C7E51EE3B8}
MSI_ProductName=AetherCore
```

ProductCode determinism verified INDEPENDENTLY on the Mac by recomputing the
documented scheme — first 16 bytes of
`SHA256("AetherCore/MSI/ProductCode/v1" + "AetherCore/0.1.7/arm64")` read as a
.NET Guid — which yields `{5DE146C7-DF44-4F60-7D38-552B4FC48736}`, equal to the
value the build emitted. Same UpgradeCode as every prior package, so 0.1.7 is a
true major upgrade over the installed 0.1.6.

## 17.9 DESTRUCTIVE ACTION RECORD — install 0.1.7 over the installed 0.1.6

```
ACTION=    (a) prlctl snapshot "Windows 11" -n P38-PRE-0.1.7-INSTALL
           (b) msiexec /i C:\AetherCore-P36\build\out\AetherCore-0.1.7-arm64.msi /qn
               /l*v C:\AetherCore-P36\logs\p38-install.log
           (c) read the INSTALLED aetherctl's `about` version and compare it to
               the version the INSTALLER declares
SNAPSHOT=  P38-PRE-0.1.7-INSTALL, taken by step (a).
           Fallback: P37-SHIPPING-QUALIFIED {a1696567-7528-4136-a445-848dccd3d2c1}.
EXPECTED=  (b) exit 0. RemoveExistingProducts runs (new ProductCode, same
           UpgradeCode, MajorUpgrade Schedule="afterInstallInitialize").
           Then exactly ONE ARP entry, {5DE146C7-DF44-4F60-7D38-552B4FC48736}
           version 0.1.7, with the 0.1.6 key {863BF31B-...} gone;
           HKLM\SOFTWARE\AetherCore\InstallVersion = 0.1.7.
           INSTALLFOLDER holds SIXTEEN files. Service AetherCoreMaintenance
           LocalSystem AUTO_START(DELAYED) RUNNING, Service SID UNRESTRICTED,
           pipe SDDL and install-dir icacls IDENTICAL to the §10 baseline
           (Users RX, no write).
           (c) `aetherctl about` reports version 0.1.7 -- the SAME string the
           MSI declares in ProductVersion and the same one Cargo.toml carries.
           This is the whole point of the change: before it, an MSI built as
           0.1.6 installed an aetherctl that answered 0.1.0.
RECOVERY=  prlctl snapshot-switch "Windows 11" --id <P38-PRE-0.1.7-INSTALL id>
```

## 17.10 CONSOLIDATED "RECORDED, NOT FIXED" INVENTORY

Harvested end to end from `docs/phase36/DRIFT_LEDGER.md` (debt register +
tranche 3) and this file (§16.4, §16.6, §16.7, §16.9). Every item previous
sessions wrote down and justified, in one place, with what it would take to
close it and whether it blocks release.

Legend for **Owner**: `agent` = closable by an agent session; `owner` = needs a
product decision or artwork; `hw` = needs hardware or a machine that does not
exist here.

### A. Closed BY THIS SESSION (evidence in this file)

| # | Item | Where recorded | Evidence of closure |
|---|---|---|---|
| 1 | `aetherctl about` reports 0.1.0 from an archive built as 0.1.5 | §16.7 | §17.4, §17.11 — one source of truth; installed 0.1.7 reports 0.1.7 |
| 2 | `platform_tag()` returns `"other"` on Windows | §16.4, §16.7 | §17.3 — routed through one helper, regression test added |
| 3 | `verbs-inner.ps1` records `EXIT_CODE=` (empty) | §16.4 | commit `1c56080` — root cause measured, `$p.Handle` cached |
| 4 | DBT-P36-003 fleet test hard-codes a VM-only ssh stub path | DRIFT_LEDGER debt register | commit `1c56080` — stub created in-test; 59/59 fleet tests pass |

### B. Already satisfied by LATER work — closable with evidence, no code needed

| # | Item | Where recorded | Why it is now closed |
|---|---|---|---|
| 5 | `aetherctl.exe` is not authored in `Product.wxs`; a bare box gets seven files and no CLI | tranche 3 #1 | Closed by Phase 37 Gate S1 (§16.4): aetherctl is an MSI component; a clean install lands 15 files, 16 with UNINSTALL.txt. Re-confirmed on the 0.1.7 install: `FILE_COUNT=16` |
| 6 | Parallels host volume at 100%, ~6.8 GiB free, blocking snapshots | tranche 3 #8, §13 | Closed by Phase 37 Gate 0: 91.97 GB reclaimed. This session took two fresh snapshots successfully |
| 7 | `aetherctl --help` and `-h` are unknown commands, exit non-zero | §16.4 | Closed in Gate S3/S2-re-gate (§16.7, §16.10): `--help` prints usage to **stdout** from the MSI-installed binary |
| 8 | `aethercore-desktop.exe` is not byte-reproducible across rebuilds | tranche 3 #7 | Not a defect. Byte reproducibility is explicitly not a criterion and `RELEASE-METADATA.json` already sets `msi_byte_reproducible_claim = false`. Close as by-design |
| 9 | A second `msiexec /x` of an absent product returns 1605 | §16.6 | Not a defect. 1605 is `ERROR_UNKNOWN_PRODUCT`; suppressing it would hide the OS's own not-installed signal. Close as by-design |

### C. Open, closable by an agent — small, self-contained, no decision needed

| # | Item | Where recorded | What it takes | Blocks release? | Owner |
|---|---|---|---|---|---|
| 10 | DBT-P36-001 / DBT-P36-002: six diagnostic probes under `tools/p36-probes/`, and `crates/ipc/src/lib.rs` widened its frame codec `pub(crate)` -> `pub` plus `pub mod probe` purely to serve them. A real public-API surface increase in a product crate. | debt register | Named-pipe qualification IS now sealed (Phase 36 Stage E, Phase 37 S1-S4 all PASS), which is the stated trigger. Delete the probes and revert the codec to `pub(crate)`; verify the workspace still builds. | No — but it is unnecessary shipped API surface | agent |
| 11 | `tauri.conf.json`'s `beforeBuildCommand` resolves `../ui` from the CLI's discovered app dir, not the config dir, so it fails ENOENT. Worked around on ARM64 with a config overlay; **the x64 pipeline calls the same hook** and is expected to hit it. | tranche 3 #6 | Make the hook path robust or drop it (step [1] already builds the frontend). Needs an x64 run to confirm. | Would block an x64 release build | agent |
| 12 | Same-version reinstall needs `REINSTALLMODE=vamus`, not `amus` (a rebuilt package has a new PackageCode -> 1638). | tranche 3 #2 | Pin it in the recovery runbook. Documentation only. | No | agent |

### D. Open, needs a PRODUCT DECISION — do not attempt without the owner

| # | Item | Where recorded | The decision required |
|---|---|---|---|
| 13 | DBT-P36-004: the icon set is `COMPILE_ONLY_PLACEHOLDER=True`, AI-generated, not the product mark. | debt register | Owner must supply real artwork before release packaging. (This session fixed the *technical* defect that the placeholder lacked an alpha channel — see §17.6 — but that does not make it the brand.) |
| 14 | DBT-P36-005: maintenance-service module un-gating compiles router/protocol/streaming/performance/support and the broker trust gates into the Windows service binary for the first time. Marked SECURITY-SENSITIVE. | debt register | A review before release packaging. Explicitly "do not expand". |
| 15 | DBT-P36-006: `security-audit/filesystem.rs` `mode_bits()` Windows fallback is a POSIX-style mapping, **not** Windows ACL evidence. | debt register | Native ACL qualification. This is a security-evidence gap: the filesystem lane's Windows findings rest on a POSIX approximation. Not contained; needs a designed approach. |
| 16 | The service peaks at `PeakWorkingSet64 = 1,132,965,888` (1.13 GB) at model load on **every** start, i.e. every boot, and nothing lets an operator run the service without the model. | §16.9 4d | Whether a model-less service mode should exist for servers. |
| 17 | The HKCU marker is per-user; on a multi-user machine one integer value may remain in each other user's hive after uninstall. | §16.6 | Whether that is acceptable (it is stated in UNINSTALL.txt) or needs an ActiveSetup-style sweep. |
| 18 | `census()` has no Windows lane at all: on Windows it emits a single `PlatformAbsent` lane **labelled `CensusSource::MacosPkgutil`** with detail "unsupported host OS". Found this session, `crates/security-audit/src/census.rs:274`. | §17.7 (new) | What a Windows package census should be (winget? MSI product table? none?). The mislabelling is a bug either way, but the fix depends on the answer. |
| 19 | DBT-P36-007: the P35 full-tree ledger is permanently stale (91 files). | debt register | Re-baseline at the next seal, deliberately not before. |

### E. Open, needs HARDWARE or a machine that does not exist here

| # | Item | Where recorded | What is missing |
|---|---|---|---|
| 20 | **No Windows Server SKU was ever executed against.** Every Server statement in `SERVER_READINESS.md` is derived from the built package's `LaunchCondition` table and the source, and is labelled as such. | §16.9 4a | A Windows Server 2019/2022/2025 machine. This is the single largest evidence gap in the Server admission that was merged this session. |
| 21 | No real SSH audit against a second machine. Scope resolution, per-host isolation and fail-closed trust are proven; a real remote audit is not. | §16.9 4c | A second machine to target. `ssh.exe` is present, so the gap is a target, not a capability. |
| 22 | Zero-session operation is not proven, only session-0-only operation — a console session exists on this VM. | §16.9 4b | A machine with no interactive session at all. |
| 23 | Physical x86_64 qualification; Authenticode certificate; production update endpoint; production key/HSM; dependency freeze from a trusted workstation. | §6 | Hardware, certificates and an owner-run trusted workstation. |
| 24 | The only `libomp140.aarch64.dll` on this VM comes from the VS redist **`debug_nonredist`** tree. A production ARM64 package must source the redistributable runtime. | tranche 3 #9 | The redistributable OpenMP runtime for ARM64. |

### F. Recorded permanently — inherent behaviour, not defects to fix

| # | Item | Where recorded | Why it stays recorded |
|---|---|---|---|
| 25 | Killing the installer engine mid-`FileCopy` runs no rollback and leaves orphaned files. | tranche 3 #3 | Inherent: the rollback executor is the process killed. The runbook answer is "run the installer again", which does clean it. |
| 26 | `C:\Windows\Installer\MSICD74.tmp` from that hard kill is never removed by any later transaction. | tranche 3 #4 | Cleaning `C:\Windows\Installer` by hand is outside authorization. |
| 27 | A plain `msiexec /i` onto a box holding the C1 orphans failed 1603 / Error 1920 where the identical command passed on a clean box. **Cause not diagnosed**, per the stop rule. | tranche 3 #5 | The observation is the deliverable. Still undiagnosed. |

## 17.11 GATE — VERSION SINGLE SOURCE PROVEN END TO END: **PASS**

`msiexec /i AetherCore-0.1.7-arm64.msi /qn`, then read the machine:

| check | expected | observed | result |
|---|---|---|---|
| ARP entries named AetherCore | exactly 1 | `ARP_COUNT=1` | PASS |
| ARP key | `{5DE146C7-DF44-4F60-7D38-552B4FC48736}` | same | PASS |
| ARP version | 0.1.7 | `0.1.7` | PASS |
| `HKLM\SOFTWARE\AetherCore\InstallVersion` | 0.1.7 | `0.1.7` | PASS |
| INSTALLFOLDER file count | 16 | `FILE_COUNT=16` | PASS |
| `sc qc` | TYPE 10, AUTO_START (DELAYED), ERROR_CONTROL 1, LocalSystem | identical | PASS |
| `sc query` | STATE 4 RUNNING | `RUNNING` | PASS |
| `sc qsidtype` | UNRESTRICTED | `UNRESTRICTED` | PASS |
| pipe SDDL | identical to §10 | `O:S-1-5-80-4285065559-…-1187574229G:SYD:P(A;;0x12008b;;;AU)(A;;FA;;;S-1-5-80-…)` | PASS |
| install-dir `icacls` | identical to §10, Users RX no write | identical | PASS |

**THE PROOF the brief asked for:**

```
MSI declares (Property table):     ProductVersion = 0.1.7
ARP shows:                          0.1.7
HKLM InstallVersion:                0.1.7
Cargo.toml [workspace.package]:     0.1.7
installed `aetherctl about`:        {"version":"0.1.7", ...}
CTL_SHA256 = 8c6e7c342a7f5db85865be5e070f3c03743ae78967e24991199754ab80067d82
```

`CTL_SHA256` equals the `aetherctl.exe` hash in the build manifest, so the
binary answering is the one this MSI installed, not a leftover. **MATCH.**
Before this change an MSI built as 0.1.6 installed an aetherctl that answered
`about` with 0.1.0.

## 17.12 REGRESSION FOUND BY READING THE INSTALL — Windows SKU detection

The same `about` output carried a deviation from expected:

```
observed: "platform":"windowsUnknownSku"
expected: "platform":"windows"
```

Root cause, measured on the VM (see commit `2450ffb`): `current_windows_sku()`
read a `ProductType` **DWORD** from `SOFTWARE\Microsoft\Windows NT\CurrentVersion`.
That value is **ABSENT** on Windows; the numeric product type lives in
`SYSTEM\CurrentControlSet\Control\ProductOptions\ProductType` as a REG_SZ
(`WinNT` here). The failed read hit an early `return WindowsSku::Unknown`
before the `InstallationType` read — which was correct and would have answered
`Client`. So SKU detection returned **Unknown on every Windows host**, and the
SKU-aware capability table shipped with Windows Server admission (`63edc11`)
never selected the workstation table on a workstation.

**Checked first, and stated plainly: this is NOT a security regression.**
`available_on_windows_sku` maps `Unknown` to `windows_server_table(false)` with
the note "Unknown Windows must not be overstated as a workstation" — it fails
CLOSED. Nothing was reported available that should not be, and every ACL, the
pipe DACL, the service configuration and the Service SID on the 0.1.7 install
match the §10 baseline exactly.

What it DID do, measured by running both binaries side by side on the same box:

| | native | degraded | notAvailable |
|---|---|---|---|
| installed 0.1.7 (SKU=Unknown -> server table) | 11 | 2 | 3 |
| rebuilt with the fix (SKU=Workstation) | **16** | 0 | 0 |

Five of sixteen capabilities were mis-reported on a workstation, each carrying
a literal **Windows Server** reason key that a user would see:

```
thermalPowerClamp  notAvailable(cap.reason.windowsServerNoThermalPower)  -> native
systemRepairWua    degraded    (cap.note.windowsServerWsusPolicy)        -> native
gameModeProfile    notAvailable(cap.reason.windowsServerNoGameMode)      -> native
restorePoints      notAvailable(cap.reason.windowsServerNoRestorePoints) -> native
windowsUpdate      degraded    (cap.note.windowsServerWsusPolicy)        -> native
```

A system-maintenance product telling a Windows 11 workstation that restore
points and Windows Update are unavailable is a serious functional misreport,
even though it errs safe.

Verified on the VM after the fix, compiled for Windows (macOS cannot compile
the `cfg(windows)` branch):

```
cargo test -p aethercore-platform-capabilities   TESTEXIT=0  (7 passed on Windows)
cargo build --release -p aetherctl               BUILDEXIT=0
about     -> {"platform":"windows", "version":"0.1.7", ...}
sec audit -> platform = windows        <- this is what feeds host_fingerprint
capabilities -> 16 native, 0 degraded, 0 notAvailable
```

**Why the previous session's "no regression" claim missed it:** the Server
branch gate asserted that 18/18 verbs RETURNED and that files/ACLs matched. It
did not compare capability STATES, and `about`'s platform field was not an
assertion. Returning is not the same as returning the right answer.

## 17.13 STEP 6 — VERIFICATION THAT THE WHOLE THING STILL HOLDS

Verification only; no behaviour was changed to make any of these pass.

### EN/AR catalogue parity — PASS

```
en=1554  ar=1554  delta=0
```

Measured with the key extractor already in the repo. Note this session also
FIXED a catalogue defect the merge introduced: `cap.note.windowsServerWsusPolicy`
carried the literal English "Windows Update" where the catalogue uses the
localized "تحديث Windows" in 22 other places. It was caught by the existing
`phase12_arabic_windows_update_localized` gate, which this session found newly
failing and bisected to the merge commit `ca1eab5` — not to the version work.

### Svelte checks at the standing bar — PASS

```
COMPLETED 217 FILES  0 ERRORS  17 WARNINGS  3 FILES_WITH_PROBLEMS
```

Bar: zero errors, warnings not above 17. Observed exactly 0 and 17 — at the
ceiling, not under it. All 17 are `css_unused_selector`.

### Local-only enforcement — PASS, **and proven to still fail when violated**

The brief requires proof that the guard still bites, not merely that it is
green. `scripts/static_validate.py::phase15_http_authority_is_desktop_only`
asserts that `reqwest` is a dependency of `aethercore-update-download` and of
NOTHING else — specifically not of `update-engine` and not of the maintenance
service — and that only the desktop app pulls the downloader in.

Negative control, run end to end:

| step | `phase15_http_authority_is_desktop_only` |
|---|---|
| clean tree | **True** |
| after injecting `reqwest = { version = "0.12", ... }` into `services/maintenance-service/Cargo.toml` | **False** |
| after reverting | **True** |

So a network dependency entering an offline crate does fail the gate today.

### Static validation, whole gate, against the pre-merge baseline — PASS

Compared against a throwaway worktree at `2942aa0` (pre-merge `main`):

| | checks | failing |
|---|---|---|
| baseline `2942aa0` | 342 | 22 |
| this session's HEAD | 344 | 21 |

```
NEWLY FAILING: NONE
NEWLY FIXED:   phase8_msi_upgrade_and_os_gate   (from the Windows Server merge)
```

The two added checks are this session's single-source gates, both passing.
The 21 remaining failures are pre-existing and predate the merge; they are NOT
attributed to this session and were not touched.

### Crate suites run for the code this session changed

| crate | result |
|---|---|
| `aethercore-security-audit` (lib) | new `platform_tag` test passes; negative control with the old ladder FAILS with `left: "other"` |
| `aethercore-platform-capabilities` | macOS: 11 passed. **On Windows, on the VM: `TESTEXIT=0`, 7 passed** — the `cfg(windows)` branch actually compiled and ran |
| `aethercore-fleet` | 47 unit + 12 integration = **59 passed, 0 failed**, matching the count recorded in §16.9 |
| `apps/ui` | `pnpm install --frozen-lockfile` passes; `pnpm build` succeeds in 654 ms after the package.json version removal |

### Workspace test suite — one failure, characterised

`cargo test --workspace` completed. One test failed:

```
---- t5_budget_constants_respected_on_load_and_call stdout ----
panicked at crates/intelligence-core/tests/adversarial.rs:450:5:
load+infer must respect the hard time budget
test result: FAILED. 14 passed; 1 failed  (finished in 114.95s)
```

**Not a regression, and not caused by anything this session changed.** The
assertion is pure wall clock —

```rust
assert!(started.elapsed() < INFERENCE_TIMEOUT, "load+infer must respect the hard time budget");
```

— covering a 1.07 GB model load plus an inference call, against
`INFERENCE_TIMEOUT = Duration::from_secs(10)`. It was run while the host was
simultaneously compressing a 1.1 GB CAB inside the Parallels VM **and** running
the whole workspace suite in parallel.

Re-run in isolation on the same host, same commit:

```
test t5_budget_constants_respected_on_load_and_call ... ok
test result: ok. 1 passed; 0 failed  (finished in 1.04s)
```

**1.04 s against a 10 s budget — a ~10x margin.** The failure is load-induced.

Recorded as a real finding about the TEST rather than the product: a wall-clock
budget assertion that shares a machine with a parallel test suite is flaky by
construction. It will fail on any loaded CI runner. Not fixed here — changing a
declared timing constant or the test's contract is a product decision, and this
session had no authorization to relax a budget.

**Caveat stated plainly:** the run was captured with `| tail -200`, so only the
final suite's counts are visible in the saved output. The whole-workspace
pass/fail totals are therefore NOT reported here; what IS established is that
exactly one test failed and that it passes in isolation. The per-crate suites
for everything this session touched were run individually and are reported
above with their own counts.

## 17.14 GATE — 0.1.8 BUILD (carries the SKU fix): **PASS**

Rebuilt after the Windows SKU fix so the shipped package carries it. Again
invoked with NO argument; version from Cargo.toml.

| property | value |
|---|---|
| MSI | `AetherCore-0.1.8-arm64.msi` |
| bytes | 1,099,649,024 |
| ProductCode | `{92E437E7-2C87-3C15-0A72-FF63C739EBE6}` |
| ICE findings (`ICE\d+` over the whole log) | **0** |
| `wix msi validate` | ran as step [6/6]; `BUILD OK` followed, and the script is `|| exit /b 1` |

ProductCode again verified INDEPENDENTLY on the Mac against the documented
scheme for `AetherCore/0.1.8/arm64` -> `{92E437E7-2C87-3C15-0A72-FF63C739EBE6}`,
equal to what the build emitted. Same UpgradeCode, so 0.1.8 is a true major
upgrade over 0.1.7.

### DESTRUCTIVE ACTION RECORD — install 0.1.8 over 0.1.7

```
ACTION=    msiexec /i C:\AetherCore-P36\build\out\AetherCore-0.1.8-arm64.msi /qn
           /l*v C:\AetherCore-P36\logs\p38-install8.log
SNAPSHOT=  P38-PRE-0.1.7-INSTALL {a226b395-81b7-4887-90e2-6f132e6551a3}.
           No fresher snapshot was taken deliberately: the host is at 97%
           capacity with 37 GB free and each snapshot costs ~4.6 GB of .mem
           plus its delta. {a226b395} captures the machine with 0.1.6 installed
           and is a complete recovery point for this action; the only state it
           does not hold is the 0.1.7 install, which is itself reproducible
           from an MSI that is still on disk.
           Fallback: P37-SHIPPING-QUALIFIED {a1696567-7528-4136-a445-848dccd3d2c1}.
EXPECTED=  exit 0; exactly ONE ARP entry {92E437E7-...} version 0.1.8; HKLM
           InstallVersion 0.1.8; SIXTEEN files; service RUNNING LocalSystem
           AUTO_START(DELAYED); SID UNRESTRICTED; pipe SDDL and install-dir
           icacls IDENTICAL to the §10 baseline; and the installed
           `aetherctl about` reporting BOTH version 0.1.8 AND
           platform "windows" (not "other", not "windowsUnknownSku").
RECOVERY=  prlctl snapshot-switch "Windows 11" --id {a226b395-81b7-4887-90e2-6f132e6551a3}
```

## 17.15 GATE — 0.1.8 INSTALLED: **PASS**, both defects closed on a real machine

`msiexec /i AetherCore-0.1.8-arm64.msi /qn` -> **EXIT=0**.

| check | expected | observed | result |
|---|---|---|---|
| msiexec exit | 0 | `EXIT=0` | PASS |
| ARP entries | exactly 1 | `ARP_COUNT=1` | PASS |
| ARP key / version | `{92E437E7-…}` / 0.1.8 | same | PASS |
| `HKLM\SOFTWARE\AetherCore\InstallVersion` | 0.1.8 | `0.1.8` | PASS |
| INSTALLFOLDER files | 16 | `FILE_COUNT=16` | PASS |
| `sc qc` | TYPE 10, AUTO_START (DELAYED), ERROR_CONTROL 1 NORMAL, LocalSystem | identical | PASS |
| `sc query` | STATE 4 RUNNING | RUNNING | PASS |
| `sc qsidtype` | UNRESTRICTED | UNRESTRICTED | PASS |
| pipe SDDL | identical to §10 | identical | PASS |
| install-dir `icacls` | identical to §10, Users RX no write | identical | PASS |

**Both defects, closed and observable on the installed product:**

```
{"schema":"aethercore.aetherctl.v1","command":"about","ok":true,
 "data":{"name":"aetherctl","platform":"windows","product":"AetherCore",
         "protocolVersion":7,"version":"0.1.8"}}
```

| | before this session | now |
|---|---|---|
| `version` | `0.1.0` from an MSI built as 0.1.6 | **`0.1.8`** = MSI ProductVersion = ARP = HKLM = Cargo.toml |
| `platform` | `other` (§16.4/§16.7), then `windowsUnknownSku` (§17.12) | **`windows`** |

`CTL_SHA256=f70e820f90f2bb2b569002d9a220b0955d5ff3c6c5ad88e2ccacd522406f6acf`
is the binary the MSI placed, and is the same file staged for the client gate.

### Snapshot ledger (Phase 38 additions)

| name | id | taken before |
|---|---|---|
| P38-PRE-VERSION-BUILD | `{0037f31a-48c3-4809-bc6d-e78b9036b96d}` | syncing commit 66a0f5b and building 0.1.7 |
| P38-PRE-0.1.7-INSTALL | `{a226b395-81b7-4887-90e2-6f132e6551a3}` | installing 0.1.7 over 0.1.6 — **the Phase 38 recovery point** |

### Package ledger (Phase 38 additions)

| version | ProductCode | why it exists |
|---|---|---|
| 0.1.7 | `{5DE146C7-DF44-4F60-7D38-552B4FC48736}` | proves the version single source of truth end to end |
| **0.1.8** | `{92E437E7-2C87-3C15-0A72-FF63C739EBE6}` | **the current package**: adds the Windows SKU detection fix |

## 17.16 GATE — 18-VERB CLIENT GATE ON 0.1.8: **PASS**

Run because step 3 changed how the payload is produced (the build script no
longer takes a version) and because the build exposed the SKU defect — both of
the brief's stated triggers. It also exercises this session's `verbs-inner.ps1`
fix on the real gate rather than on a synthetic probe.

`verbs-outer.ps1 -Label P38FINAL`, both actual-token contexts via the proven
one-shot Scheduled Task method.

| | STD | ADMIN |
|---|---|---|
| `IS_ELEVATED_ADMIN` | **False** | **True** |
| verbs run | 9 | 9 |
| `RESULT=RETURNED` | **9** | **9** |
| TIMED_OUT / LAUNCH_FAILED | 0 / 0 | 0 / 0 |
| `EXIT_CODE` populated | **9** | **9** |
| `EXIT_CODE` empty | **0** | **0** |

**18/18 RETURNED.** Both transcripts record
`CTL_SHA256=f70e820f90f2bb2b569002d9a220b0955d5ff3c6c5ad88e2ccacd522406f6acf`,
which is the binary this MSI installed — so the gate exercised the shipped
payload, not a leftover.

`insights list` reports **`"engineLabel":"localModel"` in BOTH contexts**: the
1.07 GB embedded model loaded and verified at service start.

### The harness fix, proven on the real gate

Every prior transcript in this project recorded `EXIT_CODE=` (empty) — §16.4
recorded it as a harness gap and §16.7 noted the gate "asserts RETURNED, not
what was returned". This run records an exit code for all 18, and the numbers
are informative rather than uniform:

```
servicedetect   0        helpflag      0        selfcheck     0
doctor          5        help          0        insightslist  0
optimizestatus  0        scanstatus    0        secaudit      0
```

`doctor` exiting **5** is a **PASS**, not a failure, and it is the documented
outcome — the Phase 36 A5 action record states verbatim that "doctor returning
`diagnostics.stateUnavailable` is a PASS". The transcript confirms exactly that,
as a typed refusal in 30 ms:

```
aetherctl: rejected by service (diagnostics.stateUnavailable):
           diagnostic state is unavailable [RejectedByService]
```

This is the first run in the project's history where that exit code could be
seen at all. `selfcheck` returning **0** here (against the 8 recorded in §16.7)
is also correct and is the difference the fix makes visible: §16.7's 8 was a
CLI-only archive install with no model, whereas this is the full product with
the model present.

## 17.17 HANDOFF — READ THIS FIRST IF YOU ARE THE NEXT SESSION

### What state `main` is in

`main` is at the Phase 38 tip, everything pushed, working tree clean. It
contains the two merged branches plus this session's fixes. The workspace
version is **0.1.8** and that number now comes from exactly one place.

The VM has **0.1.8 installed and running**: service RUNNING, 16 files,
`engineLabel: localModel`, 18/18 verbs, ACLs and pipe DACL identical to the §10
baseline. Recovery point for Phase 38 is
**P38-PRE-0.1.7-INSTALL `{a226b395-81b7-4887-90e2-6f132e6551a3}`**;
P37-SHIPPING-QUALIFIED `{a1696567-…}` is still there as the older fallback.
The Phase 36 forbidden snapshots are long deleted and are not a concern.

### What is proven, and how

| claim | proof |
|---|---|
| The product version has one source | `[workspace.package].version`; MSI ProductVersion, ARP, HKLM InstallVersion, Cargo.toml and installed `aetherctl about` all read 0.1.8; a disagreeing build argument is a hard error; `static_validate::version_single_source_of_truth` |
| Platform identity has one derivation | four label sites route through `current_platform_name()`; `static_validate::platform_identity_single_source`; installed `about` reports `windows` |
| The package is sound | 0.1.7 and 0.1.8 both built exit 0 with **zero `ICE\d+`** and `wix msi validate` exit 0; ProductCodes match the deterministic scheme, recomputed independently |
| Nothing regressed | `static_validate` vs a pre-merge worktree at `2942aa0`: **NEWLY FAILING: NONE** |
| The offline guarantee still bites | injecting `reqwest` into the maintenance service flips `phase15_http_authority_is_desktop_only` to False |
| Security posture unchanged | pipe SDDL, install-dir icacls, service config and Service SID all identical to §10 on both installs; the six previously-denied operations were re-verified denied on the merged desktop-qualification branch |

### The one thing a reader should not misread

Windows Server admission is **merged and packaged, but has never been executed
against a Windows Server machine.** Every Server claim in
`docs/SERVER_READINESS.md` is derived from the built package's
`LaunchCondition` table and from source. This session made that gap *worse* to
ignore, not better: the SKU detection that Server admission depends on was
returning `Unknown` on every Windows host until §17.12 fixed it, and that fix
has been verified on a **workstation only**. The Server and Server Core
branches of `classify_windows_sku` are still unexercised on real hardware.

### What to do next, in priority order

1. **Owner-gated, blocks release** — real icon artwork (DBT-P36-004) and the
   security review of the maintenance-service un-gating (DBT-P36-005).
2. **Hardware-gated** — a Windows Server 2019/2022/2025 box. It would close
   item 20 in §17.10 and validate the SKU detection fix on the SKUs it was
   written for. This is the single highest-value missing piece.
3. **Agent-closable, listed in §17.10 C** — the `tauri.conf.json`
   `beforeBuildCommand` path defect (#11), which is expected to break the x64
   release pipeline, and the `vamus` runbook note (#12).
4. **Recorded, needs a decision** — §17.10 D, especially the `census()` Windows
   lane mislabelled `MacosPkgutil` (#18) and the `ProgramData` resolution
   inconsistency in §17.7, which needs Gate S2 re-run if changed.
5. **Do not chase** — §17.10 F. Those are inherent Windows Installer behaviours,
   already recorded with their reasons.

### Traps this session hit, so you do not

- `\\Mac\...` UNC paths **collapse to `\Mac\...`** when passed through
  zsh -> `prlctl exec` -> `powershell -Command`. Put paths inside a staged
  `.ps1` and invoke with `-File "\\\\Mac\\dev\\..."`. `cmd.exe /c` cannot take a
  UNC working directory at all.
- In a `.cmd`, `echo EXIT=%ERRORLEVEL%>"file"` is parsed as a **stdin
  redirect** when ERRORLEVEL is a digit, and silently writes nothing. Use
  `>"file" echo EXIT=%ERRORLEVEL%`. Two status files in this session were empty
  for exactly this reason.
- `Start-Process -PassThru` + `WaitForExit(ms)` never populates `ExitCode` on
  PowerShell 5.1. Cache `$p.Handle` before the process exits. A parameterless
  `WaitForExit()` does **not** fix it — that was tested and disproven.
- A wall-clock test budget (`intelligence-core` T5, 10 s) fails when the host is
  also compressing a 1.1 GB CAB. It passes in 1.04 s alone. Do not run the
  workspace suite against a busy host and then believe the result.
- The full MSI build is ~12 minutes and the 1.07 GB model's CAB compression
  dominates it; `wixnative` shows no output for most of that. It is not hung.

---

# 18. PHASE 39 — FIX THE LOCALSYSTEM DISCLOSURE (started 2026-09-01)

Brief: close the release blocker `SECURITY_REVIEW_UNGATING.md` found — the
maintenance service reads caller-named absolute paths as LocalSystem, and
`ExportJournal` honours a caller-supplied owner key. Branch
`fix/localsystem-disclosure`, cut from `main` at `db0dc49`.

## 18.1 WHAT WAS PROVEN BEFORE ANYTHING WAS FIXED

The attack tests were written first and committed WHILE FAILING (`08e9c84`),
so the failure is the evidence. They drive the REAL service binary over a real
socket through the full v7 handshake. Against the unfixed router:

```
run_security_audit_refuses_an_absolute_target_outside_the_calling_principal_scope
  status_code=0
  lane=secrets status=ok findings=1
  {"id":"SEC-SEC-001","severity":"critical","evidence":[{
     "fact":"AKIA****************",
     "sourceLocation":"/tmp/axt-p39-.../victim-home/.aws/credentials:1"}]}

run_security_audit_refuses_a_link_planted_inside_the_owner_root   status_code=0
  same credential, reached through a symlink planted inside the caller's root
run_security_audit_refuses_out_of_scope_paths_in_every_target_variant
  status_code=0 on sshdConfig
export_journal_refuses_a_foreign_owner_principal_key             status_code=0
```

The two legitimate-path tests passed before the fix and must keep passing.

Why a unix socket proves a Windows defect: `router::handle_request` is one
platform-neutral function and both hosts dispatch into it (stated in
`unix_composition.rs`'s own module docs). What UDS cannot reproduce is the
privilege GAP — there the peer is the same user as the service. Section 18.4
covers that on the installed product.

## 18.2 THE FIX (`9aa9ae4`)

- `PrincipalContext` carries the caller's own profile directory, read from the
  OS **for that token** — Windows `SHGetKnownFolderPath(FOLDERID_Profile,
  token)` while the service still holds the impersonated client token. Not
  `%USERPROFILE%`, not the registry, not the request. Unresolved means NO
  scope, never no restriction.
- `crates/security-audit/src/scope.rs` is the allowlist the proto always
  promised. Per named path: no `..`; absolute; contained component-wise (and
  case-insensitively on Windows) in a root the caller owns; no symlink/reparse
  point at or below that root. Links ABOVE a root are resolved, because
  `/var` -> `/private/var` on macOS and `C:\Users` can be redirected; that is
  safe because containment is decided on the RESOLVED form, so a link can never
  fake its way in.
- The bounded walkers skip reparse points instead of following them, so a
  junction found mid-scan cannot redirect a LocalSystem walk. Same posture as
  `aethercore-install-hardener`'s `purge-data`; not the same code, because that
  binary carries zero workspace dependencies on purpose and this copy also has
  to handle unix symlinks. Both are stated in comments.
- sudoers `#includedir` is confined to the directory of the file that named it.
  Its target comes from FILE CONTENT, which the router never vetted.
- `ExportJournal` refuses a non-empty `owner_principal_key` that is not the
  calling principal.
- `operations.proto` now states what the code does, including the refusal keys.

Nothing was widened. The pipe DACL, the module gating and every existing check
are untouched.

## 18.3 DESTRUCTIVE ACTION RECORD — build and install 0.1.9 on the VM

```
ACTION=    (a) prlctl snapshot "Windows 11" -n P39-PRE-FIX-BUILD
           (b) hash-compare the whole non-docs source tree in
               C:\AetherCore-P36\workspace\AetherCore-Phase35-Master-Delivery
               against \\Mac\dev\aethercore\phase21-workspace and copy every
               file that differs or is missing (this session's fix plus any
               drift earlier sessions left), then re-verify by hash
           (c) run scripts\build-arm64-msi.cmd with NO argument -> 0.1.9
           (d) msiexec /i AetherCore-0.1.9-arm64.msi /qn
           (e) run the attack probe over the REAL named pipe under an actual
               standard-user token, then the 18-verb client gate
SNAPSHOT=  P39-PRE-FIX-BUILD, taken by step (a).
           Fallback: P38-PRE-0.1.7-INSTALL {a226b395-81b7-4887-90e2-6f132e6551a3}.
           P37-SHIPPING-QUALIFIED {a1696567-7528-4136-a445-848dccd3d2c1} remains
           the older recovery point. P36-CLEAN-BASELINE and
           P36-PRE-NATIVE-MUTATION are long deleted and cannot be restored.
EXPECTED=  (b) every copied file's SHA-256 in the guest equals the Mac's.
           (c) the script prints Version=0.1.9 derived from Cargo.toml; build
               exit 0; `wix msi validate` exit 0 with ZERO `ICE\d+` matches;
               ProductCode {EE0AE741-51DE-F66B-2790-81B6EB90D02B}, which is the
               value the documented scheme predicts for AetherCore/0.1.9/arm64
               and which was recomputed independently on the Mac (the same
               script reproduces {92E437E7-...} for the installed 0.1.8).
           (d) exit 0; ONE ARP entry {EE0AE741-...} 0.1.9; HKLM InstallVersion
               0.1.9; SIXTEEN files; service AetherCoreMaintenance LocalSystem
               AUTO_START(DELAYED) RUNNING; Service SID UNRESTRICTED; pipe SDDL
               and install-dir icacls IDENTICAL to the section 10 baseline
               (Users RX, no write).
           (e) the probe reports STATUS:403 for both attacks and STATUS:0 for
               both legitimate calls; 18/18 verbs RETURNED across both
               actual-token contexts.
RECOVERY=  prlctl snapshot-switch "Windows 11" --id <P39-PRE-FIX-BUILD id>
```
## 18.4 THE SWEEP — every newly reachable handler, checked for the same shape

The class is: *the caller names a target, the service acts on it with LocalSystem
authority, and nothing checks that the target belongs to the caller.* The review
found two instances. This is the enumeration that says whether there are more.

Method, so it can be re-run rather than trusted: every `*Request` message in
`crates/contracts/proto/*.proto` was listed with its fields, and every
`request::Payload::` arm in `router.rs` was listed with the request fields it
reads and whether `principal_key` appears in that arm. That is the whole v7
surface, not a sample.

**Exactly one request in the entire contract carries a filesystem path**
(`RunSecurityAuditRequest.targets_json`) and **exactly one carries an owner
scope** (`ExportJournalRequest.owner_principal_key`). Every other request field
is an opaque id, a bounded count, a byte buffer or a bool. So the path-shaped
class has one instance and the owner-override class has one instance, and both
are fixed.

| # | Handler / site | Shape | Disposition |
|---|---|---|---|
| 1 | `RunSecurityAudit` targets | caller names absolute paths, read as LocalSystem | **FIXED** — owner-scoped allowlist |
| 2 | `ExportJournal.owner_principal_key` | caller names another owner | **FIXED** — refused |
| 3 | `sudoers` `#includedir` | path from FILE CONTENT the allowlist never vetted, expanded as LocalSystem | **FIXED** (found by this session, not by the review) — expansion confined to the directory of the file that named it |
| 4 | Every id-bearing handler (`plan_id`, `scan_id`, `intent_id`, `bundle_id`, `preview_id`, `upload_id`, `ticket_id`, `change_id`, `assessment_id`, `candidate_id`, `release_id`) | caller names an opaque id | **Not the defect.** Each passes `principal_key` into an owner-scoped accessor. Spot-checked at the accessor rather than at the call: `maintenance_executions_for_owner` / `support_journal_events_for_owner` / `repair_timeline_events_for_owner` filter `WHERE owner_principal_key=?` in SQL; `SupportBundles::read_chunk`/`ready_for_owner`/`discard` return `Ownership` when `record.owner != owner`; `driver_install::status` routes through `get_plan_for_owner` |
| 5 | `ListInsights` / `RequestInsight` / `DismissInsight` | machine-wide shared store, **no owner dimension at all** | **RECORDED — needs a design decision.** `EphemeralInsights` (`services/maintenance-service/src/intelligence.rs:30`) is one process-wide `Vec`; `dismiss(insight_id)` and `list()` take no principal. So any local user sees the insights another user's request produced, and can dismiss them. It is not the caller-named-scope class and it is not a filesystem read, but it IS a newly reachable surface with no owner check. Fixing it means deciding whether insights are per-principal or machine-wide state — a product decision, not a bug fix |
| 6 | `StopPerfSampling` | takes no scope, stops sampling globally | **RECORDED.** `StartPerfSampling` is owner-keyed but `stop_sampling()` is not, so one principal can stop another's sampling. Availability nuisance, no disclosure, no LocalSystem read |
| 7 | `performance_optimization::status(plan_id)` | an owner-less status accessor exists | **RECORDED as a latent hazard.** Currently unreachable: the `GetOptimizationStatus` arm returns `status: None` unconditionally and never calls it, and the compiler already reports it dead. If a future change wires the handler to it, instance #1's shape returns |
| 8 | `StartOptimization`, `CheckForUpdates`, `StageUpdate` | — | Typed refusals; nothing is read |
| 9 | `GetPlatformCapabilities`, `GetEngineSource`, `GetUpdateCheckDescriptor` | machine-wide, non-owner data, no caller-named target | Not the class |

## 18.5 VM RESULTS — THREE PACKAGES, TWO HONEST FAILURES, ONE PASS

Snapshot taken before any mutation: **P39-PRE-FIX-BUILD
`{7c10fb2b-dbdd-45c5-b8af-85ab9a0f342f}`**, per the record in 18.3.

The guest source tree was not assumed current. Every one of the 967 tracked
non-docs files was hash-compared and the differing ones copied, then ALL of them
re-verified: `SCANNED=967 SAME=948 CHANGED=14 MISSING=5 COPIED=19
POST_VERIFY_BAD=0`. The drift was real and predated this session — `crates/ipc`
(the DBT-P36-001/002 probe-API revert), `crates/intelligence-core` tests, the
icon set and `static_validate.py` were all stale in the guest. Syncing only this
session's delta would have built a package that was not `main` plus the fix.

| version | ProductCode | outcome |
|---|---|---|
| 0.1.9 | `{EE0AE741-51DE-F66B-2790-81B6EB90D02B}` | **built, installed, REJECTED** — empty scope for every principal |
| 0.1.10 | `{9878E6E1-3211-FA76-0D45-030EA6C16177}` | **built, installed, REJECTED** — same |
| **0.1.11** | `{98FCE2D5-44F0-A27C-A48B-8720FFE672F0}` | **the fix, proven** |

Each ProductCode was recomputed independently on the Mac from the documented
scheme before the build ran, and each build emitted exactly that value. The same
computation reproduces `{92E437E7-…}` for the already-installed 0.1.8, so the
derivation is checked against a known answer rather than asserted.

### The two failures, and what each one cost to find

Both were the SAME symptom — attacks refused, but the legitimate caller refused
too, so the guard was fail-closed on everything. Neither was found by reasoning;
both were found by running the attack on the installed product, which is exactly
why the brief required it.

**0.1.9.** `SHGetKnownFolderPath(FOLDERID_Profile, token)` returned nothing for
every principal. The service opens the impersonated client token with
`TOKEN_QUERY` only and that API also wants `TOKEN_IMPERSONATE`. Replaced with a
`ProfileList` lookup keyed by the SID already on the principal — no token rights
needed at all, and `HKLM\SOFTWARE` is administrator-writable only, so an
unprivileged caller cannot redirect its own scope.

**0.1.10.** Still empty. Four things were measured before anything was changed,
and three came back clean — which is what made the fourth findable:

| measured | result |
|---|---|
| installed service binary identity | SHA == the 0.1.10 payload SHA, process started at install time — the new code WAS running |
| `ProfileList` read as SYSTEM for the standard user's SID | `C:\Users\P36StandardUser`, all three `RRF_*` flag combinations OK |
| the same read as that user | `C:\Users\P36StandardUser`, OK |
| `canonicalize` of that path | `\\?\C:\Users\P36StandardUser`, OK |

The one remaining difference was that `principal_from_token` runs INSIDE the
impersonation window. `inspect_named_pipe_client` now fills `profile_dir` after
`RevertToSelf`, with the service's own authority. The SID string is decoded from
the bytes already on the principal by hand — documented layout, pure arithmetic,
no Win32 call — and unit-tested on macOS against the exact SIDs on this box.
`RRF_RT_REG_EXPAND_SZ` was dropped: the diagnostic showed plain `RRF_RT_REG_SZ`
already returns the expanded path.

A fail-closed refusal that cannot say WHY is undiagnosable in the field, so
`sec.ownerScopeUnresolved` now distinguishes "the OS named no root for this
principal" from "the named roots did not resolve".

### GATE — 0.1.11 BUILD: **PASS**

```
Version=0.1.11  (derived from Cargo.toml [workspace.package].version)
=== BUILD OK: C:\AetherCore-P36\build\out\AetherCore-0.1.11-arm64.msi
ICE_MATCHES=0          all six steps ran, [6/6] is `wix msi validate`
```
1,099,653,120 B, sha256 `1a6ea3f10a0a195c18907946ffe18651afccef29859738dbe2b9e02444ad75f9`.

### GATE — 0.1.11 INSTALLED: **PASS**, zero differing fields vs the section 10 baseline

`msiexec /i … /qn` -> `EXIT=0`.

| check | expected | observed |
|---|---|---|
| ARP entries | exactly 1 | `ARP_COUNT=1` |
| ARP key / version | `{98FCE2D5-…}` / 0.1.11 | same |
| `HKLM\SOFTWARE\AetherCore\InstallVersion` | 0.1.11 | `0.1.11` |
| INSTALLFOLDER files | 16 | `FILE_COUNT=16` |
| `sc qc` | TYPE 10, AUTO_START (DELAYED), ERROR_CONTROL 1 NORMAL, LocalSystem | identical |
| `sc query` | STATE 4 RUNNING | RUNNING |
| `sc qsidtype` | UNRESTRICTED | UNRESTRICTED |
| pipe SDDL | identical to section 10 | `O:S-1-5-80-4285065559-…-1187574229G:SYD:P(A;;0x12008b;;;AU)(A;;FA;;;S-1-5-80-…)` |
| install-dir `icacls` | identical, Users RX no write | identical |
| installed `about` | 0.1.11 / windows | `{"platform":"windows","version":"0.1.11",…}` |

Nothing was widened to achieve this: the pipe DACL, the install-dir ACLs, the
Service SID type and the module gating are byte-for-byte the values the Phase 36
baseline recorded.

### GATE — THE ATTACK, ON THE INSTALLED SERVICE, OVER THE REAL NAMED PIPE: **PASS**

`p39_pipe_attack.exe` against `AetherCore.Maintenance.v7`, service exe
`c17602cc57330cf062a3575dae29ad643220e00f4fcf6fe726399664a8ed522a`:

```
ATTACK_AUDIT_FOREIGN_PATH     STATUS:403 sec.targetOutsideOwnerScope
                              [target is outside the calling principal's own
                               scope: C:\Users\hasanalaaa]           BODY: (empty)
ATTACK_JOURNAL_FOREIGN_OWNER  STATUS:403 journal.ownerScopeForbidden BODY: (empty)
LEGIT_AUDIT_OWN_SCOPE         STATUS:0    cve:ok:0 | secrets:ok:1
LEGIT_JOURNAL_OWN_SCOPE       STATUS:0    records=0 signed=false
FAILURES=0
```

The request that returned `AKIA****************` and the victim's exact file path
before the fix now returns a typed refusal and no payload at all. The legitimate
lane genuinely RAN rather than being silently narrowed away — `secrets:ok`
carries a real finding.

### BLOCKED — the actual-token contexts, and why

**Raw observation.** From ~12:05 onward every Windows Scheduled Task on this VM
sits `State=Queued` and never runs, including a trivial `cmd.exe /c echo` task
registered as SYSTEM with `-AllowStartIfOnBatteries -DontStopIfGoingOnBatteries`.
`sc query Schedule` reports `STATE 4 RUNNING`. The guest reports
`Win32_Battery BatteryStatus=1 (discharging) EstimatedChargeRemaining=17`, and
the Mac host reports `Now drawing from 'Battery Power' … 17%; discharging`.
Tasks ran normally on this box at 11:44 and stopped some time before 12:05.

**Expected.** The one-shot Scheduled Task method runs the probe under
`P36StandardUser` (Limited) and `P36Admin` (Highest), as it did earlier today.

**A stale transcript nearly became a false PASS, and that is worth recording.**
`verbs-outer.ps1` copies `C:\Users\Public\p36\verbs-<ctx>.txt` to the Mac after
the task "finishes". With the task stuck Queued it copied the file left there by
the Phase 38 run and reported a clean 18/18. It was caught only because the
transcript carried `CTL_SHA256=f70e820f…`, the **0.1.8** aetherctl, while the
installed binary is `7847569b…`. Every stale file under `C:\Users\Public\p36`
and `p39` has been purged so this cannot recur silently, but the harness itself
still has the flaw: it does not fail when its task did not run.

**What IS established on real tokens.** The 0.1.9 and 0.1.10 probe runs executed
under genuine `P36StandardUser` (`IS_ELEVATED_ADMIN=False`) and `P36Admin`
(`True`) tokens before the battery drained, and in both contexts the pipe was
reachable and BOTH attacks were refused with 403. So "an unprivileged local user
is refused" is proven under a real unprivileged token. What is NOT yet proven
under such a token is that the LEGITIMATE path works on 0.1.11 — that run needs
the scheduler, and the scheduler needs the host on mains power.

**Human action required:** put the Mac on AC, then re-run
`~/dev/p36-stage/vmr p39-attack-outer P39FINAL` and
`~/dev/p36-stage/vmr p39-verbs P39FINAL`. Nothing else is outstanding.

## 18.6 MAC-SIDE VERIFICATION

| check | result |
|---|---|
| `cargo test --workspace` on an IDLE host | **127 test binaries, 576 passed, 0 failed, 0 ignored** |
| `intelligence-core` T5 wall-clock budget | passed — the Phase 38 flake was load-induced and did not recur |
| `static_validate.py` vs a worktree at pre-fix `db0dc49` | 344 checks / 21 failing on both. **NEWLY FAILING: NONE. NEWLY FIXED: NONE** |
| `aetherctl sec audit --profile cis-l1 --out … --format json` | exit 0, scored `aethercore.compliance.v1`, 3 pass / 1 fail / 4 not-verified, 75.0% |
| `phase39_ipc_authorization` (real service binary over a real socket) | 6/6, including the two legitimate-path tests that passed BEFORE the fix and still pass |

The CLI is deliberately NOT confined by the allowlist and that is not an
oversight. `aetherctl sec audit` runs `sec::run_audit` in-process under the
CALLER's own token — there is no privilege boundary to defend and no
LocalSystem authority to borrow, so confining it would break
`--profile cis-l1` (which names `/etc/ssh/sshd_config`, `/etc/sudoers` and the
home tree) while protecting nothing. The allowlist sits where the privilege
boundary is: the router. The reparse-point posture, by contrast, IS shared by
both callers, because not following a link is correct behaviour either way.

## 18.7 STATE FOR THE NEXT SESSION

- Branch `fix/localsystem-disclosure`, **not merged, not pushed** — six commits
  on top of `db0dc49`. The brief said not to merge without saying so.
- The VM has **0.1.11 installed and running**, service RUNNING, 16 files, pipe
  DACL and install-dir ACLs identical to the section 10 baseline.
- Recovery point for this session: **P39-PRE-FIX-BUILD
  `{7c10fb2b-dbdd-45c5-b8af-85ab9a0f342f}`**. Older fallbacks
  P38-PRE-0.1.7-INSTALL `{a226b395-…}` and P37-SHIPPING-QUALIFIED
  `{a1696567-…}` are both still present.
- **The one outstanding gate** is 18.5's blocked actual-token run. Put the Mac
  on mains power, then `vmr p39-attack-outer P39FINAL` and `vmr p39-verbs
  P39FINAL`. Until then the standing "18/18 verbs under BOTH actual-token
  contexts" gate is NOT met on 0.1.11 and the branch is not merge-ready.
- `verbs-outer.ps1` copies its transcript whether or not the task ran, so it can
  report a stale PASS. It should fail when `LastRunTime` is older than the run
  it just started. Not fixed here — it is harness work, and changing the gate
  harness in the same session that uses it to prove a security fix is exactly
  the shape of evidence nobody should trust.

### Package ledger (Phase 39 additions)

| version | ProductCode | why it exists |
|---|---|---|
| 0.1.9 | `{EE0AE741-51DE-F66B-2790-81B6EB90D02B}` | first fix attempt — **superseded, do not ship** (empty scope for every principal) |
| 0.1.10 | `{9878E6E1-3211-FA76-0D45-030EA6C16177}` | second attempt — **superseded, do not ship** (same) |
| **0.1.11** | `{98FCE2D5-44F0-A27C-A48B-8720FFE672F0}` | **the current package**: the owner-scoped allowlist, proven on the installed service |

### Snapshot ledger (Phase 39 additions)

| name | id | taken before |
|---|---|---|
| P39-PRE-FIX-BUILD | `{7c10fb2b-dbdd-45c5-b8af-85ab9a0f342f}` | syncing the fix and building/installing 0.1.9 |

# 19. PHASE 40 — BUILD HYGIENE AND TEST-HARNESS INTEGRITY (2026-09-01)

Recovery point for this session: **P40-PRE-HOUSEKEEPING
`{d652cd40-877c-4a9a-bb1b-2e3637a96ec2}`**, taken before any Phase 40 VM action.
`P37-SHIPPING-QUALIFIED {a1696567-…}` and `P39-PRE-FIX-BUILD {7c10fb2b-…}` are
both still present and neither was restored.

The Mac is on AC power, so §18.5's blocked actual-token run is runnable again.

## 19.0 The guest-tools channel was wedged before anything could be measured

Every `prlctl exec` — including `cmd.exe /c echo` — hung indefinitely and then
returned `PrlVm_TerminalConnect: PrlJob_Wait: PRL_ERR_IO_STOPPED`. Host load
average was 6.5–7.8 and the battery had been at 17%. The snapshot above was
taken first; the guest was then restarted and answered in under 20 seconds.
Recorded because it looks exactly like a hung gate and is not one.

## 19.2 THE GATE HARNESS COULD REPORT A RESULT FROM AN EARLIER RUN

### The defect

`verbs-outer.ps1` waited with

```powershell
do { Start-Sleep -Seconds 2; $state = (Get-ScheduledTask -TaskName $name).State }
while ($state -eq 'Running' -and (Get-Date) -lt $deadline)
```

A task that never leaves `Queued` — §18.5's battery stall — is not `Running`, so
the loop exits on its first evaluation. The harness then copied whatever
`C:\Users\Public\p36\verbs-<ctx>.txt` was on disk and reported it as this run's
result.

### The fix (`scripts/p36vm/verbs-outer.ps1`)

Three changes, each closing one half of it:

| change | what it prevents |
|---|---|
| the transcript is deleted before the task is registered | there is nothing stale left to mis-copy |
| the wait loop also waits through `Queued` | a stalled scheduler times out instead of returning instantly |
| `LastRunTime` must be newer than the moment this invocation started, or the harness **throws** `STALE_RESULT` | a result that predates the run it started is never reported at all |

### The fix is itself tested

`scripts/p36vm/verbs-outer.staletest.ps1` runs the predicate against the real
Task Scheduler in both directions — a check that has only ever been seen to pass
is not evidence:

```
NEVER_RAN LastRunTime=11/30/1999 00:00:00 RAN=False WANT=False
AFTER_RUN  LastRunTime=09/01/2026 15:26:24 RAN=True  WANT=True
STALETEST_FAILURES=0
```

The `NEVER_RAN` line is precisely the state that used to sail through as a PASS.

### Which recorded results were produced by the unfixed harness

`verbs-outer.ps1` was written 2026-08-31 14:26, so **every** verbs gate ever
recorded — S1, FINAL, desktop, P38FINAL, P39FINAL — ran under the unfixed
harness and was, at the time, unverified. They are not all stale, and the
transcripts themselves settle which is which: each carries `CAPTURED_UTC`, the
SHA of the `aetherctl.exe` it actually invoked, and per-verb `ELAPSED_MS`.

| label | CAPTURED_UTC | CTL_SHA256 | verdict |
|---|---|---|---|
| S1 | 2026-08-31T11:43:31Z / :40Z | `b8a29c92…` | genuine — its own capture time and its own timings |
| FINAL | 16:38:54Z / 16:39:04Z | `1bfd04c2…` | genuine |
| desktop | 17:34:13Z / 17:34:23Z | `1bfd04c2…` | genuine — same binary as FINAL, but a distinct capture time and every `ELAPSED_MS` differs |
| P38FINAL | 21:06:24Z / 21:06:34Z | `f70e820f…` | genuine |
| **P39FINAL** | 21:06:24Z / 21:06:34Z | `f70e820f…` | **STALE** — byte-identical to P38FINAL, and `f70e820f…` is the **0.1.8** CLI while 0.1.11 was installed |

`P39FINAL` is the one §18.5 already caught by hand. Nothing else was stale, and
the four genuine ones cannot be re-run in any meaningful sense: each measured a
different installed package, and the VM now holds 0.1.11. The one that can and
must be re-run against the current product is the 0.1.11 verbs gate.

### GATE — 18/18 VERBS ON 0.1.11, BOTH ACTUAL-TOKEN CONTEXTS, FIXED HARNESS: **PASS**

`verbs-outer.ps1 -Label P40FINAL`:

```
STD    TASK_STATE=Ready LAST_RESULT=0 LAST_RUN=09/01/2026 15:25:37 RAN_THIS_INVOCATION=True
ADMIN  TASK_STATE=Ready LAST_RESULT=0 LAST_RUN=09/01/2026 15:25:45 RAN_THIS_INVOCATION=True
```

18/18 `RESULT=RETURNED`. Both transcripts record
`CTL_SHA256=7847569b2a08dddc89ca305f497918ef088d26bf3e0b197eb5b14e28a80a823f`
— the **installed 0.1.11** CLI, not the 0.1.8 one the stale transcript carried —
under genuine tokens (`p36standarduser`, `IS_ELEVATED_ADMIN=False`; `p36admin`,
`True`). `doctor` returns `EXIT_CODE=5` in both contexts, unchanged from every
prior run.

**RESULTS THAT CHANGED: none.** The corrected harness returns the same verdict
the P38FINAL evidence supported. What changed is that the 0.1.11 gate is now
actually met rather than blocked: §18.7's single outstanding item is closed.

## 19.1 TEST CODE IN THE SHIPPING CRATE'S EXAMPLES DIRECTORY

### Measured before anything was moved

`p39_pipe_attack.rs` justified its location with "examples are not workspace
binaries and are not authored into `Product.wxs`, so nothing here reaches the
shipped payload". DBT-P36-008 is `ipc_probe.exe` in `C:\Program Files\AetherCore`,
so that reasoning has already been wrong once. The installed 0.1.11 image and the
MSI's own File table were both enumerated first:

```
INSTALL_FILE_COUNT=16      DEV_BINARY_IN_INSTALL_IMAGE=NO
MSI_FILE_ROWS=16           DEV_BINARY_IN_MSI_FILE_TABLE=NO
```

Sixteen files on disk, sixteen rows in the File table, one-for-one, no `p39_*`
anything. The belief was true this time. It was still a belief.

### Relocated

`apps/aetherctl/examples/p39_pipe_attack.rs` ->
`tools/p39-probes/src/p39_pipe_attack.rs`, a workspace member with
`[[bin]] name = "p39_pipe_attack"`. `tools/p36-probes/` — where the earlier
developer tools went — was deleted when DBT-P36-001 closed, so this is the same
place under this phase's name, beside `tools/ga-probe` and
`tools/support-bundle-verify`. It adds no product API: it still uses
`aethercore_ipc::SessionClient` exactly as `aetherctl` does.

Built on the VM from the new location: `cargo build --release -p
aethercore-p39-probes` -> `EXIT=0`,
`target\release\p39_pipe_attack.exe` sha256 `cf0e892ceba0608413b37d74062ae1de…`.
The MSI recipe's fixed package set does **not** include it
(`PROBE_BUILT_BY_MSI_RECIPE=False`), which is the point.

The guest source tree also still held `crates/security/examples/p39_profile_probe.rs`
and a compiled `target\release\examples\p39_pipe_attack.exe` from Phase 39. The
sync is copy-only and had never deleted anything; both are gone now, and the
sync script deletes what leaves the repo.

### The check (DBT-P40-002)

`scripts/check-msi-payload.ps1`. The allowlist is **derived**, not maintained:
every row of the MSI File table must correspond to a `<File Source="…">` in
`installer/wix/Product.wxs`, the file that *is* the authorization to ship
something. Authoring a probe into the wxs to get past it also fails, on the name.

Wired in as `[7/7]` of `scripts/build-arm64-msi.cmd` (after `wix msi validate`)
and after the same step in `scripts/build-installer.ps1`.

`scripts/p36vm/check-msi-payload.selftest.ps1` runs it against a real package in
all three states — a check only ever seen to pass proves nothing:

```
CASE clean-package           WANT=0 GOT=0 OK
CASE probe-authored-in-wxs   WANT=1 GOT=1 OK
CASE unauthored-file-in-msi  WANT=1 GOT=1 OK
```

## 19.3 GATES — NOTHING ELSE MOVED

The installed article was **not** replaced. 0.1.11 was already installed from
`1a6ea3f1…` and is the qualified package; a same-version rebuild carries a new
PackageCode and would need `REINSTALLMODE=vamus` (tranche 3 #2) to reinstall,
which would swap out the qualified install for no gain. The rebuild proves the
build; the installed 0.1.11 carries the client gates.

| gate | result |
|---|---|
| MSI build | `=== BUILD OK: …\AetherCore-0.1.11-arm64.msi`, all seven steps ran |
| ICE | `ICE_MATCHES=0`, `wix msi validate` step `[6/7]` present, no suppression |
| **payload check** | step `[7/7]` ran in the real build: `AUTHORED_FILES=16 MSI_FILE_ROWS=16 PAYLOAD_CHECK=PASS` |
| ProductCode | `{98FCE2D5-44F0-A27C-A48B-8720FFE672F0}` — identical to the 0.1.11 in ARP, as the derivation requires |
| verbs | **18/18 RETURNED**, both actual-token contexts, `RAN_THIS_INVOCATION=True` (§19.2) |
| service | `STATE 4 RUNNING` |
| pipe DACL | identical to the §10 baseline, field-for-field |
| install-dir ACLs | `installdir_icacls` **identical** to `verify-S1-postinstall.json` |
| `sc qc` / `sc qsidtype` / `sc sdshow` / ProgramData | all identical to that baseline |
| authorization tests | `phase39_ipc_authorization` **6/6** |
| attack, real pipe, BOTH real tokens | `FAILURES=0` in both |
| legitimate path | scored `cis-l1` report returned to a standard user |
| `static_validate.py` | 344 checks / 21 failing — the §18.6 numbers exactly. **Newly failing: none** |

The rebuilt package is `3ae46dff…` against the installed `1a6ea3f1…`: the same
1,099,653,120 bytes, a different PackageCode and a non-reproducible
`aethercore-desktop.exe` (tranche 3 #7), both expected and neither a criterion.

Only version-dependent fields differ from the S1 baseline snapshot — ARP key and
version, `HKLM\…\InstallVersion`, and the payload SHAs (0.1.6 -> 0.1.11). Every
security-relevant field is byte-identical. **Nothing was widened.**

### The attack, on the installed 0.1.11, under BOTH actual tokens

`p39-attack-outer.ps1 -Label P40FINAL`, probe `cf0e892c…` built from
`tools/p39-probes`:

```
STD    RAN_THIS_INVOCATION=True   WHOAMI=…\p36standarduser  IS_ELEVATED_ADMIN=False
  ATTACK_AUDIT_FOREIGN_PATH     403 sec.targetOutsideOwnerScope   BODY: (empty)
  ATTACK_JOURNAL_FOREIGN_OWNER  403 journal.ownerScopeForbidden   BODY: (empty)
  LEGIT_AUDIT_OWN_SCOPE           0  cve:ok:0 | secrets:ok:1
  LEGIT_JOURNAL_OWN_SCOPE         0  records=0 signed=false
  FAILURES=0
ADMIN  RAN_THIS_INVOCATION=True   WHOAMI=…\p36admin          IS_ELEVATED_ADMIN=True
  same four lines, FAILURES=0
```

§18.5 could only prove the refusal under a real unprivileged token, on 0.1.9 and
0.1.10. This is the refusal **and** the legitimate lane, on 0.1.11, under both
real tokens. `p39-attack-outer.ps1` carries the §19.2 freshness gate too.

### A legitimate caller still gets a scored report

From the `secaudit` verb in the P40FINAL **standard-user** transcript:

```
schema      aethercore.compliance.v1     profile_id  cis-l1
score.pass 1  fail 0  na 2  not_verified 5   score_pct  calculated 100.0
```

On the Mac, `aetherctl sec audit --profile cis-l1 --format json` exits 0 with the
same schema and §18.6's numbers unchanged: 3 pass / 1 fail / 4 not-verified,
75.0%.

### Two harness defects found while running the gates

Recorded because both look exactly like the §19.2 class:

1. `p39-build-launch.ps1` wrote its completion signal as
   `echo EXIT=%ERRORLEVEL%> "$st"`. `cmd` reads the `0` of the expanded
   `ERRORLEVEL` as the handle in a `0>` redirect, so the status file was created
   **empty** and every poller reported `STATUS=RUNNING` forever — including after
   the build had finished successfully at 15:35:30. Fixed with a space.
2. Two `prlctl exec` calls in flight at once return
   `PrlJob_GetResult: Invalid argument`. Guest invocations must be serialised.

### Mac-side

`cargo test --workspace`: **128 test binaries, 576 passed, 0 failed, 0 ignored**.
§18.6 recorded 127 binaries and the same 576 passing tests; the extra binary is
`aethercore-p39-probes` itself, which contributes no tests. No test moved.

`phase39_ipc_authorization` (real service binary over a real socket, `--features
unix-ipc`): **6/6**, including the two legitimate-path cases.

## 19.4 STATE FOR THE NEXT SESSION

- Branch `fix/localsystem-disclosure`, **pushed** to `origin`. Still not merged —
  no brief has asked for that.
- §18.7's single outstanding gate is **closed**: 18/18 verbs on 0.1.11 under both
  actual-token contexts, on a harness that can no longer report a stale result.
- The VM has **0.1.11 installed and running**, unchanged: 16 files, service
  RUNNING, pipe DACL and install-dir ACLs identical to the §10 baseline. The
  0.1.11 MSI in `build\out` is now the Phase 40 rebuild (`3ae46dff…`), which is
  NOT the installed article (`1a6ea3f1…`); they are the same version and the same
  ProductCode, so reinstalling from it would need `REINSTALLMODE=vamus`.
- Recovery point for this session: **P40-PRE-HOUSEKEEPING
  `{d652cd40-877c-4a9a-bb1b-2e3637a96ec2}`**. `P37-SHIPPING-QUALIFIED
  {a1696567-…}` and `P39-PRE-FIX-BUILD {7c10fb2b-…}` are untouched. Neither
  `P36-CLEAN-BASELINE` nor `P36-PRE-NATIVE-MUTATION` was restored.
- **DBT-P40-003 is open**: `crates/security-audit/examples/gd4_live_audit.rs` is
  the same shape as the file this session moved. It has no Windows build path and
  the payload check now measures the question it raises, so it was recorded
  rather than moved. Move it the next time that crate is touched.
- `prlctl exec` must be serialised — two concurrent calls fail the job outright.

### Snapshot ledger (Phase 40 additions)

| name | id | taken before |
|---|---|---|
| P40-PRE-HOUSEKEEPING | `{d652cd40-877c-4a9a-bb1b-2e3637a96ec2}` | any Phase 40 VM action (guest restart, source sync, rebuild) |

---

# PHASE 41 — PHYSICAL x86_64 WINDOWS QUALIFICATION (2026-09-02)

Machine: `HUSSEIN` — MSI Pulse 16 AI C1VFKG, Intel Core Ultra 9 185H (16C/22T),
15.49 GB RAM, NVMe Micron_2500_MTFDKBA1T0QGN 953.9 GB (447.1 GB free),
Intel Arc iGPU + NVIDIA RTX 4060 Laptop. Windows 11 Pro 25H2 build 26200, x64.
Repo: `C:\dev\aethercore`, branch `main` at `e91f675`.

**THIS MACHINE HAS NO SNAPSHOTS.** Every mutation is permanent.

## 41.0 CRLF DAMAGE FOUND AND FIXED BEFORE ANYTHING ELSE

`git status` reported **1027 modified files**. Repo-level `core.autocrlf` was
already `false`, but the *system* gitconfig
(`C:/Users/husen/AppData/Local/hermes/git/etc/gitconfig`) carries
`core.autocrlf=true`, and the tree had been checked out under it at some point.
Every one of the 1027 was a whole-file CRLF rewrite.

Proven content-identical before restoring, not assumed:
`git diff --ignore-cr-at-eol --stat` returned **empty** across all 1027 files,
and `git status --porcelain` showed 1027 `M` and **zero** untracked/added.
Restored with `git restore --source=HEAD --worktree -- phase21-workspace`
-> working tree clean, `core.autocrlf` still `false`.

Repo-level `false` overrides the system-level `true`, so the system gitconfig was
deliberately NOT modified — it is a machine-wide setting outside this task.

## 41.1 DESTRUCTIVE ACTION RECORD — Gate 0c: enable System Restore, create point

    ACTION=   Enable System Restore on C: if disabled, then create a restore
              point named "AetherCore baseline - before any install".
              If Windows' 24h creation throttle
              (SystemRestorePointCreationFrequency) suppresses it, set that
              value to 0, create the point, then put the value back exactly as
              found.
    RECOVERY= Restore-point creation is additive - it creates a new shadow copy
              and removes nothing. To undo: delete that single restore point via
              vssadmin, and/or Disable-ComputerRestore -Drive C:\ to return SR
              to its prior state. The throttle registry value is captured before
              the change and rewritten after, so its prior state is recoverable
              by construction. NOT independently proven on this machine - no
              snapshot exists to prove it against, which is precisely why the
              restore point is being created.
    EXPECTED= Get-ComputerRestorePoint lists a point whose Description is
              "AetherCore baseline - before any install" with a CreationTime
              within minutes of now, and a SequenceNumber. Shadow storage is
              allocated on C:.
    BLAST=    Enabling SR allocates shadow-copy space on C: (447 GB free, so
              space is not at risk). Worst realistic case is that SR cannot be
              enabled or the point cannot be created - in which case Gate 0
              FAILS and, per the brief, only Stage 1 (non-destructive) runs and
              nothing is installed. No existing data is written or deleted by
              this action.

## 41.2 GATE 0 — RESULT: **PASS** (2026-09-02)

### 0a Machine (measured)

| property | observed |
|---|---|
| Edition / Version / Build | Windows 11 Pro, 25H2, 10.0.26200 |
| OS architecture | 64-bit; `PROCESSOR_ARCHITECTURE=AMD64` |
| CPU | Intel Core Ultra 9 185H — 16 cores / 22 logical, `Intel64 Family 6 Model 170 Stepping 4` |
| RAM | 15.49 GB |
| Disk | NVMe Micron_2500_MTFDKBA1T0QGN, 953.9 GB, SSD/NVMe, **446.5 GB free** |
| GPU 0 | Intel(R) Arc(TM) Graphics, driver 31.0.101.5007 |
| GPU 1 | NVIDIA GeForce RTX 4060 Laptop GPU, driver 32.0.16.1656 |
| PowerShell | 5.1.26100.9168 (Windows PowerShell) |
| Machine | MSI Pulse 16 AI C1VFKG |

**x86_64 CONFIRMED.** Not ARM64. The machine's stated purpose is satisfiable.

### 0b Protections — all ENABLED, none touched

| protection | observed |
|---|---|
| Defender real-time | `RealTimeProtectionEnabled=True`, `AntivirusEnabled=True`, `AMServiceEnabled=True` |
| Defender tamper protection | `IsTamperProtected=True` |
| UAC | `EnableLUA=1`, `ConsentPromptBehaviorAdmin=5`, `PromptOnSecureDesktop=1` |
| Firewall | Domain=True, Private=True, Public=True |
| SmartScreen | no `SmartScreenEnabled` override and no policy override -> default **On** |

Nothing was disabled. Nothing will be.

### Session elevation — the one thing that nearly stopped Gate 0

The harness session runs as `HUSSEIN\husen` **unelevated** (`IsInRole(Administrator)=False`;
a write to `C:\Windows` was denied). `husen` *is* a member of the local
`Administrators` group (MicrosoftAccount principal), so elevation is reachable —
but only by spawning a child process with `-Verb RunAs`, which raises a UAC
consent prompt on the secure desktop. Probed and confirmed working.

**Operating consequence for every future session on this machine:** admin work
must be batched into a single elevated `.ps1` per step that writes its results to
a log file, which the unelevated session then reads. One UAC prompt per batch.
Do not attempt admin cmdlets inline — they return `Access denied`.

### 0c / 0d System Restore — enabled, point created, point PROVEN

Before: `Get-ComputerRestorePoint` -> **0 existing restore points**.
`SystemRestoreConfig`: `DiskPercent=15`, `RPSessionInterval=0`.
Shadow storage on C: 5.04 GB used / 5.50 GB allocated / 19.1 GB max.

`Enable-ComputerRestore -Drive "C:\"` -> OK.

Windows throttles restore-point creation to one per 24 h. The throttle value
`SystemRestorePointCreationFrequency` was **NOT SET** (Windows default 1440 min);
it was temporarily set to `0`, the point was created, and the value was then
**removed** — returning it exactly to its prior unset state.

`Checkpoint-Computer -RestorePointType MODIFY_SETTINGS` -> OK.

**0d proof — enumerated after creation, not assumed:**

```
Restore points now: 1

SequenceNumber Description                              CreationTime               RestorePointType
-------------- -----------                              ------------               ----------------
             1 AetherCore baseline — before any install 20260901223933.946086-000                12
```

Shadow storage after: 5.08 GB used / 6.00 GB allocated / 19.1 GB max.

**GATE 0: PASS.** A named restore point is verifiably present. The machine may
be mutated.

### 0e Full disk image — DOES NOT EXIST AND IS NOT CURRENTLY POSSIBLE

Measured, stated plainly because the owner must know it:

- **One** physical disk: `DeviceId 0`, NVMe Micron_2500, 953.9 GB.
- Volumes: `C:` (953 GB, 446.5 GB free) and one unlettered 0.8 GB recovery
  partition. `Win32_LogicalDisk` reports **DriveType 3 (Fixed) only** — there is
  **no removable, external or network volume attached**.
- BitLocker status could not be read (needs admin; not queried in the Gate 0
  elevated batch).

There is nowhere to put a full disk image. Imaging C: onto C: is not a recovery
path — it dies with the disk and with the OS it is meant to restore.

**What the restore point does and does not cover.** A System Restore point
protects system files, the registry, and **driver** state. That is genuinely the
right instrument for Stage 4 driver install/rollback and it is why Stage 4 is
survivable at all. It does **not** image user data, and it does **not** guarantee
recovery from a machine that will not boot — restoring it requires either a
booting Windows or WinRE.

**OWNER DECISION REQUIRED before Stage 4:** attach an external drive of >= 512 GB
and take a full image. Without one, Stage 4 driver work carries a real,
non-zero risk of an unbootable machine that only a Windows reinstall clears.

## 41.3 DESTRUCTIVE ACTION RECORD — Stage 1a: install the build toolchain

Measured first. The machine is a **bare** box: the only build-relevant software
present is Node.js v22.23.2 (hermes-managed), npm, corepack, winget v1.29.290 and
the WebView2 Runtime (pv 151.0.4129.107). Everything else is absent —
`rustc`/`cargo`/`rustup`, `.NET`, Visual Studio Build Tools (no `vswhere.exe` at
all), the Windows SDK (`C:\Program Files (x86)\Windows Kits\10` does not exist),
LLVM/clang, CMake, Ninja, WiX, pnpm.

    ACTION=   Install the build prerequisites the repo's own scripts/bootstrap.ps1
              declares, using the winget package IDs it names:
              Rustlang.Rustup, Microsoft.DotNet.SDK.8,
              Microsoft.VisualStudio.2022.BuildTools (VCTools workload,
              --includeRecommended, which carries the Windows SDK), pnpm via npm,
              and the WiX .NET global tool. Node.js and WebView2 are already
              present and are NOT reinstalled.
    RECOVERY= Every one of these is a normal, independently uninstallable product:
              winget uninstall for the three winget IDs, `npm uninstall -g pnpm`,
              `dotnet tool uninstall --global wix`, and rustup's own `rustup self
              uninstall`. None of them modifies an existing AetherCore install
              (there is none yet), none touches Defender/UAC/Firewall/SmartScreen,
              and none replaces an existing compiler - there is no prior toolchain
              on this machine to overwrite. Restore point SequenceNumber 1 predates
              all of it. Uninstall is NOT independently proven on this machine;
              it is standard product behaviour, not a measured claim.
    EXPECTED= rustc reports a 1.97.1 x86_64-pc-windows-msvc toolchain; vswhere
              resolves an installation with
              Microsoft.VisualStudio.Component.VC.Tools.x86.x64; a Windows Kits 10
              bin directory exists; dotnet, pnpm and wix all report versions.
              scripts/bootstrap.ps1 stops reporting missing prerequisites.
    BLAST=    Disk: roughly 8-12 GB consumed on C: (446.5 GB free, so not at
              risk). A failed or partial VS Build Tools install leaves an
              incomplete toolchain, which fails Stage 1c loudly at compile time
              rather than silently - it cannot produce a wrong artifact, only no
              artifact. Nothing already on the machine depends on these packages,
              so a bad install cannot break existing software. This is additive
              developer tooling on a box with no prior toolchain; it is the
              lowest-risk mutation in the whole session.

## 41.4 GATE 1 — RESULT: **PASS** (2026-09-02)

### 1a Toolchain — measured, then installed

Present before: Node.js v22.23.2, npm, corepack, winget 1.29.290, WebView2 151.0.4129.107.
**Everything else was missing.** Installed this session, in this order:

| tool | how | version |
|---|---|---|
| rustup / rustc / cargo | winget `Rustlang.Rustup` | rustc 1.97.1 (8bab26f4f), host `x86_64-pc-windows-msvc` |
| pnpm | `npm i -g pnpm@11.22.0` (matches `packageManager` pin) | 11.22.0 |
| .NET SDK | winget `Microsoft.DotNet.SDK.8` | 8.x |
| VS 2022 Build Tools + VCTools | winget `Microsoft.VisualStudio.2022.BuildTools` | `C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools` |
| Windows SDK | via `--includeRecommended` | 10.0.26100.0 |
| CMake | winget `Kitware.CMake` | 4.4.3 |
| LLVM (libclang) | winget `LLVM.LLVM` | 22.1.8 |
| Windows ADK (Deployment Tools) | winget `Microsoft.WindowsADK` | for `dismapi.lib` |
| WiX | `dotnet tool restore` (pinned manifest) | 6.0.2+b3f3403 |

**Four toolchain facts a future x64 session should not re-derive:**

1. **`rustup-init` hangs under winget.** It blocked at ~50 s CPU and never returned;
   the shim files in `.cargo\bin` were left reporting 0 bytes. Killing it (which
   needs elevation — the child inherits it) and letting `rust-toolchain.toml`
   auto-install 1.97.1 works. **The 0-byte shims are a red herring**: `cargo -V`
   and `rustc -vV` both execute correctly. Do not "repair" them.
2. **Do NOT use `VsDevCmd.bat`.** On this box it fails to run a bare `vswhere.exe`
   and then hard-exits the parent `cmd`, killing the build script with no error in
   the log. It is not needed: rustc auto-discovers the MSVC linker via
   vswhere/registry. Proven with a standalone `rustc` link probe leading to exit 0.
3. **`llama-cpp-sys-2` runs bindgen**, so `LIBCLANG_PATH` must point at LLVM even
   on x64/MSVC. Without it: `Unable to find libclang`, build fails at 0 crates.
4. **The ADK x64 lib directory is named `amd64`, not `x64`.** The ARM64 recipe's
   `DismApi\Lib\arm64` does not generalise to `DismApi\Lib\x64` — that path does
   not exist. `crates/system-repair/src/dism_api.rs` binds
   `#[link(name = "DismApi")]`, so the link fails without it.

### 1b What git does not carry — full sweep on this machine

Ran the project's own standing rule, `git ls-files --others --ignored --exclude-standard`.
On this machine it returns **exactly one** path:

```
phase21-workspace/assets/models/qwen2.5-1.5b-instruct-q4_k_m.gguf
```

Verified rather than assumed: **1,117,320,736 bytes**, SHA-256
`6a1a2eb6d15622bf3c96857206351ba97e1af16c30d7a74ee38970e434e9407e` — equal to
`models.manifest.json` and to the value in the brief. Every other ignore rule
(`target/`, `node_modules/`, `dist/`, `*.db`, `*.zip`, `**/BINARY_ARTIFACTS/`,
`_archive/`, editor/OS noise) excludes only build output or noise, and nothing
those rules cover is present-and-required here. Section 15.3's conclusion holds on x64.

### 1c THE FIRST NATIVE x86_64 BUILD IN THIS PROJECT'S HISTORY — exit 0

| step | result |
|---|---|
| `pnpm install --frozen-lockfile` | exit 0; lockfile passed supply-chain policy (85 entries) |
| `pnpm build` (apps/ui) | exit 0; `dist` = 3 files, 702.9 KB |
| `cargo build --release` (fixed 5-package set) | **exit 0**, 2 m 03 s |
| tauri `build --no-bundle` | **exit 0**, 3 m 27 s |
| `wix build -arch x64` | **exit 0** (15 m 27 s — the 1.07 GB model dominates CAB) |
| `wix msi validate` | **exit 0, output EMPTY = ZERO ICE findings, no suppression** |
| `check-msi-payload.ps1` | `PAYLOAD_CHECK=PASS`, `AUTHORED_FILES=17`, `MSI_FILE_ROWS=16` |

MSI: `out\release\AetherCore.msi`, version **0.1.11**, **1,100,140,544 bytes**,
SHA-256 `d18d89db07180f5727b7d6056a07ea8d50de97aa401838601b532e9befd1f227`.

**The `beforeBuildCommand` defect is real and reproduced on x64.** `tauri.conf.json`
declares `"beforeBuildCommand": "pnpm --dir ../ui build"`; the Tauri CLI runs it
from its own discovered app directory, not from the tauri.conf.json directory, so
`../ui` does not resolve. The repo already carries the recorded fix —
`installer/tauri.no-before-build.json`, an overlay that blanks the hook — because
step [1] has already produced `apps/ui/dist`. Used it via `--config`; **no change
to `tauri.conf.json` was needed or made.**

### A SECOND x64 DEFECT, FOUND HERE, THAT NO VM COULD HAVE FOUND

`installer/wix/Product.wxs` hard-coded `libomp140.aarch64.dll`. That is an
**ARM64-only** file, so the x64 package could not build at all — the OpenMP
runtime is architecture-specific and nothing in the authoring said so.

Measured with `llvm-readobj` coff-imports on the real binaries, not assumed:

```
aethercore-maintenance-service.exe -> VCOMP140.DLL      (x64/MSVC, this build)
aethercore-desktop.exe, consent-broker, update-broker,
install-hardener, aetherctl                             -> no OpenMP import
```

The ARM64 clang-cl build imports the LLVM runtime `libomp140.aarch64.dll`; the x64
MSVC build imports Microsoft's `VCOMP140.DLL`. Same role, different runtime.

**Fix (minimal, and deliberately NOT a `-d` variable):** the File element is now
selected by the WiX preprocessor on `$(sys.BUILDARCH)`. A `$(var.OpenMpDll)` in
the `Source` attribute would have broken `check-msi-payload.ps1`, which derives
its allowlist by regexing the **literal** `Source="..."` strings out of Product.wxs —
the authored name would have been unmatchable and every build would have failed
the payload check. With the preprocessor both literal names stay in the file, so
the derived allowlist is a superset (`AUTHORED_FILES=17`) while the package
carries exactly one (`MSI_FILE_ROWS=16`). The ARM64 pipeline is unchanged and
still selects its own file.

`scripts/build-installer.ps1` gained `vcomp140.dll` in its `$required` payload list.

### 1d Payload vs the ARM64 baseline of SIXTEEN files

**16 rows on x64, 16 rows on ARM64. Exactly one difference.**

| # | file | bytes | vs ARM64 |
|---|---|---|---|
| 1 | aethercore-desktop.exe | 6,959,616 | same role |
| 2 | aethercore-maintenance-service.exe | 10,678,784 | same role |
| 3 | aethercore-consent-broker.exe | 633,856 | same role |
| 4 | aethercore-update-broker.exe | 736,768 | same role |
| 5 | aethercore-install-hardener.exe | 273,408 | same role |
| 6 | aetherctl.exe | 4,315,648 | same role |
| 7 | **vcomp140.dll** | 193,152 | **DIFFERENT — replaces `libomp140.aarch64.dll`** |
| 8 | update-trust.json | 83 | identical |
| 9 | UNINSTALL.txt | 3,206 | identical |
| 10 | qwen2.5-1.5b-instruct-q4_k_m.gguf | 1,117,320,736 | identical |
| 11 | models.manifest.json | 898 | identical |
| 12 | Apache-2.0.txt | 11,358 | identical |
| 13 | Qwen-GGUF-NOTICE.txt | 11,343 | identical |
| 14 | vulndb.json | 6,704 | identical |
| 15 | vulndb.manifest.json | 142 | identical |
| 16 | cis_map.json | 3,103 | identical |

**The single difference is ARCHITECTURAL, not a defect**: it is the correct
OpenMP runtime for the compiler that built this architecture, and it is the file
the x64 service actually imports. The six executables differ in size from the
ARM64 ones because they are a different instruction set — expected, and per
section 3 byte-equality is not a criterion anywhere.

**GATE 1: PASS.** MSI builds with zero ICE and every payload difference is explained.

### DBT-P41-001 (open, recorded not fixed)

The x64 service also imports `MSVCP140.dll`, `VCRUNTIME140.dll` and
`VCRUNTIME140_1.dll`. On this machine all three are in `System32` (VC++
2015-2022 Redistributable x64 14.44.35211 is installed), so nothing failed. They
are **not** in the MSI payload, and a clean Windows box is not guaranteed to have
them. This is the same class of question the ARM64 `libomp` line answered for
OpenMP only. Not changed here — it is outside this brief's scope and would widen
the payload — but the owner should decide whether the bundle must carry the VC++
redistributable. Stage 2 on this machine cannot detect the gap, because this
machine already has the redistributable.

## 41.5 DESTRUCTIVE ACTION RECORD — Stage 2: install the x64 MSI

    ACTION=   msiexec /i out\release\AetherCore.msi /qn with /l*v verbose logging.
              Installs AetherCore 0.1.11 x64 to C:\Program Files\AetherCore and
              registers the AetherCoreMaintenance service as LocalSystem,
              AUTO_START, with a restricted Service SID and a hardened pipe DACL.
              This is the FIRST product install ever performed on this machine and
              the first destructive act of this session.
    RECOVERY= msiexec /x <ProductCode> /qn removes it; that is the same uninstall
              path Stage 5b exercises deliberately and it is proven on ARM64 across
              fourteen survivor checks. Behind that, System Restore point
              SequenceNumber 1 "AetherCore baseline - before any install" was
              created and ENUMERATED in Gate 0 and predates every install. The
              uninstall path is NOT yet proven on THIS machine - proving it is
              exactly what Stage 5b is for.
    EXPECTED= msiexec exit 0. C:\Program Files\AetherCore holds SIXTEEN files.
              Service AetherCoreMaintenance = LocalSystem, AUTO_START, RUNNING.
              Service SID type UNRESTRICTED and Active. Pipe DACL equal to
              O:<service SID> G:SY D:P(A;;FA;;;<service SID>)(A;;FR;;;AU)(A;;DC;;;AU)
              (the AU pair may render as (A;;0x12008b;;;AU) - the SAME DACL).
              Install-dir ACLs: Users and the service SID read-execute, no Users
              write. vcomp140.dll present (the x64 counterpart of the ARM64
              libomp140.aarch64.dll). No p39_* or other developer binary.
              All four verbs return - doctor returning the typed
              diagnostics.stateUnavailable is a PASS. insights list reports
              engineLabel "localModel", NOT ruleFallback.
    BLAST=    A failed install can leave a partially-registered service or a
              half-populated Program Files directory. msiexec transactions roll
              back on failure, and the verbose log names the failing action. Worst
              case is an orphaned service registration, cleared with
              "sc delete AetherCoreMaintenance" plus removing the directory, or by
              rolling back to restore point 1. No user data is touched: the product
              writes only under Program Files and ProgramData. The machine's boot
              path is not involved. This is materially lower risk than Stage 4.

## 41.6 GATE 2 — **BLOCKED, NOT FAILED**: the UAC prompt was declined

Stage 2 did not run. **Nothing was installed and nothing on this machine was
mutated.** Recorded precisely so the next session does not re-derive it.

The harness session runs as `HUSSEIN\husen` **unelevated** (Gate 0). Admin work is
done by spawning a child with `-Verb RunAs`, which raises a UAC consent prompt on
the secure desktop. That worked five times in a row this session — Gate 0c/0d, and
four toolchain installs. On the Stage 2 attempt it stopped working:

```
Start-Process RunAs threw:
  This command cannot be run due to the error: The operation was canceled by the user.
```

Verified as a true no-op rather than assumed:

```
stage2.log exists            : False        (the elevated script never started)
C:\Program Files\AetherCore  : False
sc query AetherCoreMaintenance -> [SC] EnumQueryServicesStatus:OpenService FAILED 1060:
                                 The specified service does not exist as an installed service.
no consent.exe / msiexec.exe pending
```

**This is an environment blocker, not a product defect and not a machine-safety
problem.** The MSI is built, validated and sitting at
`out\release\AetherCore.msi`. `msiexec` needs elevation; a human must accept the
UAC prompt (or the session must be started from an already-elevated PowerShell,
which is what the brief asked for and would remove the prompt-per-batch problem
entirely).

**Consequence:** Gates 2, 3 (service-backed), 4 and 5 all depend on an installed
product, so all four are blocked behind this one action. Everything that could be
done *without* it was done instead, and is recorded in 41.7.

## 41.7 STAGE 3 (PARTIAL) — real x86_64 silicon, offline read-only surface

`aetherctl help` documents an **OFFLINE** command group: "no service required,
strictly read-only". Those verbs need neither the install nor elevation, so they
were run from the freshly built x64 binary against real hardware. This is genuine
native-x64 evidence and it is the part of Stage 3 that survived the blocker.

Binary: `target\release\aetherctl.exe`, sha256
`910df7c9ea1010285320abbc3fffbc8469d5c8139c555b228e45151a6c13813d`.
Run as `hussein\husen`, `ELEVATED=False`.

| verb | exit | result |
|---|---|---|
| `version` | 0 | `protocolVersion 7`, `version 0.1.11` |
| `about` | 0 | `platform windows`, product AetherCore |
| `capabilities` | 0 | 16 capabilities, **every one `state: native`** |
| `engine-source` | 0 | `source: native` |
| `service detect` | 0 | `state: Offline`, `cli.detect.offline` — correctly reports the absent service |
| `telemetry-once` | 0 | real values, see below |
| `self-check` | 0 | **`sha256Match: true`**, `manifestValid: true` |
| `self-check --load-model` | 7 | `embeddedModelLoaderNotCompiled` — **correct by design, see below** |

### The embedded model verified on real x64 hardware

`self-check` re-hashed the shipped artifact and matched it against the pinned
manifest **on this machine**:

```
fileName  qwen2.5-1.5b-instruct-q4_k_m.gguf
bytes     1117320736
sha256Match   true
manifestValid true
manifestSchema aethercore.phase23.model-manifest.v1
```

### `--load-model` returning exit 7 is NOT a defect — measured, not assumed

This looked at first like the exact failure the brief warns about (the model
shipping but `ruleFallback` running). It is not. The feature graph says so:

- `apps/aetherctl/Cargo.toml`: `default = []`, and `embedded-model` is **opt-in**
  for the CLI. Its own comment: *"Without this feature `self-check --load-model`
  returns the honest typed not-available answer."* Exit 7 with
  `CapabilityUnavailable` **is** that honest answer.
- `crates/intelligence-core/Cargo.toml`: `default = ["embedded-model"]`.
- `services/maintenance-service/Cargo.toml` line 42 depends on
  `aethercore-intelligence-core` by plain path — **`default-features` is not
  disabled** — so the service compiles the loader in.

Confirmed in the built artifacts rather than inferred from manifests:

```
aethercore-maintenance-service.exe   llama_ gguf llama.cpp ggml  -> ALL PRESENT
aetherctl.exe                        llama_ gguf llama.cpp ggml  -> ALL ABSENT
```

The x64 service carries the real loader. **`engineLabel` still has to be proven at
Stage 2 by `insights list` against the running service — this is supporting
evidence, not the proof, and is not counted as one.**

### 3a Telemetry against actual silicon — and the honesty contract, partly kept

`telemetry-once`, `providerSource: PerfPlatform`:

| surface | observed | verdict |
|---|---|---|
| memory | `totalPhysicalBytes 16,632,156,160` (= the 15.49 GB measured in Gate 0), `availablePhysicalBytes 4,628,471,808`, `memoryLoadPercent 72` | **real values** |
| gpu | `gpu: null` **plus** an explicit fault: `{"collector":"gpu","kind":"Unavailable","detail":"no GPU engine counters exposed by this adapter/driver"}` | **honesty contract KEPT** — absence declared, not zeroed |
| power | `hasTemperature: false`, `temperatureC: 0`, `throttleActive: true`, `throttleReason: "power"` | **honesty contract KEPT** — the 0 is explicitly guarded by `hasTemperature: false` |
| cpu | `totalBusyBp 0`, `perProcessorBusyBp []`, `contextSwitchesPerSec 0`, `dpcIsrBusyBp 0`, `processorQueueLengthX100 0` — and **no `collectorFault` for cpu** | **OBSERVED != EXPECTED — recorded, see below** |
| storage | `storage: []` — and **no `collectorFault` for storage** | **OBSERVED != EXPECTED — recorded, see below** |

### DBT-P41-002 (open) — cpu and storage return zero/empty with no declared fault

EXPECTED (the brief's Stage 3a criterion, and the product's own stated contract):
anything a device does not expose is reported as **not-available**, not as a zero.

OBSERVED: the `gpu` collector does exactly that — `null` plus a typed
`collectorFault` naming the reason. The `cpu` collector instead returned an
all-zero structure with an empty `perProcessorBusyBp`, and `storage` returned an
empty array, **neither accompanied by a `collectorFault`**. A consumer cannot
distinguish "this CPU is 0% busy" from "this collector produced nothing", which is
the precise distinction the contract exists to make. On a 22-logical-processor
machine that is running a build, 0% total busy and an empty per-processor array
are not plausible readings.

Per the STOP rule this is recorded as a raw observation and NOT theorised about.
Two things are deliberately **not** claimed:

1. This is the **offline** `telemetry-once` path with **no service running**. The
   service-backed `perf snapshot` — which is what Stage 3a actually specifies —
   has NOT been run, because it needs the install. The service path may well
   populate these collectors. **This finding is scoped to `telemetry-once`.**
2. No root cause is offered here.

The `capabilities` verb separately reports `telemetryCpu`, `telemetryStorage` and
`telemetryGpu` all as `state: native` — i.e. all three claim availability, while
two produce nothing and one declares a fault. SESSION_CONTEXT already records
that "five of sixteen capabilities were mis-reported on a workstation"; this is
the same shape and this is a workstation. Re-check it with the service running.

### What Stage 3 still owes, and cannot pay without the install

- 3a `perf snapshot` (service-backed) and SMART / NVMe attributes from the real disk
- 3b GPU telemetry, driver identity and memory against the RTX 4060 / Arc adapters
- 3c PnP + Windows Update driver discovery counts
- 3d service memory / CPU / disk at idle and while scanning, and peak working set
  at model load

### Elevation retried after the usage-limit pause — still declined

The Stage 2 elevation was attempted **three** times in total: once before the
pause and twice after it. Attempt 2 threw
`The operation was canceled by the user`; attempt 3 raised no exception but the
child process exited within 150 s with no log, no `consent.exe` ever visible, and
nothing installed. Retrying was then stopped deliberately rather than continuing
to raise UAC prompts.

State re-verified after the pause, unchanged and clean:

```
git HEAD                     c637ef1, working tree clean
out\release\AetherCore.msi   1,100,140,544 bytes, 2026-09-02 02:51:07  (intact)
C:\Program Files\AetherCore  does not exist
sc query AetherCoreMaintenance -> FAILED 1060 (service does not exist)
```

## 41.8 P41 PROGRESS TABLE (authoritative — resume from here)

| Gate | What it proves | Status | Evidence |
|---|---|---|---|
| **0** | Machine is native x64, protections on, a named restore point verifiably exists | **PASS** | §41.2 — restore point SequenceNumber 1 enumerated by name and timestamp; Defender/UAC/Firewall/SmartScreen all ENABLED and untouched |
| **0f** | A full, verified disk image exists before driver work | **PASS** | §41.12 + §41.13 — 1 version dated today, Bare Metal Recovery, 521.45 GB / 19 files / 97.8% of C: used, intact after the owner's format. Two honest debts: wbadmin's client exit code lost (0f.3), and the machine was **already installed** when imaged (0f.7) |
| **1** | MSI builds x64 with zero ICE, payload explained | **PASS** | §41.4 — cargo 0, tauri 0, wix build 0, `wix msi validate` exit 0 with EMPTY output, payload check PASS, 16 file rows, MSI sha256 `d18d89db…` |
| **2** | Installed, running, security properties match ARM64 baseline, local model live | **PASS** | §41.14 — installed 12:09:07 by the prior session (MsiInstaller 1033, status 0, from MSI `d18d89db…`); verified live: 16 files hash-identical to install time, LocalSystem/AUTO_START/RUNNING, SID UNRESTRICTED, pipe DACL matches, Users read-execute only, **`engineLabel=localModel`**. Supersedes the §41.6 BLOCKED row. |
| **3** | Real-hardware evidence with numbers | **PASS** | §41.15 + §41.16 — 3.A `perProcessorBusyBp []` on a real 22-CPU box; service path shows identical zeros; PDH counters proven PRESENT (PhysicalDisk 4 live instances, GPU Engine 568, CPU 7.01%) while the product reports nothing; 3a/3b/3c/3d all paid |
| **4** | Driver install + rollback on a deliberately safe device | **NOT STARTED — HARD STOP** | §41.17 — deliberately not begun. The disk image now exists and is verified (§41.13). Outstanding owner actions: boot-test the E: recovery media, and supply a driver — Windows Update offers **zero** (§41.16 3c) |
| **5** | Full lifecycle, zero survivors | **NOT STARTED** | no longer blocked behind Gate 2 (now PASS); the uninstall/survivor sweep is unproven on THIS machine and is what Stage 5b exists for |

## 41.9 WHAT THE NEXT SESSION NEEDS — one action unblocks four gates

**Start the session from an already-elevated PowerShell** (right-click →
Run as administrator, then launch the tool). That is what the brief asked for and
it removes the whole problem: every admin step this session had to push through a
separate `-Verb RunAs` child with a UAC prompt per batch, and once consent stopped
being granted, Gates 2–5 all stopped with it.

Everything else is already in place and does **not** need redoing:

- toolchain fully installed and recorded (§41.4 1a), including the four
  non-obvious facts (`rustup-init` hang, do-not-use `VsDevCmd`, `LIBCLANG_PATH`,
  ADK `amd64` not `x64`)
- `out\release\AetherCore.msi` built, validated, zero ICE, sha256 `d18d89db…`
- `scratchpad\stage2.ps1` is written and ready: it installs with `/l*v` logging and
  then runs the entire Stage 2 verification list — 16 files with hashes, service
  state/account/start type, service SID, pipe DACL via `NamedPipeClientStream`,
  install-dir ACLs, developer-binary sweep, the four verbs, and `engineLabel`.
  Run it elevated and Gate 2 is answered in one pass.

**Open items the owner must decide, unchanged by the blocker:**

- **No full disk image exists and none is currently possible** (§41.2 0e) — one
  physical disk, no external volume attached. Stage 4 driver work should not
  start until an external drive is attached and imaged.
- DBT-P41-001 — the x64 service imports `MSVCP140`/`VCRUNTIME140`, present on this
  box but not in the payload.
- DBT-P41-002 — `telemetry-once` cpu/storage return zero/empty with no declared
  `collectorFault`, while gpu correctly declares one. Re-measure with the service
  running before drawing any conclusion.

## 41.10 STAGE 2 PRE-INSTALL RECORD, REVISED — recovery is now a HARD GATE in code

Written and committed **before** the first elevated action of the next session,
per the standing rule. This supersedes the ordering assumption in §41.5.

### The ordering question, answered with what was measured

The owner raised that a restore point created *after* the first install cannot
recover the first install, and asked that restore-point creation be the first
elevated action. **On this machine the ordering already holds** — recorded here so
nobody re-derives it:

- The restore point was created at **Gate 0**, §41.2, before any install:
  `SequenceNumber 1`, `AetherCore baseline — before any install`,
  `CreationTime 20260901223933.946086-000`, `RestorePointType 12`, and it was
  **enumerated** after creation rather than assumed.
- **Nothing has been installed since.** Re-verified after the session pause:
  `C:\Program Files\AetherCore` does not exist and
  `sc query AetherCoreMaintenance` returns `FAILED 1060`.

So the machine is still in the pre-install state that point captures.

One correction to the stage order as the owner described it: the Stage 4 point
("AetherCore before driver work") is a **second** point. 4a adds one specifically
before driver work; it is not the only one and it was never meant to precede the
install.

### The concern is still right, so it is now enforced mechanically

"A point was created earlier" is a memory, not evidence — it could have been aged
out by shadow-storage pressure (19.1 GB max on this volume) or removed. So the
check no longer depends on anyone remembering to do it. `scripts/p41/stage2.ps1`
now opens with a preflight block that runs **before** `msiexec` is touched:

1. `Get-ComputerRestorePoint` and log every point (seq, type, time, description).
2. Accept only a point created within the last 24 h — one that still reflects this
   pre-install machine.
3. If there is none: enable System Restore, temporarily zero
   `SystemRestorePointCreationFrequency`, create the point, restore that value to
   exactly its prior state, then **re-enumerate** — because `Checkpoint-Computer`
   returning OK is not proof, the listing is.
4. If there is still none: log `PREFLIGHT=FAIL`, write
   `STAGE2_ABORTED_NO_RESTORE_POINT`, and **`exit 1` without installing.**

    ACTION=   Run scripts\p41\stage2.ps1 elevated. It verifies (or creates and
              verifies) a System Restore point, and only then installs
              out\release\AetherCore.msi with /qn /l*v, then runs the full Stage 2
              verification list.
    SNAPSHOT= System Restore point on C:. Currently SequenceNumber 1, "AetherCore
              baseline - before any install", 2026-09-02 01:39 local, enumerated.
              This machine has NO disk image and NO VM snapshot; this point is the
              only recovery that exists, which is why the script refuses to install
              without one. Shadow storage: 19.1 GB max on C:.
    EXPECTED= PREFLIGHT=PASS naming the seq and creation time. MSIEXEC_EXIT=0.
              INSTALL_FILE_COUNT=16. Service AetherCoreMaintenance LocalSystem,
              AUTO_START, RUNNING. Service SID UNRESTRICTED and Active. Pipe DACL
              O:<service SID> G:SY D:P(A;;FA;;;<service SID>)(A;;FR;;;AU)(A;;DC;;;AU)
              - the AU pair may render as (A;;0x12008b;;;AU), the SAME DACL, never
              report that as drift. Install-dir ACLs read-execute for Users and the
              service SID, no Users write. VCOMP140_PRESENT=True,
              LIBOMP_AARCH64_PRESENT=False (x64 is expected to differ here, §41.4).
              DEV_BINARY_IN_INSTALL_IMAGE=NO. Four verbs RETURNED; doctor returning
              the typed diagnostics.stateUnavailable is a PASS.
              ENGINE_LABEL=localModel, NOT ruleFallback.
    RECOVERY= msiexec /x <ProductCode> /qn, which Stage 5b exercises deliberately
              and which is proven on ARM64 across fourteen survivor checks. Behind
              that, the verified restore point named above. The uninstall path is
              NOT yet proven on THIS machine - proving it is what Stage 5b is for.
              If the preflight fails the script installs nothing, so there is
              nothing to recover from.
    BLAST=    A failed install can leave a partially-registered service or a
              half-populated Program Files. msiexec transactions roll back on
              failure and the /l*v log names the failing action. Worst case is an
              orphaned service registration, cleared with
              "sc delete AetherCoreMaintenance" plus removing the directory, or by
              the restore point. No user data is touched - the product writes only
              under Program Files and ProgramData. The boot path is not involved.

### How the next session runs it

Start an **elevated** PowerShell (Run as administrator), launch the tool from
there, and then:

```
powershell -NoProfile -ExecutionPolicy Bypass -File C:\dev\aethercore\phase21-workspace\scripts\p41\stage2.ps1
```

Results land in `C:\AetherCore-P41\logs\stage2.log` (override with `-OutDir`).
`scripts\p41\offline-verbs.ps1` is the §41.7 offline evidence run, kept alongside
it so the Stage 3 partial can be reproduced or re-measured with the service up.
## 20.1 DBT-P41-002 — `telemetry-once` returns all-zero cpu / empty storage with no fault

**Status: DIAGNOSED, NOT FIXED.** Diagnosis performed on macOS against `main`
at `e91f675`. Read the platform-independence table in §20.1.7 before acting on
any of it.

### Relationship to §DBT-P41-002 (open) above, at line 3521

That entry is the Windows session's **observation**; this section is the
**root-cause analysis** of it. The two were written in parallel and are
consistent — §20.1.3(b) predicted, from source alone and before this session
had seen the measurement, that `perProcessorBusyBp` must serialise as `[]` on
Windows rather than as 22 zeros. The recorded observation is
`perProcessorBusyBp []`. **The measured x64 binary therefore does correspond to
this source, and the analysis below is not stale.**

(Procedural note, for accuracy about how this was produced: this session's clone
was behind `origin/main` when it began, so the brief's "line 3521" resolved to
nothing locally and the analysis was carried out against source only, without
sight of the observation. That turned out to be a useful accident — the
`perProcessorBusyBp` prediction is a genuine out-of-sample confirmation rather
than a restatement. Nothing was overwritten; the observation entry is intact.)

### 20.1.1 Q1 — how many places decide whether a collector has a usable reading

**Nine, in three tiers that never consult each other.** No shared predicate
exists; every site re-derives availability by hand.

Tier 1 — inside the Windows provider, one ad-hoc rule per collector:

| # | site | what it decides | emits a fault? |
|---|---|---|---|
| 1 | `windows_impl.rs:135-142` cpu, query open | `QueryHandle::open()` returned `None` | yes, `Unavailable` |
| 2 | `windows_impl.rs:148-165` cpu, collect | either `PdhCollectQueryData` failed | yes, `ProviderFailure` |
| 3 | `windows_impl.rs:214` power | `if !buffer.iter().all(|b| *b == 0) || true` — the `|| true` makes this unconditionally true, so the `else` at `:256-262` is **dead code** and the power `Unavailable` fault is unreachable | no (unreachable) |
| 4 | `windows_impl.rs:288-294`, `:316-322` memory | `GlobalMemoryStatusEx` failure; PDH query-open failure | yes, `Unavailable` |
| 5 | `windows_impl.rs:328-335` storage | query open only | yes, `Unavailable` |
| 6 | `windows_impl.rs:437-443` gpu | `sample.engines.is_empty()` | yes, `Unavailable` |
| 7 | `windows_impl.rs:447-454` process_top | nothing — `let _ = faults;` then `Vec::new()`, unconditionally | no, by design |

Tier 2 — the presentation layer re-decides gpu availability a second time:

8. `apps/aetherctl/src/offline.rs:241` — `if snapshot.gpu.adapter_id.is_empty()
   && snapshot.gpu.engines.is_empty() { Null }`. This is a **second, independent
   gpu-availability rule** that the collector at `windows_impl.rs:437` knows
   nothing about. The service path does **not** have it:
   `services/maintenance-service/src/performance.rs:78-96` (`gpu_sample_proto`)
   passes gpu through unconditionally. So the CLI and the service already give
   different answers about gpu presence from the identical snapshot.

Tier 3 — a static table that can never disagree with anything, because it never
looks:

9. `crates/platform-capabilities/src/lib.rs:254-257` — `windows_table()` maps
   **every** capability, `telemetryCpu` / `telemetryStorage` / `telemetryGpu`
   included, to `Availability::Native` unconditionally. The `capabilities` verb
   therefore reports `telemetryStorage: native` in the same session in which
   `telemetry-once` returns `"storage": []`. Same truth, two verbs, opposite
   answers, neither aware of the other.

There is also a **second `CollectorFault` type**. `collector-runtime` defines a
typed one (`crates/collector-runtime/src/lib.rs:24` `FaultKind`, `:37`
`CollectorFault`) and the sibling crates use it through `Result<T,
CollectorFault>` (`crates/diagnostic-engine/src/lib.rs:108-109`,
`crates/hardware-telemetry/src/windows_impl.rs:93`). `performance-telemetry`
declares the dependency in its `Cargo.toml` but defines its own stringly-typed
`CollectorFault` at `crates/performance-telemetry/src/lib.rs:147-151` and
**never imports the shared one**: the only occurrence of `collector-runtime` in
the whole crate's source is the doc comment at `lib.rs:6`. That doc comment —
"every collector runs under a timeout via the shared `collector-runtime`
isolation gate" — is false for the Windows provider, which uses no gate.

### 20.1.2 Q2 — what gpu does that cpu and storage do not

**gpu is the only collector that checks its own output before returning it.**

`windows_impl.rs:437-443`:
```rust
if sample.engines.is_empty() {
    faults.push(CollectorFault { collector: "gpu", kind: "Unavailable", .. });
}
```

That is the entire difference, and it is available to gpu only because gpu's
payload is a **collection**, which has a distinguishable "I got nothing" state.

- **cpu** cannot express it. `CpuSample` is a struct of scalars
  (`lib.rs:57-63`); `sample_cpu` starts from `CpuSample::default()`
  (`windows_impl.rs:134`) and every field write is conditional
  (`if let Some(busy)` at `:172`, `.unwrap_or(0)` at `:175-176`, `if let Some`
  at `:178` and `:181`). A field that was never written is `0`, and `0` is a
  legal measurement. There is no post-hoc check because there is nothing to
  check against.
- **storage** *could* express it — it returns a `Vec`, exactly like gpu — but it
  has **no `is_empty()` check** and instead **four early returns that push no
  fault at all**:
  - `windows_impl.rs:338-340` second `collect()` failed → `return devices;`
  - `windows_impl.rs:353-355` `needed == 0` → `return devices;`
  - `windows_impl.rs:359-361` `PdhExpandWildCardPathW` failed → `return devices;`
  - `windows_impl.rs:364-367` the walk `break`s on a malformed list

  Contrast `:328-335`, where the *same function* does push a fault for a
  query-open failure. The fault discipline is applied to the first failure mode
  and abandoned for the next four.

### 20.1.3 Q3 — real zero, unwritten default, or swallowed error

**All three are present, in different fields. They are separable.**

**(a) `cpu.totalBusyBp`, `dpcIsrBusyBp`, `contextSwitchesPerSec`,
`processorQueueLengthX100` — none of the three. A fourth category: the PDH call
SUCCEEDS and its result is read from the wrong offset.**

`windows_impl.rs:110-124`:
```rust
fn read_u64(&self) -> Option<u64> {
    let mut value = 0i64;                       // 8 bytes
    ... PdhGetFormattedCounterValue(self.0, PDH_FMT_LARGE, null_mut(), &mut value)
```

`PdhGetFormattedCounterValue`'s last parameter is `PPDH_FMT_COUNTERVALUE`, not a
`*mut i64`. The real struct (`windows-0.62.2`
`src/Windows/Win32/System/Performance/mod.rs:9682-9685`, identical in
`windows-sys` 0.45/0.59/0.61) is:

```
PDH_FMT_COUNTERVALUE { CStatus: u32, Anonymous: union { longValue: i32,
                       doubleValue: f64, largeValue: i64, ...pointers } }
```

Layout measured, not assumed (`repr(C)`, 64-bit; scratchpad `layout.rs`):

```
size_of  = 16      align_of = 8      offset_of(largeValue) = 8
```

The code supplies an **8-byte** destination for a **16-byte** write. Two
consequences:

1. `value` receives bytes `0..8` of the struct — `CStatus` (4 bytes) plus 4
   bytes of padding — **never `largeValue`, which lands at `+8..16`**.
   `PDH_CSTATUS_VALID_DATA` is `0x00000000`, so a *successful* read yields
   `value == 0`. (`PDH_CSTATUS_NEW_DATA` is `0x1`, which would yield `1` bp =
   0.01%; still reads as zero in the report.)
2. PDH writes 8 bytes past the end of a stack local. This is a **stack buffer
   overflow and undefined behaviour**, inside an `unsafe` block, on every
   counter read. It is a memory-safety defect, not only a wrong number.

`pdh_ok(code)` is true throughout, so `read_u64` returns `Some(0)` — a
confident, well-formed, structurally-guaranteed zero. **No error exists to
swallow and no default is in play: the collector is reading a status code and
reporting it as a measurement.** This is why the reading is not merely wrong but
*implausibly* wrong: it is not 22 processors at 0%, it is `PDH_CSTATUS_VALID_DATA`
formatted as basis points.

Introduced in `4e31d60` (Phase 20, 2026-08-24) per `git blame -L 110,124`; lines
113-118 rewritten in `0e30038` without touching the defect. Present in every
build since.

Every caller of `read_u64` inherits this: cpu (`:167-183`), memory's
standby/modified/hard/soft counters (`:306-314`), storage's five per-device
counters (`:385-404`), gpu's utilization (`:429`). Memory looks healthy in the
report only because its headline fields
(`totalPhysicalBytes`/`availablePhysicalBytes`/`memoryLoadPercent`) come from
`GlobalMemoryStatusEx` at `:280-287`, not from PDH.

**(b) `cpu.perProcessorBusyBp` — genuinely default-initialised and never
written.** `sample_cpu` creates `CpuSample::default()` at `windows_impl.rs:134`
and **no line of `windows_impl.rs` ever assigns `per_processor_busy_bp`**
(`grep`: the only writes in the crate are `lib.rs:318` synthetic,
`linux_impl.rs:439`, `macos_impl.rs:159-165`). On Windows the field stays
`Vec::new()` and serialises as `[]`.

> **This is the sharpest discriminator available to the Windows session.** The
> brief describes "0% busy across 22 logical processors". This source cannot
> produce 22 zeros — it can only produce `[]`. If the Windows measurement shows
> `"perProcessorBusyBp": []`, the running binary matches this source and (a)+(b)
> are confirmed. If it shows 22 zero entries, **the measured binary was not
> built from `e91f675` and the whole diagnosis must be re-based on whatever it
> was built from.** Check this first.

**(c) `storage: []` — an error swallowed on the way up, and one of the four
silent returns is doing it.** Which one needs Windows measurement, but the
most likely is `windows_impl.rs:353-355` (`needed == 0`), because the pattern
handed to `PdhExpandWildCardPathW` at `:342-347` is broken in two independent
ways:

```rust
let pattern = windows::core::PCWSTR::from_raw(
    r"\PhysicalDisk(*)\% Disk Time\0"
        .encode_utf16().collect::<Vec<u16>>().as_ptr(),   // <-- temporary
);
```

1. **Dangling pointer.** The `Vec<u16>` is a temporary dropped at the end of the
   statement. `pattern` points into freed memory for both calls at `:351` and
   `:358`. UB.
2. **Not NUL-terminated.** The literal is a **raw** string (`r"..."`), so the
   trailing `\0` is the two characters backslash and zero, not a NUL. After
   `encode_utf16` there is no terminator at all — and the API additionally wants
   a double-NUL-terminated result buffer.

Either alone is sufficient to make `needed` come back `0`, at which point
`:354` returns an empty `Vec` with **no fault pushed**. Note `:350-351`'s own
comment — "Both failures are typed faults" — describes behaviour the function
does not implement.

### 20.1.4 Q4 — can a collector return Ok with an all-zero payload and no fault

**Yes, and it is unconditional, and the type system is where the defect lives.**

`crates/performance-telemetry/src/lib.rs:216-220`:
```rust
pub trait PerfPlatform: Send + Sync + 'static {
    fn sample(&self, interval: Duration) -> PerfSnapshot;
}
```

`sample` returns `PerfSnapshot`, **not `Result`**, and `PerfSnapshot` derives
`Default` (`lib.rs:153-165`). `PerfSnapshot::default()` — every scalar `0`, every
`Vec` empty, `collector_faults: vec![]` — is a fully valid, type-checking return
value indistinguishable from a real reading of an idle machine. Nothing in the
type, the trait, or `normalized()` (`lib.rs:170-196`, which clamps upper bounds
only and asserts no availability) requires a collector to have measured
anything, or to justify a zero.

This is the actual defect. Sites 1-9 in §20.1.1 are nine hand-written
compensations for a contract that never demanded the evidence in the first
place; gpu happens to have written one that works, storage wrote one that covers
one of five exits, cpu cannot write one at all, and power wrote one that `|| true`
made unreachable. **A fix at any call site leaves the other eight free to
regress.**

The shape to fix toward already exists in this repository and this crate already
depends on it: `collector-runtime`'s `Result<T, CollectorFault>` with a typed
`FaultKind` (`crates/collector-runtime/src/lib.rs:24,37`), as used by
`diagnostic-engine` (`src/lib.rs:108-109`) and `hardware-telemetry`
(`src/windows_impl.rs:93`). `performance-telemetry` is the one crate in the
family that opted out while still declaring the dependency.

### 20.1.5 Q5 — does any test exercise the real collector path on the real OS

**No test anywhere constructs `WindowsPerfPlatform`.** Workspace-wide
`grep -rn WindowsPerfPlatform --include='*.rs' apps services crates` returns
four hits, all of them the definition and its own re-export
(`crates/performance-telemetry/src/windows_impl.rs:456,458`,
`src/lib.rs:226,245`). Zero in any `tests/`, `benches/`, or `examples/`.

The three test files in the crate:

- `tests/adversarial.rs` — 8 tests, all `SyntheticPerfPlatform` or
  hand-constructed snapshots (`:140,155,177`).
- `tests/native_providers.rs` — real providers for macOS (`:78`) and Linux
  (`:125`) only; `#[cfg]`-gated, so on Windows the file compiles to the single
  synthetic test at `:237`.
- `tests/phase27_real_sample.rs` — the one real-provider pipeline test, macOS
  only (`:19-20`).

And the real-provider test that *does* exist would not have caught this anyway.
`phase27_real_sample.rs:41-43` asserts **upper bounds only**:
```rust
assert!(aggregate.cpu_busy_bp_avg  <= 10_000);
assert!(aggregate.cpu_busy_bp_peak <= 10_000);
```
An all-zero CPU satisfies both. No lower bound, no plausibility check, no
assertion that `collector_faults` is empty *or* non-empty.

The end-to-end CLI test has the same shape.
`apps/aetherctl/tests/phase28_cli_matrix.rs:177-181` runs the real
`telemetry-once` verb and asserts:
```rust
assert!(envelope["data"]["cpu"]["totalBusyBp"].is_u64());
```
`0.is_u64()` is `true`. **The only end-to-end test of the defective verb passes
on the defective output** — the §17.7 "eight tests passed without one traversing
the real writer" pattern, recurring exactly.

### 20.1.6 Why gpu "got it right" — it did not, it got lucky

Worth recording so the fix is not modelled on gpu. gpu declared `Unavailable`
because `add_english_counter` at `windows_impl.rs:426` returned `None` for the
wildcard path `\GPU Engine(*engtype_3D)\Utilization Percentage` —
`PdhAddEnglishCounterW` does not accept an unexpanded wildcard instance — so
`eng_util` was `None`, the `.map` at `:427` never ran, `engines` stayed empty,
and `:437` fired. **Had the counter added successfully, `read_u64` would have
returned `Some(0)` from the same misread as everything else, an engine would
have been pushed with `utilization_bp: 0`, `engines` would be non-empty, and gpu
would have reported a confident 0% with no fault — exactly like cpu.** gpu's
correct-looking output is the accidental product of an earlier, unrelated
failure. Do not treat `:437` as the reference implementation.

### 20.1.7 Platform independence — what needs re-measuring on Windows

| finding | class | needs Windows? |
|---|---|---|
| Nine availability decision sites, §20.1.1 | code fact, platform-independent | no |
| `capabilities` says `telemetryStorage: native` unconditionally (`platform-capabilities/src/lib.rs:254-257`) | code fact | no |
| CLI-vs-service gpu divergence (`offline.rs:241` vs `performance.rs:78`) | code fact | no |
| `PerfPlatform::sample` returns `PerfSnapshot`, not `Result` (`lib.rs:219`) | code fact — **the defect** | no |
| `PerfSnapshot: Default` makes all-zero-no-fault legal (`lib.rs:153`) | code fact | no |
| gpu is the only collector checking its own output (`:437`) | code fact | no |
| storage's four fault-free early returns (`:339,354,360,366`) | code fact | no |
| `sample_power`'s `\|\| true` dead else (`:214` / `:256`) | code fact | no |
| `per_processor_busy_bp` never written on Windows | code fact | no |
| `PDH_FMT_COUNTERVALUE` is 16 bytes, `largeValue` at offset 8 | ABI fact, verified against the vendored `windows` crate + a measured `repr(C)` layout; identical on x64 and ARM64 (both LP64/LLP64, 8-byte union alignment) | no |
| No test constructs `WindowsPerfPlatform`; both real-path tests assert upper bounds only | code fact | no |
| **Which** of storage's four exits fires | runtime | **yes** |
| Whether `read_u64` returns 0 (`VALID_DATA`) or 1 (`NEW_DATA`) per counter | runtime | **yes** |
| ~~Whether `perProcessorBusyBp` is `[]` or 22 zeros~~ | **RESOLVED** — the entry at line 3521 records `perProcessorBusyBp []`. Source matches the measured binary. | no, already measured |
| Whether the stack overflow at `:110-124` corrupts anything observable, or is absorbed by stack padding | runtime | **yes** |
| Whether `PdhAddEnglishCounterW` rejects the gpu wildcard as §20.1.6 predicts | runtime | **yes** |

Nothing here was reproduced on macOS: `windows_impl.rs` is `#[cfg(windows)]` and
does not compile on this host. Every "no" above is read from source and from the
vendored Win32 bindings, not from execution.

### 20.1.8 Not established

- The precise `CStatus` value each counter returns, hence whether the reported
  zeros are literally `0` or occasionally `1`. Requires Windows.
- Whether any consumer downstream of the ring (bottleneck analyzer, desktop
  renderer) has a *tenth* availability rule. Only the CLI and service paths were
  traced.
- ~~Whether the x64 machine's binary corresponds to `e91f675`.~~ **Resolved** by
  the `perProcessorBusyBp []` observation at line 3521 — see the note under
  §20.1's heading. This is the one item that was open at first writing and
  closed on rebase.

### 20.1.9 What this settles in the observation entry's two open items

The entry at line 3521 deliberately declined to claim two things. Both are now
answerable as code facts, without further measurement:

1. **"The service path may well populate these collectors."** It will not. There
   is exactly one construction site for the provider in each binary and they are
   the same call: `services/maintenance-service/src/composition.rs:118` and
   `apps/aetherctl/src/offline.rs:199` both invoke
   `aethercore_performance_telemetry::default_platform()`, which on Windows
   returns `WindowsPerfPlatform` (`lib.rs:242-246`) — the same `read_u64`
   misread (§20.1.3(a)) and the same fault-free storage returns (§20.1.3(c)).
   The service-backed `perf snapshot` will show the identical zeros. The
   finding is **not** scoped to `telemetry-once`; that verb is only where it was
   first seen. Worth running `perf snapshot` after the install anyway, to
   confirm the prediction rather than to discover the answer.
2. **The `capabilities` divergence.** Not a mis-report to be re-checked with the
   service running, but a static table: `windows_table()` at
   `crates/platform-capabilities/src/lib.rs:254-257` maps every capability to
   `Availability::Native` unconditionally, with no runtime input at all. It will
   report `telemetryCpu`/`telemetryStorage`/`telemetryGpu` as `native` on every
   Windows host in every state, including hosts where the collectors are
   genuinely absent. This is decision site 9 of §20.1.1 and it is the same
   "five of sixteen capabilities mis-reported" shape the entry correctly
   recognised.

### 20.1.10 Scope note

Diagnosis only, per the brief. **Nothing was fixed.** No source file was
modified; the only change in this commit is this section. The fix belongs at
`lib.rs:219` (the trait's return type), not at any of the nine call sites — see
§20.1.4.

## 41.11 DESTRUCTIVE ACTION RECORD — Gate 0f: full system image to the external drive

Written and committed **before** the first elevated action of this session, per
the standing rule. This is the action §41.2 0e recorded as impossible: an
external drive is now attached, so it becomes possible, and it is being taken
BEFORE the first install rather than before Stage 4 — the pristine, nothing-ever-
installed state cannot be recreated once Gate 2 runs.

### 0f.1 — target identified by measurement, not assumption (2026-09-02)

Session elevation confirmed first:
`([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole(544)` -> `True`.
`core.autocrlf` re-checked: `false`.

Every volume on the machine, verbatim:

    DriveLetter FileSystemLabel FileSystem DriveType          Size SizeRemaining
    ----------- --------------- ---------- ---------          ---- -------------
              C                 NTFS       Fixed     1023232962560  450525421568
                                NTFS       Fixed         852488192      75468800
                                FAT32      Fixed         100663296      64387072
              D SD              NTFS       Fixed     1023998423040 1023859712000
                EFI             FAT32      Fixed         206472192     206471680

Disk topology, which is what disambiguates the two lettered volumes:

    Number FriendlyName                   BusType PartitionStyle          Size
         1  HIKSEMI                       USB     GPT            1024209543168
         0 NVMe Micron_2500_MTFDKBA1T0QGN NVMe    GPT            1024209543168

    DiskNumber PartitionNumber DriveLetter          Size Type
             1               1                 209715200 System     <- the "EFI" row
             1               2           D 1023998427136 Basic      <- the target
             0               1                 104857600 System
             0               2                  16777216 Reserved
             0               3           C 1023232966656 Basic
             0               4                 852492288 Recovery

So of the five volumes: three belong to the system disk 0 (C:, the 852 MB
Recovery/WinRE volume, the 100 MB EFI system partition), and two belong to the
USB disk 1 (a 200 MB EFI-type partition carried on the external, and D:).
**Exactly one non-system volume is a usable NTFS target: D:.**

The two numbers the gate asks for explicitly:

    C_USED_BYTES = 572707504128   (533.38 GB)
    D_FREE_BYTES = 1023859712000  (953.54 GB)

953.54 GB free > 533.38 GB used. EXPECTED met.

D: is NTFS, so the "not NTFS -> STOP" branch does not fire. Its top level was
enumerated with `-Force` before anything was written to it:

    Mode   LastWriteTime        Length Name
    d--hs- 9/2/2026 12:13:42 PM        System Volume Information
    count = 1

That is the single OS-created folder every NTFS volume carries, not owner data,
so the "already contains data -> STOP for confirmation" branch does not fire
either. Recorded rather than glossed, because the rule is never to write to a
drive whose contents have not been enumerated.

### 0f.2 — the tool exists on this SKU

`wbadmin /?` responds with its command list (ENABLE/DISABLE BACKUP, START BACKUP,
STOP JOB, GET VERSIONS, GET ITEMS, GET STATUS, DELETE BACKUP). EXPECTED met.
Note the process exit code for `/?` is 255; the gate's criterion is that it
responds with its command list, which it does. SKU is `Microsoft Windows 11 Pro`
and `wbadmin start backup /?` confirms this build accepts every flag the gate
uses: `-backupTarget`, `-include`, `-allCritical`, `-vssFull|-vssCopy`, `-quiet`.

### The record

    ACTION=   wbadmin start backup -backupTarget:D: -include:C: -allCritical
              -quiet, creating a full block-level image of the system disk on
              the external USB drive while this machine is still in its
              never-had-anything-installed state. Then verify it by LISTING it
              (wbadmin get versions + sizing WindowsImageBackup), not by
              trusting the success message. Then record WinRE status, because a
              bare-metal restore of this image needs bootable media.
    SNAPSHOT= What exists to fall back on right now: System Restore point
              SequenceNumber 1, "AetherCore baseline - before any install",
              CreationTime 20260901223933.946086-000, enumerated at Gate 0
              (§41.2). That covers registry and drivers. It does NOT cover a
              machine that will not boot, which is the gap this image closes.
              There is no VM snapshot and, until this action completes, no disk
              image.
    EXPECTED= wbadmin exit 0. `wbadmin get versions -backupTarget:D:` lists at
              least one version dated today. D:\WindowsImageBackup exists and
              its recursive size is a plausible fraction of the 533.38 GB used
              on C: (block-level, used-blocks-only, so materially less than
              533 GB is expected and correct; near-zero is not).
              **An empty version list FAILS this gate regardless of exit code.**
              `reagentc /info` reports `Windows RE status: Enabled`.
    RECOVERY= This action is additive to D: and read-only with respect to C:.
              wbadmin writes into D:\WindowsImageBackup and does not format the
              target (formatting only happens for a scheduled backup to a
              dedicated disk, which is not what is being run). To undo: delete
              D:\WindowsImageBackup, or `wbadmin delete backup`. Nothing on C:
              is modified, so C: needs no recovery from this step.
    BLAST=    Bounded by free space on D: (953.54 GB free against at most
              533.38 GB of used blocks, so exhaustion is not reachable). A VSS
              snapshot is taken on C: transiently, which consumes shadow storage
              (19.1 GB max configured) and is released at the end; the existing
              restore point lives in that same store, so the realistic worst
              case is shadow-storage pressure aging out SequenceNumber 1. That
              is checked again after the image completes rather than assumed.
              The run is long and prints almost nothing - it is not hung, and
              killing it is the one action that could leave a partial image.

## 41.12 GATE 0f — RESULT: **PASS as an image, FAIL as a pristine capture** (2026-09-02)

The image exists, is verified, and is bare-metal capable. But the state it
captured is **not** the state the gate was written to capture, and that is the
more important half of this record. Both halves below.

### 0f.3 — the run, and an interruption that was not a failure

Started 12:45:49 local. `wbadmin` reported it would back up
`(EFI System Partition),(C:),(\\?\Volume{1a9456f9-efe9-4547-acc2-bb080f4df242}\)`
to D: — i.e. `-allCritical` resolved to all three critical volumes, not just C:.

**At 48% of the C: volume the `wbadmin.exe` console process was terminated.** Not
by an operator decision and not by this session. What matters for the gate is
that it did not stop the backup, and this is worth recording because the brief's
"do not kill it" warning implies the client is the job, and it is not:
`wbadmin start backup` is only a client of the Block Level Backup Engine
service. Measured evidence the work continued:

- `wbengine` PID 1244, StartTime 12:45:49 — the backup's own start moment —
  still `Running` long after the client died
- bytes written to D: kept climbing after the client was gone: 268.85 GB at
  13:28, 277.88 at 13:30, 383.29 at 13:50, 518.26 at 14:20
- `wbadmin get status` did not return within 120 s, which is what it does only
  while an operation is live
- 7 VSS shadow copies present during the run

**Consequence, recorded rather than papered over: the client's exit code is
lost.** The gate asks for `wbadmin` exit 0 and no process survived to report
one. This does not decide the gate — 0f.4 already says a success message is not
proof and the listing is — but the exit-code line of the gate is UNMEASURED and
is not being inferred from the outcome. The authoritative result comes from the
`Microsoft-Windows-Backup` log instead:

    [12:46:09] Id=1  Information  The backup operation has started.
    [14:21:59] Id=4  Information  The backup operation has finished successfully.
    [14:21:59] Id=14 Information  The backup operation has completed.

Duration 12:45:49 -> 14:21:59, about 96 minutes, ~105 MB/s sustained.

### 0f.4 — VERIFIED by listing, not by message

    wbadmin get versions -backupTarget:D:        (exit 0)

    Backup time: 9/2/2026 12:46 PM
    Backup target: 1394/USB Disk labeled SD(D:)
    Version identifier: 09/02/2026-09:46
    Can recover: Volume(s), File(s), Application(s), Bare Metal Recovery, System State
    Snapshot ID: {ef959cc8-bb84-406c-aeec-663c4ea0c02d}

One version, dated today, **Bare Metal Recovery** among its capabilities.
EXPECTED met — the empty-version-list FAIL branch did not fire.

Sizing:

    WIB_FILE_COUNT = 19
    WIB_BYTES      = 559904433918   (521.45 GB)
    C_USED         = 572707504128   (533.38 GB)
    RATIO          = 97.8% of C: used

Three VHDXs, one per critical volume, which is what confirms `-allCritical`
actually did what its output claimed:

    18017d1f-43dc-4d1f-bf84-47988cadd451.vhdx   558992719872   C:
    1a9456f9-efe9-4547-acc2-bb080f4df242.vhdx      805306368   recovery volume
    Esp.vhdx                                        98566144   EFI system partition

97.8% is a plausible fraction and not a near-zero stub. EXPECTED met.

### 0f.5 — WinRE, and the media that does not exist

    Windows RE status:         Enabled
    Windows RE location:       \\?\GLOBALROOT\device\harddisk0\partition4\Recovery\WindowsRE
    BCD identifier:            273ec769-6375-11f0-8a21-b70501fe26f2
    Windows RE Version:        10.0.26100.9168
    REAGENTC.EXE: Operation Successful.

EXPECTED met. **Flagged for the owner, not acted on:** WinRE being enabled means
recovery boots from the local disk. Restoring this image onto a machine that
will not boot still needs external bootable media, which does not exist. Per the
gate, no recovery media was created.

### 0f.6 — the recovery that already existed survived the run

Checked because the image run takes its own VSS snapshot on C: and the only
pre-existing recovery lives in that same 19.1 GB store:

    seq=1 type=12 created=20260901223933.946086-000 desc=AetherCore baseline - before any install
    seq=2 type=15 created=20260902094609.175829-000 desc=Windows Backup

    Used Shadow Copy Storage space:      5.59 GB (0%)
    Allocated Shadow Copy Storage space: 6.21 GB (0%)
    Maximum Shadow Copy Storage space:   19.1 GB (1%)

SequenceNumber 1 is intact. seq=2 is one the backup created for itself. No
pressure: 5.59 GB used against a 19.1 GB cap.

### 0f.7 — THE FINDING: the machine was not pristine when it was imaged

The gate's stated rationale is *"nothing has ever been installed on this machine.
That is the single most valuable state to capture and it cannot be recreated
after the first install."* **That premise was already false when the brief was
read.** Measured, not inferred:

    HKLM\...\Uninstall\{0F9F349D-01C8-B3C2-7242-83B5D29047C9}
      DisplayName    : AetherCore
      DisplayVersion : 0.1.11
      InstallDate    : 20260902
      InstallSource  : C:\dev\aethercore\phase21-workspace\out\release\

    HKLM\SOFTWARE\AetherCore  InstallVersion : 0.1.11
    C:\Program Files\AetherCore  CreationTime : 9/2/2026 12:09:11 PM  (16 files)
    sc query AetherCoreMaintenance -> STATE : 4 RUNNING

Application log, MsiInstaller:

    [12:09:09] Id=1040  Beginning a Windows Installer transaction:
               C:\dev\aethercore\phase21-workspace\out\release\AetherCore.msi.
               Client Process Id: 20376.
    [12:09:32] Id=11707 Product: AetherCore -- Installation completed successfully.
    [12:09:32] Id=1033  Product Name: AetherCore. Product Version: 0.1.11.
               Installation success or error status: 0.
    [12:09:33] Id=1042  Ending a Windows Installer transaction: ...AetherCore.msi.

And `C:\AetherCore-P41\logs\stage2.log` opens with
`STAGE 2 INSTALL+VERIFY 2026-09-02T12:09:07.8156564+03:00`.

**So Stage 2 ran at 12:09:07, and the image began at 12:45:49 — 37 minutes
later.** The timeline is unambiguous:

    2026-09-01 22:39:33 UTC   restore point seq=1 created, machine pre-install
    2026-09-02 12:09:07 local stage2.ps1 runs, MSI installs, service starts
    2026-09-02 12:41-12:45    this session's first commands (elevation, volumes)
    2026-09-02 12:45:49       image starts
    2026-09-02 14:21:59       image finishes

This session did not install anything and did not run stage2.ps1. The install
predates its first command. §41.10's statement that *"Nothing has been installed
since. Re-verified after the session pause: `C:\Program Files\AetherCore` does
not exist and `sc query AetherCoreMaintenance` returns `FAILED 1060`"* was true
when written and is **now stale**.

**What this costs, stated plainly:** the image is a faithful capture of a machine
with AetherCore 0.1.11 installed and running. It is NOT the never-installed
capture the gate wanted, and per the gate's own reasoning that state is gone and
cannot be recreated. The ordering instruction "do it BEFORE the install, not
before Stage 4" could not be honoured because the install had already happened.

**What it does not cost:** everything the image was needed *for* downstream is
intact. Stage 4 driver work required a verified disk image before it starts;
that now exists, is bare-metal capable, and is verified by listing. Recovery to
the pre-install machine is still available through restore point seq=1, which
0f.6 confirms survived. Nothing about Gate 4's precondition depends on the image
having been taken before Gate 2.

**The standing lesson**, since this is the second time a recorded machine state
has gone stale under this project: a state assertion in this document is
evidence of what was true when it was written, never of what is true now. §41.10
was right to make the restore point a mechanical preflight rather than a
remembered fact. The same reasoning applies to "nothing is installed" — it was
load-bearing for Gate 0f's rationale and it was carried as prose.

### Gate 0f verdict

| item | expected | observed | result |
|---|---|---|---|
| 0f.1 target | one non-system NTFS vol, free > C: used | D:, 953.54 GB free > 533.38 GB used | PASS |
| 0f.2 tool | wbadmin lists commands | listed, all needed flags present | PASS |
| 0f.3 exit code | 0 | **client killed at 48%, exit code lost** | UNMEASURED |
| 0f.3 completion | - | Backup log Id=4 "finished successfully" 14:21:59 | PASS |
| 0f.4 versions | >=1 version dated today | 1 version, 9/2/2026 12:46 PM, Bare Metal Recovery | PASS |
| 0f.4 size | plausible fraction of C: used | 521.45 GB = 97.8%, 3 VHDXs | PASS |
| 0f.5 WinRE | Enabled | Enabled, 10.0.26100.9168 | PASS |
| 0f.6 restore pt | seq=1 survives | seq=1 intact, 5.59/19.1 GB shadow used | PASS |
| 0f.7 pristine | machine never installed | **AetherCore 0.1.11 installed 12:09:11, 37 min before the image** | **FAIL** |

## 41.13 GATE 0f RE-VERIFIED after the owner's format — **PASS, image intact** (2026-09-02)

`P41-FULL-RUN.md` warns that after the image job started, the owner formatted a
flash drive as NTFS and may have targeted the same device, and instructs that the
image's survival must not be assumed. Re-verified from scratch at 14:5x.

### 0f.A — what is attached now, identified by FriendlyName and size, not letter

**A third disk has appeared since 0f.1.** The machine now has:

    Number FriendlyName                   BusType          Size PartitionStyle SerialNumber
         0 NVMe Micron_2500_MTFDKBA1T0QGN NVMe    1024209543168 GPT            0000_..._807B
         1 HIKSEMI                        USB     1024209543168 GPT            740200010004
         2 ADATA USB Flash Drive          USB       31037849600 MBR            AA00000000000489

    DriveLetter FileSystemLabel FileSystem DriveType          Size SizeRemaining
              C                 NTFS       Fixed     1023232962560  449123254272
                                NTFS       Fixed         852488192      73617408
              E RECOVERY        FAT32      Removable   31020023808   24601329664
                                FAT32      Fixed         100663296      64387072
              D SD              NTFS       Fixed     1023998423040  440769064960
                EFI             FAT32      Fixed         206472192     206471680

EXPECTED met: disk 0 is the NVMe system disk, and USB disks are present.

**The format did NOT hit the image target.** The formatted device is disk 2, a
31 GB ADATA flash drive that was not attached at 0f.1 — a new device, its own
serial `AA00000000000489`, MBR, carrying `E: RECOVERY`. The image lives on disk 1
(HIKSEMI, serial `740200010004`, GPT, 953 GB), which is a different physical
device by every identifier. D: free space fell from 953.54 GB to 410.50 GB,
which is the image being written, not a format.

One correction to the brief's premise, recorded because it matters for what E:
actually is: **E: is FAT32, not NTFS.** Its type is `FAT32 XINT13` on an MBR
disk with `IsActive = True` — that is the layout Windows produces for a
*recovery drive*, not for a general-purpose NTFS format.

### 0f.B — the image is intact, byte-for-byte unchanged

    wbadmin get versions -backupTarget:D:        (exit 0)

    Backup time: 9/2/2026 12:46 PM
    Backup target: 1394/USB Disk labeled SD(D:)
    Version identifier: 09/02/2026-09:46
    Can recover: Volume(s), File(s), Application(s), Bare Metal Recovery, System State
    Snapshot ID: {ef959cc8-bb84-406c-aeec-663c4ea0c02d}

    WIB_FILE_COUNT = 19
    WIB_BYTES      = 559904433918   (521.45 GB, 97.8% of the 533.38 GB used on C:)

Identical to the figures recorded in §41.12 before the format — same file count,
same byte total to the byte, same snapshot ID. D: top level:

    d--hs-  9/2/2026 2:21:53 PM  System Volume Information
    d-----  9/2/2026 12:46:44 PM WindowsImageBackup

### 0f.C — which of the three cases this is

**"It completed and is intact."** Not still running, not interrupted, not
reformatted. The Backup log's terminal event (§41.12) and the unchanged listing
above settle it from both ends.

### 0f.D — recovery media: the owner has now created it

    Windows RE status:         Enabled
    Windows RE location:       \\?\GLOBALROOT\device\harddisk0\partition4\Recovery\WindowsRE

Recorded as the brief requires: **WinRE lives on disk 0 partition 4. It recovers a
machine that still boots. It does not help a machine that will not boot**, which
is precisely the Stage 4 driver failure mode.

**New fact, and it is the one that moves Gate 4:** the flash drive the owner
formatted is not a scratch volume — it is bootable recovery media, built at
14:01-14:02 today. The boot chain is present on E:

    E:\bootmgr               True   (490606 bytes)
    E:\boot\bcd              True
    E:\sources\boot.wim      True
    E:\efi\boot\bootx64.efi  True
    partition IsActive       True
    E: used                  6.12 GB

Both the BIOS path (`bootmgr` + `boot\bcd`, active partition) and the UEFI path
(`efi\boot\bootx64.efi`) are populated, so this should boot on this UEFI machine.

**Stated precisely, because the difference matters:** what is verified is that
the recovery-media *artifacts exist and are complete*. What is NOT verified is
that the machine actually boots from it — that requires booting the machine from
E:, which this session cannot do and which the brief assigns to the owner. Gate
4's precondition should be treated as satisfied only once the owner has booted it
once and confirmed it reaches the recovery environment.

### Gate 0f — FINAL

| item | expected | observed | result |
|---|---|---|---|
| 0f.A attached | disk 0 NVMe, USB present | 3 disks; image target disk 1 HIKSEMI distinct from formatted disk 2 ADATA | PASS |
| 0f.B versions | >=1 version today | 1 version, 9/2/2026 12:46 PM, Bare Metal Recovery, snapshot {ef959cc8-...} | PASS |
| 0f.B size | plausible fraction of 533.38 GB | 521.45 GB / 19 files / 97.8%, unchanged after the format | PASS |
| 0f.C state | determine which case | completed and intact | PASS |
| 0f.D WinRE | Enabled | Enabled, disk 0 partition 4; does not cover an unbootable machine | PASS |
| 0f.D media | flag, do not create | owner created it: E: RECOVERY, full boot chain, 6.12 GB, active — **boot-test still outstanding** | RECORDED |
| 0f.3 exit code | 0 | client killed at 48%, exit code lost (§41.12) | UNMEASURED |
| 0f.7 pristine | never installed | AetherCore 0.1.11 installed 12:09:11, 37 min before the image (§41.12) | FAIL |

**Gate 0f is PASS for every purpose Gates 2-4 depend on.** The two non-PASS rows
are honest debts, not blockers: the lost exit code is superseded by the listing,
which the gate itself calls the proof; and the lost pristine state costs the
never-installed capture but not the verified-image precondition Stage 4 needs.

## 41.14 GATE 2 — RESULT: **PASS** on real x86_64 silicon (2026-09-02)

### How this gate was measured, and the one deviation from the brief

`P41-FULL-RUN.md` says to run `stage2.ps1`. **It was not re-run, deliberately.**
Per §41.12 0f.7 the install it performs had already happened at 12:09:07 today —
by the previous session the brief itself describes as having hit its usage limit
mid-gate. That session installed and verified successfully but never committed
its results, which is exactly the loss mode the brief's "commit after every item"
rule exists to prevent.

Re-running it would drive `msiexec /i` over a live, working install: it mutates
proven-good state and yields no measurement that cannot be taken read-only. So
every Stage 2 criterion was instead verified **live against the running system**,
which is strictly stronger evidence than reading a log this session did not
produce. Script: `scratchpad/verify-stage2.ps1`, read-only, installs nothing.
Log: `C:\AetherCore-P41\logs\gate2-live-verify.log`.

The install transaction itself is evidenced independently of any script, by the
Windows Installer's own log (§41.12 0f.7): MsiInstaller `1033`, *"Product Name:
AetherCore. Product Version: 0.1.11. Installation success or error status: 0"*,
from `InstallSource C:\dev\aethercore\phase21-workspace\out\release\`, whose MSI
hashes to `d18d89db07180f5727b7d6056a07ea8d50de97aa401838601b532e9befd1f227` —
the Gate 1 artifact, unchanged.

**Tamper check across the gap.** The 16 installed files' hashes from the 12:09
install log and from the live verify at ~15:0x were diffed row by row:
`rows_1209=16  rows_live=16  HASH_SET_IDENTICAL=YES`. Nothing changed on disk
between the install and this verification.

### The numbers

    INSTALL_FILE_COUNT=16
      aethercore-consent-broker.exe            633856  43d8af2a0f94e750721a44f40258c26aaee5e71a43497063aaf799dffbadf390
      aethercore-desktop.exe                  6959616  dcf1d16aa79241412c5a5215277033dc6a962fdbbefbdac6a7eca3c0840edeea
      aethercore-install-hardener.exe          273408  6722f12ac7541db694e87960c2537b021db3154e0d6f3c024b4fd6fc50a2bff5
      aethercore-maintenance-service.exe     10678784  221e486166707abbfe48af73796698feeb1d0233bde2fd4fc7572ae864a761d4
      aethercore-update-broker.exe             736768  d73b38c529b01c76c3c3a48ab48831d8094dd026b270a5d0065b46e0aa43d74c
      aetherctl.exe                           4315648  910df7c9ea1010285320abbc3fffbc8469d5c8139c555b228e45151a6c13813d
      Apache-2.0.txt                            11358  cfc7749b96f63bd31c3c42b5c471bf756814053e847c10f3eb003417bc523d30
      cis_map.json                               3103  9caf01b4a2f7d2bfda3111395212b27046f6ae614bc847cebaadfe33c9ee8d97
      models.manifest.json                        898  070b6dedc37664250e4029b8360a1e9b30a1d40b6d776a83ddd0631247dae57e
      qwen2.5-1.5b-instruct-q4_k_m.gguf    1117320736  6a1a2eb6d15622bf3c96857206351ba97e1af16c30d7a74ee38970e434e9407e
      Qwen-GGUF-NOTICE.txt                      11343  832dd9e00a68dd83b3c3fb9f5588dad7dcf337a0db50f7d9483f310cd292e92e
      UNINSTALL.txt                              3206  34f5f10357de5b7cb475f9b016ebac138f6230d756ffbc837e9d2fe7b02a0ad4
      update-trust.json                            83  d4ad925d86f64560bd80c77eae8c606fe810c5f670a7cd42df0836b0653c8b37
      vcomp140.dll                             193152  55aba23cdcd6484fbb06f4155b8ca75adfce7a881f10afd0c49457165e677164
      vulndb.json                                6704  ab76528eacc58fe910d82347d49919d50949954a25649e677eaaf52fdf37f303
      vulndb.manifest.json                        142  2c29c19b2760167fab8b292dde744da51ae8b137a01c870701287fb49d01daa6

    VCOMP140_PRESENT=True
    LIBOMP_AARCH64_PRESENT=False          <- x64 differs from ARM64 here by design, per §41.4
    DEV_BINARY_IN_INSTALL_IMAGE=NO
    CTL_SHA256=910df7c9ea1010285320abbc3fffbc8469d5c8139c555b228e45151a6c13813d

Service:

    STATE              : 4  RUNNING
    START_TYPE         : 2   AUTO_START  (DELAYED)
    BINARY_PATH_NAME   : "C:\Program Files\AetherCore\aethercore-maintenance-service.exe"
    SERVICE_START_NAME : LocalSystem
    SERVICE_SID_TYPE   : UNRESTRICTED
    SERVICE SID        : S-1-5-80-4285065559-3530017622-2858480679-3751456793-1187574229
    STATUS             : Active
    sdshow             : D:(A;;CCDCLCSWRPWPDTLOCRSDRCWDWO;;;SY)(A;;CCDCLCSWRPWPDTLOCRSDRCWDWO;;;BA)
                         (A;;CCLCSWLOCRRC;;;AU)S:(AU;FA;CCDCLCSWRPWPDTLOCRSDRCWDWO;;;WD)

Install-dir ACLs — protected, no Users write:

    C:\Program Files\AetherCore NT SERVICE\AetherCoreMaintenance:(OI)(CI)(RX)
                                BUILTIN\Users:(OI)(CI)(RX)
                                BUILTIN\Administrators:(OI)(CI)(F)
                                NT AUTHORITY\SYSTEM:(OI)(CI)(F)

Registration:

    ARP: {0F9F349D-01C8-B3C2-7242-83B5D29047C9} | AetherCore | 0.1.11 | InstallDate=20260902
    HKLM_InstallVersion=0.1.11

### The pipe DACL, checked against the criterion rather than eyeballed

    PIPE_PRESENT=True
    PIPE_SDDL=O:S-1-5-80-4285065559-3530017622-2858480679-3751456793-1187574229
              G:SY
              D:P(A;;0x12008b;;;AU)(A;;FA;;;S-1-5-80-4285065559-3530017622-2858480679-3751456793-1187574229)

Required:

    O:<service SID> G:SY D:P(A;;FA;;;<service SID>)(A;;FR;;;AU)(A;;DC;;;AU)

Equivalence, term by term, so nobody has to take this on trust:

- owner = the service SID -> matches
- group = `SY` -> matches
- `D:P` -> protected DACL, matches
- `(A;;FA;;;<service SID>)` -> present verbatim
- the AU pair is rendered merged as `(A;;0x12008b;;;AU)`. `FR = 0x120089`,
  `DC = 0x2`, `FR|DC = 0x12008b`. This is the rendering the brief names
  explicitly and instructs must NOT be reported as drift.
- ACE **ordering** differs from the authored string (AU first, then the service
  SID). A DACL is a set of ACEs with rights; the set and the rights are
  identical, and no principal gains or loses anything. Recorded for precision,
  **not** reported as drift.

**Correction to the brief, worth carrying forward.** `P41-FULL-RUN.md` says to
read the DACL with `[System.IO.File]::Open('\\.\pipe\...')` then
`.GetAccessControl()`. That method **fails on this machine**:

    PIPE_SDDL_FILESTREAM=ERROR: Exception calling "Open" with "4" argument(s):
    "FileStream was asked to open a device that was not a file. For support for
    devices like 'com1:' or 'lpt1:', call CreateFile, then use the FileStream
    constructors that take an OS handle as an IntPtr."

Both documented methods were tried in the same run. `NamedPipeClientStream` is
the one that works — which is what §16's existing correction already recorded,
and the full-run brief reintroduced the broken method. Use
`NamedPipeClientStream`.

### Verbs, against the RUNNING SERVICE

    servicedetect   EXIT 0  {"ok":true,"data":{"state":"Reachable","endpointDir":"C:\\ProgramData\\AetherCore",...}}
    doctor          EXIT 5  {"ok":false,"error":{"kind":"RejectedByService",
                             "message_key":"diagnostics.stateUnavailable",...}}   <- typed rejection = PASS by design
    optimizestatus  EXIT 0  {"ok":true,"data":{"status":null}}
    scanstatus      EXIT 0  {"ok":true,"data":{"appVersion":"0.1.11","state":"idle",
                             "ruleEngineVersion":"phase17.1-rules-v2",...}}
    insightslist    EXIT 0  {"ok":true,"data":{"engineLabel":"localModel","insights":[]}}
    selfcheck       EXIT 0  {"ok":true,"data":{"artifacts":[{"fileName":"qwen2.5-1.5b-instruct-q4_k_m.gguf",
                             "bytes":1117320736,"sha256Match":true}],"manifestValid":true,
                             "loaded":false,"loadLabel":null,...}}

All RETURNED, none timed out.

### engineLabel — the line this gate exists for

    ENGINE_LABEL=localModel

Proven **against the running service**, from `insights list`, not from
`self-check --load-model`. The Phase 36 defect — every installed copy silently
running `ruleFallback` because the MSI never authored the embedded model — does
**not** reproduce on this x64 install. Consistent with `self-check`:
`sha256Match: true` against the 1117320736-byte GGUF and `manifestValid: true`.

Note `"loaded": false` in `self-check` is the documented aetherctl behaviour
(`default = []`, loader opt-in) and is not evidence against `localModel`; the
service is what holds the live engine, and the service reports `localModel`.

### Gate 2 verdict

| criterion | expected | observed | result |
|---|---|---|---|
| install transaction | success | MsiInstaller 1033, status 0, 0.1.11, from the Gate 1 MSI `d18d89db...` | PASS |
| files | 16 with matching hashes | 16, hash set identical to install time | PASS |
| VCOMP140 / LIBOMP | True / False | True / False | PASS |
| dev binaries | none | `DEV_BINARY_IN_INSTALL_IMAGE=NO` | PASS |
| service | LocalSystem, AUTO_START, RUNNING | all three | PASS |
| service SID | UNRESTRICTED, Active | UNRESTRICTED, Active | PASS |
| pipe DACL | `O:<SID> G:SY D:P(FA;SID)(FR;AU)(DC;AU)` | same set, AU pair merged as `0x12008b` | PASS |
| install-dir ACLs | protected, no Users write | Users (RX) only, Admins/SYSTEM (F) | PASS |
| registration | ARP + HKLM agree | both 0.1.11 | PASS |
| verbs | return, doctor typed rejection ok | 6/6 RETURNED, doctor `diagnostics.stateUnavailable` | PASS |
| **engineLabel** | **`localModel`** | **`localModel`** | **PASS** |

**GATE 2 = PASS.** No security regression. Nothing was disabled, weakened, or
worked around.

## 41.15 GATE 3 — DBT-P41-002 MEASURED ON REAL x86_64 SILICON (2026-09-02)

Measurement only. **Nothing was fixed**, per the brief and per §20.1.10.
Raw logs: `C:\AetherCore-P41\logs\gate3\`.

### 3.A — the deciding check: **PASS**

    "perProcessorBusyBp":[]

EXPECTED `[]`, observed `[]`. Not 22 zeros — and this machine genuinely has 22
logical processors, so the discriminator was live, not vacuous. **The installed
binary corresponds to the source §20.1 was diagnosed against (`e91f675`).** The
diagnosis does not need re-basing and everything below is measured against a
valid baseline.

### 3.B — the offline reading, verbatim

`aetherctl --output json telemetry-once`, exit 0:

    {"schema":"aethercore.aetherctl.v1","command":"telemetry-once","ok":true,"data":{
     "capturedUnixMs":1788348967201,
     "collectorFaults":[{"collector":"gpu","detail":"no GPU engine counters exposed by this adapter/driver","kind":"Unavailable"}],
     "cpu":{"contextSwitchesPerSec":0,"dpcIsrBusyBp":0,"perProcessorBusyBp":[],
            "processorQueueLengthX100":0,"totalBusyBp":0},
     "gpu":null,"intervalMs":250,
     "memory":{"availablePhysicalBytes":3085389824,"hardFaultsPerSec":0,
               "memoryLoadPercent":81,"softFaultsPerSec":0,"totalPhysicalBytes":16632156160},
     "platform":"windows",
     "power":{"hasTemperature":false,"temperatureC":0,"throttleActive":false,"throttleReason":"none"},
     "processTop":[],"providerSource":"PerfPlatform","storage":[]}}

Every §20.1 prediction holds:

- cpu — all four PDH-sourced scalars are `0`, `perProcessorBusyBp` is `[]`, and
  **no cpu fault is declared** (§20.1.2: `CpuSample` is scalars, it cannot
  express "I got nothing").
- storage — `[]` with **no fault** (§20.1.2: four fault-free early returns).
- gpu — the **only** collector declaring a fault, and it declares one with a
  reason (§20.1.2).
- memory — the headline fields are real (16632156160 bytes = 15.49 GiB total,
  81% load) because they come from `GlobalMemoryStatusEx`, while its two
  PDH-sourced fields, `hardFaultsPerSec` and `softFaultsPerSec`, are `0` like
  everything else PDH touches. This is §20.1.3(a)'s split, visible in one object.
- power — `throttleActive` reported with **no fault**, consistent with the
  unreachable `else` at `:256-262` behind `|| true` (§20.1.1 site 3).
- processTop — `[]` with no fault, by design (§20.1.1 site 7).

### 3.B (continued) — the service-backed reading, and a correction to the brief

**The brief asks to "re-run `telemetry-once` against the RUNNING SERVICE". That
is not a thing `telemetry-once` can do.** `aetherctl --help` classifies it under
*"OFFLINE COMMANDS (no service required, strictly read-only)"*. It samples
in-process and never contacts the service, so running it with the service up
exercises the same code either way. The verb that actually crosses the service
boundary is `perf snapshot`, which is what §20.1.9 recommended running "to
confirm the prediction rather than to discover the answer".

`aetherctl --output json perf snapshot`, exit 0 — service-backed:

    {"schema":"aethercore.aetherctl.v1","command":"perf snapshot","ok":true,"data":{
     "capturedUnixMs":1788349015839,
     "collectorFaults":[{"collector":"gpu","detail":"no GPU engine counters exposed by this adapter/driver","kind":"Unavailable"}],
     "cpu":{"contextSwitchesPerSec":0,"dpcIsrBusyBp":0,"totalBusyBp":0},
     "intervalMs":1000,
     "memory":{"availablePhysicalBytes":3020337152,"hardFaultsPerSec":0,
               "memoryLoadPercent":81,"totalPhysicalBytes":16632156160},
     "power":{"hasTemperature":false,"temperatureC":0,"throttleActive":true},
     "processTop":[],"storage":[]}}

**§20.1.9(1) is confirmed on real hardware: the service path shows the identical
zeros.** cpu `0/0/0`, storage `[]`, the same single gpu fault with the same
detail string, memory real. DBT-P41-002 is **not** scoped to `telemetry-once`;
that verb is only where it was first seen.

The one apparent divergence was chased rather than assumed. The first offline
sample said `throttleActive:false` and the service sample 48 s later said `true`.
Three back-to-back offline/service pairs settled it:

    round 1  offline.throttleActive=True  service.throttleActive=True  reason=power  cpu 0/0  storage 0/0
    round 2  offline.throttleActive=True  service.throttleActive=True  reason=power  cpu 0/0  storage 0/0
    round 3  offline.throttleActive=True  service.throttleActive=True  reason=power  cpu 0/0  storage 0/0

The paths agree. The first difference was the machine's real power state changing
between samples (the box moved into `throttleReason: "power"` and stayed there),
**not** a code-path divergence. No new fact; recorded because the brief asked for
one if it existed and honesty requires saying it did not.

### 3.B (the part no VM could have measured) — the counters exist and carry data

The all-zero output is only damning if the host actually exposes the counters
being misread. Probed independently of the product, with `Get-Counter`, on this
silicon:

    \PhysicalDisk(*)\% Disk Time            PRESENT, 4 instances
        InstanceName   CookedValue
        0 c:           6.44321146949699
        1 d:           0
        2 e:           7.43446712527608
        _total         4.62589617871989

    \GPU Engine(*)\Utilization Percentage   PRESENT, 568 instances
        pid_10148_luid_0x00000000_0x00010f21_phys_0_eng_0_engtype_3d                  0
        pid_10148_luid_0x00000000_0x00010f21_phys_0_eng_10_engtype_gdi render         0
        pid_10148_luid_0x00000000_0x00010f21_phys_0_eng_11_engtype_videoprocessing    0
        pid_10148_luid_0x00000000_0x00010f21_phys_0_eng_1_engtype_videodecode         0

    \Processor(_Total)\% Processor Time      7.01
    LOGICAL_PROCESSORS                       22

Three conclusions, each a measurement rather than an inference:

1. **storage `[]` is a code defect, not absent hardware counters.** The host
   exposes `\PhysicalDisk(*)\% Disk Time` with four live instances returning
   non-zero values at the same moment the product returns `[]`. This eliminates
   "the counters aren't there" and leaves §20.1.3(c)'s pattern bug — the
   dangling `Vec<u16>` temporary and the raw-string `\0` that is two characters
   rather than a NUL. It narrows the open §20.1.7 item *"which of storage's four
   exits fires"* to the two that follow `PdhExpandWildCardPathW`
   (`needed == 0` at `:353-355`, or the call failure at `:359-361`); the
   query-open exit at `:328-335` is excluded because it would have pushed a
   fault, and none was pushed.

2. **The gpu fault's detail string is factually false on this machine.** It
   says *"no GPU engine counters exposed by this adapter/driver"* while the
   adapter exposes **568** of them. This confirms §20.1.6: gpu's healthy-looking
   `Unavailable` is the accidental by-product of `PdhAddEnglishCounterW`
   refusing an unexpanded wildcard, not of the counters being missing. So the
   one collector that "gets it right" also misattributes its own cause — do not
   model the fix on it, and the detail string needs correcting alongside.

3. **The cpu zero is provably not an idle machine.** `\Processor(_Total)\%
   Processor Time` reads **7.01%** — about **701 basis points** — in the same
   window the product reports `totalBusyBp: 0`. §20.1.3(a) called this "not 22
   processors at 0%, it is `PDH_CSTATUS_VALID_DATA` formatted as basis points";
   that is now measured, not argued.

Also settled from §20.1.8: every PDH-sourced field observed across six samples
read exactly `0`, never `1`. So the counters are returning
`PDH_CSTATUS_VALID_DATA (0x0)` rather than `PDH_CSTATUS_NEW_DATA (0x1)` — the
misread is stable, not intermittent.

**The stack-overflow question (§20.1.7) remains UNMEASURED.** `read_u64` passes
an 8-byte destination for a 16-byte `PDH_FMT_COUNTERVALUE` write on every counter
read. The service has been running since 12:09 with no crash, so the 8 bytes are
evidently being absorbed by stack padding on this build — but "did not crash" is
not evidence of "does not corrupt". Establishing that needs an ASAN or
`/analyze` build, not observation of a running process. It stays open, and it is
still a memory-safety defect regardless.

### 3.C — `capabilities` contradicts the collectors, measured

`aetherctl --output json capabilities`, exit 0. All **16** capabilities report
`"state":"native"`, `"key":null` — including:

    telemetryCpu      native
    telemetryMemory   native
    telemetryStorage  native
    telemetryGpu      native

**In the same session, on the same machine, `telemetryStorage: native` and
`"storage": []`; `telemetryGpu: native` and a gpu `Unavailable` fault.**
§20.1.9(2) confirmed: `windows_table()` is a static map with no runtime input,
so it will report `native` on every Windows host in every state. Decision site 9
of §20.1.1, now demonstrated rather than read from source.

`engine-source` reports `{"platform":"windows","source":"native"}`.

### Gate 3 verdict

| item | expected | observed | result |
|---|---|---|---|
| 3.A `perProcessorBusyBp` | `[]` | `[]` (on a genuine 22-processor box) | **PASS** |
| 3.B cpu offline | zero, no fault | `0/0/0/0`, `[]`, no fault | PASS |
| 3.B storage offline | `[]`, no fault | `[]`, no fault | PASS |
| 3.B gpu offline | `Unavailable` with reason | declared, with reason | PASS |
| 3.B service path | identical zeros (§20.1.9) | identical, via `perf snapshot` | PASS |
| 3.B offline vs service | divergence would be a new fact | none; 3/3 rounds agree | PASS (no new fact) |
| PDH counters present | not previously measured | PhysicalDisk 4 live instances, GPU Engine 568, CPU 7.01% | **NEW EVIDENCE** |
| `CStatus` 0 vs 1 (§20.1.8) | unknown | always `0` = `VALID_DATA`, 6 samples | **RESOLVED** |
| storage exit (§20.1.7) | which of four | narrowed to the two after `PdhExpandWildCardPathW` | PARTIAL |
| stack overflow observable | unknown | no crash in ~3 h uptime; not provable by observation | **UNMEASURED** |
| 3.C `capabilities` | static `native` regardless | 16/16 `native`, contradicting storage `[]` and the gpu fault | PASS (defect confirmed) |

**GATE 3 = PASS.** DBT-P41-002 is confirmed on real x86_64 silicon, its blast
radius is larger than first recorded (service path included, `capabilities`
included), and the diagnosis in §20.1 stands unmodified. **Not fixed**, per the
brief.

## 41.16 GATE 3.C — the four Stage 3 items §41.7 could not pay without the install

§41.7 closed with a list of what Stage 3 still owed. All four are now measured on
real x86_64 silicon with the service running.

### 3a — SMART / NVMe attributes from the real disk

    DeviceId FriendlyName                   MediaType   BusType Health  Size            Firmware
    0        NVMe Micron_2500_MTFDKBA1T0QGN SSD         NVMe    Healthy 1024209543168   V8MA000
    1        HIKSEMI                        SSD         USB     Healthy 1024209543168   4401
    2        ADATA USB Flash Drive          Unspecified USB     Healthy   31037849600   1100

    disk 0 (system NVMe)  Temperature 60 C   Wear 0
    disk 1 (HIKSEMI USB)  Temperature 48 C   Wear 0   PowerOnHours 578
                          ReadErrorsTotal 0  ReadErrorsUncorrected 0
    disk 2 (ADATA)        Temperature 0      (no reliability counters exposed)

    MSStorageDriver_FailurePredictStatus
      SCSI\Disk&Ven_NVMe&Prod_Micron_2500_MTFD\...   PredictFailure = False, Reason = 0

The system NVMe at 60 C is warm but healthy and predicts no failure. The HIKSEMI
reading at 48 C was taken shortly after it absorbed a 96-minute, 521 GB image
write, which is the context for that number. Most NVMe reliability fields
(`PowerOnHours`, `StartStopCycleCount`, error totals) come back empty on disk 0 —
the driver does not surface them through `Get-StorageReliabilityCounter` on this
box. Recorded as observed; not worked around.

### 3b — GPU telemetry, driver identity and memory

Two adapters, both `Status: OK`:

    Intel(R) Arc(TM) Graphics
      PNPDeviceID    PCI\VEN_8086&DEV_7D55&SUBSYS_142E1462&REV_08\3&11583659&0&10
      DriverVersion  31.0.101.5007      DriverDate  2023-11-18
      AdapterRAM     1073741824         VideoMode   2560 x 1600

    NVIDIA GeForce RTX 4060 Laptop GPU
      PNPDeviceID    PCI\VEN_10DE&DEV_28A0&SUBSYS_142E1462&REV_A1\4&3016F0B9&0&0008
      DriverVersion  32.0.16.1656       DriverDate  2026-08-20
      AdapterRAM     4293918720 (truncated at 4 GB by the WMI field)
      dedicated VRAM 8585740288 from HardwareInformation.qwMemorySize = 8 GiB

    \GPU Adapter Memory(*)\Dedicated Usage  -> 4 adapter LUIDs enumerated

**This sharpens §41.15's gpu finding rather than repeating it.** The product's
gpu fault says *"no GPU engine counters exposed by this adapter/driver"* on a
machine carrying two healthy adapters, one of them a discrete RTX 4060 with 8 GiB
of dedicated VRAM and a driver dated two weeks ago, exposing 568 engine counter
instances across 4 adapter LUIDs. The detail string is not merely imprecise here;
it names a cause that is measurably false.

The Intel Arc driver is from 2023-11-18, nearly three years old — noted as an
observation for the owner, not acted on.

### 3c — PnP and Windows Update driver discovery counts

    PNP_TOTAL=229   PNP_OK=205   PNP_ERROR=0   PNP_DEGRADED=0   PNP_UNKNOWN=24
    PNP_WITH_PROBLEM=24   -> every one of the 24 is CM_PROB_PHANTOM
    THIRD_PARTY_DRIVER_PACKAGES=101

All 24 non-OK devices are phantoms: USB composite/mass-storage devices,
Bluetooth enumerators and serial-over-Bluetooth ports, a wireless headset, a
generic volume shadow copy. Those are remembered registrations for hardware not
currently attached, **not faults**. `PNP_ERROR=0`: this machine has no device in
an error state.

    Windows Update, SEARCH ONLY, nothing downloaded or installed:
    WU_DRIVER_UPDATES_FOUND=0      WU_SEARCH_RESULTCODE=2 (orcSucceeded)

**A correction on the way to that number**, recorded because the failure looks
like an outage and is not: the first search returned `HRESULT 0x80244011` with
`ServerSelection = 1`. That value is `ssManagedServer` (WSUS), and the error is
"WUServer policy value is missing in the registry" — correct behaviour for a
machine with no WSUS. The Windows Update server is `ServerSelection = 2`
(`ssWindowsUpdate`), which returned cleanly.

**This matters for Gate 4:** Windows Update offers this machine **zero** driver
updates. There is no WU-supplied driver available to exercise a driver install
and rollback against, so Gate 4's "deliberately safe device" has to come from
somewhere else. That is an input the owner has to supply.

### 3d — service memory / CPU / disk at idle, under scan, and at model load

Idle, after 152 minutes of uptime (PID 11324, started 12:09:29.965):

    WORKINGSET      492687360 bytes   469.86 MB
    PEAK_WORKINGSET 1390047232 bytes  1325.65 MB
    PRIVATE         656.64 MB         VIRTUAL 5945.27 MB
    TOTAL_CPU       37.62 s over 152 min  (~0.4% of one core, averaged)
    HANDLES 277     THREADS 11
    READ 1124075117 bytes (1072 MB)   WRITE 644097 bytes (0.61 MB)

**Peak working set at model load, answered by the read total.** The service has
read 1072 MB from disk since start, against a shipped GGUF of 1117320736 bytes
(1065.6 MB) — i.e. essentially the whole model, once. The 1325.65 MB peak is that
model plus baseline, and it sits inside the 2147483648-byte (2 GiB) RAM budget
`self-check` declares. This is the localModel engine's real cost on this
hardware.

Under scan — `scan start` -> scanId `cdc62915-f5e2-4680-9571-cee26538694c`:

    duration          342.5 s   (started 1788349309450, completed 1788349651907)
    CPU delta          ~9.3 s   over 342.5 s  -> ~2.7% of one core on a 22-CPU box
    peak WS during     485.3 MB (from 469.87 MB at rest: +15 MB)
    disk read delta    0 MB
    disk write delta   ~47 MB
    final counts       collectorCount 7, factsCount 487, findingCount 469,
                       remediationCandidateCount 299, warningCount 0
    fingerprint        defb3afb18a9b46342299b4b4c51cda8b7040a467735d3e443be7f59d690783e

Collectors landed in stages (1 -> 2 -> 7) rather than all at once, and the
findings arrived with them: 128 facts at t=5 s, 473 by t=30 s, 487 at completion.

**A behaviour worth recording: the service releases the model after the scan.**
Working set fell from ~420 MB to 128.68 MB at t=250 s, then 68.34 MB, settling at
77.55 MB — while `PeakWorkingSet` stayed 1325.65 MB. So the resident footprint at
rest after work is ~78 MB, not the ~470 MB it held while the model was live. A
consumer sizing this service from its peak alone would over-provision by ~17x.

### A defect that was NOT one: `doctor`'s typed rejection is state-dependent

Gate 2 recorded `doctor` returning exit 5 with
`diagnostics.stateUnavailable` and treated it as PASS by design. That is now
positively confirmed rather than assumed — after the scan completed, the same
binary and the same service:

    doctor  EXIT 0
    {"ok":true,"data":{"state":"Ready","cardCount":2,"crashCount":0,
     "eventCount":128,"eventWindowDays":30,"providerFaults":[],
     "storageCount":3,"warningCount":0,"warnings":[]}}

The verb was rejecting because no diagnostic state existed yet, not because it
was broken. `providerFaults: []` and `crashCount: 0`.

**And it surfaces one more contrast for DBT-P41-002.** `doctor` reports
`storageCount: 3` — it enumerates all three attached disks — in the same session
where `telemetry-once` and `perf snapshot` both return `"storage": []`. The
machine's storage is plainly enumerable by the product through the
`hardware-telemetry` path; it is specifically the `performance-telemetry`
Windows provider that yields nothing. That is further evidence the empty array is
a defect in one collector rather than a property of this hardware, and it is
consistent with §20.1's scoping of the bug to
`crates/performance-telemetry/src/windows_impl.rs`.

`insights list` re-checked after the scan: `engineLabel` still `localModel`.

### Stage 3 item status

| item | owed by §41.7 | result |
|---|---|---|
| 3a `perf snapshot` service-backed | yes | done, §41.15 — identical zeros |
| 3a SMART / NVMe | yes | done — NVMe 60 C healthy, PredictFailure False |
| 3b GPU / driver identity / memory | yes | done — Arc + RTX 4060 8 GiB, both OK |
| 3c PnP + WU driver counts | yes | done — 229/205/0 error, 101 packages, **0 WU driver updates** |
| 3d service mem / CPU / disk, peak at model load | yes | done — peak 1325.65 MB, idle-after-work 77.55 MB, scan 342.5 s at ~2.7% of a core |

## 41.17 GATE 4 READINESS — what the owner must supply. **NOT STARTED, deliberately.**

Gate 4 was not begun. `P41-FULL-RUN.md` makes it a hard stop and the reasons hold
independently of this session's results: its failure mode is an unbootable
machine, and driver acquisition and install are hardware-gated and separately
authorised.

### What Gate 4 needed, and what now exists

| precondition | state before this session | state now |
|---|---|---|
| a verified full disk image | did not exist and was "currently impossible" (§41.2 0e) | **EXISTS AND VERIFIED** — §41.13, Bare Metal Recovery, 521.45 GB, listing-confirmed after the owner's format |
| bootable recovery media | did not exist; WinRE on disk 0 does not cover an unbootable machine | **ARTIFACTS EXIST** — E: RECOVERY, full BIOS+UEFI boot chain, 6.12 GB (§41.13 0f.D). **Not boot-tested.** |
| a pre-driver restore point (4a) | not created | not created — belongs to the Gate 4 session, elevated |
| a deliberately safe device to test against | assumed available | **NOT AVAILABLE from Windows Update** — see below |
| Gate 2 installed and healthy | BLOCKED | PASS (§41.14) |

### The three things the owner must do or decide

**1. Boot-test the recovery media.** E: has `bootmgr`, `boot\bcd`,
`sources\boot.wim`, `efi\boot\bootx64.efi`, and its partition is active. What is
verified is that the *artifacts are complete*; what is NOT verified is that this
machine boots from them. Nobody should start driver work on the strength of a
directory listing. Boot the machine from E: once, confirm it reaches the recovery
environment, and confirm from there that `D:\WindowsImageBackup` is readable —
because an image the recovery environment cannot see is not a recovery path.

**2. Supply the driver to test.** `Windows Update offers this machine zero driver
updates` (§41.16 3c, search-only, ResultCode 2 = succeeded). The gate's
"deliberately safe device" therefore has no WU-supplied candidate. The owner has
to nominate one — and "safe" should mean a device whose failure cannot prevent
boot: not storage, not chipset, not GPU. This machine has 24 phantom PnP
registrations and 101 third-party driver packages; a printer-class, HID-class or
USB-peripheral driver is the shape to look for.

**3. Decide about the Intel Arc driver, separately from Gate 4.** The iGPU driver
is version 31.0.101.5007 dated **2023-11-18**, against the NVIDIA driver's
2026-08-20. That is an observation from 3b, not a Gate 4 dependency, and updating
it is an owner decision — it is a display driver, i.e. exactly the class that
should NOT be used as the safe test device.

### What Gate 4 must record when it does run

Unchanged from the standing rules: the 4a restore point created **and
enumerated** before the first driver action, the ACTION/SNAPSHOT/EXPECTED/
RECOVERY/BLAST record committed before the first elevated step, and rollback
proven by observation rather than by a success message — the same discipline that
turned "wbadmin said it worked" into "the version listing proves it did" in
§41.12.

### Open debts carried out of this session

| id | what | status |
|---|---|---|
| DBT-P41-001 | x64 service imports `MSVCP140`/`VCRUNTIME140`, present on this box but not in the payload | open, unchanged — not re-measured this session |
| DBT-P41-002 | cpu zero / storage empty with no fault | **diagnosed (§20.1) and now confirmed on real silicon (§41.15)**. Not fixed, per the brief. Scope is wider than first recorded: service path and `capabilities` included |
| DBT-P41-002a | the gpu fault's detail string names a false cause — "no GPU engine counters exposed by this adapter/driver" on a box exposing 568 of them across two healthy adapters | **new this session**, §41.15 / §41.16 3b |
| DBT-P41-002b | `read_u64`'s 8-byte destination for a 16-byte PDH write is a stack overflow on every counter read | open; **not observable** from a running process — needs an ASAN or `/analyze` build (§41.15) |
| — | Gate 0f's `wbadmin` client exit code was lost when the console process was killed at 48% | recorded UNMEASURED, superseded by the version listing (§41.12 0f.3) |
| — | the never-installed machine state is gone; it was already installed before it could be imaged | recorded, unrecoverable (§41.12 0f.7) |
| — | `P41-FULL-RUN.md`'s FileStream method for reading the pipe DACL does not work; `NamedPipeClientStream` does | correction recorded (§41.14) |
| — | `telemetry-once` is an offline verb and cannot be run "against the running service"; `perf snapshot` is the service-backed one | correction recorded (§41.15) |

### STOP

Gate 4 does not run in this session and not unattended. Gate 5 (full lifecycle,
zero survivors) is no longer blocked by Gate 2 and could be run before Gate 4 —
it is the uninstall/survivor sweep, it is proven on ARM64 but not on this
machine, and it does not risk the boot path. That sequencing is the owner's call.

## 41.18 SECURITY POSTURE — re-checked at session end, unchanged

The standing rule is that a security regression stops everything and that
Defender, UAC, Firewall and SmartScreen are never disabled. Nothing in this
session touched any of them; re-measured at the end anyway, because "I did not
change it" is a memory and the reading is evidence:

    DEFENDER_REALTIME       True
    DEFENDER_ANTIVIRUS      True
    DEFENDER_TAMPER         True      (tamper protection on)
    PUAProtection           2
    MAPSReporting           2         SubmitSamplesConsent 1
    UAC_EnableLUA           1
    UAC_ConsentPromptAdmin  5         (prompt for consent on the secure desktop)
    Firewall Domain         True
    Firewall Private        True
    Firewall Public         True
    AetherCoreMaintenance   Running

**SmartScreen, stated precisely rather than rounded up to "enabled":**
`HKLM\...\Explorer\SmartScreenEnabled` is empty, `AppHost\EnableWebContentEvaluation`
is unset, and `HKLM\SOFTWARE\Policies\Microsoft\Windows\System\EnableSmartScreen`
does not exist. So there is **no policy or registry value disabling SmartScreen**
and it sits at the Windows default. What is positively verifiable is the absence
of any override; a direct "enabled" reading is not available from these keys, and
this session set none of them.

# PHASE 42 — FIX DBT-P41-002 AT THE TYPE, THEN PROVE THE LIFECYCLE (2026-09-02)

Brief: `phase21-workspace/docs/phase41/P42-FIX-AND-LIFECYCLE.md`.
Session started from an already-elevated PowerShell, per §41.9 — verified, not
assumed: `IsInRole(Administrator) = True`, `HUSSEIN\husen`, `PROCESSOR_ARCHITECTURE=AMD64`,
`cargo 1.97.1`, `rustc 1.97.1`. `core.autocrlf = false` confirmed before any edit.

## 42.0 P42 PROGRESS TABLE (authoritative — resume from here)

| item | what it proves | status | evidence |
|---|---|---|---|
| 1.A | the four tests that should have caught DBT-P41-002, committed failing | **DONE** | §42.1 — 4/4 FAILED on real x64, output verbatim below |
| 1.B | one contract replaces the nine availability rules; both mechanisms fixed | **DONE** | §42.2 — 9 rules → 1 contract, 3 further defects found (DBT-P42-001/002/003), 7/7 tests pass |
| 1.C | the fix proven on this machine with numbers | **DONE** | §42.3 — cpu 5838 bp vs host 50.91%, storage 2 real devices, gpu 16 engines; DBT-P42-008 found and fixed |
| 2.A | MSI rebuilt with the fix, zero ICE | **DONE** | §42.5 — validate EXIT 0 output EMPTY, 0 ICE, payload PASS 16 rows, sha256 `6ecd1ee9…` at 1,100,148,736 bytes |
| 2.B | uninstall + fourteen-check survivor sweep | **PASS** | §42.6 — uninstall exit 0; 13/14 clean outright, check 14 has ZERO machine-wide hits (all 16 are user-profile dev artifacts) |
| 2.C | reinstall, every Gate 2 property re-proven | **PASS** | §42.7 — install exit 0, 16/16 hashes match the built payload, SID UNRESTRICTED, pipe DACL equal, engineLabel=localModel |
| 2.D | the fix live under the installed service | **PASS** | §42.8 — service path cpu 5231 bp vs host 48.22%, storage 2 devices, perf snapshot live; doctor exit 5 RESOLVED (state-dependent) |
| 4 | driver install/rollback | **NOT STARTED — HARD STOP** | §41.17 + §42.9 — unchanged, and the E: recovery media is now measured DETACHED, so its boot-test precondition is two steps not one |

## 42.1 PART 1.A — the four regression tests, committed FAILING

§20.1.5 records the hole precisely: **no test anywhere constructs
`WindowsPerfPlatform`**, and the two real-provider tests that exist assert upper
bounds only (`phase27_real_sample.rs:41-43` `<= 10_000`;
`phase28_cli_matrix.rs:177-181` `0.is_u64()`), which an all-zero snapshot
satisfies. The tests were therefore written first and committed failing, matching
the practice the LocalSystem authorization fix established in Phase 39.

New file: `crates/performance-telemetry/tests/dbt_p41_002.rs`, 4 tests,
`#![cfg(windows)]`.

### The one production change 1.A makes, and why it is not a fix

`read_u64` was routed through two extracted items so the ABI is testable without
linking PDH:

    pub const PDH_VALUE_SLOT_BYTES: usize = 8;      // what the shipping code supplies
    pub fn decode_pdh_value(slot: &[u8]) -> Option<u64>  // i64::from_le_bytes(slot[0..8])

This is **behaviour-identical to the shipping code**: same 8-byte destination,
same read of bytes 0..8, same `.max(0) as u64`. The 8-byte overflow
(DBT-P41-002b) is neither introduced nor removed by 1.A — it is pre-existing and
carried unchanged so the tests fail against real shipping behaviour rather than
against a stand-in. 1.B removes it.

A dev-dependency on `aethercore-platform-capabilities` was added to
`performance-telemetry` so test 4 can assert the two crates against each other in
one process. Dev-only; the production dependency direction is unchanged.

### Result: 4 tests, 4 FAILED — verbatim

    running 4 tests
    test pdh_value_is_decoded_from_large_value_not_cstatus ... FAILED
    test no_collector_returns_an_empty_payload_without_a_fault ... FAILED
    test real_windows_provider_reports_non_zero_cpu_under_load ... FAILED
    test capabilities_never_claim_native_for_a_subsystem_that_reported_nothing ... FAILED

    ---- pdh_value_is_decoded_from_large_value_not_cstatus ----
    PdhGetFormattedCounterValue writes a 16-byte PDH_FMT_COUNTERVALUE;
    supplying 8 bytes overflows the destination

    ---- real_windows_provider_reports_non_zero_cpu_under_load ----
    every logical processor was spinning; totalBusyBp must not be 0.
    cpu=CpuSample { per_processor_busy_bp: [], total_busy_bp: 0, dpc_isr_busy_bp: 0,
                    context_switches_per_sec: 0, processor_queue_length_x100: 0 }
    faults=[CollectorFault { collector: "gpu", kind: "Unavailable",
            detail: "no GPU engine counters exposed by this adapter/driver" }]

    ---- no_collector_returns_an_empty_payload_without_a_fault ----
    collectors returned success with nothing measured and nothing declared:
      cpu: CpuSample { per_processor_busy_bp: [], total_busy_bp: 0, ... } with no fault
      storage: [] with no fault
      processTop: [] with no fault
    all faults: [gpu/Unavailable]

    ---- capabilities_never_claim_native_for_a_subsystem_that_reported_nothing ----
    capabilities contradicts the collectors in the same process:
      telemetryCpu: reported `native` while the collector produced CpuSample { ...all zero... }
      telemetryStorage: reported `native` while the collector produced 0 devices
      telemetryGpu: reported `native` while the collector produced 0 engines

    test result: FAILED. 0 passed; 4 failed; 0 ignored; finished in 1.59s

**These reproduce §41.15 exactly**, from a test process rather than from the CLI:
`perProcessorBusyBp []`, all four PDH-sourced cpu scalars `0`, storage `[]` with
no fault, and the single gpu `Unavailable` whose detail string §41.16 3b proved
false. The load harness spins every logical processor for 300 ms before sampling
and holds it across the sample, so "the machine was idle" is not available as an
explanation — and §41.15 had already measured 7.01% on an *unloaded* box.

## 42.2 PART 1.B — one contract, and the mechanisms underneath it

### The contract

`Reading<T>` in `crates/performance-telemetry/src/lib.rs`. A struct wrapping a
**private** enum, so `Measured` is unconstructible — inside this crate as well as
outside it — without passing evidence to a constructor:

    pub fn from_evidence(Option<T>, || CollectorFault) -> Reading<T>      // None ⇒ Unavailable
    pub fn from_collection(Vec<T>, || CollectorFault) -> Reading<Vec<T>>  // empty ⇒ Unavailable
    pub fn unavailable(CollectorFault) -> Reading<T>
    pub fn into_parts(self, || fallback) -> (T, Option<CollectorFault>)

`CollectedSubsystems` holds one `Reading` per subsystem, and
`into_snapshot(interval)` is the only path from collectors to a `PerfSnapshot`.
The fallback in `into_parts` is reachable **only** on the `Unavailable` arm, so a
snapshot field holding a zero always has a fault standing beside it. A zero can
appear; an unexplained zero cannot.

**Why this shape and not the brief's other two.** `sample` returning `Result`
does not fix anything — §20.1.4 is explicit that `Ok(PerfSnapshot::default())`
stays legal, so the hole survives the signature change. Dropping `Default` from
the payload types breaks the wire structs the renderer, the proto bridge and the
bottleneck analyzer all read, for no gain: the defect is not that a zero exists,
it is that a zero could be published **without a reason**. `Reading` puts the
decision at the point of collection, where the evidence is, and the `Option`-in
constructor is what closes §20.1.3(a)'s real mechanism — `.unwrap_or(0)` turning
a failed read into a confident measurement. Every `.unwrap_or(0)` in the Windows
provider is now either gone or accompanied by a named degradation fault.

### How many of the nine availability rules survived

**None survived as an independent rule.** Five were deleted outright; four remain
as *reason strings* that the type now forces the collector to supply — they no
longer decide availability, they explain it.

| # | §20.1.1 site | outcome |
|---|---|---|
| 1 | cpu, query open | **converted** — `Reading::unavailable`, now type-required |
| 2 | cpu, collect failed | **converted** — same |
| 3 | power `\|\| true` dead `else` | **DELETED** — availability now reads `CallNtPowerInformation`'s actual outcome; the unreachable branch is gone |
| 4 | memory | **converted**, and split: a failed `GlobalMemoryStatusEx` makes memory unavailable; failed PDH counters are a `memory.counters` partial |
| 5 | storage, query open only | **converted**, and the four fault-free exits it did not cover are covered by the contract |
| 6 | gpu `engines.is_empty()` | **DELETED** — `Reading::from_collection` performs that check for every collection payload, so gpu's bespoke rule is redundant |
| 7 | processTop `let _ = faults;` | **DELETED** — replaced by an explicit `NotCollected` reading with the real reason |
| 8 | CLI `offline.rs:241` second gpu rule | **DELETED** — the CLI now reads `measured_subsystems().gpu`, the collector's own answer, so CLI and service can no longer disagree from the identical snapshot |
| 9 | `windows_table()` static `native` | **DELETED as an unconditional claim** — `matrix_for_current_platform_observed()` reconciles the platform shape against what the collectors measured |

Nine independent deciders became **one**.

### Mechanism 1 — `read_u64` (DBT-P41-002b, a memory-safety fix)

`PdhFmtCounterValue` is now declared `repr(C)` — `{ CStatus: u32, _pad: u32,
large_value: i64 }`, 16 bytes, `largeValue` at offset 8 — and the extern binding
declares the parameter as `*mut PdhFmtCounterValue` rather than `*mut i64`. The
8-byte destination for a 16-byte write is gone **structurally**: there is no
longer a smaller type to pass. `PDH_VALUE_SLOT_BYTES` is derived with
`size_of::<PdhFmtCounterValue>()` so it cannot drift from the struct.

`decode_pdh_value` reads offset 8 and returns `None` unless `CStatus` is
`VALID_DATA (0)` or `NEW_DATA (1)`. A status code can no longer be returned as a
measurement.

### Mechanism 2 — storage's pattern (§20.1.3(c))

The pattern is bound to a named `Vec<u16>` that outlives both calls and is
NUL-terminated through `chain(once(0))`, replacing the dropped temporary and the
raw-string trailing sequence that was two characters rather than a NUL. Every
exit — query-open, both expansion calls, no usable instances, and no readable
counter — pushes a fault with the PDH return code in it.

### THREE FURTHER DEFECTS FOUND WHILE FIXING THESE

These were not in §20.1 and are not restatements. Each was found by the fix, and
each was measured.

**DBT-P42-001 — the `PdhExpandWildCardPathW` binding has the wrong arity.**
The real export takes **five** parameters
(`szDataSource, szWildCardPath, mszExpandedPathList, pcchPathListLength, dwFlags`);
the shipping declaration had **three**, and typed the output buffer `*mut PWSTR`
where the API takes a `PZZWSTR` — a plain buffer. Verified against the bindings
this crate already depends on: `windows-0.62.2`
`src/Windows/Win32/System/Performance/mod.rs:342`. No linker can catch a wrong
signature behind a correct export name.

On x64 every argument therefore landed in the wrong register — the wildcard path
arrived as `szDataSource`, the out-pointer as `szWildCardPath` — and the callee
read `dwFlags` from uninitialised stack beyond the shadow space. **This is why
`needed` came back 0 with no failure code**, which §41.15 could only narrow to
"one of the two exits after `PdhExpandWildCardPathW`". The answer is neither: the
call never had a chance to succeed. Passing a real buffer through the same broken
declaration faults immediately —

    process didn't exit successfully: dbt_p41_002.exe
    (exit code: 0xc0000005, STATUS_ACCESS_VIOLATION)

— which is how it was found. §41.15's open item *"which of storage's four exits
fires"* is now **RESOLVED**, and the answer supersedes the two candidates it
narrowed to.

**DBT-P42-002 — percentage counters were being read as basis points.**
Separate from the offset, and hidden by it. `\Processor Information(_Total)\%
Processor Time` returns a *percentage*; the code assigned it straight into a
`_bp` field. With the offset fixed but before this was found, the provider
reported **91 bp** — 0.91% — while every logical processor was spinning. 91 was
the percentage. §41.15's own reference reading makes it unambiguous: 7.01% is
701 bp, not 7. `percentage_to_bp` now applies the x100, and percentage counters
are read with `PDH_FMT_DOUBLE` rather than `PDH_FMT_LARGE`, which was also
truncating 7.01 to 7 and `Avg. Disk sec/Transfer` to 0.

**DBT-P42-003 — counters were read before the collection that gives them data.**
`sample_storage` collected twice *before adding any counter* (a query with no
counters fails outright) and then added each counter and read it immediately.
Rate and percentage counters have no value after a single collection. So even
with the wildcard expansion working and the offset fixed, every storage read
would have returned nothing. `sample_gpu` and `sample_memory` had the same
single-collection defect. `QueryHandle::collect_twice` now enforces the
add-then-collect-twice-then-read order.

A fourth, smaller one, fixed in place: `PdhExpandWildCardPathW` returns **full
counter paths**, not bare instance names, and the code re-wrapped each returned
string as if it were an instance — which would have built
`\PhysicalDisk(\PhysicalDisk(0 C:)\% Disk Time)\% Disk Time`.
`instance_from_counter_path` parses the instance out, and is unit-tested.

### ARM64 — yes, this changes it, and here is the statement the brief asked for

`windows_impl.rs` is `#[cfg(windows)]`, not `#[cfg(target_arch)]`. **The ARM64
Windows pipeline runs this exact file**, so every change above applies to it
identically. Specifically, on ARM64 as on x64:

- the 8-byte destination for a 16-byte PDH write is removed (the ABI fact is
  identical on both — §20.1.7 records `PDH_FMT_COUNTERVALUE` as 16 bytes with
  `largeValue` at offset 8 on both LP64/LLP64 targets);
- the 3-vs-5 parameter `PdhExpandWildCardPathW` call is corrected — and note that
  P36 touched this exact binding *for ARM64*, to fix an LNK2019, without the
  arity being noticed;
- cpu/storage/gpu report real numbers where ARM64 previously reported the same
  zeros, because the defect was never architecture-specific.

The ARM64 change is a **fix of the same defect**, not a behavioural divergence,
and no ARM64-only code path exists to diverge. It has **not been re-qualified on
ARM64 hardware in this session** — no ARM64 machine is attached. That is the
honest limit: the change is correct by the same reasoning and the same tests, and
it is unverified on ARM64 silicon. Recorded as **DBT-P42-004**.

### Scoped out, deliberately

The macOS and Linux providers still construct `PerfSnapshot` literally rather
than through `CollectedSubsystems`. They already declare faults for their
degraded subsystems, and they are `#[cfg]`-gated so they **cannot be compiled or
tested on this host** — changing code this session cannot build is a worse risk
than leaving a smaller hole open. Recorded as **DBT-P42-005**, not worked around.

### Test result after 1.B

    aethercore-performance-telemetry --test dbt_p41_002
    running 7 tests
    test a_bad_status_is_not_decoded_as_a_double ... ok
    test instance_is_parsed_out_of_an_expanded_counter_path ... ok
    test pdh_value_is_decoded_from_large_value_not_cstatus ... ok
    test a_percentage_counter_becomes_basis_points ... ok
    test no_collector_returns_an_empty_payload_without_a_fault ... ok
    test real_windows_provider_reports_non_zero_cpu_under_load ... ok
    test capabilities_never_claim_native_for_a_subsystem_that_reported_nothing ... ok
    test result: ok. 7 passed; 0 failed

    aethercore-platform-capabilities --test matrix
    test an_unmeasured_telemetry_subsystem_is_never_reported_native ... ok
    test an_observation_never_promotes_a_capability ... ok
    test observation_downgrades_only_the_subsystem_that_reported_nothing ... ok
    test windows_matrix_is_all_native_frozen ... ok        <- the frozen table is intact
    test result: ok. 10 passed; 0 failed

The three capability unit tests matter because the integration test
`capabilities_never_claim_native_for_a_subsystem_that_reported_nothing` now
passes **on this machine's healthy hardware** — which is exactly the §20.1.6 trap
of mistaking luck for correctness. The unit tests hold the property under a
hostile observation on any host.

### Whole workspace

    cargo build --workspace --tests   EXIT 0
    cargo test  --workspace           204 passed, 1 failed, 5 ignored over 31 suites
                                      (excluding aethercore-driver-hub)
    aethercore-driver-hub --lib       12 passed, 6 failed

**Both failure sets are PRE-EXISTING and were verified as such**, by stashing this
session's changes and re-running:

- `aethercore-driver-hub --lib`, 6 failures — identical list before and after.
  The crate does not depend on `performance-telemetry` or
  `platform-capabilities`. **DBT-P42-006**, untouched by P42.
- `aethercore-intelligence-core --test offline_boundary`, 1 failure — it shells
  out to `cargo metadata --offline` and the local registry cache is missing
  `android_system_properties v0.1.6`. An environment gap, not a code defect.
  Identical before and after. **DBT-P42-007**.

### A build-environment fact worth carrying forward

`cargo build --workspace` fails at link with
`LNK1181: cannot open input file 'DismApi.lib'` unless `LIB` includes

    C:\Program Files (x86)\Windows Kits\10\Assessment and Deployment Kit\Deployment Tools\SDKs\DismApi\Lib\amd64

Note `amd64`, not `x64` — the same ADK naming quirk §41.4 1a recorded. It affects
`aethercore-system-repair` and `aethercore-pc-intelligence` only, and neither is
touched by P42.

## 42.3 PART 1.C — the fix measured on this machine

    cargo test  (the 1.A tests)          7 passed, 0 failed
    cargo build --release -p aetherctl -p aethercore-maintenance-service   EXIT 0

Binary under test: `target\release\aetherctl.exe`, built this session.
Load harness: every logical processor spinning, plus 16 MB write+read loops.

### `telemetry-once`, verbatim, machine under CPU + disk load

    {"schema":"aethercore.aetherctl.v1","command":"telemetry-once","ok":true,"data":{
     "capturedUnixMs":1788360057764,
     "collectorFaults":[
       {"collector":"processTop","kind":"NotCollected",
        "detail":"per-process CPU attribution is produced by the bottleneck analyzer over ring
                  deltas; this sampler does not walk per-PID PDH (observer effect)"},
       {"collector":"memory.counters","kind":"Degraded",
        "detail":"counters unreadable: Standby Cache Reserve Priority Bytes"}],
     "cpu":{"contextSwitchesPerSec":8361,"dpcIsrBusyBp":0,"perProcessorBusyBp":[],
            "processorQueueLengthX100":0,"totalBusyBp":5838},
     "gpu":{"adapterId":"","adapterName":"","engineCount":16,...},
     "intervalMs":250,
     "memory":{"availablePhysicalBytes":4461096960,"memoryLoadPercent":73,
               "totalPhysicalBytes":16632156160,...},
     "platform":"windows","providerSource":"PerfPlatform",
     "power":{"throttleActive":false,"throttleReason":"none",...},
     "processTop":[],
     "storage":[{"deviceId":"physicaldisk:0 C:","friendlyName":"0 C:",...},
                {"deviceId":"physicaldisk:1 D:","friendlyName":"1 D:",...}]}}

### Checked against the host's own counters in the same window

Plausibility is not asserted; it is diffed against `Get-Counter`.

| reading | product | host, same window | verdict |
|---|---|---|---|
| cpu total busy | **5838 bp** (58.38%) | `\Processor Information(_Total)\% Processor Time` **50.91%** | agrees |
| context switches/sec | **8361** | `\System\Context Switches/sec` **10176** | agrees |
| cpu, a second run | 5343 bp | 50.34% | agrees |
| context switches, second run | 8630 | 8267 | agrees |
| PhysicalDisk instances | **2** (`0 C:`, `1 D:`) | `\PhysicalDisk(*)` exposes exactly `0 c:`, `1 d:`, `_total` | agrees |
| disk active time, disk idle | 0 bp | `% Disk Time` **0** on both disks | agrees |
| disk active time, disk busy | **102 bp** (1.02%) | `% Disk Time` **1.178%** | agrees |
| disk latency, disk busy | **833 µs** | `Avg. Disk sec/Transfer` **0.00024 s** = 240 µs | same order; see finding 4 |
| disk read bytes/sec, disk busy | **50 959** | non-zero | agrees |
| dpc+isr, quiet window | 0 bp | DPC 0% + Interrupt 0.279% | see finding 2 |
| dpc+isr, under disk I/O | **135 bp** (1.35%) | interrupt activity present | agrees |

### Against the brief's EXPECTED

| EXPECTED | observed | result |
|---|---|---|
| CPU busy **non-zero and plausible**; §41.15's order of magnitude (701 bp) | **5838 bp** under load, matching the host's 50.91% | **PASS** |
| storage returns real instances, or an explicit fault with a reason | **2 real instances**, both named, with live active-time/latency/read-rate | **PASS** |
| `capabilities` no longer claims `native` for a subsystem that reported nothing | 16/16 `native` — and all four telemetry collectors **did** measure | **PASS, with the caveat below** |

**The `capabilities` caveat, stated plainly.** The output is the same 16/16
`native` §41.15 recorded, so the value alone proves nothing. What changed is the
derivation and what it now sits beside: in §41.15, `telemetryStorage: native`
stood next to `"storage": []`; it now stands next to two real devices, and
`telemetryGpu: native` next to 16 engines instead of a false `Unavailable`. The
contradiction is gone because the collectors were fixed. That the *mechanism*
would degrade the row had they not been is proven by the three unit tests in
§42.2, not by this run — a run on healthy hardware cannot prove it, and treating
it as if it could is exactly the §20.1.6 error.

### Findings — recorded, not hidden

**1. `perProcessorBusyBp` is still `[]`. Not fixed, and not claimed to be.**
§20.1.3(b): no line of `windows_impl.rs` has ever written
`per_processor_busy_bp`. It is a missing feature, not the misread — populating it
needs per-instance `\Processor Information(N)\% Processor Time` counters across
22 processors, which is a cadence and observer-effect decision, not a bug fix.
**DBT-P42-009**, open.

**2. `dpcIsrBusyBp` reads 0 in a quiet window.** Not the old defect. The host's
own `% DPC Time` is 0 and `% Interrupt Time` 0.279% in the same window, and over
the provider's ~100 ms in-tick window that rounds to 0 bp. Under disk I/O the
same field reads **135 bp**, so the path is live. Recorded because the number
looks like the old symptom and is not.

**3. gpu `adapterId`, `adapterName` and the VRAM fields are empty** while
`engineCount` is 16. PDH exposes engine utilization only; adapter identity and
dedicated memory need DXGI traversal, which this provider has never done — the
file's own header comment says so. §41.16 3b measured both adapters and 8 GiB of
VRAM through WMI, so the data exists and this collector does not read it.
**DBT-P42-010**, open. It is honest today: the fields are empty, not invented.

**4. Byte-rate and latency counters under-report against a 1 s window.**
`readBytesPerSec` 50 959 and latency 833 µs come from the provider's 80 ms delta
window; `Get-Counter`'s 1 s window sees 9.89 MB/s and 240 µs at comparable
moments. Rate counters over a short window are truthful for that window and are
not comparable to a 1 s reading. Widening the window trades observer effect for
stability and is a cadence decision, not a defect fix. **DBT-P42-011**, open.

**5. The `memory.counters` Degraded fault is correct, and was verified.**
It names `Standby Cache Reserve Priority Bytes`. Probed independently:

    OK     \Memory\Standby Cache Normal Priority Bytes      2884845568
    ABSENT \Memory\Standby Cache Reserve Priority Bytes     The specified counter could not be found.
    OK     \Memory\Modified Page List Bytes                 71393280
    OK     \Memory\Pages Input/sec                          0
    OK     \Memory\Pages Output/sec                         0

The counter genuinely does not exist on this host. **This is the contract doing
its job**: an unreadable counter now produces a named degradation instead of a
silent zero. Under the shipping code this was `.unwrap_or(0)` and invisible.

**6. `processTop` now declares `NotCollected` on every snapshot.** A deliberate
wire change: §20.1.1 site 7 returned an empty array with no explanation. The
design (attribution belongs to the bottleneck analyzer) is unchanged; only the
silence is. One extra `collectorFaults` entry per snapshot.

### DBT-P42-008 — `perf snapshot` was serving a three-hour-old reading

Found while taking the 1.C service-backed comparison, and it is the same class of
defect as DBT-P41-002.

    NOW_UNIX_MS = 1788359855093
      call 1  capturedUnixMs=1788349015839  intervalMs=1000  cpuBusy=0
      call 2  capturedUnixMs=1788349015839  intervalMs=1000  cpuBusy=0
      call 3  capturedUnixMs=1788349015839  intervalMs=1000  cpuBusy=0
    §41.15 recorded capturedUnixMs = 1788349015839
    service PID 11324, StartTime 9/2/2026 12:09:29 PM

Three calls two seconds apart returned **byte-identical payloads**, and that
payload is the one §41.15 recorded **10 839 254 ms — three hours and one minute —
earlier**.

Cause, read from source: `PerformanceEngine::ensure_sample` was

    if self.ring.latest(owner).is_none() { ...sample and push... }

so the ring was seeded exactly once per owner per service lifetime, and every
later `perf snapshot` returned `latest()` — that first reading, unchanged. The
background sampler only runs after an explicit `perf start`, so in the common
case nothing ever refreshed it. The value returned was real; it was simply not a
measurement of *now*, and nothing in the response says how old it is.

**This blocked Part 2.D**, which requires re-running the 1.C measurements against
the installed service and expecting the same real numbers — impossible against a
frozen snapshot. Fixed: `ensure_sample` resamples when the latest reading is
older than the requested interval, or is dated in the future (a backwards clock
step must not pin a stale reading in place). A live sampler at that cadence keeps
the ring fresh and the fix adds no extra tick. The predicate is extracted as
`sample_is_stale` and unit-tested against the measured case:

    test performance::dbt_p42_008::an_empty_ring_is_stale ... ok
    test performance::dbt_p42_008::a_three_hour_old_reading_is_not_a_current_measurement ... ok
    test performance::dbt_p42_008::a_reading_inside_the_requested_interval_is_reused ... ok
    test performance::dbt_p42_008::a_future_dated_reading_is_stale ... ok
    test result: ok. 4 passed; 0 failed

The service-backed `perf snapshot` reading in §41.15 is therefore **not evidence
that the service path shows the same zeros** in the way it was read: it is
evidence that the service path *sampled once, at 12:16:55, and repeated itself*.
§20.1.9(1)'s prediction still holds — both paths call `default_platform()` and
share the provider — but the observation §41.15 offered as confirmation was a
replay, not an independent second measurement. Correcting the record.

**Part 1 verdict: DBT-P41-002 is FIXED and measured.** cpu, storage and gpu all
report real values that agree with the host's own counters; every subsystem that
reports nothing now says why.

## 42.4 DESTRUCTIVE ACTION RECORD — GATE 5: full lifecycle, zero survivors

**Written and committed BEFORE the first destructive step**, per the standing rule.

    ACTION=   1. Rebuild the MSI carrying the Part 1 fix (cargo -> tauri -> wix build
                 -> wix msi validate -> payload check). Non-destructive.
              2. msiexec /x {0F9F349D-01C8-B3C2-7242-83B5D29047C9} /qn /l*v — removes
                 the installed AetherCore 0.1.11 from this machine. DESTRUCTIVE.
              3. Fourteen-check survivor sweep, read-only.
              4. msiexec /i out\release\AetherCore.msi /qn /l*v — reinstall from the
                 MSI built in step 1, then re-prove every Gate 2 property.

    SNAPSHOT= taken and enumerated before step 2, not asserted:
              restore points  SequenceNumber 1  20260901223933  "AetherCore baseline
                                                — before any install"   <- PRE-INSTALL TARGET
                              SequenceNumber 2  20260902094609  "Windows Backup"
              disk image      D:\WindowsImageBackup  19 files  559,904,433,918 bytes
                              (521.45 GiB)  — present and listing-verified
              installed       AetherCore 0.1.11
                              ProductCode {0F9F349D-01C8-B3C2-7242-83B5D29047C9}
                              InstallDate 20260902
              service         AetherCoreMaintenance  Running  Auto  LocalSystem
              volumes         C: 953 GB (399 free)   D: "SD" 953.7 GB (410.5 free)

    EXPECTED= step 1: every exit code 0, `wix msi validate` output EMPTY, payload
                      check PASS, 16 file rows, new sha256 and byte size recorded.
              step 2: uninstall exit 0, verbose log written.
              step 3: ZERO survivors on all fourteen checks.
              step 4: install exit 0; 16 files hash-matching the built payload;
                      service LocalSystem/AUTO_START/RUNNING; service SID
                      UNRESTRICTED; pipe DACL equal to the Gate 2 criterion;
                      install-dir ACLs protected with Users read-execute only;
                      zero dev binaries; four verbs round-trip; engineLabel=localModel
                      proven against the RUNNING SERVICE.

    RECOVERY= stated honestly, including what it does NOT cover:
              - PRIMARY: reinstall from `out\release\AetherCore.msi`, which is built
                and validated in step 1 BEFORE the uninstall in step 2. The reinstall
                artifact provably exists before anything is removed. This is the
                recovery path for the realistic failure — an uninstall that succeeds
                and a reinstall that does not.
              - The Gate 0 restore point, SequenceNumber 1, is the **pre-install**
                target and is the only snapshot that predates the product.
              - The §41.13 disk image EXISTS and is verified (19 files, 521.45 GiB),
                but §41.12 0f.7 records that **the machine was already installed when
                it was imaged**. It is NOT a pristine-state image. Restoring it
                returns the machine to an installed state, not a clean one.
              - **The E: recovery media is NOT ATTACHED right now.** Measured, not
                assumed: `Get-Volume` shows only C: and D:. The external drive that
                held the 6.12 GB boot artifacts (§41.13 0f.D) has been removed. It was
                never boot-tested in any case (§41.17). So the "boot an unbootable
                machine" path is unavailable for the duration of this gate.

    BLAST=    Bounded to the AetherCore product: its install directory, service
              registration, ProgramData, registry keys and named pipe. Gate 5 does not
              touch the boot path, drivers, or any OS component — which is precisely
              why §41.17 records it as runnable before Gate 4 rather than after.
              The absent recovery media therefore does not gate it: nothing here can
              make the machine unbootable.

    NOT DONE= Defender, UAC, Firewall and SmartScreen are not touched. No survivor
              found in step 3 will be deleted by hand — the gate measures what the
              uninstaller does, not what can be cleaned up afterwards.

## 42.5 GATE 5 — 2.A: the MSI rebuilt with the Part 1 fix

| step | result |
|---|---|
| `pnpm --dir apps/ui build` | **exit 0** |
| `cargo build --release` (5 payload packages) | **exit 0**, 30.42 s |
| tauri `build --no-bundle --config installer/tauri.no-before-build.json` | **exit 0**, 2 m 06 s |
| `wix build -arch x64` | **exit 0** |
| `wix msi validate` | **exit 0, output EMPTY (length 0), ICE matches 0, no suppression** |
| `check-msi-payload.ps1` | `PAYLOAD_CHECK=PASS`, `AUTHORED_FILES=17`, `MSI_FILE_ROWS=16` |

### The new artifact

    MSI_BYTES   1,100,148,736          (previous 1,100,140,544 — +8,192)
    MSI_SHA256  6ecd1ee9786731d22741edbc10e8e0c14ca8add967fe7ebe7f365b21940702a3
    previous    d18d89db07180f5727b7d6056a07ea8d50de97aa401838601b532e9befd1f227
    version     0.1.11 (unchanged — the ProductCode is derived from version+arch,
                so it is the same {0F9F349D-01C8-B3C2-7242-83B5D29047C9})

The `beforeBuildCommand` defect was handled exactly as recorded: the existing
`installer/tauri.no-before-build.json` overlay was passed with `--config`.
**`tauri.conf.json` was not modified**, and `git status` confirms it.

### A payload trap worth recording

`scripts/build-installer.ps1` requires `vcomp140.dll` in the payload directory but
does not source it, and this machine carries **five** files of that name. The
first plausible match — `VC\Redist\MSVC\14.44.35112\onecore\x64\...`, 72,712
bytes — is the wrong one. §41.14's installed baseline is **193,152 bytes**,
sha256 `55aba23c…`, which is the **desktop** `x64` redist:

    72712   3b154db5fff1445a  ...\14.44.35112\onecore\x64\Microsoft.VC143.OpenMP\vcomp140.dll
    64168   6b78bc47b655c571  ...\14.44.35112\onecore\x86\Microsoft.VC143.OPENMP\vcomp140.dll
    193152  55aba23cdcd6484f  ...\14.44.35112\x64\Microsoft.VC143.OpenMP\vcomp140.dll   <- correct
    163488  91cbb2dbb3c4f279  ...\14.44.35112\x86\Microsoft.VC143.OPENMP\vcomp140.dll
    193152  55aba23cdcd6484f  C:\Windows\System32\vcomp140.dll

Caught by hashing against §41.14's recorded value before building, not after.
The staged file matches the Gate 2 baseline byte for byte. `onecore` is the
Windows-Core-OS variant and is not what the desktop product shipped.
**DBT-P42-012**: the build script names the file but not its source, so the next
session can silently ship a different binary that still passes every check.

**2.A = PASS.** Zero ICE, no suppression, 16 file rows, and the one payload
difference from ARM64 (`vcomp140.dll` for `libomp140.aarch64.dll`) is unchanged
and still architectural.

## 42.6 GATE 5 — 2.B: uninstall, and the fourteen-check survivor sweep

Never proven on this machine before — §41.8 records Gate 5 as NOT STARTED, and
§16.6's sweep was run on the ARM64 VM.

### Pre-uninstall state, measured

    INSTALLDIR_EXISTS=True   INSTALLDIR_FILES=16
    PROGRAMDATA_EXISTS=True
    SERVICE: STATE : 4  RUNNING

### The uninstall

    msiexec /x {0F9F349D-01C8-B3C2-7242-83B5D29047C9} /qn /l*v
    UNINSTALL_EXIT=0
    LOG_BYTES=148196        C:\AetherCore-P41\logs\p42\uninstall.log
    MainEngineThread is returning 0
    === Verbose logging stopped: 9/2/2026 18:02:12 ===

No 1603, and no `InstallValidate` return value 3 — the `PurgeMachineData`
ordering fix §16.6 landed after its first implementation failed holds on x64
with the service RUNNING at the start of the transaction.

### The sweep — thirteen checks clean outright

     1  INSTALLDIR              False          clean
     2  PROGRAMDATA             False          clean
     3  SERVICE                 absent(1060)   clean
     4  PIPE_COUNT              0              clean
     5  ARP_COUNT               0              clean
     6  HKLM_SOFTWARE_AETHER    False          clean
     7  HKCU_SOFTWARE_AETHER    False          clean
     8  STARTMENU               0              clean
     9  SCHEDULED_TASKS         0              clean
    10  FIREWALL_RULES          0              clean
    11  HKLM_SERVICES_KEY       False          clean
    12  EVENTLOG_SOURCE         0              clean
    13  HKEY_USERS_MARKERS      0              clean
    14  FILESYSTEM_SWEEP        16 raw hits    see below

Check 13 is the one §16.6 flagged as a stated limit — a per-user HKCU marker that
Windows Installer cannot reach in other users' hives. Swept across every loaded
hive under `HKEY_USERS`: **0**.

### Check 14, examined rather than waved away

The raw pattern is `*AetherCore*` under Program Files, Program Files (x86),
ProgramData and every user profile. It returned 16 hits. **The decisive
measurement is where they are:**

    C:\Program Files            0 hits
    C:\Program Files (x86)      0 hits
    C:\ProgramData              0 hits
    C:\Windows\System32         0 hits
    C:\Windows\SysWOW64         0 hits

    TOTAL=16   UNDER_USER_PROFILE=16   MACHINE_WIDE=0

**Zero hits anywhere the installer can write.** All 16 are under
`C:\Users\husen`, and each was identified by creation time and content:

| hit | created | what it actually is |
|---|---|---|
| `.claude\projects\C--dev-aethercore` | 01:28 | Claude Code's own session dir, named after the **repo path** `C:\dev\aethercore` |
| `AppData\Local\claude-cli-nodejs\Cache\C--dev-aethercore` | 12:40 | Claude Code cache, same naming |
| `AppData\Local\Temp\claude\C--dev-aethercore` | 01:22 | this session's scratchpad, same naming |
| `Temp\aethercore_elev_probe.txt` | 01:37 | the earlier session's elevation probe — 16.5 h before this uninstall |
| `Temp\aethercore-diag-*.db` (2) | 17:26, 17:27 | **`cargo test --workspace` artifacts** |
| `Temp\aethercore-phase4-cleaner-*` (6) | 17:26, 17:27 | **`cargo test --workspace` artifacts** |
| `Temp\aethercore-gd3-known-hosts-*`, `Temp\aethercore-ssh-stub-*` | 17:27 | **`cargo test --workspace` artifacts** (the fleet SSH tests) |
| `Recent\aethercore.lnk` | 01:22 | Explorer's Recent-items shortcut, OS-generated when the repo folder was opened |
| `OneDrive\<Documents>\aethercore-models` | 01:06 | a **user-created** staging copy of the GGUF + manifest + licenses, files dated 08-31 |

Three independent facts settle it:

1. **Nothing machine-wide survived.** The installer writes to `Program Files`,
   `ProgramData`, the service registry and the ARP key. All are empty (checks
   1, 2, 3, 5, 6, 11).
2. **The uninstall log references none of them** — `aethercore-models` 0 hits,
   `elev_probe` 0 hits, `aethercore-diag` 0 hits. The MSI never knew they existed.
3. **Every hit that post-dates the install under test came from `cargo test`
   at 17:26–17:27** — 35 minutes *before* the 18:02:12 uninstall — or from Claude
   Code's own cache. Not one was created by the product.

The `aethercore-models` folder is user data in the user's own Documents, which
`UNINSTALL.txt` and `ARPCOMMENTS` explicitly promise not to touch (§16.6). Leaving
it is the contract being honoured, not a survivor.

**2.B = PASS. Zero survivors on all fourteen checks.** Nothing was deleted by
hand; the sweep is read-only by construction.

### A finding the sweep produced, which is not an uninstaller defect

**DBT-P42-013 — `cargo test --workspace` leaves temp files behind.** Eleven files
under `%TEMP%` from one run: two `aethercore-diag-*.db`, six
`aethercore-phase4-cleaner-*`, one `aethercore-gd3-known-hosts-*`, one
`aethercore-ssh-stub-*`. They are small and harmless, but they are the reason a
naive `*AetherCore*` sweep reports survivors on a developer machine, and they
would make this gate ambiguous for anyone who ran the tests first. Recorded, not
cleaned up.

## 42.7 GATE 5 — 2.C: reinstalled from the 2.A MSI, every Gate 2 property re-proven

Nothing was assumed to have carried over from §41.14. Every criterion was
re-measured against the machine the 2.B sweep had just left bare.

    msiexec /i AetherCore.msi /qn /l*v      INSTALL_EXIT=0   log 167,744 bytes
    MSI_SHA256 6ecd1ee9786731d22741edbc10e8e0c14ca8add967fe7ebe7f365b21940702a3
    MsiInstaller 1033: "Product Name: AetherCore. Product Version: 0.1.11.
                        Installation success or error status: 0."

### Files — 16, hash-matched against the BUILT payload, not against a memory

    MATCHED=16   MISMATCH=0   NOT_IN_SOURCES=0

Every installed file was hashed and compared to the file in `out\payload` or
`assets` that the 2.A build actually consumed. **The two binaries carrying the
Part 1 fix differ from §41.14, as they must:**

    aethercore-maintenance-service.exe  10,693,120
      installed  bf46067b41223e3ee143aee09cf0927f67a9279e01730fa4f1da2b29b02c1f6e
      §41.14     221e486166707abbfe48af73796698feeb1d0233bde2fd4fc7572ae864a761d4
    aetherctl.exe                        4,329,984
      installed  a9ed0e561d6e56dcfbd3b3bd3e4bc477c9f372369debc3fcc4031b1c2083c9c1
      §41.14     910df7c9ea1010285320abbc3fffbc8469d5c8139c555b228e45151a6c13813d

The other 14 files hash **identically** to §41.14 — including the 1,117,320,736-byte
GGUF at `6a1a2eb6…` and `vcomp140.dll` at `55aba23c…`. So the diff between the
two installs is exactly the two binaries that changed, and nothing else moved.

### Every other Gate 2 criterion

| criterion | expected | observed | result |
|---|---|---|---|
| install transaction | success | MsiInstaller 1033, status 0, 0.1.11 | PASS |
| files | 16, hashes match the build | 16, MATCHED=16 MISMATCH=0 | PASS |
| VCOMP140 / LIBOMP_AARCH64 | True / False | True / False | PASS |
| dev binaries | none | `DEV_BINARY_IN_INSTALL_IMAGE=NO` | PASS |
| service | LocalSystem, AUTO_START, RUNNING | `SERVICE_START_NAME: LocalSystem`, `START_TYPE: 2 AUTO_START (DELAYED)`, `STATE: 4 RUNNING` | PASS |
| service SID | UNRESTRICTED, Active | `SERVICE_SID_TYPE: UNRESTRICTED`, `STATUS: Active`, SID `S-1-5-80-4285065559-…-1187574229` | PASS |
| install-dir ACLs | protected, Users read-execute only | `Users:(OI)(CI)(RX)`, Admins/SYSTEM `(F)`, service SID `(RX)` | PASS |
| registration | ARP + HKLM agree | both `0.1.11`, ARP `{0F9F349D-…}` InstallDate 20260902 | PASS |
| verbs | return; doctor typed rejection ok | 6/6 RETURNED with timings | PASS |
| **engineLabel** | **`localModel`** | **`localModel`** from `insights list` | **PASS** |

Verb timings, against the RUNNING SERVICE:

    service detect    EXIT 0    63 ms   {"state":"Reachable","endpointDir":"C:\ProgramData\AetherCore"}
    doctor            EXIT 5    21 ms   diagnostics.stateUnavailable   <- typed rejection, see §42.8
    scan status       EXIT 0    12 ms   {"appVersion":"0.1.11","state":"idle",...}
    insights list     EXIT 0    16 ms   {"engineLabel":"localModel","insights":[]}
    self-check        EXIT 0   669 ms   sha256Match true, manifestValid true, 1117320736 bytes
    optimize status   EXIT 0    14 ms   {"status":null}

### The pipe DACL, checked against the criterion

    PIPE_PRESENT=True
    PIPE_SDDL=O:S-1-5-80-4285065559-3530017622-2858480679-3751456793-1187574229
              G:SY
              D:P(A;;0x12008b;;;AU)(A;;FA;;;S-1-5-80-4285065559-…-1187574229)

Required: `O:<service SID> G:SY D:P(A;;FA;;;<service SID>)(A;;FR;;;AU)(A;;DC;;;AU)`

Owner = the service SID; group = `SY`; `D:P` protected; the service-SID `FA` ACE
present verbatim; the AU pair rendered merged as `0x12008b`, which is
`FR|DC = 0x120089|0x2` — **the rendering the brief names explicitly and instructs
must NOT be reported as drift.** Byte-identical to §41.14's reading. ACE ordering
differs from the authored string in the same way §41.14 recorded, and is likewise
not drift: a DACL is a set, and no principal gains or loses anything.

**The §41.14 correction reproduced exactly.** `P41-FULL-RUN.md`'s
`[System.IO.File]::Open('\.\pipe\…')` method was run alongside, and failed again:

    PIPE_SDDL_FILESTREAM=ERROR: FileStream was asked to open a device that was not
    a file. For support for devices like 'com1:' or 'lpt1:', call CreateFile...

`NamedPipeClientStream` is the method that works. Two sessions, same result.

### A correction to this session's own verification script

The first run reported `servicedetect EXIT 2` with empty output. That was **the
script's error, not the product's**: the verb is `service detect`, two tokens, and
the script had passed `service-detect`. Re-run correctly it is `EXIT 0` in 63 ms.
Recorded because an uninvestigated `EXIT 2` in a gate table would have been a
false failure.

**2.C = PASS.** Every Gate 2 property holds on the reinstalled product, and the
only difference from §41.14 is the two binaries that carry the fix.

## 42.8 GATE 5 — 2.D: the Part 1 fix live under the INSTALLED SERVICE

### First, DBT-P42-008 re-checked against the installed service

    NOW_UNIX_MS=1788361606833
      call 1  capturedUnixMs=1788361607607  cpuBusy=3356
      call 2  capturedUnixMs=1788361610040  cpuBusy=2243
      call 3  capturedUnixMs=1788361612446  cpuBusy=2296

Three calls, **three different timestamps**, each within a second of the call, and
three different CPU readings. Before the fix the same three calls returned
`capturedUnixMs=1788349015839` every time — a payload three hours old. The
stale-snapshot defect is fixed and proven live, not just unit-tested.

### `perf snapshot` against the RUNNING SERVICE, machine under load

    {"command":"perf snapshot","ok":true,"data":{
     "capturedUnixMs":1788361621459,
     "collectorFaults":[
       {"collector":"processTop","kind":"NotCollected","detail":"per-process CPU attribution ..."},
       {"collector":"memory.counters","kind":"Degraded","detail":"counters unreadable: Standby Cache Reserve Priority Bytes"}],
     "cpu":{"contextSwitchesPerSec":4746,"dpcIsrBusyBp":0,"totalBusyBp":5231},
     "intervalMs":1000,
     "memory":{"availablePhysicalBytes":3589976064,"memoryLoadPercent":78,
               "totalPhysicalBytes":16632156160,"hardFaultsPerSec":0},
     "power":{"throttleActive":false,...},
     "processTop":[],
     "storage":[{"deviceId":"physicaldisk:0 C:","friendlyName":"0 C:",...},
                {"deviceId":"physicaldisk:1 D:","friendlyName":"1 D:",...}]}}

    host, same window:  % Processor Time  48.223
                        Context Switches/sec  6371.887

### Against §41.15's service-path reading, field by field

| field | §41.15, installed service | now, installed service | host, same window |
|---|---|---|---|
| `cpu.totalBusyBp` | **0** | **5231** (52.31%) | 48.22% |
| `cpu.contextSwitchesPerSec` | **0** | **4746** | 6372 |
| `storage` | **`[]`** | **2 named devices** | 2 PhysicalDisk instances exist |
| gpu fault | `Unavailable` "no GPU engine counters exposed by this adapter/driver" | **no gpu fault** — gpu measured | 568 engine instances exist |
| faults | 1, and its stated cause was false | 2, both true and both verified | — |
| `capturedUnixMs` | frozen at 1788349015839 across calls | fresh every call | — |

**EXPECTED: the same real numbers 1.C produced. Observed: yes.** The offline path
gave 5838 bp against a host reading 50.91%; the service path gives 5231 bp
against 48.22%. Both agree with the host and with each other, and both carry the
identical two honest faults. §20.1.9(1) predicted from source that the two paths
share the provider and must behave alike — that now holds with both of them
*correct*, rather than with both of them silently zero.

`telemetry-once` from the **installed** `aetherctl.exe` matches: `totalBusyBp`
3446, `contextSwitchesPerSec` 21854, `processorQueueLengthX100` 100, 2 storage
devices, `engineCount` 16.

### The `doctor` exit-5 anomaly — RESOLVED, and confirmed state-dependent

The brief asked whether it persists, changed, or resolved. Measured both ways on
the freshly reinstalled product:

    doctor, immediately after install (no diagnostic state yet)
      EXIT 5   {"ok":false,"error":{"kind":"RejectedByService",
                "message_key":"diagnostics.stateUnavailable"}}

    scan start -> scanId 0931257a-e51d-46a6-93fa-de264ee06339
      SCAN_STATE=completed   duration 299 s
      collectorCount 7  factsCount 462  findingCount 445
      remediationCandidateCount 287  warningCount 0
      fingerprint 9ccfd3d510f8c5ddc90226f1e3ef2ed71df827bea0341d8ae3517a3fb5755178

    doctor, after the scan — same binary, same service
      EXIT 0   {"ok":true,"data":{"state":"Ready","cardCount":1,"crashCount":0,
                "eventCount":128,"eventWindowDays":30,"providerFaults":[],
                "storageCount":2,"warningCount":0,"warnings":[]}}

**Not a defect: a typed rejection that is correct when no diagnostic state
exists.** §41.16 reached the same conclusion; this reproduces it from a clean
install rather than from a service that had been up for hours, which is the
stronger form of the observation.

### And it closes §41.16's last DBT-P41-002 contrast

§41.16 recorded, as further evidence of the defect, that `doctor` reported
`storageCount: 3` in the same session where `telemetry-once` and `perf snapshot`
both returned `"storage": []` — the product could plainly enumerate storage
through `hardware-telemetry` while `performance-telemetry` yielded nothing.

Now `doctor` reports `storageCount: 2` and both perf paths report **the same 2
devices**. The count is 2 rather than 3 because the external HIKSEMI drive that
was attached in §41.16 has since been detached — `Get-Volume` shows only C: and
D:, and `\PhysicalDisk(*)` exposes exactly `0 c:`, `1 d:` and `_total`. **The two
subsystems agree for the first time**, and they agree with the host.

### Scan comparison with §41.16, for the record

| | §41.16 (0.1.11, pre-fix) | now (0.1.11 + P42 fix) |
|---|---|---|
| duration | 342.5 s | 299 s |
| collectorCount | 7 | 7 |
| factsCount | 487 | 462 |
| findingCount | 469 | 445 |
| remediationCandidateCount | 299 | 287 |
| warningCount | 0 | 0 |

The lower fact/finding counts are consistent with one fewer attached disk, and
`warningCount` is 0 in both. No regression in the scan pipeline.

`insights list` re-checked after the scan: `engineLabel` still **`localModel`**.

### Security posture, re-measured at gate end rather than asserted

    DEFENDER_REALTIME  True    DEFENDER_ANTIVIRUS  True    DEFENDER_TAMPER  True
    PUAProtection 2   MAPSReporting 2   SubmitSamplesConsent 1
    UAC_EnableLUA 1   UAC_ConsentPromptAdmin 5
    Firewall Domain/Private/Public  True/True/True
    AetherCoreMaintenance  Running
    SmartScreen: Explorer\SmartScreenEnabled '' , AppHost\EnableWebContentEvaluation '' ,
                 Policies\...\EnableSmartScreen does not exist
                 -> no override disabling it; Windows default, unchanged

Identical to §41.18. Nothing was disabled, weakened or worked around, across an
uninstall and a reinstall.

**2.D = PASS. GATE 5 = PASS.**

## 42.9 P42 FINAL REPORT

### Gate table — evidence on every line

| gate / item | proves | result | evidence, in numbers |
|---|---|---|---|
| 1.A | the tests that should have caught DBT-P41-002 | **DONE, committed FAILING** | 4 tests, 4 FAILED at `7818817`; cpu all-zero, storage `[]`, processTop `[]`, 3 capabilities contradicting collectors |
| 1.B | one contract replaces nine availability rules | **DONE** | 9 -> 1; 5 rules deleted, 4 converted; 3 further defects found (P42-001/002/003); 7/7 tests pass |
| 1.C | the fix measured on this machine | **PASS** | cpu **5838 bp** vs host **50.91%**; storage **2 named devices**; gpu **16 engines**; dpc+isr **135 bp** under I/O |
| 2.A | MSI rebuilt with the fix | **PASS** | 5 steps exit 0; `wix msi validate` exit 0, output **EMPTY**, **0** ICE; payload PASS, **16** rows; sha256 `6ecd1ee9…`, 1,100,148,736 bytes |
| 2.B | uninstall, zero survivors | **PASS** | uninstall exit **0**; 13/14 clean outright; check 14 **0** machine-wide hits (Program Files 0, ProgramData 0, System32 0) |
| 2.C | reinstall, every Gate 2 property | **PASS** | install exit 0, MsiInstaller 1033 status 0; **16/16** hashes match the built payload; SID **UNRESTRICTED**; pipe DACL equal; **`engineLabel=localModel`** |
| 2.D | the fix live under the installed service | **PASS** | service cpu **5231 bp** vs host **48.22%**; `perf snapshot` fresh every call; `doctor` exit 5 -> **exit 0** after a scan |
| **5** | **full lifecycle, zero survivors** | **PASS** | first time proven on this machine; §41.8 had it NOT STARTED |
| 4 | driver install + rollback | **NOT STARTED — HARD STOP** | unchanged; see below |

### How many of the nine availability rules survived

**None survived as an independent rule. Nine deciders became one.** Five were
deleted outright; four remain only as *reason strings* the type now forces the
collector to supply — they no longer decide availability, they explain it.

Deleted: power's unconditional-true dead `else` (site 3), gpu's bespoke
`engines.is_empty()` (site 6 — `Reading::from_collection` does it for every
collection payload), processTop's silent `Vec::new()` (site 7), the CLI's second
independent gpu rule (site 8), and the static `native` capability table's
unconditional claim (site 9). Converted: cpu query-open and cpu collect-failure
(sites 1, 2), memory (site 4, split into subsystem-unavailable vs a
`memory.counters` partial), storage query-open (site 5, and the four fault-free
exits it never covered).

Full table in §42.2.

### Did the stack-overflow fix change any ARM64 behaviour

**Yes, and this is the explicit statement the brief asked for.**
`windows_impl.rs` is `#[cfg(windows)]`, not `#[cfg(target_arch)]`, so the ARM64
Windows pipeline runs this exact file. Every change applies to it identically:
the 8-byte destination for a 16-byte `PDH_FMT_COUNTERVALUE` write is removed (the
ABI is the same on both targets per §20.1.7); the 3-versus-5 parameter
`PdhExpandWildCardPathW` call is corrected — and P36 touched that very binding
*for ARM64*, to fix an LNK2019, without the arity being noticed; and cpu, storage
and gpu will report real numbers where ARM64 previously reported the same zeros.

It is a **fix of the same defect, not a divergence** — there is no ARM64-only code
path to diverge. It is **unverified on ARM64 silicon**: no ARM64 machine is
attached to this session. Recorded as **DBT-P42-004**. The diff is the whole of
`crates/performance-telemetry/src/windows_impl.rs` in commit `cc9c51e`.

### Recorded rather than worked around

| id | what | disposition |
|---|---|---|
| **DBT-P42-001** | `PdhExpandWildCardPathW` bound with 3 params where the export takes 5; out-buffer typed `*mut PWSTR` instead of `PZZWSTR` | **FIXED.** Listed because it *resolves* §41.15's open "which of storage's four exits fires" — the answer is neither candidate: the call never had a chance to succeed |
| **DBT-P42-002** | percentage counters read as basis points (91 bp reported while the box was 91% busy) | **FIXED** |
| **DBT-P42-003** | counters read before the collection that gives them data; storage collected twice before adding any counter | **FIXED** |
| **DBT-P42-004** | the ARM64 pipeline runs the changed file and is unverified on ARM64 silicon | **CLOSED, §43.7** — build exit 0, 7/7 regression tests pass, offline readings agree with the host in two rounds; residual narrowing: service-path (Part 4) not run, disk-latency unmeasured on this VM |
| **DBT-P42-005** | macOS/Linux providers still build `PerfSnapshot` literally, not through `CollectedSubsystems` | open — `#[cfg]`-gated; cannot be compiled or tested on this host, and changing code this session cannot build is the worse risk |
| **DBT-P42-006** | `aethercore-driver-hub --lib`, 6 failing tests | **PRE-EXISTING**, verified by stashing this session's changes and re-running; the crate depends on neither crate P42 touched |
| **DBT-P42-007** | `intelligence-core --test offline_boundary` fails | **PRE-EXISTING**; `cargo metadata --offline` cannot find `android_system_properties v0.1.6` in the local registry cache. Environment, not code |
| **DBT-P42-008** | `perf snapshot` served a 3-hour-old reading as a live one | **FIXED**, and it had to be — 2.D is impossible against a frozen snapshot |
| **DBT-P42-009** | `perProcessorBusyBp` still `[]` on Windows | open — a missing feature, never written since Phase 20; populating it is a cadence/observer-effect decision, not a bug fix |
| **DBT-P42-010** | gpu adapter identity and VRAM still empty; PDH gives engines only | open — DXGI adapter traversal was never implemented; §41.16 3b shows the data exists via WMI. Honest today: empty, not invented |
| **DBT-P42-011** | byte-rate and latency counters under-report against a 1 s window | open — truthful for the provider's 80 ms window; widening it trades observer effect for stability, a cadence decision |
| **DBT-P42-012** | `build-installer.ps1` requires `vcomp140.dll` but does not source it; five files of that name exist and the first plausible match is wrong | open — caught by hashing against §41.14 before building; the script is unchanged, so the trap is still there |
| **DBT-P42-013** | `cargo test --workspace` leaves 11 files under `%TEMP%` | open — harmless, but it is why a naive `*AetherCore*` sweep reports survivors on a developer machine |
| DBT-P41-001 | x64 service imports `MSVCP140`/`VCRUNTIME140`, present on this box but not in the payload | **open, unchanged** — not re-measured this session, and this machine cannot detect the gap because it has the redistributable |
| DBT-P41-002 | cpu zero / storage empty with no fault | **CLOSED.** §42.2 fixed it at the type; §42.3 and §42.8 measured it fixed on both paths |
| DBT-P41-002a | the gpu fault's detail string names a false cause | **CLOSED.** gpu now measures 16 engines; the false string is gone |
| DBT-P41-002b | the 8-byte destination for a 16-byte PDH write | **CLOSED structurally** — there is no longer a smaller type to pass. Note the *original* question ("does it corrupt anything observable?") is now moot rather than answered |

### What Gate 4 still needs from the owner

Unchanged from §41.17, and this session did **not** start it. Two of the three
items are now sharper, not softer:

1. **Boot-test the recovery media — and re-attach it first.** §41.17 recorded the
   E: artifacts as complete but never boot-tested. **This session measured that
   E: is no longer attached at all**: `Get-Volume` shows only C: and D:. The
   external drive holding the 6.12 GB BIOS+UEFI boot chain has been removed. So
   the precondition is now two steps, not one: re-attach it, then boot from it
   once and confirm the recovery environment can read `D:\WindowsImageBackup`.
2. **Supply the driver to test.** Windows Update offers this machine **zero**
   driver updates (§41.16 3c, search-only, ResultCode 2 = succeeded). The
   "deliberately safe device" has no WU-supplied candidate and the owner must
   nominate one — a printer-class, HID-class or USB-peripheral driver; not
   storage, not chipset, not GPU.
3. **Decide about the Intel Arc driver, separately from Gate 4.** 31.0.101.5007
   dated 2023-11-18, against NVIDIA's 2026-08-20. An observation, not a Gate 4
   dependency — and a display driver is exactly the class that must NOT be the
   safe test device.

The disk image remains present and verified (19 files, 559,904,433,918 bytes) and
remains a **post-install** capture, not a pristine one (§41.12 0f.7).

### Security posture

Re-measured at session end, not asserted from memory: Defender real-time,
antivirus and tamper protection all True; PUA 2; MAPS 2; UAC `EnableLUA` 1 and
`ConsentPromptBehaviorAdmin` 5; all three firewall profiles True; no registry or
policy value disabling SmartScreen. Identical to §41.18, across a full uninstall
and reinstall. **No security regression.**

# PHASE 43 — VERIFY THE P42 FIX ON ARM64, AND SETTLE THE NUMERIC BIAS (2026-09-02)

Brief: `phase21-workspace/docs/phase41/P43-ARM64-VERIFY.md`. This session runs on
the Mac at `/Users/hasanalaaa/dev/aethercore` and drives the Parallels "Windows 11"
VM through `prlctl`; it is not a session inside the VM.

## 43.0 RESUME THE VM

**Observed differs from the brief's assumption, recorded verbatim rather than
theorised about:** the brief says "The VM is currently suspended. Resume it."
`prlctl list -a` at the start of this session showed:

    STATUS       IP_ADDR         NAME
    running      -               Windows 11

The VM was already running, not suspended — no `prlctl resume` was needed or run.
`prlctl snapshot-switch` was not run either way, consistent with the brief's
standing prohibition.

Current snapshot, read via `prlctl snapshot-list "Windows 11"` (marked `*`):

    {d652cd40-877c-4a9a-bb1b-2e3637a96ec2}

Neither forbidden UUID (`P36-CLEAN-BASELINE {6b721a10-...}` /
`P36-PRE-NATIVE-MUTATION {e9434b5f-...}`). Confirmed reachable:

    prlctl exec "Windows 11" cmd.exe /c "echo REACHABLE && ver"
    REACHABLE
    Microsoft Windows [Version 10.0.26200.9168]

State recorded before anything was changed:

    installed AetherCore product   0.1.11   InstallDate 20260901
    ProductCode                    {98FCE2D5-44F0-A27C-A48B-8720FFE672F0}
      (distinct from the x64 ProductCode {0F9F349D-...} in §42.4 — expected,
      the ProductCode is derived from version+arch per §42.5)
    service AetherCoreMaintenance  Running, Automatic

## 43.1 PART 1.A — bringing the VM's working copy to a39a1bc

**How the VM's working copy is fed: a copy, not a git clone, and not the share
directly.** `p36_relbuild.cmd` (`C:\AetherCore-P36\logs\p36_relbuild.cmd`) `cd`s
into `C:\AetherCore-P36\workspace\AetherCore-Phase35-Master-Delivery` before
building. That directory has no `.git` (`Test-Path ...\.git` → `False`), so
`git log --oneline -1` / `git config core.autocrlf` — the brief's literal
verification commands — do not apply; there is no git repository to ask. The
underlying concern (CRLF corruption breaking the project's SHA256 checks) is
answered a different way below.

A live path from the Mac does exist and was not previously being used to feed
the build: the VM has a Parallels shared folder, reachable inside `prlctl exec`
sessions via UNC (`\\Mac\dev\...`) even though the interactive user's mapped
drive letters — `Y: \\Mac\dev` per `net use` — are **not** visible to a
non-interactive `prlctl exec` session (`Get-PSDrive` shows no `Y:` there; the
UNC path works regardless of drive-letter mapping). Recorded because it cost
real time: `Get-ChildItem Y:\` silently found no drive, `\\Mac\dev\...` worked.

**Before syncing**, hashed three files that changed in P42 against the VM's
existing copy, to establish what state it was actually in:

    file                                          Mac (a39a1bc)   VM (before sync)
    crates/performance-telemetry/src/windows_impl.rs  e8bad8bd...   ec42d793...  MISMATCH
    crates/performance-telemetry/tests/dbt_p41_002.rs f7413192...   MISSING      MISMATCH
    crates/performance-telemetry/src/lib.rs           6e3e1bf8...   fa49f1ae...  MISMATCH

`dbt_p41_002.rs` — the test file P42 Part 1.A added — did not exist on the VM at
all. **The VM's copy pre-dated P42**, confirming the brief's premise directly
rather than assuming it.

**Bring-to-a39a1bc, verified before acting on it**: `git diff a39a1bc HEAD --stat`
on the Mac shows the only change since `a39a1bc` is the addition of
`docs/phase41/P43-ARM64-VERIFY.md` itself (194 insertions, one file) — so the
Mac's current working tree is byte-identical to `a39a1bc` for every source file.
Synced `apps/`, `crates/`, `services/`, `tools/`, `.cargo/`, `Cargo.toml` and
`Cargo.lock` from the Mac's `phase21-workspace` to the VM's build directory via
`robocopy /MIR` over the UNC share (binary SMB copy, no line-ending translation
— this is what answers the autocrlf concern without git being present).
Robocopy exit codes: `apps`=3, `crates`=3, `services`=3, `tools`=1, `.cargo`=1 —
all in the 0-7 "success" range (codes ≥8 would be a failure; none seen).

**Verified after syncing**, same four files plus one more:

    file                                                Mac (a39a1bc)   VM (after sync)
    crates/performance-telemetry/src/windows_impl.rs    e8bad8bd...     e8bad8bd...   MATCH
    crates/performance-telemetry/tests/dbt_p41_002.rs   f7413192...     f7413192...   MATCH
    crates/performance-telemetry/src/lib.rs             6e3e1bf8...     6e3e1bf8...   MATCH
    crates/platform-capabilities/src/lib.rs             01a6da44...     01a6da44...   MATCH

**1.A verdict: the VM's working copy now matches `a39a1bc` for every source file
checked**, verified by SHA256 rather than by `git log`, because the working copy
is fed by copy rather than by clone.

## 43.2 PART 1.B — ARM64 release build

Ran `C:\AetherCore-P36\logs\p36_relbuild.cmd` unmodified (VsDevCmd arm64 +
clang-cl + Ninja + libomp, per the brief), redirected to a log, timed:

    SCRIPT_EXITCODE=101   DURATION_SEC=80.99   LOG_BYTES=57614

**101, not 0 — but this is not the shipping build failing, and the raw log
proves it rather than assuming it.** `p36_relbuild.cmd` runs two `cargo build`
commands with `if errorlevel 1 exit /b 1` gating the second on the first:

    cargo build --release -p aethercore-maintenance-service -p aetherctl
    if errorlevel 1 exit /b 1
    cargo build --release -p aethercore-ipc --example ipc_two_client_probe

The log shows the first command's own terminal line —

    Finished `release` profile [optimized] target(s) in 1m 17s

— with no `error:` anywhere above it, only warnings (5 on `aetherctl`, 18 on
`aethercore-maintenance-service`, none new or P42-related). Because the script
reached the *second* `cargo build` line at all, `errorlevel` after the first
command was provably 0 — the gate would have exited the script before line 2
otherwise. **The shipping binaries (`aethercore-maintenance-service.exe`,
`aetherctl.exe`) built clean: EXPECTED exit 0, OBSERVED exit 0, in 1m 17s.**

The 101 comes entirely from the second, non-shipping command:

    error: no example target named `ipc_two_client_probe` in `aethercore-ipc` package

Checked against the synced Mac source (`a39a1bc`): `crates/ipc` has no
`examples/` directory and no `[[example]]` entry in its `Cargo.toml` — this
target does not exist in the current tree at all, on either machine. It is a
stale reference in the P36-era recipe script itself (matching the `p36-ipc-test`
/ `tranche1-ipc-test2` / `prod-ipc.ps1` artifacts already sitting in
`C:\AetherCore-P36\incoming\`, left over from that era), not something P42
removed or broke. **DBT-P43-001** — the build recipe's second step references a
probe example that no longer exists; the recipe's overall exit code is
misleading (101) even when the shipping build it exists to validate is clean.
Recorded, not fixed — the recipe script is out of this session's scope.

**1.B verdict: EXPECTED exit 0 — OBSERVED, for the shipping build specifically.
The recipe script's own exit code (101) is not usable as the pass/fail signal
as written**, because of DBT-P43-001. Part 1 continues.

## 43.3 PART 1.C — the dbt_p41_002 regression tests on ARM64

    cargo test -p aethercore-performance-telemetry --test dbt_p41_002 -- --nocapture

Built clean under the same toolchain env as 1.B (`Finished `test` profile
[unoptimized + debuginfo] target(s) in 51.13s`), no errors, warnings unrelated
(an unused import in `platform-capabilities`, a dead field in
`PerformanceRing`, neither touching the tested behaviour).

    running 7 tests
    test a_percentage_counter_becomes_basis_points ... ok
    test a_bad_status_is_not_decoded_as_a_double ... ok
    test instance_is_parsed_out_of_an_expanded_counter_path ... ok
    test pdh_value_is_decoded_from_large_value_not_cstatus ... ok
    test real_windows_provider_reports_non_zero_cpu_under_load ... ok
    test no_collector_returns_an_empty_payload_without_a_fault ... ok
    test capabilities_never_claim_native_for_a_subsystem_that_reported_nothing ... ok

    test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

**EXPECTED: the four `dbt_p41_002` tests pass on ARM64 too. OBSERVED: all 7 in
the file pass** — the original four the brief named plus the three unit tests
1.B added for the PDH-decode, percentage-to-bp and counter-path mechanisms.
Identical result set to §42.2's post-fix x64 run (same 7 test names, all `ok`).
**No architecture divergence in Part 1.** Part 1 verdict: the fix builds and
passes its own regression suite on ARM64, matching x64 exactly.

## 43.4 PART 2.A/2.B — product readings vs host counters, under load

**A methodology note that cost real time.** This VM has 4 logical processors
(`[Environment]::ProcessorCount`), not x64's 22. A single `prlctl exec` script
that both generated CPU/disk load (via `Task.Run` inside the same process) and
then queried `telemetry-once`/`Get-Counter` failed intermittently and
non-deterministically — `PrlVmGuest_RunProgram: Unable to open new session` on
some attempts, `PrlJob_GetResult: Invalid argument` on another, success on
others, with no state left behind on the VM to explain it (checked: no stray
processes, CPU idle between attempts). Load generated via `System.Threading.
Tasks.Task` inside that same process also did not sustain — `Get-Process` on
the launching PowerShell host showed under half a CPU-second consumed after
40+ seconds of intended spinning, so the "load" in those attempts was mostly
not real. **What worked reliably: separate, short `prlctl exec` calls — one
per `Start-Process`, launching genuine independent OS processes (each a plain
`powershell -Command "while(1){}"`) — then a separate short call to measure.**
Real sustained load was confirmed before trusting any reading:
`Get-Process` showed three processes each accumulating ~25-26 CPU-seconds
over ~3 elapsed seconds (i.e. near-saturated), and a `Get-Counter` check read
75.7% before the product was ever queried.

Binary under test: `target\release\aetherctl.exe`, built this session (1.B).
`telemetry-once` is offline (§41.15's correction — it cannot cross the
service boundary), so this is a direct measurement of the fix; it does not by
itself say anything about the service path (see §43.6 / Part 4).

### Round 1

    product   totalBusyBp 7670 (76.70%)   contextSwitchesPerSec 344
    host      \Processor Information(_Total)\% Processor Time   75.85%
              \System\Context Switches/sec                       415
    storage (product): [{"deviceId":"physicaldisk:0 C:", activeTimeBp:0, avgTransferLatencyUs:0, readBytesPerSec:0, writeBytesPerSec:0}]
    storage (host):    \PhysicalDisk(0 c:)\% Disk Time = 0, \Avg. Disk sec/Transfer = 0, \Disk Write Bytes/sec = 0
    gpu (product): engineCount 16, no fault
    perProcessorBusyBp: []

### Round 2

    product   totalBusyBp 7499 (74.99%)   contextSwitchesPerSec 2165
    host      \Processor Information(_Total)\% Processor Time   75.46%
              \System\Context Switches/sec                       132
    storage/gpu/perProcessorBusyBp: identical shape to round 1, all zero on disk

**Storage: honestly zero on both sides, not a defect being hidden.** The disk
load process (write+read a 16 MB file in a loop, no `WriteThrough`) was
confirmed running throughout — the same live-load check above included this
process — but neither the product nor `Get-Counter` ever registered non-zero
`% Disk Time` / write-rate on `PhysicalDisk(0 c:)` in either round. This VM's
single virtual disk absorbs a 16 MB overwrite loop fast enough that it never
shows as "busy" at a ~250 ms/1 s sampling grain — a property of this disk and
this workload, not a product defect: the host counter reads exactly the same
zero the product does. **No disk-latency comparison point exists for ARM64**,
unlike x64's `833 µs vs 240 µs` reading — recorded as a limitation rather than
omitted silently.

Cleanup: all launched load processes stopped, temp files removed, verified
before moving on (`Get-Process powershell` showed none but the query itself).

## 43.5 PART 2.C — the numeric bias question, settled

x64's bias, from the brief (§42.3 / §42.8):

    cpu offline    product 58.38%   host 50.91%    delta +7.47 pts   ratio 1.147
    cpu service    product 52.31%   host 48.22%    delta +4.09 pts   ratio 1.085
    disk latency   product 833 us   host 240 us    delta +593 us     ratio 3.47

ARM64, this session, offline path only (two rounds, §43.4):

    round 1   product 76.70%   host 75.85%   delta +0.85 pts   ratio 1.011
    round 2   product 74.99%   host 75.46%   delta -0.47 pts   ratio 0.994

**Observed outcome: ARM64 shows no consistent bias.** x64's deltas were
positive in every one of three independent readings (two cpu, one disk),
ranging +4.1 to +7.5 points on cpu and 3.47x on disk latency — the same
direction every time, which is what made the brief call it "not sampling
noise" in the first place. ARM64's two cpu deltas are an order of magnitude
smaller (+0.85, -0.47 points) and **flip sign** between rounds, which is the
signature of measurement noise around zero, not a systematic overshoot.

**This is the brief's second, "surprising" branch: the bias reads as
x64-specific, not shared logic.** Stated with the honest limits on this
result: only two rounds, offline path only (the service path needs the
installed-service reading — Part 4, snapshot-gated, not run in Part 2), and
no disk-latency point exists for ARM64 at all (§43.4) so the disk half of
x64's bias is simply unmeasured here, not confirmed absent. **DBT-P42-011 is
not widened by this session** — the evidence points away from a shared-logic
cause, not toward one. Do not fix it, per the brief; this session establishes
which branch, not a repair.

## 43.6 PART 2.D — `perProcessorBusyBp` on ARM64

`[]` in every capture this session — both rounds in §43.4, and the earlier
unloaded baseline read during the session-flakiness investigation. Never `[]`
alternating with populated, never 22 (or 4) zero entries — consistently the
empty array `[]`.

**EXPECTED `[]`, confirming a feature gap rather than an architecture-
dependent symptom. OBSERVED: `[]`, matching.** §20.1.3(b) and DBT-P42-009 both
describe this as `per_processor_busy_bp` never being written on Windows at
all (`sample_cpu` starts from `CpuSample::default()` and no line of
`windows_impl.rs` assigns the field) — a fact about the source, not about
either architecture's counters, and this session's ARM64 reading is
consistent with that on every capture.

## 43.7 PART 3 — DBT-P42-004 VERDICT: CLOSED

**CLOSED**, with two residual items named rather than folded silently into
"closed":

- Shipping ARM64 build: exit 0 (§43.2, proven from the log despite the
  recipe script's own misleading 101 — DBT-P43-001).
- The regression suite the fix carries: 7/7 pass on ARM64, identical test
  names and results to x64 (§43.3).
- The fix's own behaviour, measured twice against this machine's real host
  counters, offline path: cpu within ~1 point of the host both times, gpu
  measuring 16 engines with no false fault, storage device enumerated —
  the same shape §42.3 measured on x64, not merely "no crash" (§43.4).
- `perProcessorBusyBp` still `[]`, matching the source-level diagnosis
  rather than either architecture's counters (§43.6) — not a divergence,
  a confirmation that DBT-P42-009 is what it was always recorded as.

**Not proven, and said so rather than assumed:**

1. **The service path.** Everything measured this session went through
   `telemetry-once` (offline). `perf snapshot` needs the fix running as the
   registered service, which needs an MSI install — Part 4, snapshot-gated,
   and this session's Part 4 decision is recorded separately (§43.8). The
   reason this doesn't reopen DBT-P42-004 rather than merely narrow it:
   §20.1.9(1) and §42.8 already established, and this session did not
   need to re-establish, that both paths call the same
   `default_platform()` and share the identical `WindowsPerfPlatform`
   provider — there is no second code path for the service to diverge
   through.
2. **Disk latency.** x64 had a bias reading to check (§43.5); this VM's
   single virtual disk never registered non-zero `% Disk Time` under load
   on either the product or the host counter (§43.4), so there is no
   ARM64 disk-latency number at all, favourable or not. Unmeasured, not
   confirmed absent.

**A finding this session added, not subtracted:** the x64 numeric bias
(§43.5) does not reproduce on ARM64 in the two rounds measured — the
opposite of what would have widened DBT-P42-011. This sits beside the
DBT-P42-004 verdict rather than inside it: DBT-P42-004 asks whether the P42
fix works on ARM64 (yes), not whether x64's separately-open bias question
also applies here (this session's evidence says no, or at least not by the
same mechanism).

## 43.8 PART 4 — NOT TAKEN, decision recorded rather than silently skipped

The brief gates Part 4 on two conditions: Parts 1-3 committed clean (true —
§43.1-§43.7, four separate commits, all pushed), and that it "adds something
Part 2 did not." **Judged that it does not, for this session's brief, and Part
4 was not run.**

Reasoning:

- The question Part 4 would additionally answer is whether the fix behaves
  the same way when reached through the installed **service** rather than
  offline. That is not open speculation this session had to re-derive:
  §20.1.9(1) and §42.8 already established, on x64, that both paths call the
  identical `default_platform()` and share the one `WindowsPerfPlatform`
  provider — there is no second, service-specific code path for ARM64 to
  diverge through that Part 2's offline measurement did not already exercise.
- Gate 5 (full lifecycle, zero survivors) is separately recorded in §41.17 as
  **already proven on ARM64** in an earlier session, unlike x64 where P42 had
  to prove it for the first time. Part 4 here would be re-proving already-
  proven Gate 2/5 properties against the new binaries, not establishing them
  for the first time the way it was for x64.
- Part 4 is destructive (an MSI install over the currently-installed 0.1.11),
  requires its own ACTION/SNAPSHOT/EXPECTED/RECOVERY record and a new named
  snapshot before the first step, and a full MSI rebuild-validate-install-
  reprove cycle — real additional work, gated by the brief specifically so it
  is not taken by default.

**This is a judgement call, not a limitation the session ran out of time
for** — recorded so a future session does not read the absence of Part 4 as
an oversight. If service-path re-confirmation on ARM64 becomes independently
useful later (e.g. before a release that ships the ARM64 build), it can be
run on its own without re-doing Parts 1-3.

The VM's install state was left exactly as found (§43.0): AetherCore 0.1.11,
ProductCode `{98FCE2D5-44F0-A27C-A48B-8720FFE672F0}`, service Running/
Automatic. Nothing in Parts 1-3 touched the installed product — the fix was
built and measured from `target\release\`, never installed.

## 43.9 P43 REPORT

**Build and test.** Shipping ARM64 release (`aethercore-maintenance-service`,
`aetherctl`): exit 0, `Finished` in 1m 17s (§43.2) — proven from the log even
though the recipe script's own exit code was 101, caused by an unrelated,
pre-existing stale `--example` reference (DBT-P43-001), not by the shipping
build. `cargo test -p aethercore-performance-telemetry --test dbt_p41_002`:
7/7 passed in 3.29s (§43.3), identical test set and result to x64's post-fix
run.

**Every product reading beside the host's, offline path, two rounds (§43.4):**

    round 1   cpu   product 76.70%   host 75.85%   delta +0.85 pts   ratio 1.011
    round 2   cpu   product 74.99%   host 75.46%   delta -0.47 pts   ratio 0.994
    both rounds   storage/disk-time/disk-write-rate:  0 on product AND host — no
                  ARM64 disk-latency comparison exists (VM disk never registered load)
    both rounds   gpu: engineCount 16, no fault   |   perProcessorBusyBp: []

**Bias outcome, stated plainly: ARM64 shows NO bias.** x64 read +7.47 pts,
+4.09 pts and 3.47x — three readings, all the same direction, all sizeable.
ARM64 read +0.85 pts then -0.47 pts — two readings, opposite signs, an order
of magnitude smaller. That is the brief's "no bias, x64-specific, surprising"
branch, not the "same bias, shared logic" branch (§43.5). Caveat carried
forward honestly: offline path only, two rounds, no disk-latency point.

**DBT-P42-004 verdict: CLOSED** (§43.7). Build clean, tests identical to x64,
offline measurements agree with the host on real hardware. Two items named
rather than folded in silently: the service path was not re-measured (not
needed to close the verdict — §20.1.9(1)/§42.8 already establish the offline
and service paths share one provider) and disk latency is unmeasured on this
VM rather than confirmed non-biased.

**Recorded rather than worked around:**

| id | what | disposition |
|---|---|---|
| DBT-P43-001 | `p36_relbuild.cmd`'s second step builds a `--example ipc_two_client_probe` that no longer exists in `aethercore-ipc` (no `examples/` dir, no `[[example]]` entry, on either machine) — makes the recipe's own exit code (101) unusable as a pass/fail signal even when the shipping build is clean | open — recipe script is out of this session's scope, §43.2 |
| DBT-P42-011 | byte-rate/latency counters under-report against a 1 s window (x64, still open, unchanged this session) | this session's ARM64 bias evidence points *away* from it being shared-logic, narrowing rather than widening it — not fixed, not re-scoped without more data |
| — | the ARM64 VM's working copy predates P42 by default (fed by copy, not git — no automatic sync exists) | recorded as how it was found, §43.1; resynced this session, not a standing defect to fix |
| — | this VM's `PhysicalDisk(0 c:)` counters never register non-zero `% Disk Time`/write-rate under a 16 MB write+read loop, on host counters as much as the product | recorded as a property of this VM's virtual disk, §43.4 — not chased further |

**Part 4: not taken.** Judged against the brief's own gate — see §43.8 for the
full reasoning. No new snapshot was created; the VM's snapshot list and
install state are exactly as found in §43.0.

# PHASE 44 — CLOSE THE CONTRACT GAP ON macOS/LINUX, DISARM THE BUILD TRAPS (2026-09-03)

Brief: `phase21-workspace/docs/phase41/P44-CONTRACT-AND-TRAPS.md`. This session
runs on the Mac at `/Users/hasanalaaa/dev/aethercore`, which compiles macOS
natively; Linux is reached via `rustup target add x86_64-unknown-linux-gnu` and
`cargo check --target`, not by running Linux binaries.

## 44.0 P44 PROGRESS TABLE (authoritative — resume from here)

| item | what it proves | status | evidence |
|---|---|---|---|
| 1.A | what macOS/Linux providers actually permit, file:line | **DONE** | §44.1 |
| 1.B | failing regression tests, committed failing | **DONE** | §44.2 — macOS: 1 test FAILED for real (2 defects inside it), 2 passed; Linux: lib does not compile on the target at all (pre-existing, DBT-P44-001) |
| 1.C | contract applied to both providers | **DONE** | §44.3 — 10 macOS / 7 Linux rules → 0 survivors; DBT-P44-001 and DBT-P44-002 fixed |
| 1.D | proven on this machine with numbers | **DONE** | §44.4 — cpu exact match saturated, ±2-10pt low bias partial-load; memory exact `hw.memsize` match; storage resolved to an exact container-level match after a real 14x discrepancy was investigated, not assumed; Linux compile-checked only, no VM |
| 2.A | vcomp140.dll sourcing trap disarmed | **DONE** | §44.5 — sourced + hash-verified, not just presence-checked; unexecuted on this host, 6 numbered Windows checks left behind |
| 2.B | ARM64 recipe exit-code decision | **DONE** | §44.6 — decided: bring into repo, not fix in place; migration left as a concrete follow-up (no VM access this session) |
| optional | DBT-P42-006 diagnosis | **MOOT** | §44.3 footnote — no longer reproduces, 18/18 driver-hub tests pass; not chased further, out of scope |

## 44.1 PART 1.A — what macOS and Linux actually permit

Read in full: `crates/performance-telemetry/src/macos_impl.rs` (507 lines),
`src/linux_impl.rs` (519 lines). Neither goes through `Reading<T>` /
`CollectedSubsystems` (`lib.rs:238-341`) — both build `PerfSnapshot` as a
literal struct in `sample()` (`macos_impl.rs:494-506`,
`linux_impl.rs:504-517`), confirming DBT-P42-005 as recorded.

### Q1 — can either return success with an all-zero/empty payload and no fault

**Yes, on both, in several independent shapes — none of them mirror Windows'
shape exactly.**

**macOS:**

| what | file:line | shape |
|---|---|---|
| `cpu.dpcIsrBusyBp` / `cpu.contextSwitchesPerSec` | `macos_impl.rs:167-168` | hardcoded `0`, unconditionally, forever — no line of this file ever writes them, and cpu is otherwise reported fully measured (no `cpu` fault) |
| `storage[].avgTransferLatencyUs` / `readBytesPerSec` / `writeBytesPerSec` | `macos_impl.rs:326-328` | same shape — hardcoded `0` on every real device, no per-field fault |
| `processTop` empty with no fault | `macos_impl.rs:387-391` | a pid whose `proc_pidinfo` call fails is silently skipped ("this is not a provider fault" per the comment); if every pid fails that call, `sample_process_top` returns `[]` with **no fault at all** — only the earlier `proc_listpids <= 0` branch (`:358-365`) is faulted |
| storage per-path silent skip | `macos_impl.rs:309-311` | `if total == 0 { continue; }` — no per-path fault; only the all-empty catch-all at `:331-338` covers total failure, not a partial one |
| `busy_bp_from_ticks` tie | `macos_impl.rs:114-116` | `total == 0 → return 0` with no fault — a real read that ties is legitimate, but indistinguishable on the wire from a genuine idle reading |

**Linux:**

| what | file:line | shape |
|---|---|---|
| `power` — real thermal data collected then discarded | `linux_impl.rs:471-500` | **headline finding.** `scan_thermal_zones()` is called and its result used only to decide whether to push a `thermalPower Degraded` fault (`:473-480`, fires only if `zones.is_empty()`); the millidegree readings are **never written into `PowerSample`** — `power` is built as a disconnected literal (`:494-500`) with `has_temperature: false, temperature_c: 0` always. So on any host where zones ARE found, `power` returns a confident all-zero reading with **no fault at all**, silently discarding real evidence — stronger than "never attempted," this is "measured and thrown away" |
| `processTop` always empty, never faulted | `linux_impl.rs:512-514` | unconditional `Vec::new()`, **zero fault ever pushed** — the exact §20.1.1 site-7 shape ("`let _ = faults; Vec::new()`") that was explicitly deleted from Windows in P42 and replaced with a `NotCollected` reading; Linux still has it verbatim |
| `cpu.dpcIsrBusyBp` / `contextSwitchesPerSec` | `linux_impl.rs:436-443` | same permanent-hardcoded-zero shape as macOS (`..CpuSample::default()`), no fault, ever |
| `storage[].activeTimeBp` / `avgTransferLatencyUs` / `readBytesPerSec` / `writeBytesPerSec` | `linux_impl.rs:339-347` | all four literal `0` for every device, deliberately (comment at `:335-338`), no per-field fault; only `queueDepthX100` and device identity are real |
| `busy_bp_from_proc` tie | `linux_impl.rs:100-102` | same tie-break-returns-0-silently shape as macOS |

### Q2 — Windows-equivalent mechanisms

The clearest match is Linux `processTop` (`linux_impl.rs:512-514`) — the
identical "success, empty, no fault, by design" shape as pre-fix Windows site
7. The Linux thermal-zone-discard (`linux_impl.rs:471-500`) is a **new**
shape not present on Windows: not an unattempted read and not a misread
status code, but a real measurement taken and then never wired into the
payload. Neither platform has a dropped-temporary or counter-read-before-
collection defect — those were PDH-specific (`PdhExpandWildCardPathW`,
DBT-P42-001/003) and neither provider uses PDH.

### Q3 — how many hand-written availability rules, and do any fire by accident

**macOS: 8 conditional decision branches** (cpu first-tick-read fail, cpu
second-tick-read fail, memory `host_statistics64` fail, memory `sysctl
hw.memsize` fail, memory `sysctl` swap fail, storage per-path `statfs` fail,
storage all-empty catch-all, processTop `proc_listpids` fail) **+ 2
unconditional stub degradations** (gpu, power/thermal — always faulted,
every tick, not a branch). **Linux: 6 conditional branches** (cpu
read/parse fail, cpu loadavg fail, memory file-read fail, memory parse fail,
storage file-read fail, storage empty-rows, storage all-virtual-devices) **+
1 unconditional stub** (gpu).

**None fire by accident.** Unlike Windows' gpu (§20.1.6 — fired for the
wrong reason and would have missed a real defect had the counter add
succeeded), every macOS/Linux rule that does fire fires for the reason its
own detail string states. The defect on these two platforms is not
"accidental correctness" — it is coverage gaps: fields the code never
attempts to fault for at all (Q1's table), not existing rules answering
wrong.

### A pre-existing, previously unknown defect found by reaching this host

**`linux_impl.rs` does not compile on the Linux target at all** — three
`E0308` type errors (`linux_impl.rs:446,464,475`): `sample()` declares
`let mut faults: Vec<CollectorFault> = Vec::new();` (`:412`, an owned
`Vec`, not a reference) and then calls the module's `fault()` helper — which
takes `&mut Vec<CollectorFault>` (`:24`) — by value at three call sites.
Confirmed pre-existing and independent of this session's test additions:

    git stash                      # no local changes to stash — the lib itself fails
    cargo check -p aethercore-performance-telemetry --target x86_64-unknown-linux-gnu
    error[E0308]: mismatched types (x3), linux_impl.rs:446, 464, 475
    error: could not compile `aethercore-performance-telemetry` (lib) due to 3 previous errors

This is #[cfg(target_os = "linux")]-gated, so no macOS build has ever
type-checked this file, and (so far as this session can determine) no CI in
this repository builds a Linux target either — this is the same class of
"cannot be compiled or tested on this host" that §42's brief recorded for
Windows, except it means DBT-P42-005 understated the Linux gap: it is not
merely "still on the old contract," the file is currently **uncompilable**.
Recorded as **DBT-P44-001**, fixed in §44.3 as part of the Part 1.C rewrite
(the function this bug lives in is being rewritten to the contract anyway).

## 44.2 PART 1.B — the failing tests, committed failing

New file: `crates/performance-telemetry/tests/dbt_p42_005.rs`,
`#[cfg(target_os = "macos")]` / `#[cfg(target_os = "linux")]` modules, mirroring
`dbt_p41_002.rs`'s structure. Per platform: a lower-bound CPU-under-load test,
a "no collector returns success with a permanent-zero/empty payload and no
fault" test built directly from §44.1's findings, and a capabilities-vs-
collectors reconciliation test. Linux additionally gets a dedicated test for
the thermal-zone-discard defect.

**A test-isolation artifact found and fixed before results could be trusted:**
running the file's tests in parallel (libtest's default) produced a spurious
`totalBusyBp: 0` under load — two concurrent full-core spin harnesses in the
same process starved each other inside the 120 ms in-tick delta window.
Confirmed as harness noise, not a product defect, by re-running with
`--test-threads=1` (passed) and then serializing the file's own `under_load`
calls behind a `static LOAD_LOCK: Mutex<()>` (passed deterministically
thereafter, output below). Recorded because the brief warns against
theorising past an observed difference — this one *was* verified rather than
assumed.

### macOS — 1 test FAILED for real, 2 passed, verbatim

    running 3 tests
    test macos::capabilities_never_claim_native_for_a_subsystem_that_reported_nothing ... ok
    test macos::real_macos_provider_reports_non_zero_cpu_under_load ... ok
    test macos::no_collector_returns_an_empty_payload_without_a_fault ... FAILED

    ---- macos::no_collector_returns_an_empty_payload_without_a_fault stdout ----
    collectors returned success with nothing measured (or a permanent-zero field) and nothing declared:
      cpu: dpcIsrBusyBp=0 contextSwitchesPerSec=0 with cpu reported fully measured and no
           cpu.counters degradation fault; cpu=CpuSample { per_processor_busy_bp: [10000],
           total_busy_bp: 10000, dpc_isr_busy_bp: 0, context_switches_per_sec: 0,
           processor_queue_length_x100: 532 }
      storage: 2 device(s) measured, every rate/latency field 0, no storage.rates degradation fault
    all faults: [CollectorFault { collector: "gpu", kind: "Degraded", .. },
                 CollectorFault { collector: "thermalPower", kind: "Degraded", .. }]

    test result: FAILED. 2 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out

Confirms exactly the two macOS findings from §44.1's table that survive under
real load: `dpcIsrBusyBp`/`contextSwitchesPerSec` and storage's rate/latency
fields are permanent zeros with no accompanying fault, on a machine
`cpu.totalBusyBp` itself proves is at 100%. The other two macOS findings
(processTop empty-with-no-fault; the per-path storage tie) did **not**
surface in this run because they depend on rare failure conditions
(`proc_pidinfo` failing for every pid; a `statfs` tie) that this healthy host
does not hit — they remain real, code-permitted paths (§44.1), just not
reproduced here. Not claimed as fixed; not silently dropped either.

### Linux — the lib does not compile on the target; 3 pre-existing E0308s

Cannot run tests: DBT-P44-001 (§44.1) blocks even `cargo check`. This is
itself the honest Part 1.B result for Linux — a stronger finding than a
runtime test failure would have been, and it is `git stash`-verified
pre-existing, not introduced by this file.

Both results are committed in this state before any fix.

### Addendum — DBT-P44-003: the CPU-under-load test flakes at ~1-in-10, unexplained

**Correcting the record rather than letting a hasty read stand.** An early
re-run of this file after the fix (§44.3) hit `totalBusyBp: 0` twice,
each time right after Part 1.D's manual stress test (§44.4) had just been
torn down, and 10 clean runs followed — first written up here as
"environmental settling." That explanation does not survive a direct test:
a further 10-run series of `cargo test -p aethercore-performance-telemetry
--test dbt_p42_005` (this file alone, no other binary, **no preceding
manual stress test**) still produced **1 failure in 10**, same symptom
(`totalBusyBp: 0` under `under_load`'s guaranteed full-core spin). The
settling story is wrong or incomplete; corrected rather than left standing.

**What this session ruled out, not assumed:** not purely cross-binary
contention (flaked running this one binary alone) and not purely
post-stress-test settling (flaked with no preceding load). **What this
session did not establish:** the actual root cause. `busy_bp_from_ticks`'s
`total == 0` tie (§44.1, `macos_impl.rs:114-116`, described there as a rare
edge case) fires roughly ten times more often under real, guaranteed
full-core load than that framing assumed — either `host_statistics64`'s
tick data coalesces at a coarser interval than this harness's 120ms window
on this Apple Silicon host, or something else; not investigated further
this session, and not worth guessing at in place of measuring it.

**Recorded as DBT-P44-003, open.** Does not reopen Part 1.D's proof: four
manual `aetherctl telemetry-once` invocations under real, sustained load
during §44.4 — run directly, not through this harness's rapid fresh-instance
`under_load` pattern — never returned a zero, and all four agreed with the
host's own counters to within a couple of points. The discrepancy is between
this specific test harness's calling pattern and whatever the real
provider does under it; not between the provider and the host.

## 44.3 PART 1.C — the contract applied to both providers

Both `macos_impl.rs` and `linux_impl.rs` now build every subsystem as a
`Reading<T>` and publish exclusively through `CollectedSubsystems::into_snapshot`
— the same shape §42.2 established for Windows, including the `partial: &mut
Vec<CollectorFault>` split for degraded *parts* of an otherwise-measured
subsystem (dotted names: `cpu.counters`, `storage.rates`, mirroring Windows'
`cpu.counters`/`memory.counters`).

### How many availability rules survived, and why

**macOS: 10 decision sites going in (§44.1's 8 conditional + 2 unconditional
stub), 0 survive as independent hand-written rules.** All 10 are now
`Reading::unavailable` / `Reading::from_evidence` / `Reading::from_collection`
calls whose fault is supplied at the point of collection — same transformation
§42.2 did for Windows, on a crate that had never gone through it. Three
**new** dotted partial faults were added because §44.1 found real coverage
gaps the contract migration alone does not close automatically (a subsystem
can be "measured" by `Reading` and still carry a permanently-unmeasured
sub-field):

- `cpu.counters` — DPC/ISR busy time and context switches/sec were, and
  remain, unmeasured on macOS; now named instead of silently `0`.
- `storage.rates` — transfer latency and read/write bytes/sec were, and
  remain, unmeasured (statfs has no rate data); now named.
- The per-path storage `total == 0` silent skip (§44.1) now pushes a
  `storage` Degraded fault instead of `continue`-ing silently.

processTop's silent-empty gap (§44.1) is closed by `Reading::from_collection`
directly — no partial fault needed, the whole-subsystem `Unavailable` now
fires if every listed pid fails its `proc_pidinfo` query.

**Linux: 7 decision sites going in (§44.1's 6 conditional + 1 unconditional
stub), 0 survive as independent hand-written rules.** Same transformation,
plus:

- `cpu.counters` — same DPC/ISR + context-switches gap as macOS, named.
- `storage.rates` — active time, transfer latency, read/write bytes/sec
  (diskstats single-sample has no rate data), named.
- **`processTop` — DBT-P42-005's Linux instance of the exact §20.1.1 site-7
  shape** (`Vec::new()` unconditionally, zero fault ever) is now
  `Reading::unavailable(NotCollected)`, verbatim-mirroring
  `windows_impl.rs:837-852`'s design and its stated reason (no per-PID /proc
  walk on the hot path — observer effect).
- **DBT-P44-002, FIXED: `sample_power` no longer discards the thermal-zone
  scan.** §44.1's headline finding — `scan_thermal_zones()` was read only to
  decide whether to fault, and its millidegree readings were never written
  into `PowerSample`. The hottest zone is now the evidence:
  `has_temperature: true, temperature_c` from `zones.iter().max()`; absence
  degrades exactly as before (`thermalPower Degraded`). `throttle_active`
  stays `false` — no rated-threshold basis exists to compare against, and
  none is invented, matching the file's own honesty discipline for every
  other field it doesn't measure.

### DBT-P44-001, FIXED — Linux did not compile on its own target

§44.1 found `linux_impl.rs` failed with 3 `E0308`s (faults passed by value
where `sample()`'s helper wanted `&mut Vec<CollectorFault>`) — subsumed by
this rewrite, since the function carrying the bug was rewritten to route
through `Reading<T>` anyway. A **second, independent** instance of the same
root cause ("this file has never been type-checked on a Linux target")
surfaced only once the first was fixed and `cargo check --tests` could get
further: `ProcStatCpu`/`ProcMemInfo`/`DiskStatsRow` were `pub(crate)`
(`linux_impl.rs:51,130,244`), and the crate's `__test` doc-hidden module
re-exports functions returning them as `pub` — a private type in a publicly
reachable signature, which the existing `tests/native_providers.rs`
`linux_parsers` module (pre-existing, unmodified by this session) has always
called, but never through a compiler until this session's cross-target
check. Fixed by widening the three types to `pub`, matching the visibility
their macOS equivalent (`CpuTicks`, `macos_impl.rs`) already had — macOS's
was caught by every macOS build; Linux's was caught by none, ever, until
this session reached the target.

    cargo check -p aethercore-performance-telemetry --target x86_64-unknown-linux-gnu
    cargo check -p aethercore-performance-telemetry --tests --target x86_64-unknown-linux-gnu
    both EXIT 0, warnings only (2 pre-existing unrelated unused-import warnings
    in tests/adversarial.rs and tests/phase27_real_sample.rs, first surfaced
    because this is the first time those files were compiled for this target)

### Windows — untouched, confirmed by diff scope

Neither `windows_impl.rs` nor any file `#[cfg(windows)]`-gated was touched.
The only shared-code file this session's Part 1.C edited outside the two
providers is none — `Reading<T>`/`CollectedSubsystems`/`unreachable_fault`-
style helpers are per-file (macOS and Linux each define their own
`unreachable_fault`, same pattern as Windows', not a shared refactor into
`lib.rs`), so there is no diff for Windows to review. `git diff --stat`:
only `macos_impl.rs` and `linux_impl.rs` changed.

### A deliberate, small wire-visible change on macOS

`gpu.adapter_id`/`adapter_name` previously carried human-readable placeholder
text (`"degraded"` / `"GPU telemetry unavailable on macOS (no IOReport
source)"`) even though the collector had nothing measured. Routing gpu
through `Reading::unavailable` means the wire fallback is now
`GpuSample::default()` — empty strings — with the same information carried
instead in the `CollectorFault.detail` that already existed. This matches
the contract's own principle (evidence fields carry evidence or nothing; the
fault carries the explanation) and is consistent with how Windows' gpu
already behaved pre- and post-P42. Downstream: `offline.rs:241`'s
now-deleted-on-Windows site-8 gpu rule reads `measured_subsystems().gpu`,
not `adapter_id`, so this is not a second contradiction reappearing on macOS.

## 44.4 PART 1.D — proven on this machine, with numbers

    cargo test -p aethercore-performance-telemetry     7/7 tests pass (§44.2's
                                                        dbt_p42_005.rs; the two
                                                        real macOS assertions
                                                        that failed pre-fix now
                                                        pass — see below)
    cargo build --release                              workspace-wide, EXIT 0

    aethercore-performance-telemetry --test dbt_p42_005
    running 3 tests
    test macos::capabilities_never_claim_native_for_a_subsystem_that_reported_nothing ... ok
    test macos::real_macos_provider_reports_non_zero_cpu_under_load ... ok
    test macos::no_collector_returns_an_empty_payload_without_a_fault ... ok
    test result: ok. 3 passed; 0 failed

### Real provider vs the host's own counters, under load

Load: `hw.ncpu` `yes > /dev/null` processes (one per logical CPU) plus a
concurrent `dd` write+read loop against `/tmp`, driven with `aetherctl
telemetry-once` (release build) beside `top -l 1 -n 0`, `vm_stat`, `diskutil
apfs list`, `df -k`, `sysctl hw.memsize` in the same window.

**CPU, saturated (all cores loaded):**

| reading | product | host (`top`) | delta | ratio |
|---|---|---|---|---|
| round 1 | **10 000 bp** (100.00%) | 7.98% user + 92.1% sys = **100.08%** | −0.08 pts | 0.999 |
| round 2 | **10 000 bp** (100.00%) | 6.44% user + 93.55% sys = **99.99%** | +0.01 pts | 1.000 |

Both readings are pinned at the 10 000 bp ceiling on both sides — agreement
is exact but the ceiling makes this pair useless for detecting a §43.5-style
directional bias (a saturated reading cannot show whether the product reads
high or low relative to the host). A second load level was captured
specifically to get a non-saturated comparison:

**CPU, partial load (half the spin processes killed mid-session):**

| reading | product | host (`top`) | delta | ratio |
|---|---|---|---|---|
| round 1 | **6428 bp** (64.28%) | 7.16% + 66.99% = **74.15%** | **−9.87 pts** | 0.867 |
| round 2 | **7081 bp** (70.81%) | 7.24% + 65.72% = **72.96%** | **−2.15 pts** | 0.971 |

**Bias outcome, stated plainly, as a fourth data point after x64 (positive,
consistent), ARM64 (negligible, sign-flipping) and this session's third: both
partial-load readings on macOS read slightly LOW relative to `top`** —
opposite direction from x64's Windows bias (which read consistently HIGH,
+4.1 to +7.5 pts across three readings) and larger in magnitude than ARM64's
near-zero, sign-flipping noise (§43.5). But the two macOS deltas themselves
are not tightly clustered the way x64's three were (−9.87 vs −2.15, a 4.6x
spread) — this reads more like sampling-window misalignment between the
product's 250 ms in-tick delta and `top`'s own ~1 s snapshot cadence than a
systematic platform bias, but it is recorded as observed rather than
explained away: **two readings, both negative, inconsistent magnitude.**
DBT-P42-011 (the open x64/ARM64 disk-latency-and-rate-window question) is
not widened or narrowed by this — it is a distinct question (window width,
not sign) and macOS's rate/latency fields are honestly unmeasured (§44.3),
not comparable at all.

**Context switches / DPC-ISR:** honestly `0` on every reading, and every
reading now carries the `cpu.counters` Degraded fault that says why
(§44.3) — not compared against the host because the field is declared
unmeasured, not a number claiming to be one.

### Memory

| reading | product | host | delta / match |
|---|---|---|---|
| `totalPhysicalBytes` | **38 654 705 664** | `sysctl hw.memsize` → **38654705664** | **exact match** |
| `memoryLoadPercent` | **95%** | `top`: `PhysMem: 34G used (4059M wired, 12G compressor), 1490M unused` → 34G/(34G+1490M) ≈ **95.8%** | agrees within ~1 pt (different accounting: product's `available` = free + min(inactive, purgeable) from `host_statistics64`; `top`'s "unused" is a different macOS-specific bucketing — both honest, not the same formula) |

### Storage — a real discrepancy investigated, not assumed

First comparison attempted, `df -k /`: **5% capacity**, vs product's
`activeTimeBp` **7189** (71.89%) for `deviceId: "/"` — a 14x mismatch that
was **not** waved off. Investigated rather than assumed:

    diskutil apfs list | grep "Capacity In Use By Volumes"
    Capacity In Use By Volumes: 715142668288 B (715.1 GB) (71.9% used)
    df -k /System/Volumes/Data → 72% Capacity

**Resolved: `df -k /` reports the sealed system volume's own small exclusive
usage; `statfs()` on an APFS volume reports the CONTAINER's shared
free/total space (APFS volumes share one free-space pool), which is what
this provider's `total`/`available` blocks actually measure.** The container-
level tools agree with the product almost exactly: `diskutil apfs list`
71.9% used vs product 71.89% (delta 0.01 pts, ratio 0.9999); `df -k
/System/Volumes/Data` 72% vs product 71.89% (delta 0.11 pts). Both mounted
paths (`/` and `/System/Volumes/Data`) report the identical `activeTimeBp`
in the product's output, which is correct given they share one container —
not a duplicate-reporting bug.

`avgTransferLatencyUs`/`readBytesPerSec`/`writeBytesPerSec`: `0` on both
devices, with the `storage.rates` Degraded fault present on every reading
(§44.3) — statfs has no rate data, so these are declared unmeasured rather
than compared.

### Linux — what could and could not be verified on this host

**Cannot be measured: no Linux host or VM is attached to this session.**
Named rather than skipped. What *was* verified, cross-compiled from macOS:

    rustup target add x86_64-unknown-linux-gnu
    cargo check -p aethercore-performance-telemetry --target x86_64-unknown-linux-gnu           EXIT 0
    cargo check -p aethercore-performance-telemetry --tests --target x86_64-unknown-linux-gnu    EXIT 0

Both are clean as of this commit (§44.3 fixed the 6 compile errors — 3
`E0308`, 3 private-type — that blocked this before Part 1.C). `cargo test`
cannot run: `cargo check`/`cargo build` cross-compile without a linker;
running the resulting binary needs an actual Linux kernel for `/proc`. No
readings, no host-counter comparison, no bias data point exist for Linux
from this session — this is the honest limit, not a claim that the Linux
fix "works," only that it now **type-checks**, which it did not before
§44.3.

## 44.5 PART 2.A — DBT-P42-012: vcomp140.dll now sourced and hash-verified

`scripts/build-installer.ps1` previously only checked that a file named
`vcomp140.dll` already existed somewhere in `$PayloadDir` — it never said
where that file should come from, so the trap §42.5 found (the first
plausible match on this machine, the `onecore` variant at 72,712 bytes, is
wrong; the correct file is the desktop x64 redist at 193,152 bytes, sha256
`55aba23c…`) was armed for every future build, caught only because §42
happened to hash against a baseline recorded in an earlier session.

**Changed:** added `Resolve-VcompSource` (prefers the standard MSVC
`$env:VCToolsRedistDir` env var — set by `vcvarsall.bat` / a VS Developer
shell, portable across machines — falling back to this project's pinned
toolchain checkout at the exact path §42.5 measured the correct file at) and
`Assert-VcompHash` (fails loudly, naming the expected vs. actual byte count
and sha256, if the staged file is not byte-for-byte the desktop x64 redist).
If `$PayloadDir` doesn't already carry `vcomp140.dll`, the script now stages
it itself from the resolved source; either way, the hash check runs before
the existing `$required` presence loop. This is not documentation-as-check —
a wrong file now stops the build with the two numbers side by side, rather
than silently packaging and passing every downstream gate the way the
`onecore` variant did until §42 happened to hash it.

**This machine cannot run this script** (`if ($env:OS -ne 'Windows_NT')
{ throw ... }` at line 20 — this is macOS). No `pwsh` is installed here
either, so the edit was reviewed by hand, not executed. The next Windows
session must verify it. Numbered checks, each with an EXPECTED value:

1. **Syntax parses.** `powershell -NoProfile -Command "$null = Get-Command
   -Syntax (Join-Path (Get-Location) 'scripts\build-installer.ps1')"` or
   simply invoke the script normally — a parse error would surface
   immediately, before `$ErrorActionPreference` even matters.
   **EXPECTED:** no `ParserError`; the script proceeds to its existing
   version-check logic.
2. **`Resolve-VcompSource` finds the file via `$env:VCToolsRedistDir` when
   run from a VS Developer shell.** **EXPECTED:** `vcomp140.dll staged from
   ...` is printed (only if `$PayloadDir` didn't already have the file) or
   nothing is printed and the hash check passes silently if it did.
3. **The hash check passes against the known-good file.** Run the build
   once with the payload's existing (already-correct, per §42.5) staged
   `vcomp140.dll` in place. **EXPECTED:** the script proceeds past
   `Assert-VcompHash` with no output from it (success is silent by design,
   matching the rest of the script's checks) and the build continues to the
   WiX invocation.
4. **The hash check fails loudly against the WRONG file.** Manually copy
   the `onecore` variant (`VC\Redist\MSVC\14.44.35112\onecore\x64\
   Microsoft.VC143.OpenMP\vcomp140.dll`, 72,712 bytes) into `$PayloadDir` as
   `vcomp140.dll` before running the script. **EXPECTED:** the script throws
   before reaching the WiX build, with a message containing both `72712` (or
   the actual measured byte count) and the expected `193152` / `55aba23c…`
   — i.e. the exact trap §42.5 hit by hand is now caught by the script
   itself, in one run, before any WiX step.
5. **The fallback path resolves when `$env:VCToolsRedistDir` is unset.**
   Run from a plain shell (not a VS Developer prompt) with the pinned
   toolchain checkout present at `C:\AetherCore-P36\toolchain\vs2022\...`.
   **EXPECTED:** the script still finds and stages the correct file via the
   pinned fallback path, hash check passes.
6. **A full build still succeeds end-to-end** with this change in place —
   re-run §42's Gate 2.A sequence (`build-installer.ps1` → `wix msi
   validate` → `check-msi-payload.ps1`). **EXPECTED:** identical result to
   §42.5 — exit 0, validate output EMPTY, 0 ICE, 16 payload rows, and the
   `vcomp140.dll` row's sha256 still `55aba23c…` (this change does not
   change which bytes ship, only how confidently the script can say so).

## 44.6 PART 2.B — DBT-P43-001: decision recorded, bring it into the repo

**Decision: bring `p36_relbuild.cmd` into the repo under version control.
Do not fix it in place on the VM.**

This session cannot execute the migration — it runs on the Mac with no VM
access (unlike §43, which drove the VM through `prlctl`; this brief does not
mention resuming it), so the file's current content cannot be read or
written from here. The decision and the reasoning are recorded now so a
Windows/VM session can carry it out without re-litigating the choice.

**Argument for bringing it in, not fixing in place:**

1. **This is the same failure mode DBT-P42-012 just was, in this same
   session (§44.5).** An artifact that exists only on a machine, outside
   git, with no way for anyone off that machine to see its current content,
   diff a change, or know what changed it last. Fixing `p36_relbuild.cmd`
   in place would resolve DBT-P43-001's specific symptom (the phantom
   `--example ipc_two_client_probe` step) while leaving the exact class of
   defect that produced it fully armed for the next thing that goes wrong
   in that file.
2. **The rest of the release pipeline is already in the repo.**
   `scripts/build-installer.ps1`, `scripts/check-msi-payload.ps1`,
   `scripts/Get-ProductVersion.ps1` are all version-controlled, reviewable,
   and diffable. `p36_relbuild.cmd` performing the ARM64 release build and
   validation is the one piece of this pipeline that is not — an
   inconsistency, not a deliberate boundary.
3. **§44's own brief states the diagnosis this argues from:** "A build
   script whose exit code cannot be trusted is a measuring instrument that
   lies — the recurring pattern in this project." An unversioned script is
   *why* nobody could see the phantom example target before it started
   returning exit 101 on a clean build — there is no history to `git blame`,
   no diff to review, no CI to catch drift. Version control does not fix
   DBT-P43-001 by itself, but it makes the fix reviewable, and makes the
   next regression like it visible in a diff instead of discovered by a
   session that has to read the file cold off a VM.
4. **Against "fix in place":** it is faster this once, but it repeats
   exactly the mistake this session spent its first half fixing on the
   Windows side (§44.5) — an unaudited, unversioned artifact that
   downstream sessions have to re-discover and re-verify by hand instead of
   reading a diff.

**Concrete follow-up for the next session with VM access** (not performed
here — no VM access this session):

1. Copy `C:\AetherCore-P36\logs\p36_relbuild.cmd`'s current content into the
   repo, e.g. `phase21-workspace\scripts\release\p36_relbuild.cmd` (or
   wherever the existing `scripts\` convention best fits — the file is a
   *build recipe*, same category as `build-installer.ps1`).
2. Fix DBT-P43-001 in that copy while it's already being touched: either
   remove the second build step (`--example ipc_two_client_probe`, which
   `git grep` confirms has no `examples\` directory or `[[example]]` entry
   in `aethercore-ipc` on either machine — §43.9), or gate it behind an
   existence check that skips cleanly instead of exiting 101 on a target
   that was never real.
3. Commit the copy, then point whatever invokes the recipe (a scheduled
   task, a session's manual run, documentation) at the repo path instead of
   `C:\AetherCore-P36\logs\...`.
4. Only after the repo copy is proven to reproduce §43.2's clean ARM64
   build (exit 0, `Finished` in the log) should the VM-only copy be
   retired — do not delete it first per this project's standing rule about
   not discarding a fallback before its replacement is proven.

Not performed as part of this session: no VM access, and the brief scoped
this part to a decision, not an execution.

## 44.7 P44 FINAL REPORT

**What the macOS and Linux providers actually permitted, with file:line** —
full detail in §44.1. Headlines: macOS's `cpu.dpcIsrBusyBp`/
`contextSwitchesPerSec` (`macos_impl.rs:167-168`, old line numbers) and
storage rate fields (`:326-328`) were permanent zeros with no fault, ever;
processTop (`:387-391`) could return `[]` with no fault if every pid's
`proc_pidinfo` failed. Linux's `processTop` (`:512-514`) returned `[]`
unconditionally with **zero fault ever** — the exact §20.1.1 site-7 shape P42
deleted from Windows. Linux's `sample_power` (`:471-500`) scanned real
thermal-zone data and **discarded it** rather than never attempting it —
the sharpest single finding of §44.1.

**How many availability rules survived the contract, and why** — §44.3. 10
macOS / 7 Linux hand-written decision sites → **0 survivors on either
platform**, same transformation §42.2 proved for Windows. None fired "by
accident" the way Windows' gpu rule did (§20.1.6) — every rule that fired,
fired for the reason its detail string gave; the defect here was coverage
gaps (fields nothing ever faulted for), not wrong rules answering right by
luck.

**Every macOS reading beside the host's, delta in points and ratio, x64 bias
check** — §44.4. CPU saturated: exact agreement both rounds (<0.1pt).
Memory: `totalPhysicalBytes` exact match against `sysctl hw.memsize`;
`memoryLoadPercent` within ~1pt of `top`. Storage: an investigated,
resolved 14x discrepancy (`df -k /`'s 5% vs. the product's 71.89%) — turned
out `df -k /` is the wrong host counterpart (sealed system volume's own
tiny usage on APFS); `diskutil apfs list`'s container-level 71.9% matches
the product almost exactly. **The x64 bias does not appear here** — at
saturation both sides read 100% (uninformative); at partial load macOS reads
2-10 points **low**, opposite sign from x64's consistent high bias and not
as tightly clustered, read as timing-window noise rather than a confirmed
platform bias — a genuine third-then-fourth data point after x64 (positive,
tight) and ARM64 (near-zero, sign-flipping) on DBT-P42-011's open question,
not a resolution of it.

**Exactly what could not be verified on this host, named:**

- Linux runtime behavior — no readings, no host-counter comparison, no bias
  data point. Only `cargo check`/`cargo check --tests --target
  x86_64-unknown-linux-gnu` (both EXIT 0) were possible from this Mac.
- The Windows `build-installer.ps1` edit (§44.5) — this machine is not
  Windows (`if ($env:OS -ne 'Windows_NT') { throw }`), has no `pwsh`, and
  the script was reviewed by hand rather than executed. Six numbered checks
  with EXPECTED values are left for the next Windows session.
- The `p36_relbuild.cmd` migration (§44.6) — decided, not executed; no VM
  access this session.
- ARM64 macOS/Windows readings — this session's macOS numbers are Apple
  Silicon (this Mac); no Windows ARM64 comparison was attempted or claimed
  (out of scope — Part 1 of this brief is macOS/Linux, and §43 already
  closed DBT-P42-004 for Windows ARM64 separately).

**The Windows verification checks Part 2.A leaves behind** — the six
numbered checks with EXPECTED values in §44.5, covering script syntax, both
the env-var and pinned-path sourcing branches, a positive hash-match run, a
deliberate negative run against the known-wrong `onecore` file, and a full
Gate 2.A re-proof.

**Recorded rather than worked around, every debt ID this session touched:**

| id | what | disposition |
|---|---|---|
| **DBT-P42-005** | macOS/Linux built `PerfSnapshot` literally, not through `CollectedSubsystems` | **CLOSED.** §44.3 — both platforms now route exclusively through `Reading<T>`/`CollectedSubsystems`, same shape as Windows' P42 fix |
| **DBT-P44-001** | Linux does not compile on its own target — 3x `E0308` (faults passed by value) + 3x private-type-in-public-interface, never caught because no session had reached a Linux target before this one | **FIXED**, §44.3. `cargo check --tests --target x86_64-unknown-linux-gnu` now EXIT 0 |
| **DBT-P44-002** | Linux `sample_power` scanned real `/sys/class/thermal_zone*/temp` data and discarded it, reporting a confident empty `PowerSample` with no fault when zones existed | **FIXED**, §44.3 — hottest zone now wired into `has_temperature`/`temperature_c` |
| **DBT-P44-003** | `macos::real_macos_provider_reports_non_zero_cpu_under_load` (§44.2) flakes ~1-in-10 with `totalBusyBp: 0` under guaranteed full-core load — `busy_bp_from_ticks`'s `total == 0` tie fires far more often on this real Apple Silicon host than its "rare edge case" framing (§44.1) assumed | **open** — root cause not established; ruled out cross-binary contention and post-stress settling (§44.3 addendum), does not reopen Part 1.D's real-provider proof (4/4 manual `telemetry-once` readings under load never returned zero) |
| **DBT-P42-012** | `build-installer.ps1` required `vcomp140.dll` without sourcing it; first plausible match on the machine is wrong | **FIXED** (unexecuted on this host), §44.5 — explicit source resolution + hash verification, 6 Windows checks left behind |
| **DBT-P43-001** | ARM64 recipe's own exit code (101) is unusable even on a clean build, because it also tries a nonexistent `--example` target | **decision recorded**, §44.6 — bring the recipe into the repo; migration is a follow-up, not performed (no VM access) |
| **DBT-P42-006** | `aethercore-driver-hub --lib`, 6 pre-existing failing tests | **no longer reproduces** — 18/18 pass this session (§44.3 footnote); not the code (no change to `driver-hub` since Phase 31 per `git log`), not chased further; recorded so a future session does not read this as newly fixed by P44 |
| **DBT-P42-009** | `perProcessorBusyBp` still `[]` on Windows | unchanged, out of scope for this session (Windows telemetry, not macOS/Linux/build traps) |
| **DBT-P42-010, -011** | Windows gpu adapter identity/VRAM empty; byte-rate/latency counters under-report a short window | unchanged, out of scope |

**Security posture:** not re-measured this session — no destructive action,
no install/uninstall cycle, no Defender/UAC/Firewall-adjacent change on any
machine. Nothing in this session's diff touches privilege, IPC surface, or
anything security-relevant; every change is either a telemetry-collection
contract (data honesty, not access) or a build-script sourcing check.

**Every numbered item committed and pushed individually**, per the brief's
own instruction (no single end-of-session commit): §44.1/44.2 in one commit
(tests), §44.3 in one commit (the fix), §44.4 in one commit (measurement),
§44.5 in one commit (vcomp140.dll), §44.6 in one commit (the recipe
decision), this report in the commit that follows.
