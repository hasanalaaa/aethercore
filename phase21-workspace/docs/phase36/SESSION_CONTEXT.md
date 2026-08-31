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
| 3 | Version single source of truth | **CODE DONE — MSI proof pending** | `66a0f5b`; `[workspace.package].version = 0.1.7` |

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
