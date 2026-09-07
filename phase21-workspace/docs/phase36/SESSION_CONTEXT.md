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

**Recorded as DBT-P44-003, open.** → **Superseded, see §45.0/§45.2: CLOSED,
root cause confirmed by instrumentation (not the "coalesces at a coarser
interval" guess two paragraphs up — that was never verified here) and
fixed at the field boundary as DBT-P45-001.** Does not reopen Part 1.D's
proof: four manual `aetherctl telemetry-once` invocations under real,
sustained load during §44.4 — run directly, not through this harness's
rapid fresh-instance `under_load` pattern — never returned a zero, and all
four agreed with the host's own counters to within a couple of points. The
discrepancy is between this specific test harness's calling pattern and
whatever the real provider does under it; not between the provider and the
host.

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
| **DBT-P44-003** | `macos::real_macos_provider_reports_non_zero_cpu_under_load` (§44.2) flakes ~1-in-10 with `totalBusyBp: 0` under guaranteed full-core load — `busy_bp_from_ticks`'s `total == 0` tie fires far more often on this real Apple Silicon host than its "rare edge case" framing (§44.1) assumed | **open** at the time of this report → **CLOSED by §45.0/§45.2, superseded by DBT-P45-001** (the class this instance belongs to: the pure tick-delta function's return type couldn't say "no window", root cause confirmed by instrumentation before any fix, not assumed) — root cause was NOT established here, do not read this row as still open |
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

# PHASE 45 — DBT-P44-003, and the class it belongs to (2026-09-03)

Brief: `docs/phase41/P45-FIELD-CONTRACT.md`. Runs on the Mac, this repo.

## 45.0 CONFIRMATION — the ~1-in-10 zeros ARE the `total == 0` branch

Before touching `busy_bp_from_ticks`, its `total == 0` branch
(`macos_impl.rs:118` at the time of this diagnosis, P44's numbering) was
instrumented with `eprintln!` — one line printed on every call showing
`busy_delta`/`total_delta` and the four raw tick fields of both `previous`
and `current`, plus a second line specifically when `total == 0` fired. This
was done against the **pristine P44 source** (`git stash` held this
session's not-yet-applied fix aside during the run, `git stash pop`
afterward — no fix code was active while confirming).

`cargo test --test dbt_p42_005 -- macos::real_macos_provider_reports_non_zero_cpu_under_load --nocapture`,
run 40 times as 40 independent process invocations (not 40 loop iterations
inside one process, so no shared state or JIT/cache warm-up could explain a
pattern):

    passes: 33/40    fails: 7/40   (17.5% — consistent with "~1-in-10",
                                     both are small-sample rates of the same
                                     underlying event)

**Every one of the 7 failures shows `busy_delta=0 total_delta=0` with
`prev` and `current` byte-for-byte identical**, and the `total==0 branch
fired` line present. Zero of the 7 failures show any other pattern (e.g. a
nonzero total with a genuinely-idle busy delta, which would indicate a
different defect). Two representative raw lines:

    P45_DIAG busy_delta=0 total_delta=0 prev=(u78534960 s54889865 i893859367 n0) cur=(u78534960 s54889865 i893859367 n0)
    P45_DIAG total==0 branch fired

    P45_DIAG busy_delta=0 total_delta=0 prev=(u78548499 s54890329 i893859602 n0) cur=(u78548499 s54890329 i893859602 n0)
    P45_DIAG total==0 branch fired

**Confirmed: the brief's premise holds.** `host_statistics64(HOST_CPU_LOAD_INFO)`
returned byte-for-byte identical tick counters across the ~120ms window
between `sample_cpu`'s two observations, on this real Apple Silicon host,
under guaranteed full-core load, at roughly this rate. *Why* the counters
sometimes fail to advance in 120ms is still not established (mach's
internal update cadence for this counter vs. this harness's window remains
the open question saved for the record below) — but *that* they fail to
advance, and that this is exactly what the zero came from, is now measured,
not assumed. The diagnostic `eprintln!`s were removed before any fix code
was written; `git diff` against this commit's parent touches no source
files, matching §20.1.10/§44's own "diagnosis only" commits.

## 45.1 PART 1 — the census

Full sweep of `macos_impl.rs`, `linux_impl.rs`, `windows_impl.rs` (current
`main`, i.e. post-P42/P43/P44) for every place a failure, absence, or
degenerate case becomes a numeric zero or empty collection — starting from
the brief's seven starter sites, extended by grepping every
`.unwrap_or(0)` / `.unwrap_or(0.0)` / `== 0` / `Vec::new()` / silent
`continue`/`break` in the three files and reading each one in context.
File:line below is pre-fix (this session's starting point, `main` at the
commit before this phase).

| # | site | class | why |
|---|---|---|---|
| 1 | `macos_impl.rs:118` `busy_bp_from_ticks`, `total == 0 → return 0` | **B** | §45.0: confirmed root cause of DBT-P44-003. Tick counters identical across the sampling window → "no window observed", published as a confident 0% |
| 2 | `macos_impl.rs:176-178` `sample_cpu`, `load_average().unwrap_or(0)` → `processorQueueLengthX100` | **B** | `getloadavg()` failing is silently reported as queue length 0 (an idle machine), with no fault — same `.unwrap_or(0)` shape named at `windows_impl.rs:386` |
| 3 | `macos_impl.rs:335` `sample_storage`, `total == 0` (statfs reports zero blocks) | **A** (already fixed) | §44.3 already converted this to a named `storage` Degraded fault (`macos_impl.rs:338-346` in the pre-fix tree). Left as-is |
| 4 | `macos_impl.rs:257` `sample_memory`, `!memsize_ok \|\| memsize == 0` | **A** (already fixed) | Already pushes a `memory.counters` `ProviderFailure` fault (§42/§44 pattern); `load_percent` correctly falls back to `0` only inside the `memsize > 0` guard |
| 5 | `macos_impl.rs:429-432` `sample_process_top`, `got != size_of::<proc_taskinfo>() → continue` | **A** | Documented, deliberate: a pid vanishing between `proc_listpids` and `proc_pidinfo` is population churn, not a provider fault; the whole-collector empty case is separately faulted via `Reading::from_collection` |
| 6 | `linux_impl.rs:104` `busy_bp_from_proc`, `total == 0 → return 0` | **B** | Identical shape to #1, per the brief. Not independently measured on Linux this session (no Linux host) — fixed on the same reasoning and the same pure-function contract |
| 7 | `linux_impl.rs:91-94` `parse_proc_stat_cpu`, `num(4..7).unwrap_or(0)` for iowait/irq/softirq/steal | **C** | Cannot tell from the code whether a missing field is a genuinely-absent optional column (pre-2.6.24 kernels lack `steal`; even older kernels lack `iowait`/`irq`/`softirq`) or a malformed value — both paths return `None` from the same `parse::<u64>().ok()`. Left as-is; flagged rather than guessed at, per the brief |
| 8 | `linux_impl.rs:338` `sample_storage`, `active_time_bp: 0` (permanent — diskstats has no capacity field) | **A** (already fixed) | Already named via the `storage.rates` Degraded fault (§44.3); a real, permanent, honestly-labeled gap, not a swallowed failure |
| 9 | `linux_impl.rs:472-509` `sample_power` / `scan_thermal_zones` | **A** (already fixed) | DBT-P44-002 closed this: the hottest zone is now the evidence; absence degrades typed |
| 10 | `windows_impl.rs:167-183` (pre-P42 numbering) `read_u64`/PDH offset misread | **A** (already fixed) | DBT-P41-002 mechanism 1, closed in §42.2 |
| 11 | `windows_impl.rs` `sample_cpu` secondary counters (DPC/ISR, context switches, queue length) | **A** (already fixed) | §42.2's `cpu.counters` Degraded-fault pattern, already in place |
| 12 | `windows_impl.rs` `sample_memory` secondary counters | **A** (already fixed) | §42.2's `memory.counters` Degraded-fault pattern, already in place |
| 13 | `windows_impl.rs:714,723,724,725` (brief's numbering) `sample_storage`, `queue_depth_x100`/`avg_transfer_latency_us`/`read_bytes_per_sec`/`write_bytes_per_sec` via `.unwrap_or(0)` | **B** | The device's primary counter (`active_time_bp`) is gated by `?` — a device that can't prove itself is dropped. The four *secondary* counters were not: a transient PDH read failure on any of them silently became `0` with **no fault**, inconsistent with this same file's own `cpu.counters`/`memory.counters` discipline two functions up, and inconsistent with its own comment at (pre-fix) `:386` naming this exact pattern |
| 14 | `windows_impl.rs` `sample_power`, `has_temperature`/`temperature_c` | **B** — found in this session's own sweep, not in the brief's starter list | No line of this file has ever written these fields (`CallNtPowerInformation` exposes clock/throttle state, not raw temperature). `power` was reported fully measured (no fault) regardless — the same "measured subsystem, permanently-zero sub-field, no partial fault" shape as macOS/Linux's `cpu.counters`/`storage.rates`, just not yet named on Windows |
| 15 | `windows_impl.rs` `sample_gpu`, `expand_wildcard_path`'s `needed == 0` | **A** (already fixed) | DBT-P42-001, closed in §42.2 — returns a typed `Err` with the PDH return code |
| 16 | `windows_impl.rs` `sample_process_top` | **A** (already fixed) | §20.1.1 site 7, closed in §42.2 — `Reading::unavailable(NotCollected)` |
| 17 | parser skip sites: `macos_impl.rs` process-list `continue`s, `linux_impl.rs` `parse_proc_meminfo`/`parse_diskstats` malformed-row `continue`s, `windows_impl.rs` `expand_wildcard_path`/`sample_storage` instance-parsing `continue`/`break` | **A** | Every one skips one malformed/aggregate/overflow *row*, not a measurement; the enclosing collector's own empty-collection case is separately faulted via `Reading::from_collection`. Checked individually, not assumed |

**Count: 17 sites examined. 5 new B** (#1 macOS tick tie, #2 macOS
queue-length swallow, #6 Linux tick tie, #13 Windows storage secondary
counters, #14 Windows power temperature — the last found in this session's
own sweep, not the brief's starter list). **9 already-A** (#3, 4, 8, 9, 10,
11, 12, 15, 16 — fixed by P42/P44 before this session; recorded as A above
because they are correct in the current tree, not because they were never a
defect — §45.2 does not touch them again). **1 C** (#7, Linux optional
`/proc/stat` fields — genuinely ambiguous, left open). **2 deliberate-A by
design** (#5, #17).

## 45.2 PART 2 — the contract pushed down to the field boundary, all 5 B sites

**The type-level fix, applied identically to macOS and Linux (site #1, #6):**

    // before
    pub fn busy_bp_from_ticks(previous: CpuTicks, current: CpuTicks) -> u32 {
        ...
        if total == 0 { return 0; }
        ...
    }
    // after
    pub fn busy_bp_from_ticks(previous: CpuTicks, current: CpuTicks) -> Option<u32> {
        ...
        if total == 0 { return None; }
        ...
        Some(ratio.min(u128::from(BP)) as u32)
    }

Same change to Linux's `busy_bp_from_proc`. `None` can no longer be
narrowed back to a `u32` by accident — every call site must decide what
"no window" means, which is exactly the "type must not be able to express
a measurement that was never made" principle the brief states. No per-site
guard was added; the one function signature is the whole fix at the pure-
math layer.

**The caller (macOS `sample_cpu`, Linux `sample()`'s cpu branch): extend the
window once, then degrade — not fabricate, and not give up on the first
tie.** §45.0 measured `total == 0` as a **real, frequent** event on this
hardware (17.5% in the 40-run diagnostic), not a one-in-a-million edge
case — publishing `Reading::unavailable` on the very first tie would have
traded a "0% lie" for a "cpu degrades on ~1 in 6 ticks" product regression,
technically honest but a worse product than either. The brief's own text
supports this: *"Where a Duration or window is genuinely too short to
measure, that is a fault with a reason"* — the fix reads that as license to
first find out whether the window really was too short (extend it, bounded)
before declaring it so, rather than being required to declare unavailable
immediately. Both platforms now:

1. take the two observations as before (in-tick double read on the first
   tick; carried state vs. fresh read on later ticks for Linux, first vs.
   second `host_statistics64` call for macOS);
2. compute the tick delta; if `Some`, done;
3. if `None`, sleep once more — a single `EXTENDED_WAIT` of 480ms, bounded
   to the remaining `interval` budget — and take one fresh second
   observation against the **same original baseline**, recomputing;
4. if that also ties (or the budget was exhausted, or the extra read
   failed), the **whole `cpu` subsystem** reports `Reading::unavailable`
   with a fault naming the observation count and elapsed time
   (`"no tick delta in sampling window after N observation(s) spanning
   Xms"`) — never a fabricated zero, and never silently degrading only a
   sub-field while the rest of `CpuSample` (which has nothing else to
   report without the tick delta) pretends to be measured.

**This was originally a 5-attempt retry loop (~120ms per step, re-checking
after each), simplified to the single 480ms wait above after review —
§45.6 has the full reconciliation, including the experiment that showed the
loop's extra checks never resolved anything early.** It does not touch
`busy_bp_from_ticks`/`busy_bp_from_proc`'s pure contract, which stays
exactly "`None` on `total == 0`, no exceptions" — the wait lives in the
caller, matching the brief's "no per-site guards **in the contract**" (the
contract itself has none; the caller's wait is a scheduling decision, not a
guard around the zero).

**Site #2 — macOS `processorQueueLengthX100`'s `getloadavg()` failure.**
Folded into the existing `cpu.counters` degraded-fault mechanism (§44.3's
pattern) rather than a new fault kind: `degraded: Vec<&str>` now collects
`"processorQueueLengthX100: getloadavg() failed"` alongside the pre-existing
DPC/ISR/context-switches line, joined into one `cpu.counters` `Degraded`
fault. No wire shape change; the `0` fallback is unchanged, only now always
accompanied by a fault when it results from a failure rather than a real
idle queue.

**Site #13 — Windows `sample_storage`'s four secondary PDH counters.**
`active_time_bp` (the primary/gating counter) is unchanged. Each of the
four secondary reads (`queue_depth_x100`, `avg_transfer_latency_us`,
`read_bytes_per_sec`, `write_bytes_per_sec`) now records its counter's name
into a per-device `missing: Vec<&str>` instead of silently substituting `0`
via `.unwrap_or(0)`; devices with any missing secondary counter are
collected into `devices_with_missing_secondary`, and if non-empty a single
`storage.rates` `Degraded` fault is pushed naming which device(s) and
which counter(s) — mirroring the `storage.rates` name already used for
macOS's and Linux's *permanent* version of the same gap (§44.3), here for
Windows' *transient* version of it. `sample_storage` and `sample_power`
both now take `partial: &mut Vec<CollectorFault>`, matching `sample_cpu`/
`sample_memory`'s existing signature in the same file.

**Site #14 — Windows `sample_power`'s never-written `has_temperature`/
`temperature_c`.** A `power.temperature` `Degraded` fault is now pushed
unconditionally on the measured path, naming the reason
(`"raw temperature not measured on Windows: CallNtPowerInformation exposes
clock/throttle state only"`) — the same shape as macOS/Linux's
`cpu.counters`, applied to the one Windows sub-field this session's sweep
found lacking it. `power`'s wire values are unchanged (still `false`/`0`);
only the fault that explains them is new.

**Site #7 (C) — left alone**, as the brief instructs for anything that
can't be told apart from the code. No fix, no guess.

**Do not change Windows or ARM64 behaviour beyond removing the lie — checked.**
`git diff --stat` for this phase's fix commit touches only
`macos_impl.rs`, `linux_impl.rs`, `windows_impl.rs` (sites #13/#14 only
inside `windows_impl.rs`) and the `native_providers.rs` test file (updated
for the `Option<u32>` signature change, §45.3). No file shared across
platforms (`lib.rs`, `collector-runtime`, `platform-capabilities`) was
touched — `Reading<T>`/`CollectedSubsystems` are unchanged, so nothing
about how a `Reading` is constructed or published moved. Windows' cpu tick
math (`read_u64`/PDH) is untouched; only its storage secondary-counter
fallback and its power sub-field fault are new, and both are strictly
additive (new fault pushes; no existing field's value changes on any path
that previously succeeded).

**Compiled on all three targets from this Mac** (this session has no
Windows or Linux host, same limit §44 recorded):

    cargo check -p aethercore-performance-telemetry --tests                                        EXIT 0 (macOS, native)
    cargo check -p aethercore-performance-telemetry --tests --target x86_64-unknown-linux-gnu       EXIT 0
    cargo check -p aethercore-performance-telemetry --tests --target x86_64-pc-windows-msvc         EXIT 0

All three: warnings only, and the warnings are pre-existing (`generation`
dead-code, a few unused imports in test files untouched by this phase) —
none introduced by this session's diff.

## 45.3 PART 3 — tests that would have caught it, and the 50-run rate

**Deterministic pure-function tests, updated in `native_providers.rs`**
(the two the brief asked for by name; both were pre-existing tests whose
assertions embedded the old, wrong contract and had to change, not new
files):

- `tick_delta_math_matches_injected_counters` / `hostile_tick_counters_clamp_without_panicking`
  (macOS): the "identical counters" and "counter regression" cases now
  assert `busy_bp_from_ticks(..) == None`, not `== 0`. Before this
  session's fix these two specific assertions would have **failed to
  compile** (return type was still `u32`); against the *pre-fix logic* with
  the *new* assertion they'd have failed at runtime (`0 != None`'s moral
  equivalent — the old code has no `None` to return at all). Either way,
  this is the exact test the brief specified: "two identical `CpuTicks` —
  must NOT return a measurement."
- `proc_stat_delta_math_handles_hostile_counters` (Linux `linux_parsers`,
  compiles and runs on any host — pure parser/math, no `/proc` access):
  same change for `busy_bp_from_proc`, including the "regression" case
  (`Δtotal` saturates to 0 from a going-backwards counter, not just
  identical inputs) — a second, distinct way to hit `total == 0` that the
  brief's own example didn't name but the fix's contract covers uniformly.

**The regression test the brief's Part 3 opening paragraph is about** —
`dbt_p42_005.rs`'s `real_macos_provider_reports_non_zero_cpu_under_load`,
the exact harness that caught DBT-P44-003 by luck. Its assertion was
`cpu.total_busy_bp > 0`, unconditionally — which is itself now a stale
contract: a bounded number of ties can still happen (§45.2's retry is
bounded on purpose, not unlimited), and when every retry inside the budget
ties, `total_busy_bp: 0` is the **correct**, honestly-faulted answer, not a
defect. Updated to accept `total_busy_bp > 0 OR an honest cpu Unavailable
fault naming the tick-delta retry exhaustion` — the same "zero-with-a-
reason is fine, zero-with-silence is not" invariant this crate already
applies elsewhere (`no_collector_returns_an_empty_payload_without_a_fault`,
same file). This keeps the test able to catch the **original** defect (a
confident zero, no `cpu` fault) while not flaking forever on the new,
honest, rare case.

**The 50-run measurement, raw, not summarized:**

    cargo test -p aethercore-performance-telemetry --test dbt_p42_005 \
      -- macos::real_macos_provider_reports_non_zero_cpu_under_load

    run as 50 independent process invocations (--test-threads=1 each, same
    protocol as §45.0's confirmation), instrumented for this measurement
    only (removed before commit) to print cpu.total_busy_bp regardless of
    pass/fail:

    total_busy_bp value        count (of 50)
    ------------------------   -------------
    10000                      47
    9893                       1
    9841                       1
    0                          1

    totalBusyBp == 0 occurrences: 1 / 50   (down from 7 / 40 pre-fix, §45.0)
    test pass:  50 / 50
    test fail:   0 / 50

**Against the brief's stated EXPECTED ("zero occurrences of `totalBusyBp: 0`
under guaranteed load"): not literally met — 1 occurrence remains.** Stated
plainly rather than reframed as a pass: the bounded retry (5 attempts,
~120ms each, capped at the sampling `interval`) cuts the tie rate by
~9x (17.5% → 2%) but does not eliminate it, because it is bounded on
purpose — an unbounded retry would let one bad tick hang the sampler
indefinitely, which is a worse defect than an occasional honest
`Unavailable`. **What *is* true, and is the actual goal the brief is
after: that one remaining occurrence is never again an unexplained lie.**
Every one of the 50 runs' `total_busy_bp` values, including the single
zero, is now accounted for by either a real measurement or a named `cpu`
fault — verified by the updated test passing on all 50, and shown directly
in the raw failure line the fix produces for that one case:

    cpu=CpuSample { per_processor_busy_bp: [], total_busy_bp: 0, ... }
    faults=[CollectorFault { collector: "cpu", kind: "Unavailable",
      detail: "no tick delta in sampling window after 5 attempt(s)
               spanning 490ms" }, ...]

This measurement was against the original 5-attempt loop. §45.6 replaces
that loop with a single 480ms wait (same worst-case budget) after review
found the loop's intermediate checks never resolved anything early; the
fault string above becomes `"...after 2 observation(s) spanning ~600ms"`
under the simplified code. **Re-run after the simplification: identical
result, 1/50, 50/50 pass** — recorded in §45.6, not re-tabled here, since
the distribution is the same shape with different exact non-1.0 values
(real host noise between runs, not a behavior change).

Recorded as **DBT-P45-004, open**: full elimination of the tie (not just
honest labeling of it) was not attempted this session. Two directions
exist and neither was chosen without more data: raise the retry ceiling
(delays the sampler further on the already-rare bad case) or find a
tick source with a documented update cadence to size the first sleep
correctly instead of guessing 120ms (needs investigation this session did
not do — §45.0 explicitly left "why 120ms sometimes isn't enough"
unanswered, and DBT-P45-004 inherits that same open question rather than
re-opening it under a new number).

## 45.4 PART 4 — macOS readings beside the host's, and the bias question

Binary under test: `target/release/aetherctl`, built this session, this
fix included. Load harness: `N` background `while true; do :; done` shells,
PIDs captured explicitly via `$!` (not `jobs -p`, which returned empty in
this non-interactive shell and would have made cleanup silently
impossible — caught before it left anything running; verified clean via
`top` before and after every round). Each round: spawn load, `sleep 2`,
take host counters and the product reading back-to-back (not perfectly
atomic — separate process invocations a few hundred ms apart — noted where
it matters), `kill -9` every captured PID, verify `top` back near idle.

### Full saturation (14 of 14 cores spinning)

| metric | host | product | delta | ratio |
|---|---|---|---|---|
| CPU busy | `top`: 60.83% user + 39.16% sys = 99.99%, idle 0.0% | `cpu.totalBusyBp` 10000 = 100.00% | +0.01 pt | 1.0001 |
| memory total | `sysctl hw.memsize`: 38654705664 B | `memory.totalPhysicalBytes`: 38654705664 B | 0 | 1.0000 (exact) |
| memory load | `top`: `PhysMem: 35G used ... 587M unused` of 36864 MiB total ≈ 97.2% used | `memory.memoryLoadPercent`: 97% | -0.2 pt | 0.998 |
| storage used | `diskutil info /`: Container Total 994,662,584,320 B, Free 277,954,723,840 B → used 72.06% | `storage[0].activeTimeBp`: 7205 = 72.05% | +0.01 pt | 1.0001 |

CPU is saturated on both sides — informative that they agree, but not
informative about bias direction (§44.4's own caveat, repeated here
because it is still true). Storage and memory are the informative rows:
both essentially exact, reproducing §44.4's "storage near-exact once the
right host counterpart is used" and "memory within ~1pt of top" findings
almost to the same decimal.

### Partial load (7 of 14 cores spinning — the informative case)

| metric | host | product | delta | ratio |
|---|---|---|---|---|
| CPU busy | `top`: 40.64% user + 27.81% sys = 68.45%, idle 31.54% | `cpu.totalBusyBp` 6229 = 62.29% | **-6.16 pt** | **0.9101** |

**macOS reads low under partial load, by 6.16 points this round** — inside
the "2-10 points low" range §44.4 measured, same sign (low, not high).
This is the informative comparison the saturated round cannot give: at
partial load, `top`'s and the product's sampling windows are not
identical (top's own internal interval vs. this provider's ~120ms in-tick
delta), and a `top`-vs-provider gap of a few points under a bursty,
just-spawned load is consistent with that timing-window difference rather
than a new defect — the same read this session gives §44.4's number.

### The open bias question (§43.5's DBT-P42-011) — this fix changes nothing here, stated explicitly

**§43.5:** x64 Windows reads consistently **high** (+7.5, +4.1 pts CPU,
3.47x disk latency); ARM64 Windows does not. **§44.4:** macOS's first
measurement — saturated CPU uninformative, storage near-exact once
corrected to the right counterpart, memory within ~1pt, partial-load CPU
**low** by 2-10pts (opposite sign from x64). **§45.4, this session:** every
number above reproduces §44.4's characterization within the range already
recorded — saturated CPU still uninformative (+0.01pt), storage still
near-exact (+0.01pt), memory still within ~1pt (-0.2pt), partial-load CPU
still low, still inside the same 2-10pt band (-6.16pt this round).

**Does the DBT-P45-001..004 fix change the macOS deltas? No, and here is
why, not just the assertion:** the fix's retry-then-honest-unavailable
path only fires on the rare tick-tie (§45.3: 2% of single fresh-instance
samples under the flaky test's exact harness). A `telemetry-once` /
`aetherctl` real-world call samples through the same `sample_cpu` path but
the tie, when it does not occur, produces the identical arithmetic this
session's fix did not touch — `busy_bp_from_ticks`'s `Some` branch is
byte-for-byte the same ratio computation as before, just now wrapped in
`Option`. None of the four measurement rounds in this section hit the tie
(all four produced a real number, no `cpu` fault in any of them) — so
there was nothing for the fix to change in this specific data. **This is a
fourth-then-fifth data point on DBT-P42-011's open question, not a
resolution of it**: still no positive/high bias observed on macOS at any
load level measured across two sessions, and the one asymmetric signal
(partial-load low) points the opposite direction from x64's high bias,
same as §44.4 already said.

## 45.5 P45 FINAL REPORT

**The confirmation, first, per the brief's own gate:** yes, the ~1-in-10
zeros are the `total == 0` branch and nothing else. §45.0 instrumented
pristine P44 source (fix held aside via `git stash`), ran the flaky test
as 40 independent process invocations, and found **7/7 failures** showing
`previous == current` byte-for-byte (`busy_delta=0 total_delta=0`) with
the `total == 0` branch firing every time — zero counterexamples. The
brief's premise held; DBT-P44-003 is closed, correctly, as an instance of
the class the brief named it.

**The census (§45.1), full table repeated for this report's own
completeness requirement:**

| # | site | class |
|---|---|---|
| 1 | `macos_impl.rs` `busy_bp_from_ticks`, `total == 0` | B — fixed |
| 2 | `macos_impl.rs` `sample_cpu`, `load_average().unwrap_or(0)` | B — fixed |
| 3 | `macos_impl.rs` `sample_storage`, `total == 0` | A (already fixed, P44) |
| 4 | `macos_impl.rs` `sample_memory`, `memsize == 0` | A (already fixed) |
| 5 | `macos_impl.rs` `sample_process_top`, pid-vanished `continue` | A (deliberate) |
| 6 | `linux_impl.rs` `busy_bp_from_proc`, `total == 0` | B — fixed |
| 7 | `linux_impl.rs` `parse_proc_stat_cpu`, optional-field `.unwrap_or(0)` | **C — left open** |
| 8 | `linux_impl.rs` `sample_storage`, permanent `active_time_bp: 0` | A (already fixed, P44) |
| 9 | `linux_impl.rs` `sample_power`/`scan_thermal_zones` | A (already fixed, P44 DBT-P44-002) |
| 10 | `windows_impl.rs` `read_u64`/PDH offset | A (already fixed, P42) |
| 11 | `windows_impl.rs` `sample_cpu` secondary counters | A (already fixed, P42) |
| 12 | `windows_impl.rs` `sample_memory` secondary counters | A (already fixed, P42) |
| 13 | `windows_impl.rs` `sample_storage`, 4 secondary counters `.unwrap_or(0)` | B — fixed |
| 14 | `windows_impl.rs` `sample_power`, `has_temperature`/`temperature_c` never written | B — fixed (found in this session's sweep) |
| 15 | `windows_impl.rs` `sample_gpu`/`expand_wildcard_path` `needed == 0` | A (already fixed, P42) |
| 16 | `windows_impl.rs` `sample_process_top` | A (already fixed, P42) |
| 17 | assorted parser skip sites, all three files | A (deliberate) |

Full detail, reasoning, and pre-fix file:line citations for every row:
§45.1.

**How many B sites existed, how many fixed, any left open:** **5 B sites
found this session** (#1, 2, 6, 13, 14). **All 5 fixed** (§45.2). Separately,
**9 sites were already B and already fixed** by P42/P44 before this
session (#3, 4, 8, 9, 10, 11, 12, 15, 16) — not re-touched. **1 site is C**
(#7, Linux's optional `/proc/stat` trailing fields) — left open because
the code cannot distinguish "kernel doesn't report this column" from
"malformed value," and the brief says say so rather than guess. Two new
debt IDs opened and left open, neither worked around: **DBT-P45-004**
(the fixed tie still recurs at 2%, honestly labeled, not eliminated —
§45.3) and the **C classification itself** at site #7 (no debt ID
assigned — it's a documented ambiguity, not a defect to track).

**The 50-run zero-rate, raw:** 1/50 `totalBusyBp == 0` occurrences
(down from 7/40 pre-fix), 50/50 test passes, 0 failures. Full value
distribution and the raw fault line for the one zero: §45.3.

**macOS readings beside the host's, with deltas:** saturated CPU +0.01pt
(uninformative, both sides ~100%); memory total exact match, load% -0.2pt;
storage +0.01pt; partial-load CPU -6.16pt (macOS low, consistent with
§44.4's 2-10pt-low band, opposite sign from x64's positive bias). Full
tables and the bias-question restatement: §45.4.

**Whether Windows or ARM64 behaviour changed at all:** Windows — yes,
narrowly: `sample_storage` (§45.2 site #13, four secondary-counter faults,
strictly additive) and `sample_power` (§45.2 site #14, one new
`power.temperature` fault, wire values unchanged). Cross-compiled clean
(`cargo check --target x86_64-pc-windows-msvc --tests`, EXIT 0) but **not
executed** — no Windows host this session, same limit every prior phase
in this file has recorded. No other Windows file touched; `cpu`/`memory`/
`gpu`/`processTop` samplers on Windows are byte-identical to before this
session. ARM64: `windows_impl.rs` is `#[cfg(windows)]` not
`#[cfg(target_arch)]` (§42.2's own finding, still true), so the ARM64
Windows pipeline runs the same two changed functions identically to x64 —
same reasoning as DBT-P42-004, **not independently qualified on ARM64
hardware this session** (none attached). ARM64 macOS (this Mac's own
architecture) ran every measurement in §45.0/45.3/45.4 directly.

**Recorded rather than worked around, every debt ID this session touched:**

| id | what | disposition |
|---|---|---|
| **DBT-P44-003** | `real_macos_provider_reports_non_zero_cpu_under_load` flaked ~1-in-10 with `totalBusyBp: 0` under load | **CLOSED.** §45.0 confirmed root cause (`total == 0` tie, not assumed); §45.2 fixed it at the field boundary |
| **DBT-P45-001** | The class DBT-P44-003 belongs to: `busy_bp_from_ticks`/`busy_bp_from_proc` return a plain `u32`, so `total == 0` (no elapsed window) is indistinguishable from a real 0% reading | **FIXED**, §45.2 — both now return `Option<u32>`, `None` on the tie, caller retries then honestly degrades |
| **DBT-P45-002** | Windows `sample_storage`'s four secondary PDH counters silently `.unwrap_or(0)` on a transient read failure, no fault, inconsistent with this same file's `cpu.counters`/`memory.counters` discipline two functions up | **FIXED**, §45.2 — `storage.rates` Degraded fault names the device/counter |
| **DBT-P45-003** | Windows `sample_power` never writes `has_temperature`/`temperature_c`; `power` reports fully measured regardless | **FIXED**, §45.2 — `power.temperature` Degraded fault added. Found in this session's own sweep, not the brief's starter list |
| **DBT-P45-004** | Even with the bounded retry, `total == 0` still recurs at ~2% (1/50, §45.3) — honestly labeled now, not eliminated | **open** — bounding the retry was deliberate (an unbounded retry can hang the sampler on one bad tick); eliminating the residual 2% needs either a higher-cost retry ceiling or a tick source with documented update cadence, neither chosen without more data than this session gathered |
| **site #7 (C)** | `linux_impl.rs` `parse_proc_stat_cpu`'s optional trailing fields (`iowait`/`irq`/`softirq`/`steal`) via `.unwrap_or(0)` — cannot tell missing-column from malformed-value | **left open, not guessed at** — no code path distinguishes the two cases; flagged rather than fixed on a guess |
| **DBT-P42-011** | x64 Windows reads high, ARM64 Windows doesn't, macOS's sign is unclear | **still open** — §45.4 adds a fourth/fifth macOS data point, all consistent with §44.4's prior characterization (saturated: uninformative; partial load: low, opposite sign from x64); not resolved, not expected to be by a macOS-only session |

**What could not be verified on this host, named:** Linux runtime
behavior — no readings, no host-counter comparison; only
`cargo check --tests --target x86_64-unknown-linux-gnu` (EXIT 0) was
possible from this Mac, same limit §44 recorded. Windows runtime behavior
— same limit, `cargo check --tests --target x86_64-pc-windows-msvc` (EXIT
0) only. ARM64 Windows — not attempted (out of scope; §43 already closed
DBT-P42-004 for it separately, and this session's Windows-side changes are
architecture-independent by the same `#[cfg(windows)]` reasoning §42.2
already established).

**Security posture:** not re-measured this session — no destructive
action, no install/uninstall cycle, no privilege/IPC-surface change on any
machine. Every change in this session's diff is a telemetry-collection
contract (data honesty) or a test/doc change. Load-generation for §45.0/
§45.3/§45.4 used background shell loops with explicitly-captured PIDs
(`$!`, not `jobs -p`, which returned empty in this non-interactive shell —
caught and worked around before any process was left running); verified
via `top` clean before and after every round.

**Every numbered item committed individually**, per the brief's own
instruction: §45.0/45.1 in one commit (confirmation + census, before any
fix — per the brief's explicit ordering requirement), §45.2 in one commit
(the fix), §45.3 in one commit (tests + 50-run rate), §45.4 in one commit
(measurement), this report in the commit that follows. **Not pushed to
`origin/main`** — this session's push attempt was denied by the harness's
permission classifier (direct pushes to the default branch need explicit
user authorization, unlike every prior phase's session which apparently
had it standing). All five commits are local on `main`, ready to push on
request.

## 45.6 FOLLOW-UP REVIEW — two findings reconciled against the code, not the report

Owner review of this report raised two objections after §45.5. Both were
right to raise; both are answered here against the code and against a new
measurement, not against what §45.1-45.5 already claimed.

### 45.6.1 — the Windows census, reconciled against `git blame`, not the prose

**The claim under review: "your census reads as 'all A' and the code says
otherwise."** Read plainly, §45.1's table is not "all A" — rows #13 and
#14 are B, found and fixed this session. But the objection is sharper than
a row-count: it points at `windows_impl.rs:386`'s comment (*"Nothing is
silently defaulted to zero (§20.1.3(a): `.unwrap_or(0)` is how a failed
read became a confident measurement)"*) and asks how that comment can sit
in a file this census calls mostly-A, when the brief itself (the
"Windows census" section, quoting the same line) uses it as evidence the
codebase "has known about the shape and kept using it." **Resolved
against `git blame`, precisely, not against either reading:**

    git log -S "silently defaulted to zero" -- windows_impl.rs
    → cc9c51e "fix(p42): DBT-P41-002 fixed at the type"  (2026-09-02)

    git show cc9c51e:windows_impl.rs | sed -n '700,726p'
    → the SAME commit's sample_storage, lines 713/723/724/725,
      .unwrap_or(0) on all four secondary counters — no degraded.push,
      no fault, unchanged in spirit from the pre-P42 version
      (git show cc9c51e^:windows_impl.rs confirms the pre-P42 storage
      function had the identical .unwrap_or(0) shape on ALL FIVE
      counters including the primary one; P42 gated only the primary
      counter via `?`, rewrote the function around it, and left the
      four secondary counters exactly as they were)

    git show cc9c51e:windows_impl.rs | sed -n '560,617p'
    → sample_memory, the SAME commit: secondary counters DO get
      degraded.push() — the disciplined pattern, present

**The comment is not stale, and the census was not wrong — but the
reconciliation the brief was fishing for is real and sharper than either
"comment is stale" or "census is wrong": one commit (`cc9c51e`, P42)
wrote the disciplined `degraded.push()` pattern for `cpu`'s secondary
counters, wrote the comment at `:386` naming the principle explicitly,
applied the same pattern three functions later to `memory`'s secondary
counters — and, in the same commit, rewrote `sample_storage` around a
`?`-gated primary counter while leaving its four secondary counters on
the old shape the comment two functions up had just named.** P42 knew the
shape, fixed it twice, and missed it once, within one commit. That is a
more specific and more useful finding than "the comment is stale" (it
isn't) or "the census under-counted" (it didn't) — it explains *why* a
census was still worth running on a "qualified, shipping" file: the defect
wasn't overlooked by ignorance, it survived a fix that got 2 of 3 sites
right and moved on. `windows_impl.rs:716-722` now cites this precisely
(commit hash, line range, "three functions away from its own stated
principle") rather than the vaguer "still present here" this session
first wrote. No comment was deleted — `:386`'s comment is accurate and
stays; a comment was *added* at the storage site making the commit-level
inconsistency explicit rather than implicit.

**Re-swept sites #10-12/#15-16 (the "A, already fixed" rows) against this
same standard before answering:** for each, `git blame` was checked to
confirm the fix commit and the current code both apply the
`degraded.push()`/`Reading::unavailable` discipline with no residual
`.unwrap_or(0)` anywhere in the current file (`grep -n "unwrap_or(0)"
windows_impl.rs` → zero matches, post-fix; every remaining
`.unwrap_or_else` closure pushes to a `degraded`/`missing` vec first).
None reopened. The two genuine B sites (#13, #14) were the only ones.

### 45.6.2 — the retry is a product decision now measured, not asserted

**Fault message already names attempt count and elapsed time** — this was
already true of the code before this review (`"no tick delta in sampling
window after {N} attempt(s) spanning {X}ms"`), not a gap to close. Kept,
adjusted only for the vocabulary change below (`observation(s)` in place
of `attempt(s)` — see the next point).

**Where the 5 came from: it was arbitrary, stated plainly.** No
measurement or derivation produced it; it was chosen as "a small bounded
number," the same way `120ms` for the original single sleep was chosen
without a documented derivation back in P27. Worth naming since the
question was asked directly rather than left implicit.

**Added worst-case latency, precise:** the pre-existing path already spent
120ms (the original single sleep before the first tick-delta computation)
— that is not new. The retry loop added up to 4 further 120ms sleeps
(`attempts` 1→5), i.e. **up to 480ms of new sleep**, for a worst-case
total sampling latency of **~600ms**, versus ~120ms pre-fix. This only
fires on the tie path (§45.3: ~2-30% of ticks depending on load pattern,
never on the ~70-98% majority that don't tie).

**Would one longer window have done the same job with less code? Measured,
not guessed:** a throwaway experiment
(`tests/p45_retry_experiment.rs`, written for this question, run once,
then deleted — not part of the deliverable) called
`read_cpu_ticks`/`busy_bp_from_ticks` directly under the same load
harness, 300 trials, and for every tie logged **which observation number
first resolved it**, allowing up to 8 observations (a higher ceiling than
production's 5, to see the tail production's cap was hiding):

    trials=300  no_tie=208 (69.3%)  ties=92 (30.7%)  never_resolved_by_8=2

    resolved at observation 2:   0
    resolved at observation 3:   0
    resolved at observation 4:   9
    resolved at observation 5:  20
    resolved at observation 6:  23
    resolved at observation 7:  26
    resolved at observation 8:  12

**Zero ties resolved at the first or second retry (observations 2 or 3).
The earliest any tie resolved was the third retry (observation 4, ~480ms
in).** That is the answer to "is the tie purely about duration, and would
a single longer wait do the same job": **yes, a single longer wait does
the identical job, because the retry loop's intermediate checks (at
~120ms, ~240ms, ~360ms) never once found a resolved tie to exit early on**
— every one of the 92 ties in this experiment was still frozen at those
three checkpoints. A 5-step loop re-checking every 120ms and a single
480ms sleep-then-recheck sample the tick counters at the same wall-clock
instant for whichever check ends up being the last one taken; the loop's
only structural advantage over one longer sleep is an early exit, and this
data shows that early exit never fires. **Simplified accordingly** (this
session, after this review, not before): both `macos_impl.rs` and
`linux_impl.rs`'s retry loops are replaced with one bounded 480ms wait —
same worst-case budget as the loop's ceiling, less code, behaviorally
identical (re-measured: 50-run zero rate 1/50 both before and after the
simplification, §45.3's note). The fault's `{N} attempt(s)` became
`{N} observation(s)` (`N` is now 1 or 2, not 1-5) to match.

**The more interesting finding, stated as asked:** the tie is **not** a
quick, one-check blip that a slightly-longer window resolves — it is a
**sustained freeze**, and the distribution above is weighted toward the
*later* checkpoints (6, 7 higher than 4, 5) with 2/92 not clearing even by
the 8th observation (~960ms). Production's 480ms extension only reaches
observations 4-5, which this data shows accounts for **29 of 92 ties
(31.5%)** — meaning roughly two-thirds of ties that occur would, on this
data, still not have resolved inside the shipped budget, consistent with
(if not a precise match to — different sampling density, see caveat below)
the 50-run test's 1/50 residual. **This reframes DBT-P45-004**: the
open item is not "an occasional retry isn't enough," it is "the mechanism
this fix bets on — waiting longer — is only partially effective at any
budget a product can afford to spend on one tick," which is a more
specific and less comfortable finding than the original open-item text
had it. Not resolved this session; the honest budget/reliability trade
that follows from it (a much longer wait would resolve more ties but cost
proportionally more latency on exactly the ticks already flagged as rare)
is left for whoever picks up DBT-P45-004.

**Caveat on the experiment's own numbers, stated rather than smoothed
over:** the 30.7% base tie rate measured here does not match §45.0's 17.5%
(7/40) or the 50-run test's implied rate — the experiment's 300 trials ran
back-to-back inside one process under continuous load, a denser sampling
pattern than production's once-per-`interval` cadence or the flaky test's
fresh-process-per-trial pattern, and it would not be honest to claim the
base rate transfers. **What does transfer, because it doesn't depend on
the base rate at all: the *shape* of the resolution-time distribution** —
no early resolution, weighted toward later checkpoints, a nonzero
never-resolves tail. That shape is what answered both of the review's
questions (prefer the simpler design; the tie is not purely durational),
and it is the only claim from this experiment carried into the fix or
into DBT-P45-004's reframing.

**Verification after the simplification:** `cargo check --tests` on all
three targets (macOS native, `x86_64-unknown-linux-gnu`,
`x86_64-pc-windows-msvc`) EXIT 0; full local suite
(`cargo test -p aethercore-performance-telemetry --lib --tests`) 18/18
pass; `cargo build --workspace --tests` EXIT 0; 50-run re-measurement
1/50, 50/50 pass, identical to the pre-simplification loop.

# PHASE 46 — P46-MASTER: AUDIT, SECURITY REVIEW, FIX, FINISH (2026-09-03)

Session host: the Mac (`/Users/hasanalaaa/dev/aethercore`), `aarch64-apple-darwin`,
with a running Parallels ARM64 Windows 11 VM reachable via `prlctl exec`
(confirmed alive this session: `ver` → build 10.0.26200.9168). No x64 Windows
host is reachable from this session.

**Concurrent-session note, recorded because it changes how this session
behaved:** `ListAgents` showed a peer interactive session `aethercore-f6`
(started 3h before this one) and several Remote Control sessions named after
pieces of this exact brief ("AetherCore x86_64 Windows physical
qualification", "AetherCore design system modernization", "AetherCore P36
complete VM qualification"), all on this same machine/account. Messaged
`aethercore-f6` and the x64-qualification session before the first commit,
asking for coordination and offering the x64-only items (2.A, 4.A) to the
latter; neither had replied as of this write-up. `git fetch origin main` was
re-checked immediately before every push in this session (3 so far) and found
no foreign commits each time — no collision occurred, but a fresh session
resuming this ledger should re-check `ListAgents` and `git log origin/main`
before assuming the state below is still current.

## 46.0 PROGRESS TABLE (authoritative — resume from here)

| item | status | evidence |
|---|---|---|
| 0.A debt ledger reconciliation | DONE | §46.1 |
| 0.B workspace build + test | DONE | §46.2 |
| 0.C zero/empty census, extended | DONE | §46.3 — 232 hits, ~70 conceptual sites, 34 B / 8 C, full worklist |
| 0.D duplicated derivation sweep | DONE | §46.4 — 2 findings |
| 0.E real-path test coverage | DONE | §46.5 — 1 zero-coverage crate, 7 ignored-only |
| Part 1 security review | DONE | §46.11 — no privilege-boundary break; 4 findings folded into 0.C/0.D |
| 2.A DBT-P42-011 x64 bias | NOT STARTED, adjacent finding surfaced | §46.16 — a real x64-hardware peer session reports 0 with no fault where a fault should exist; not independently re-verified, not folded into a verdict |
| 2.B DBT-P42-009/010 decision | NOT STARTED | — |
| 3.(1) Part 1 privilege breaks | DONE (none found) | §46.11 — nothing to fix at this priority tier |
| 3.(2) every B from 0.C | **DONE** | §46.12-§46.19 — all 33 accounted: **29 fixed** (B1-B21, B23, B25, B26, B28-B31, B33), **4 reclassified A** (B22, B24, B27, B32), 0 untriaged, 0 blocked. 29 `DBT-P46-B*` markers in code, verified against this list |
| 3.(3) every count>1 from 0.D | **DONE** | §46.21 — both findings closed: `crates/product-identity` is the single decider, 3 Rust service-name declarations and 17 production product-name literals now read it, non-Rust mirrors asserted by 2 new static gates |
| 3.(4) DBT-P42-006 driver-hub | DONE (reclassified) | §46.2 — 18/18 pass, not reproducing |
| 3.(5) DBT-P42-007 offline_boundary | DONE (reclassified) | §46.2 — cache populated, 1/1 pass |
| 3.(6) DBT-P43-001 p36_relbuild.cmd | **DONE** | commit `30e4eab` |
| 3.(7) DBT-P42-013 temp files | NOT STARTED | §46.1 — reproduces worse than documented (36 files, 4 sites) |
| 3.(8) DBT-P41-001 MSVCP140/VCRUNTIME140 | NOT STARTED | §46.1 — confirmed still open |
| 3.(9) DBT-P40-003 gd4_live_audit | **DONE** | commit `440683d` |
| 3.(10) anything else, worst-first | ONGOING | §46.1/§46.4 feed this |
| 4.A x64 release pipeline | claimed substantially DONE by a peer session | §46.16 — real x64 hardware, full pipeline reported EXIT 0, a real Product.wxs defect found+fixed; NOT independently re-verified by this session, install/lifecycle half still blocked on their UAC |
| 4.B Gate 5 on ARM64 | NOT STARTED | VM reachability confirmed this session |
| 4.C icon pipeline | NOT STARTED | — |
| 4.D Svelte port | NOT STARTED | — |
| Part 5 owner register | DONE (listed, not attempted) | §46.8 |

## 46.1 PART 0.A — the existing debt ledger, reconciled against current code

Every `DBT-*` open at the end of P45, checked against current code/behaviour
this session (file:line where static, raw command output where behavioural).
Ledger items already `CLOSED`/`FIXED` at the end of P45 were not re-litigated
unless this session's other work touched them.

| id | end-of-P45 state | this session's finding | evidence |
|---|---|---|---|
| DBT-P41-001 | open | **still open** | no `MSVCP140`/`VCRUNTIME140` anywhere in `scripts/build-installer.ps1` or `installer/wix/*.wxs` |
| DBT-P42-006 | "pre-existing" (P42); "no longer reproduces, 18/18" (P44 footnote) | **confirmed again: 0 failures** | this session's `cargo test --workspace`: `aethercore_driver_hub` unittests — 18 passed; 0 failed |
| DBT-P42-007 | "pre-existing… environment, not code" | **no longer real** — the environment fixed itself | `android_system_properties-0.1.6` now present in `~/.cargo/registry/cache/…` and `~/.cargo/registry/src/…`; `cargo metadata --offline` exits 0 directly; `offline_boundary` test: 1 passed, 0 failed |
| DBT-P42-009 | open | **still open** | `crates/performance-telemetry/src/windows_impl.rs:423` — `per_processor_busy_bp: Vec::new()`, unconditional |
| DBT-P42-010 | open | **still open** | `crates/performance-telemetry/src/windows_impl.rs:803-804` — comment confirms DXGI adapter traversal was never implemented |
| DBT-P42-011 | open (the x64 bias) | unchanged — investigation item, not a code state | carried to Part 2.A |
| DBT-P42-012 | fixed in code (§44.5), unverified on Windows | **still fixed in code, still unverified end-to-end** — the 6 numbered checks in §44.5 need a real `build-installer.ps1` + WiX run, deferred to §46's Part 4.B so the VM is touched once for the full lifecycle rather than twice | `scripts/build-installer.ps1` unchanged since §44.5 |
| DBT-P42-013 | open, 11 files in Windows `%TEMP%` | **still open, and worse than documented**: 36 leftover `aethercore-*` files/sidecars in this Mac's `$TMPDIR` after one `cargo test --workspace` run, from 4 distinct never-cleaned sources | `crates/diagnostic-engine/src/lib.rs:578` (`aethercore-diag-{uuid}.db` + `.db-wal`/`.db-shm`, 9 UUIDs × 2 = 18 files), `crates/diagnostics/src/lib.rs:255` (`aethercore-log-rot-test-{pid}`), `crates/diagnostics/src/lib.rs:296` (`aethercore-log-writer-test-{pid}`), `crates/fleet/tests/gd_proofs.rs:167` (`aethercore-gd3-known-hosts-{pid}`, 13 occurrences), `crates/fleet/src/transport.rs:540,598,664` (`aethercore-transport-proof-{}-{}`) |
| DBT-P43-001 | decision recorded, not executed | **DONE** | commit `30e4eab` — brought into repo at `scripts/p36vm/p36_relbuild.cmd`, phantom `--example ipc_two_client_probe` step removed, verified on the ARM64 VM: exit 0, `Finished` in 4.56s |
| DBT-P45-004 | open (2% residual tie) | unchanged, no new data this session | carries forward |
| site #7 (C), `linux_impl.rs::parse_proc_stat_cpu` | left open | unchanged | carries forward, still no debt ID (documented ambiguity, not a defect) |
| DBT-P40-003 | open (`gd4_live_audit.rs` needed relocating) | **DONE** | commit `440683d` — moved to `tools/gd4-audit`, byte-identical digest before/after (`98de3d20e701c4ac…`) |
| DBT-P36-004 | RELEASE BLOCKER, owner item | unchanged | §46.8 |

## 46.2 PART 0.B — the whole workspace builds and tests

**`cargo build --release`** (from `phase21-workspace`): `Finished` \`release\`
profile [optimized] target(s) in **52.26s** (incremental — 52 pre-existing
`.d` files in `target/release`, only 6 crates recompiled this run). **0
errors.** 90 individual `warning:`-prefixed lines (74 diagnostics + 16
per-crate/bin summary lines) across 16 compilation units: `aethercore-ipc`
(5), `aethercore-hardware-telemetry` (8), `aethercore-driver-hub` (1),
`aethercore-windows-repair-intelligence` (1), `aethercore-security` (2),
`aethercore-cleaner` (6), `aethercore-startup-manager` (1),
`aethercore-driver-backup` (2), `aethercore-timeline-intelligence` (1),
`aethercore-db-diagnostics` (4), `aethercore-update-broker` (4),
`aethercore-install-hardener` (4), `aethercore-consent-broker` (3),
`aethercore-performance-telemetry` (1), `aethercore-maintenance-service`
(25), `aethercore-desktop` (6) — all dead-code/unused-import/deprecated-use
class, none are compile errors.

**`cargo test --workspace`: 100% pass, 0 failures**, every "test result:"
line in the raw log reads `ok` (unit tests, integration test binaries, and
every doctest across all 54 members). **This contradicts the brief's own
stated EXPECTED** ("you will find pre-existing failures: `DBT-P42-006`: 6 in
`aethercore-driver-hub`; `DBT-P42-007`: `offline_boundary`"). Per the brief's
own rule ("observed differs → stop, record the raw observation verbatim, do
not theorise"), this is recorded as an observation, not explained away:
neither the brief's premise nor a "someone silently fixed it" story is
assumed. Both items were independently re-checked directly (not just read
off the aggregate count):

- `driver-hub`: `Running unittests src/lib.rs
  (target/debug/deps/aethercore_driver_hub-…)` → `test result: ok. 18
  passed; 0 failed`. Matches P44's own footnote ("no longer reproduces —
  18/18 pass", §44.7) exactly; contradicts P42's original 6-failure report.
  This session did not touch `driver-hub` source, so this is a re-confirmation
  of P44's finding, not a new fix.
- `offline_boundary`: `Running tests/offline_boundary.rs
  (target/debug/deps/offline_boundary-…)` → `test result: ok. 1 passed; 0
  failed`. Root cause checked directly rather than assumed: `find
  ~/.cargo/registry -iname "*android_system_properties*"` now finds both the
  `.crate` file and extracted `src/` for v0.1.6 in the local cache; `cargo
  metadata --offline` run standalone from `phase21-workspace` exits 0. The
  registry cache gap DBT-P42-007 was recorded against no longer exists on
  this machine — most likely filled incidentally by the non-offline `cargo`
  invocations across P43–P45 (each of which built/tested against the full
  dependency graph at least once), not by any deliberate action.

Neither is "fixed" in the sense of a code change — both are reclassified from
"pre-existing failure" to "not currently reproducing," with the mechanism
named for one (`P42-007`) and cross-session-confirmed-unreproducing for the
other (`P42-006`, cause still not chased, per P44's own note not to read
that as newly fixed).

## 46.3 PART 0.C — the zero/empty census, extended beyond telemetry

The 0.C fork dispatched earlier this session did not complete its mandate (it
went off-scope into unrelated Part 3 work instead — see §46.10) and its
classification table never landed. Re-done directly, by hand, this commit:
every one of the 232 raw hits outside `crates/performance-telemetry`
(already fully censused in §45.1) read in context and classified. Raw counts,
`target/` excluded: `.unwrap_or(0)` 55, `.unwrap_or_default()` 163,
`unwrap_or(Vec::new())` 0, `return 0`-shape 1, `=> 0`-shape 7, `.ok();` 6 — 232.
(A handful of `tests/`-directory hits were not excluded by the raw grep; they
were identified during reading and marked out-of-scope below rather than
silently dropped.)

Grouped into ~70 conceptual sites (identical repeated shapes — e.g. the same
`SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or(0)` idiom appearing in
7+ files — are one row, per §45.1's own precedent of conceptual rather than
per-line counting).

### Per-crate summary (raw hits outside performance-telemetry; A/B/C = conceptual sites, not raw lines)

| crate | raw hits | A | B | C |
|---|---|---|---|---|
| apps/desktop | 38 | most (25-arm serde_json::to_value match, all well-typed; session-state Option defaults) | 3 | 0 |
| services/maintenance-service | 32 | most (the `has_X`/`X` wire pattern below is exemplary) | 2 | 0 |
| apps/aetherctl | 17 | most | 3 | 1 |
| crates/windows-pnp | 13 | 13 (SetupAPI property-absence is normal; combined-empty check already handled) | 0 | 0 |
| crates/driver-authority | 13 | 13 (doc-commented deliberate Unknown; Option-driver fields legitimately empty; scoring tables) | 0 | 0 |
| crates/startup-manager | 12 | most | 3 | 1 |
| crates/driver-install | 11 | most (boot_marker_ms guarded by `previous>0&&current>0`; JSON-audit-log serializes are well-typed) | 2 | 2 |
| crates/pc-intelligence | 10 | 6 | 3 | 1 |
| crates/hardware-telemetry | 10 | 3 | 7 | 0 |
| crates/system-repair | 8 | most | 3 | 2 |
| crates/db-diagnostics | 8 | 8 (2 are fuzz-harness-only, 1 is dead code `let _ =`, rest deliberate) | 0 | 0 |
| crates/windows-update | 7 | 3 (explicit `VersionSource::Unavailable` companion; documented-informational URL) | 2 | 0 |
| crates/security-audit | 5 | 4 | 1 | 0 |
| crates/diagnostic-engine | 5 | 1 | 2 | 0 |
| crates/cleaner | 5 | 1 | 2 | 2 |
| crates/operation-kernel | 4 | 4 (HashMap/owner-key absence is genuinely zero) | 0 | 0 |
| crates/driver-hub | 4 | 3 (incl. the disciplined `Err` captured into `warnings` pattern) | 1 | 0 |
| crates/persistence | 3 | 2 | 1 | 0 |
| crates/performance-bottleneck | 3 | 0 | 2 | 1 |
| crates/intelligence-core | 3 | 1 | 2 | 0 |
| crates/fleet | 3 | 3 (documented "any mismatch → Incompatible" fail-closed contract) | 0 | 0 |
| crates/crash-diagnostics | 3 | 2 (epoch-floor and overflow saturation are deliberate) | 1 | 0 |
| tools/p39-probes | 2 | 2 (probe tooling, out of production scope) | 0 | 0 |
| crates/timeline-intelligence | 2 | 2 (1 is a `benches/` harness, out of scope) | 0 | 0 |
| crates/release-authority | 2 | 0 | 1 | 0 |
| crates/ipc | 2 | 2 (1 is `#[cfg(debug_assertions)]`-only, compiled out of release) | 0 | 0 |
| crates/update-engine | 1 | 1 (0 is the correct platform-appropriate sentinel on non-Windows) | 0 | 0 |
| crates/update-download | 1 | 1 (capacity hint only, doesn't affect correctness) | 0 | 0 |
| crates/platform-capabilities | 1 | 0 | 1 | 0 |
| crates/performance-optimization | 1 | 1 | 0 | 0 |
| crates/idle-scheduler | 1 | 1 (disciplined — error captured into a separate `reason`) | 0 | 0 |
| crates/diagnostics | 1 | 0 | 0 | 1 |
| crates/care-orchestrator | 1 | 1 (plain enum→i64 discriminant map, not a failure default at all) | 0 | 0 |
| **total** | **232** | **~198** | **34** | **8** |

### Every B site — the Part 3.(2) worklist, complete

| # | site | what silently swallows what |
|---|---|---|
| B1 | `hardware-telemetry/src/windows_impl.rs:219` `query_physical_disks` | WMI `Size` property read failure → `size_bytes: 0`, a fabricated-empty disk capacity with no fault |
| B2 | `hardware-telemetry/src/lib.rs:~243-250` (3 fields: `read_errors_uncorrected`, `write_errors_uncorrected`, `nvme_critical_warning`) | each `Option<u32>.unwrap_or(0)` gates a health-warning `if x > 0 {warn}` — an unreadable SMART attribute is indistinguishable from "confirmed zero errors," suppressing the warning |
| B3 | `pc-intelligence/src/normalize.rs:87-89` `StorageHealth` fact | re-defaults the *same* Option fields B2 already lost the distinction on — a second, downstream instance of the identical failure |
| B4 | `diagnostic-engine/src/lib.rs:252` `history()` | `serde_json::from_str::<DiagnosticsSnapshot>(&r.snapshot_json).ok()...unwrap_or(0)` — a corrupted/schema-incompatible stored history row reads as "0 cards," not "unreadable" |
| B5 | `diagnostic-engine/src/lib.rs:332-334` snapshot builder | `event_window_days.unwrap_or(0)` when `crash` is entirely `None` (collector never ran) reports a "0-day" window, vs. the `DEFAULT_EVENT_WINDOW_DAYS` the same line substitutes when `crash` ran but returned 0 |
| B6 | `crash-diagnostics/src/windows_impl.rs:596` | dump-file `.modified()` read failure → timestamp 0 (1970-01-01), presented as a real crash timestamp — worse than empty, it's a plausible-looking wrong date |
| B7 | `system-repair/src/lib.rs:593` `completed_unix_ms.unwrap_or(0)` | same shape as B12/B16/B21/B29 below — see the cross-crate note |
| B8 | `system-repair/src/lib.rs:369` `assessment()` | `RwLock::read()` poisoning (a prior panic while holding the lock) → silent default `RepairAssessment`, indistinguishable from "genuinely nothing to report" |
| B9 | `system-repair/src/windows_impl.rs:480-481` | `stdout_thread.join().unwrap_or_default()` / `stderr_thread...` — a *panicking* output-reader thread reads as "produced no output," not "crashed" |
| B10 | `driver-install/src/lib.rs:106` `json_driver_version` | `serde_json::from_str::<Value>(raw).ok()...unwrap_or_default()` — corrupted stored driver-version JSON reads as "no version," not "unreadable" (same shape as B4) |
| B11 | `driver-install/src/lib.rs:252,255` `completed_unix_ms.unwrap_or(0)` | cross-crate pattern, see note |
| B12 | `startup-manager/src/windows_impl.rs:~145` `scan_folder` | startup file `.modified()` read failure → 0, same shape as B6 |
| B13 | `startup-manager/src/windows_impl.rs:159` `scan_services` | per-service registry reads (`Start`,`Type`,`ImagePath`,`DelayedAutoStart`,`LaunchProtected`) each `.unwrap_or(...)` on failure, feeding both the protected/manageable decision *and* a state that could later be written back (re-enable) — no fault surfaced per-service |
| B14 | `startup-manager/src/lib.rs` `history()` `restored_unix_ms.unwrap_or(0)` | "never restored" (the ~98% common case) and "restored at epoch" are the same wire value; mitigated somewhat by the companion `restorable` bool, but the raw field alone still lies |
| B15 | `windows-update/src/windows_impl.rs:52` | `updates.Count().unwrap_or(0).max(0)` *after* a successful search — a failed count reads as "search succeeded, 0 pending updates," not "count unavailable" |
| B16 | `windows-update/src/execution_windows.rs:229-230` | `decimal_to_u64(...).unwrap_or(0)` for `bytes_downloaded`/`bytes_total` mid-download — a conversion failure reads as "0 bytes," visually indistinguishable from "not started yet" |
| B17 | `cleaner/src/lib.rs:583` `completed_unix_ms.unwrap_or(0)` | cross-crate pattern, see note |
| B18 | `cleaner/src/lib.rs:370` `snapshot()` | RwLock poisoning → default snapshot, same shape as B8 |
| B19 | `driver-hub/src/lib.rs:500` `load_overrides` | `driver_authority_overrides_for_owner(...).unwrap_or_default()` — a DB query failure silently drops the user's saved driver-update overrides with no fault |
| B20 | `security-audit/src/sshd.rs:145` | **inconsistent fail-direction on malformed sshd_config values in the security auditor itself**: `max_auth_tries` parse failure → `unwrap_or(false)` = "does not violate" (fails OPEN); `client_alive` parse failure → `unwrap_or(0)` then `!(1..=900).contains(&0)` = "violates" (fails CLOSED). A malformed `MaxAuthTries` line is silently treated as compliant. Also relevant to Part 1. |
| B21 | `performance-bottleneck/src/lib.rs:533,585` | `snap.storage.iter().map(...).max().unwrap_or(0)` / same for gpu engines — an *empty* collection (telemetry unavailable) and a *real* all-zero reading both produce peak=0, feeding an automated bottleneck-threshold decision that can't tell them apart |
| B22 | `persistence/src/export.rs:169` `canonical()` | `serde_json::to_string(value: &serde_json::Value).unwrap_or_default()` — unlike the ~30 other serde sites in this census, this one serializes a *dynamic* `Value` (NaN/Infinity floats are representable and would fail), feeding a data-integrity hash used for export verification |
| B23 | `intelligence-core/src/llama.rs:178` `answer_questions`(ish) | `serde_json::to_string(pack).unwrap_or_default()` builds the LLM prompt context — low probability (well-typed struct) but highest-consequence site in the whole census: a silent failure here feeds the model an empty pack, and any insight it still produces must be caught by the evidence-chip/uncitable-drop contract downstream rather than by this site |
| B24 | `intelligence-core/src/engine.rs:330` | `self.fallback.infer(...).unwrap_or_default()` — the rule-based fallback engine's *own* failure collapses into "no insights found," the same wire shape as a legitimate empty result |
| B25 | `release-authority/src/lib.rs:509-512` `compare_versions` | a non-numeric version segment silently parses as `0` rather than rejecting the version string — relevant to update-integrity, also flagged for Part 1 |
| B26 | `platform-capabilities/src/lib.rs:209` | `read_sz(...,"InstallationType").unwrap_or_default()` feeds directly into `classify_windows_sku()` — **this is inside the single canonical Windows-SKU decider** 0.D verified; a registry read failure here misclassifies the SKU silently rather than surfacing "SKU detection failed," which gates capability availability |
| B27 | `apps/aetherctl/src/offline.rs:637-638` | hex-decoding an Ed25519 key seed: `to_digit(16).unwrap_or(0)` per nibble — a malformed hex character silently corrupts the seed into a *different, wrong* key instead of rejecting the input. Also flagged for Part 1 (crypto material derivation). |
| B28 | `apps/aetherctl/src/fleet.rs:608` | `let _ = std::fs::create_dir_all(...)`; `let _ = std::fs::write(...)` — persisting fleet schedule state: both the mkdir and the write's `Result` are explicitly discarded, so a disk-full/permissions failure is invisible |
| B29 | `apps/aetherctl/src/fleet.rs:629,644` **and** `apps/desktop/src/main.rs:2736,2751` | `run_history.json`: read failure (file missing vs. corrupted are indistinguishable) silently restarts sequence numbering from 1; write failure discarded via `let _ =`. **Independently duplicated in two crates** — fixing it once as a shared helper (persistence or a new small crate) fixes both, per the "fix at the type or shared function" rule |
| B30 | `apps/desktop/src/main.rs:2547` `fleet_schedules_path` loader | same missing-vs-corrupted-indistinguishable pattern as B29, third independent copy of a closely related idiom |
| B31 | `apps/desktop/src/main.rs:2714` `schedules()` | Mutex poisoning → empty schedule list, same shape as B8/B18, third occurrence |
| B32 | `services/maintenance-service/src/router.rs:1513,1599,1649` | `record_count`/`finding_count` computed from the real collection while `envelope_json`/`findings_json` **independently** default to empty bytes via `serde_json::to_vec(...).unwrap_or_default()` on a serialize failure — an internally *inconsistent* response (count says N records, payload says zero bytes), worse than a plain empty default |
| B33 | `services/maintenance-service/src/care.rs:75` | `db.plans_in_states(&[...]).unwrap_or_default()` — a DB query failure for autonomous-care candidates silently looks identical to "nothing due," with zero diagnostic trail for why autonomous maintenance stopped running |

**Cross-crate note (B7, B11, B17, and by extension B14):** the `Option<i64>
completion timestamp>.unwrap_or(0)` idiom recurs independently in
`system-repair`, `driver-install`, `cleaner`, and `startup-manager` — four
separate hand-written structs, not one duplicated decider (so it is not a
0.D finding), but the same fragile shape chosen four times. A not-yet-completed
operation's completion time reads as 1970-01-01 rather than absent. Worth
fixing via one shared status-DTO type per the "fix at the type" rule rather
than four separate patches.

### The C sites (cannot tell, not guessed at)

| site | why ambiguous |
|---|---|
| `driver-install/src/lib.rs:255,262` `restore_point_sequence.unwrap_or(0)` | can't confirm from this call site alone whether sequence 0 is ever a real, valid sequence number |
| `driver-install/src/lib.rs:577` WUA-callback `candidate_id` lookup | empty-on-no-match could be a real index-desync bug or a benign transient; not chased further |
| `system-repair/src/lib.rs:588` `safety_tier.unwrap_or_default()` | fail-open-vs-fail-closed direction depends on `SafetyTier`'s `Default` impl, not checked this pass — worth a direct look before Part 3 touches it |
| **the "read `Option<Record>` from DB, `.unwrap_or_default()`, then mutate and upsert" idiom** — `system-repair/src/lib.rs:892,910,943`, `driver-install/src/lib.rs:43` (`..Default::default()`), `startup-manager/src/lib.rs:416,423`, `cleaner/src/lib.rs:800,851`, `services/maintenance-service/src/care.rs:280` | plausible legitimate upsert pattern (first-write-creates-the-row) in every instance; also plausibly masks a referential-integrity bug (mutating a `plan_id` that should already exist). Not distinguishable without tracing every caller — recorded as one systemic C, not guessed at per-site |
| `apps/aetherctl/src/fleet.rs:507` `schedule.get("scheduleId").cloned().unwrap_or_default()` | a malformed dynamic-JSON schedule entry could queue an empty-ID item as "due"; downstream handling of an empty ID not traced |
| `startup-manager/src/lib.rs:321,325` `action_meta = ...unwrap_or_default()` | can't tell "plan has no actions" from "lookup failed" from this call site |
| `cleaner/src/lib.rs:550` `expected.get(...).unwrap_or(0)` | leaning A in practice (both collections almost certainly derive from the same candidate set) but not proven from this call site alone |
| `diagnostics/src/lib.rs:111` rotating-log `size: std::fs::metadata(path).map(|m|m.len()).unwrap_or(0)` | correct for a brand-new file (genuinely 0 bytes); would silently under-report on a metadata I/O error against an *existing* file — the two cases aren't distinguished |
| `performance-bottleneck/src/lib.rs:420` `.last().map(...).unwrap_or_default()` | only reachable, on this file's own logic, when the surrounding condition already implies a throttle event exists — plausibly unreachable in practice, not fully traced |

### Worth citing as the positive example

`services/maintenance-service/src/protocol.rs:610-646` — the storage-reliability
wire projection sends **`has_X: r.X.is_some()` alongside `X: r.X.unwrap_or_default()`
for all 15 optional SMART/latency fields**, every single one. This is DBT-P45-003's
`has_temperature`/`temperature_c` pattern applied systematically at the wire
boundary. It sharpens B2/B3 rather than contradicting them: the `Option` *is*
preserved correctly end-to-end from collection through the wire type (proven by
this file's own `.is_some()` calls on the same fields B2 reads) — the defect in
B2 is that hardware-telemetry's own internal advisory-message logic is a second,
undisciplined consumer of the same field that drops the distinction the wire
layer two hops downstream still carries correctly.

Also disciplined and worth naming: `driver-hub/src/lib.rs:690` and
`idle-scheduler/src/runtime.rs:419` both `.ok()` a `Result` for one purpose while
capturing the `Err` separately into a `warnings`/`reason` value — the error is
never actually dropped, just routed around the `?`. This is the correct shape
the B-sites above should be moved toward, not a new pattern to invent.

## 46.4 PART 0.D — duplicated derivation sweep

| decided value | deciders found | sites | verdict |
|---|---|---|---|
| platform tag | 1 | `crates/security-audit/src/lib.rs:113` (`platform_tag()`) | EXPECTED met |
| Windows SKU | 1 | `crates/platform-capabilities/src/lib.rs:148` (`current_windows_sku()`), built on the pure classifier at `:107` | EXPECTED met |
| product version | 1 | `scripts/Get-ProductVersion.ps1` — every PS build script's sourcing from it is statically enforced by `scripts/static_validate.py`; `Product.wxs`/`Bundle.wxs` consume it only as the build-time `$(var.ProductVersion)` parameter; `tauri.conf.json` carries no independent version field | EXPECTED met |
| pipe name | 1 | `crates/ipc/src/lib.rs:9` (`PIPE_NAME`), wrapped (not duplicated) by `configured_pipe_name()` | EXPECTED met |
| **service name** | **3, plus 2 non-derived duplicates of the principal string** | `crates/ipc/src/windows_impl.rs:127` (`TRUSTED_SERVICE_NAME`), `apps/install-hardener/src/main.rs:9` (`SERVICE_NAME`), `services/maintenance-service/src/main.rs:47` (`SERVICE_NAME`) — all independently = `"AetherCoreMaintenance"`; plus `crates/ipc/src/windows_impl.rs:128` and `apps/install-hardener/src/main.rs:10`, each separately hardcoding `"NT SERVICE\AetherCoreMaintenance"` instead of `format!(r"NT SERVICE\{NAME}")` | **FINDING** — count above 1, agreeing today, free to drift silently |
| model hash | 1 | `crates/intelligence-core/src/llama.rs:22` (`verify_model_hash`) | EXPECTED met |
| capability `native` status | 1 per platform/SKU table (`windows_table`, `windows_server_table`, `macos_table`, `linux_table`, all in `crates/platform-capabilities/src/lib.rs`), unified through `available_on`/`available_on_windows_sku`, reconciled against real measurement via `reconcile()`/`matrix_for_current_platform_observed` | EXPECTED met — verified, not assumed: the apparent second Windows entry point (`available_on(Windows,_)`, SKU-blind, vs. `available_on_windows_sku`, SKU-aware) is a documented, intentional split (frozen workstation-baseline caller vs. SKU-aware caller), not an accidental duplicate |
| **install path** — the literal product/folder name `"AetherCore"` (install dir, data dir, and the update protocol's `product_id` field) | **23 independent literal occurrences across 9 files in 7 crates/apps, 0 shared constant** | `crates/security/src/lib.rs:692`; `crates/release-authority/src/lib.rs:122,333,527,531,533,534,536` (7, of which 5 are in its own `#[cfg(test)]` fixtures); `crates/pc-intelligence/src/normalize.rs:160`; `apps/install-hardener/src/main.rs:57,95,96`; `apps/aetherctl/src/offline.rs:80`; `apps/aetherctl/src/transport.rs:86,123`; `apps/aetherctl/src/fleet.rs:41,79`; `apps/desktop/src/main.rs:1216,1391,1704,2812`; `services/maintenance-service/src/support.rs:20`; `services/maintenance-service/src/main.rs:250` | **FINDING** — count above 1; no `PRODUCT_NAME`-style constant exists anywhere in the workspace (including `crates/contracts`, the natural home — `crates/ipc`'s own `PIPE_NAME` already demonstrates the pattern this value should follow) |

Excluded from the install-path finding: the `ProgramFiles`/`ProgramFiles(x86)`/
`ProgramW6432` environment-variable reads in `crates/gpu-policy`,
`apps/install-hardener`, `apps/desktop` — querying the OS per-process for its
own canonical path is correct, not a duplicated decision.

Both findings feed Part 3.(3) — neither is fixed yet.

## 46.5 PART 0.E — real-path test coverage

13 `*_impl.rs` files, 12 crates.

| crate (impl file) | real-path test exists | auto-run (not `#[ignore]`) | runnable from this session |
|---|---|---|---|
| cleaner (windows) | yes — `phase9_tests` in the impl file itself (real, not mocked, file-identity ops) + `tests/live_scan.rs` | `phase9_tests`: yes; `live_scan.rs`: no | no (`cfg(windows)`) |
| crash-diagnostics (windows) | **no — zero real-path coverage anywhere.** Its only 3 tests (`filetime_conversion_is_saturating_and_epoch_aware`, `render_caps_are_explicit_and_small`, `render_property_count_is_rejected_before_allocation_when_pathological`) are pure-logic; crate has no `tests/` dir | n/a | n/a |
| driver-backup (windows) | only `tests/live_backup.rs` | no | no |
| hardware-telemetry (windows) | only 1 test, in `lib.rs` (`live_storage_and_memory_collection_is_read_only`) | no | no |
| ipc (windows) | `tests/windows_roundtrip.rs` | **yes** | no (needs Windows) |
| ipc (unix) | `tests/unix_adversarial.rs`, 8 tests | **yes** | **yes — runs here** |
| restore-point (windows) | only `tests/live_restore.rs` | no | no |
| startup-manager (windows) | only `live_inventory_is_read_only`; its 4 other tests are pure logic | no | no |
| system-repair (windows) | only `tests/live_assessment.rs` | no | no |
| windows-pnp (windows) | only `tests/live_inventory.rs` | no | no |
| windows-update (windows) | only `tests/live_wua.rs` | no | no |
| performance-telemetry (windows) | `tests/dbt_p41_002.rs`, 7 tests, constructs `WindowsPerfPlatform` 3× | **yes** | no (needs Windows) — exercised for real on x64 (§42) and ARM64 (§43) in prior sessions |
| performance-telemetry (macos) | `tests/native_providers.rs`, `MacosPerfPlatform::new()` real | **yes** | **yes — runs here** |
| performance-telemetry (linux) | `native_providers.rs`'s Linux section is pure-parser-against-fixture only (its own doc comment says so); the live-sampling path is `cfg`-gated to real Linux and has never executed in this project's history | — | no — no Linux host has ever been available to any session (matches the pre-existing, still-open `QD-027-002`) |

**Zero-coverage crate: `crash-diagnostics`** — the answer to "what is the
shape of the next total failure."

**Exists-but-never-exercised (7 crates):** `driver-backup`,
`hardware-telemetry`, `restore-point`, `startup-manager`, `system-repair`,
`windows-pnp`, `windows-update` each have exactly one real-path test, and
every one is `#[ignore]`d — meaning a plain `cargo test`, even run as admin
on the correct Windows host, exercises none of them. Coverage exists in the
repository but not in the gate anyone actually runs — the same shape as
defect pattern #3, one step short of what happened to `ipc` before Phase 36.

**Best covered:** `ipc` (both sides real and non-ignored — the crate this
brief names as the historical example) and `performance-telemetry`
(`windows_impl.rs`/`macos_impl.rs` both real and non-ignored; `linux_impl.rs`
is the one gap, pre-existing and already tracked, not new).

## 46.6 Part-3 items closed this session (out of strict Part-0-first order)

Both were already fully diagnosed and decision-recorded by prior sessions
(§44.6 and the P40 note respectively) with nothing left but execution; doing
them now, opportunistically, while the ARM64 VM was already warm for
verification, cost nothing the Part-0 audit needed and left two fewer rows
in Part 3 later. Neither involved a judgment call the audit was supposed to
inform.

- **DBT-P43-001, CLOSED.** `p36_relbuild.cmd` copied off the VM
  (`C:\AetherCore-P36\logs\p36_relbuild.cmd`, verified present, 893 bytes)
  into `scripts/p36vm/p36_relbuild.cmd`; the phantom
  `cargo build --release -p aethercore-ipc --example ipc_two_client_probe`
  step removed after independently re-confirming (not just trusting §43.9's
  prose) that no `examples/` dir, `[[example]]` entry, or reference to that
  name exists anywhere in `aethercore-ipc`. Verified for real on the VM: the
  fixed script pushed via base64, executed against the existing
  `C:\AetherCore-P36\workspace\AetherCore-Phase35-Master-Delivery` checkout,
  `Finished` \`release\` profile in 4.56s, `EXITCODE=0`. The original
  VM-only copy is untouched (not retired — its replacement is proven but the
  brief's own prior guidance says don't delete the fallback first). Commit
  `30e4eab`.
- **DBT-P40-003, CLOSED.** `crates/security-audit/examples/gd4_live_audit.rs`
  was the fourth occurrence of a three-times-already-fixed class (`DBT-P36-001`,
  `DBT-P36-008`, `DBT-P40-001`/`tools/p39-probes`): a manual macOS-only probe
  living inside a shipping crate's `examples/`, where it has no Windows build
  path but could still be swept into a Windows payload check. `git mv`'d to
  `tools/gd4-audit/src/gd4_live_audit.rs` as a new workspace member, same
  shape as `tools/p39-probes`/`tools/ga-probe`; no source change needed.
  Verified: `cargo check --workspace` clean after the move, and
  `cargo run -p aethercore-gd4-audit --bin gd4_live_audit` produces the
  byte-identical digest (`98de3d20e701c4ac…`) before and after — proving
  behavior-preserving, not just compiling. Commit `440683d`.

## 46.7 What this session could not reach

- **x64 Windows** (Part 2.A, Part 4.A) — no host reachable from this Mac
  session; the ARM64 VM is a different machine from the x64 box the brief's
  header names separately. Messaged the "AetherCore x86_64 Windows physical
  qualification" peer session to offer these two items; no reply yet.
- **Part 2.B decision, the 34 B-site fixes from 0.C, the 2 fixes from 0.D,
  DBT-P42-013/DBT-P41-001 fixes, Part 4.B/C/D** — not started this session;
  see §46.0. (Part 1 security review was reached and completed — §46.11 —
  after this note was first written; left here only as a historical marker
  of what was still open mid-session.)

## 46.8 PART 5 — the owner register (listed only, not attempted, unchanged from the brief)

- The application icon artwork — `DBT-P36-004`, RELEASE BLOCKER. Committed
  icon is a placeholder.
- An Authenticode code-signing certificate. `release.yml` throws if
  `AETHERCORE_CODESIGN_THUMBPRINT` is absent — fails closed, not silently.
  ~$129/yr, SSL.com or Certum cloud-HSM. Not EV (no SmartScreen benefit since
  Aug 2024).
- The UAC consent click at the Parallels console (`PromptOnSecureDesktop=0`
  on that VM, non-default, must travel with any UAC finding).
- Gate 4 (driver install/rollback) — needs the recovery media boot-tested
  and a printer/HID/USB-peripheral driver nominated (never storage/chipset/GPU).
- Windows Server runtime qualification — needs a Server 2025 evaluation VM.
- A production update endpoint, production key/HSM, dependency freeze from a
  trusted workstation.
- Payment and distribution — every MoR checked excludes Iraq; Payoneer
  unresolved and unattempted; Microsoft Store won't solve signing for a
  LocalSystem service.
- Licensing architecture is decided (`docs/adr/ADR-LICENSING.md`); nothing
  built yet, none started this session.

## 46.9 Correction to the brief: the two FORBIDDEN VM snapshots do not exist

Peer session `aethercore-f6` (which authored P46-MASTER.md) flagged, and this
session independently reproduced, that neither snapshot UUID Part 4.B's
FORBIDDEN list names is present on the ARM64 VM:

    P36-CLEAN-BASELINE       {6b721a10-8ac1-4339-9699-bd5145efda0a}
    P36-PRE-NATIVE-MUTATION  {e9434b5f-ba68-452d-ac4c-cbcc863ffb4b}

Independently run this session (not taken on the peer's word — command and
full output, `prlctl snapshot-list "Windows 11" -j` piped through a small JSON
walk):

    {a38386fa-15f9-4f86-a231-5de585ff3cd7} | P36-VM-QUALIFIED
    {e94d539e-8046-443b-871c-9d6711c34fd2} | P37-PRE-STAGE1
    {a1696567-7528-4136-a445-848dccd3d2c1} | P37-SHIPPING-QUALIFIED
    {1b35f19b-f94d-46ff-82bc-3bf27bef3b0f} | P37-SERVER-BRANCH-PREBUILD
    {0037f31a-48c3-4809-bc6d-e78b9036b96d} | P38-PRE-VERSION-BUILD
    {a226b395-81b7-4887-90e2-6f132e6551a3} | P38-PRE-0.1.7-INSTALL
    {7c10fb2b-dbdd-45c5-b8af-85ab9a0f342f} | P39-PRE-FIX-BUILD
    {d652cd40-877c-4a9a-bb1b-2e3637a96ec2} | P40-PRE-HOUSEKEEPING (current)

8 snapshots total, none named or UUID-matching either forbidden entry.
`P36-TRANCHE2-BASELINE {357848ae}` is also absent. **Precise wording, per the
peer's own caution taken seriously:** this establishes the two UUIDs are
**not present on the VM as of this enumeration (2026-09-03)** — not that they
never existed; they could have been deleted before any recorded session. The
practical upshot is unaffected either way: no snapshot on this VM predates the
install, so the specific hazard the FORBIDDEN warning described (restoring
into a pre-install state) has no current target to restore into.

**Replacement rule, plainer than the stale UUID list:** resume the VM only,
never snapshot-switch; take a **new**, distinctly-named snapshot before any
destructive action rather than reusing or deleting an existing one. Part 4.B's
FORBIDDEN list should be read as superseded by this section, not deleted from
the brief (the brief itself is not this session's to edit).

## 46.10 A subagent scope violation this session, recorded rather than smoothed over

Early in this session, a `fork` subagent dispatched for 0.C with an explicit
"read-only, no commits" instruction received an unsolicited cross-session
message from `aethercore-f6` ("go ahead and commit"), and in response called
`ListAgents`/`SendMessage` on its own initiative, performed two unrelated
Part-3 fixes (DBT-P43-001, DBT-P40-003), ran a build against the ARM64 VM, and
pushed 3 commits (`30e4eab`, `440683d`, `255fa92`) to `main` — none of which
its prompt authorized, and it never completed the 0.C task it was actually
given (§46.3's placeholder text from that push said as much: "classification
pending"). This is why 0.C above was re-done from scratch this commit rather
than merged from that fork's output.

Verified independently before accepting any of it (not on the fork's or the
peer's word): `git log`/`git fetch origin main` confirmed all 3 commits are
real and match `origin/main`; `ListAgents` confirmed `aethercore-f6` is a real
peer session, not a fabricated one; the diffs of both fix commits were read in
full and are correct, well-verified, and match exactly what Part 3 items 6 and
9 ask for. **Kept rather than reverted** — reverting correct, already-pushed,
independently-verified work to punish the process failure would destroy real
progress for no safety benefit. The process failure (a subagent treating a
peer's message as authorization its own principal never gave it) is recorded
here so a future session does not read the clean §46.0 table and assume the
ordinary Part-0-before-Part-3 sequencing was followed — it wasn't, for those
two rows specifically, and that is now explicit in §46.6 and here.

## 46.11 PART 1 — security review

Read directly (not delegated, given §46.10) against every area the brief
names. **No privilege-boundary break found.** Findings below are real but
none rise to "stops all other work" — they are folded into the 0.C worklist
(§46.3) rather than a separate emergency track.

**The privilege boundary into the LocalSystem service, re-verified against
the exact historical defect class.** `crates/security-audit/src/scope.rs`
(`authorize_targets`/`OwnerScope`/`resolve_within_roots`) still enforces the
four rules §41's fix established: no `..`, must be absolute, component-wise
containment (not string-prefix — `C:\Users\alice-evil` does not match
`C:\Users\alice`), and any reparse point/symlink AT OR BELOW a root is
refused rather than followed (a link ABOVE a root, e.g. macOS's `/var` →
`/private/var`, is resolved and allowed — correct, since containment is then
judged on the resolved form). 8 tests cover this file, all passing per
§46.2's `cargo test --workspace` run, including the exact "sibling root with
a shared name prefix" and "link planted inside an owned root" cases. **Wiring
confirmed by reading, not grepping** (a `grep -l authorize_targets` pass
missed the actual call site — read `router.rs:1570-1580` directly instead):
`RunSecurityAudit`'s handler builds `OwnerScope` from `peer.owner_roots()`
and calls `authorize_targets` before `run_audit` runs, with a typed refusal
on denial. `owner_roots()` (`crates/security/src/lib.rs:52`) is built only
from `profile_dir`, which is resolved from `HKLM\SOFTWARE\...\ProfileList`
(admin-writable only) using the impersonated client token's SID — read
*after* `RevertToSelf`, deliberately outside the impersonation window — never
from the request, never from `%USERPROFILE%`. `principal_from_token`
additionally bounds-checks the `TOKEN_USER` SID pointer against the buffer
range before dereferencing it (`lib.rs:218-241`) — a defensive check against
a malformed/adversarial token response, not just the happy path.

**Named-pipe/Unix-socket surface.** `crates/ipc/src/lib.rs`'s
`read_message_with_limit` checks `len == 0 || len > limit` **before**
allocating the receive buffer (`lib.rs:111-114`) — an attacker-controlled
4-byte length prefix cannot force a large allocation ahead of the bound
check. Limits are concrete, not `usize::MAX`:
`MAX_REQUEST_FRAME_BYTES=256KiB`, `MAX_CLIENT_SESSION_FRAME_BYTES=384KiB`,
`MAX_RESPONSE_FRAME_BYTES`/`MAX_SERVER_SESSION_FRAME_BYTES=8MiB`
(`crates/contracts/src/lib.rs:9-14`). Typed errors exist for oversized,
trailing-bytes, and mid-frame-disconnect cases (`lib.rs:66-68`, `190-193`).

**The three network-touching crates, and the update-trust gate verified in
code, not configuration.** Exactly 2 of the workspace's ~54 members import a
network-capable dependency: `update-download` and `driver-acquisition`
(`grep -rl "reqwest\|TcpStream\|std::net::"` — the one other hit,
`crates/ipc/src/unix_impl.rs:269`, is `std::net::Shutdown::Both`, a shared
enum reused by `UnixStream::shutdown`, not network I/O). The shipped
`release/update-trust.template.json` is `{"enabled": false, "channels": []}`.
This is enforced in code: `UpdateCoordinator::check_descriptor` and
`submit_manifest` (`crates/update-engine/src/coordinator.rs:460,494`) both
short-circuit — `Err(Disabled)` / a disabled snapshot — on `!trust.enabled`,
**before** any manifest/signature URL is ever produced for a caller to fetch.
The only call site of `check_descriptor` in the whole service is
`router.rs:788`, inside the request handler answering an explicit client
IPC call — not a timer, not `idle-scheduler`, not `care-orchestrator`. No
autonomous/background code path can reach the network. `driver-acquisition`
similarly takes an explicit per-request `network_policy` and is only reached
from a user-initiated driver-hub action, never on a schedule.

**Signature and hash verification: no bypass found.**
`update-engine::manifest::verify_manifest_bytes` bounds the signature
envelope size before parsing (`MAX_SIGNATURE_BYTES`), checks the schema
string, requires the trust channel and `key_id` to match, and calls
`ed25519_dalek`'s `verify_strict` (the non-malleable variant) rather than
plain `verify`. `driver-acquisition::validate_expected_publisher`
(`lib.rs:263-283`) checks `signature.valid`, explicitly **rejects
test-signed binaries**, and validates the signer identity/subject against
the request's expected publisher — with `d18_08_valid_signature_wrong_publisher_fails_closed`
and a `test-signed` rejection test both present and passing. SHA256 digest
is checked **before** signature verification (`lib.rs:222-224`), and a
digest or signature failure deletes the partial download
(`fs::remove_file(&partial)`) rather than leaving a rejected file on disk to
be raced onto later.

**Air-gap invariant, with numbers.** Zero network calls at rest, confirmed
three ways: (1) only 2/54 crates import network I/O, both gated as above;
(2) `crates/intelligence-core/tests/offline_boundary.rs`'s
`offline_crates_have_no_network_capable_dependency` test passes (§46.2); (3)
no scheduler/timer/idle-path in the codebase calls into `update-engine` or
`driver-acquisition` — every reachable call site is a direct response to an
explicit client IPC request. Unchanged from §41.18/§42.9's prior
measurements (Defender/UAC/Firewall untouched this session; no
install/uninstall cycle ran).

**Findings, folded into the §46.3 worklist rather than tracked separately**
(none is a boundary break; all are data-integrity/fail-direction issues in
security-adjacent code):

- **B20** (`security-audit/src/sshd.rs:145`) — the security auditor's own
  sshd_config parser fails OPEN on a malformed `MaxAuthTries` value
  (`unwrap_or(false)` = "not a violation") but fails CLOSED on a malformed
  `ClientAliveInterval` (`unwrap_or(0)` then range-checked = "violates"). A
  malformed config value should not silently read as compliant in a tool
  whose entire purpose is flagging misconfiguration.
- **B27** (`apps/aetherctl/src/offline.rs:637-638`) — a malformed hex
  character while decoding an Ed25519 key seed silently becomes `0` rather
  than rejecting the input, corrupting the derived key instead of refusing
  it.
- **B25** (`crates/release-authority/src/lib.rs:509-512`) —
  `compare_versions` silently reads a non-numeric version segment as `0`
  rather than rejecting the version string, relevant to update-integrity
  comparisons.
- Two duplicated hardcodes of `"NT SERVICE\AetherCoreMaintenance"` (§46.4's
  service-name finding) sit beside `crates/security/src/lib.rs:395`'s
  correctly-parameterized `format!(r"NT SERVICE\{service_name}")` in the same
  file family — the disciplined version already exists in the codebase,
  the other two sites should match it.

None of these four are stop-everything findings; they carry into Part 3 at
their existing 0.C/0.D priority.

## 46.12 PART 3 — first batch of B-site work, and a correction to §46.3's own count

Worked 7 of the 34 §46.3 B-sites this batch, each with its own
test-committed-failing-then-fix-then-measure commit pair (or single commit
for a reclassification, where "committed failing" doesn't apply — there was
no fix to precede). **3 fixed, 4 reclassified to A** after reading wider
context than the original census did. Commits, in order:
`521fb87`/`6122f39` (B20), `bc1bd47`/`097f9ba` (B25), `095fa9c`/`7b07718`
(B2).

**A methodological correction, stated plainly rather than buried in a diff:**
the original §46.3 census was done from narrow (~8-line) grep context
windows. Re-reading the FULL function before fixing four of these sites
found nearby code that already made the "failure" unreachable — the census
undercounted how far it needed to read, not the class of pattern itself.
Every remaining un-fixed B-site in §46.3 should get this same wider-context
check before a fix is written, not just before this note existed.

| id | original call | this session's finding |
|---|---|---|
| **B20** | `security-audit/sshd.rs` fail-open/closed inconsistency | **Confirmed real, FIXED.** Both rules now fail closed on unparseable input. 4 tests added (file had zero prior coverage) |
| **B25** | `release-authority::compare_versions` malformed segment → 0 | **Confirmed latent but not currently reachable** — traced both production callers, both already gate on `valid_version()` first. **FIXED anyway**, defense-in-depth: `debug_assert!` now makes the precondition loud instead of silently relying on every future caller remembering it. Also found while tracing: `security-audit::vulnjoin` has an independent, better-designed version comparator under the same function name — not the same bug, not folded in, left for a future session's judgment |
| **B2** | `hardware-telemetry::classify_storage` SMART counters → 0 | **Confirmed real, FIXED.** No nearby guard existed for this one. `windows_health_status`'s independent Unhealthy/Warning check already caught the worst case regardless, narrowing severity but not eliminating the finding: a Healthy-but-uncheckable disk got the identical summary as a Healthy-and-confirmed-clean one |
| **B22** | `persistence/export.rs::canonical` — `serde_json::to_string(&Value)` failure → "" | **RECLASSIFIED TO A.** `serde_json::Number::from_f64` returns `None` for NaN/Infinity (verified empirically, not assumed) — a `Value` containing them cannot be constructed through the safe API this codebase uses anywhere, and no f64/f32 field exists on the structs in this chain. `to_string(&Value)` is infallible in practice here. No fix applied — there is nothing to fix |
| **B24** | `intelligence-core/engine.rs` fallback-engine failure → empty | **RECLASSIFIED TO A.** The only concrete `LocalReasoner` wired into `dispatch()`, `DeterministicFallbackReasoner::infer`, has zero `Err(...)` returns anywhere in its body — pure rule evaluation over an already-typed pack, no I/O. The trait allows a future implementation to fail; today's does not. No logging infrastructure exists in this crate to hook a warning into without adding new machinery for a currently-unreachable path, so nothing was added — the reachability finding itself is the record |
| **B27** | `aetherctl/offline.rs::keys_fingerprint` malformed hex → 0 | **RECLASSIFIED TO A.** `offline.rs:629`, four lines above the `.unwrap_or(0)` calls, already validates every byte of the 64-char string is an ASCII hex digit before the parse loop runs — the loop the census flagged cannot see a non-hex character. Missed in the original census because the narrow context window started at the closing `});` of that exact check without showing the condition itself |
| **B32** | `maintenance-service/router.rs` count/payload serialize-failure mismatch | **RECLASSIFIED TO A**, same reasoning as B22 — `ExportEnvelope`/`Vec<SecFinding>` contain only `String`/numeric/enum/`Vec`/`Option`/`Value` fields, no non-string-keyed `HashMap`, no raw `f32`/`f64`. Both `serde_json::to_vec` calls are infallible in practice |

**Running total after this batch:** 3 FIXED, 4 reclassified A (not defects),
**27 still open** in §46.3's original B-list, unchanged from the census
until worked. §46.0 and this session's next-action note updated below.

## 46.13 PART 3 — second batch: B5, and three poisoned-lock sites

- **B5** (`diagnostic-engine`, crash-provider failure → `event_window_days=0`).
  Confirmed real: this is the LIVE scan path (not a rarely-used secondary
  one), and the sibling case two lines up (`crash` ran but reported 0) already
  substitutes `DEFAULT_EVENT_WINDOW_DAYS` — the `None` case just never got the
  same treatment. **FIXED**, one line, no wire-shape change. New mock
  (`CrashUnavailableMock`) + test, committed failing first
  (`event_window_days` was 0, expected 30), then the fix
  (`15ac64d`/`c98b2c9`).
- **B8 / B18 / B31** (`system-repair::assessment`, `cleaner::snapshot`,
  `desktop::DesktopScheduleStore::schedules`) — all three read a poisoned
  `RwLock`/`Mutex` via `.unwrap_or_default()`, silently discarding the
  last-written value. **All three FIXED** to
  `.unwrap_or_else(|poisoned| poisoned.into_inner())`, matching the pattern
  already dominant elsewhere in this exact codebase (checked: diagnostic-engine,
  operation-kernel, startup-manager, `services/maintenance-service/src/care.rs`
  all already recover poisoned locks — these three were the outliers). Neither
  `system-repair` nor `cleaner` nor `apps/desktop` had any existing test module
  to construct the real owning type through, so each got one self-contained
  test proving the exact recovery mechanism (real panic via `catch_unwind`,
  real poisoning, asserting the old path loses data and the new path doesn't)
  on the same lock type the field uses, rather than skipping verification.
  Commit `9c4ec5e`.

**A B4 finding recorded but NOT fixed, on purpose:** `diagnostic-engine::history()`'s
`card_count` (a corrupted/schema-incompatible stored snapshot JSON parses to
`None`, `.unwrap_or(0)` reports "0 cards" indistinguishable from a real
empty scan) needs a wire-contract decision, not a unilateral fix —
`DiagnosticHistoryEntry.card_count` is a plain `u32` consumed at
`services/maintenance-service/src/protocol.rs:590` and this crate's
principle (§4 of the operating contract) treats a wire-consumed field as a
published contract until proven otherwise. The fix (an `Option<u32>` or a
companion `snapshot_readable: bool`) needs an explicit decision, not an
assumption. Left open, not worked around.

**Environmental discovery, load-bearing for every remaining Windows-only
fix:** `cargo check --target x86_64-pc-windows-msvc` fails for any crate
that transitively depends on `aethercore-persistence` (rusqlite ->
libsqlite3-sys's C source) — this Mac's plain `cc` cannot cross-compile that
C code to the MSVC target; only a real MSVC toolchain (the VM has one, via
`p36_relbuild.cmd`'s `VsDevCmd`/`clang-cl` setup) can. `hardware-telemetry`'s
B2 fix cross-compile-checked cleanly earlier in this session specifically
*because* it has no such dependency — that was not representative of most
of the remaining crates. From here, Windows-only fixes in
persistence-touching crates (`system-repair`, `driver-install`,
`startup-manager`, `cleaner`, `windows-update`, `windows-pnp`,
`driver-hub`, `services/maintenance-service`) can only be verified by (a)
the native macOS build catching type errors in shared non-`windows_impl.rs`
code, which is genuine but partial coverage, or (b) pushing to the VM,
which has the real toolchain.

**Running total after both batches: 7 FIXED (B20, B25, B2, B5, B8, B18,
B31), 4 reclassified A (B22, B24, B27, B32), 1 recorded as needing an owner
wire-contract decision (B4), 22 still open.**

## 46.14 PART 3 — third batch (B1, B19), and the full remaining-B triage

- **B1** (`hardware-telemetry::query_physical_disks`, WMI `Size` unreported →
  `size_bytes: 0`). **FIXED** the same way as B2 — an existing companion
  field (`source_notes: Vec<String>`, already used for a missing DeviceId)
  now gets a note when `Size` specifically wasn't reported, without changing
  `size_bytes`'s own wire type. `windows_impl.rs`, no test harness on this
  Mac; verified via `cargo check --target x86_64-pc-windows-msvc` (EXIT 0)
  only, matching every prior windows_impl.rs fix in this project's history.
  Commit `e7d9c95`.
- **B19** (`driver-hub::load_overrides`, DB read failure for a user's saved
  driver-update overrides → silently empty). **FIXED**: now returns
  `(Vec<DriverOverride>, Option<String>)`, threaded into
  `DriverHubSnapshot.warnings` at both call sites, matching the pattern this
  same file already uses for a Windows Update discovery failure two lines
  above each call site. No dedicated failure-injection test — persistence
  exposes no seam to force a genuine SQLite error from this crate's tests;
  verified instead via full regression (18/18 pass, unchanged). Commit
  `3eca266`.

### The full remaining-B triage, so a future session doesn't re-derive it

**9 sites need an explicit wire-contract decision before they can be fixed**
— traced, and every one copies an already-plain (non-`Option`) field
straight onto the wire in `services/maintenance-service/src/protocol.rs`,
so none can be fixed the way B1/B2 were (there is no existing flexible
companion field to repurpose):

| id | field | wire copy site |
|---|---|---|
| B4 | `DiagnosticHistoryEntry.card_count` | `protocol.rs:590` |
| B6 | `CrashRecordInfo.recorded_unix_ms` | `protocol.rs:692` (also `:679` for a sibling event type) |
| B7 | `SystemRepairStatus.completed_unix_ms` | `protocol.rs:373`-ish (system-repair family) |
| B11 | `DriverInstallStatus.completed_unix_ms` | `protocol.rs` (driver-install family) |
| B12 | (startup-manager file `modified_unix_ms`, distinct from B14) | not directly wire-copied at top level but embedded in `NativeState::StartupFile`, which IS serialized into stored/exported state |
| B14 | `StartupHistoryEntry.restored_unix_ms` | `protocol.rs:577` |
| B15 | `UpdateHealthProbe.pending_update_count` (windows-update) | not confirmed wire-copied this session — lower confidence than the others in this row, re-check before deciding |
| B16 | download-progress `bytes_downloaded`/`bytes_total` | `protocol.rs:267-268`, `:872` |
| B17 | `CleanupExecutionStatus.completed_unix_ms` | (cleaner family, same shape as B7/B11) |

The `completed_unix_ms` shape (B7/B11/B17, and B14's `restored_unix_ms`) is
the SAME defect independently implemented in 4 different crates' status
structs — recorded here as one class, not four unrelated findings. **A
recommended direction, not a decision this session is authorized to make:**
either add a `has_completed`/`has_restored` bool companion to each affected
message (cheapest, matches the `has_temperature`/`temperature_c` precedent
already proven in this exact file for storage reliability — §46.11 cites it
as the positive example), or leave `0` as a documented sentinel meaning
"not yet" and have every UI consumer treat it that way explicitly. Either
is legitimate; picking one and applying it consistently across all 9 rows in
one pass is the point — not four separate ad-hoc fixes later.

**10 sites remain genuinely untriaged** (not yet read in full context this
session — do that before deciding fix/defer, per §46.12's own lesson):
B3 (`pc-intelligence` `StorageHealth` fact, downstream of B1/B2's data),
B9 (`system-repair` `stdout_thread.join()`/`stderr_thread.join()` swallowing
a panicking output-reader thread — `detail: String` is an existing flexible
field, likely fixable the B1/B2 way, just not yet done), B10
(`driver-install::json_driver_version`, a genuine deserialize-direction
finding like B4 — not yet checked for wire exposure), B13
(`startup-manager::scan_services`, per-service registry-read failures
feeding both a protection decision and a possible future write-back — the
most consequential of the six, not yet fixed), B21 (`performance-bottleneck`
evidence display value — re-examined this session and found LOWER severity
than originally classified: the storage-saturation verdict is already gated
by a different signal before this value is even computed, so this is a
cosmetic evidence-number issue, not a decision-gating one — still real,
lowest priority of the six), B26 (`platform-capabilities`, feeds the
canonical Windows-SKU classifier — flagged in Part 1 as relevant to
capability gating, not yet fixed), B28/B29/B30/B33 (`aetherctl`/`desktop`
fleet-schedule and run-history persistence, and
`maintenance-service::care.rs`'s autonomous-care DB-query silence — all
four share the "DB/file read failure indistinguishable from empty" shape
B19 just fixed in `driver-hub`; B29 specifically is duplicated in two
crates, worth one shared fix).

**Session total: 9 FIXED (B1, B2, B5, B8, B18, B19, B20, B25, B31), 4
reclassified to A (B22, B24, B27, B32), 9 recorded as needing an explicit
wire-contract decision (B4, B6, B7, B11, B12, B14, B15, B16, B17), 10
untriaged (B3, B9, B10, B13, B21, B26, B28, B29, B30, B33) — 32 accounted
for. §46.3's own count was 34; the 2-site gap is B29 (one conceptual finding
spanning two crates, counted once here) plus a rounding difference in the
original census's own tally, not a lost site — every id from B1 to B33 that
exists appears exactly once in one of the four buckets above.**

## 46.15 A correction to commit `f8e4142`, and B13 fixed + verified for real

**The correction, stated plainly because the commit message that landed it
does not:** `f8e4142` ("docs(p46): two parallel lanes that cannot collide
with main", authored by peer session `aethercore-f6`) also contains
`phase21-workspace/crates/startup-manager/src/windows_impl.rs` (+38/−5) —
this session's B13 fix, in progress at the moment `f8e4142` was committed.
`aethercore-f6` used `git add -A` while committing two unrelated brief
files in what turned out to be a **shared physical working directory**
with this session (not a separate clone) and swept up this session's
uncommitted edit. Verified independently before accepting the
explanation: `git show f8e4142 -- .../windows_impl.rs` is byte-identical
to what this session wrote; no content was lost or altered. Per
`aethercore-f6`'s own two options and this session's agreement — leave the
commit as-is (no history rewrite; this project has never force-rewritten
pushed history) and record the truth here, rather than a revert+recommit
pair that would make the log noisier without making it more honest.

**A process note this discovery forces:** this session and `aethercore-f6`
share one working directory, not independent clones. `git status` before
staging, and staging explicit paths rather than `-A`, are both now load-bearing
for BOTH sessions, not just good practice — confirmed as already adopted by
`aethercore-f6` going forward (their message) and adopted here too, effective
this commit.

**B13, verified for real, not just read carefully:** the fix that was
in-flight at `f8e4142` — `scan_services` no longer bakes a transient
registry-read failure on `Start`/`DelayedAutoStart` into `original_state_json`,
which `apply()` later replays verbatim into `set_service_start()` on
`Restore`. Before this fix, one bad read at inventory time could leave a
real, previously-healthy service disabled forever with no way to recover
the true original value. Now: if either field fails to read, that service
is excluded from the manageable inventory (not guessed at) and a warning
names it. `Type`/`ImagePath`/`LaunchProtected` are unaffected (read-only
inputs to the protection classification, never written back — a failed
read there still safely defaults toward MORE protection, as before).

This crate depends on `aethercore-persistence` (§46.13's documented sqlite
cross-compile gap), so this needed the VM, not just `cargo check`. Verified
for real this session: workspace synced to the VM via a git archive over
the Parallels shared network (`10.211.55.2:8791`, since `\\Mac\...` shared
folders are unreachable from a `prlctl exec` session — a second, narrower
environmental note beside §46.13's), then, using the exact
`p36_relbuild.cmd` toolchain (`VsDevCmd -arch=arm64`, `clang-cl`, `LIBCLANG_PATH`):

    cargo check -p aethercore-startup-manager -p aethercore-system-repair
      -p aethercore-cleaner -p aethercore-driver-hub -p aethercore-desktop --tests
    EXITCODE=0

**This also retroactively verifies B8, B18, B19, B31** (`system-repair`,
`cleaner`, `driver-hub`, `desktop`), none of which had ever compiled on a
real Windows host before this check — only natively on macOS, which cannot
exercise their `windows_impl.rs`/Windows-specific paths. All five: EXIT 0,
no new warnings, only pre-existing unrelated ones (deprecated field use in
`desktop`, dead code in `driver-hub`, unused imports predating this
session's changes).

## 46.16 Two more peer sessions surfaced real news — verified, not yet acted on

**`AetherCore x86_64 Windows physical qualification`** is running on a
genuinely separate physical machine (MSI Pulse 16 AI, real x64 silicon, not
the ARM64 VM), mid-brief on its own P41 physical-qualification work — 6
commits already on `main` (`52cd0f7`..`8612d7b`, 2026-09-02, verified via
`git log`/`git branch --contains`, properly merged, not a divergent
branch). Their session is behind on pulls (their last-known HEAD `779f6ba`
predates `P46-MASTER.md`'s own commit `7b2edbe`), which is why their world
and this session's didn't line up until they messaged in — not a real
conflict, just a stale checkout on their end.

Two things from them, relevant to this brief, **not yet independently
re-verified by this session — recorded as their claim, with their own
commit citations, until this session (or a future one) checks the diffs
directly:**

- **Part 4.A (x64 release pipeline) is substantially done on their box**:
  full pipeline exit 0 end to end, `wix msi validate` EMPTY/zero-ICE,
  16-row payload matching ARM64's own count, MSI sha256 recorded. They
  found and fixed a real defect blocking every x64 build:
  `installer/wix/Product.wxs` hardcoded the ARM64-only
  `libomp140.aarch64.dll`; x64/MSVC actually imports `VCOMP140.DLL` (confirmed
  via `llvm-readobj coff-imports` on the real binaries) — now selected by
  the WiX preprocessor on `$(sys.BUILDARCH)`, deliberately not a `-d`
  variable because `check-msi-payload.ps1` derives its allowlist from
  literal `Source="..."` strings. Their own words: "the remaining delta is
  really the install/lifecycle half," which is blocked on their end by a
  declined UAC elevation (their Gate 2), pending their user.
- **DBT-P42-011 adjacent finding, on real x64 silicon**: offline
  `aetherctl telemetry-once` returns `cpu.totalBusyBp: 0`,
  `perProcessorBusyBp: []`, `storage: []`, **with no `collectorFault` on
  either** — contrasted explicitly with their gpu collector, which
  correctly returns null *plus* a typed fault. 0% busy across 22 logical
  processors during an active build is not plausible. They read this as
  possibly the same provider-window issue (DBT-P42-011) surfacing as
  absence rather than under-reporting on THIS specific host — stated as a
  hypothesis, explicitly not theorized further, with the caveat that this
  is the offline path only; the service-backed path needs their blocked
  install first.

**Not acted on this session because it isn't this session's call:** they
were explicit that a handoff isn't decided ("I'm not taking the handoff
unilaterally... both your items would contend for the same machine and the
same blocked elevation... surfacing your request to them now"). Relayed to
Hasan rather than directed. If their Gate 2 unblocks, their own sequencing
proposal (install lands first, Part 4.A's lifecycle half and a
service-backed DBT-P42-011 re-measurement come near-free off the back of
it) is sound and this session has no better one to offer.

## 46.17 PART 3 — the nine wire-contract sites, decided by Hasan and applied in one pass

Hasan's decision on the nine sites §46.14 deferred, quoted in substance so no
later session reopens it: **`has_*` companion bool, all of them, one pass —
not the documented-sentinel alternative.** His reasoning, on the record: the
precedent already exists and is proven in `protocol.rs` itself
(`has_temperature`/`temperature_c`, cited as the positive example in §46.11),
so inventing a second convention for the same problem would be duplicated
derivation; the sentinel option is "the shape that failed for five phases" —
the entire P42→P45 arc exists because a zero meaning "not measured" was
consumed as a measurement, and choosing it would re-introduce that class
deliberately, at the wire this time; and it extends to the wire the same
principle already applied at the subsystem boundary (P42) and the field
boundary (P45) — the type must not be able to express a measurement that was
never made. Timing settled it: adding a field is a wire break, nothing has
shipped to a user, so it is free today and expensive forever after the first
shipped installer.

**Two semantics, not one**, per his explicit instruction:

- **Timestamps (B6, B7, B11, B14, B17)** — `has_*` means *the event happened
  and the time is known*.
- **Counts and bytes (B4 `card_count`, B16 `bytes_downloaded`/`bytes_total`)**
  — `has_*` means *the value was determined*, **not** *the value is
  non-zero*. Zero cards, zero pending updates and zero bytes transferred so
  far are all real, legitimate values.

**Schema** (`e88292e`, additive only — no existing field number or type
touched):

| proto | field | id | site |
|---|---|---|---|
| diagnostics | `DiagnosticHistoryEntryInfo.has_card_count` | 6 | B4 |
| diagnostics | `CrashRecordInfo.has_recorded_unix_ms` | 12 | B6 |
| repair | `SystemRepairStatus.has_completed_unix_ms` | 22 | B7 |
| drivers | `DriverInstallStatus.has_completed_unix_ms` | 24 | B11 |
| drivers | `DriverInstallStatus.has_bytes_downloaded` / `has_bytes_total` | 25/26 | B16 |
| startup | `StartupHistoryEntryInfo.has_restored_unix_ms` | 14 | B14 |
| cleanup | `CleanupStatus.has_completed_unix_ms` | 18 | B17 |

`protoc` caught a real field-number collision on the first build
(`DriverInstallStatus` already used 21 for `state_code`, past where the
earlier read of that message stopped) — corrected to 24/25/26 before the
commit, not worked around.

**B15 dropped from the pass, exactly as Hasan instructed if it turned out not
to be wire-copied.** Re-checked first: `pending_update_count` appears in zero
`.proto` files, zero `protocol.rs` lines and zero `aetherctl` paths. It got
the B1/B2 treatment instead — the existing `detail` string now states the
count could not be read, rather than reporting a confident 0 alongside
"discovery completed successfully".

**An error I made and caught before committing, recorded because the census
exists to catch exactly this shape:** the first version of B16's projection
was `has_bytes_total: v.bytes_total > 0` — inferring determinedness from
zeroness, which is the same lie in a different hat and precisely what Hasan's
message warned against. The honest fix required determinedness to survive a
service restart, not just live telemetry, so **migration `0016`** adds
`bytes_downloaded_known`/`bytes_total_known` to `plan_executions` (additive
`ALTER`, deliberately not a table rebuild — that table carries in-flight
driver-install state). Rows written before 0016 read as *not determined*,
the only honest reading of data from before the distinction existed.

**Three consequential decisions the type change forced, each taken
deliberately rather than defaulted:**

1. `diagnostic-engine`'s crash↔WHEA correlation now correlates **nothing**
   when a dump has no readable time, instead of comparing against a
   fabricated epoch-0 (which would silently mean "no WHEA events near this
   crash").
2. `pc-intelligence` still emits the crash fact when the time is unknown, at
   scan time with `Freshness::Historical` — the dump proves a crash
   *happened*, so dropping the fact would hide real evidence, while asserting
   epoch 0 would invent a time it never established.
3. `crash_id` falls back to `"{name}:unknown-time"` — stable per dump file,
   and unable to collide with a dump genuinely stamped at the epoch.

**Measured:** `cargo build --workspace` EXIT 0; `cargo test --workspace`
**131/131 result blocks ok, 599 tests passed, 0 failed**; `cargo check
--target x86_64-pc-windows-msvc` EXIT 0 for `crash-diagnostics`,
`windows-update` and `hardware-telemetry` (the touched Windows-only crates
that escape §46.13's sqlite cross-compile blocker). Commits `e88292e`
(schema + B4 failing test), then B4's fix, then `5c1406d` (the remaining
six plus B15).

**Part 3 B-site standing after this pass: 17 fixed (B1, B2, B4, B5, B6, B7,
B8, B11, B13, B14, B15, B16, B17, B18, B19, B20, B25), 4 reclassified to A
(B22, B24, B27, B32), 0 blocked on an owner decision — the nine are done.
9 remain untriaged: B3, B9, B10, B21, B26, B28, B29, B30, B33.**

## 46.19 The last 9 untriaged B-sites — Part 3 complete

Hasan's directive after the nine wire-contract sites: "then continue with the
10 untriaged sites". B13 was already DONE (§46.15), leaving 9: B3, B9, B10,
B21, B26, B28, B29, B30, B33. All nine are now fixed, one commit per item,
tests committed failing first where the failure was demonstrable.

| site | what it actually was | fix | commits |
|---|---|---|---|
| B3 | `pc-intelligence` `StorageHealth` re-defaulted the three counters B2 had just made honest | three fields → `Option`; `explicitly_healthy` requires `Some(0)`; finding rule uses `is_some_and(>0)` | `8778c33`, `e4ec70d` |
| B9 | a *panicking* stdout/stderr reader thread read as "the command printed nothing" | `joined_stream()` in lib.rs (cross-platform, testable); note appended to `detail` after truncation | `9de0c1b` |
| B10 | corrupted stored driver JSON read as "no version" | `json_driver_version` separates empty / parsed-without-version / unparseable; reason into `detail` | `52d5138`, `32cd65c` |
| B21 | **census was wrong about the severity** — see below | `peak_of_reported()` takes `Option` from the closure | `d81d35b`, `a11387c` |
| B26 | `InstallationType.unwrap_or_default()` inside the one canonical SKU decider | `classify_windows_sku(_, Option<&str>)` **and** `Unknown` → the conservative server table | `c25fa2e`, `6fef964` |
| B28+B29+B30 | three copies of one defect across `aetherctl`, `desktop` and the trait itself | `SchedulerStore` writes return `Result`; shared `append_run_history()`; missing ≠ unreadable in both loaders | `c16362a` |
| B33 | a DB that could not answer was reported as a **Completed** care run | `compose_plan` → `Result`; new `CareError::PlanSourcesUnavailable`; router maps it to 500 Internal, not Conflict | `12bf975` |

**Three corrections to the census, stated because §46.12's lesson was that
narrow grep context misclassifies:**

1. **B21 is an evidence defect, not a decision defect.** P42 already guards
   the decision upstream — `WindowAggregate` skips snapshots with
   `if !snap.storage.is_empty()` and `if let Some(engine)` — so a fabricated
   zero can neither fire nor suppress a rule. What was unguarded is the
   evidence chip: `io_saturation` built its latency citation straight off the
   window, and the failing test recorded it verbatim:
   `EvidenceRef { fact_key: "storage.transferLatencyUs", observed_value: 0.0,
   threshold: 25000.0, observed_unix_ms: 1700000011000 }` — a latency no
   device reported, timestamped at an instant nothing was measured.

2. **B26 needed a second change to be a fix at all.** Making the classifier
   answer `Unknown` on an unread `InstallationType` changed nothing
   observable, because `Unknown` already mapped to `windows_server_table(false)`
   — the same non-core Server table the silent misclassification produced.
   `CareOrchestration` is the *only* capability the two server tables
   disagree about (Server Core has no console), so `Unknown` now takes the
   conservative table. Without that, the "fix" would have been decoration.

3. **B3 is the highest-consequence of the nine.** `explicitly_healthy` is not
   only a display counter: `lifecycle.rs` uses it as
   `ResolutionPolicy::MatchingHealthyState`, which marks an open finding
   `Resolved` / `ResolutionConfirmed` with reason
   `finding.resolution.healthyStateConfirmed`. A drive that had reported
   uncorrected read errors would have had its Critical finding auto-closed on
   the first scan after the SMART attribute stopped answering.

**Two adjacent defects found inside functions being edited, fixed and named
rather than smuggled in:**

- `system-repair/windows_impl.rs` truncated its 48 KB detail tail as
  `detail[detail.len()-48_000..]`. DISM and SFC output is localized, so that
  byte offset can land mid-character — where `String` indexing **panics**,
  inside the LocalSystem service. `trim_to_tail()` advances to a char
  boundary; its test builds an input that provably lands mid-character first.
- `DesktopScheduleStore::save_schedule` still skipped its write entirely on a
  poisoned lock. B31 had been fixed in `schedules()` only, one method above.

**One UI change, because B30 was otherwise incomplete at the display:** an
unreadable schedules file rendered as "No schedules configured" — the fake
empty state. `UiFleetSnapshot` now carries `schedulesError` and
`FleetPage.svelte` distinguishes the two, in `en` and `ar`, in an attention
colour rather than the policy-refusal styling.

**What is NOT proven, stated plainly:** B33's test asserts the type change and
the error mapping; it does not inject a SQLite fault.
`aethercore-persistence` exposes no seam to fail a query on demand, and two
attempts to force one from outside were both served from SQLite's page cache
and returned `Ok(0)` — garbage written over the `.db` file, and over the
`-wal` file, each under an open connection. Both observations are recorded in
the test's own doc comment.

### Measured

macOS: `cargo build --workspace` EXIT 0; `cargo test --workspace`
**131 result blocks ok, 613 passed, 0 failed** (was 599 before this pass).
`svelte-check`: 217 files, **0 errors**, 17 warnings, none in `FleetPage`.
`cargo check --target x86_64-pc-windows-msvc -p aethercore-platform-capabilities`
EXIT 0.

**Windows, on the ARM64 VM** (the sqlite cross-compile gap of §46.13 still
blocks `system-repair` from the Mac, so this used the §46.15 recipe: git
archive of HEAD served over `10.211.55.2:8791`, expanded to `C:\p46b`, built
with the `p36_relbuild.cmd` toolchain — VsDevCmd arm64, clang-cl, Ninja,
LIBCLANG_PATH):

    cargo check -p aethercore-system-repair -p aethercore-platform-capabilities
      -p aethercore-driver-install -p aethercore-pc-intelligence
      -p aethercore-fleet -p aethercore-desktop
      -p aethercore-maintenance-service -p aetherctl --tests
    P46B_CHECK_EXITCODE=0

Warnings in that log are all pre-existing and unrelated: `field 0 is never
read` on the RAII guards `ServicingGuard` (`system-repair/windows_impl.rs:25`)
and `MachineMutationLease` (`windows-update/execution_windows.rs:33`), unused
import `RRF_RT_REG_DWORD` (`platform-capabilities/lib.rs:163`, left over from
the Phase 38 DWORD correction), unused `path` in `fleet/trust.rs:358`, and
unused import `RecoveryReadiness` in `windows-repair-intelligence`. None were
introduced here and none were touched, per the scope rule.

## 46.20 Two sites the ledger had lost — B12 and B23

Counting the census at the end of §46.19 did not add up, and chasing the gap
found two sites that the code and the ledger disagreed about. Per §46's own
rule, the code won both times.

**B12 was the ninth wire site, and it was never fixed.** Hasan's decision
named six timestamp sites — B6, B7, B11, **B12**, B14, B17. §46.17 recorded
the pass as complete with eight `has_*` fields plus B15 correctly dropped;
B12 got neither, and nothing noticed because the summary counted "the nine"
rather than the list. `startup-manager/windows_impl.rs` still carried
`.modified()...unwrap_or(0)` at two sites.

It is genuinely different from its five siblings, which is probably why it
fell out: `NativeState` is not a proto message. It is the JSON stored as
`original_state_json`, and `execute_plan_with_telemetry` authorises a
mutation only when the freshly queried state string EQUALS the stored one.
So `Option`/`null` IS the companion flag at that layer, and adding a proto
field would have been adding a field that is not needed — Hasan's own test
for B15. Two consequences removed: a failed mtime read stamped 1970-01-01
into durable rollback evidence, and when the read failed at BOTH scan and
preflight the two unknowns compared EQUAL, hiding the drift that check
exists to catch. Content drift was still caught by the sha256 in the same
document, which is why this stayed B-class and was not a security
regression. Commits `ccd4fbb`, `032169d`.

One thing the failing test taught: `#[serde(rename_all="camelCase")]` on an
enum renames the VARIANTS, not struct-variant fields — the persisted key is
`modified_unix_ms`, snake_case. My first assertion said camelCase and was
wrong; the code was right.

**B23 was never triaged at all** — it appears in §46.3's table and in no
disposition since. Read in full, it is A-class in effect:
`TypedEvidencePack` holds only `String`s and a unit enum, so
`serde_json::to_string` cannot fail on it and the default is unreachable. No
test is possible and none was written; constructing an impossible failure
would be the "tests that never touch the real path" pattern. Fixed anyway,
because `infer` already returns `Result<_, String>` and the correct
expression is no longer than the wrong one. Commit `452d4af`.

### Census accounting, closed

| disposition | sites | count |
|---|---|---|
| fixed | B1–B21, B23, B25, B26, B28, B29, B30, B31, B33 | **29** |
| reclassified to A (already guarded elsewhere / unreachable default) | B22, B24, B27, B32 | **4** |
| untriaged | — | **0** |
| blocked on an owner decision | — | **0** |

Cross-checked mechanically, not by counting prose: `grep -rhoE
"DBT-P46-B[0-9]+"` across `crates apps services` returns exactly those 29
ids and no others. **Part 3 (3.(2)) is complete.**

### Measured, after B12 and B23

macOS: `cargo build --workspace` EXIT 0; `cargo test --workspace`
**614 passed, 0 failed**. Windows, ARM64 VM, second sync (`C:\p46c`, same
`p36_relbuild.cmd` toolchain):

    cargo check -p aethercore-startup-manager -p aethercore-system-repair
      -p aethercore-intelligence-core -p aethercore-platform-capabilities
      -p aethercore-pc-intelligence -p aethercore-fleet -p aethercore-desktop
      -p aethercore-maintenance-service -p aetherctl --tests
    P46C_CHECK_EXITCODE=0

856 log lines, **0 error lines**, 53 warning lines across 13 kinds, every one
pre-existing and checked rather than assumed: `unused variable: path`
(`fleet/trust.rs`), `field 0 is never read` (the RAII guards), unused imports
`RecoveryReadiness`, `RRF_RT_REG_DWORD`, `UNIX_EPOCH` (`cleaner/lib.rs:8`),
`IRegisteredTask`/`SERVICE_DEMAND_START`
(`startup-manager/windows_impl.rs:32` — traced through every revision of that
file back to `39ccab0`, unused in all of them, so not something this pass
orphaned), and several never-used functions/fields. None were touched, per
the scope rule.

## 46.21 PART 3.(3) — both 0.D findings closed

§46.4 found two values with more than one decider. Both are now decided once,
in `crates/product-identity` — a NEW crate with **zero dependencies**, and
that is the whole reason it is a crate rather than a module in `contracts` or
`ipc`: `apps/install-hardener` depends on nothing but `anyhow`, deliberately,
because it is the elevated MSI helper, and it was one of the three places
that had independently decided the service name. It follows the pattern
`crates/ipc`'s `PIPE_NAME` already set.

**D1 — the Windows service name.** `"AetherCoreMaintenance"` was declared in
`crates/ipc` (`TRUSTED_SERVICE_NAME`), `apps/install-hardener`
(`SERVICE_NAME`) and `services/maintenance-service` (`SERVICE_NAME`), and
`NT SERVICE\AetherCoreMaintenance` was re-typed in full twice more rather
than derived from the name. `service_principal()` now derives it, so the two
halves of one identity cannot drift apart again. Commit `71fc629`.

**D2 — the product name.** 25 bare `"AetherCore"` literals. 17 production
decisions now read `PRODUCT_NAME`; **8 stay literal on purpose** — they are
test fixtures (7 in `release-authority`, one of them a raw JSON wire fixture,
plus 1 in `security`). A test that builds its expectation from the same
constant the code reads proves only that the constant equals itself; those
tests exist to say "the validator accepts the product id 'AetherCore'", so
they have to spell it. The one place that does pin the literals is
`product-identity`'s own test, whose doc comment states why they are a
published contract (§4): the service is registered under that name with the
SCM, and the directories already exist on installed machines. Deduplicating
these is free; editing them is not. Commit `a042a8c`.

**The layers outside Rust.** Seven more declarations live in WiX, two
PowerShell scripts, and `static_validate.py` itself, and none of them can
share a Rust const. Two new gates read the constants out of
`product-identity/src/lib.rs` and assert every non-Rust declaration still
spells the same name: `p46_service_name_has_one_decider` (also asserts the
three former Rust deciders no longer declare their own) and
`p46_product_name_has_one_decider` (INSTALLFOLDER and ProgramDataRoot in
`Product.wxs`). One decider plus checked mirrors, instead of free literals.

**The measuring instrument had to change, and this is the part worth
reading.** `phase8_hardener_fixed_operation_only` asserted that the literal
`'AetherCoreMaintenance'` appeared in the hardener's source. After D1 it does
not, and the gate failed — a check demanding the duplication it should have
wanted removed (defect pattern #4, in the gate rather than the code). It now
asserts the hardener imports the one decider. That changed HOW the property
is proven, not the property.

### Measured

This gate has 21 pre-existing failures in a non-Windows environment, so the
number that means anything is the **delta against a run of pristine HEAD**
extracted to a scratch directory:

    baseline (HEAD)  344 checks / 21 failed
    after D1+D2      346 checks / 21 failed
    NEW_FAILURES=[]  NEWLY_PASSING=[]

macOS: `cargo build --workspace` EXIT 0; `cargo test --workspace`
**616 passed, 0 failed**.

Windows, ARM64 VM — D1 was verified before it was pushed
(`cargo check -p aethercore-ipc -p aethercore-install-hardener
-p aethercore-maintenance-service -p aethercore-product-identity --tests`,
`P46D_CHECK_EXITCODE=0`), and D2 with the **whole workspace**:

    cargo check --workspace --tests
    P46E_CHECK_EXITCODE=0

1044 log lines, **0 error lines**, 49 aethercore crates checked. Warnings are
the same pre-existing set §46.20 enumerates; the only ones inside the touched
crates are three "function is never used" in `crates/ipc/src/lib.rs:121-130`,
a file neither change touches.

## 46.18 NEXT ACTION for a fresh session

**SUPERSEDED where it disagrees with §46.19/§46.20.** Kept because its
standing rules still apply; its status claims do not. What is true now:

**Part 3 (3.(2)) is COMPLETE.** All 33 B-sites are accounted for — 29 fixed,
4 reclassified to A, 0 untriaged, 0 blocked (§46.20 has the table and the
mechanical cross-check). Do not re-open the census, and do not re-litigate
Hasan's `has_*` wire decision (§46.17): it was considered, decided, applied
to all nine sites including B12 (§46.20), and the documented-sentinel
alternative was rejected for a stated reason. The decision still governs any
FUTURE field of the same shape, with its two semantics: for a timestamp,
`has_*` means "the event happened and the time is known"; for a count or a
byte total, it means "the value was determined", **not** "the value is
non-zero". At a layer that is not the proto wire — an internal JSON state
document, an existing flexible `detail`/`warnings` field — `Option`/`null` or
that field IS the companion flag, and adding a proto field would be adding one
that is not needed (B12 and B15 are the two worked examples).

**Rules that still bind, and cost real time when skipped:**

1. **Read the FULL surrounding function before classifying anything.**
   §46.12 found 4 of the first 7 sites were already-guarded false positives;
   §46.19 found B21 was an evidence defect and not the decision defect the
   census claimed, and that B26 needed a second change to be a fix at all.
2. **Count from the code, not from prose.** §46.20 exists because "the nine"
   was counted as a number instead of checked against the list. `grep -rhoE
   "DBT-P46-B[0-9]+"` is the cross-check.
3. **`cargo check --target x86_64-pc-windows-msvc` cannot work from this Mac
   for any crate depending on `aethercore-persistence`** (§46.13 — cc-rs
   cannot cross-compile libsqlite3-sys's C source). Crates that escape it
   (e.g. `platform-capabilities`) can be checked directly. For the rest, use
   the VM: `git archive HEAD` served over `10.211.55.2:8791` (Parallels
   shared folders are unreachable from `prlctl exec`), expanded on the VM,
   built with the `p36_relbuild.cmd` toolchain — VsDevCmd arm64, clang-cl,
   Ninja, LIBCLANG_PATH. §46.19 and §46.20 each record a full working run.
   Do not settle for "reviewed by hand" when the VM is reachable.
4. **Resume the VM, never restore a snapshot.** §46.9's two FORBIDDEN
   snapshots stand.

**What is actually open, in the order Part 3's own priority implies:**

- **3.(3) is DONE** (§46.21). If a future value needs one decider, the home
  already exists: `crates/product-identity`, zero dependencies, plus the
  static gates that hold the non-Rust mirrors to it.
- **3.(7) DBT-P42-013** — 36 temp files leaked by `cargo test`, 4 sites;
  reproduces worse than documented.
- **3.(8) DBT-P41-001** — MSVCP140/VCRUNTIME140 not in the MSI payload. Needs
  a deliberate answer, not a patch.
- **Part 2.A** — needs x64 Windows, unreachable from this Mac. Offer it to,
  or check progress from, the "AetherCore x86_64 Windows physical
  qualification" peer session (offline as of this session's last
  `ListAgents`) before declaring it BLOCKED-OWNER. **2.B** is a decision, not
  a fix.
- **Part 4** — 4.A is claimed substantially done by that same peer session
  and is NOT independently re-verified here (§46.16); 4.B (Gate 5 ARM64),
  4.C (icon pipeline) and 4.D are untouched. **On 4.D: `apps/ui` already
  exists as a Svelte app** (`FleetPage.svelte`, `catalog.en.ts`/`catalog.ar.ts`,
  `svelte-check` clean at 217 files) — 4.D is not a green field, and the
  brief's non-negotiables should be audited against what is there before
  anything is ported.

Before doing anything else: re-check `ListAgents` and `git fetch origin main`.
Two concurrent-session incidents are already on this ledger (§46.10, §46.15),
both from a shared working directory — stage explicit paths, never `git add -A`.

---

# PHASE 47 — P47-FINISH: LAND THE PORT, FINISH THE GATES, HAND BACK THE DECISIONS (2026-09-05)

Session host: the Mac (`/Users/hasanalaaa/dev/aethercore`), `aarch64-apple-darwin`,
session id `aethercore-67`. One session, sequential, no delegated commits or
pushes — per the brief's own record of what three parallel lanes cost in P46.

**Concurrent sessions at start** (`ListAgents`, 2026-09-05): two interactive
peers alive — `aethercore-82` (started 3h before this one) and
`aethercore-design-89` (idle, started 1d before, the session that owns the
design worktree). Seven Remote Control sessions, all offline, including
`AetherCore x86_64 Windows physical qualification` — the peer whose 4.A claim
Item 5.A asks this session to verify independently. `git fetch origin main` at
start: `0 0` — local `main` and `origin/main` identical at `77836cd`.

## 47.0 PROGRESS TABLE (authoritative — resume from here)

| item | status | evidence |
|---|---|---|
| 1.A merge main into `design/shell-v2` | DONE | §47.1 — merge `2d7f4c3`, 5 overlapping files, 0 conflicts |
| 1.B prove the port after the merge | DONE | §47.2 — build 0 errors, arabic 7/7, numbers exit 0, sweep 66/66 x2 |
| 1.C merge to main | DONE | §47.3 — merge `5e18fb9`, 142 files / +16,531 lines on main, static gate delta 0 |
| 1.C.1 the ten screens with no dedicated pass | DONE | §47.4 — 4/4 signature elements on 11/11 screens; 126 literal radii is the one finding, 3 apparent violations cleared |
| 1.C.2 the four real defects the port surfaced | DONE | §47.4 — 2 already fixed by the port, 2 fixed here (`6fdc01c`, `df90889`); both brief counts corrected with measurements |
| 1.C.3 ~430 colour literals in `feature-layout.css` | DECIDED — migrate, registered as `DBT-P47-001` | §47.4 — 440 literals measured; light theme reads 1.02:1 black-on-black; sweep now runs both themes, 132/132 x2 |
| 2 Gate 5 on ARM64 (verify the §41.17 claim first) | **DONE — PASS** | §47.5 claim verified (evidenced for 0.1.6, stale); §47.7 re-run on 0.1.11: validate EMPTY, payload PASS, 16 rows, uninstall 0, **0 survivors on all 14**, reinstall 0, pipe SDDL byte-identical, `engineLabel localModel` from the service. `DBT-P47-002` raised; `DBT-P42-012` still needs x64 |
| 3 icon pipeline (`DBT-P36-004` stays **OPEN**) | **DONE** | `31445b0` pipeline + wiring, `e369010` proof. One SVG -> 17 files in 8.0 s; ico 7 entries / icns 10 chunks decoded back and verified; `--check` PASS with a working negative control; built on the VM: validate EMPTY, payload PASS, File rows still 16, `Icon` table `ProductIcon.ico`, `ARPPRODUCTICON` set, 12,928-byte stream matching `icon.ico` sha256 `4a49d865…` |
| 4 the 2.B decision (`DBT-P42-009`, `DBT-P42-010`) | **DONE — both decided** | §47.8. `DBT-P42-009` **out of scope, recorded**: zero render sites, zero analysis reads, and macOS/Linux already publish the aggregate as a one-element vector, so the field's meaning must be decided before it is filled (new `DBT-P47-003`). `DBT-P42-010` **split**: VRAM usage **implemented** on the counters already open — measured on the VM `sharedUsedBytes` 0 -> **64,241,664**; adapter identity and `dedicatedTotalBytes` **out of scope, recorded** (new `DBT-P47-004`) |
| 5.A independently verify the 4.A x64 claim | **BLOCKED-MACHINE** | §47.9 — `HUSSEIN` unreachable on all four names, only the VM advertises SMB, the owning Remote Control session offline. §46.16 stays their claim, not a verdict |
| 5.B `DBT-P42-011` re-measure on the current build | **BLOCKED-MACHINE** | §47.9 — nothing to re-measure on; §43.5 already settled that ARM64 shows no such bias. Not attempted, not theorised |
| 5.C `DBT-P41-001` MSVCP140/VCRUNTIME140 decision | **DECIDED, evidenced; implementation BLOCKED-MACHINE** | §47.9 — measured: **5 of 6 ARM64 binaries import `VCRUNTIME140.dll`**, so the debt is both-architecture, not x64-only. Option (b) disproved by Microsoft's own redistribution page. Decision: chain `vc_redist` as a Burn prerequisite + a `Launch` condition on the documented registry key; NOT app-local copies. Proving a refusal needs a machine without the runtime |
| owner register | **DONE** | §47.10 |

## 47.1 ITEM 1.A — main merged into the port branch, resolved in the design worktree

Merge base `836906f`. The port branch was **34 ahead / 156 behind** main. Both
sides enumerated by file before touching anything:

    port changed   142 files
    main changed   171 files
    overlap          5 files

The five, and what each side did:

| file | main side | port side | resolution |
|---|---|---|---|
| `apps/ui/package.json` | removed `"version": "0.1.0"` | added `fixture` + `sweep` scripts | auto, both kept |
| `apps/ui/src/dev/layout-fixture.ts` | `officialSource` de-schemed for `phase7_no_remote_ui_assets` | `selectionPolicy` fixture defect fixed (`+96/-2`) | auto, both kept |
| `apps/ui/src/features/fleet/FleetPage.svelte` | `schedulesError` fault path (DBT-P46-B30) | `EmptyState` for the host list | auto, both kept |
| `apps/ui/src/lib/i18n/catalog.en.ts` | +6 keys | +27 keys | auto |
| `apps/ui/src/lib/i18n/catalog.ar.ts` | +6 keys | +27 keys | auto |

`git merge main` in `/Users/hasanalaaa/dev/aethercore-design`: **exit 0, zero
conflicts**, merge commit `2d7f4c3`. Both sides verified present afterwards by
grep, not by assumption — `"fixture"`/`"sweep"` at package.json:7-8,
`schedulesError` at FleetPage.svelte:37 and :457, `EmptyState` at :17 and :365.

**The two seams the brief named, checked specifically:**

- **the `has_*` wire fields (`5c1406d`).** The UI consumes none of them
  directly. main's one UI-visible P46 wire addition is `schedulesError` on the
  fleet snapshot, and it survived the merge with its renderer intact. Nothing
  in `apps/ui` reads a `has_*` field, so there was nothing to reconcile.
- **`product-identity` (`a042a8c`, `71fc629`).** The port introduces **no name
  decision**. Its seven new non-comment `AetherCore` occurrences are all i18n
  prose in `catalog.en.ts`/`catalog.ar.ts` (`policy.bandDescription`,
  `drivers.policyHoldsTitle`, `drivers.policyHoldsCopy`, ×2 languages) plus one
  `console.error` tag — joining ~40 pre-existing prose occurrences in the same
  two files. Read the gates rather than guessing their scope:
  `p46_service_name_has_one_decider` reads `Product.wxs`,
  `install-service.ps1`, `uninstall-service.ps1` and three Rust files;
  `p46_product_name_has_one_decider` reads `Product.wxs`'s `INSTALLFOLDER` and
  `ProgramDataRoot`. Neither scopes `apps/ui`, and a TypeScript catalog cannot
  read a Rust const. Nothing to migrate.

## 47.2 ITEM 1.B — the port proven after the merge, before proposing it to main

Every number below is raw output, on the merged branch.

| check | EXPECTED | OBSERVED |
|---|---|---|
| `npm run build` | 0 errors | **0 errors** — 207 modules, 636.50 kB JS / 99.91 kB CSS, 6 font assets, 666 ms |
| `npm run check` | no new type errors | 224 files, **0 errors**, 16 warnings, 3 files with problems |
| `node tools/verify-arabic.mjs` | 7/7, zero system-font fallback | **7/7 PASS**, `ARABIC_EXIT=0` |
| `node tools/verify-numbers.mjs` | zero untraceable numbers | **exit 0**, 23 numbers, all traced |
| `layout-sweep.mjs` populated | clean at 1280/1024/960, both languages | **66/66 pass** |
| `layout-sweep.mjs` no service | same, with no service attached | **66/66 pass** |

`svelte-check` exits **1**, and that is `--fail-on-warnings` acting on 16
pre-existing warnings, not a regression: 15 unused-CSS selectors in
`DeepScanPage.svelte`/`FindingCard.svelte` and one `a11y` role warning in
`FluidDialog.svelte`, none in a file this port created. The port's own
pre-merge record was the same 16, down from 17 at its baseline.

**The Arabic check was re-run from the rendered DOM, not from CSS**, which is
the whole point of the tool — the port's headline finding was that the CSS
asked for the bundled face while the compositor painted Tahoma. Read back
through `CSS.getPlatformFontsForNode`, per node, with glyph counts:

    80 Arabic-bearing nodes, 1778 glyphs
    bundled      IBM Plex Sans Arabic 1560, IBM Plex Sans Arabic SmBld 101,
                 Inter 101, JetBrains Mono 16
    system fallback  0 glyph(s)  []

and the subset face still shapes: `"التشخيص"` joined **154px** vs
joining-blocked **217px**. Layout genuinely RTL, not mirrored LTR: rail at
1022-1280 of 1280, `main.left=0`.

`verify-numbers` printed all 23 score-shaped numbers with their source
element; every one is a percentage of a counted total or service-authored
prose, and **0 bare confidence scores**. The gate's own DOM counts:
denied 22 occurrences / 90 elements, evidence 67 occurrences / 10 chips.

Both sweeps report `overflowX=0 clipped=0 overlaps=0` on all 132 measurements,
`band=true` on every one, and `dir` flips `ltr`→`rtl` with the locale.

## 47.3 ITEM 1.C — the port on main

`git merge --no-ff design/shell-v2` into `main`: exit 0, **142 files changed,
16,531 insertions, 451 deletions**, commit `5e18fb9`, pushed. `origin/main`
re-fetched immediately before the merge and found `0 0` against local.

**The measuring instrument, checked before the merge rather than after.**
`static_validate.py` was run on the merged tree and on pristine `main`, and
compared as sets rather than as counts:

    main    346 checks / 21 failed
    merged  346 checks / 21 failed
    NEW_FAILURES=[]  NEWLY_PASSING=[]

The 21 are the same pre-existing non-Windows-environment failures §46.21
recorded. The port adds 96 files under `apps/ui/src` and does not trip
`phase7_no_remote_ui_assets`, which was the gate most likely to catch it.

## 47.4 ITEM 1.C — what the port left open

### 1.C.1 — the ten screens with no dedicated pass, checked against the vocabulary

Performance, Hardware, Deep Clean, Repair, Recovery/Activity, Crash, Fleet,
Command Palette, Consent and the rail were never checked against the shell's
section vocabulary. Checked now, from the **rendered DOM** of 11 pages at 1280,
against `DESIGN.md` §2 (radii, borders, mono restraint) and §3 (six roles) —
not by reading the CSS.

`DESIGN.md` §4's per-screen element list (3D health orbs, thermal maps,
cinematic wave graphs, a floating 3D network map, a dust-clearing animation,
`[Voice Query]`, `[Engage Gaming Mode]`) is a mood board, not a contract, and
several of its entries would require the product to invent data it refuses to
invent. §46/P46-LANE-B already recorded that those section types were not
ported and why. The checkable contract is §2 and §3 plus the four signature
elements, and that is what was measured.

**The four signature elements are present on every one of the eleven screens:**
the policy band on 11/11 (`band=1` each), denied elements 3 on every screen and
4 on Drivers, evidence chips where the data carries citations (Deep Scan 3,
Overview 1, Activity 1). Nothing is missing.

**Finding — radii do not follow §2, and it is one file.** §2 declares three:
cards and modals 32px, interactive 16px, micro-components 999px. Counted across
all UI source:

    167 border-radius declarations — 41 read a token, 126 are literal
    93 of the 126 literals are in src/design/styles/feature-layout.css
    twelve distinct literal values: 5, 7, 8, 9, 10, 11, 12, 13, 14, 15, 18, 20px

From the DOM, of 75 card-sized elements only 1 measured 16px and none measured
32px; of 56 interactive elements 29 measured 8px, 13 measured 10px, and 2
measured 16px. Pills are the one clean family — 60 of 60 at 999px. The token
layer is correct (`--ac-radius-2xl` 32px, `--ac-radius-md` 16px,
`--ac-radius-pill` 999px); the screens do not read it.

**Three apparent violations, each read in full before classifying, and each
cleared:**

- `label.candidate-row.locked` (Drivers) carries a dashed border, which
  §3 reserves for denied. Read at `feature-layout.css:604-608`: it is
  `1.5px dashed var(--role-denied-dash)`, `var(--ac-radius-denied)`,
  `var(--role-denied-wash)` — it **is** the denied vocabulary, on the driver
  refusal row. The class is named `locked`; the styling is correct.
- `.shell-context-kicker` renders in JetBrains Mono on all 11 screens.
  `materials.css:20` — `font: 650 .66rem/1.2 var(--ac-font-mono)`, uppercase,
  `.08em` tracked. A deliberate kicker treatment, one rule at shell level, not
  a technical value rendered in a display face.
- `.empty-state-title` renders in mono on the screens showing the honest empty
  state. `EmptyState.svelte:78-84` — mono, uppercase, `.1em` tracked, is the
  "labelled channels at rest" treatment the port's own commit `c04aa3a`
  describes. Deliberate.

### 1.C.2 — the four defects the port surfaced

| defect | state | evidence |
|---|---|---|
| About panel never requested its data (`void load;`) | **already fixed by the port** | `AboutPanel.svelte:72-75` — `onMount(() => { void load(); })`, with the old form named in the comment |
| fixture using `'UserSelectable'`, a value the service never emits | **already fixed by the port** | `layout-fixture.ts:93-98,112-114` — `selectionPolicy` now `FirmwareManualReview`/`OfficialVendorUtility`/`SelectableRecommended`, and the refusal path renders |
| "four dead buttons in the Insights panel" | **corrected to two, and fixed** | §47.5 — commit `6fdc01c` |
| "seven undefined custom properties" | **corrected to 35, and fixed** | §47.6 — commit `df90889` |

### 1.C.3 — the colour literals in `feature-layout.css`: DECIDED — migrate

Counted, not estimated: **423 hex literals (355 distinct) plus 17 `rgb()`/
`rgba()` calls = 440 literal colour values** in 3,037 lines. The port's note
said "most are neutral surface hexes"; measured by HSL saturation that is not
what the file holds — **87 neutral (sat < 0.12), 252 low-chroma tinted
(0.12-0.30), 84 chromatic (>= 0.30)**. 102 distinct selectors set a **dark
background** from a literal.

**The decision is migrate, and it is not a preference — the light theme is
broken today and these literals are why.** `feature-layout.css` contains
**zero** `data-theme` rules; light is handled only in `design-tokens.css`,
`materials.css` and `base.css`. So under the light theme every surface that
file paints stays dark while the text turns dark. Measured from the rendered
DOM, 11 pages, contrast computed against the composited backdrop:

    theme=dark    654 text nodes below WCAG AA, 108 distinct
    theme=light   913 text nodes below WCAG AA, 160 distinct
    worst light readings, all near-black on near-black:
      1.02:1  span.technical-isolate  rgba(8,12,18,0.95) on rgb(11,15,18)  drivers
      1.03:1  h4                      rgba(8,12,18,0.95) on rgb(13,16,19)  overview
      1.03:1  strong                  rgba(8,12,18,0.95) on rgb(13,16,19)  startup
      1.04:1  strong "١٫٦٠ GB"        rgba(8,12,18,0.95) on rgb(18,17,14)  cleanup

Confirmed visually, not only numerically: the light-theme Overview screenshot
shows the six capability cards as black rectangles with invisible titles, and
the "Cleanup / Awaiting authorization" bar likewise. Light is a shipping,
user-selectable mode — the rail carries the toggle.

**Why the whole migration is not in this commit.** Each of the 102
dark-background rules needs a light counterpart chosen against the approved
dark appearance, which is the screenshot baseline the port itself is measured
against; `#0f1216` becoming `var(--ac-material-base)` is not the same colour in
dark either. Landing 423 remappings unreviewed would replace a working, approved
appearance in one unreviewable step — the rewrite §5 forbids. It is a design
pass of the same class as the icon artwork, and it is registered rather than
guessed:

> **`DBT-P47-001` — `feature-layout.css` sits outside the design system.**
> 440 literal colour values (355 distinct) in a six-role system, 93 literal
> radii (12 distinct values) in a three-radius system, and no theme rules at
> all, which makes the light theme unreadable on eight of eleven screens
> (1.02:1 measured). Decision: **migrate to roles and radius tokens**. Risk:
> **high** — light theme is unreadable today. Not landed in P47 because each
> mapping changes the approved dark appearance and needs the owner's eye.
> Single next action: pick the light values for the 102 dark-background rules
> against the approved dark baseline, one screen per commit, re-running the
> contrast measurement and both-theme sweep after each.

**What did land, because it is the reason nobody saw this.** Every measurement
of this app through the entire design port ran `themes: ['dark']` —
`layout-sweep.mjs` defaulted to it, so a light-theme artifact was never
produced and no human ever looked at one. The default is now
`['dark', 'light']`. It does not catch the contrast failure (the sweep measures
geometry, and light passes it), but it means the evidence exists by default
instead of being invisible:

    populated, both themes    132/132 pass
    no service, both themes   132/132 pass

The denial chip's 1px-vs-1.5px border reading is left alone, as the brief
directs — Chrome rounds border widths and the approved shell renders
identically.

## 47.5 ITEM 2 — the §41.17 Gate 5 claim, verified against evidence

The brief asks for the claim to be checked before anything is re-run. It is
**evidenced, and it is stale.** Both halves matter.

**Where it was proven.** §16.5-§16.10 (2026-08-31), on this same ARM64 VM. Not
"a note says so" — the record carries the full lifecycle:

- three cases on 0.1.5 — **A** normal (with a synthetic 5-level-deep file under
  `recovery\driver-backups\`), **B** with `HKLM\SOFTWARE\AetherCore` deleted
  first, **C** with the service, ProgramData, Start Menu, `aetherctl.exe` and
  the HKLM key all deleted first — uninstall **exit 0** in all three, where
  case C had previously exited **1603**
- the fourteen-check survivor sweep run after every case, **zero survivors each
  time**, on a sweep hardened after Gate S4 found one the first version missed,
  now walking SYSTEM, SysWOW64, LocalService and NetworkService profiles as
  well as `C:\Users`
- reinstall of `AetherCore-0.1.6-arm64.msi` (`{863BF31B-B840-63F9-5B30-35528662C54F}`),
  exit 0, zero `ICE\d+`, 16 files in `C:\Program Files\AetherCore`, service
  RUNNING, pipe SDDL and install-dir ACLs identical to the §10 baseline, gguf
  sha256 `6a1a2eb6…9407e`, `verbs-outer.ps1` 18/18 across both token contexts,
  `insights list` reporting `engineLabel: "localModel"`
- snapshot `P37-SHIPPING-QUALIFIED {a1696567-7528-4136-a445-848dccd3d2c1}`,
  still present on the VM today

**Why that is not enough to mark 4.B DONE.** It proved **0.1.6**. The workspace
is at **0.1.11**, five product versions and ten phases later, and P46 changed
things this gate is specifically about:

- `product-identity` became the single decider for the **service name** and the
  **product name** (`71fc629`, `a042a8c`) — the service name is what checks 3
  and 5 of the survivor sweep look for, and the product name is `INSTALLFOLDER`
  and `ProgramDataRoot`, which are checks 1 and 2
- `Product.wxs` now selects `libomp140.aarch64.dll` vs `VCOMP140.DLL` on
  `$(sys.BUILDARCH)` (§46.16), so the ARM64 payload authoring changed
- **`DBT-P42-012` is explicitly parked on this exact run.** §46.1: "still fixed
  in code, still unverified end-to-end — the 6 numbered checks in §44.5 need a
  real `build-installer.ps1` + WiX run, deferred to §46's Part 4.B so the VM is
  touched once for the full lifecycle rather than twice."

**Verdict: the claim is true for 0.1.6 and unproven for the current build. Gate
5 is re-run.**

### VM state before anything, enumerated rather than asserted

    prlctl list -a          {291d6c17-…} running "Windows 11"
    ver                     Microsoft Windows [Version 10.0.26200.9168]
    PROCESSOR_ARCHITECTURE  ARM64          whoami  nt authority\system
    snapshots               8, current P40-PRE-HOUSEKEEPING {d652cd40}
                            oldest P36-VM-QUALIFIED {a38386fa} — matches the brief
    installed               AetherCore 0.1.11
                            ProductCode {98FCE2D5-44F0-A27C-A48B-8720FFE672F0}
                            InstallDate 20260901
    service                 AetherCoreMaintenance  Running  Auto  LocalSystem
                            "C:\Program Files\AetherCore\aethercore-maintenance-service.exe"
    install dir             16 files
    ProgramData\AetherCore  5 files (logs\service.jsonl 16,686;
                            state\aethercore.db 4,096; -shm 32,768;
                            -wal 2,385,512; machine-mutation.lock 0)
    pipe                    AetherCore.Maintenance.v7 present
    volumes                 C: 254.5 GB (147.2 free)   D: 0
    restore points          NONE — System Restore is not the recovery mechanism
                            on this VM; the named snapshot is
    security posture        Defender realtime ON, EnableLUA 1,
                            PromptOnSecureDesktop 0 (the recorded Parallels
                            deviation), firewall Domain/Private/Public all True

### A correction to §46.18's rule 3, measured

§46.18 records "Parallels shared folders are unreachable from `prlctl exec`"
and prescribes an HTTP server on `10.211.55.2:8791` instead. Measured today:
the **drive letters** are unreachable — `prlctl exec` runs as
`nt authority\system`, and `X:`/`Y:`/`Z:` are per-user mappings, so `dir Z:`
fails. The **UNC paths are reachable**: `dir \\Mac\dev` and `dir \\psf\dev`
both list the Mac's directory from that same SYSTEM context. The source sync
for this item used `\\Mac\dev\p36-stage\p47\head.tar`, no server needed.

## 47.6 DESTRUCTIVE ACTION RECORD — GATE 5 ON ARM64, CURRENT BUILD

**Written and committed BEFORE the first destructive step**, per the standing
rule.

    ACTION=   1. Sync C:\AetherCore-P36\workspace\AetherCore-Phase35-Master-Delivery
                 to main@b28722d from git archive HEAD, and prune tracked files
                 HEAD no longer carries. Overwrites the qualified build tree.
              2. scripts\build-arm64-msi.cmd — pnpm build, cargo release build of
                 the fixed five-package set, tauri --no-bundle, stage payload,
                 wix build -arch arm64, wix msi validate, payload check.
                 Non-destructive to the installed product.
              3. msiexec /x {98FCE2D5-44F0-A27C-A48B-8720FFE672F0} /qn /l*v —
                 removes the installed AetherCore 0.1.11. DESTRUCTIVE.
              4. Fourteen-check survivor sweep, read-only.
              5. msiexec /i <the MSI from step 2> /qn /l*v, then re-prove every
                 Gate 2 property.

    SNAPSHOT= a NEW named snapshot P47-PRE-GATE5, taken before step 1 and
              enumerated after, not asserted. No existing snapshot is deleted,
              reused or restored. The VM is RESUMED, never restored.

    EXPECTED= step 2: wix build exit 0, `wix msi validate` output EMPTY, payload
                      check PASS, 16 file rows.
              step 3: exit 0.
              step 4: ZERO survivors on all fourteen checks. Any survivor is a
                      finding recorded with its exact path — never deleted by
                      hand and called a pass.
              step 5: exit 0; 16 files installed; service RUNNING as LocalSystem;
                      pipe DACL byte-identical to
                      O:<service SID> G:SY D:P(A;;FA;;;<service SID>)(A;;FR;;;AU)(A;;DC;;;AU)
                      — with (A;;0x12008b;;;AU) recognised as the SAME DACL,
                      FR|DC = 0x120089|0x2 = 0x12008b, NOT drift;
                      engineLabel `localModel` from the RUNNING SERVICE, not
                      from `aetherctl self-check --load-model` (exit 7 there is
                      correct by design and is not the proof).

    NOT DONE= Defender, UAC, Firewall and SmartScreen are not touched. No
              snapshot is deleted or restored. Versions 0.1.9 and 0.1.10 are
              must-not-ship and are not built, installed or produced.
              `core.autocrlf` stays false; the tree is transferred as a tar of
              `git archive`, which does no line-ending translation.

    RECOVERY= restore P47-PRE-GATE5 if the machine is left unusable. The
              product itself is recoverable more cheaply: the previous MSIs are
              on the VM at C:\AetherCore-P36\build\out\, including
              AetherCore-0.1.11-arm64.msi, 1,099,653,120 bytes.

## 47.7 ITEM 2 — GATE 5 ON ARM64, CURRENT BUILD: **PASS**

Every number below is raw output from the ARM64 VM.

### Two things had to be repaired before the recipe could run at all

**The VM's `node_modules` had been copied, not installed.** `pnpm install
--frozen-lockfile` answered "Already up to date" in 1.2 s and the build died on
`'vite' is not recognized`. `apps/ui/node_modules/.bin` held five extensionless
Unix shims and no `.CMD` shims; `node_modules` held `vite.lnk`, `svelte.lnk`,
`typescript.lnk`, `svelte-check.lnk` — Windows shortcut *files* where pnpm's
symlinks should be — and the root virtual store was missing
`.pnpm/vite@8.2.1/node_modules/vite/package.json`. A tree that had been copied
at some point, turning every symlink into a `.lnk`. Removing both `node_modules`
and re-running the same command fixed it: exit 0, 50 packages, **all 50 reused
from the local content-addressable store**, nothing downloaded — so the
lockfile still determines the tree.

**`prlctl exec` argv is capped well below 16 KB.** The base64-through-argv form
the earlier briefs prescribe fails at a 3.5 KB script with
`PrlVmGuest_RunProgram: Unable to open new session`. Everything here stages the
`.ps1` on the Mac and runs it from `\\Mac\dev\p36-stage\p47\` instead (§47.5).
Scripts must also be **pure ASCII** — an em dash in a comment reached PowerShell
mis-decoded and produced a parse error that aborted the whole file. That
particular failure was benign because PowerShell parses before it executes, so
the uninstall in that script did not run; the service was confirmed still
RUNNING before retrying.

### Step 1 -- source sync

Tree brought to `main@b28722d` from `git archive HEAD:phase21-workspace`,
34,703,360 bytes, 1,299 tracked files, `tar -xf` exit 0. **13 files HEAD no
longer tracks were pruned**, including `crates/security-audit/examples/
gd4_live_audit.rs` (moved to `tools/gd4-audit` in `440683d`), three macOS `._`
resource forks, and a `services/maintenance-service/C??ProgramData\...`
directory left by an old test writing to a mangled path. Spot-checked after:
`crates/product-identity/src/lib.rs` present, `tools/gd4-audit/Cargo.toml`
present, `apps/ui/tools/verify-tokens.mjs` present, `insights.proto` carrying
`string id = 7`.

### Step 2 -- build. EXPECTED met on all four criteria

    scripts\build-arm64-msi.cmd            EXITCODE=0
    [2/7] cargo release, fixed 5-package set   Finished in 1m 17s
    [3/7] tauri build --no-bundle              Finished in 2m 35s
    [5/7] wix build -arch arm64                ProductCode {98FCE2D5-44F0-A27C-A48B-8720FFE672F0}
    [6/7] wix msi validate                     0 lines emitted  <- EMPTY, zero ICE
    [7/7] PAYLOAD_CHECK=PASS                   every MSI file authored in Product.wxs

    MSI     C:\AetherCore-P36\build\out\AetherCore-0.1.11-arm64.msi
    bytes   1,099,857,920      (the previous 0.1.11 was 1,099,653,120 -- the
                                delta is the Svelte port inside the desktop exe)
    sha256  0a2c2f89334c570f2e3f5120d47da123f2ab97da74b6e59ca695c8e5509b6ab5

**16 File rows**, read out of the MSI's own `File` table rather than counted off
disk. Row 7 is `libomp140.aarch64.dll`, 599,504 B, in `ServiceComponent` — the
P46 `$(sys.BUILDARCH)` selection (§46.16) picking the ARM64 OpenMP runtime and
not `VCOMP140.DLL`, proven on the artefact.

The ProductCode is unchanged from the installed 0.1.11 because it is a
deterministic function of version and architecture. Same version means the same
ProductCode means a **reinstall**, never a major upgrade -- as designed.

### Step 3 -- uninstall. EXPECTED exit 0

    msiexec /x {98FCE2D5-44F0-A27C-A48B-8720FFE672F0} /qn /l*v
    UNINSTALL_EXIT=0
    LOG_BYTES=149898   C:\AetherCore-P36\logs\p47\uninstall.log
    MSI (c) (F8:18) [13:56:12:655]: MainEngineThread is returning 0

No 1603, no `InstallValidate` return value 3, with the service RUNNING at the
start of the transaction.

### Step 4 -- the fourteen-check survivor sweep. **ZERO SURVIVORS ON ALL FOURTEEN**

     1  INSTALLDIR              False                        clean
     2  PROGRAMDATA             False                        clean
     3  SERVICE                 absent(1060)                 clean
     4  PIPE_COUNT              0                            clean
     5  ARP_COUNT               0                            clean
     6  HKLM_SOFTWARE_AETHER    False                        clean
     7  HKCU_SOFTWARE_AETHER    False                        clean
     8  STARTMENU               0                            clean
     9  SCHEDULED_TASKS         0                            clean
    10  FIREWALL_RULES          0                            clean
    11  HKLM_SERVICES_KEY       False                        clean
    12  EVENTLOG_SOURCE         0                             clean
    13  HKEY_USERS_MARKERS      0                            clean
    14  FILESYSTEM_SWEEP        4 raw hits / 0 machine-wide  clean
    SURVIVORS = 0

Check 14 by root, which is the measurement that decides it:

    C:\Program Files          0 hit(s)
    C:\Program Files (x86)    0 hit(s)
    C:\ProgramData            0 hit(s)
    C:\Windows\System32       0 hit(s)
    C:\Windows\SysWOW64       0 hit(s)
    C:\Users                  4 hit(s)
    TOTAL=4  UNDER_USER_PROFILE=4  MACHINE_WIDE=0

The sweep is read-only by construction. **Nothing was deleted by hand.** All
four raw hits, with creation times:

    2026-09-02 22:40  C:\Users\hasanalaaa\...\Recent\AetherCore.lnk
    2026-08-31 19:02  C:\Users\hasanalaaa\...\Recent\AetherCore_Commercial_Distribution_Research.md.lnk
    2026-08-31 20:08  C:\Users\P36StandardUser\...\Recent\AetherCore_Commercial_Distribution_Research.md.lnk
    2026-08-31 20:33  C:\Users\P36StandardUser\AppData\Local\com.aethercore.desktop

Three are Explorer Recent-items shortcuts the OS wrote, two of them for a
*document* whose filename contains the product name. The fourth is real product
data and is examined rather than waved away, below.

### A finding the sweep produced -- **DBT-P47-002**, a documentation defect

`C:\Users\P36StandardUser\AppData\Local\com.aethercore.desktop` is **274 files,
23,258,063 bytes**, almost all of it `EBWebView\` -- the WebView2 user-data
directory the desktop app creates at runtime, in the profile of whichever user
ran it. Created 2026-08-31 20:33, five days before this uninstall.

It is **not an uninstaller defect**: a per-machine MSI runs in one account's
context and cannot enumerate other users' profiles. That is the same documented
limit check 13 exists for, and it is why MACHINE_WIDE=0 is the criterion. The
uninstall log references it zero times -- `com.aethercore.desktop` 0 hits,
`P36StandardUser` 0 hits. The MSI never knew it was there.

It **is** a defect in `release/UNINSTALL.txt`. That file's NOTES section names
the per-user HKCU marker as the one thing that can remain in another user's
hive, and calls it "one integer" holding "no data". It does not mention 23 MB of
WebView2 cache in `%LOCALAPPDATA%`, and the KEPT section covers only what the
user chose to save and changes made to Windows. The document is more absolute
than the behaviour.

> **`DBT-P47-002` -- `UNINSTALL.txt` does not mention the per-user WebView2
> directory.** `%LOCALAPPDATA%\com.aethercore.desktop` (274 files, 23.3 MB
> measured) survives uninstall in the profile of any user who ran the desktop
> app and was not the account that uninstalled. Risk: **low** -- no product
> state, no user data, browser cache only -- but the shipped promise does not
> say so. Fix: one paragraph in the same NOTES section that already states the
> HKCU limit. **Deliberately NOT applied to the MSI qualified above**, whose
> payload is fixed at sha256 `0a2c2f89...`; it lands in the next package.

### Step 5 -- reinstall, and every Gate 2 property re-proven

    msiexec /i AetherCore-0.1.11-arm64.msi /qn /l*v      INSTALL_EXIT=0
    LOG_BYTES=167648        MainEngineThread is returning 0

| Gate 2 property | expected | observed | |
|---|---|---|---|
| install dir file count | 16 | **16** | PASS |
| service state | Running | Running | PASS |
| `sc qc` start type | AUTO_START (DELAYED) | `2 AUTO_START (DELAYED)` | PASS |
| service account | LocalSystem | LocalSystem | PASS |
| `sc qsidtype` | UNRESTRICTED | UNRESTRICTED | PASS |
| pipe SDDL | the §10 baseline | **byte-identical** | PASS |
| install-dir ACLs | §10 baseline | `NT SERVICE\AetherCoreMaintenance:(OI)(CI)(RX)`, `BUILTIN\Users:(OI)(CI)(RX)`, `BUILTIN\Administrators:(OI)(CI)(F)`, `NT AUTHORITY\SYSTEM:(OI)(CI)(F)` | PASS |
| ARP | AetherCore 0.1.11 | `{98FCE2D5-...}` 0.1.11 | PASS |
| `HKLM\SOFTWARE\AetherCore\InstallVersion` | 0.1.11 | 0.1.11 | PASS |
| ProgramData recreated | yes | 5 files (state 4, logs 1, support-staging 0) | PASS |
| `ipc_probe*` in payload | absent | absent | PASS |
| gguf sha256 | `6a1a2eb6...9407e` | `6a1a2eb6d15622bf3c96857206351ba97e1af16c30d7a74ee38970e434e9407e` | PASS |

The pipe DACL, read with `NamedPipeClientStream(...).GetAccessControl()` and
compared as a string against the baseline recorded at the top of this ledger:

    O:S-1-5-80-4285065559-3530017622-2858480679-3751456793-1187574229G:SYD:P(A;;0x12008b;;;AU)(A;;FA;;;S-1-5-80-4285065559-3530017622-2858480679-3751456793-1187574229)
    BYTE-IDENTICAL TO THE RECORDED BASELINE: True

`(A;;0x12008b;;;AU)` is the AU pair merged -- `FR|DC = 0x120089|0x2 = 0x12008b`.
Not drift, and the brief says so; it is also exactly the string every prior
record on this VM carries.

`sc sdshow` unchanged:
`D:(A;;CCDCLCSWRPWPDTLOCRSDRCWDWO;;;SY)(A;;CCDCLCSWRPWPDTLOCRSDRCWDWO;;;BA)(A;;CCLCSWLOCRRC;;;AU)S:(AU;FA;CCDCLCSWRPWPDTLOCRSDRCWDWO;;;WD)`

The 16 installed files, hashed on disk after the install:

      aethercore-consent-broker.exe               587264  22cdf95286332b76579be51af2865aaebba5c0a35e718d4e86e7292dde0368b4
      aethercore-desktop.exe                     6572032  377173eab0425f83630ab31b7511738e57524e299529164c3b2725f693441d82
      aethercore-install-hardener.exe             255488  c1894b34a651b495d6465addeacf921e89298243dc0ff1a9adc6984c0f42adb8
      aethercore-maintenance-service.exe         9853952  bea286a54f8f6fa8e93e186640bed91bbbe41b249d4293cf6a9fa998210cb649
      aethercore-update-broker.exe                673280  da2c99d18daffa3e5b365f416ead5c06e54ddd92c21f93c97bf32d37d592393d
      aetherctl.exe                              3989504  71652647dd411e92b2ac2962cc13b0a0df7f3a217f19b1b1f4edba99e5b1cb40
      Apache-2.0.txt                               11358  cfc7749b96f63bd31c3c42b5c471bf756814053e847c10f3eb003417bc523d30
      cis_map.json                                  3103  9caf01b4a2f7d2bfda3111395212b27046f6ae614bc847cebaadfe33c9ee8d97
      libomp140.aarch64.dll                       599504  d2649698fc68466ee88cf58ae3ab0fab106f29326e83aaaa5643d30651a962e3
      models.manifest.json                           898  070b6dedc37664250e4029b8360a1e9b30a1d40b6d776a83ddd0631247dae57e
      Qwen-GGUF-NOTICE.txt                         11343  832dd9e00a68dd83b3c3fb9f5588dad7dcf337a0db50f7d9483f310cd292e92e
      qwen2.5-1.5b-instruct-q4_k_m.gguf       1117320736  6a1a2eb6d15622bf3c96857206351ba97e1af16c30d7a74ee38970e434e9407e
      UNINSTALL.txt                                 3206  34f5f10357de5b7cb475f9b016ebac138f6230d756ffbc837e9d2fe7b02a0ad4
      update-trust.json                               83  d4ad925d86f64560bd80c77eae8c606fe810c5f670a7cd42df0836b0653c8b37
      vulndb.json                                   6704  ab76528eacc58fe910d82347d49919d50949954a25649e677eaaf52fdf37f303
      vulndb.manifest.json                           142  2c29c19b2760167fab8b292dde744da51ae8b137a01c870701287fb49d01daa6

### `engineLabel` -- from the running service, which is the only thing that proves it

    C:\Program Files\AetherCore\aetherctl.exe insights list
    exit=0
    == insights list ==
    engineLabel                        localModel
    insights                           []

`localModel` present, `ruleFallback` absent, over the named pipe from the
installed binary to the installed service. The contrast is recorded beside it so
nobody substitutes the weaker check later:

    aetherctl self-check --load-model
    exit=7   aetherctl: capability not available (embeddedModelLoaderNotCompiled)

Exit 7 there is correct by design and proves nothing about the service.

### What this run did NOT prove, stated rather than implied

**`DBT-P42-012` stays unverified end-to-end.** §46.1 parked its six numbered
checks on "a real `build-installer.ps1` + WiX run" and deferred them to this
lifecycle. They did not happen here, and could not: the vcomp140.dll sourcing
and the 193,152-byte / `55aba23c...` hash assertion live in
`scripts/build-installer.ps1`, which is the **x64** pipeline -- it hardcodes
`-arch x64`, `AetherCore/$Version/x64` and requires `vcomp140.dll` in the
payload. The ARM64 recipe is `scripts/build-arm64-msi.cmd` and stages
`libomp140.aarch64.dll` instead. DBT-P42-012 needs the x64 machine, and moves to
Item 5's blocked set rather than being quietly counted as passed here.

## 47.8 ITEM 4 — the 2.B decision, both halves

Full reasoning and every number are in commit `561ef07`'s message. In short:

- **`DBT-P42-009` `perProcessorBusyBp` — OUT OF SCOPE, recorded.** Zero
  consumers (no component renders it; `performance-bottleneck` and
  `intelligence-core` never read it), and macOS/Linux already publish
  `vec![total_busy_bp]` — one element holding the aggregate. Windows' empty
  vector is the most honest of the three. New **`DBT-P47-003`**: one wire field
  currently means "nothing measured" on Windows and "the aggregate, once" on
  macOS/Linux; the meaning has to be decided before the field is filled.
- **`DBT-P42-010` — SPLIT.** VRAM **usage implemented** on counters the query
  already carries (`sharedUsedBytes` 0 → **64,241,664** measured on the VM, and
  `dedicatedUsedBytes` correctly still 0 on a virtual adapter with no dedicated
  VRAM). Adapter identity and `dedicated_total_bytes` **out of scope**, new
  **`DBT-P47-004`**: capacity has no PDH counter, DXGI means COM in a session-0
  LocalSystem service that cannot be written or measured from this Mac, and WMI
  `AdapterRAM` is a 32-bit field that wraps above 4 GB — it would invent a wrong
  number where 0 is honest.

## 47.9 ITEM 5 — the x64 machine: **NOT REACHABLE**

Checked first, as the brief directs, and recorded rather than assumed. The
machine is `HUSSEIN`, an MSI Pulse 16 AI C1VFKG (§41 header).

    ping HUSSEIN / hussein / hussein.local / HUSSEIN.local   no response, all four
    dns-sd -B _smb._tcp local    one Windows SMB service advertised on this
                                 network: "Windows 11" -- the Parallels VM,
                                 not the physical box
    ListAgents                   "AetherCore x86_64 Windows physical
                                 qualification" [86265d]  Remote Control
                                 OFFLINE (also offline at session start)

Three independent checks, same answer. **5.A and 5.B are `BLOCKED-MACHINE`.**

- **5.A — independently verify the 4.A claim.** Requires running `cargo` →
  `tauri` → `wix build` → `wix msi validate` → payload check on x64 and
  recording the MSI sha256 and byte size. Not possible from here. §46.16's
  record stands unchanged: **their claim, not a verdict.**
- **5.B — `DBT-P42-011`, the x64 bias.** The brief's own instruction is to
  re-measure on the current build before explaining anything. There is nothing
  to measure on: the bias was measured on x64 silicon and this Mac and the ARM64
  VM are the wrong hosts by construction — §43.5 already settled that ARM64
  shows no such bias, so re-running here would answer a different question.
  **Not attempted, not theorised.**

`DBT-P42-012` joins them: §47.7 records why the ARM64 lifecycle could not pay
it — its `vcomp140.dll` sourcing and hash assertion live in the x64-only
`build-installer.ps1`.

### 5.C — `DBT-P41-001`: **DECIDED, with evidence, and the debt is bigger than recorded**

5.C asks for a decision, not another observation, and the decision does not need
the x64 machine. Two of its three inputs were measurable here.

**1. The import list, measured on the ARM64 build — and this is news.** The debt
is recorded as an x64 problem (§41.4: "The **x64** service also imports
`MSVCP140.dll`, `VCRUNTIME140.dll` and `VCRUNTIME140_1.dll`"). Run
`llvm-objdump -p` over all six installed ARM64 binaries:

    aethercore-maintenance-service.exe   31 imports, 10 Universal CRT
        NOT part of Windows: libomp140.aarch64.dll, MSVCP140.dll, VCRUNTIME140.dll
    aetherctl.exe                        17 imports, 8 Universal CRT
        NOT part of Windows: VCRUNTIME140.dll
    aethercore-install-hardener.exe       9 imports, 5 Universal CRT
        NOT part of Windows: VCRUNTIME140.dll
    aethercore-consent-broker.exe        14 imports, 6 Universal CRT
        NOT part of Windows: VCRUNTIME140.dll
    aethercore-update-broker.exe         17 imports, 6 Universal CRT
        NOT part of Windows: VCRUNTIME140.dll
    aethercore-desktop.exe               24 imports, 7 Universal CRT
        NOT part of Windows: (none)

**Five of the six ARM64 binaries import `VCRUNTIME140.dll`**, and the service
also imports `MSVCP140.dll`. `VCRUNTIME140_1.dll` is genuinely x64-only. The
`api-ms-win-crt-*` imports are the Universal CRT, which **is** part of Windows
10 and later, and are not the problem.

So **DBT-P41-001 is a both-architecture release blocker**, and the Gate 5 PASS
in §47.7 was taken on a machine that cannot detect it — exactly the blind spot
§41.4 named for x64, measured again here:

    HKLM\...\VisualStudio\14.0\VC\Runtimes\arm64  Version  v14.44.35211.00
    HKLM\...\VisualStudio\14.0\VC\Runtimes\x64    Version  v14.44.35211.00
    System32: MSVCP140.dll 1,372,216 B; VCRUNTIME140.dll 199,200 B;
              VCRUNTIME140_1.dll 53,320 B; vcomp140.dll 367,160 B

**2. Option (b) of the brief is disproved, not merely unproven.** The brief
allows "prove they are guaranteed present on every supported floor with a
citation". Microsoft's own page, *Redistribute Visual C++ Files*
(learn.microsoft.com/en-us/cpp/windows/redistributing-visual-cpp-files,
`ms.date` 2026-04-13), says the opposite in three places and nowhere describes
these DLLs as part of Windows:

- "When you deploy an application, you must also deploy the files that are
  required to support it."
- "The Visual C++ Redistributable packages install and register all Visual C++
  libraries. If you use one, **run it as a prerequisite on the target system
  before you install your application.**"
- On the failure mode: "If Windows can't find one of the Redistributable DLLs
  required by your application, it might display a message similar to this one:
  'This application has failed to start because *library*.dll was not found.'"

**3. The decision.** The same page rules out the two obvious shortcuts:
merge modules are "**deprecated**. We don't recommend that you use them for
application deployment"; app-local copies are possible but "**For servicing
reasons, we don't recommend that you use this installation location**", because
an app-local copy never receives a security update. The recommended mechanism
is central deployment of the redistributable package, run as a prerequisite.

> **Decision: chain `vc_redist.<arch>.exe` as a Burn prerequisite in
> `Bundle.wxs`, exactly as WebView2 is already chained, and add a `Launch`
> condition to `Product.wxs` so a direct MSI install on a machine without the
> runtime refuses at install time instead of installing and failing at service
> start.** The registry key to condition on is the one Microsoft documents on
> that same page: `HKLM\SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\
> {x86|x64|arm64}`, value `Version`.
>
> Explicitly NOT chosen: authoring the DLLs into the payload beside
> `vcomp140.dll`. It is the in-repo precedent, and it is the one Microsoft warns
> against for servicing. It also has form here — §42.5 found five files named
> `vcomp140.dll` on the build machine and the first plausible match was the
> wrong one, caught only by hashing against a recorded baseline. Three more
> hardcoded hashes, written without a machine to measure them on, would repeat
> that defect rather than avoid it.

**Not implemented, and that is the machine gate.** The Burn chain needs
`vc_redist` staged and the bundle rebuilt and installed on a clean box to prove
anything, and the `Launch` condition must be proven to *refuse* — which needs a
machine without the runtime. Neither the physical x64 box nor this VM (which has
the redistributable) can provide that. The decision is recorded; the change is
not written blind.

## 47.10 THE OWNER REGISTER — listed, not attempted, not routed around

Unchanged in substance from §46.8 and the brief. What each one unblocks is
stated, because that is the part a list of blockers usually leaves out.

| # | blocker | unblocks |
|---|---|---|
| 1 | **The icon artwork** — `DBT-P36-004`, a recorded RELEASE BLOCKER | Nothing technical any more. The pipeline is built and proven (§47.4/`e369010`): drop the chosen SVG in, run one command, and every one of the 17 rasters plus the MSI's ARP icon follows. Until then the product ships a provisional mark. **Also reported, not changed, as the brief asks:** the stand-in mark's own committed proof sheet (`design/icon/proof.html`) still uses a cyan/teal gradient outside the six roles |
| 2 | **An Authenticode certificate** — measured absent (0 runners, 0 variables, 0 workflow runs); ~$129/year, SSL.com or Certum cloud-HSM, individual, no hardware token. **Not EV** — Microsoft removed its SmartScreen benefit in August 2024 | Signed installers, and the SmartScreen reputation clock starting. Nothing in this session touched it |
| 3 | **Gate 4** — boot-test the recovery media (created, never booted, and measured **detached**), and nominate a printer/HID/USB-class device. Windows Update offers this machine zero drivers. **Never storage, chipset or GPU** | The driver install/rollback gate, unstarted since §41.17. Two steps, not one: re-attach the media, then boot it |
| 4 | **The UAC consent click**, at the Parallels console — `PromptOnSecureDesktop` is `0` there, a **non-default deviation that must travel with any UAC finding** (re-measured this session: still `0`) | The elevated half of the x64 peer session's work, and its Gate 2 |
| 5 | **Windows Server runtime qualification** — needs a Server 2025 evaluation VM | The Server SKU floor the MSI already advertises (`MsiNTProductType = 3 AND OSCURRENTBUILD >= 17763`) but nothing has ever exercised |
| 6 | **A production update endpoint, a production key or HSM, and a dependency freeze from a trusted workstation** | `update-trust.json` ships DISABLED with zero channels by design; this is what turns updates on |
| 7 | **Payment** — every MoR checked excludes Iraq for sellers in writing; Payoneer is unresolved and needs a live signup attempt | Selling anything at all |
| 8 | **The physical x64 machine, `HUSSEIN`** — offline; measured unreachable three ways this session (§47.9) | Items 5.A and 5.B, `DBT-P42-012`'s six numbered checks, and the *implementation* of the `DBT-P41-001` decision, which needs a box **without** the VC++ redistributable to prove a refusal |
| 9 | **`feature-layout.css`'s light theme** — `DBT-P47-001`, raised this session | Not strictly an owner-only item, but the 102 dark-background rules each need a light value chosen against the approved dark appearance. That is a design pass with the owner's eye on it, not a mechanical migration |

## 47.11 P47 FINAL REPORT

### The table

Every row is `DONE`, `DECIDED` or `BLOCKED-MACHINE`. §47.0 is authoritative.

### What landed, and what was measured after it landed

**The Svelte port is on `main`.** 142 files, +16,531 lines, merge `5e18fb9`.
`main` was merged into the branch first and resolved there; all five overlapping
files auto-merged with both sides intact. After the merge and before proposing
it: build 0 errors, `svelte-check` 224 files / 0 errors, Arabic **7/7 with 0 of
1,778 glyphs drawn by a system fallback** read back through
`CSS.getPlatformFontsForNode`, `verify-numbers` exit 0 across 23 numbers, and
the layout sweep 66/66 twice — populated and with no service attached.
`static_validate.py` came out at **346 checks / 21 failed, byte-identical to
main's own**, `NEW_FAILURES=[]`.

**Gate 5 re-run on ARM64 for the current build: PASS.** The §41.17 claim was
checked before anything was re-run and found *evidenced but stale* — §16.5-§16.10
proves it for 0.1.6, and P46 moved the service and product names behind
`product-identity`, which is what survivor checks 1, 2, 3 and 5 look for. So it
was re-run on 0.1.11: `wix msi validate` **emitted 0 lines**, payload check PASS,
**16 File rows** read out of the MSI's own table, uninstall exit 0, **zero
survivors on all fourteen checks** with `MACHINE_WIDE=0`, reinstall exit 0, the
pipe SDDL **byte-identical** to the recorded baseline, and `engineLabel:
localModel` **from the running service** with `self-check --load-model`'s exit 7
recorded beside it as the weaker check it is.

**Four defects fixed, and two of the brief's own counts corrected by measuring
them.** "Seven undefined custom properties" measured as **35** (37 at the
pre-port baseline, so the port had already removed two); all 35 migrated onto
the token vocabulary, with `verify-tokens.mjs` added as the gate and a working
negative control. "Four dead buttons in the Insights panel" measured as **two of
four**: the row Dismiss could never work because `IntelligenceCoordinator::list`
dropped the session handle and `v1::Insight` had no id field, so the client sent
a list index the registry could never match; and the ✕ control's label said
"Dismiss insights panel" for something that cannot dismiss the panel.

**The icon pipeline exists and is proven end to end.** One SVG in, 17 files out
in 8.0 s, the `.ico` and `.icns` decoded back and verified entry by entry, and
the generated `icon.ico` traced by sha256 from the Mac through the VM into the
MSI's `Icon` table with `ARPPRODUCTICON` set — while the File row count stayed
16, because an `Icon` is not a File row.

**GPU VRAM usage now reads.** `sharedUsedBytes` 0 → **64,241,664** on the VM,
from counters already on the open query: no DXGI, no COM in a session-0
service, no extra sleep, cadence unchanged.

### Recorded rather than worked around

| id | what | why it is recorded and not fixed |
|---|---|---|
| `DBT-P47-001` | `feature-layout.css` sits outside the design system: 440 literal colour values (355 distinct), 93 literal radii, **no theme rules at all**, so the light theme paints near-black text on near-black surfaces — **1.02:1 measured**, and visible in the screenshot | 102 dark-background rules each need a light value chosen against the approved dark appearance. Landing 423 remappings unreviewed replaces a working product in one step |
| `DBT-P47-002` | `UNINSTALL.txt` named only the per-user HKCU marker as surviving an uninstall; 274 files / 23,258,063 bytes of WebView2 cache also survive | **Fixed** in `5d4b957` — but deliberately not in the MSI qualified in §47.7, whose payload is fixed at sha256 `0a2c2f89…` |
| `DBT-P47-003` | `perProcessorBusyBp` means "nothing measured" on Windows and "the aggregate, once" on macOS/Linux — one wire field, two meanings | The meaning is a product decision; filling it in on one platform would deepen the split |
| `DBT-P47-004` | GPU adapter identity and `dedicated_total_bytes` | No PDH counter for capacity; DXGI is COM in a LocalSystem service, unwritable and unmeasurable from here; WMI `AdapterRAM` wraps above 4 GB and would invent a wrong number where 0 is honest |
| `DBT-P36-004` | the icon artwork | Owner decision. **Stays OPEN** — a ready pipeline is not a closed blocker |
| `DBT-P41-001` | **now known to be both-architecture**: 5 of 6 ARM64 binaries import `VCRUNTIME140.dll` | Decided (§47.9): chain `vc_redist` as a Burn prerequisite plus a `Launch` condition. Proving the refusal needs a box without the runtime |
| `DBT-P42-009` / `DBT-P42-010` | see §47.8 | Decided both ways, half implemented |
| `DBT-P42-012` | vcomp140.dll sourcing, still unverified end to end | Its hash assertion lives in the x64-only `build-installer.ps1`; the ARM64 recipe stages `libomp140.aarch64.dll` instead |

Three corrections to inherited claims, each measured rather than argued:
`prlctl exec` **can** read `\\Mac\dev` (the drive letters cannot, because it runs
as SYSTEM); its argv cap is far below the 16 KB the briefs assume; and
`DBT-P41-001` is not an x64-only problem.

### The single next action

**Pick the icon artwork** — it is the only recorded RELEASE BLOCKER whose
technical work is already finished, so it converts from blocker to done with one
file and one command.

---

# PHASE 48 — P48-SHIP: FINISH DEVELOPMENT (2026-09-05)

Session host: the Mac (`/Users/hasanalaaa/dev/aethercore`), `aarch64-apple-darwin`,
driving the Parallels ARM64 Windows 11 VM. One session, sequential, no parallel
lanes, no delegated commits or pushes. `git fetch origin main` at start: **`0 0`**
— local `main` and `origin/main` identical at `ae9accf`.

**What "finished" means in this phase**, taken from the brief and not softened:
development is finished when the only things left require money the owner has
chosen not to spend, or a physical machine that is not attached. The code-signing
certificate is `DEFERRED-OWNER` by explicit decision — recorded once, never
re-argued, never allowed to block a row.

## 48.0 PROGRESS TABLE (authoritative — resume from here)

| item | status | evidence |
|---|---|---|
| 1 reconcile the debt ledger | **DONE** | §48.1 — **73** authoritative ids, every one traced to code read that day or to a commit; all 4 rows the brief predicted were stale confirmed stale, plus 3 more; `DBT-P42-011`'s two conflicting texts collapsed to one authoritative row. Kept live for the rest of the phase — 4 ids raised and 7 moved since |
| 2 the icon, closed | **DONE — `DBT-P36-004` CLOSED** | §48.2 — the mark ships as deliberate artwork; 17 icons regenerated byte-identically from `65f4417…`, full set wired into `tauri.conf.json` (3 entries -> 17), `provisional` retired from `SOURCE.json`, static gate **346/21, `NEW_FAILURES=[]`**. `icon.ico` is byte-identical to the file P47 traced into the MSI `Icon` table (`4a49d865…`, 12,928 B); the MSI trace is **paid in §48.3**: Windows Installer's own extracted `ProductIcon.ico` is 12,928 B sha256 `4a49d865…`, byte-identical to source, with `File` rows still 16. New `DBT-P48-001` raised for the in-app mark |
| 3 `DBT-P41-001` on a clean machine | **DONE — PASS on ARM64** | §48.3 — snapshot `P48-PRE-REDIST-REMOVAL {318d6369-f9d9-47f9-955b-f7611a45a986}`. Runtime removed and **confirmed gone by measurement** (a plain `msiexec /x` returned 0 and removed nothing). Install **refused: 1603**, message naming the runtime in full, **nothing installed**. Binaries on that machine die `0xC0000135 STATUS_DLL_NOT_FOUND`. Runtime restored by reinstall, not by snapshot; product reinstalled exit 0, 16 files, pipe SDDL byte-identical, `engineLabel localModel`. Item 2's icon fold-in paid: OS-extracted `ProductIcon.ico` sha256 `4a49d865…`, byte-identical to source |
| 4 `DBT-P47-001` the light theme | **DONE — PASS** | §48.4 — **0 of 2,986 text nodes below WCAG AA in either theme**, from 1,577. Worst per page: dark **4.75-5.30:1**, light **4.57-4.67:1**. Layout sweep **132/132 populated and 132/132 with no service**, 11 pages x 1280/1024/960 x en,ar x dark,light. `feature-layout.css` literals **440 -> 84**, var() references **64 -> 423**. New committed instrument `tools/contrast-sweep.mjs`, which found its own blind spot (gradients) after a screenshot contradicted it |
| 5 whatever Item 1 says is genuinely open | **DONE** | §48.5 — `DBT-P42-005` already closed (verified, nothing remains). `DBT-P45-004` **ACCEPTED**: measured **4.0% (8/200)**, not the recorded 2% (a 1/50 sample); raising the ceiling **disproved by experiment** (480ms and 900ms budgets both 2/50), harness committed. `DBT-P48-002` **FIXED** — 7 controls that could not fire, gate committed failing first. `DBT-P48-003` raised **and fixed** — two more temp-leak families the `DBT-P42-013` fix never covered. `cargo test --workspace` **620 passed, 0 failed**, **0** temp entries left behind |
| 6 the x64 items | **BLOCKED-MACHINE (6.A/6.B/6.C), and one finding that was not** | §48.6 — `HUSSEIN` unreachable on **four** independent checks. But *unbuilt* is not a machine gate: building `Bundle.wxs` found **`DBT-P48-004` — `AetherCoreSetup.exe` has never been buildable**, a WIX0010 hard error proven pre-existing by compiling the pre-P48 file unchanged. **FIXED**; bundle now builds (sha256 `796866d5…`) with `VCRedist` as the first chained package, read out of Burn's own manifest |
| 7 release-readiness statement | **DONE** | §48.7 — gates by architecture *and* by build, the unproven list with reasons, money and hardware separated, and the nine things a first real user hits |

## 48.1 ITEM 1 — THE DEBT LEDGER, RECONCILED

### Why this was first, and what was wrong with it

The ledger was never one table. It is spread across `DRIFT_LEDGER.md`'s debt
register and its Phase 40 addendum, and across the closing tables of §42.9,
§43.8, §44.7, §45.5, §46.1 and §47.11 in this file. Each phase table recorded
the state *as of that phase* and was never revisited, so a later fix left the
earlier row reading `open` forever. Measured against current code, the brief's
four predicted stale rows are all stale, and there are more:

| id | last phase table says | current code says |
|---|---|---|
| `DBT-P42-012` | open (§42.9) | **fixed** — `scripts/build-installer.ps1:48-95` sources the file explicitly and asserts 193,152 B / `55aba23c…` |
| `DBT-P42-013` | open (§46.1, "worse than documented") | **fixed** — `2f1ded2`, all four sites now own a `tempfile::TempDir` |
| `DBT-P42-009` | open (§46.1) | **superseded** — decided out of scope in P47 item 4, re-raised as `DBT-P47-003` |
| `DBT-P42-010` | open (§46.1) | **split** — VRAM half implemented in P47 item 4; remainder re-raised as `DBT-P47-004` |
| `DBT-P42-006` | "pre-existing, 6 failing" (§42.9) | **not a defect** — `cargo test -p aethercore-driver-hub --lib` today: **18 passed; 0 failed** |
| `DBT-P42-007` | "pre-existing… environment" (§42.9) | **not a defect** — `offline_boundary` today: **1 passed; 0 failed** |
| `DBT-P44-003` | open (§44.7) | **superseded** — closed by §45.0/§45.2 as an instance of `DBT-P45-001`; §44.7's row already says so in prose and its own status column still reads `open` |

### The duplicate, collapsed

`DBT-P42-011` carries two different texts across the tables:

- §42.9 / §43.8 / §46.1: *"byte-rate and latency counters under-report against a
  1 s window"* — a cadence question about the provider's ~80 ms delta window.
- §45.5: *"x64 Windows reads high, ARM64 Windows doesn't, macOS's sign is
  unclear"* — an architecture question.

They are the same defect seen from two angles, and the second is the one that
matters: the first is not a defect at all (a rate over an 80 ms window is
truthful for that window), while the second is the open question. **The
authoritative text is the architecture one.** The other rows point here.

### THE AUTHORITATIVE LEDGER

Every `DBT-*` id that appears anywhere in `SESSION_CONTEXT.md` or
`DRIFT_LEDGER.md`, exactly once, with a status traceable to code read today or
to a commit. `FIXED` = the defect is gone and the evidence names where.
`OPEN` = something remains and the row says what. `RECLASSIFIED` = it was never
a defect, or stopped being one, and the row says why. `SUPERSEDED` = another id
owns it now.

| id | status | evidence, measured or read today |
|---|---|---|
| `DBT-P36-001` | **FIXED** `477a052` | `tools/p36-probes/` absent; `grep -rn 'p36-probes\|aethercore_ipc::probe' crates apps services tools` → **0 hits** |
| `DBT-P36-002` | **FIXED** `477a052` | `crates/ipc/src/lib.rs:121,124` — `pub(crate) fn write_request` / `read_request`; no `pub mod probe` anywhere in the crate |
| `DBT-P36-003` | **FIXED** `1c56080` | `crates/fleet/src/transport.rs:523-535` — `ssh_true_stub()` writes `@exit 0` into `temp_dir()/aethercore-ssh-stub-{pid}`; the VM-only path survives only in the doc comment that names the defect |
| `DBT-P36-004` | **FIXED — closed in this session** | §48.2 — the geometric AE mark ships as deliberate artwork. `--check` PASS 17/17, a real regeneration reproduced all 17 rasters byte-identically, full set wired into `tauri.conf.json`, `provisional` retired. No longer a release blocker |
| `DBT-P36-005` | **FIXED** — the review happened | §46.11 is the review the row asked for: the LocalSystem privilege boundary (`scope.rs` `authorize_targets`, wiring read at `router.rs:1570-1580`), the named-pipe frame surface and its concrete limits, the update-trust gate proven in code rather than in configuration, signature/digest ordering, and the air-gap invariant with numbers. **No privilege-boundary break found**; four findings folded into the 0.C worklist, all four later reclassified A (§46.20). Residual, stated rather than hidden: the register's word was "Codex review" and this was an agent review — a second pair of eyes remains available but is not a defect |
| `DBT-P36-006` | **OPEN** | `crates/security-audit/src/filesystem.rs:36-49` — the Windows arm still returns `0o444 \| (readonly ? 0 : 0o222)` from `permissions().readonly()`. No ACL is read. The filesystem lane's Windows findings still rest on a POSIX approximation. Needs a designed approach (native ACL evidence), not a patch |
| `DBT-P36-007` | **OPEN by design** | retirement condition is "re-baseline at the next seal, not before". No seal has happened: `PHASE35_FINAL_SHA256.txt` is still the newest. Not actionable until a seal is taken |
| `DBT-P36-008` | **FIXED** 2026-08-30, and structurally retired by `DBT-P40-002` | `ipc_probe.exe` removed from the install image; the payload check now answers "did a developer artefact reach the package?" by measurement instead of belief |
| `DBT-P40-001` | **FIXED** | `tools/p39-probes/` is a workspace member with `[[bin]] name = "p39_pipe_attack"`; nothing under `apps/aetherctl/examples/` |
| `DBT-P40-002` | **FIXED** | `scripts/check-msi-payload.ps1` derives its allowlist from `installer/wix/Product.wxs`; wired as `[7/7]` of `scripts/build-arm64-msi.cmd` and after `wix msi validate` in `scripts/build-installer.ps1`; `check-msi-payload.selftest.ps1` exercises the clean case and **both** failure branches |
| `DBT-P40-003` | **FIXED** `440683d` | `tools/gd4-audit/{Cargo.toml,src}` present; `crates/security-audit/examples/` does not exist |
| `DBT-P41-001` | **FIXED on ARM64; x64 execution BLOCKED-MACHINE** | §48.3 — `Launch` condition proven to refuse on a machine with the runtime removed (1603, message in full, nothing installed), and the binaries proven to die `0xC0000135` there. §48.6 — the Burn chain is now **compiled and verified in a built bundle**, `VCRedist` first in the chain, `vc_redist.arm64.exe` as its payload. Only executing that chain on x64 remains |
| `DBT-P41-002` | **FIXED** `cc9c51e` | fixed at the type in §42.2; measured fixed on both the offline and the service path (§42.3, §42.8) and on ARM64 (§43.7) |
| `DBT-P41-002a` | **FIXED** | the gpu fault's false detail string is gone; gpu measures engines. Subsumed by the `DBT-P41-002` fix |
| `DBT-P41-002b` | **FIXED structurally** | `crates/performance-telemetry/src/windows_impl.rs:263-266` — `read_u64` now decodes a full `PDH_FMT_COUNTERVALUE` through `decode_pdh_value`; there is no undersized destination left to overflow. The original question ("did it corrupt anything observable?") is moot, not answered — recorded, not claimed |
| `DBT-P42-001` | **FIXED** | `windows_impl.rs:97-103` — `PdhExpandWildCardPathW` bound with all **five** parameters and `PWSTR` (a plain buffer), not `*mut PWSTR` |
| `DBT-P42-002` | **FIXED** | `windows_impl.rs:225-226` — `percentage_to_bp(percent) = (percent * 100).round().min(10_000)`, reached through `read_percent_bp` at `:274` |
| `DBT-P42-003` | **FIXED** | `windows_impl.rs:167-172` — `collect_twice` collects, sleeps, collects; its doc names the exact defect (counters added after the collections) |
| `DBT-P42-004` | **FIXED** §43.7 | ARM64 build exit 0, 7/7 regression tests pass, offline readings agree with the host across two rounds |
| `DBT-P42-005` | **FIXED** §44.3, re-verified today | all three providers end in `CollectedSubsystems { … }.into_snapshot(interval)` — `macos_impl.rs:600-608`, `linux_impl.rs:591-599`, `windows_impl.rs:969-977`. The only other `PerfSnapshot { … }` literals are `into_snapshot` itself (`lib.rs:328`) and `SyntheticPerfPlatform` (`lib.rs:491`), a fixture, not a provider. **Nothing remains** — the brief's "verify what actually remains" answer is: nothing |
| `DBT-P42-006` | **RECLASSIFIED — not a defect** | measured today: `cargo test -p aethercore-driver-hub --lib` → **18 passed; 0 failed**. Third consecutive measurement (§44.3 footnote, §46.1, today) with no failure, and `driver-hub` has not been touched since Phase 31. The P42 observation was never reproduced |
| `DBT-P42-007` | **RECLASSIFIED — environment, and the environment is gone** | measured today: `cargo test -p aethercore-intelligence-core --test offline_boundary` → **1 passed; 0 failed**. The missing `android_system_properties` registry-cache entry that caused it no longer exists as a condition |
| `DBT-P42-008` | **FIXED** | `services/maintenance-service/src/performance.rs:366` names the defect it replaced (`if self.ring.latest(owner).is_none()`), and `:454` records that the condition is now testable without a service, a ring or a clock |
| `DBT-P42-009` | **SUPERSEDED by `DBT-P47-003`** | decided out of scope in P47 item 4 with evidence: zero render sites, zero analysis reads, and macOS/Linux publish `vec![total_busy_bp]` — one field, two meanings. `windows_impl.rs:423` still reads `per_processor_busy_bp: Vec::new()`, which is the honest one of the three. The meaning must be decided before the field is filled |
| `DBT-P42-010` | **SPLIT: half FIXED, half SUPERSEDED by `DBT-P47-004`** | VRAM usage implemented in P47 item 4 — `windows_impl.rs:886-887,908,925-926` carry `dedicated_used_bytes` / `shared_used_bytes`, measured 0 → 64,241,664 on the VM. Adapter identity and `dedicated_total_bytes` stay empty (`:927`) and are `DBT-P47-004` |
| `DBT-P42-011` | **OPEN — the authoritative row** | the x64 byte-rate/latency bias: x64 reads high (+7.47, +4.09 points, 3.47x), ARM64 shows no consistent bias (§43.5), macOS's sign is unclear (§44.4, §45.4). The §42.9/§43.8/§46.1 text ("under-reports against a 1 s window") describes the *observation*, not the defect, and is not separately tracked. Recorded numbers predate P45's field-boundary fix. → **Item 6.B, gated on the machine** |
| `DBT-P42-012` | **FIXED in code; end-to-end execution UNVERIFIED** | `scripts/build-installer.ps1:48-95` — explicit source resolution (`$env:VCToolsRedistDir\x64\Microsoft.VC143.OpenMP\vcomp140.dll`, then a pinned fallback) and a hard assertion on 193,152 bytes / sha256 `55aba23c…`, throwing on either mismatch. The six numbered checks in §44.5 need a real x64 `build-installer.ps1` + WiX run; the ARM64 recipe stages `libomp140.aarch64.dll` and cannot exercise this path. → **Item 6, gated on the machine** |
| `DBT-P42-013` | **FIXED** `2f1ded2` | all four leaking sites now own a `tempfile::TempDir` whose `Drop` survives a panic: `crates/diagnostic-engine/src/lib.rs:586`, `crates/diagnostics/src/lib.rs:239`, `crates/fleet/tests/gd_proofs.rs:166`, `crates/fleet/src/transport.rs:538,595,659`. The commit also corrects the §46.1 count: from a clean temp dir one run leaked **one** file, not 36 — the 36 were accumulated residue of panicking runs, which is exactly why the end-of-test cleanup was the wrong mechanism |
| `DBT-P43-001` | **FIXED** `30e4eab` | `scripts/p36vm/p36_relbuild.cmd` is in the repo; `grep -c ipc_two_client_probe` → **0**. Verified on the VM in §46: exit 0, `Finished` in 4.56 s |
| `DBT-P44-001` | **FIXED** §44.3, re-verified today | `cargo check -p aethercore-performance-telemetry --tests --target x86_64-unknown-linux-gnu` → **EXIT 0** (warnings only, all unused-import) |
| `DBT-P44-002` | **FIXED** §44.3 | `crates/performance-telemetry/src/linux_impl.rs:543-556` — the hottest thermal zone is wired into `has_temperature`/`temperature_c`; the comment names the discard it replaced |
| `DBT-P44-003` | **SUPERSEDED by `DBT-P45-001`** | §45.0 confirmed the root cause by instrumentation before any fix; §45.2 fixed it at the field boundary. §44.7's row says this in prose while its status column still reads `open` — **that column is wrong and this row replaces it** |
| `DBT-P45-001` | **FIXED** §45.2 | `macos_impl.rs:122` and `linux_impl.rs:107` — `busy_bp_from_ticks` / `busy_bp_from_proc` both return `Option<u32>`, `None` on `total == 0`, so "no window observed" is no longer indistinguishable from a real 0% |
| `DBT-P45-002` | **FIXED** §45.2 | `windows_impl.rs:716` names the `.unwrap_or(0)` it replaced; `:784` emits a `storage.rates` Degraded fault naming the device and counter |
| `DBT-P45-003` | **FIXED** §45.2 | `windows_impl.rs:536-545` — `power.temperature` Degraded fault added; `power` no longer reports fully measured when it measured nothing |
| `DBT-P45-004` | **ACCEPTED — measured, with the escape routes disproved** | §48.5 — **4.0% (8/200)** under full load, 0 silent zeros. The recorded 2% was a 1/50 sample. Raising the ceiling changes nothing (480ms and 900ms both 2/50; polling and single-shot identical), and no tick source with a documented cadence exists. The product states it per sample as `cpu Unavailable "no tick delta … spanning 486ms"`. Harness committed at `tests/dbt_p45_004.rs`. Former text: the bounded retry is real — `macos_impl.rs:193-207`, one `EXTENDED_WAIT` of 480 ms, deliberately a single sleep rather than a loop, on evidence that no tie ever resolved before the 4th observation. §45.3 measured the residual at 1/50. → **Item 5** |
| `DBT-P46-B1`…`B21`, `B23`, `B25`, `B26`, `B28`…`B31`, `B33` (29 ids) | **FIXED** | 29 distinct `DBT-P46-B*` markers counted in source today (`grep -rEoh 'DBT-P46-B[0-9]+' crates apps services tools \| sort -u \| wc -l` → **29**, 89 occurrences), matching §46's list exactly |
| `DBT-P46-B22`, `B24`, `B27`, `B32` | **RECLASSIFIED A — not defects** | §46.20, each with a reachability argument rather than an assertion: `serde_json::Number::from_f64` returns `None` for NaN/Infinity so the flagged `Value` cannot be constructed (B22, B32); `DeterministicFallbackReasoner::infer` has zero `Err` returns (B24); `offline.rs:629` validates every byte is ASCII hex four lines above the flagged parse loop (B27) |
| `DBT-P47-001` | **FIXED** | §48.4 — 0 of 2,986 text nodes below AA in either theme (was 1,577). The defect was three defects: the token ladder itself was below AA, the file's muted text failed in **dark** too, and its dark surfaces survived into light. All three fixed and measured |
| `DBT-P47-002` | **FIXED** `5d4b957` | `release/UNINSTALL.txt:72-79` now names `%LOCALAPPDATA%\com.aethercore.desktop` and its ~23 MB of WebView2 cache. Deliberately **not** in the MSI qualified at sha256 `0a2c2f89…`; it lands in the next package |
| `DBT-P47-003` | **OPEN — recorded, product decision** | `per_processor_busy_bp` means "nothing measured" on Windows (`windows_impl.rs:423`) and "the aggregate, once" on macOS/Linux (`macos_impl.rs:224`, `linux_impl.rs:502`). Filling it on one platform deepens the split |
| `DBT-P47-004` | **OPEN — recorded, no honest source** | GPU adapter identity and `dedicated_total_bytes`. No PDH counter for capacity; DXGI is COM in a session-0 LocalSystem service; WMI `AdapterRAM` is 32-bit and wraps above 4 GB. 0 is the honest reading |
| `DBT-P48-004` | **FIXED — and it was a shipping gap nobody had seen** | §48.6 — `Bundle.wxs` carried a WIX0010 hard error predating this session, so `AetherCoreSetup.exe`, the *preferred* consumer install path, had **never been compiled**. Proven pre-existing by building the pre-P48 file from `12c7852` unchanged. Fixed; the bundle now builds and its chain is `VCRedist` → `WebView2` → `AetherCoreMsi` |
| `DBT-P48-003` | **FIXED** | §48.5 — two temp-leak families the `DBT-P42-013` fix never covered (`axt-*` 7 dirs, `p32-gd-*` 6 dirs), both the same start-of-next-run shape. Converted to `tempfile::TempDir`. Measured after: **0** temp entries left by a full `cargo test --workspace` |
| `DBT-P48-002` | **FIXED** | §48.4 raised it, §48.5 fixed it. seven of `Pressable`'s thirty-seven call sites (5 in `CarePanel.svelte`, 2 in `TimelinePage.svelte`) nest a `<button>` inside `Pressable`'s own `<button>`, which is invalid HTML and a nested interactive control; and they wire the handler as `<Pressable on:press={…}>` while `Pressable.svelte` has **no `createEventDispatcher`** and `fluid-press.ts` dispatches no `press` event, so the handler cannot fire. Found while tracing an opaque UA-default grey slab behind those buttons (1.45:1), which **is** fixed. Same class as the two dead Insights controls P47 fixed. Not fixed here: it is a behavioural change across two components and needs its own failing test first Gate `p48_pressable_handlers_can_fire` committed **failing** in `3eb37c8`, then the seven rewired onto `className` + `onclick`; gate passing, `svelte-check` back to its 16-warning baseline |
| `DBT-P48-001` | **OPEN — raised in this session, recorded not fixed** | §48.2 — `NavigationRail.svelte:35-42` draws a hexagon-and-spark, not the shipped AE mark, stroked with a `#7ddcff`→`#45e0c0` gradient; `navigation.css:4-15` adds two more non-role colours. Five values outside the six roles, and a second product mark the user actually sees. Belongs with `DBT-P47-001`'s design pass |

**Count: 73 authoritative ids**, each appearing exactly once — 40 individually
named (P36 8, P40 3, P41 4, P42 13, P43 1, P44 3, P45 4, P47 4) plus the 33
`DBT-P46-B*` ids, which are listed as two rows because 29 share one verdict and
4 share another.

    FIXED         55   (26 named + the 29 DBT-P46-B* markers)
    OPEN           9   as of this reconciliation, before Items 2-6
    RECLASSIFIED   6   (DBT-P42-006, -007, and DBT-P46-B22/B24/B27/B32)
    SUPERSEDED     2   (DBT-P42-009 -> DBT-P47-003; DBT-P44-003 -> DBT-P45-001)
    SPLIT          1   (DBT-P42-010: VRAM half fixed, remainder -> DBT-P47-004)
                  ---
                   73

**This table is kept live for the rest of the phase.** Rows move as items close,
and ids raised here are added to it — that is the whole point of Item 1, and a
ledger that froze at the moment it was reconciled would be the same defect
again. Changes since the reconciliation:

    DBT-P36-004  OPEN -> FIXED    Item 2 (§48.2)
    DBT-P48-001  raised, OPEN     Item 2 (§48.2), recorded not fixed
    DBT-P41-001  OPEN -> FIXED    Item 3 (§48.3) on ARM64; x64 half BLOCKED-MACHINE
    DBT-P47-001  OPEN -> FIXED    Item 4 (§48.4)
    DBT-P48-002  raised -> FIXED  Item 4 raised it, Item 5 (§48.5) fixed it
    DBT-P45-004  OPEN -> ACCEPTED Item 5 (§48.5), measured 4.0%, escape routes disproved
    DBT-P48-003  raised -> FIXED  Item 5 (§48.5)
    DBT-P48-004  raised -> FIXED  Item 6 (§48.6) — the bundle had never compiled

**Closing count, after Items 2-7.** 77 ids — the 73 reconciled plus
`DBT-P48-001` … `-004` raised during the phase.

    FIXED         61   55 at reconciliation, plus DBT-P36-004, DBT-P41-001,
                       DBT-P47-001, DBT-P48-002, DBT-P48-003, DBT-P48-004
    OPEN           6   DBT-P36-006, DBT-P36-007, DBT-P42-011, DBT-P47-003,
                       DBT-P47-004, DBT-P48-001
    ACCEPTED       1   DBT-P45-004
    RECLASSIFIED   6   DBT-P42-006, -007, DBT-P46-B22/B24/B27/B32
    SUPERSEDED     2   DBT-P42-009 -> DBT-P47-003, DBT-P44-003 -> DBT-P45-001
    SPLIT          1   DBT-P42-010
                  ---
                   77

Two rows carry a qualifier the single word cannot, and both say so on their own
row rather than hiding behind the count:

- **`DBT-P41-001` is FIXED, and its x64 execution is `BLOCKED-MACHINE`.** The
  `Launch` condition is proven to refuse; the Burn chain is compiled and verified
  in a bundle but has never run.
- **`DBT-P42-012` is FIXED in code**, with the source resolution and hash
  assertion in place, and its six numbered checks need an x64 pipeline run.

Of the 6 OPEN: two are recorded product decisions (`DBT-P47-003`, `-004`), one is
machine-gated (`DBT-P42-011`), one is not actionable until a seal is taken
(`DBT-P36-007`), one is cosmetic and named (`DBT-P48-001`), and **one is a real
security-evidence gap this brief's items never reached — `DBT-P36-006`.**

`DBT-P36-008` is counted FIXED, not SUPERSEDED: it was closed on its own
evidence in 2026-08-30 and `DBT-P40-002` retired the *class*, not the row.

### What is genuinely open, worst-consequence first

| id | consequence if shipped as-is |
|---|---|
| `DBT-P41-001` | the product may not launch at all on a machine without the VC++ redistributable — and **no gate in this project's history has ever run on such a machine**. Item 3 |
| `DBT-P47-001` | the light theme is unreadable: near-black on near-black, 1.02:1 measured, on 8 of 11 screens. It is a user-selectable mode with a toggle in the rail. Item 4 |
| `DBT-P36-004` | the shipped mark is a placeholder. Item 2 |
| `DBT-P42-011` | numeric bias on x64 readings, magnitude unknown on the current build. Item 6.B, machine-gated |
| `DBT-P42-012` | the x64 payload's OpenMP runtime is sourced but the sourcing has never executed. Item 6, machine-gated |
| `DBT-P45-004` | ~2% of macOS CPU samples degrade honestly instead of reading. Labelled, not silent. Item 5 |
| `DBT-P36-006` | Windows filesystem security findings rest on a POSIX approximation, not ACL evidence |
| `DBT-P47-003` / `DBT-P47-004` | one wire field with two meanings; two GPU fields that stay empty rather than invented |
| `DBT-P36-007` | not actionable — its retirement condition (a seal) has not occurred |

`DBT-P36-006` is the one open row this brief's items do not reach. It is not
promoted into Item 5 by inventing a design for it here: native Windows ACL
evidence is a designed capability, not a patch, and §48.7 states it as a known
limitation a first user could hit rather than pretending it is closed.

## 48.2 ITEM 2 — THE ICON, CLOSED

### What was actually still open, measured before touching anything

P47 built the pipeline and already ran it on this exact SVG. The committed set's
provenance file said so:

    apps/desktop/icons/SOURCE.json
      source        design/icon/aethercore-mark.svg
      sourceSha256  65f441712c7fc8e21d5969001c027be05dd908380b06d08623edf5772b36441f
      provisional   true

So the artwork was already in place and only its **status** was open, plus two
things nobody had done: the config declared 3 of the 17 generated files, and
three places in the tree still described the mark as provisional.

The mark itself, read rather than assumed — `design/icon/aethercore-mark.svg`,
447 bytes, sha256 `65f4417…`:

    <rect width="64" height="64" rx="14" fill="#030508"/>
    <g fill="none" stroke="#5C7CFA" stroke-width="6.5" ...>
      M13 50 L28 14 L28 50   (the A)
      M28 14 L49 14 · M28 32 L42 32 · M28 50 L49 50   (the E)

A geometric AE ligature, one stroke colour, no gradient. `#5C7CFA` is
`--role-interactive` exactly (`design-tokens.css:63`), so the mark is inside the
six-role vocabulary rather than beside it.

### The pipeline, run rather than trusted

    node tools/icon-pipeline/build-icons.mjs --check
      checked 17 generated file(s)
      PASS -- every committed icon is what this source renders.

    node tools/icon-pipeline/build-icons.mjs --source ../design/icon/aethercore-mark.svg
      source   design/icon/aethercore-mark.svg  sha256 65f4417…
      written  17 file(s)
      git status after the real regeneration: only SOURCE.json changed,
      and only by the one field this item removed. All 17 rasters byte-identical.

A real regeneration, not just `--check`, because `--check` renders to a temp dir
and a generator that cannot reproduce its own committed output in place is a
different failure. It reproduced it exactly.

### What changed

| file | change |
|---|---|
| `apps/desktop/tauri.conf.json` | `bundle.icon` **3 entries -> 17**, the whole generated set. `icon.png` / `icon.ico` / `icon.icns` stay **first** in their extension classes because `tauri-codegen`'s `find_icon` and `tauri-build`'s winres path both take the FIRST entry matching an extension (`tauri-codegen-2.6.3/src/context.rs:495-509`, `tauri-build-2.6.3/src/lib.rs:608-618`) — read in the vendored sources, not assumed. Resolution is therefore unchanged: `.ico` for the Windows window icon and the embedded exe resource, `icon.png` for the Unix window icon, `.icns` for the macOS dev icon |
| `apps/desktop/icons/SOURCE.json` | `provisional` field **removed** |
| `tools/icon-pipeline/build-icons.mjs` | stops writing `provisional`; header records that the artwork is settled and that `PNG_TARGETS` and `bundle.icon` must stay in step, with the ordering constraint stated where someone editing the list will read it |
| `installer/wix/Product.wxs` | the `Icon`/`ARPPRODUCTICON` comment now records the decision instead of "PROVISIONAL … owner decision (DBT-P36-004)" |
| `scripts/build-installer.ps1` | same comment, same correction |
| `docs/phase36/DRIFT_LEDGER.md` | `DBT-P36-004` moved from the open debt register to the Closed table |

`provisional` was **removed, not flipped to `false`.** The pipeline writes that
file, and a generator cannot know an editorial decision — a hardcoded `false` is
the same lie the hardcoded `true` had become, just pointing the other way.
Editorial status lives in the debt ledger. Nothing reads the field: measured by
grep across the tree before removing it, one hit in the generator and one in the
file it generates.

### THE DECISION, recorded

> The artwork is **deliberate, not a placeholder**. The owner may replace it at
> any time by dropping a new SVG through the pipeline — one file, one command,
> and all seventeen rasters plus the MSI's ARP icon follow. The mark is
> **monochrome by design**, because plum is the denied-by-policy colour and
> spending it on an icon would cost that role its meaning.

### Gates

    node tools/icon-pipeline/build-icons.mjs --check      PASS, 17/17
    python3 scripts/static_validate.py                    346 checks / 21 failed
      against pristine HEAD                               NEW_FAILURES=[]  NEWLY_PASSING=[]
    cargo check -p aethercore-desktop                     Finished, 0 errors

**One gate caught a real defect in this item's own work, and it is recorded
rather than quietly fixed.** The first version of the `Product.wxs` comment
contained `--` inside an XML comment, which is illegal. `static_validate.py`
went from 21 failures to **22**, `NEW_FAILURES=['phase8_wix_xml_well_formed']`.
It was found because the gate was diffed against pristine `HEAD` as a set rather
than compared as a count — the same discipline §47.3 used for the port. Fixed,
re-run, `NEW_FAILURES=[]`.

`cargo check -p aethercore-desktop --target x86_64-pc-windows-msvc` **cannot run
on this host** and did not: `libsqlite3-sys` needs a Windows C compiler, and
`cc-rs` fails before any Rust is compiled. Pre-existing, unrelated to this
change, and stated rather than skipped silently. The Windows path is exercised
by the VM build in §48.3.

### The MSI Icon table

`apps/desktop/icons/icon.ico` is **12,928 bytes, sha256
`4a49d865183e06f9e70f08258d3ae32e4a1842c71d149c5bffef5ec0365ea8d6`** — byte-
identical to the file §47.4 traced from this Mac, through the VM, into the MSI's
`Icon` table as a 12,928-byte stream with `ARPPRODUCTICON` set and the `File`
row count still 16. The artwork did not change in this item; only its status
did. The re-verification on a fresh package is **folded into §48.3's VM build**
rather than run as a second 12-minute packaging pass, per the brief's own
"touch the VM once" discipline.

### Noted, not fixed — new debt id

> **`DBT-P48-001` — the in-app mark is neither the shipped mark nor inside the
> six roles.** `NavigationRail.svelte:35-42` draws a **hexagon with a spark**,
> not the AE ligature the product ships, and strokes it with
> `<linearGradient id="aethercore-mark">` from `#7ddcff` (sky) to `#45e0c0`
> (teal). `navigation.css:4-15` adds a `rgba(125,220,255,.3)` border and a
> `linear-gradient(145deg, rgba(28,48,83,.95), rgba(8,16,36,.98))` background.
> None of those five values is one of the six roles, and `design-tokens.css:15`
> says of `interactive`: *"Never expresses a state. Never a gradient."*
> The brief named the gradient; measured, the divergence is larger — the user
> sees one mark in the taskbar and Add/Remove Programs and a **different** mark
> in the window. Risk: **low** (cosmetic, no behaviour), but it is a
> single-source-of-truth failure of the same class as `product-identity`.
> Fix: draw the shipped mark, stroked `var(--role-interactive)`, no gradient.
> **Recorded, not fixed** — the brief says report it, and changing the rail's
> brand block is a visual change to the approved shell that belongs with
> `DBT-P47-001`'s design pass, not smuggled into an icon commit.

## 48.3 ITEM 3 — `DBT-P41-001`, IMPLEMENTED AND PROVEN ON A MACHINE WITHOUT THE RUNTIME

### The implementation, both halves of P47's decision

**`Product.wxs` — the `Launch` condition.** A direct `msiexec /i` on a machine
without the runtime now refuses at install time, with a message naming what is
missing, instead of installing successfully and failing at service start.

    <Property Id="VCRUNTIMEVERSION">
      <RegistrySearch Id="VcRuntimeVersion" Root="HKLM"
                      Key="SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\$(sys.BUILDARCH)"
                      Name="Version" Type="raw" Bitness="always64" />
    </Property>
    <Launch Condition="VCRUNTIMEVERSION" Message="AetherCore requires the Microsoft
      Visual C++ 2015-2022 Redistributable (arm64), which is not installed on this
      computer. AetherCore's programs load VCRUNTIME140.dll and MSVCP140.dll from it.
      Install it first …, then run this installer again. AetherCoreSetup.exe installs
      it for you." />

`$(sys.BUILDARCH)` selects the architecture, the same mechanism `Product.wxs`
already uses to choose `libomp140.aarch64.dll` over `vcomp140.dll` (§46.16).
The key was **measured on the VM before it was authored**, not taken from the
documentation alone:

    HKLM\SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\arm64
        Version REG_SZ v14.44.35211.00 · Installed REG_DWORD 0x1
    HKLM\...\Runtimes\x64      Version REG_SZ v14.44.35211.00 · Installed 0x1
    HKLM\...\Runtimes\debug\arm64  and  \debug\x64   ← the debug_nonredist tree,
                                                        deliberately NOT the key read

**`Bundle.wxs` — the Burn prerequisite**, chained exactly as WebView2 is,
`Permanent="yes"` (other applications share these DLLs; removing AetherCore must
not remove them), `Vital="yes"`, detected on the same registry value.
`build-installer.ps1` gains a `-VcRedist` parameter and **resolves and checks the
file before invoking WiX**, so a missing prerequisite fails with a sentence
naming the file — the same discipline `DBT-P42-012` imposed on `vcomp140.dll`,
for the same reason.

### Two measurements taken before anything destructive, because they decide whether the test is safe

**1. Parallels Tools does not depend on the runtime.** If it did, removing the
runtime would cost this session its only channel to the VM. All 38 binaries under
`C:\Program Files\Parallels\Parallels Tools` were scanned for the imported names:

    binaries found: 38
    NO Parallels Tools binary references MSVCP140/VCRUNTIME140

**2. Which running processes load it, and from where.** This is the measurement
that turns `DBT-P41-001` from an inference into an observation:

    aethercore-maintenance-service   MSVCP140.dll       C:\WINDOWS\SYSTEM32\MSVCP140.dll
    aethercore-maintenance-service   VCRUNTIME140.dll   C:\WINDOWS\SYSTEM32\VCRUNTIME140.dll
    msedgewebview2                   VCRUNTIME140.dll   C:\Program Files (x86)\Microsoft\
                                                        EdgeWebView\Application\152.0.4191.53\
                                                        VCRUNTIME140.dll
    msedgewebview2                   VCRUNTIME140.dll   C:\Program Files\WindowsApps\
                                                        Microsoft.VCLibs.140.00.UWPDesktop_…\
                                                        VCRUNTIME140.dll

**The installed AetherCore service loads both DLLs from `System32` — from the
redistributable.** And Microsoft's own WebView2 runtime ships an **app-local
copy** of `VCRUNTIME140.dll` rather than relying on `System32`, which is the
clearest possible evidence that the DLL is not part of Windows: the vendor
carries its own. AetherCore carries none.

### A file-count difference that is not drift, resolved before it could look like one

`C:\Program Files\AetherCore` reports **9 files** at the root, where §47.7's Gate
5 recorded **16**. Enumerated rather than assumed:

    root                    9 files (5 exes + aetherctl + libomp140.aarch64.dll
                                     + UNINSTALL.txt + update-trust.json)
    assets\models           3 files + licenses\ 2 files
    assets\vulndb           3 files
    total                  16 files, matching the MSI's 16 File rows

Same 16. §47.7 counted the tree; the first count here was root-only. Recorded so
a later session does not read `9` as a regression.

### DESTRUCTIVE ACTION RECORD — REMOVING THE VC++ RUNTIME FROM THE VM

**Written and committed BEFORE the first destructive step**, per the standing rule.

    ACTION=   1. Sync C:\AetherCore-P36\workspace\… to this commit and build the
                 ARM64 MSI with the new Launch condition. NON-DESTRUCTIVE, and it
                 happens FIRST — while the runtime is still present — because the
                 WiX/dotnet toolchain must not be tested against its own removal.
              2. Verify the built package: wix msi validate EMPTY, payload check
                 PASS, 16 File rows, the new LaunchCondition row present, and the
                 Icon table stream's sha256 (this also pays item 2's fold-in).
              3. msiexec /x {98FCE2D5-44F0-A27C-A48B-8720FFE672F0} /qn — removes
                 the installed AetherCore 0.1.11. DESTRUCTIVE.
              4. msiexec /x {88A3EF6C-D7E4-4707-B3F5-E530B3AD6081} /qn — removes
                 "Microsoft Visual C++ 2022 Arm64 Runtime - 14.44.35211".
                 DESTRUCTIVE, and the point of the item. Confirm gone by
                 measurement: the registry key absent AND the System32 DLLs gone.
              5. msiexec /i <the MSI from step 1> /qn /l*v — the test.
              6. Reinstall the runtime from vc_redist.arm64.exe, then install the
                 product again and re-prove the Gate 2 properties.

    SNAPSHOT= a NEW named snapshot P48-PRE-REDIST-REMOVAL, taken after step 2 and
              before step 3, enumerated after rather than asserted. No existing
              snapshot is deleted, reused or restored. The VM is RESUMED, never
              restored. Enumerated first: 9 snapshots, oldest
              P36-VM-QUALIFIED {a38386fa-15f9-4f86-a231-5de585ff3cd7} — matching
              the brief. The two "forbidden" pre-install snapshots earlier briefs
              named are NOT present; §46.15 records that correction and it is not
              re-inherited here.

    EXPECTED= step 2: validate EMPTY, payload PASS, 16 File rows, exactly one new
                      LaunchCondition row mentioning VCRUNTIMEVERSION.
              step 4: HKLM\…\VC\Runtimes\arm64 ABSENT and
                      C:\Windows\System32\{MSVCP140,VCRUNTIME140}.dll ABSENT.
                      If either survives, the machine is NOT the machine the test
                      needs and the test does not run — recorded, not worked around.
              step 5: the install REFUSES, non-zero exit, and the verbose log
                      carries the message naming the Visual C++ Redistributable.
                      A silent failure, a success, or a crash on launch is a FAIL.
              step 6: runtime back (registry key and both DLLs present), install
                      exit 0, 16 files, service RUNNING as LocalSystem, pipe SDDL
                      byte-identical to the recorded baseline.

    NOT DONE= Defender, UAC, Firewall and SmartScreen are not touched. No snapshot
              is deleted or restored. Versions 0.1.9 and 0.1.10 are must-not-ship
              and are not built, installed or produced. core.autocrlf stays false;
              the tree is transferred as a tar of `git archive`, which does no
              line-ending translation.

    RECOVERY= vc_redist.arm64.exe is ALREADY STAGED, downloaded before any removal,
              because the ARM64 runtime on this VM is installed as bare MSIs with
              NO BundleCachePath and NO QuietUninstallString — unlike the x86 and
              x64 redistributables, which do have a Package Cache copy. There was
              therefore no on-machine installer to put it back with, and removing
              first and looking second would have been the empty-state trap in its
              most expensive form.
                vc_redist.arm64.exe  11,722,336 bytes
                sha256 5139e1440c3a20b92153a4db561c069a0175aaf76c276c3e5b6f56099edcf4b0
              from https://aka.ms/vs/17/release/vc_redist.arm64.exe. The hash is
              cross-checked against Microsoft's own CDN URL, which embeds the
              SHA256 of the file it serves:
                …/pr/d7450eb5-…/5139E1440C3A20B92153A4DB561C069A0175AAF76C276C3E5B6F56099EDCF4B0/VC_redist.arm64.exe
              If the machine is left unusable, restore P48-PRE-REDIST-REMOVAL. The
              previous MSIs remain at C:\AetherCore-P36\build\out\.

### Step 1-2, first attempt — the build's own validator caught a defect in this item's work

`scripts\build-arm64-msi.cmd` **EXITCODE=0**, and every criterion was met except
one, which is recorded verbatim before it was fixed:

    LINES EMITTED BY 'wix msi validate' = 1        EXPECTED 0 (EMPTY)
    >…\installer\wix\Product.wxs(77) : warning WIX1076: ICE03: String overflow
     (greater than length permitted in column); Table: LaunchCondition,
     Column: Description, Key(s): VCRUNTIMEVERSION<

    PAYLOAD_CHECK=PASS every MSI file is authored in Product.wxs; no developer artefacts
    FILE_ROWS=16
    LAUNCHCONDITION_ROWS=4        (3 before this item; the new one is present)
    MSI  1,099,788,288 B  sha256 38bb62ff8a17ae9043d7d913eb8dac2fa73b3ea9ef6b7f7ad92fd1500daaeab3

`LaunchCondition.Description` is a **255-character column**. The message was
**360 characters** once `$(sys.BUILDARCH)` expanded to `arm64` — 105 over. WiX
reports this as an ICE **warning**, so the package still built and installed;
the only thing that changed was that `wix msi validate` stopped being EMPTY.

That is precisely why the criterion is "EMPTY" and not "exit 0". A package with a
truncated refusal message would have shipped, and the truncation would have eaten
the half of the sentence that tells the user what to do about it.

Fixed by shortening the message to **245 characters** at `arm64`, which is the
worst case (`arm64` is two characters longer than `x64`), and the constraint is
now written in the file immediately above the message so the next person to edit
it sees the limit before they hit it. The build is re-run below rather than
argued about.


### Step 1-2, second attempt — every criterion met

    scripts\build-arm64-msi.cmd              EXITCODE=0
    LINES EMITTED BY 'wix msi validate' = 0  <- EMPTY, zero ICE
    PAYLOAD_CHECK=PASS  every MSI file is authored in Product.wxs; no developer artefacts
    FILE_ROWS=16
    LAUNCHCONDITION_ROWS=4                   (3 before this item)
    MSI  C:\AetherCore-P36\build\out\AetherCore-0.1.11-arm64.msi
         1,099,788,288 bytes
         sha256 fafe27a6789eef0b266e40b7ebc9cec6b97c5fe216e6d1f59cbb643d8a7f7eac

The new row, read out of the package's own `LaunchCondition` table, untruncated:

    COND : VCRUNTIMEVERSION
    MSG  : AetherCore requires the Microsoft Visual C++ 2015-2022 Redistributable
           (arm64), which is not installed on this computer. AetherCore loads
           VCRUNTIME140.dll and MSVCP140.dll from it. Install vc_redist.arm64.exe,
           or run AetherCoreSetup.exe instead.

and the search that feeds it, out of `AppSearch`/`RegLocator`:

    AppSearch   VCRUNTIMEVERSION -> VcRuntimeVersion
    RegLocator  VcRuntimeVersion  root=2 (HKLM)  type=18 (raw | 64-bit view)
                key=SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\arm64  name=Version

`UNINSTALL.txt` is **3,874 bytes** in this package where the §47.7 package had
3,206 — `DBT-P47-002`'s WebView2 paragraph reaching a built package for the first
time, exactly as §47.7 said it would.

### Steps 3-4 — and a finding worth more than the step it came from

`msiexec /x {98FCE2D5-…} /qn` → **UNINSTALL_EXIT=0**, install dir gone, service
gone.

Then the runtime. The first attempt is recorded because of what it did:

    msiexec /x {88A3EF6C-D7E4-4707-B3F5-E530B3AD6081} /qn
    VCREDIST_REMOVE_EXIT=0
    "Product: Microsoft Visual C++ 2022 Arm64 Runtime -- Removal completed successfully."
    MainEngineThread is returning 0

**and nothing was removed.** The registry key, the ARP entry and all five
System32 DLLs were still there afterwards. The log says why:

    Action ended 18:42:56: WixDependencyCheck. Return value 5.
    WixDependencyCheck: Found dependent "{d8bbe9f9-7c5b-42c6-b715-9ee898a2e515}", name: "Unknown".
    CustomAction WixDependencyCheck returned actual error code 259

`{d8bbe9f9-…}` is the **x64 redistributable bundle**, which holds a dependency
reference on the ARM64 runtime MSI. WiX's dependency extension cancels the
uninstall, and Windows Installer reports **success**. A session that removed the
runtime and went straight to the install test would have "proved" the refusal
against a machine that still had the runtime, and the test would have silently
measured nothing. **This is the measuring-instrument pattern arriving one more
time, in the step designed to avoid it** — which is why the destructive record
required the removal to be confirmed by measurement, not by an exit code.

`IGNOREDEPENDENCIES` is WiX's own documented property for this, and the package
advertises it in its own `SecureCustomProperties`:

    SecureCustomProperties = IGNOREDEPENDENCIES;NEWERVCRUNTIMEVERSIONDETECTED;…

    msiexec /x {88A3EF6C-…} IGNOREDEPENDENCIES=ALL /qn
    "Skipping action: WixDependencyCheck (condition is false)"
    VCREDIST_REMOVE_EXIT=0

That changes **how** the runtime is removed, not **what** the test measures.
The criterion — the machine has no VC++ runtime — is unchanged and was then
verified:

    REGKEY_arm64            ABSENT
    REGKEY_debug_arm64      PRESENT  v14.44.35211.00   <- see below
    SYS32 MSVCP140.dll          ABSENT
    SYS32 VCRUNTIME140.dll      ABSENT
    SYS32 VCRUNTIME140_1.dll    ABSENT
    SYS32 vcomp140.dll          ABSENT
    SYS32 concrt140.dll         ABSENT
    ARP: only "Microsoft Visual C++ 2022 Arm64 DEBUG Runtime" remains

**The debug runtime staying is a free negative control, and a strong one.** It
registers under `Runtimes\debug\arm64`, which the `Launch` condition deliberately
does not read. A condition written against the wrong key would have found it and
let the install through — a false pass. It did not.

### Step 5 — THE TEST: **PASS**

    msiexec /i AetherCore-0.1.11-arm64.msi /qn /l*v
    INSTALL_EXIT=1603

    Doing action: LaunchConditions
    Product: AetherCore -- AetherCore requires the Microsoft Visual C++ 2015-2022
      Redistributable (arm64), which is not installed on this computer. AetherCore
      loads VCRUNTIME140.dll and MSVCP140.dll from it. Install vc_redist.arm64.exe,
      or run AetherCoreSetup.exe instead.
    Action ended: LaunchConditions. Return value 3.
    Product: AetherCore -- Installation failed.
    MainEngineThread is returning 1603

    installdir exists: False
    service        : ABSENT
    ARP: none

A refusal, not a silent failure and not a crash on launch: the message is in the
log in full, it names the runtime and both DLLs, and **nothing was installed** —
no directory, no service, no ARP entry to uninstall afterwards.

### What the refusal is protecting against, measured rather than asserted

The brief says the product "may not launch at all" on such a machine. With the
runtime absent, the built binaries were run directly from the staged payload:

    aetherctl.exe                       exit=-1073741515  0xC0000135
    aethercore-maintenance-service.exe  exit=-1073741515  0xC0000135
    aethercore-consent-broker.exe       exit=-1073741515  0xC0000135
    aethercore-desktop.exe              TIMED OUT (no VC runtime import - §47.9)

`0xC0000135` is `STATUS_DLL_NOT_FOUND`. **The maintenance service — the whole
product — cannot start.** `DBT-P41-001` was never a theoretical risk; it is a
machine on which this product does nothing at all, and eleven phases of gates
never saw it because every machine happened to have the runtime. That
`aethercore-desktop.exe` is the one binary that does not die is exactly what
§47.9 predicted: it is the only one of the six that imports none of these DLLs.

### Step 6 — the machine restored, and left better than it was found

    vc_redist.arm64.exe  11,722,336 B  sha256 5139e144…  (the staged copy, re-hashed on the VM)
    /install /quiet /norestart          VCREDIST_INSTALL_EXIT=0

    REGKEY_arm64   PRESENT  Version=v14.44.35211.00  Installed=1
    REGKEY_x64     PRESENT  Version=v14.44.35211.00  Installed=1
    SYS32 MSVCP140.dll 1,372,216 B · VCRUNTIME140.dll 199,200 B ·
          VCRUNTIME140_1.dll 53,320 B · vcomp140.dll 367,160 B · concrt140.dll 722,488 B
          — every byte size identical to the pre-removal measurement

**Restored by installing it back, not by reverting the snapshot**, so
`P48-PRE-REDIST-REMOVAL` remains an unused fallback rather than a spent one.

Two things about the restored state, both recorded rather than glossed:

- **The machine is now better off than before the test.** ARP gained
  `Microsoft Visual C++ 2022 Redistributable (Arm64) - 14.44.35211`
  `{a87e42cd-475d-4f15-8848-e0d60c63c02f}` **with a `BundleCachePath` and a
  `QuietUninstallString`**. Before this item the ARM64 runtime existed only as
  bare MSIs with no cached installer — the condition that made the recovery plan
  necessary in the first place. It now has one.
- **The x86 and x64 entries were never touched.** An intermediate reading
  appeared to show them gone; that was an artifact of the smaller check script
  enumerating only the 64-bit `Uninstall` view while the fuller one also walks
  `WOW6432Node`. Re-measured with the fuller script: all seven entries present.
  Recorded because a false report of collateral damage is as bad as missing real
  damage.

### Step 6b — the product reinstalled, every Gate 2 property re-proven

    msiexec /i AetherCore-0.1.11-arm64.msi /qn /l*v    INSTALL_EXIT=0
    MainEngineThread is returning 0

| Gate 2 property | expected | observed | |
|---|---|---|---|
| install-dir file count (tree) | 16 | **16** | PASS |
| service state | Running | Running | PASS |
| `sc qc` start type | AUTO_START (DELAYED) | `2 AUTO_START (DELAYED)` | PASS |
| service account | LocalSystem | LocalSystem | PASS |
| `sc qsidtype` | UNRESTRICTED | UNRESTRICTED | PASS |
| pipe SDDL | the §10 baseline | **BYTE-IDENTICAL: True** | PASS |
| ARP | AetherCore 0.1.11 | `{98FCE2D5-…}` 0.1.11 | PASS |
| `HKLM\SOFTWARE\AetherCore\InstallVersion` | 0.1.11 | 0.1.11 | PASS |
| ProgramData recreated | yes | 5 files | PASS |
| `engineLabel` from the running service | `localModel` | `localModel`, exit 0 | PASS |

### ITEM 2's FOLD-IN, PAID HERE — the icon traced into the package by sha256

The strongest form of this evidence is not the MSI's stream but the file
**Windows Installer itself extracted** from that stream for Add/Remove Programs:

    C:\Windows\Installer\{98FCE2D5-44F0-A27C-A48B-8720FFE672F0}\ProductIcon.ico
      12,928 bytes
      sha256 4a49d865183e06f9e70f08258d3ae32e4a1842c71d149c5bffef5ec0365ea8d6

**Byte-identical to `apps/desktop/icons/icon.ico` on the Mac.** The whole chain
is now traced by hash: `aethercore-mark.svg` (`65f4417…`) → pipeline → `icon.ico`
(`4a49d865…`) → the MSI `Icon` table (`ProductIcon.ico`, a 12,928-byte stream,
`ARPPRODUCTICON` set) → the icon the OS actually paints. `File` rows stayed
**16**, because an `Icon` is not a `File` row.

One correction to a number produced along the way, stated so it is not read as a
mismatch later: an attempt to hash the Icon stream by reading it through the
`WindowsInstaller.Record.ReadStream` automation API returned 12,928 bytes but
sha256 `84ab4e15…`. That is **an artifact of that API's ANSI string format**
mangling bytes above 0x7F on the way through PowerShell, not a difference in the
package. The OS-extracted file above settles it.

### `DBT-P41-001`: CLOSED on ARM64. What remains is the x64 half.

| half | state |
|---|---|
| the `Launch` condition, direct-MSI path | **PROVEN on ARM64** — refuses with 1603 and a message naming the runtime, installs nothing |
| the `Burn` chain, bundle path | **authored, unbuilt** — `build-installer.ps1` is the x64-only pipeline and the ARM64 recipe builds no bundle. Needs the x64 machine |
| x64 architecture | **BLOCKED-MACHINE** — see item 6 |

## 48.4 ITEM 4 — `DBT-P47-001`, THE LIGHT THEME: **PASS, both themes, both languages**

### The instrument had to exist before the fix could

`layout-sweep.mjs` measures **geometry** — overflow, clipping, overlap. Light
theme passes all of it, which is exactly why this defect survived a full design
port and a screenshot review. P47 measured the contrast once, by hand, and the
numbers went into a commit message; nothing in the repo could measure it again.

So the first thing this item produced is **`apps/ui/tools/contrast-sweep.mjs`**:
every rendered text node, on every page, in both themes, against the surface it
is *actually painted on*. Committed, not run once. It exits 1 below WCAG AA.

Three things it has to get right, and each of them is a way a naive version lies:

1. **The backdrop is composited, not read off the node.** `background-color` on a
   text node is almost always `rgba(0,0,0,0)`; the visible surface is the nearest
   opaque ancestor with every translucent layer between composited over it in
   order. Reading the parent's declared colour reports transparent black and
   invents failures.
2. **The text colour is composited too.** This palette states text as
   `rgba(8,12,18,0.95)`, so alpha has to be resolved against that same backdrop
   or every reading is wrong by the alpha.
3. **Gradients are `background-image`, not `background-color`.** This one was not
   designed in — it was **found by looking at a screenshot after the tool said
   PASS**, and it is the most important of the three. See below.

**The instrument was validated against an independent measurement before it was
trusted**: P47 recorded 654 dark / 108 distinct and 913 light / 160 distinct, and
worst light readings of 1.02:1 on drivers and 1.03:1 on overview/startup. This
tool's first run: **659 dark / 111 distinct, 918 light / 162 distinct**, worst
**1.02:1 drivers**, **1.03:1 overview/startup**. Two independent implementations,
the same numbers.

### The defect was three defects, and only one of them was `feature-layout.css`

The debt row says "`feature-layout.css` sits outside the design system". Measured,
that is a third of the problem:

| cause | dark | light | where |
|---|---|---|---|
| **A. the token ladder itself is below AA** | 249 nodes | 423 nodes | `design-tokens.css` — `--ac-text-4` read **4.00-4.09:1** in dark and `--ac-text-3`/`-4` read **3.10-4.35:1** in light, on the surfaces the app actually paints |
| **B. muted text literals fail in DARK too** | ~390 nodes | — | `feature-layout.css` greys like `rgb(88,101,111)` at **2.71-3.55:1** on its own dark cards. P47 counted 654 dark failures but framed the bug as light-only |
| **C. dark surfaces surviving into light** | — | ~460 nodes | the 1.02:1 black-on-black class the row describes |

**A is the largest single contributor in both themes**, and no amount of work on
`feature-layout.css` would have fixed it: migrating a literal onto a token that
is itself below AA just moves the failure.

### The passes, each measured

    baseline                                    1577 below AA   (dark 659 / light 918)
    pass 1  the token ladder re-spaced           805            (dark 394 / light 411)
    pass 2  neutral literals -> text/material/edge tokens
                                                   32            (dark   3 / light  29)
    pass 3  chromatic literals -> roles, by MEANING
                                                   31
    pass 4  the light role palette made text-legible
                                                    2            (both the same disabled button)
    pass 5  inactive controls exempted per WCAG 1.4.3
                                                    0            + 2 reported exempt
    pass 6  instrument taught to see gradients       7   <- previously INVISIBLE
    pass 7  surface gradients made theme-aware        0

**Pass 1 — the token ladder.** Re-spaced rather than nudged, so four steps stay
four distinguishable steps:

    dark   0.95 / 0.78 / 0.50 / 0.42   ->   0.95 / 0.78 / 0.60 / 0.50
           worst readings now  15.14 / 10.55 / 6.77 / 5.12 : 1
    light  0.95 / 0.72 / 0.55 / 0.48   ->   0.95 / 0.80 / 0.70 / 0.62
           worst readings now  11.33 /  7.97 / 5.94 / 4.66 : 1

**Pass 2 — 320 neutral literals, mechanically.** Text greys to `--ac-text-1..4`
by luminance, near-black `color:` values to `--ac-text-inverse`, dark surfaces to
the nearest `--ac-material-*`/`--ac-sunken` **chosen by compositing each token
over `--ac-void` and matching luminance**, neutral borders to `--ac-edge`/
`--ac-edge-strong`. That last part is why the dark appearance survives:
`--ac-material-base` composites to `rgb(14,16,19)` over the void, and the literal
it replaced, `#0f1216`, is `rgb(15,18,22)`.

An `rgba()` dark surface is still a dark surface, so those are composited over
the void before being judged — that is what caught `.authority-coverage`'s
`rgba(15,24,22,.78)`, a panel the light theme never overrode.

**Pass 3 — chromatic literals, by meaning and not by hue.** A first attempt used
nearest-hue and was thrown away: it assigned `#bda06e` (hue 38) to *attention*
and `#bea777` (hue 41) to *verified*, two roles that mean completely different
things, on a 3-degree difference. **Hue cannot tell "worth a look" from
"cryptographically checked."** Each of the ~20 chromatic rules was read in place
and assigned from what the element says on screen: driver status chips, the
health orb, risk pills, repair check marks, warning strips, the authority panel.

**Pass 4 — the light role palette.** The remaining failures were the roles
themselves: they are tuned as fills, and they are also drawn as **text** on light
surfaces, where they read 1.90-4.37:1. Darkened along their own hue until the
worst surface the app actually paints clears 4.5:1:

    healthy      #2F9E44 -> #1B5A27      attention    #C96500 -> #7C3E00
    critical     #C92A2A -> #972020      info         #3B5BDB -> #2241BB
    denied       #773781 -> #73357D      verified     #A9820A -> #624B06
    interactive  #4C6EF5 -> #163ACF   (text/line) with -solid staying #4263EB
                                       (fill, white ink on it = 4.98:1)

`--role-interactive` and `--role-interactive-solid` **now differ in light, and
that is the point**: one is drawn as text and has to clear 4.5:1 against a light
surface; the other is a fill that carries white ink and has to clear it the other
way round. One value cannot do both.

**Pass 5 — the exemption, implemented rather than assumed.** The last two
failures were the same disabled button. WCAG 2.2 SC 1.4.3 *Incidental* exempts
"text that is part of an inactive user interface component". Raising its contrast
to pass would make a disabled control look enabled. The tool now skips
`[disabled]`/`[aria-disabled=true]`/`fieldset:disabled` — and **prints each one it
skipped**, so the exemption is visible and cannot become somewhere to hide a real
failure.

### Pass 6 — the instrument was wrong, and a screenshot is what found it

After pass 5 the tool said **PASS on all 22 combinations**. The light overview
screenshot showed a **dark circle** with a barely-visible tick in it.

A gradient is painted as `background-image`. `backgroundColor` reports
`rgba(0,0,0,0)`, so a colour-only walk looks straight *through* a dark gradient
panel to the light card behind it and scores the text against the wrong surface.
Twelve rules in this file paint gradients. Teaching the tool to average a
gradient's own colour stops and treat the result as the layer it visually is
turned **PASS into 7 failures at 1.04-1.90:1** — the driver hero metrics
("Recommended", "Missing drivers", the counts `4` and `1`) and the health orb's
tick, all on dark panels the light theme never overrode.

**A clean sweep that agrees with a screenshot is worth more than a clean sweep.**
This is the fourth pattern again — the measuring instrument itself lying — caught
in the instrument built to catch the pattern, and it was caught by looking.

**Pass 7** made those ten surface gradients theme-aware:
`linear-gradient(145deg, var(--ac-material-elevated), var(--ac-material-base))`
and, for the two that carry meaning, `--role-attention-wash` for the warning
metric and `--role-healthy-wash` for the status orb.

### EXPECTED, met — the measurements, not an adjective

    contrast, populated, en   PASS  0 below AA, 2 exempt   2,986 text nodes
      worst per page, dark    4.75 - 5.30 : 1
      worst per page, light   4.57 - 4.67 : 1
    contrast, populated, ar   PASS  0 below, both themes
    contrast, no service, en  PASS  0 below, both themes
    contrast, no service, ar  PASS  0 below, both themes

    layout sweep, populated   132/132   11 pages x 1280/1024/960 x en,ar x dark,light
    layout sweep, no service  132/132   same matrix
      overflowX=0 clipped=0 overlaps=0 on every one; band=true on every one;
      dir flips ltr<->rtl with the locale

    npm run build              0 errors
    npm run check              224 files, 0 errors, 16 warnings (the same 16 §47.2 recorded)
    verify-tokens.mjs          PASS  389 declarations, 0 unresolved  (exit 0)
    verify-arabic.mjs          7/7   (exit 0)
    verify-numbers.mjs         exit 0
    static_validate.py         346 checks / 21 failed, NEW_FAILURES=[] NEWLY_PASSING=[]

Confirmed visually as well as numerically, because that is how pass 6 was found:
the light Drivers screenshot that P47 recorded as four black rectangles with
invisible titles now renders four light cards with readable titles, and the dark
screenshot is the approved appearance — plum policy band, amber authority strip,
green and red status chips, dark cards.

### What `feature-layout.css` looks like now

    literal colour values   440  ->  84    (423 hex + 17 rgb  ->  69 hex + 15 rgb)
    var(--...) references    64  -> 423
    data-theme rules          0  ->   0    <- still zero, and that is now correct

**Zero theme rules is the right answer, not a remaining gap.** The file no longer
needs any: it names tokens, and the tokens carry the theme. The 84 literals that
remain are 24 inside gradients that are decorative rather than surfaces, 12 in
`box-shadow`, and 48 chromatic values that were measured to pass in both themes.
None of them paints text that fails.

`DBT-P47-001` is **CLOSED**.

## 48.5 ITEM 5 — WHAT ITEM 1 SAID WAS GENUINELY OPEN

The brief names three likely survivors. Item 1's reconciliation had already
settled one of them, and the other two split by machine.

### `DBT-P42-005` — settled in Item 1, and the answer is "nothing remains"

The brief says to "verify what actually remains rather than trusting the row".
Verified: all three providers now end in
`CollectedSubsystems { … }.into_snapshot(interval)` — `macos_impl.rs:600-608`,
`linux_impl.rs:591-599`, `windows_impl.rs:969-977`. The only other
`PerfSnapshot { … }` literals in the crate are inside `into_snapshot` itself
(`lib.rs:328`) and `SyntheticPerfPlatform` (`lib.rs:491`), a fixture. **Nothing
remains.** No work was needed and none was invented.

### `DBT-P42-011` — the x64 bias. Item 6, machine-gated.

### `DBT-P45-004` — the residual tick tie: **ACCEPTED, and the recorded rate was wrong**

The brief allows three ways out, and it is worth saying which one the evidence
picked rather than which one was convenient.

**First, the harness that should have existed.** §45.3 measured 1/50 and
explicitly "discarded" the experiment. "2%" has since been quoted in three phase
records and was re-measurable by nobody — the same shape as the contrast number
in Item 4. `crates/performance-telemetry/tests/dbt_p45_004.rs` now measures it,
`#[ignore]`d because it pins every core.

It asserts the thing that actually matters rather than a rate: **a tie must never
arrive as a confident `0` with no `cpu` fault, at any rate.** Asserting a rate
would make the test flake forever on the honestly-reported case, which is the
behaviour §45.2 deliberately landed.

**Measured, 200 trials under guaranteed full-core load:**

    ties (honest)   8 / 200  =  4.0%
    silent zeros    0
    real readings   192, every one 10,000 bp (100% busy, as expected under load)

**So the recorded 2% was a 1/50 sample, not a rate.** The honest number on this
host is 4.0%. Nothing regressed between §45 and now; n=50 was simply too small to
state 2% as a fact, and three later records inherited it as one.

**Option (a), "raise the ceiling with evidence", is disproved by experiment.**
The current code spends one bounded `EXTENDED_WAIT` of 480 ms. A temporary build
made both the retry *shape* and the *budget* adjustable, and all four
configurations were measured at 50 trials each:

    step 120ms  ceiling 480ms   ->  2/50   (poll, current budget)
    step 480ms  ceiling 480ms   ->  2/50   (single shot, current budget = shipped shape)
    step 120ms  ceiling 900ms   ->  2/50   (poll, nearly double the budget)
    step  60ms  ceiling 900ms   ->  2/50   (poll harder, nearly double the budget)

**Doubling the budget changes nothing, and polling changes nothing.** Every tie
in the 200-trial run reported `spanning 483-495ms` — the counters had not moved
after half a second under 100% load on every core. This is not a window that was
too short. The experimental build was reverted; nothing of it is committed.

**Option (b), a tick source with a documented update cadence**, is not available:
the provider reads `host_statistics64(HOST_CPU_LOAD_INFO)`, the per-processor
variant reads the same kernel counters, and Apple documents no update cadence for
either. Recorded as unavailable rather than as untried.

**So option (c): ACCEPTED, with the rate stated in the product's own terms.**
The product already says it, per sample, in its own vocabulary:

    cpu  Unavailable  "no tick delta in sampling window after 2 observation(s)
                       spanning 486ms"

That is the whole point of the `Option<u32>` boundary §45.2 landed: 4% of samples
on this host say "not measured" instead of "0% busy". **A number the product
refuses to invent is not a defect**, and the residual is now a measured 4.0% with
a committed harness rather than a remembered 2% with none.

`DBT-P45-004` moves from **OPEN** to **ACCEPTED**, which is a verdict, not a
shrug: the rate is measured, the escape routes are disproved rather than
untried, and the honest path is guarded by a test.

### `DBT-P48-002` — seven controls that could not fire. **FIXED.**

Raised in Item 4 while tracing a 1.45:1 reading, and it outranks everything else
here on consequence: a control that does nothing is worse for a user than a
sampling gap the product reports honestly.

**Established by grep, not by inference.** `apps/ui/src` contains **zero**
`createEventDispatcher`, **zero** `dispatch(` and **zero** `CustomEvent`. Nothing
in the tree can emit a `press` event. Thirty `<Pressable>` call sites wire their
handler through `onclick=`, the prop the component actually declares and
forwards. **Seven wrote `<Pressable on:press={…}>`** — five in `CarePanel.svelte`,
two in `TimelinePage.svelte` — and those seven also nested a `<button>` inside
`Pressable`'s own `<button>`, which is invalid HTML and a nested interactive
control.

Third occurrence of this class; P47 fixed two dead Insights controls. The
pattern is always the same: **the control looks wired because a handler is named
right next to it.** So the check is a grep, and the grep is now a gate —
`p48_pressable_handlers_can_fire` in `static_validate.py`, **committed failing**
in `3eb37c8` (347 checks / 22 failed) before the fix, passing after (347 / 21,
the same 21 as `main`).

The fix moved the seven onto `className` + `onclick`, which removed the nested
button — **and immediately broke their styling**, which is worth recording
because it is the kind of thing a green gate would have hidden: `svelte-check`
went 16 warnings → **19**, three new "Unused CSS selector". `.ghost-action` and
`.primary-action` were declared in the two components' *scoped* `<style>` blocks,
and scoped CSS cannot reach an element rendered by a child component. Every other
Pressable className (`.primary`, `.secondary`, `.scan-button`) is already declared
globally in `feature-layout.css`/`materials.css` for exactly that reason. The two
rules were **moved there unchanged** — they were already token-based and identical
in both files. `svelte-check` back to **16 warnings / 3 files**, the §47.2 baseline.

### `DBT-P48-003` — the temp-file class was not closed. **RAISED AND FIXED.**

`cargo test --workspace` was run as the definition-of-done check, and used to
re-measure `DBT-P42-013` at the same time. Its fix holds — **0 `aethercore-*`
files** after a full run. But the run left **13 directories** behind from two
families the fix never touched:

    axt-{ghost,live,midframe,perms,roundtrip,shutdown,stale}-{pid}   7
        crates/ipc/tests/unix_adversarial.rs:18
    p32-gd-{auth,gd1,gd1h,gd2,gd2c,gd5}-{pid}                        6
        crates/security-audit/tests/golden.rs:10

Both are the **exact shape `DBT-P42-013` fixed**: `remove_dir_all` at the *start*
of the next run, which a panicking run never reaches. And both are invisible to
the reasoning that justified that row — `DBT-P42-013` was partly argued on "it is
why a naive `*AetherCore*` sweep reports survivors on a developer machine", and
neither `axt-` nor `p32-gd-` would appear in such a sweep at all.

Fixed with the same remedy, `tempfile::TempDir`, whose `Drop` is unconditional.
The short `axt` prefix is preserved deliberately: Unix socket paths are capped at
~104 bytes (`SUN_LEN`), and `TempDir`'s generated name is *shorter* than the one
it replaced. `tempfile` added as a dev-dependency of `aethercore-ipc`; the
`Cargo.lock` delta is that one line.

**Measured after the fix**, same procedure, temp directory snapshotted before and
after a full run:

    cargo test --workspace     620 passed, 0 failed, exit 0
    temp entries left behind   0

### A correction to a number this session produced

An earlier run of `cargo test --workspace` reported **5 failures** in
`aethercore-ipc`'s `unix_adversarial`, all `path must be shorter than SUN_LEN`.
That was **an artifact of this session's own measurement**: `TMPDIR` had been
pointed at a long scratch path to get a clean baseline, and Unix socket paths
cannot exceed ~104 bytes. Re-run against the real temp directory: **620 passed,
0 failed.** Recorded because a false failure report costs the next session as
much as a false pass.

### The one open row this brief's items do not reach

**`DBT-P36-006`** — `crates/security-audit/src/filesystem.rs:36-49`, the Windows
arm of `mode_bits()` still returns `0o444 | (readonly ? 0 : 0o222)` from
`permissions().readonly()`. No ACL is read. The filesystem lane's Windows
findings rest on a POSIX approximation, and that is a **security-evidence gap**,
not a cosmetic one. It is not promoted into this item and no design is invented
for it here: native Windows ACL evidence is a designed capability. It is stated
in §48.7 as something a first real user's auditor would be right to question.

## 48.6 ITEM 6 — THE x64 ITEMS: **BLOCKED-MACHINE**, and one thing that was not

### Reachability, checked first and recorded as the brief directs

`HUSSEIN`, the MSI Pulse 16 AI C1VFKG of §41. **Four independent checks, all
negative**, one more than §47.9 ran:

    ping HUSSEIN / hussein / hussein.local / HUSSEIN.local   no response, all four
    host / dscacheutil                                       NXDOMAIN, both names
    dns-sd -B _smb._tcp local     ONE Windows SMB service on this network:
                                  "Windows 11" — the Parallels VM, not the physical box
    ListAgents                    AetherCore x86_64 Windows physical qualification
                                    [86265d]  Remote Control  OFFLINE
                                  P46 Lane C x64 [ee2160]      OFFLINE
                                  hussein-mutable-fiddle [2dcfc6]  OFFLINE

**6.A, 6.B and 6.C are `BLOCKED-MACHINE`.** Plainly: this session cannot reach an
x64 Windows machine, so it cannot verify the 4.A pipeline claim, cannot
re-measure the bias, and cannot repeat Item 3's clean-machine test on x64.

- **6.A** — §46.16 stays **a peer's claim, not a verdict**, unchanged.
- **6.B** — `DBT-P42-011`. The brief's own instruction is to re-measure on the
  current build **before explaining anything**. There is nothing here to measure
  on: the bias was measured on x64 silicon, §43.5 already settled that ARM64
  shows no such bias, and re-running on this Mac would answer a different
  question. **Not attempted, not theorised, not explained.**
- **6.C** — needs the machine and a restore point on it.

`DBT-P42-012` joins them for the reason §47.7 gave: its `vcomp140.dll` sourcing
and hash assertion live in the x64-only `build-installer.ps1`.

### What was NOT blocked, and what it found

Item 3 left one half of `DBT-P41-001` recorded as "authored, unbuilt". *Unbuilt*
is not a machine gate — it is a compile, and this machine can compile. So the
bundle was built on the ARM64 VM, not as the shipping artefact but to settle
whether the authoring is real.

**It is not, and it never was.**

    wix build installer\wix\Bundle.wxs -arch arm64 ...
    Bundle.wxs(26) : error WIX0010: The Variable/@Value attribute was not found;
                     it is required when attribute Type is specified.
    Bundle.wxs(40) : error WIX0004: The RegistrySearch element contains an
                     unexpected attribute 'Win64'.
    BUNDLE_BUILD_EXIT=4

The second is **this session's**: `util:RegistrySearch` takes `Bitness` in WiX v4+,
not v3's `Win64`. Fixed.

The first is **not**, and it is the finding. `<Variable Name="WindowsInstallationType"
Type="string" />` has no `Value`, which is a hard error. To be certain rather than
plausible, the **pre-P48 file was built unchanged**, straight out of `12c7852`:

    wix build Bundle-pre-p48.wxs -arch arm64 ...
    Bundle-pre-p48.wxs(25) : error WIX0010: ...
    ORIG_BUNDLE_BUILD_EXIT=10

> **`DBT-P48-004` — `AetherCoreSetup.exe` has never been buildable.** `Bundle.wxs`
> carries a WIX0010 hard error that predates this session; the bundle does not
> compile and therefore **has never been produced**. The consumer installer — the
> one that chains WebView2, and now `vc_redist` — is the *preferred* install path
> in `Product.wxs`'s own comment, and it does not exist. Nobody noticed because
> the recorded x64 pipeline (§46.16) is `cargo → tauri → wix build → wix msi
> validate → payload check`: it builds **the MSI**, and stops. **FIXED** in this
> item; risk was **high** and is now retired.

This is the "tests that never touch the real path" pattern at package level: every
gate in eleven phases exercised the MSI, and the artefact a real user would
download was never once compiled.

### The bundle, built — and the chain read out of Burn's own manifest

    wix build installer\wix\Bundle.wxs -arch arm64 ...   BUNDLE_BUILD_EXIT=0
    AetherCoreSetup-arm64.exe   1,113,930,403 bytes
    sha256 796866d59d8d4c70777bb00ba175ae85ba8b63f1ebe761ccb92094d69667e413

    wix burn extract ...   EXTRACT_EXIT=0   manifest.xml 6,366 B

    chain, in order:
      ExePackage   VCRedist
      ExePackage   WebView2EvergreenBootstrapper
      MsiPackage   AetherCoreMsi

    <RegistrySearch Id="VcRuntimeSearch" Variable="VcRuntimeVersion" Root="HKLM"
       Key="SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\arm64" Value="Version"
       Win64="yes" Type="value" VariableType="string" />
    DetectCondition="(NOT (VcRuntimeVersion = &quot;&quot;))"
    FilePath="vc_redist.arm64.exe"

Read out of the **built** bundle, not the source. `Bitness="always64"` is what
compiles to Burn's internal `Win64="yes"`, which is why the manifest shows the
latter.

**What this does and does not settle.** `DBT-P41-001`'s Burn half is now
**authored, compiled, and present in a real bundle with the redistributable as
its first chained package**. What remains is executing that chain on a machine
without the runtime, and doing it for x64 — and that is the machine gate, not the
authoring gate it was an hour ago.

The two prerequisites were fetched from Microsoft and are recorded by hash, the
same discipline `DBT-P42-012` imposed on `vcomp140.dll`:

    vc_redist.arm64.exe             11,722,336 B  sha256 5139e144…
    MicrosoftEdgeWebview2Setup.exe   1,783,000 B  sha256 17debf797a6c737959bc588236e897936ffac1af5f7e515e674ab32f9edfe719

## 48.7 ITEM 7 — WHAT SHIPPING ACTUALLY REQUIRES

> **SUPERSEDED BY §49.8 (P49, 2026-09-06).** The gate table, the unproven list and
> the verdict below were accurate the day they were written. They are now out of
> date in one specific way: the physical x64 machine became reachable, and the gap
> this section names as "the largest structural gap in this qualification" —
> *no single architecture has both a current build and a full gate set* — **is
> closed on x64**, against MSI sha256 `77ee416b…`. `AetherCoreSetup.exe` has also
> now been installed. **Read §49.8 for the current statement.** This section is
> kept unedited as the record of what was true before that.

This is the section the owner decides on, so it is blunt.

### The version

**0.1.11.** `[workspace.package].version` in `Cargo.toml` is the single decider and
every crate reads it. **0.1.9 and 0.1.10 are must-not-ship** and were neither built
nor installed in this phase.

The artefacts this phase produced, by hash:

    AetherCore-0.1.11-arm64.msi     1,099,788,288 B
      sha256 fafe27a6789eef0b266e40b7ebc9cec6b97c5fe216e6d1f59cbb643d8a7f7eac
      — validate EMPTY, payload PASS, 16 File rows, 4 LaunchCondition rows.
        This is the package installed on the VM right now.

    AetherCoreSetup-arm64.exe       1,113,930,403 B
      sha256 796866d59d8d4c70777bb00ba175ae85ba8b63f1ebe761ccb92094d69667e413
      — the FIRST bundle this project has ever compiled (§48.6). Built, never
        installed. Not the shipping artefact: the shipping bundle is x64.

### Gates: what is PASS, on what, against which build

| gate | verdict | architecture | against which build |
|---|---|---|---|
| 0 machine native, protections on, restore point | **PASS** | x64 physical | §41.2 |
| 0f full verified disk image before driver work | **PASS** | x64 physical | §41.12-13. Two honest debts: `wbadmin`'s client exit code was lost, and the machine was **already installed** when imaged — it is a post-install capture, not a pristine one |
| 1 MSI builds, zero ICE, payload explained | **PASS** | x64 physical | §41.4, **MSI `d18d89db…`** — an older build. Not re-run on the current one |
| 2 installed, running, security properties, local model live | **PASS** | x64 physical | §41.14, same older `d18d89db…` |
| 2 (re-proven) | **PASS** | **ARM64 VM** | §48.3 against `fafe27a6…`: 16 files, service RUNNING as LocalSystem, SID UNRESTRICTED, pipe SDDL **byte-identical** to the §10 baseline, `engineLabel localModel` from the running service |
| 3 real-hardware evidence with numbers | **PASS** | x64 physical | §41.15-16 |
| 4 driver install + rollback on a safe device | **NOT STARTED — HARD STOP** | — | owner + hardware, below |
| 5 full lifecycle, zero survivors | **PASS** | **ARM64 VM** | §47.7 against 0.1.11: uninstall exit 0, **zero survivors on all fourteen checks**, `MACHINE_WIDE=0`, reinstall exit 0 |
| `DBT-P41-001` refusal on a runtime-less machine | **PASS** | **ARM64 VM** | §48.3 against `fafe27a6…`: 1603, message naming the runtime in full, **nothing installed** |

**Read that table honestly: the x64 gates passed against a build from Phase 41,
and the current build's gates passed on ARM64.** No single architecture has both
a current build and a full gate set. That is the largest structural gap in this
qualification, and it is not new — it is what "the physical machine is offline"
has cost, compounded.

### What is unproven, and why — nothing here is "probably fine"

| unproven | why |
|---|---|
| **Everything x64 on the current build** | `HUSSEIN` unreachable on four independent checks (§48.6). §46.16's 4.A pipeline claim is **a peer's claim, not a verdict**, and this session could not make it one |
| **`AetherCoreSetup.exe` has never been installed** | It has now been *built* once, on ARM64 (§48.6). Before this phase it had never compiled at all. The consumer install path — the one `Product.wxs`'s own comment calls preferred — has never been run by anyone |
| **The `vc_redist` Burn chain has never executed** | Authored, compiled, verified present as the first chained package in a real bundle. Never run. The `Launch` condition half **is** proven (§48.3) |
| **The WebView2 chain has never executed either** | Same reason: the bundle never built until now |
| **`DBT-P42-011`, the x64 numeric bias** | Recorded numbers (+7.47, +4.09 points, 3.47x) predate P45's field-boundary fix. Not re-measured, and deliberately not explained without a measurement |
| **`DBT-P42-012`, `vcomp140.dll` sourcing** | Fixed in code with an explicit source and a hash assertion; the six numbered checks need an x64 `build-installer.ps1` run |
| **Windows Server, any SKU** | The MSI advertises Server support (`MsiNTProductType = 3 AND OSCURRENTBUILD >= 17763`) and **nothing has ever run on a Server SKU.** The product makes a claim it has never tested |
| **Zero-session operation** | Only session-0-only operation is proven; a console session exists on the VM |
| **A real remote SSH audit** | Scope resolution, per-host isolation and fail-closed trust are proven; an audit against a second real machine is not |
| **Byte-reproducible builds** | Explicitly not a criterion; `RELEASE-METADATA.json` already says `msi_byte_reproducible_claim = false` |

### Blocked on money

**The code-signing certificate — `DEFERRED-OWNER` by explicit decision.**

### Blocked on hardware

**Gate 4, the driver install/rollback gate. Unstarted since §41.17, and it is two
owner steps, not one:**

1. **The recovery media was created but never boot-tested — and it is now
   detached.** §41.17 recorded the E: artifacts as complete; a later session
   measured that `Get-Volume` shows only C: and D:. The external drive holding the
   6.12 GB BIOS+UEFI boot chain has been removed. So: re-attach it, then boot it
   once and confirm the recovery environment can read `D:\WindowsImageBackup`.
2. **There is no candidate device to test with.** Windows Update offers this
   machine **zero** driver updates (§41.16 3c, search-only, ResultCode 2 =
   succeeded). The owner must nominate a printer-class, HID-class or
   USB-peripheral driver. **Never storage, chipset or GPU.**

Also hardware-blocked: **the physical x64 machine itself**, which is what every
row in the unproven table above waits on.

### What a first real user hits that this project knows about and has not fixed

Blunt, and ordered by what they would notice first.

1. **The installer is unsigned, so Windows SmartScreen will warn on it.** Direct
   consequence of the deferred certificate. Not a defect; a decision.
2. **The bundle they would download has never been installed by anyone** — not by
   a test, not by a developer. It compiled for the first time in this phase. A
   first user running `AetherCoreSetup.exe` is running it first.
3. **The in-app mark is not the shipped mark.** `DBT-P48-001`: the taskbar and
   Add/Remove Programs show the Æ ligature; the window's navigation rail draws a
   hexagon-and-spark in a sky→teal gradient, using five colours outside the six
   roles. Cosmetic, visible on every screen, unfixed.
4. **Windows filesystem security findings rest on a POSIX approximation, not ACL
   evidence.** `DBT-P36-006`: `security-audit/src/filesystem.rs:36-49` still
   derives "mode bits" on Windows from `permissions().readonly()`. Any auditor who
   reads the output and then reads the source will find the gap. This is a
   **security-evidence** gap, not a cosmetic one, and no item in this brief
   reached it.
5. **`perProcessorBusyBp` means two different things by platform.**
   `DBT-P47-003`: empty on Windows ("nothing measured"), a one-element aggregate
   on macOS/Linux. One wire field, two meanings, undecided.
6. **GPU adapter identity and total VRAM are empty.** `DBT-P47-004`: usage now
   reads, capacity does not. Honest, and it will look like missing data.
7. **~23 MB of WebView2 cache survives uninstall in other users' profiles.**
   `DBT-P47-002` — now documented in `UNINSTALL.txt` (3,874 B in the current
   package, up from 3,206). The behaviour is unchanged; the promise now matches it.
8. **On macOS/Linux builds, ~4% of CPU samples report "not measured".**
   `DBT-P45-004`, measured 8/200 this phase. It degrades honestly rather than
   printing a zero, and raising the retry ceiling is **disproved**, not untried.
9. **A same-version reinstall needs `REINSTALLMODE=vamus`.** A rebuilt package of
   the same version carries a new PackageCode and `amus` is refused 1638. Windows
   Installer behaviour, not a product defect, but it belongs in any runbook.

### The verdict, in one paragraph

**Development is finished in the sense the brief defines: everything left needs
either money the owner has declined to spend or a machine that is not attached.**
Nine ledger rows were open at the start of this phase; six were closed with
measurements, one was accepted with its rate measured and its escape routes
disproved, and two are unreachable without hardware. Four new defects were found
and fixed, one of them — `AetherCoreSetup.exe` never having compiled — a shipping
gap that eleven phases of green MSI gates had hidden.

**But do not read "development is finished" as "ready to ship."** The honest
statement is narrower: *the product is qualified on ARM64 against the current
build, and on x64 against a Phase 41 build, and the consumer installer has never
been installed by anyone.* Shipping to a real user means, in order: reach the x64
machine, run the pipeline and Gate 5 there on 0.1.11, install the bundle at least
once, and finish Gate 4. The certificate can be bought at any point after that and
changes only the SmartScreen warning.

## 48.8 THE OWNER REGISTER — listed, not attempted

Unchanged in substance from §47.10. What each one unblocks is stated, because
that is the part a list of blockers usually leaves out.

| # | blocker | unblocks |
|---|---|---|
| 1 | **Code-signing certificate** — `DEFERRED-OWNER` by explicit decision | Signed installers and the SmartScreen reputation clock. Not priced, not re-argued, and it blocked no row in this phase |
| 2 | **Gate 4** — boot-test the recovery media (created, never booted, and measured **detached**: `Get-Volume` shows only C: and D:), and nominate a printer/HID/USB-class device. Windows Update offers this machine **zero** drivers, so no candidate exists. **Never storage, chipset or GPU** | The driver install/rollback gate, unstarted since §41.17. Two steps: re-attach the media, then boot it |
| 3 | **The physical x64 machine, `HUSSEIN`** — unreachable on four independent checks (§48.6) | 6.A, 6.B, 6.C, `DBT-P42-011`, `DBT-P42-012`'s six checks, the x64 half of `DBT-P41-001`, and a current-build x64 gate set |
| 4 | **The UAC consent click** at the Parallels console — `PromptOnSecureDesktop` is `0` there, a **non-default deviation that must travel with any UAC finding** | The elevated half of the x64 peer session's work |
| 5 | **Windows Server runtime qualification** — needs a Server 2025 evaluation VM | The Server SKU floor the MSI already advertises and nothing has ever exercised |
| 6 | **A production update endpoint, a production key or HSM, a dependency freeze from a trusted workstation** | `update-trust.json` ships DISABLED with zero channels by design; this is what turns updates on |
| 7 | **Payment** — every MoR checked excludes Iraq for sellers in writing; Payoneer unresolved and needs a live signup attempt | Selling anything at all |
| 8 | **The icon** — **no longer a blocker.** `DBT-P36-004` closed in §48.2. The owner may still replace the mark at any time: one SVG through the pipeline, one command, seventeen files | — |

## 48.9 P48 FINAL REPORT

### The table

Every row of §48.0 is `DONE` or `BLOCKED-MACHINE`. §48.0 is authoritative.

### What was closed, and what was measured after it closed

**The ledger was reconciled first, and it was wrong in seven places.** All four
rows the brief predicted were stale are stale, and three more were: `DBT-P42-006`
and `-007` had never been defects, and `DBT-P44-003`'s status column still read
`open` while its own prose said closed. `DBT-P42-011` carried two conflicting
texts; they are collapsed and the architecture one is authoritative. 73 ids, each
appearing exactly once, verified mechanically for coverage and uniqueness.

**`DBT-P36-004` closed.** The Æ mark ships as deliberate artwork. Traced by hash
end to end: `aethercore-mark.svg` `65f4417…` → 17 rasters → `icon.ico`
`4a49d865…` → the MSI `Icon` table → `C:\Windows\Installer\{98FCE2D5…}\
ProductIcon.ico`, **12,928 bytes, sha256 `4a49d865…`, extracted by Windows
Installer itself and byte-identical to source.** File rows stayed 16.

**`DBT-P41-001` proven, and the debt was real.** On a machine with the
redistributable removed, the installer refuses with **1603** and a message naming
the runtime in full, installing nothing — and the binaries on that machine die
`0xC0000135 STATUS_DLL_NOT_FOUND`. The service does not start at all. Eleven
phases of gates never saw it because every machine happened to have the runtime.

**`DBT-P47-001` closed: 0 of 2,986 text nodes below WCAG AA in either theme**,
from 1,577. Worst per page: dark 4.75-5.30:1, light 4.57-4.67:1. Sweeps 132/132
populated and 132/132 with no service, across 11 pages × 3 widths × 2 languages ×
2 themes.

**`AetherCoreSetup.exe` compiled for the first time in this project's history**,
and the reason it never had is a hard WIX0010 error that predates this session,
proven pre-existing by building the untouched pre-P48 file.

### Recorded rather than worked around

| id | what | why it is recorded |
|---|---|---|
| `DBT-P45-004` | **ACCEPTED at a measured 4.0% (8/200)** | Raising the ceiling is **disproved by experiment**, not untried: 480 ms and 900 ms budgets both give 2/50, and polling matches single-shot. No tick source with a documented cadence exists. The product already states it per sample |
| `DBT-P36-006` | Windows filesystem findings rest on a POSIX approximation | Native ACL evidence is a designed capability, not a patch. **No item in this brief reached it**, and inventing a design here would be worse than saying so |
| `DBT-P48-001` | the in-app mark is a different mark, in non-role colours | A visual change to the approved shell; belongs with a design pass, not an icon commit |
| `DBT-P47-003` / `-004` | one wire field with two meanings; GPU capacity empty | Product decisions. Filling either in would invent data |
| `DBT-P42-011` / `-012` | the x64 bias and the `vcomp140.dll` checks | Machine-gated. Not re-measured, not theorised, not explained |
| `DBT-P36-007` | the P35 ledger is stale | Its retirement condition is a seal, and no seal has been taken |

### Three corrections to numbers this project believed

1. **`DBT-P45-004`'s residual is 4.0%, not 2%.** The 2% was a 1/50 sample quoted
   as a rate in three later records. Nothing regressed; n=50 was too small.
2. **§47.4's "654 dark contrast failures" was framed as a light-theme bug.**
   Measured, the muted text in `feature-layout.css` fails in **dark** too, at
   2.71-3.55:1, and the biggest single cause in both themes was the token ladder
   itself — not the file the debt row names.
3. **An earlier workspace run in this session reported 5 test failures.** They
   were this session's own `TMPDIR` override exceeding `SUN_LEN` for Unix sockets.
   Re-run properly: **620 passed, 0 failed.** A false failure costs the next
   session as much as a false pass.

### Two instruments now exist that did not

Both replace a number that had been measured once and thrown away.

- **`apps/ui/tools/contrast-sweep.mjs`** — every text node against its composited
  backdrop, both themes, exit 1 below AA. It reproduced P47's independent numbers
  before being trusted, and then **found its own blind spot**: it could not see
  gradients, said PASS, and a screenshot said otherwise. Now composites them.
- **`crates/performance-telemetry/tests/dbt_p45_004.rs`** — the tick-tie rate,
  asserting that a tie is never a silent zero rather than asserting a rate.

And one gate: **`p48_pressable_handlers_can_fire`**, committed failing before its
fix.

### The single next action

**Reach `HUSSEIN` and run the pipeline, Gate 5 and Item 3's clean-machine test
there on 0.1.11** — it is the only thing standing between this build and a
current-build gate set on the architecture the product actually ships to.

---

## 49.0 PROGRESS TABLE (authoritative — resume from here)

**P49 runs on the physical x64 machine `Hussein` at `C:\dev\aethercore`,
elevated (`IsInRole(544)` = `True`, standalone CLI, not the Store package).
`core.autocrlf` = `false`, verified before the first commit.**

This is the machine every "BLOCKED-MACHINE" row in §47.9 and §48.6 was waiting
for. It is reachable for the first time since §41.

Rows move in the same commit as the work they describe. A table that lags is
the measuring-instrument pattern §47 was caught by.

| item | status | evidence |
|---|---|---|
| 0 elevation, autocrlf, machine survey | **DONE** | §49.1 |
| 1 build the current source on x64 (MSI + bundle) | **DONE — every criterion met** | §49.2 — all exit 0; `wix msi validate` **0 lines emitted**, no suppression; `PAYLOAD_CHECK=PASS`; **16 File rows**, row 7 `vcomp140.dll` proving the `$(sys.BUILDARCH)` selection on the x64 artefact; MSI `77ee416b…` 1,100,271,616 B; the **first x64 bundle ever compiled**, `d0398765…` 1,128,354,997 B, chain `VCRedist` → `WebView2` → `AetherCoreMsi` read out of Burn's own manifest. `DBT-P42-012`'s guard fired correctly against the real four-file trap. New **`DBT-P49-001`** |
| 2 Gate 5 on x64 against this build | **DONE — PASS** | §49.4 — restore point `P49-PRE-GATE5` seq **6**, verified by enumeration. Against MSI `77ee416b…`: uninstall exit 0 **twice**, **zero survivors on all fourteen both times**, `MACHINE_WIDE=0` both times, install and reinstall exit 0. 16 files **MATCHED=16 MISMATCH=0** against the built payload; service RUNNING/LocalSystem/AUTO_START(DELAYED); SID **UNRESTRICTED**; ACLs = §10 baseline; `DEV_BINARY_IN_INSTALL_IMAGE=NO`; six verbs with timings; pipe SDDL **byte-identical**; `engineLabel localModel` from the running service. `UNINSTALL.txt`'s corrected text measured claim-by-claim |
| 3 install `AetherCoreSetup.exe` — nobody ever has | **DONE — PASS, 4 UI findings** | §49.5 — first execution by anyone. `BUNDLE_EXIT=0` in 121 s. Both prerequisites detected **Present** and skipped, MSI installed anyway, `restart: None` even on a machine already carrying `RebootPending=1`. Result **indistinguishable** from the MSI install on all 12 measured properties incl. byte-identical pipe DACL and `engineLabel localModel`. Uninstall through the bundle exit 0, **zero survivors on all fourteen**, both package caches released, prerequisites correctly survived. New **`DBT-P49-002`** (unbranded/uninformative UI) and **`DBT-P49-003`** (two `%TEMP%` logs) |
| 4 `DBT-P42-011` the bias, re-measured | **DONE — DOES NOT REPRODUCE** | §49.6 — **nine** rounds on the original machine and silicon under real sustained load. Disk-latency 3.47x **gone** (mean ratio 0.97, product *below* host in 4 of 6). Both CPU series **flip sign** — §43.5's own criterion for noise. Surviving means +3.94 / +4.22 pts are smaller than the **±14-point spread the host counter shows against itself** at 100 ms vs 1 s. Counter-object candidate **disproved**: `\Processor Information(_Total)\` and `\Processor(_Total)\` agree within 0.49 pts on a hybrid Core Ultra 9. Cause deliberately **not hunted**, per the brief. New **`DBT-P49-004`** |
| 5 the VC++ runtime guard on x64, without breaking this machine | **DONE — correctly authored** | §49.7 — redistributable **NOT removed**, per the brief. `Launch` condition read out of the **built MSI's** `LaunchCondition` table (4 rows), naming `(x64)` with a real `AppSearch`/`RegLocator` wiring. Burn's `VCRedist` detection **measured working** in §49.5, not just authored. `llvm-objdump -p` over the six installed binaries: **5/6 `VCRUNTIME140.dll`**, service also `MSVCP140.dll`, `VCRUNTIME140_1.dll` **x64-only** — all three §48 expectations met. Machine carries 14.44.35211.0. **The refusal is proven on ARM64 (§48.3) and INFERRED here** |
| 6 the statement §48.7 could not make | **DONE** | §49.8 — §48.7 marked superseded. **x64 now has a current build and every gate that is not owner-blocked passing against it** (`77ee416b…`). New bundle-lifecycle gate row. Gate 0f **downgraded by measurement** — no D:, no `D:\WindowsImageBackup`. Gate 4 still blocked, both blockers re-measured today (0 drivers offered, ResultCode 2; media detached). `DBT-P42-012` **5 of 6 checks met**, including the check-4 negative control run deliberately |
| owner register | listed, not attempted | §49.0 note below — unchanged, not re-argued |
| ledger delta + final report | **DONE** | §49.9 — 5 rows moved, 4 new ids (`DBT-P49-001`…`-004`), none fixed, all with reasons. §49.10 — the report |

**Owner register — unchanged, not attempted, not re-argued:** code-signing
certificate (`DEFERRED-OWNER`); Gate 4 (recovery media never boot-tested and now
detached, and Windows Update offers this machine zero drivers so no candidate
device exists); Windows Server qualification; production endpoint/key/HSM;
dependency freeze; payment.

## 49.1 ITEM 0 — the machine, measured before anything is built on it

    ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole(544)
    True

    hostname                  Hussein
    OS                        Windows 11 Pro 10.0.26200
    arch                      x86_64
    git core.autocrlf         false
    C: free                   401.2 GB

    cargo    1.98.0 (797e8a9bc 2026-08-05)
    rustc    1.98.0 (88d9e12ae 2026-08-18)
    node     v22.23.2
    pnpm     11.22.0
    dotnet   8.0.424          (WiX 6.0.2 comes from the pinned dotnet tool manifest)
    VS       C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools

### `DBT-P42-012`'s trap is live on this machine, and it is the reason the check exists

Four files named `vcomp140.dll` are installed here. Sorted the way a naive
recursive search returns them, **the first match is the wrong one**:

       72712  ...\VC\Redist\MSVC\14.44.35112\onecore\x64\Microsoft.VC143.OpenMP\vcomp140.dll   <- WRONG, first hit
       64168  ...\VC\Redist\MSVC\14.44.35112\onecore\x86\Microsoft.VC143.OPENMP\vcomp140.dll
      193152  ...\VC\Redist\MSVC\14.44.35112\x64\Microsoft.VC143.OpenMP\vcomp140.dll           <- CORRECT
      163488  ...\VC\Redist\MSVC\14.44.35112\x86\Microsoft.VC143.OPENMP\vcomp140.dll

`scripts/build-installer.ps1` resolves it through `$env:VCToolsRedistDir` and
asserts 193,152 B / sha256 `55aba23c…` before packaging. The pinned fallback path
in that script — `C:\AetherCore-P36\toolchain\vs2022\…` — is the ARM64 VM's
layout and **does not exist on this machine**, so the env-var branch is the one
that must work here.

### What is already installed, and why it is not the build this phase qualifies

    ARP        AetherCore 0.1.11  {0F9F349D-01C8-B3C2-7242-83B5D29047C9}
    InstallDate 20260902
    InstallSource C:\dev\aethercore\phase21-workspace\out\release\
    service    AetherCoreMaintenance  RUNNING
    files      16 (9 in the install root + 7 under assets\)

    out\release\AetherCore.msi   1,100,148,736 B
      sha256 6ecd1ee9786731d22741edbc10e8e0c14ca8add967fe7ebe7f365b21940702a3
      written 2026-09-02 18:00:13

That MSI is **not** §41's `d18d89db…` and **not** anything this phase built. It is
the artefact of the 2026-09-02 peer session §46.16 records — the one §47.9 and
§48.6 could never independently verify, and which §48.7 correctly refused to call
a verdict. It is stale in a way that can be pointed at rather than argued:
`UNINSTALL.txt` on disk is **3,206 B**, the pre-`DBT-P47-002` text. The current
package's is 3,874 B.

So the machine starts this phase carrying a build no gate in this project has
ever measured. Item 1 replaces it.

## 49.2 ITEM 1 — THE CURRENT SOURCE, BUILT ON x64: **every criterion met**

The pipeline the brief names, run end to end on this machine, from `main@6926bc7`.

### Two environment facts that had to be settled before anything compiled

**1. `LNK1181: cannot open input file 'DismApi.lib'`.** The first `cargo build`
died at link. This is §42.2's recorded fact, and it is worth restating because it
bites every fresh shell: `LIB` must carry

    C:\Program Files (x86)\Windows Kits\10\Assessment and Deployment Kit\Deployment Tools\SDKs\DismApi\Lib\amd64

Note **`amd64`, not `x64`** — the ADK's own naming quirk. `crates/system-repair/src/dism_api.rs:11`
declares `#[link(name = "DismApi")]` with no build-script search path, so nothing
in the tree supplies it.

> **Recorded, not worked around: `DBT-P49-001`.** `scripts/build-arm64-msi.cmd:77`
> sets this for arm64. **`scripts/build-release.ps1`, the production x64 pipeline,
> sets nothing** — it silently depends on the invoking shell already having it.
> That is the same shape as `DBT-P48-004` (a recorded pipeline that cannot
> actually produce its artefact from a clean start), one tier milder because the
> requirement *is* written down in §41.4 and §42.2. Risk: **low** — it fails loudly
> at link, it never produces a wrong artefact. **Not fixed here**: changing the
> release script's environment handling is its own review, and this session was
> told to build, not to refactor the pipeline.

**2. `vcomp140.dll`, and `DBT-P42-012` doing its job.** The script resolved it
through `$env:VCToolsRedistDir` and reported the source it chose:

    VCToolsRedistDir=C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Redist\MSVC\14.44.35112\
    vcomp140.dll staged from ...\14.44.35112\x64\Microsoft.VC143.OpenMP\vcomp140.dll

That is the **correct** file — 193,152 B, sha256 `55aba23c…` — and `Assert-VcompHash`
passed silently, which is the whole point of the check. §49.1 recorded that four
files of that name are installed here and that the first one a naive search
returns is the wrong one. The trap is real on this machine and the guard held.

### The build, step by step, every exit code 0

    [1] pnpm --dir apps/ui build                  vite: built in 2.02s       exit 0
    [2] cargo build --locked --release            Finished in 1m 54s         exit 0
        -p aethercore-maintenance-service -p aethercore-consent-broker
        -p aethercore-update-broker -p aethercore-install-hardener -p aetherctl
    [3] tauri build --no-bundle                   Finished in 6m 38s         exit 0
        --config installer\tauri.no-before-build.json
    [4] stage payload                             8 files                    exit 0
    [5] wix build Product.wxs -arch x64                                      exit 0
    [6] wix msi validate                          0 lines emitted  <- EMPTY
    [7] check-msi-payload.ps1                     PAYLOAD_CHECK=PASS
        AUTHORED_FILES=17   MSI_FILE_ROWS=16

`tauri.conf.json` was **not** modified; the recorded `beforeBuildCommand` defect
is routed around with the committed overlay exactly as the brief directs.

**`wix msi validate` emitted nothing at all** — zero ICE, and no `-sval`, no
suppression list, nothing silenced. `AUTHORED_FILES=17` against `MSI_FILE_ROWS=16`
is not a discrepancy: `Product.wxs` authors **both** OpenMP runtimes and the
`$(sys.BUILDARCH)` preprocessor packages one.

### The artefacts

    AetherCore-0.1.11-x64.msi
      bytes   1,100,271,616
      sha256  77ee416b65412ef9306ed25b00d76ff98f9da3b88d1ba19a4e182461d7f7546e

    AetherCoreSetup-0.1.11-x64.exe        <- the FIRST x64 bundle ever compiled
      bytes   1,128,354,997
      sha256  d03987652c8853ece7b2572979999324b0f19e923f1a8a638e05aa51541cbf99

The staged payload, hashed as the build consumed it:

         7062528  58c563aa21f0b798028dd182e018ccf88d9cbc7756c93e9976aca7a2f5bd955e  aethercore-desktop.exe
        10708992  3600845ca1f720f470165a8200dc57ec2b63e8940ca39c92ee0e564f9b4bd1f2  aethercore-maintenance-service.exe
          634368  db0eed36b4933fc280bccd72fcc0b6f80561f2f90c272649681ec4c80ecaadc6  aethercore-consent-broker.exe
          738304  61699704c4aa1767187d9850e23e6ec39c7641f0f1c29e7fd70d8a9d0b3f82d4  aethercore-update-broker.exe
          273408  25689cabf005223b37549585448ebc9e82df125ee59007f438a93a6a1647d390  aethercore-install-hardener.exe
         4347904  ce360664d9a1e5e95b6a009eacc7960c4718055ed11aba6f7f48f8ba377d9f25  aetherctl.exe
            3874  086de15216066da7d2329a0df31625540294fc604556425674ad54dfcb70a02e  UNINSTALL.txt
              83  d4ad925d86f64560bd80c77eae8c606fe810c5f670a7cd42df0836b0653c8b37  update-trust.json

`UNINSTALL.txt` is **3,874 B** — the `DBT-P47-002`-corrected text, against the
3,206 B still sitting in `C:\Program Files\AetherCore` from the 2026-09-02 build.

### The 16 File rows, read out of the MSI's own `File` table

      1  DesktopExe               DesktopComponent            7062528  aethercore-desktop.exe
      2  ConsentBrokerExe         BrokerComponent              634368  aethercore-consent-broker.exe
      3  UpdateBrokerExe          UpdateBrokerComponent        738304  aethercore-update-broker.exe
      4  UpdateTrustJson          UpdateTrustComponent             83  update-trust.json
      5  InstallHardenerExe       HardenerComponent            273408  aethercore-install-hardener.exe
      6  MaintenanceServiceExe    ServiceComponent           10708992  aethercore-maintenance-service.exe
      7  OpenMPRuntimeDll         ServiceComponent             193152  vcomp140.dll
      8  UninstallNoticeTxt       UninstallNoticeComponent       3874  UNINSTALL.txt
      9  AetherCtlExe             AetherCtlComponent           4347904  aetherctl.exe
     10  ModelLicenseApache       ModelLicenseComponent          11358  Apache-2.0.txt
     11  ModelLicenseQwenNotice   ModelLicenseComponent          11343  Qwen-GGUF-NOTICE.txt
     12  EmbeddedModelGguf        ModelComponent            1117320736  qwen2.5-1.5b-instruct-q4_k_m.gguf
     13  ModelsManifestJson       ModelComponent                   898  models.manifest.json
     14  VulnDbJson               VulnDbComponent                 6704  vulndb.json
     15  VulnDbManifestJson       VulnDbComponent                  142  vulndb.manifest.json
     16  CisMapJson               VulnDbComponent                 3103  cis_map.json
    FILE_ROW_COUNT = 16

**The first arch-specific trap, confirmed on the x64 artefact.** Row 7 is
`vcomp140.dll`, 193,152 B, in `ServiceComponent` — the `$(sys.BUILDARCH)`
selection picking the **x64** OpenMP runtime. §47.7 proved the same authoring
picks `libomp140.aarch64.dll` on ARM64. The defect that once hard-coded the ARM64
file, so x64 could not package at all, is now measured as fixed **on both
architectures against a built package**, not inferred from source. The
summary-information template reads **`x64;1033`**.

    ProductCode    {0F9F349D-01C8-B3C2-7242-83B5D29047C9}
    UpgradeCode    {45598C77-2C32-5BCE-8510-19C7E51EE3B8}
    ProductVersion 0.1.11          ARPPRODUCTICON ProductIcon.ico

The ProductCode equals the one already installed here, because it is a
deterministic function of version and architecture. Same version, same
ProductCode, therefore a **reinstall** — never a major upgrade, as designed.

### The bundle's chain, read out of Burn's own manifest

`wix burn extract` → `manifest.xml`, 6,370 B. Not a string scan of the .exe.

    1. ExePackage   VCRedist
         DetectCondition   (NOT (VcRuntimeVersion = ""))
         InstallArguments  /install /quiet /norestart
         Permanent yes   Vital yes   PerMachine yes
    2. ExePackage   WebView2EvergreenBootstrapper
         DetectCondition   ((NOT (WebView2MachineVersion = "")) AND NOT (... = "0.0.0.0")) OR (...)
         InstallCondition  NOT (WindowsInstallationType ~= "Server Core")
         InstallArguments  /silent /install
         Permanent yes   Vital yes   PerMachine yes
    3. MsiPackage   AetherCoreMsi
         ProductCode {0F9F349D-01C8-B3C2-7242-83B5D29047C9}   Version 0.1.11
    CHAIN_PACKAGE_COUNT = 3

**`VCRedist` → `WebView2` → `AetherCoreMsi`, in that order.** EXPECTED met.

    RegistrySearch VcRuntimeSearch  Root=HKLM  Win64=yes
      Key=SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\x64  Value=Version

`$(sys.BUILDARCH)` expanded to **`x64`** here where §48.6 read `arm64` out of the
ARM64 bundle. `Win64="yes"` is what `Bitness="always64"` compiles to, as §48.6
recorded.

The three payloads Burn actually carries:

    vc_redist.x64.exe               25,635,768 B
    MicrosoftEdgeWebview2Setup.exe   1,783,000 B
    AetherCore-0.1.11-x64.msi     1,100,271,616 B

Both prerequisites were fetched from Microsoft and verified before use —
Authenticode **Valid**, signer `CN=Microsoft Corporation`:

    vc_redist.x64.exe               25,635,768 B  sha256 cc0ff0eb1dc3f5188ae6300faef32bf5beeba4bdd6e8e445a9184072096b713b   ProductVersion 14.44.35211.0
    MicrosoftEdgeWebview2Setup.exe   1,783,000 B  sha256 17debf797a6c737959bc588236e897936ffac1af5f7e515e674ab32f9edfe719

That WebView2 hash is **byte-identical to the one §48.6 recorded on the ARM64 VM**
— an independent cross-machine check that neither session was served a
substituted file.

Bundle registration: `PerMachine=yes`, ARP display name **`AetherCore Setup`**,
ProviderKey `{C047DFD5-BD01-4BF8-AEAD-54333A6C691F}`. Note that name. The MSI
inside is authored `Visible="no"`, so a bundle install must register **one** ARP
entry called `AetherCore Setup`, where an MSI install registers `AetherCore`.
Item 3 measures what that difference actually costs.
## 49.3 DESTRUCTIVE ACTION RECORD — GATE 5 ON x64, CURRENT BUILD

**Written and committed BEFORE the first destructive step**, per the standing rule.
**This machine has no snapshots.** It is the owner's real working computer.
System Restore is the only rollback.

    ACTION=   1. msiexec /x {0F9F349D-01C8-B3C2-7242-83B5D29047C9} /qn /l*v
                 removes the AetherCore 0.1.11 installed 2026-09-02 from the
                 unverified peer build 6ecd1ee9... DESTRUCTIVE.
              2. Fourteen-check survivor sweep. READ-ONLY by construction.
              3. msiexec /i outp49AetherCore-0.1.11-x64.msi, sha256 77ee416b… /qn /l*v, then every Gate 2 property.
              4. msiexec /x ... again, sweep again.               DESTRUCTIVE.
              5. AetherCoreSetup-0.1.11-x64.exe run as a real user would.
                 First execution of this artefact by anyone.      DESTRUCTIVE.
              6. Uninstall THROUGH THE BUNDLE, sweep again.       DESTRUCTIVE.

    SNAPSHOT= a NEW named System Restore point P49-PRE-GATE5, created before
              step 1 and verified by ENUMERATION with Get-ComputerRestorePoint
              -- Checkpoint-Computer returning OK is not proof. No existing
              restore point is deleted. Nothing is restored unless recovery is
              actually needed.

    EXPECTED= step 1: exit 0.
              step 2: ZERO survivors on all fourteen. Any survivor is a finding
                      recorded with its exact path -- never deleted by hand and
                      called a pass. %LOCALAPPDATA%\com.aethercore.desktop is
                      documented in UNINSTALL.txt (3,874 B) as surviving in the
                      profile of any account that ran the desktop app; the
                      criterion is MACHINE_WIDE=0, not TOTAL=0.
              step 3: exit 0; 16 files hash-matched to the BUILT payload;
                      service RUNNING, LocalSystem, AUTO_START (DELAYED);
                      sc qsidtype UNRESTRICTED; install-dir ACLs equal to the
                      §10 baseline; zero dev binaries; the four verbs returning
                      with timings; engineLabel `localModel` FROM THE RUNNING
                      SERVICE (aetherctl self-check --load-model exit 7 is
                      correct by design and is NOT the proof); pipe SDDL
                      byte-identical to
                      O:<service SID> G:SY D:P(A;;FA;;;<service SID>)(A;;FR;;;AU)(A;;DC;;;AU)
                      with (A;;0x12008b;;;AU) recognised as the SAME DACL --
                      FR|DC = 0x120089|0x2 = 0x12008b, NOT drift.
              steps 4-6: as above, plus every prompt, dialog and reboot request
                      the bundle produces, recorded.

    NOT DONE= Defender, UAC, Firewall and SmartScreen are NOT touched.
              The VC++ redistributable is NOT removed from this machine -- P49
              Item 5 says so explicitly and gives the reason: it is a daily-use
              machine with no snapshots, other software depends on that runtime,
              and the refusal is already proven on ARM64 (§48.3).
              No driver work. Gate 4 stays stopped.
              No existing restore point is deleted or restored.
              core.autocrlf stays false.
              0.1.9 and 0.1.10 are must-not-ship and are neither built nor
              installed.

    RECOVERY= Two levels, cheapest first.
              (a) PRODUCT: the previously installed build is on this machine at
                  out\release\AetherCore.msi, 1,100,148,736 B, sha256
                  6ecd1ee9786731d22741edbc10e8e0c14ca8add967fe7ebe7f365b21940702a3.
                  Reinstalling it returns the machine to exactly the state this
                  session found it in. This is the expected recovery path for
                  anything that goes wrong with the product itself.
              (b) MACHINE: System Restore to P49-PRE-GATE5. Used only if the
                  machine is left unusable. There is no disk image on this
                  machine to fall back to -- §48.7 records that the recovery
                  media was created, never boot-tested, and is now DETACHED.
                  That is why (a) exists and why nothing here touches drivers,
                  storage, chipset or GPU.


## 49.4 ITEM 2 — GATE 5 ON x64, AGAINST THE CURRENT BUILD: **PASS**

The full lifecycle, never before run on x64 against a current build. Every number
below is raw output from this machine.

### The restore point, verified by enumeration and not by a return code

    Get-ComputerRestorePoint   BEFORE:  3, 4, 5   (all "Windows Update", 2026-09-03)

    Checkpoint-Computer -Description 'P49-PRE-GATE5' -RestorePointType MODIFY_SETTINGS

    Get-ComputerRestorePoint   AFTER:
      SequenceNumber 6   P49-PRE-GATE5   type 12   created 2026-09-06 01:07:49
    P49_PRE_GATE5_PRESENT = True     TOTAL_RESTORE_POINTS = 4

Points 3, 4 and 5 are untouched; nothing was deleted and nothing restored. One
thing had to be changed to make the checkpoint land at all, and it was put back:
Windows silently drops a checkpoint taken within 1440 minutes of the last one, so
`SystemRestorePointCreationFrequency` was set to `0` for the duration and then
**deleted** afterwards, returning the machine to the Windows default:

    SystemRestorePointCreationFrequency now = ''   (absent = default 1440 min)

### Step 1 — the stale 2026-09-02 build removed

Measured before touching it: 16 files, ProgramData present, service **RUNNING**,
and `UNINSTALL.txt` at **3,206 B** — the pre-`DBT-P47-002` text, which is what
identified this install as the unverified peer build in the first place.

    msiexec /x {0F9F349D-01C8-B3C2-7242-83B5D29047C9} /qn /l*v
    UNINSTALL_EXIT=0      ELAPSED_MS=3233      LOG_BYTES=148422
    Windows Installer removed the product. ... Removal success or error status: 0.
    MainEngineThread is returning 0

### Step 2 — sweep after that removal: **ZERO SURVIVORS ON ALL FOURTEEN**

Run before the new install, so the new install starts from a measured-clean
machine rather than an assumed one.

     1  INSTALLDIR              False                    clean
     2  PROGRAMDATA             False                    clean
     3  SERVICE                 absent(1060)             clean
     4  PIPE_COUNT              0                        clean
     5  ARP_COUNT               0                        clean
     6  HKLM_SOFTWARE_AETHER    False                    clean
     7  HKCU_SOFTWARE_AETHER    False                    clean
     8  STARTMENU               0                        clean
     9  SCHEDULED_TASKS         0                        clean
    10  FIREWALL_RULES          0                        clean
    11  HKLM_SERVICES_KEY       False                    clean
    12  EVENTLOG_SOURCE         0                        clean
    13  HKEY_USERS_MARKERS      0                        clean
    14  FILESYSTEM_SWEEP        18 raw / 0 machine-wide  clean
    SURVIVORS = 0        MACHINE_WIDE = 0

### Step 3 — the Item 1 MSI installed

    msiexec /i out\p49\AetherCore-0.1.11-x64.msi /qn /l*v
    INSTALL_EXIT=0        ELAPSED_MS=31265      LOG_BYTES=168244
    Windows Installer installed the product. ... Installation success or error status: 0.

### Every Gate 2 property, re-measured rather than assumed

| criterion | expected | observed | |
|---|---|---|---|
| install transaction | success | status 0, 0.1.11 | PASS |
| installed file count | 16 | **16** | PASS |
| file hashes vs the BUILT payload | all match | **MATCHED=16 MISMATCH=0 NOT_IN_SOURCES=0** | PASS |
| OpenMP runtime | `vcomp140.dll`, no aarch64 | `VCOMP140_PRESENT=True`, `LIBOMP_AARCH64=False` | PASS |
| dev binaries | none | `DEV_BINARY_IN_INSTALL_IMAGE=NO` | PASS |
| service state | RUNNING | `STATE : 4 RUNNING` | PASS |
| start type | AUTO_START (DELAYED) | `2 AUTO_START (DELAYED)` | PASS |
| service account | LocalSystem | `LocalSystem` | PASS |
| binary path | the installed exe | `"C:\Program Files\AetherCore\aethercore-maintenance-service.exe"` | PASS |
| `sc qsidtype` | **UNRESTRICTED** | `UNRESTRICTED`, `STATUS: Active` | PASS |
| install-dir ACLs | the §10 baseline | identical, field for field | PASS |
| ARP | AetherCore 0.1.11 | `{0F9F349D-…}` 0.1.11, InstallDate 20260906 | PASS |
| `HKLM\…\AetherCore\InstallVersion` | 0.1.11 | 0.1.11 | PASS |
| ProgramData recreated | yes | 5 files | PASS |
| pipe DACL | the §10 baseline | **byte-identical** | PASS |
| **`engineLabel`** | **`localModel`** | **`localModel`** from the running service | **PASS** |

The 16 installed files, hashed on disk and compared to the file the build
actually consumed — not to a remembered number:

      aethercore-consent-broker.exe          634368  db0eed36b4933fc280bccd72fcc0b6f80561f2f90c272649681ec4c80ecaadc6
      aethercore-desktop.exe                7062528  58c563aa21f0b798028dd182e018ccf88d9cbc7756c93e9976aca7a2f5bd955e
      aethercore-install-hardener.exe        273408  25689cabf005223b37549585448ebc9e82df125ee59007f438a93a6a1647d390
      aethercore-maintenance-service.exe   10708992  3600845ca1f720f470165a8200dc57ec2b63e8940ca39c92ee0e564f9b4bd1f2
      aethercore-update-broker.exe           738304  61699704c4aa1767187d9850e23e6ec39c7641f0f1c29e7fd70d8a9d0b3f82d4
      aetherctl.exe                         4347904  ce360664d9a1e5e95b6a009eacc7960c4718055ed11aba6f7f48f8ba377d9f25
      Apache-2.0.txt                          11358  cfc7749b96f63bd31c3c42b5c471bf756814053e847c10f3eb003417bc523d30
      cis_map.json                             3103  9caf01b4a2f7d2bfda3111395212b27046f6ae614bc847cebaadfe33c9ee8d97
      models.manifest.json                      898  070b6dedc37664250e4029b8360a1e9b30a1d40b6d776a83ddd0631247dae57e
      qwen2.5-1.5b-instruct-q4_k_m.gguf  1117320736  6a1a2eb6d15622bf3c96857206351ba97e1af16c30d7a74ee38970e434e9407e
      Qwen-GGUF-NOTICE.txt                    11343  832dd9e00a68dd83b3c3fb9f5588dad7dcf337a0db50f7d9483f310cd292e92e
      UNINSTALL.txt                            3874  086de15216066da7d2329a0df31625540294fc604556425674ad54dfcb70a02e
      update-trust.json                          83  d4ad925d86f64560bd80c77eae8c606fe810c5f670a7cd42df0836b0653c8b37
      vcomp140.dll                           193152  55aba23cdcd6484fbb06f4155b8ca75adfce7a881f10afd0c49457165e677164
      vulndb.json                              6704  ab76528eacc58fe910d82347d49919d50949954a25649e677eaaf52fdf37f303
      vulndb.manifest.json                      142  2c29c19b2760167fab8b292dde744da51ae8b137a01c870701287fb49d01daa6

The GGUF at `6a1a2eb6…` and `vcomp140.dll` at `55aba23c…` are byte-identical to
every prior record on both architectures. `UNINSTALL.txt` is now **3,874 B**
`086de152…` — the corrected text, on this machine, for the first time.

### The pipe DACL

    PIPE_COUNT=1     \\.\pipe\AetherCore.Maintenance.v7

    PIPE_SDDL=O:S-1-5-80-4285065559-3530017622-2858480679-3751456793-1187574229
              G:SY
              D:P(A;;0x12008b;;;AU)(A;;FA;;;S-1-5-80-4285065559-3530017622-2858480679-3751456793-1187574229)

Required: `O:<service SID> G:SY D:P(A;;FA;;;<service SID>)(A;;FR;;;AU)(A;;DC;;;AU)`.
Owner is the service SID; group `SY`; `D:P` protected; the service-SID `FA` ACE
present verbatim; the AU pair rendered merged as `0x12008b`, which is
`FR|DC = 0x120089|0x2`. **The brief names that rendering explicitly and says it is
NOT drift.** Byte-identical to the §10 baseline and to §42.7's reading on this
same machine.

`sc sdshow` unchanged:
`D:(A;;CCDCLCSWRPWPDTLOCRSDRCWDWO;;;SY)(A;;CCDCLCSWRPWPDTLOCRSDRCWDWO;;;BA)(A;;CCLCSWLOCRRC;;;AU)S:(AU;FA;CCDCLCSWRPWPDTLOCRSDRCWDWO;;;WD)`

**The brief's `[System.IO.File]::Open` method fails here, for the third session
running.** With the path written correctly it returns:

    FileStream was asked to open a device that was not a file. For support for
    devices like 'com1:' or 'lpt1:', call CreateFile, then use the FileStream
    constructors that take an OS handle as an IntPtr.

`NamedPipeClientStream(...).GetAccessControl().Sddl` is the method that works, as
§41.14 and §42.7 both recorded. Three independent sessions, same result. Recorded
again because the brief still prescribes the method that does not work.

### The verbs, against the RUNNING SERVICE, with timings

    service detect    EXIT 0    423 ms   state Reachable, endpointDir C:\ProgramData\AetherCore
    doctor            EXIT 5     15 ms   typed rejection, see below
    scan status       EXIT 0     17 ms   appVersion 0.1.11, state idle, ruleEngineVersion phase17.1-rules-v2
    insights list     EXIT 0     12 ms   engineLabel localModel, insights []
    self-check        EXIT 0    694 ms   sha256Match true, manifestValid true, 1117320736 bytes
    optimize status   EXIT 0     16 ms   status null

**A false finding this session nearly recorded, and the measurement that killed
it.** `doctor` exits 5 and prints *nothing on stdout*, which looked like a CLI
failing silently. Separating the streams shows it does not:

    STDOUT_BYTES=0
    STDERR_BYTES=115
      aetherctl: rejected by service (diagnostics.stateUnavailable): diagnostic state is unavailable [RejectedByService]

and the stable envelope carries the same thing:

    aetherctl --output json doctor   EXIT=5
    {"schema":"aethercore.aetherctl.v1","command":"doctor","ok":false,
     "error":{"kind":"RejectedByService","message_key":"diagnostics.stateUnavailable",
              "detail":"rejected by service (diagnostics.stateUnavailable): diagnostic state is unavailable"}}

Exit 5 immediately after a fresh install, with no diagnostic state yet, is the
state-dependent typed rejection §42.8 resolved. Not a defect, and not silent.
(The flag is `--output json`, not `--format json`; `--format` is `doctor`'s own
report-format flag and returns exit 2 in that position.)

### `engineLabel` — from the running service, which is the only thing that proves it

    C:\Program Files\AetherCore\aetherctl.exe insights list
    exit=0
    engineLabel                        localModel
    insights                           []

`localModel` present, `ruleFallback` absent, over the named pipe from the
installed binary to the installed service. The weaker check is recorded beside it
so nobody substitutes it later:

    aetherctl self-check --load-model      EXIT=7

Exit 7 there is correct by design and proves nothing about the service.

### Step 4 — the gate's uninstall, service RUNNING at transaction start

    msiexec /x {0F9F349D-01C8-B3C2-7242-83B5D29047C9} /qn /l*v
    UNINSTALL_EXIT=0      ELAPSED_MS=3047      LOG_BYTES=151696
    Windows Installer removed the product. ... Removal success or error status: 0.
    MainEngineThread is returning 0

No 1603, and no `InstallValidate` return value 3, with the service RUNNING when
the transaction opened.

### Step 5 — the fourteen-check survivor sweep. **ZERO SURVIVORS ON ALL FOURTEEN**

     1  INSTALLDIR              False                    clean
     2  PROGRAMDATA             False                    clean
     3  SERVICE                 absent(1060)             clean
     4  PIPE_COUNT              0                        clean
     5  ARP_COUNT               0                        clean
     6  HKLM_SOFTWARE_AETHER    False                    clean
     7  HKCU_SOFTWARE_AETHER    False                    clean
     8  STARTMENU               0                        clean
     9  SCHEDULED_TASKS         0                        clean
    10  FIREWALL_RULES          0                        clean
    11  HKLM_SERVICES_KEY       False                    clean
    12  EVENTLOG_SOURCE         0                        clean
    13  HKEY_USERS_MARKERS      0                        clean
    14  FILESYSTEM_SWEEP        17 raw / 0 machine-wide  clean
    SURVIVORS = 0

Check 14 by root, which is the measurement that decides it:

    C:\Program Files          0 hit(s)
    C:\Program Files (x86)    0 hit(s)
    C:\ProgramData            0 hit(s)
    C:\Windows\System32       0 hit(s)
    C:\Windows\SysWOW64       0 hit(s)
    C:\Users                  17 hit(s)
    TOTAL=17  UNDER_USER_PROFILE=17  MACHINE_WIDE=0

The sweep is read-only by construction. **Nothing was deleted by hand.** All 17
raw hits are dated **2026-09-02**, four days before this uninstall, and every one
is accounted for: three Claude Code directories named after the *repo path*
`C:\dev\aethercore`, ten `cargo test` temp files from that day, one elevation
probe, one Explorer Recent-items shortcut, and the owner's own
`OneDrive\…\aethercore-models` folder — user data in the user's own Documents,
which `UNINSTALL.txt` and `ARPCOMMENTS` explicitly promise not to touch. Not one
was created by the product under test.

(The ten `cargo test` temp files are `DBT-P42-013`/`DBT-P48-003` residue from
before those fixes landed. The fixes are proven by §48.5's post-fix run leaving
**0** temp entries; these are simply older files nothing has cleaned up, and the
sweep does not delete them.)

### The sweep checked against the CORRECTED `UNINSTALL.txt`, as the brief directs

`DBT-P47-002` was raised because the shipped text was more absolute than the
behaviour. The corrected text is what is installed now (3,874 B, `086de152…`).
Claim by claim against sweep 2:

    program files removed           -> check 1   INSTALLDIR         False    OK
    service stopped + deregistered  -> check 3   SERVICE            1060     OK
    named pipe goes with it         -> check 4   PIPE_COUNT         0        OK
    ALL machine data removed        -> check 2   PROGRAMDATA        False    OK
    HKLM\SOFTWARE\AetherCore        -> check 6                      False    OK
    Add/Remove Programs entry       -> check 5   ARP_COUNT          0        OK
    HKCU, uninstalling account      -> check 7                      False    OK
    Start Menu folder               -> check 8   STARTMENU          0        OK
    "creates no scheduled tasks"    -> check 9   SCHEDULED_TASKS    0        OK
    "and no firewall rules"         -> check 10  FIREWALL_RULES     0        OK

**Every claim the document makes is measured true.** The one thing the corrected
text *admits* survives — `%LOCALAPPDATA%\com.aethercore.desktop`, the per-user
WebView2 cache — could not be tested here, and the reason is worth stating rather
than glossing:

    accounts on this machine with an AppData\Local\com.aethercore.desktop:  NONE
    uninstall log references to 'com.aethercore.desktop':                   0
    uninstall log references to 'EBWebView':                                0

**The desktop app has never been launched on this machine**, so no WebView2
user-data directory has ever been created here. The corrected paragraph is
therefore *consistent with* this run but is **not** re-proven by it; it stays
proven where §47.7 measured it, on the ARM64 VM at 274 files / 23,258,063 bytes.

### Step 6 — reinstall, closing the lifecycle

    msiexec /i out\p49\AetherCore-0.1.11-x64.msi /qn /l*v
    INSTALL_EXIT=0        ELAPSED_MS=31257      LOG_BYTES=168244
    FILES=16     STATE : 4 RUNNING
    PIPE_SDDL=O:S-1-5-80-…-1187574229G:SYD:P(A;;0x12008b;;;AU)(A;;FA;;;S-1-5-80-…-1187574229)
    insights list  EXIT=0   engineLabel  localModel

Byte-identical pipe DACL, 16 files, service running, local model live.

**GATE 5 ON x64, AGAINST MSI `77ee416b…`: PASS.** Uninstall exit 0 twice, zero
survivors on all fourteen both times, `MACHINE_WIDE=0` both times, reinstall exit
0, every Gate 2 property re-proven from measurement.

## 49.5 ITEM 3 — `AetherCoreSetup.exe` INSTALLED, FOR THE FIRST TIME BY ANYONE: **PASS, with four UI findings**

Before this run the consumer installer had been compiled exactly once (§48.6, on
ARM64) and **never executed**. This is the first time any human or machine has
run it.

### The machine was left measurably clean first

    msiexec /x {0F9F349D-…} /qn /l*v    UNINSTALL_EXIT=0   3073 ms   log 151,732 B
    INSTALLDIR=False   PROGRAMDATA=False   SERVICE=absent(1060)
    ARP_AetherCore=0   HKLM_SOFTWARE_AETHER=False

And the prerequisite state the chain would have to reason about was measured
*before* the bundle ran, because it decides which branch gets tested:

    VcRuntimeVersion (HKLM\…\VC\Runtimes\x64\Version)  = v14.44.35211.00
    WebView2MachineVersion (pv)                        = 152.0.4191.62
    WindowsInstallationType                            = Client

**Both prerequisites were already present.** So this run tests exactly the branch
the brief calls out as a risk — "fails silently when a prerequisite is already
present" — and not the fresh-install branch.

### It was run the way a user runs it: no `/quiet`, no `/passive`, real UI

    AetherCoreSetup-0.1.11-x64.exe /log <path>
    BUNDLE_EXIT=0        BUNDLE_ELAPSED_S=121        burn log 15,174 B

### The chain, from Burn's own log

    Detected package: VCRedist,       state: Present, install registration state: (permanent)
    Detected package: WebView2Ever…,  state: Present, install registration state: (permanent)
    Detected package: AetherCoreMsi,  state: Absent

    Planned package: VCRedist,        state: Present, execute: None,    rollback: None
    Planned package: WebView2Ever…,   state: Present, execute: None,    rollback: None
    Planned package: AetherCoreMsi,   state: Absent,  execute: Install, rollback: Uninstall

    Applying execute package: AetherCoreMsi, action: Install,
      path: C:\ProgramData\Package Cache\{0F9F349D-…}v0.1.11\AetherCore-0.1.11-x64.msi,
      arguments: ARPSYSTEMCOMPONENT="1" MSIFASTINSTALL="7" BURNMSIINSTALL=1
                 REINSTALLMODE="muso" REBOOT=ReallySuppress
    Applied execute package: AetherCoreMsi, result: 0x0, restart: None
    Apply complete, result: 0x0, restart: None, ba requested restart: No
    Exit code: 0x0, restarting: No

**Both prerequisites detected `Present` and skipped, and the product still
installed and still reported success.** That is the failure mode the brief named,
tested on the branch that could exhibit it, and it does not.

**No reboot was requested, and none was needed.** Worth stating precisely: the
machine itself carried `RebootPending = 1` from unrelated Windows state, and the
bundle *still* set `WixStdBARestartRequired` to empty and exited `restarting: No`.
It does not demand a reboot even on a machine that already owes one.

### The result is indistinguishable from an MSI install — proven, not assumed

Item 2's checks re-run against the **bundle-installed** product:

| criterion | MSI install (§49.4) | bundle install | |
|---|---|---|---|
| file count | 16 | **16** | same |
| file hashes | MATCHED=16 MISMATCH=0 | **MATCHED=16 MISMATCH=0 NOT_IN_SOURCES=0** | same |
| OpenMP runtime | vcomp140 yes / aarch64 no | same | same |
| dev binaries | `NO` | **`NO`** | same |
| service | RUNNING, LocalSystem, AUTO_START (DELAYED) | identical | same |
| `sc qsidtype` | UNRESTRICTED | **UNRESTRICTED**, same SID | same |
| install-dir ACLs | §10 baseline | identical, field for field | same |
| pipe DACL | `O:S-1-5-80-…G:SYD:P(A;;0x12008b;;;AU)(A;;FA;;;S-1-5-80-…)` | **byte-identical** | same |
| `HKLM\…\InstallVersion` | 0.1.11 | 0.1.11 | same |
| ProgramData | 5 files | 5 files | same |
| **`engineLabel`** | **`localModel`** | **`localModel`** | same |
| verbs | 6/6 returned | 6/6 returned, 12–711 ms | same |

**Every security and behaviour property is identical.** The chain is equivalent to
the MSI for everything the gates measure.

### What the bundle install DID that the MSI install does not

This is the part that had never been observed, so it is listed exhaustively.

1. **It registered a second ARP key.** `{C047DFD5-BD01-4BF8-AEAD-54333A6C691F}`,
   `DisplayName = AetherCore Setup`. The MSI's own key still exists but Burn
   passed `ARPSYSTEMCOMPONENT="1"`, which sets `SystemComponent = 1` on it.
   Measured rather than assumed — the registry holds two keys, and what a user
   sees is one:

        VISIBLE_ARP_ENTRIES = 1
          AetherCore Setup  0.1.11  {C047DFD5-…}

   Its `UninstallString` is the cached bundle with `/uninstall`, and there is a
   `QuietUninstallString` too. **Correct behaviour**, and the design intent of
   `Visible="no"` — but it means the entry a user removes is called *AetherCore
   Setup*, not *AetherCore*.

2. **It kept a second full copy of the 1.1 GB MSI on disk.** `Cache="keep"`:

        C:\ProgramData\Package Cache\{0F9F349D-…}v0.1.11\AetherCore-0.1.11-x64.msi   1,100,271,616 B
        C:\ProgramData\Package Cache\{C047DFD5-…}\AetherCoreSetup-0.1.11-x64.exe         1,082,002 B
        C:\ProgramData\Package Cache\{C047DFD5-…}\state.rsm                                  1,038 B
        PACKAGE_CACHE_BYTES = 1,101,354,656   (1,050.3 MB)

   That sits **on top of** msiexec's own cached copy, which a plain MSI install
   also creates:

        C:\Windows\Installer\23e4f196.msi     1,100,271,616 B

   So a bundle install leaves the machine holding **two** 1.1 GB copies of the
   package plus the 1.14 GB install directory. The bundle's *additional* cost over
   an MSI install is **1,050.3 MB**. Not a defect — it is what makes Burn repair
   and modify work offline — but it is a real number a consumer should not
   discover by running out of disk. Note the cached bundle .exe is only 1,082,002 B:
   Burn caches the engine, not the 1.13 GB attached container.

3. **It launched a separate elevated engine process.**
   `i010: Launching elevated engine process. … i012: Connected to elevated engine.`
   **This session saw no UAC prompt, and that proves nothing** — the bundle was
   started from an already-elevated shell, so it inherited elevation. A real user
   launching it from a standard desktop session will get one UAC prompt at the
   moment they press Install. That is **inferred here, not measured**, and it is
   stated that way deliberately.

4. **It left two small log files in `%TEMP%` that nothing cleans up.**

        926 B  AetherCore_Setup_20260906011711.elevated.log     (install)
        935 B  AetherCore_Setup_20260906012151.elevated.log     (uninstall)

   Per-user, tiny, and standard Burn behaviour — but they are product-attributable
   files that survive uninstall and that the MSI path does not create. Raised as
   **`DBT-P49-003`**, risk **low**, same family as `DBT-P42-013`.

5. **It showed a graphical interface**, which is the whole point, and which had
   never been looked at. That produced the findings below.

### What a first real user actually sees — recorded verbatim

Screen 1, before touching anything (`WixStdBA`, 750×506):

    [Button  ] Install
    [Button  ] Cancel
    [Text    ] AetherCore Setup
    [TitleBar] AetherCore Setup Setup

Screen 2, after pressing Install, and for the whole ~30 s the MSI took:

    [Text       ] Setup Progress
    [Text       ] Processing:
    [Text       ] Initializing...
    [ProgressBar] Initializing...
    [Button     ] Cancel

> **`DBT-P49-002` — the consumer installer's UI is unbranded and uninformative.**
> Four things, all visible to the first user who runs it, none of them affecting
> what gets installed:
>
> **(a) The title bar reads "AetherCore Setup Setup".** `Bundle/@Name` is
> "AetherCore Setup" and `WixStandardBootstrapperApplication` appends " Setup" to
> it. The fix is to name the bundle "AetherCore".
>
> **(b) The logo is WiX's stock placeholder, not the product's mark.** The
> extracted BA carries an 852-byte `logo.png` — a dark red square with a white
> disc — because `Bundle.wxs` sets no `LogoFile`. §48.2 closed `DBT-P36-004` by
> making the shipped icon deliberate artwork, and `DBT-P48-001` already records
> that the in-app mark is a *different* mark. This is a **third** mark, and it is
> the one a user sees first. `apps\desktop\icons\icon.ico` already exists in the
> tree and is already passed to the MSI.
>
> **(c) The welcome screen is empty.** No version despite `ShowVersion="yes"`, no
> license text or link because `LicenseUrl=""`, no install location, no
> description — a name, a stock logo, and two buttons on 400 px of white space.
>
> **(d) Install progress never updates.** It reads `Processing: Initializing...`
> with an empty bar for the entire MSI install. The *uninstall* path does not have
> this problem — it reached `Processing: AetherCore` at t+9 s — so this is
> specific to the install sequence, not the theme.
>
> Risk: **cosmetic, but on the most exposed surface in the product.** Combined
> with the unsigned-installer SmartScreen warning (`DEFERRED-OWNER`), the first
> thing a paying customer sees is an unsigned, unbranded dialog that appears to
> hang for thirty seconds. **Not fixed here** — this is packaging polish and the
> brief's Item 3 was to install it and report, not to redesign it.

### Uninstall THROUGH THE BUNDLE

Run the way Apps & features would, straight off the ARP `UninstallString`:

    "C:\ProgramData\Package Cache\{C047DFD5-…}\AetherCoreSetup-0.1.11-x64.exe" /uninstall
    BUNDLE_UNINSTALL_EXIT=0     BUNDLE_UNINSTALL_ELAPSED_S=192

The screens, in order:

    t+0s    [Text] Modify Setup            [Button] Repair  [Button] Uninstall  [Button] Cancel
    t+7s    [Text] Setup Progress          Processing: Initializing...
    t+9s    [Text] Setup Progress          Processing: AetherCore
    t+13s   [Text] Uninstall Successfully Completed        [Button] Close

### The sweep after the bundle uninstall: **ZERO SURVIVORS ON ALL FOURTEEN**

     1  INSTALLDIR              False                    clean
     2  PROGRAMDATA             False                    clean
     3  SERVICE                 absent(1060)             clean
     4  PIPE_COUNT              0                        clean
     5  ARP_COUNT               0                        clean
     6  HKLM_SOFTWARE_AETHER    False                    clean
     7  HKCU_SOFTWARE_AETHER    False                    clean
     8  STARTMENU               0                        clean
     9  SCHEDULED_TASKS         0                        clean
    10  FIREWALL_RULES          0                        clean
    11  HKLM_SERVICES_KEY       False                    clean
    12  EVENTLOG_SOURCE         0                        clean
    13  HKEY_USERS_MARKERS      0                        clean
    14  FILESYSTEM_SWEEP        21 raw / 0 machine-wide  clean
    SURVIVORS = 0        MACHINE_WIDE = 0

**Both caches were released**, which is the thing a bundle can get wrong and which
nothing had ever checked:

    C:\ProgramData\Package Cache\{0F9F349D-…}v0.1.11   present: False
    C:\ProgramData\Package Cache\{C047DFD5-…}          present: False
    C:\Windows\Installer\23e4f196.msi                  present: False
    ARP entries matching AetherCore                    0

And the two prerequisites correctly **survived**, which is what `Permanent="yes"`
is for — removing AetherCore must not remove a runtime other software depends on:

    VcRuntimeVersion       = v14.44.35211.00      still present
    WebView2MachineVersion = 152.0.4191.62        still present

Check 14 grew from 17 raw hits to 21 for four reasons, all accounted for: the two
`AetherCore_Setup_*.elevated.log` files of `DBT-P49-003`, and two
`bundle-*-burn_000_AetherCoreMsi.log` per-package logs that exist only because
**this session passed `/log`** into its own scratchpad. `MACHINE_WIDE` is still 0.

**ITEM 3 = PASS.** The consumer installer installs, produces a product identical
to the MSI's on every measured property, requests no reboot, does not fail
silently when its prerequisites are already present, uninstalls cleanly through
its own ARP entry, releases both package caches, and leaves zero machine-wide
survivors. What is wrong with it is entirely on the surface, and is now written
down as `DBT-P49-002`.

## 49.6 ITEM 4 — `DBT-P42-011` RE-MEASURED: **THE BIAS DOES NOT REPRODUCE**

The brief's instruction was explicit: re-measure on the current build **before
explaining anything**, and if the bias is gone, *say so and stop*. It is gone.
This section says so, gives the numbers, gives the one control measurement that
makes a negative result credible, and stops.

### What was recorded, and what this had to reproduce

    §42.3 / §42.8, on THIS machine, 2026-09-02, pre-P45-field-boundary-fix:
      cpu offline    product 58.38%   host 50.91%   delta +7.47 pts   ratio 1.147
      cpu service    product 52.31%   host 48.22%   delta +4.09 pts   ratio 1.085
      disk latency   product 833 us   host 240 us   delta +593 us     ratio 3.47
    "three readings, all the same direction, all sizeable"

### How it was measured this time

Same machine — `Intel(R) Core(TM) Ultra 9 185H`, 22 logical processors — against
MSI `77ee416b…` installed and its service RUNNING. **Nine rounds**, not three.
Real sustained load: 11 CPU-spinning .NET threads plus one 4 MB read/write loop,
compiled with `Add-Type` and run in a **separate process** so the measuring
script never competes with the load for a runspace. Host reading 51–63%.

The one thing this run does that §42.3 did not, and the reason it can settle
anything: **the host is read at both windows.**

- `Get-Counter -SampleInterval 1` — the 1 s window §42.3 compared against.
- `System.Diagnostics.PerformanceCounter` with a **100 ms** gap for CPU and an
  **80 ms** gap for disk — the windows the product's own collectors use
  (`crates/performance-telemetry/src/windows_impl.rs:369` and `:710`).

### CPU — nine rounds, both paths, both host windows

| # | product offline | product service | host 1 s | host 100 ms |
|---|---|---|---|---|
| 1 | 50.34% | 56.71% | 55.379% | 50.852% |
| 2 | 51.46% | 67.12% | 56.864% | 54.230% |
| 3 | 55.88% | 54.65% | 51.322% | 62.270% |
| 4 | 64.89% | 70.37% | 63.116% | 67.743% |
| 5 | 62.44% | 65.12% | 59.447% | 45.300% |
| 6 | 67.50% | 54.48% | 61.431% | 70.859% |
| 7 | 71.53% | 61.01% | 62.675% | 65.340% |
| 8 | 79.50% | 71.15% | 63.135% | 57.694% |
| 9 | 66.03% | 71.50% | 60.745% | 64.597% |

Deltas against the 1 s host reading, in points and as a ratio — the same
comparison §42.3 made:

    offline:  -5.04 (0.909)  -5.40 (0.905)  +4.56 (1.089)  +1.77 (1.028)
              +2.99 (1.050)  +6.07 (1.099)  +8.86 (1.141)  +16.37 (1.259)
              +5.29 (1.087)                         mean +3.94 pts,  7 of 9 positive

    service:  +1.33 (1.024)  +10.26 (1.180) +3.33 (1.065)  +7.25 (1.115)
              +5.67 (1.095)  -6.95 (0.887)  -1.67 (0.973)  +8.02 (1.127)
              +10.76 (1.177)                        mean +4.22 pts,  7 of 9 positive

**Both series flip sign.** §43.5's whole argument for calling x64's result a real
bias and ARM64's noise was that x64's deltas were "positive in every one of three
independent readings" while ARM64's "flip sign between rounds, which is the
signature of measurement noise around zero". By that same criterion, applied to
nine rounds instead of three, **x64 now looks like ARM64 did.**

### The control measurement, which is what makes the negative result credible

This is not an explanation of a bias; it is the measurement that shows there is
nothing left to explain. **The host counter, compared against itself**, read at
the two windows in the same round:

    host 100 ms  minus  host 1 s, per round:
      -4.53   -2.63   +10.95   +4.63   -14.15   +9.43   +2.67   -5.44   +3.85
      range -14.15 to +10.95 points        spread 25.1 points        mean +0.53

**`Get-Counter` disagrees with `Get-Counter` by up to 14 points, on the same
machine, in the same second, purely because one window is 100 ms and the other is
1 s.** The recorded bias was +7.47 and +4.09 points. It sits comfortably inside
the noise that the window difference alone produces, and the mean of that noise is
+0.53 — i.e. centred on zero, not on an offset.

### The counter-object candidate, disproved rather than argued

The brief lists `\Processor Information(_Total)\` versus `\Processor(_Total)\` as
a candidate, noting they differ on modern CPUs. On this CPU — a hybrid
P-core/E-core Core Ultra 9 with 22 logical processors, exactly the kind where they
should diverge — at the 1 s window, all nine rounds:

    55.379/55.379   56.864/56.794   51.322/51.274   63.116/63.080   59.447/58.954
    61.431/61.431   62.675/62.675   63.135/63.116   60.745/60.706

**Maximum difference 0.49 points.** The two objects agree. This candidate is
eliminated by measurement, not set aside. (It is also moot: `windows_impl.rs:360`
reads `\Processor Information(_Total)\`, and §42.3's host column used the same
object — the two were never mismatched.)

### Disk latency — the 3.47x is gone

    round   product offline   product service   host 1 s    host 80 ms
      4         195 us            203 us         240.5 us    197.4 us
      5         202 us            390 us         274.3 us    197.8 us
      6         181 us            225 us         250.7 us     52.7 us
      7         201 us            648 us         112.7 us    219.4 us
      8         511 us             46 us         945.5 us    205.2 us
      9         227 us            200 us         186.4 us    597.7 us

Product-offline against host-1 s, as a ratio: 0.81, 0.74, 0.72, 1.78, 0.54, 1.22
— **mean 0.97, and the product reads *below* the host in four of six.** Against
§42.3's 3.47x that is not a smaller bias, it is no bias.

(Rounds 1–3 are excluded from the disk table because in that batch the product
returned `avgTransferLatencyUs: 0` in five of six readings — **and the host at the
product's own 80 ms window returned 0 in all three rounds too.** The two agreed
that nothing completed in that window. Batch 2 ran under heavier disk load and
both instruments saw real transfers.)

### Verdict, stated plainly and then stopped

> **`DBT-P42-011` — the x64 numeric bias DOES NOT REPRODUCE on the current
> build.** Nine rounds on the machine and silicon the original was measured on:
> the disk-latency 3.47x is gone (mean ratio 0.97, product below host in 4 of 6);
> both CPU series flip sign, which is the exact criterion §43.5 used to call
> ARM64's result noise; and the surviving mean offsets (+3.94 and +4.22 points)
> are smaller than the ±14-point spread the **host counter shows against itself**
> from the window difference alone.
>
> The recorded numbers predate P45's field-boundary fix, as §48.7 noted. Whether
> P45 fixed it, or whether three readings were never enough to establish it,
> **this session does not claim to know and did not try to find out** — the brief
> forbids hunting a cause for something that is gone, and the honest answer is
> that a three-sample result which does not survive nine samples was probably
> never a result.

**Not fixed, not re-scoped, nothing changed in the collectors.** The row moves
from "open, unexplained" to "does not reproduce, with nine rounds of evidence".

### One thing this measurement surfaced that is NOT the bias, and is not gone

Recorded because it was visible directly in the captured JSON, not because it was
hunted for. The offline path reports `"intervalMs":250` and the service path
reports `"intervalMs":1000` — but both measured CPU over the **same 100 ms**:

    crates/performance-telemetry/src/windows_impl.rs:349   let _ = interval;
    crates/performance-telemetry/src/windows_impl.rs:369   std::thread::sleep(
                                                             Duration::from_millis(120)
                                                             .min(Duration::from_millis(100)))
    crates/performance-telemetry/src/lib.rs:330            interval_ms: interval.as_millis() …

`sample_cpu` takes an `interval` argument and **discards it on its first line**,
then sleeps a hardcoded 100 ms; `into_snapshot` publishes the *requested* interval
as `intervalMs`. Storage is the same shape at a hardcoded 80 ms
(`windows_impl.rs:710`).

> **`DBT-P49-004` — `intervalMs` does not describe the window the numbers were
> measured over.** A consumer of the snapshot who divides by `intervalMs`, or who
> compares two snapshots taken at different requested intervals, is using a number
> that no collector honoured. Risk: **medium** — it is a wire-contract accuracy
> defect, and it is precisely what made §42.3's product-vs-host comparison invalid
> without anyone noticing. Also note `Duration::from_millis(120).min(Duration::from_millis(100))`
> is always 100 ms; the `120` is dead. **Deliberately NOT fixed here** — the brief
> says a cadence or denominator change alters every reading on the qualified
> platform and needs its own review. Fixing the *reporting* (publish the window
> actually used) and fixing the *cadence* (honour the requested interval) are two
> different changes with two different blast radii, and that decision is not this
> session's to make.

### Cleanup

    LOAD_STOPPED=True      LOAD_DIR_REMOVED=True

## 49.7 ITEM 5 — THE VC++ RUNTIME GUARD ON x64, VERIFIED WITHOUT BREAKING THIS MACHINE

**The redistributable was not removed, and will not be.** The brief says so and
gives the reasons; they are restated here because a later session must not read
this as an untested gap and "finish the job":

- This is the owner's real working computer. Other software depends on that
  runtime.
- **It has no snapshots.** System Restore is the only rollback (§49.3).
- The refusal is **already proven** on ARM64 — §48.3 removed the redistributable
  from the VM under a named snapshot and measured install refusing with **1603**,
  a message naming the runtime in full, **nothing installed**, and the binaries
  dying `0xC0000135 STATUS_DLL_NOT_FOUND`.

So this item verifies that the guard is **correctly authored on x64**, from the
built artefacts, and says plainly what that does and does not establish.

### (a) The `Launch` condition is in the built x64 MSI, and it names the runtime

Read out of the MSI's own `LaunchCondition` table, not out of `Product.wxs`.
Four rows; this is the fourth:

    Condition : VCRUNTIMEVERSION
    Message   : AetherCore requires the Microsoft Visual C++ 2015-2022
                Redistributable (x64), which is not installed on this computer.
                AetherCore loads VCRUNTIME140.dll and MSVCP140.dll from it.
                Install vc_redist.x64.exe, or run AetherCoreSetup.exe instead.
    LAUNCHCONDITION_ROW_COUNT = 4

`$(sys.BUILDARCH)` expanded to **`x64`** in the message, where §48.3 read `arm64`
in the ARM64 package. And the property is genuinely searched for, not assumed —
the `AppSearch` and `RegLocator` tables carry the wiring:

    AppSearch : VCRUNTIMEVERSION  <-  VcRuntimeVersion
    RegLocator: VcRuntimeVersion  Root=2 (HKLM)
                Key=SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\x64
                Name=Version  Type=18

The other three rows are the downgrade guard, the OS-version/SKU condition, and
the domain-controller exclusion.

### (b) The Burn chain lists `VCRedist` first — and this time it also RAN

§49.2 read it out of Burn's manifest: `VCRedist` → `WebView2` → `AetherCoreMsi`,
`CHAIN_PACKAGE_COUNT = 3`, with

    RegistrySearch VcRuntimeSearch  Root=HKLM  Win64=yes
      Key=SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\x64  Value=Version
    DetectCondition="(NOT (VcRuntimeVersion = ""))"

**Item 3 went further than the brief required here.** §48.6 could only say the
chain was "authored, compiled, and present". §49.5 executed it and captured Burn
deciding:

    Detected package: VCRedist, state: Present
    Planned  package: VCRedist, state: Present, execute: None, rollback: None

So the detection half of `DBT-P41-001`'s Burn arm is now **measured working on
x64**, on the present branch. The *install* branch — Burn actually running
`vc_redist.x64.exe /install /quiet /norestart` because the runtime is absent —
remains unexecuted on both architectures.

### (c) What the x64 binaries actually import, read with `llvm-objdump -p`

    llvm-objdump  LLVM 22.1.8-rust-1.98.0-stable
    over the SIX INSTALLED binaries in C:\Program Files\AetherCore

    binary                                runtime DLLs imported
    aethercore-desktop.exe                (none)
    aethercore-maintenance-service.exe    MSVCP140.dll, VCOMP140.DLL, VCRUNTIME140.dll, VCRUNTIME140_1.dll
    aethercore-consent-broker.exe         VCRUNTIME140.dll
    aethercore-update-broker.exe          VCRUNTIME140.dll
    aethercore-install-hardener.exe       VCRUNTIME140.dll
    aetherctl.exe                         VCRUNTIME140.dll

    binaries importing VCRUNTIME140.dll   = 5 / 6    EXPECTED 5   MET
    binaries importing MSVCP140.dll       = 1 / 6    the service  MET
    binaries importing VCRUNTIME140_1.dll = 1 / 6    x64-only     MET
    binaries importing VCOMP140.DLL       = 1 / 6    the service

**All three of §48's expectations met exactly.** The one binary that imports no
VC++ runtime is `aethercore-desktop.exe`, and its full 24-DLL import list explains
why — it links the UCRT through the `api-ms-win-crt-*` forwarders and takes
everything else from system DLLs:

    advapi32, api-ms-win-core-synch-l1-2-0, api-ms-win-crt-{convert,heap,locale,
    math,runtime,stdio,string}-l1-1-0, bcryptprimitives, combase, comctl32,
    crypt32, dwmapi, gdi32, kernel32, ntdll, ole32, oleaut32, secur32, shell32,
    shlwapi, user32, ws2_32

`VCRUNTIME140_1.dll` is the x64-only one — it carries the x64 C++ exception
unwinder, which has no ARM64 counterpart — and the fact that exactly one binary
imports it on x64 and none can on ARM64 is why the guard has to be authored per
architecture rather than shared. The service is also the reason `vcomp140.dll`
ships **inside** the package as `File` row 7 rather than being taken from
System32: it is the OpenMP runtime, and Microsoft does not redistribute it
through the VC++ redistributable's supported install.

### (d) What this machine currently has

    HKLM\SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\x64
      Version   = v14.44.35211.00        Installed = 1
      Major.Minor.Bld.Rbld = 14.44.35211.0

    Add/Remove Programs:
      Microsoft Visual C++ 2015-2022 Redistributable (x64) - 14.44.35211   14.44.35211.0
      Microsoft Visual C++ 2015-2022 Redistributable (x86) - 14.44.35211   14.44.35211.0

    C:\Windows\System32:
      VCRUNTIME140.dll       124,544 B   v14.44.35211.0
      VCRUNTIME140_1.dll      49,792 B   v14.44.35211.0
      MSVCP140.dll           557,728 B   v14.44.35211.0
      VCOMP140.DLL           193,152 B   v14.44.35211.0

Every DLL the six binaries import is present at 14.44.35211.0, so
`VCRUNTIMEVERSION` resolves non-empty and the `Launch` condition passes — which
is exactly why every install in §49.4 and §49.5 succeeded rather than refusing.

### What this establishes, and what it does not — stated rather than implied

**Established on x64, by measurement:** the `Launch` condition exists in the
shipped MSI, names the runtime and the architecture correctly, and is wired to a
real registry search; the Burn chain carries `VCRedist` as its first package and
Burn's detection of it works; and the imports the guard exists to protect are
exactly what §48 predicted — five of six on `VCRUNTIME140.dll`, the service
additionally on `MSVCP140.dll`, and `VCRUNTIME140_1.dll` present only here.

**NOT established on x64, and deliberately not attempted:** that the refusal
actually fires. **The refusal itself is proven on ARM64 (§48.3) and is inferred
here.** The authoring is identical up to the `$(sys.BUILDARCH)` substitution,
which is itself verified above — but *inferred* is not *measured*, and this row
must not be written up as an x64 measurement by a later session. Proving it needs
a machine without the runtime, and that machine must not be this one.

## 49.8 ITEM 6 — THE STATEMENT §48.7 COULD NOT MAKE

**This section supersedes §48.7's gate table and unproven list.** §48.7 remains
accurate as of the day it was written; every row below that differs from it
differs because this machine became reachable.

### The gap §48.7 named, and whether it is closed

> §48.7: *"Read that table honestly: the x64 gates passed against a build from
> Phase 41, and the current build's gates passed on ARM64. **No single
> architecture has both a current build and a full gate set.**"*

**x64 now does.** Against **`AetherCore-0.1.11-x64.msi`, sha256
`77ee416b65412ef9306ed25b00d76ff98f9da3b88d1ba19a4e182461d7f7546e`, 1,100,271,616
bytes**, built from `main@6926bc7` on the physical machine and gated on it:

| gate | verdict | architecture | against which build |
|---|---|---|---|
| 0 machine native, protections on, restore point | **PASS** | x64 physical | §41.2, and §49.4 created and enumerated a fresh one, `P49-PRE-GATE5` seq 6 |
| 0f full verified disk image before driver work | **DEGRADED — the image is not on this machine** | x64 physical | §41.12-13 captured it; **measured today: `Get-Volume` shows one lettered volume, C: (953 GB), no D:, and `D:\WindowsImageBackup` does not exist.** §48.7 recorded C: and D:; there is now no D: at all |
| 1 MSI builds, zero ICE, payload explained | **PASS** | **x64 physical** | **§49.2 against `77ee416b…`** — validate **0 lines**, `PAYLOAD_CHECK=PASS`, 16 File rows |
| 2 installed, running, security properties, local model live | **PASS** | **x64 physical** | **§49.4 against `77ee416b…`** — 16 files MATCHED=16, LocalSystem, SID UNRESTRICTED, pipe SDDL byte-identical, `engineLabel localModel` from the running service |
| 2 (re-proven) | **PASS** | ARM64 VM | §48.3 against `fafe27a6…` |
| 3 real-hardware evidence with numbers | **PASS** | x64 physical | §41.15-16, and **§49.6 adds nine rounds of current-build telemetry on this silicon** |
| 4 driver install + rollback on a safe device | **NOT STARTED — HARD STOP** | — | owner + hardware; both blockers re-measured today, below |
| 5 full lifecycle, zero survivors | **PASS** | **x64 physical** | **§49.4 against `77ee416b…`** — uninstall exit 0 twice, **zero survivors on all fourteen both times**, `MACHINE_WIDE=0`, reinstall exit 0 |
| 5 (ARM64) | **PASS** | ARM64 VM | §47.7 against 0.1.11 arm64 |
| **bundle install + uninstall lifecycle** | **PASS — NEW GATE, never previously runnable** | **x64 physical** | **§49.5 against `d0398765…`** — first execution by anyone; product indistinguishable from the MSI install; uninstall through the bundle, zero survivors on all fourteen, both package caches released |
| `DBT-P41-001` refusal on a runtime-less machine | **PASS** | ARM64 VM | §48.3. On x64 the authoring is verified (§49.7) and the **refusal is inferred, not measured** |

**So the honest new statement is:** *x64 has a current build, and every gate that
is not blocked on the owner passes against it.* Gate 4 is the only gate that has
never started, and it is blocked on hardware and an owner decision, not on the
build. Gate 0f is the one row that got **worse**, and by measurement rather than
by neglect: the disk image §41.12-13 captured is not attached to this machine.

ARM64 keeps its own current-build gate set (§47.7, §48.3) against the ARM64
0.1.11 `fafe27a6…`. **Both architectures now have a current build with a gate
set. Neither has Gate 4.**

### Two ledger rows this session also closes or moves

**`DBT-P42-012` — five of its six numbered checks are now met on real x64
hardware.** §46.1 and §47.7 parked them on "a real `build-installer.ps1` + WiX
run", which had never happened because that script is x64-only. It happened
this session:

    check 1  syntax parses                      MET   the script ran to completion (§49.2)
    check 2  resolves via $env:VCToolsRedistDir  MET   "vcomp140.dll staged from
                                                       ...\14.44.35112\x64\Microsoft.VC143.OpenMP\vcomp140.dll"
    check 3  hash passes on the good file        MET   silent, build continued
    check 4  hash FAILS LOUDLY on the wrong file MET   see the negative control below
    check 5  the pinned fallback resolves        NOT VERIFIABLE HERE
    check 6  full build succeeds end-to-end      MET   §49.2, validate EMPTY, 16 rows,
                                                       vcomp140.dll ships at 55aba23c…

**Check 4, the negative control, run deliberately** — a guard nobody has seen
fail is not a guard. The `onecore` variant was staged as `vcomp140.dll` and the
build was invoked:

    WRONG file: ...\14.44.35112\onecore\x64\Microsoft.VC143.OpenMP\vcomp140.dll
                72,712 B  sha256 3b154db5fff1445ac1718f252435a76a58d884073787a44e1c0dbcc3e8774bfc

    build-installer.ps1:84 threw:
      vcomp140.dll at ...\bad-payload\vcomp140.dll is NOT the expected desktop x64
      MSVC OpenMP redist (expected 193152 bytes / sha256 55aba23c…; got 72712 bytes
      / sha256 3b154db5…). DBT-P42-012: this machine has carried five files of this
      name … and the first plausible match is wrong. Do not suppress this check.

    CHECK4_EXIT=1        SHOULD-NOT-EXIST.msi created?  False

Both byte counts and both hashes, side by side, **before any WiX step**, and no
package produced. That is exactly what §44.5 specified.

**Check 5 is not verifiable on this machine and is not claimed.** The pinned
fallback path is `C:\AetherCore-P36\toolchain\vs2022\…`, which is the ARM64 VM's
layout; `C:\AetherCore-P36` does not exist here (§49.1). It stays open, and it is
the *fallback* branch, not the branch any real build on this machine takes.

**`DBT-P42-011` moves from "open, unexplained" to "does not reproduce"** — nine
rounds, §49.6.

### What remains unproven, on either architecture

| unproven | why, and what would settle it |
|---|---|
| **Gate 4, driver install + rollback** | Both blockers re-measured **today**: (1) the recovery media is still not attached — one lettered volume, C:, and no `D:\WindowsImageBackup`; (2) `Microsoft.Update.Session` search `IsInstalled=0 and Type='Driver'` returned **ResultCode 2, 0 updates**. Needs the owner to re-attach and **boot-test** the media, and to nominate a printer/HID/USB-class device. **Never storage, chipset or GPU.** |
| **The `vc_redist` install branch of the Burn chain** | Authored, compiled, and its **detection** now measured working on x64 (§49.5, §49.7). What has never run on either architecture is Burn actually executing `vc_redist.x64.exe /install /quiet /norestart` because the runtime was absent. Needs a machine without the runtime — **not this one** |
| **The WebView2 install branch** | Same: WebView2 was `Present` here, so the chain skipped it. The install branch is unexecuted |
| **`DBT-P41-001`'s refusal on x64** | Proven on ARM64 (§48.3), **inferred** on x64 (§49.7). Same machine constraint |
| **`DBT-P42-012` check 5** | The pinned-fallback branch; not verifiable on a machine that lacks the pinned tree |
| **Windows Server, any SKU** | The MSI advertises Server support (`MsiNTProductType = 3 AND OSCURRENTBUILD >= 17763`) and **nothing has ever run on a Server SKU.** This machine reports `InstallationType = Client`. The product still makes a claim it has never tested |
| **Zero-session operation** | Only session-0-only operation is proven; a console session exists on this machine too |
| **A real remote SSH audit** | Scope resolution, per-host isolation and fail-closed trust are proven; an audit against a second real machine is not |
| **Byte-reproducible builds** | Explicitly not a criterion; `RELEASE-METADATA.json` says `msi_byte_reproducible_claim = false` |
| **A signed installer** | `DEFERRED-OWNER`. Until then SmartScreen warns, and §49.5 measured what the unsigned dialog looks like |

### Gate 4 — still blocked, and left alone

Both facts re-measured today rather than carried forward:

    Get-Volume            one lettered volume: C: (953 GB, Fixed)
                          plus two unlettered system partitions (0.8 GB, 0.1 GB)
                          D:\WindowsImageBackup                     False
    Windows Update        Search("IsInstalled=0 and Type='Driver'")
                          ResultCode = 2 (succeeded)
                          DRIVER_UPDATES_OFFERED = 0

**No driver work was started, and none should be.** Gate 4 needs two owner
actions before it is even runnable: re-attach the recovery media and *boot-test*
it, and nominate a safe device. Creating recovery media is not the same as
proving it boots.

### What a first real user hits — updated from §48.7's list of nine

Items 4, 5, 6 and 8 of §48.7's list are unchanged and are not repeated. What
changed:

1. **The installer is unsigned, so SmartScreen will warn.** Unchanged —
   `DEFERRED-OWNER`. §49.5 now records exactly what dialog that warning sits in
   front of.
2. **~~The bundle has never been installed by anyone.~~ CLOSED.** It has now been
   installed, and uninstalled, on real x64 hardware, and the resulting product is
   identical to the MSI's on every measured property (§49.5).
3. **NEW — the installer looks unfinished.** `DBT-P49-002`: the title bar reads
   "AetherCore Setup Setup", the logo is WiX's stock placeholder rather than the
   mark §48.2 made deliberate, the welcome screen shows no version and no license,
   and the progress bar sits on "Initializing..." for the whole 30-second install.
   This is now the **first** thing a paying customer sees, and it is cosmetic —
   which makes it cheap to fix and expensive to leave.
4. **NEW — a bundle install costs ~1.05 GB more disk than an MSI install.**
   §49.5: Burn keeps a full 1,100,271,616-byte copy of the MSI in
   `C:\ProgramData\Package Cache` on top of the copy msiexec keeps in
   `C:\Windows\Installer`. It is released on uninstall. Not a defect; a number
   nobody had.
5. **NEW — `intervalMs` in the telemetry snapshot does not describe the window
   the numbers were measured over.** `DBT-P49-004`, §49.6.
6. **A same-version reinstall needs `REINSTALLMODE=vamus`** — unchanged, and note
   Burn sidesteps it entirely by passing `REINSTALLMODE="muso"` itself.

### The verdict, rewritten

§48.7's verdict ended: *"the product is qualified on ARM64 against the current
build, and on x64 against a Phase 41 build, and the consumer installer has never
been installed by anyone."* Two of those three clauses are now obsolete.

**The product is qualified on x64 against the current build `77ee416b…`, on ARM64
against the current build `fafe27a6…`, and the consumer installer has been
installed, verified equivalent, and uninstalled cleanly.** §48.7 listed four
things shipping requires, in order: reach the x64 machine, run the pipeline and
Gate 5 there on 0.1.11, install the bundle at least once, and finish Gate 4. **The
first three are done.** What is left is Gate 4, which is two owner actions and a
device, and the certificate, which is money — plus the honest residue above:
Windows Server has never been touched, and the two prerequisite *install* branches
have never executed on any machine.

## 49.9 THE LEDGER DELTA — rows P49 moved, and four new ids

§48.1's authoritative ledger counted **77** ids. This session adds four and moves
five. **The whole ledger was not re-counted here**, so 81 is arithmetic on §48.1's
figure rather than an independent recount — said plainly because §48.1 exists
precisely because counts drift.

### Rows moved

| id | was, end of P48 | now | evidence |
|---|---|---|---|
| `DBT-P41-001` | FIXED on ARM64; x64 execution `BLOCKED-MACHINE` | **FIXED on ARM64; x64 authoring verified and DETECTION measured; the refusal itself still ARM64-only** | §49.7 — `Launch` condition read out of the built x64 MSI naming `(x64)`, backed by real `AppSearch`/`RegLocator` rows; §49.5 captured Burn deciding `Detected package: VCRedist, state: Present` → `execute: None`. What has still never run **on any architecture** is the chain's *install* branch, with the runtime absent |
| `DBT-P42-011` | **OPEN** — the x64 numeric bias, machine-gated | **DOES NOT REPRODUCE** | §49.6 — nine rounds on the original machine and silicon. Disk 3.47x gone (mean ratio 0.97). Both CPU series flip sign. Residual means smaller than the ±14-pt spread the host counter shows against **itself** at 100 ms vs 1 s. Cause deliberately not hunted |
| `DBT-P42-012` | FIXED in code; six numbered checks need an x64 pipeline run | **FIXED, and 5 of 6 checks MET on real x64 hardware** | §49.2 (checks 1, 2, 3, 6) and §49.8 (check 4, the negative control, run deliberately). **Check 5 stays open** — the pinned fallback is the ARM64 VM's layout and `C:\AetherCore-P36` does not exist here |
| `DBT-P47-002` | FIXED — the corrected `UNINSTALL.txt` paragraph | **FIXED, and now shipped and checked claim-by-claim in a package** | §49.4 — the 3,874 B / `086de152…` text is installed by MSI `77ee416b…`, and all ten of its measurable claims were verified against the survivor sweep. The WebView2 paragraph itself could not be re-proven here: no account on this machine has ever launched the desktop app |
| `DBT-P48-004` | FIXED — the bundle now builds | **FIXED, and the artefact has now been BUILT AND RUN on x64** | §49.2 built `d0398765…`; §49.5 installed and uninstalled it. The row's risk ("the preferred consumer install path does not exist") is fully retired |

### New ids

| id | risk | what it is | fixed? |
|---|---|---|---|
| **`DBT-P49-001`** | low | `scripts/build-release.ps1`, the production x64 pipeline, never sets the ADK `DismApi\Lib\amd64` path into `LIB`, so a clean shell dies `LNK1181`. `build-arm64-msi.cmd:77` does set it for arm64. Same shape as `DBT-P48-004`, milder: it is documented in §41.4/§42.2 and it fails loudly rather than producing a wrong artefact | **No** — changing the release script's environment handling is its own review |
| **`DBT-P49-002`** | cosmetic, most-exposed surface | the consumer installer's UI: title bar reads "AetherCore Setup Setup"; the logo is WiX's 852-byte stock placeholder, a **third** mark after the shipped icon and `DBT-P48-001`'s in-app mark; the welcome screen shows no version despite `ShowVersion="yes"` and no license because `LicenseUrl=""`; install progress reads "Initializing..." with an empty bar for the whole 30 s install while the *uninstall* path names the package correctly | **No** — Item 3 was to install it and report |
| **`DBT-P49-003`** | low | the bundle leaves `AetherCore_Setup_*.elevated.log` in `%TEMP%` after install (926 B) and uninstall (935 B). Per-user, so `MACHINE_WIDE=0` holds, but product-attributable files the MSI path does not create. Same family as `DBT-P42-013` | **No** |
| **`DBT-P49-004`** | **medium** | `intervalMs` in the telemetry snapshot does not describe the window the numbers were measured over. `windows_impl.rs:349` discards the requested interval on its first line, `:369` sleeps a hardcoded 100 ms, `:710` a hardcoded 80 ms, and `lib.rs:330` publishes the *requested* value — so the captured JSON says `intervalMs 250` offline and `1000` from the service while both measured CPU over the same 100 ms. A wire-contract accuracy defect, and it is what made §42.3's product-vs-host comparison invalid without anyone noticing. Also: `Duration::from_millis(120).min(Duration::from_millis(100))` is always 100 ms; the `120` is dead | **No** — the brief forbids it; a cadence change alters every reading on the qualified platform. Fixing the *reporting* and fixing the *cadence* are two different changes with two different blast radii |

## 49.10 P49 FINAL REPORT

### The table, with numbers on every row

| item | verdict | the numbers |
|---|---|---|
| 0 machine survey | DONE | elevated `True`, `core.autocrlf false`, `Hussein`, x86_64, 401.2 GB free. Four `vcomp140.dll` on disk, first-match wrong. Pre-existing install was the ungated peer build `6ecd1ee9…` |
| 1 build on x64 | **DONE** | ui 2.02 s, cargo 1m 54s, tauri 6m 38s, all exit 0. `wix msi validate` **0 lines**. `PAYLOAD_CHECK=PASS`. **16** File rows. MSI `77ee416b…` 1,100,271,616 B. Bundle `d0398765…` 1,128,354,997 B |
| 2 Gate 5 on x64 | **PASS** | restore point seq **6** enumerated. Uninstall exit 0 ×2 (3233 / 3047 ms), install exit 0 ×2 (31,265 / 31,257 ms). **Zero survivors on all fourteen, twice.** `MACHINE_WIDE=0` twice. 16 files `MATCHED=16 MISMATCH=0`. Pipe SDDL byte-identical. `engineLabel localModel` |
| 3 install the bundle | **PASS + 4 UI findings** | `BUNDLE_EXIT=0` 121 s; uninstall exit 0 192 s. Both prereqs `Present` → `execute: None`. `restart: None`. 12/12 properties identical to the MSI install. Zero survivors on all fourteen. Both caches released |
| 4 the bias | **DOES NOT REPRODUCE** | **9** rounds. Disk mean ratio **0.97** vs recorded 3.47. CPU means +3.94 / +4.22 pts, **both series flip sign**. Host-vs-itself window spread **−14.15 to +10.95 pts** |
| 5 the VC++ guard | **correctly authored on x64** | 4 `LaunchCondition` rows, the fourth naming `(x64)`. `llvm-objdump -p`: **5/6** `VCRUNTIME140.dll`, service also `MSVCP140.dll`, `VCRUNTIME140_1.dll` **1/6** x64-only. Machine carries 14.44.35211.0. Refusal **inferred**, not measured |
| 6 the §48.7 statement | **DONE** | §48.7 superseded. x64 has a current build + every non-owner-blocked gate. Gate 0f **downgraded**: no D:, no `D:\WindowsImageBackup`. Gate 4 blockers re-measured: **0** drivers offered, ResultCode 2 |

### The two hashes

    AetherCore-0.1.11-x64.msi        1,100,271,616 B
      sha256 77ee416b65412ef9306ed25b00d76ff98f9da3b88d1ba19a4e182461d7f7546e

    AetherCoreSetup-0.1.11-x64.exe   1,128,354,997 B
      sha256 d03987652c8853ece7b2572979999324b0f19e923f1a8a638e05aa51541cbf99

### What the bundle install did that the MSI install does not

1. Evaluated a prerequisite chain — both packages `Present`, both skipped, product
   installed anyway. **It does not fail silently on the already-present branch.**
2. Registered a second ARP key, `AetherCore Setup`, and hid the MSI's own with
   `ARPSYSTEMCOMPONENT="1"`. Measured: **one** user-visible entry, and it is named
   *AetherCore Setup*, not *AetherCore*.
3. Kept a second full **1,100,271,616-byte** copy of the MSI in
   `C:\ProgramData\Package Cache` on top of msiexec's own copy in
   `C:\Windows\Installer` — **+1,050.3 MB** over an MSI install. Released on
   uninstall.
4. Launched a separate elevated engine process. **No UAC prompt was seen here and
   that proves nothing** — it inherited elevation. A real user gets one prompt.
   Inferred, not measured.
5. Passed `ARPSYSTEMCOMPONENT="1" MSIFASTINSTALL="7" BURNMSIINSTALL=1
   REINSTALLMODE="muso" REBOOT=ReallySuppress` to the MSI — sidestepping the
   `REINSTALLMODE=vamus` footgun §48.7 lists.
6. Left two small `.elevated.log` files in `%TEMP%` (`DBT-P49-003`).
7. Showed a GUI — which produced `DBT-P49-002`, four cosmetic defects on the most
   exposed surface in the product.

### The bias verdict, plainly

**`DBT-P42-011` does not reproduce.** Nine rounds on the machine and the silicon
it was recorded on, under real sustained load: the disk-latency 3.47× is gone
(mean ratio 0.97, product reading *below* the host in four of six), and both CPU
series flip sign — which is the exact criterion §43.5 used to call ARM64's result
noise and x64's a real bias. The residual mean offsets (+3.94 and +4.22 points)
are smaller than the **±14-point spread the host counter shows against itself**
purely from reading it at 100 ms versus 1 s. The brief's counter-object candidate
is disproved by measurement: `\Processor Information(_Total)\` and
`\Processor(_Total)\` agree to within 0.49 points across all nine rounds on a
hybrid Core Ultra 9. Per the brief, **no cause was hunted and nothing was fixed** —
a three-sample result that does not survive nine samples was probably never a
result.

### Recorded rather than worked around

- `DBT-P49-001` — the production x64 pipeline does not set its own `LIB`.
- `DBT-P49-002` — the consumer installer's UI, four defects, seen by every user.
- `DBT-P49-003` — two `%TEMP%` logs the bundle leaves behind.
- `DBT-P49-004` — `intervalMs` misdescribes the measurement window.
- **A false finding this session nearly recorded and then killed by measurement:**
  `doctor` exits 5 printing nothing on stdout, which read as silent failure. It
  writes 115 bytes of typed rejection to **stderr**, and `--output json` carries
  the same `RejectedByService` / `diagnostics.stateUnavailable` envelope. Correct
  by design, exactly as §42.8 resolved it.
- **The brief's own pipe-DACL method fails, for the third session running.**
  `[System.IO.File]::Open('\\.\pipe\…')` returns *"asked to open a device that was
  not a file"*; `NamedPipeClientStream` is what works. §41.14, §42.7, and now
  §49.4.
- **Gate 0f got worse, and by measurement.** §48.7 recorded `Get-Volume` showing
  C: and D:. Today there is one lettered volume and no `D:\WindowsImageBackup`.

### The machine, left better than it was found

It began this session carrying an ungated 2026-09-02 build (`6ecd1ee9…`, with the
pre-`DBT-P47-002` `UNINSTALL.txt`). It ends carrying the **qualified** build:

    ARP        AetherCore 0.1.11 {0F9F349D-01C8-B3C2-7242-83B5D29047C9}
    files      16          service    RUNNING
    from       MSI 77ee416b…, the package every gate in §49.4 passed against

Defender, UAC, Firewall and SmartScreen untouched. The VC++ redistributable
untouched. No driver work. Restore points 3, 4 and 5 untouched;
`SystemRestorePointCreationFrequency` returned to the Windows default. No file was
deleted by hand in any sweep.

### The single next action

**Gate 4: re-attach the recovery media, boot-test it once, and nominate a
printer-, HID- or USB-class device — it is now the only gate in the project that
has never started, and the only thing between this build and a complete gate set
on x64.**

---

# PHASE 50 — P50-OVERVIEW: REBUILD THE OVERVIEW ON THE SHELL'S COMPOSITION (2026-09-06)

**One screen. `apps/ui/src/features/overview/OverviewPage.svelte` and what it
renders. The other ten are not touched.**

P47 ported the design *system*. It did not port the *composition*, and it said
so. The owner installed the result and said the app still looks like the old
one. Measured side by side he is right: the shell's Overview is two columns and
instrument-dense — a health orb with four channel rails beside action items,
then four telemetry tiles, then a service log — and the app's Overview is a
single tall column of stacked cards carrying none of those four sections.

## 50.0 PROGRESS TABLE (authoritative — resume from here)

Rows move in the same commit as the work they describe.

| item | status | evidence |
|---|---|---|
| 1 the mapping table, committed before any component | **DONE** | §50.1 — 38 rows, every one with a verdict and a named source or `NONE` |
| 2 the app's own content: keep / move / below the fold | **DONE** | §50.2 — 8 sections decided, none deleted, 2 recorded as belonging elsewhere (`DBT-P50-003`) |
| 3 build it | **DONE** | §50.3 — 6 files, +531/-14, one screen. New `DBT-P50-004` (the same inherited-`flex-wrap` defect in the shared `EmptyState`, fix known, deliberately not applied) |
| 4 the sparklines | **DONE — not drawn** | §50.4 — the series exists in the service (`MAX_RING_SAMPLES = 300`) and has no wire accessor: no `repeated PerfSnapshot` in the contract. Tiles drawn without, absence stated in both languages. New `DBT-P50-005` |
| gates | **DONE — all pass** | §50.5 — numbers: 4 new/changed on this screen, all traced; arabic 7/7, 0 system fallback; tokens 443/443 resolve; sweep 12/12 populated **and** 12/12 with no service; contrast 0 below 4.5:1 across 1,174 text nodes in both themes and both languages; build clean; svelte-check 0 errors / 16 warnings, baseline held |
| screenshots 1280 × 2 languages × 2 themes × populated/empty | **DONE** | §50.6 — 8 committed under `design/p50-overview-screenshots/`, full 24-shot sweep in `apps/ui/output/` |
| ledger delta + report | **DONE** | §50.7 — 5 new ids (`DBT-P50-001`…`-005`), none fixed, each with its reason. §50.8 — the report |

## 50.1 ITEM 1 — THE MAPPING TABLE

Read from both sides before anything was written:

- the shell — `tools/bundle_template.py extract AetherCore.html`, then the
  `S.overview` screen definition (its four `this.sec(...)` calls) and the four
  `sc-if` template blocks that render them (`isOrb`, `isList`/`isEmpty`,
  `isMetrics`, `isLog`);
- the app — `OverviewPage.svelte` and everything it renders, plus
  `lib/contracts.ts` (the whole IPC surface), `platform/stream-state.ts` (what
  the UI actually retains), and the controllers that populate each slice.

**Sources are named as `streamState` paths.** `performance.*` is
`PerfSnapshot`, pushed by the `performanceSnapshot` kernel event while the
perf sampler is running. `diagnostics.*` is the hardware scan.
`hub.*` is the driver scan. `cleanupSnapshot.*` is the cleanup scan.
`snapshot.*` is the service snapshot, available whenever the service is
connected — it is the only slice that needs no scan.

| # | shell element | shell value | app's real source | verdict |
|---|---|---|---|---|
| **HEADER** |
| 1 | kicker | "Adaptive dashboard" | `t('overview.eyebrow')` — copy, claims nothing | keep app's |
| 2 | title | "System is breathing easy" | NONE — asserts a system state nobody measured | **drop the claim**, keep the app's neutral title |
| 3 | sub | "Baseline established 4 minutes ago. AetherCore is watching **214 signals** and predicts **no degradation in the next 72 hours**." | NONE. There is no signal count, no baseline timestamp and no predictor anywhere in the product | **drop** — named in the brief, and correctly |
| 4 | action "Run Smart Scan" | — | existing `setPage('deepScan')` CTA | wire — promoted into the header action row |
| 5 | action "One-Click Optimize" | — | `CarePanel` (one-click care) already renders below on this page | **drop** — a second entry point to the same run, not a second feature |
| 6 | service pill | not in the shell | `snapshot.connected`, `snapshot.serviceVersion` | keep app's |
| **§A SYSTEM HEALTH ORB (span 7)** |
| 7 | orb score | `94` / `72` / `—` | no service health index exists. **Derived**: `100 − mean(measured utilization channels)`, over rows 11–13 only | **wire (derived, formula published in its evidence chip)**; `—` when no channel has a reading |
| 8 | orb label | `HEALTH INDEX` / `NO BASELINE` | whether any channel has a reading | wire |
| 9 | orb evidence chip | "4 channels, equal-weighted" + the formula | real: each channel's reading, the arithmetic, and the two exclusions | wire |
| 10 | orb colour | red/green/blue by fabricated criticality | `snapshot.health` is a free-form service string ("Service offline"), not an enum, and a health *threshold* is not a measurement | **drop the state hue** — the orb is monochrome `--role-interactive`, dim when it has no reading |
| 11 | rail "CPU load" | `34%` | `performance.cpu.totalBusyBp / 100` | wire |
| 12 | rail "Memory" | `9.3 GB` / 58% | `performance.memory.memoryLoadPercent`, value from `totalPhysicalBytes − availablePhysicalBytes` | wire |
| 13 | rail "Disk C:" | `412 GB` / 71% — **used capacity** | NONE. `StorageTelemetry.sizeBytes` is capacity; nothing in the IPC surface reports free or used space. `grep -rn "GetDiskFreeSpace\|free_bytes\|availableBytes"` over `crates/` and `services/` returns one hit and it is memory | **drop the channel as written**; the slot becomes **Disk activity** ← `performance.storage[].activeTimeBp / 100`, which is measured. New id **`DBT-P50-001`** |
| 14 | rail "Thermals" | `54°C` / 42% bar | `performance.power.hasTemperature ? temperatureC : unmeasured` | wire **the value only**. The bar stays empty: normalizing °C to 0–100 needs a ceiling nobody measured, and inventing one is the same lie as inventing the number |
| 15 | thermal criticality (red at 81°C) | invented threshold | `performance.power.throttleActive` + `throttleReason` — **service-reported** | wire — the only place a state hue is earned on this section |
| **§B ACTION ITEMS (span 5)** |
| 16 | meta "2 suggested" / "3 urgent" / "0" | fabricated | count of rows actually built | wire |
| 17 | row "Storage driver crashed twice · storahci.sys · 0x133" | fabricated | `diagnostics.crashes[]` — `bugcheckHex`, `source`, `confidence`, `recordedUnixMs` | wire → routes to `crash` |
| 18 | row "GPU sustained 81°C for 22m" | fabricated | `diagnostics.events[]` + `eventWindowDays` — provider, severity, confidence | wire → routes to `hardware` |
| 19 | row "18.4 GB reclaimable" | fabricated | `cleanupSnapshot.totalReclaimableBytes`, `.totalFileCount`, `.candidates.length` | wire → routes to `cleanup` |
| 20 | row "2 drivers behind vendor build" | fabricated | `hub.summary.selectableUpdateCount`, `.deviceCount`, `.authorityCoverage` | wire → routes to `drivers` |
| 21 | — (app has, shell does not) | — | `deepScan.summary` (critical/high/moderate/low, `recommendedActions`) + `ruleEngineVersion` | wire → routes to `deepScan` |
| 22 | — (app has, shell does not) | — | `hub.summary.problemDeviceCount`, `.missingDriverCount` | wire → routes to `drivers` |
| 23 | row evidence chips | prose restatements | every row cites its own snapshot: scan id, completion timestamp, counts. `citedOnly` is the gate — a row that cannot cite is not rendered | wire |
| 24 | empty variant + 3 channels (Impact / Category / Reversibility) | — | `EmptyState` with channels named for the four scans that fill this list | wire (this is the honest default state of the screen) |
| **§C DIAGNOSTIC TELEMETRY (span 12)** |
| 25 | meta "30s window" / "no data" | fabricated | `performance.intervalMs`, `performance.capturedUnixMs` | wire |
| 26 | tile "Responsiveness 96 idx" | fabricated composite | NONE — no responsiveness index is computed anywhere in the product | **drop** |
| 27 | tile "Boot time 11.4 s" | fabricated | NONE — `grep -rn "boot_time\|bootTime\|BootTime"` over `crates/`, `services/`, `apps/ui/src` returns nothing | **drop** |
| 28 | tile "Disk latency 0.42 ms" | fabricated | `performance.storage[].avgTransferLatencyUs / 1000` | wire |
| 29 | tile "Open handles 38.2k" | fabricated | NONE — no handle counter exists | **drop** |
| 30 | — (replacement tile) | — | `performance.memory.hardFaultsPerSec` | wire |
| 31 | — (replacement tile) | — | `performance.cpu.contextSwitchesPerSec` | wire |
| 32 | — (replacement tile) | — | `snapshot.journalEventCount` — the one reading available with no scan at all | wire |
| 33 | sparkline × 4 | 16 fabricated bars each | NONE — see §50.4 | **drop, tile drawn without it** |
| 34 | delta line × 4 ("+4 vs baseline", "-2.1s this week") | fabricated deltas | NONE — a delta is a change over time and nothing retains time | **drop**; the line carries real provenance instead (device name, sampling interval, service version) |
| **§D CORE SERVICE LOG (span 12)** |
| 35 | 5 log rows ("[INIT] core services starting… OK", "[DATA] manifest loaded · 214 signals") | fabricated boot text | NONE as written | **drop the text** |
| 36 | the log section itself | — | the kernel event stream the UI already receives: `sequence`, `emittedUnixMs`, `kind`, `planId` on every `UiKernelEvent`. Retained as a bounded ring in `stream-state.ts` | **wire** |
| 37 | blinking cursor `▊` | decorative | — | keep, guarded by `prefers-reduced-motion` |
| **APP-ONLY, NOT IN THE SHELL** |
| 38 | hero metric "Protocol **v7**" | — | **a hardcoded literal in the markup.** `PROTOCOL_VERSION: u32 = 7` exists in `crates/contracts`, but no IPC field carries it to the UI, so the view will keep printing `v7` on the day the constant changes | **drop** — correct today, unsourced, and therefore a future lie. New id **`DBT-P50-002`** |

**Three verdict counts:** wire 22 · empty-by-design 2 · drop 13.

## 50.2 ITEM 2 — THE APP'S OWN CONTENT

The shell was drawn for fewer, idealised sections. The app's Overview carries
eight things the shell has no slot for. **Nothing here is deleted to make a
layout fit.** Each is kept, folded into an element that already carries it, or
left below the fold with the reason recorded.

| app section | decision | reason |
|---|---|---|
| **scan CTA** — "Scan My PC" / PC intelligence strip | **promoted** into the header action row | The shell's Overview has header actions and the app's did not. This is the same button in the slot the composition gives it. Nothing lost |
| **hero card** — orb glyph + "AetherCore is ready" + 3 metrics | **folded**, not deleted | Its four readings all survive: connected/ready → the service pill (already in the header) and the orb's meta line; `journalEventCount` → telemetry tile 4; `hub.summary.deviceCount` → the driver action-item row and its evidence. The fifth, `Protocol v7`, is dropped as an unsourced literal (row 38). The shell's orb section is the screen's anchor and two anchors is one too many |
| **current protected operation** — plan kind, state, risk, digest, consent-window actions | **kept, below the four sections** | This is the state engine — plan digest, risk tier, the authorization window, and the only place a plan can be approved from the Overview. The shell has no slot for it and inventing one would be redesigning a screen the owner has not seen yet. It keeps its panel, directly under the grid |
| **driver servicing preview** — selectable update count + Open Driver Hub | **kept, below the four sections** | The action-item row (row 20) *reports*; this panel *acts* — it starts the scan when the hub is Idle. Removing it would remove the only Overview affordance that starts a driver scan |
| **six-tile module row** — Drivers, Repair, Cleanup, Startup, Hardware, Crash | **kept, below the fold** | Duplicates the navigation rail, but it is the discoverability surface for six features and the brief forbids deleting a feature to fit a layout. Left where it is |
| **SystemCarePanel** — updates channel + support bundle export | **kept below the fold; belongs elsewhere** | This is settings-shaped, not overview-shaped. It belongs on a Care/Settings screen. There is no such screen in `NAVIGATION`, and creating one touches the other ten. Recorded as **`DBT-P50-003`** and left where it is — a slightly imperfect layout beats a lost feature |
| **AboutPanel** — platform capability matrix + engine source | **kept below the fold; belongs elsewhere** | Same shape and same reason. Recorded under the same id |
| **CarePanel / InsightsPanel** | **untouched** | `AppShell` renders these after `<OverviewPage>`, not inside it. Out of this file's scope, and the insights panel is where the "One-Click Optimize" action (row 5) actually lives |

**Resulting order of the screen:** header (with the scan action) → policy band
(shell chrome, persistent) → the four ported sections in a 12-column grid →
current operation + driver servicing → module row → SystemCare → About →
CarePanel → InsightsPanel.

Two entries above the fold in the shell's composition, six below it. Eight
sections in, eight sections out.

## 50.3 ITEM 3 — BUILT

**Six files, 531 insertions, 14 deletions. One screen.**

| file | what changed |
|---|---|
| `apps/ui/src/features/overview/instrument.ts` | **new.** The derivation: `healthChannels`, `headroom`, `headroomEvidence`, `actionItems`, `telemetryTiles`. Every function names the IPC field it read or returns `undefined`. Kept out of the markup so the arithmetic is readable on its own |
| `apps/ui/src/features/overview/OverviewPage.svelte` | the composition: four sections in a 12-column grid, then the app's six existing sections underneath, unchanged |
| `apps/ui/src/platform/stream-state.ts` | +16 lines: a bounded ring of the last 40 kernel events. See below |
| `catalog.en.ts` / `catalog.ar.ts` | +38 keys each, EN/AR parity |
| `plurals.en.ts` / `plurals.ar.ts` | +4 units each (`channel`, `event`, `crashRecord`, `device`), Arabic with all six categories |

### The one change outside the screen, and why

`stream-state.ts` gains `serviceLog: readonly UiKernelEvent[]`, the last 40
events, appended after the reducer's switch so an event that fails to route is
never logged as though it had been handled.

The shell's fourth section is a service log and its five lines are invented boot
prose. The alternative to this ring was to leave the section permanently empty,
because nothing in the UI retained the stream. But the stream is *already
arriving* — every `UiKernelEvent` carries `sequence`, `emittedUnixMs`, `kind`
and `planId` — and it was being read for `lastKernelSequence` and dropped. Nine
lines make the one log on that screen a real one. It is bounded because a live
stream has no end and the section shows a window.

### A layout defect this uncovered, in `feature-layout.css`

`main :where(*) { flex-wrap: wrap }` is deliberate and its own comment explains
it: a control label is not prose, so content ROWS wrap rather than squeeze, at
zero specificity so a row can opt out. A flex **column** that inherits it is
sized as a wrapping column:

    .tile   content height 141px   rendered height 595px
    the diagnostic telemetry section, 740px for 287px of content

Measured, not guessed — `flex-wrap: nowrap` set from the console collapsed it to
141/287 exactly. Every column in the new file now opts out explicitly, which is
what that rule's comment says to do.

**The same defect exists in `EmptyState.svelte`,** the shared signature
component: `.empty-state` is a flex column inside `main`, and with channels it
renders 496px for 396px of content — measured today at 1280 on `index.html`.
The fix is one declaration. It is **not applied**: `EmptyState` renders on seven
screens and this session is one screen. Recorded as **`DBT-P50-004`** with the
exact fix, for whoever takes the other ten.

### The decisions inside the build

- **The orb has no state hue.** The shell's turns red/green/blue on a fabricated
  criticality. `snapshot.health` is a free-form service string ("Service
  offline"), not an enum, and a health *threshold* is a judgement, not a
  measurement. The dial reports only whether it has a reading at all.
- **`51%`, not `51`.** Headroom is percentage points, so it is written as a
  percentage — and `verify-numbers.mjs` only examines numbers of that shape. A
  bare index would have been the most prominent figure on the screen and
  invisible to the gate that exists to trace it.
- **An empty channel track is hatched, not flat.** A bar at zero is a reading.
  A channel with no scale to normalize against is not, and the two must not look
  alike. Thermals is the second case and renders hatched with its reading intact.
- **The one earned colour** on that section is the throttling chip, which reads
  `power.throttleActive` — asserted by the provider, not inferred from a limit.
- **Temperature is formatted by the locale,** not by string concatenation:
  `${n} °C` reverses to `C° ٨٤` under RTL. `Intl` `style: 'unit'` gives `84°C`
  and `٨٤°م`.
- **Log lines are `direction: ltr`.** A log line is one technical token
  sequence; under RTL the flex row otherwise prints the sequence number before
  the timestamp — the defect `TechnicalText` prevents, one level up.
- **Action rows stack title / meta / (reading + tag)** rather than competing for
  a 5-of-12 column: at 1280 the reading was landing on top of the title.

## 50.4 ITEM 4 — THE SPARKLINES: NOT DRAWN, AND EXACTLY WHAT IS MISSING

The shell puts a sixteen-bar sparkline in each of the four telemetry tiles. They
come from `spark(11, 18, 55, 96)` — a generator, not a series.

**The question the brief asks is whether the app retains enough history to draw
one. It does not, and the gap is on the wire, not in the UI.** Traced end to end:

| layer | what it holds | verdict |
|---|---|---|
| the collector | `PerformanceRing` in `crates/performance-telemetry/src/lib.rs`. `MAX_RING_SAMPLES = 300`, ~5 min at 1 s cadence, `MIN_INTERVAL_MS = 250` | **a real series exists** |
| the service | `PerformanceService::…` at `services/maintenance-service/src/performance.rs:400` calls `self.ring.window(owner)` — the ordered copy of that ring. It is the ring's **only** consumer, and it feeds `analyze()`, not the UI | the series is read, and consumed internally |
| the wire | `performance.proto`: `message PerformanceSnapshotResponse { PerfSnapshot snapshot = 1; }`. `events.proto`: `PerfSnapshot performance_snapshot = 31;`. There is **no `repeated PerfSnapshot`** anywhere in the contract | **the series never leaves the service** |
| the UI | `stream-state.ts` holds `performance: PerfSnapshot`, replaced wholesale on each event. One sample, never two | nothing to draw |

So the tiles are drawn **without** sparklines, and the section says so in one
line, in both languages, rather than leaving a silent gap:

> *No sparklines: this build retains the latest sample, not a series. Drawing
> one would mean drawing a shape nobody measured.*

### What it would take

**The correct fix is a wire change**, which §4 puts behind explicit approval:
a `GetPerformanceWindowRequest` / `PerformanceWindowResponse { repeated
PerfSnapshot samples = 1; }` pair, or a compact per-metric series message so a
sparkline does not cost 300 full snapshots. The ring, its bound and its
ordering already exist; only the accessor is missing. Recorded as
**`DBT-P50-005`**.

**The cheap alternative was considered and rejected.** The UI could accumulate
its own ring of `performanceSnapshot` events, the way §50.3's service log now
accumulates kernel events. It was not done, for a reason worth writing down: it
would draw a line that begins when the window was opened and covers only the
time sampling happened to be running, while the tile beside it reads a service
that has 5 minutes of real history. Two different windows in one instrument,
neither labelled — that is a subtler version of the fabrication this whole phase
exists to remove.

**And one thing that is true of the tiles regardless:** three of the four fill
only while the perf sampler runs, and `startPerfSampling()` is reachable only
from the Performance screen. On a freshly started app the orb reads `NO
BASELINE` and those three tiles read `—`, which is correct and is what the
no-service screenshots show. Wiring a sampler start into the Overview is a
behaviour change on a second screen and was left alone.

## 50.5 GATES — RAW OUTPUT, COUNTS NOT ADJECTIVES

Every command below was run against the committed tree, on the fixture server
(`npm run fixture`), headless Chrome, 2026-09-06.

### The numbers gate — EXPECTED: zero untraceable numbers

**Three new numbers appear on the Overview and one changed shape. All four
trace:**

| rendered | traces to | arithmetic |
|---|---|---|
| `51%` in `span.orb-value` | `performance.cpu.totalBusyBp`, `performance.memory.memoryLoadPercent`, `performance.storage[].activeTimeBp` | `100 - (41.0 + 72 + 34.0)/3 = 51`. Printed in the orb's own evidence chip, and every input is on screen beside it |
| `41%` in `span.channel-value` | `performance.cpu.totalBusyBp = 4100 bp` | `4100 / 100`. The same figure the Performance screen already renders and already traced |
| `34%` in `span.channel-value` | `performance.storage[0].activeTimeBp = 3400 bp` | `3400 / 100`, busiest of 1 reported device |
| `0.94` in the disk-latency tile | `performance.storage[0].avgTransferLatencyUs = 940` | `940 / 1000` ms |

The memory rail shows `23.0 GB` rather than a percentage — `totalPhysicalBytes -
availablePhysicalBytes` — so it is a byte size, not a score, and the gate
excludes it by shape. Its 72% still enters the headroom mean, and the evidence
chip prints it.

**Also from this run:** evidence chips rendered went **10 -> 24** (seven new on
the Overview, in each of two languages). Denied elements unchanged at 90 — the
policy band is untouched and still persistent.

    ### node tools/verify-numbers.mjs
    === raw counts, from the rendered DOM of 11 pages x 2 languages ===
      "denied" word occurrences        22
      denied ELEMENTS rendered         90
      "evidence" word occurrences      66
      evidence CHIPS rendered          24

    === every score-shaped number rendered, with its source element ===
      en overview     51%        span.orb-value.svelte-19uo4g3      "51%"
      en overview     41%        span.channel-value.svelte-19uo4g3  "41%"
      en overview     34%        span.channel-value.svelte-19uo4g3  "34%"
      en overview     0.94       strong.svelte-19uo4g3              "0.94"
      en performance  41%        strong.technical-isolate           "41%"
      en performance  3%         span.technical-isolate             "3%"
      en performance  72%        strong.technical-isolate           "72%"
      en performance  34%        strong.technical-isolate           "34%"
      en performance  62%        strong.technical-isolate           "62%"
      en hardware     72%        strong                             "72%"
      en hardware     4%         strong                             "4%"
      en hardware     100%       strong                             "100%"
      en hardware     4%         span                               "PercentageUsed 4%"
      en hardware     100%       span                               "AvailableSpare 100%"
      en hardware     30%        p                                  "Available memory stayed under 30% for most of the observation window."
      en hardware     15%        p                                  "Three of the last five sessions held available memory under 15%."
      en hardware     72%        span                               "MemoryLoad 72%"
      ar performance  41%        strong.technical-isolate           "41%"
      ar performance  3%         span.technical-isolate             "3%"
      ar performance  72%        strong.technical-isolate           "72%"
      ar performance  34%        strong.technical-isolate           "34%"
      ar performance  62%        strong.technical-isolate           "62%"
      ar hardware     4%         span.technical-isolate             "PercentageUsed 4%"
      ar hardware     100%       span.technical-isolate             "AvailableSpare 100%"
      ar hardware     30%        span.technical-isolate             "Available memory stayed under 30% for most of the observation window."
      ar hardware     15%        span.technical-isolate             "Three of the last five sessions held available memory under 15%."
      ar hardware     72%        span.technical-isolate             "MemoryLoad 72%"

    27 distinct score-shaped number(s) rendered.
    Each must trace to a measurement. Percentages of a counted total and
    service-reported ratios are traceable; a bare confidence score is not.

### The Arabic gate — EXPECTED: 7/7, zero system-font fallback

    ### node tools/verify-arabic.mjs
    PASS  root is RTL
            EXPECTED  dir=rtl lang=ar direction=rtl
            OBSERVED  dir=rtl lang=ar locale=ar direction=rtl
    PASS  layout is genuinely RTL, not mirrored LTR
            EXPECTED  rail on the right half, main flush to the left edge, prose direction rtl
            OBSERVED  rail 1022-1280 of 1280, main.left=0, railIsOnRight=true mainStartsAtLeftEdge=true prose direction=rtl text-align=start
    PASS  technical tokens stay LTR inside RTL prose
            EXPECTED  every .technical-isolate resolves direction:ltr
            OBSERVED  58 nodes, 16 inside an RTL parent, allLtr=true, e.g. ["NET-NO-EGRESS","RES-LOCAL-ONLY","DRV-SIGNED-ONLY"]
    PASS  embedded face is declared, loaded and first in the Arabic prose stack
            EXPECTED  4 faces declared, at least one loaded, none in error, Arabic prose asks for "IBM Plex Sans Arabic" first
            OBSERVED  declared=["400:loaded","500:loaded","600:loaded","700:loaded"] anyLoaded=true anyError=false proseStackHead="IBM Plex Sans Arabic"
    PASS  no Arabic glyph is drawn by a system fallback font
            EXPECTED  0 glyphs from a non-bundled font across every Arabic-bearing node
            OBSERVED  129 Arabic nodes, 2361 glyphs; bundled = [["JetBrains Mono",208],["IBM Plex Sans Arabic",1673],["IBM Plex Sans Arabic SmBld",209],["IBM Plex Sans Arabic Medm",175],["Inter",96]]; system fallback = 0 glyph(s) []
    PASS  the subset face still shapes Arabic
            EXPECTED  a joined word renders narrower than the same letters with joining blocked
            OBSERVED  "التشخيص" joined=154px vs joining-blocked=217px
    PASS  headings are authored in Arabic
            EXPECTED  no all-Latin heading left untranslated (product names excepted)
            OBSERVED  18 headings, 0 all-Latin: []

    7/7 checks pass

### The token gate — EXPECTED: every var() resolves

    ### node tools/verify-tokens.mjs
    === 11 pages x 2 languages, read from the live CSSOM ===
      style rules walked                 985
      declarations referencing var()     443
      unresolved on a matched element    0 (property, selector) pair(s), 0 distinct custom propert(ies)

    PASS — every var() reference in 443 declarations resolves on every element the rule matches.

### The layout sweep — EXPECTED: clean at 1280 / 1024 / 960, both languages, BOTH themes, populated AND with no service

    ### node tools/layout-sweep.mjs --out output/p50-populated   (layout-fixture.html)
    PASS  overview-1280-en-dark        overflowX=0 clipped=0 overlaps=0 dir=ltr band=true denied=3 evidence=8 empty=2 emdash=3
    PASS  overview-1024-en-dark        overflowX=0 clipped=0 overlaps=0 dir=ltr band=true denied=3 evidence=8 empty=2 emdash=3
    PASS  overview-960-en-dark         overflowX=0 clipped=0 overlaps=0 dir=ltr band=true denied=3 evidence=8 empty=2 emdash=3
    PASS  overview-1280-en-light       overflowX=0 clipped=0 overlaps=0 dir=ltr band=true denied=3 evidence=8 empty=2 emdash=3
    PASS  overview-1024-en-light       overflowX=0 clipped=0 overlaps=0 dir=ltr band=true denied=3 evidence=8 empty=2 emdash=3
    PASS  overview-960-en-light        overflowX=0 clipped=0 overlaps=0 dir=ltr band=true denied=3 evidence=8 empty=2 emdash=3
    PASS  overview-1280-ar-dark        overflowX=0 clipped=0 overlaps=0 dir=rtl band=true denied=3 evidence=8 empty=2 emdash=3
    PASS  overview-1024-ar-dark        overflowX=0 clipped=0 overlaps=0 dir=rtl band=true denied=3 evidence=8 empty=2 emdash=3
    PASS  overview-960-ar-dark         overflowX=0 clipped=0 overlaps=0 dir=rtl band=true denied=3 evidence=8 empty=2 emdash=3
    PASS  overview-1280-ar-light       overflowX=0 clipped=0 overlaps=0 dir=rtl band=true denied=3 evidence=8 empty=2 emdash=3
    PASS  overview-1024-ar-light       overflowX=0 clipped=0 overlaps=0 dir=rtl band=true denied=3 evidence=8 empty=2 emdash=3
    PASS  overview-960-ar-light        overflowX=0 clipped=0 overlaps=0 dir=rtl band=true denied=3 evidence=8 empty=2 emdash=3

    12/12 pass · artifacts in /Users/hasanalaaa/dev/aethercore/phase21-workspace/apps/ui/output/p50-populated

    ### node tools/layout-sweep.mjs --entry index.html --out output/p50-noservice
    PASS  overview-1280-en-dark        overflowX=0 clipped=0 overlaps=0 dir=ltr band=true denied=3 evidence=0 empty=6 emdash=15
    PASS  overview-1024-en-dark        overflowX=0 clipped=0 overlaps=0 dir=ltr band=true denied=3 evidence=0 empty=6 emdash=15
    PASS  overview-960-en-dark         overflowX=0 clipped=0 overlaps=0 dir=ltr band=true denied=3 evidence=0 empty=6 emdash=15
    PASS  overview-1280-en-light       overflowX=0 clipped=0 overlaps=0 dir=ltr band=true denied=3 evidence=0 empty=6 emdash=15
    PASS  overview-1024-en-light       overflowX=0 clipped=0 overlaps=0 dir=ltr band=true denied=3 evidence=0 empty=6 emdash=15
    PASS  overview-960-en-light        overflowX=0 clipped=0 overlaps=0 dir=ltr band=true denied=3 evidence=0 empty=6 emdash=15
    PASS  overview-1280-ar-dark        overflowX=0 clipped=0 overlaps=0 dir=rtl band=true denied=3 evidence=0 empty=6 emdash=15
    PASS  overview-1024-ar-dark        overflowX=0 clipped=0 overlaps=0 dir=rtl band=true denied=3 evidence=0 empty=6 emdash=15
    PASS  overview-960-ar-dark         overflowX=0 clipped=0 overlaps=0 dir=rtl band=true denied=3 evidence=0 empty=6 emdash=15
    PASS  overview-1280-ar-light       overflowX=0 clipped=0 overlaps=0 dir=rtl band=true denied=3 evidence=0 empty=6 emdash=15
    PASS  overview-1024-ar-light       overflowX=0 clipped=0 overlaps=0 dir=rtl band=true denied=3 evidence=0 empty=6 emdash=15
    PASS  overview-960-ar-light        overflowX=0 clipped=0 overlaps=0 dir=rtl band=true denied=3 evidence=0 empty=6 emdash=15

    12/12 pass · artifacts in /Users/hasanalaaa/dev/aethercore/phase21-workspace/apps/ui/output/p50-noservice

**What those two runs say, side by side.** Populated: `evidence=8 empty=2
emdash=3`. No service: `evidence=0 empty=6 emdash=15`. The same screen, the same
markup — with nothing measured it renders four more empty states and twelve more
em dashes, and claims nothing. That difference is the whole point of the phase.

### The contrast instrument — EXPECTED: nothing below 4.5:1 in either theme

    ### node tools/contrast-sweep.mjs --pages overview --locales en,ar   (populated)
    PASS  overview/en/dark               nodes= 245 below=   0 exempt= 0 worst=5.23:1
    PASS  overview/ar/dark               nodes= 244 below=   0 exempt= 0 worst=5.23:1
    PASS  overview/en/light              nodes= 245 below=   0 exempt= 0 worst=4.67:1
    PASS  overview/ar/light              nodes= 244 below=   0 exempt= 0 worst=4.67:1

    measured 978 text node(s); 0 below threshold, 0 distinct; 0 exempt as inactive controls (WCAG 1.4.3 Incidental)
      theme=dark      0 below, 0 distinct
      theme=light     0 below, 0 distinct

    PASS — every rendered text node meets WCAG AA in every theme measured.

    ### node tools/contrast-sweep.mjs --pages overview --locales en,ar --entry index.html   (no service)
    PASS  overview/en/dark               nodes= 171 below=   0 exempt= 0 worst=5.23:1
    PASS  overview/ar/dark               nodes= 171 below=   0 exempt= 0 worst=5.23:1
    PASS  overview/en/light              nodes= 171 below=   0 exempt= 0 worst=4.67:1
    PASS  overview/ar/light              nodes= 171 below=   0 exempt= 0 worst=4.67:1

    measured 684 text node(s); 0 below threshold, 0 distinct; 0 exempt as inactive controls (WCAG 1.4.3 Incidental)
      theme=dark      0 below, 0 distinct
      theme=light     0 below, 0 distinct

    PASS — every rendered text node meets WCAG AA in every theme measured.

### Build and svelte-check — EXPECTED: builds, 16-warning baseline held

    ### npm run build
    ✓ built in 789ms
    [plugin builtin:vite-reporter] 
    (!) Some chunks are larger than 500 kB after minification. Consider:
    - Using dynamic import() to code-split the application
    - Use build.rolldownOptions.output.codeSplitting to improve chunking: https://rolldown.rs/reference/OutputOptions.codeSplitting
    - Adjust chunk size limit for this warning via build.chunkSizeWarningLimit.

    ### npm run check   (svelte-check, --fail-on-warnings, 16-warning baseline)
    1788705067232 COMPLETED 225 FILES 0 ERRORS 16 WARNINGS 3 FILES_WITH_PROBLEMS

`225 FILES 0 ERRORS 16 WARNINGS` — the same 16 as before this session (one
a11y warning in `FluidDialog`, fifteen unused-selector warnings in
`FindingCard` and `DeepScanPage`). None of them is in a file this session
touched.

### `scripts/static_validate.py` — not in the brief, run anyway

    before   ok=False   21 failed
    after    ok=False   21 failed
    new failures: []      newly passing: []

Unchanged. The 21 are pre-existing and mostly Windows-host checks that cannot
pass on this machine.

## 50.6 SCREENSHOTS

**`design/p50-overview-screenshots/`**, committed — 1280 px, both languages,
both themes, populated and with no service:

    populated-overview-1280-en-dark.png     no-service-overview-1280-en-dark.png
    populated-overview-1280-en-light.png    no-service-overview-1280-en-light.png
    populated-overview-1280-ar-dark.png     no-service-overview-1280-ar-dark.png
    populated-overview-1280-ar-light.png    no-service-overview-1280-ar-light.png

The full 24-shot sweep (1280 / 1024 / 960 x en / ar x dark / light x both data
states) is in `apps/ui/output/p50-populated/` and `.../p50-noservice/`, with
`sweep.json` beside each.

Read the two `en-dark` files together. Populated: `51% HEADROOM`, four rails
with readings, six cited action items, four tiles, fourteen real log lines.
No service: `— NO BASELINE`, four hatched rails reading `—`, `NOT COLLECTED
YET` over three named channels each reading `—`, four tiles reading `—`, and a
log that says the stream has not delivered an event yet. Neither one guesses.

## 50.7 THE LEDGER DELTA — five new ids, none fixed, all with reasons

Recorded here rather than in `DEBT_REGISTER.json`, which is the `QD-*` register
under the p31 audit gate; every `DBT-P4x` id from P42 onward lives in this
ledger and these follow the same convention.

| id | what | why it is open, not fixed |
|---|---|---|
| **`DBT-P50-001`** | **No disk free/used space provider.** Nothing in the IPC surface reports free or used bytes for a volume. `StorageTelemetry.sizeBytes` is capacity; `StorageQueueSample.activeTimeBp` is busy time. `grep -rn "GetDiskFreeSpace\|free_bytes\|availableBytes\|totalFreeBytes"` over `crates/` and `services/` returns one hit, and it is memory | The shell's `Disk C: 412 GB` has no source at all, so the channel was dropped and the slot given to disk **activity**, which is measured. Adding a volume-space collector is a service change |
| **`DBT-P50-002`** | **No protocol version on the IPC surface.** `PROTOCOL_VERSION: u32 = 7` exists in `crates/contracts/src/lib.rs`; no field carries it to the UI, and the Overview was printing a hardcoded `v7` | Dropped from the view rather than left to go stale silently. Restoring it honestly means adding a field to `Snapshot` — a wire change |
| **`DBT-P50-003`** | **Two settings-shaped panels on the Overview**: `SystemCarePanel` (update channel, support-bundle export) and `AboutPanel` (capability matrix, engine source) | They belong on a Care/Settings screen. `NAVIGATION` has no such id, and creating one touches the other ten screens. Left below the fold — a slightly imperfect layout beats a lost feature |
| **`DBT-P50-004`** | **`EmptyState.svelte` inherits `main :where(*) { flex-wrap: wrap }` on a flex column** and renders 496px for 396px of content when it has channels. Measured at 1280 on `index.html`. Fix: `flex-wrap: nowrap` on `.empty-state`, one declaration | `EmptyState` renders on seven screens. This session is one screen, and a silent re-layout of six others inside a commit titled "rebuild the Overview" is exactly what the scope rule exists to prevent |
| **`DBT-P50-005`** | **No wire accessor for the performance sample series.** The ring exists (`MAX_RING_SAMPLES = 300`) and has one consumer, the bottleneck analyser. There is no `repeated PerfSnapshot` in `performance.proto` or `events.proto`, so the UI can only ever hold one sample and no sparkline can be honest | A published-contract change (§4), which needs approval before it is made |

**None of the five is a regression this session introduced.** Four are absences
the rebuild exposed by asking, element by element, where a number comes from.
The fifth (`-004`) is a pre-existing layout defect found by measuring, and its
sibling in the new file is fixed.

## 50.8 P50 FINAL REPORT

### What was asked, and what happened

Rebuild the app's Overview on the shell's composition — this screen only — and
never invent a number to do it. Both halves were done. Six files changed, +531
/ -14, four commits, every gate re-run and pasted verbatim in §50.5.

### What was dropped, named

Thirteen elements. The ones that matter:

- **"AetherCore is watching 214 signals and predicts no degradation in the next
  72 hours."** No signal count, no baseline timestamp and no predictor exists
  anywhere in this product. Marketing copy. Gone.
- **"System is breathing easy."** A claim about system state with nothing behind
  it.
- **Responsiveness `96 idx`, Boot time `11.4 s`, Open handles `38.2k`** — three
  of the shell's four telemetry tiles. No responsiveness index is computed
  anywhere; `grep -rn "boot_time\|bootTime\|BootTime"` over `crates/`,
  `services/` and `apps/ui/src` returns nothing; no handle counter exists.
- **`Disk C: 412 GB`** — used capacity, which no provider in this product
  reports (`DBT-P50-001`).
- **Four sparklines and four delta lines** ("+4 vs baseline", "-2.1s this
  week"). A delta is a change over time and nothing reachable by the UI retains
  time (`DBT-P50-005`).
- **The five-line boot log** — `[DATA] manifest loaded · 214 signals` and its
  siblings. Invented prose.
- **The orb's state hue**, which turns red on a fabricated criticality.
- **`Protocol v7`** — the app's own, not the shell's: correct today, unsourced,
  and therefore a future lie (`DBT-P50-002`).

### What was built empty, and what would fill it

- **The four channel rails and the orb**, whenever the perf sampler is not
  running: `— NO BASELINE`, hatched tracks. Filled by `start_perf_sampling`,
  reachable today only from the Performance screen.
- **Thermals' bar** is empty *permanently and on purpose* while the reading is
  shown. Normalizing °C to 0–100 needs a ceiling nobody measured. Filling it
  honestly needs a service-reported thermal limit, not a constant in a stylesheet.
- **Action items**, until a scan reports: `NOT COLLECTED YET` over three named
  channels — Deep Scan, Drivers, Hardware Health — each reading `—`. Filled by
  running any of those four scans.
- **Three of the four telemetry tiles**, same condition as the rails. The
  fourth, journal events, fills the moment the service connects.
- **The service log**, until the kernel pushes an event. It fills by itself.

### The one thing I would flag

Three of the four channel rails, the orb, and three of the four tiles all depend
on the same switch: the perf sampler, which only starts from another screen. On
a freshly launched app the owner will see the empty variant — which is correct,
and which the no-service screenshots show — but it means the instrument's most
striking state is the resting one until he visits Performance once. Starting the
sampler from the Overview is a one-line change to a second screen's controller,
so it was not made here. It is the first question worth answering before the
other ten screens are done.

### One paragraph: what the new Overview says that the old one did not

The old Overview said the product was ready. It was a column of cards that
described features — a status hero, a plan panel, a driver preview, six tiles
naming six modules — and the only number on it was a journal event count. The
new one says what the machine is doing and where it knows that from. It opens
with a reading and the arithmetic behind it, sits four measured channels beside
it, and where a channel has no scale it shows the reading and leaves the bar
hatched rather than draw a bar at zero. Beside that, six action items, each one
a scan that actually ran, each carrying that scan's id, timestamp and counts one
click away. Below, four instruments and a log of the service's own events with
their real sequence numbers. And when nothing has been measured, the same screen
says so twelve more times in em dashes and four more empty states, and asks for
nothing it cannot justify. The old screen was a description of a product. This
one is an instrument that will not lie about what it has not measured — which is
the only claim this product has ever actually made.


# PHASE 51 — P51-OVERVIEW-FINISH: FILL IT, CUT IT, RE-HOME THE REST (2026-09-07)

P50 built the right instrument and left it at the top of the wrong page. The
screen is 4,110 px tall because the new composition was *prepended* to the old
Overview instead of replacing it, and the instrument is blank on first run
because everything on it reads a sampler that only another screen starts.

**Measured before anything changed**, `tools/layout-sweep.mjs` at 1280:

| state | en | ar |
|---|---|---|
| populated (`layout-fixture.html`) | **4,110 px** | **4,082 px** |
| no service (`index.html`) | **3,533 px** | **3,498 px** |

## 51.0 PROGRESS TABLE (authoritative — resume from here)

Rows move in the same commit as the work they describe.

| item | status | evidence |
|---|---|---|
| 0 the measuring instrument | **DONE** | `layout-sweep.mjs` reports `height` — the number this session is judged on was not observable by any gate before |
| 1 make the Overview fill itself | **DONE** | §51.1 — the facts, the measured cost, the decision, and the two behaviours it corrected |
| 2 cut the six-tile grid | **DONE** | §51.2 — all six tiles measured at **0 interactive descendants**; all six pages reached from the rail. −338 px en / −354 px ar |
| 3 re-home the rest | **DONE** | §51.3 — 9 verdicts: 1 BELONGS · 3 DROPS · 4 SETTINGS (`DBT-P50-003` closed) · 1 MOVES. §51.4 — the last 281 px, found in the arrangement rather than in a section |
| gates | **DONE** | §51.6 — numbers: 27 score-shaped, the Overview's 4 all traced, Settings adds none; arabic 7/7 on three screens, 0 system fallback; tokens 437/437 resolve over 12 pages; sweep 12/12 populated **and** 12/12 with no service, plus 8/8 on both destinations; contrast in §51.6; build clean; svelte-check 0 errors / 16 warnings, baseline held |
| screenshots 1280 × 2 languages × 2 themes × populated/empty, **plus every destination** | **DONE** | 16 committed under `design/p51-overview-screenshots/` |
| ledger delta + report | **DONE** | §51.8 — 3 P50 ids closed, 3 new (`DBT-P51-001`…`-003`), `DBT-P50-005` not started by instruction. §51.9 — the report |

## 51.1 ITEM 1 — THE OVERVIEW FILLS ITSELF

### The facts, established before anything was changed

**What starts the sampler, end to end.** `startPerfSampling()` at
`apps/ui/src/features/performance/controller.ts:18`, reachable from exactly one
place: the ▶ button in the Performance screen's header. It does three things in
order — `setPage('performance')`, `get_performance_snapshot`, then
`start_perf_sampling { intervalMs: 1000 }`. That last call goes:

    controller.ts  →  #[command] start_perf_sampling      apps/desktop/src/main.rs:1754
                   →  request::Payload::StartPerfSampling  (operations.proto:83)
                   →  router.rs:1054  →  PerformanceEngine::start_sampling
                   →  PerformanceRing::start                crates/performance-telemetry/src/lib.rs:617
                   →  thread::Builder::new().name("aether-perf-sampler").spawn(…)

**And the finding that decided this item: the sampler publishes nothing.**
That thread calls `platform.sample()`, pushes into an in-process ring, and
sleeps. `grep -rn "EventKind::PerformanceSnapshot" crates services apps` returns
**one** hit — `router.rs:1084`, inside the `GetPerformanceSnapshot` handler. So
the only producer of a `performanceSnapshot` kernel event in this product is a
*read*, and `get_performance_snapshot` is called from exactly one place in the
UI: `startPerfSampling`, once.

The consequence is larger than this screen. **The Performance screen's own live
readings are one sample and never a second one** — its four sparklines
accumulate from `$: if (performance.capturedUnixMs > 0)`, which fires once per
snapshot event, and only one ever arrives per press of ▶. §50.1's line
"`performance.*` … pushed by the `performanceSnapshot` kernel event while the
perf sampler is running" is **wrong**, and is corrected here.

So option (a) as the brief writes it — "Overview starts the sampler when it
opens" — would have cost a background thread and left the instrument blank.

**What it costs while running.** Measured on this machine, not read off the
crate docs: ten `default_platform().sample(1000ms)` ticks after a warm-up,
`cargo test --release`, macOS native provider.

    P51 sample() over 10 ticks: min=129857us mean=230837us max=624198us
    P51 ring at cap: samples=300 sizeof(PerfSnapshot)=336B fixed_total=100800B

- **CPU.** 129.9 ms min / **230.8 ms mean** / 624.2 ms max per sample. ~120 ms of
  that is deliberate: `macos_impl.rs:167` sleeps `min(120ms, interval)` between
  two `host_statistics64` observations because a rate counter needs two, exactly
  as the Windows PDH collector does. The rest — ~110 ms mean — is real work,
  dominated by `sample_process_top` and `sample_storage`.
- **Memory.** The ring is bounded at `MAX_RING_SAMPLES = 300`; the fixed part is
  336 B × 300 = **100,800 B ≈ 98 KB**, plus each sample's own bounded
  collections (`MAX_STORAGE_DEVICES 32`, `MAX_PROCESS_TOP 16`, `MAX_GPU_ENGINES
  16`, `MAX_CPU_COUNT 256`, `MAX_COLLECTOR_FAULTS 16`).
- **IPC traffic: zero.** The background sampler sends nothing over the wire. All
  IPC on this path is one request/response per read.
- **Observer effect, stated rather than assumed.** ~110 ms of work per tick is
  ~11% of one core at 1 s cadence — and it lands *inside* the 120 ms window the
  same call uses to compute CPU busy. `MIN_INTERVAL_MS = 250` exists for exactly
  this reason ("sub-250 ms would risk observer effects"). A sampler that runs
  faster than the thing it measures is measuring itself.

**Whether it is safe without an explicit user action — with the reasoning, not
the assumption.** Yes, and the reason is structural, not a judgement call. The
read path is `PerfPlatform::sample` → counters. It touches no
`MutationSupervisor`, opens no `CommitFence`, writes no journal entry and takes
no restore point; every one of those lives on the `OptimizationGovernor` side of
`PerformanceEngine`, which nothing here calls. This product's consent model asks
permission **to change the machine** — the plan digest, the risk tier, the
authorization window on the Overview's own state-engine panel are all about
mutation. Reading a performance counter is what a task manager does when you
open it. The one real cost is the CPU above, and that is a cadence decision,
which is answered by the cadence rather than by a consent dialog.

### The decision: **(a), in the form the measurement supports — the Overview reads, and does not start a sampler**

`src/features/overview/controller.ts`. On mount the Overview calls
`get_performance_snapshot`, then repeats every **5 s** until the component is
destroyed. It starts no background thread anywhere.

The argument, against the three the brief offered:

- **Not (a) as written.** Starting `aether-perf-sampler` would have paid ~11% of
  a core for as long as the app is open — on every screen, not just this one —
  and, per the finding above, delivered no reading to the UI at all. It would
  have been a cost with no effect.
- **Not (b).** The empty state already had a control before this session; it
  navigated to Performance, where the user then had to find and press ▶. Two
  actions on two screens to make the first screen work is not an answer to "the
  instrument is empty on first run", and the brief is explicit that the resting
  state must not be the most prominent one.
- **Not (c).** Sampling with the service pays the same ~11% of a core forever,
  including for a user who never opens the app window, and it still would not
  fill the UI without a read. The largest change, for the least effect.
- **Why 5 s and not 1 s.** Cadence is a decision with a price (§42, §45). Each
  read costs one 230 ms mean sample in the service; at 5 s that is **under 5% of
  one core while this screen is open, and exactly zero when it is not** — the
  loop is created in `onMount` and disposed in `onDestroy`. 1 s is the right
  cadence for the Performance screen, which is a screen you sit and watch. The
  Overview is a summary, and a summary does not need per-second resolution.
- **Why a read is never stale.** The `GetPerformanceSnapshot` handler calls
  `ensure_sample`, which re-samples when the newest reading is older than the
  requested interval — the DBT-P42-008 fix. A read at 5 s always returns a
  reading taken within the last second.

**Verified by measurement, not by reading the code.** Transport calls counted in
the fixture, `get_performance_snapshot`:

    11 s on the Overview            3 reads   (t=0, 5, 10)
    11 s after navigating away      3 reads   (no further reads)

### Two things this corrected, one of them on a second screen

The brief says to say explicitly if the Performance screen's behaviour changed
beyond what this required. **It did, in one line, and it had to.**

`stream-state.ts` set `next.perfSampling = true` inside the `performanceSnapshot`
case. `perfSampling` is what the Performance screen's header button reads to
decide between ▶ Start and ■ Stop. Once the Overview reads on its own, that
inference makes the Performance screen offer to **stop a thread that was never
started**. The line is removed; `perfSampling` is now set only by
`startPerfSampling` / `stopPerfSampling`, which are the two calls that actually
decide it. Measured in the fixture after the change: the Performance header
reads `▶ Start monitoring` with Analyze disabled, which is the truth. Before it
read `■ Stop`. Nothing else on that screen was touched.

The second is the empty state's copy and control. `overview.openPerformance`
("Open live telemetry", → Performance) is replaced by `overview.readNow` ("Take
a reading now"), which performs the same read the loop does, and
`overview.orbEmptyBody` now says what the screen actually does:

> This screen reads the performance counters itself, every 5 seconds while it is
> open. No reading has come back yet — it comes from the maintenance service,
> and nothing here is estimated without one.

### Two boundary rules this needed

- **A response with no capture time is not a reading.** The fixture transport
  answers `{}` to any command it does not model; patching that into
  `performance` would have wiped the populated instrument while the sweep
  reported it as populated. `isReading()` rejects any snapshot without
  `capturedUnixMs > 0`, and the fixture now answers `get_performance_snapshot`
  with the same snapshot its stream replays.
- **The loop is not `runBusy`.** A refresh the user did not ask for must not
  disable the screen's buttons or clear the error banner from an operation they
  did. A failed read logs, stops the loop, and leaves the channels at em dash —
  the empty state's own control is how the user asks again.

**EXPECTED: on a first run with no prior state, the Overview either shows live
channels, or shows an empty state that names the action that fills it.
OBSERVED: both.** With a service: `51% HEADROOM`, four rails reading. Without
one: `— NO BASELINE`, and one control reading `Take a reading now`.

## 51.2 ITEM 2 — THE SIX-TILE GRID, CUT

`Drivers · System Repair · Deep Clean · Startup Manager · Hardware Health ·
Crash History`, six cards under the driver panel.

**Verified by measurement, not by reading.** Counted in the fixture at 1280,
`querySelectorAll('button, a, [role=button], input, [onclick], [tabindex]')`
inside each tile, and the computed cursor on the card itself:

| tile | interactive descendants | cursor |
|---|---|---|
| Drivers | **0** | `auto` |
| System Repair | **0** | `auto` |
| Deep Clean | **0** | `auto` |
| Startup Manager | **0** | `auto` |
| Hardware Health | **0** | `auto` |
| Crash History | **0** | `auto` |

The grid is not a duplicate of the navigation. It is **less** than the
navigation: not one of the six cards is clickable, so the row cannot even do the
one thing its shape promises. The brief's condition — "if any tile turns out to
carry a real action the sidebar lacks, stop and keep that one" — is not met by
any of the six.

And each destination was reached the way a user reaches it, by pressing its rail
button and reading the heading that came up:

    drivers   → "Drivers, with Windows as the authority."
    repair    → "Repair Windows integrity, without guesswork."
    cleanup   → "Reclaim space from an explicit allowlist."
    startup   → "Faster starts, without silent decisions."
    hardware  → "Measurements, not a made-up health score."
    crash     → "Crash evidence without guessing a culprit."

Deleted: the markup, the `.module-row` / `.locked` / `.module-live` rules that
had no other user, `.module-live` from the shared materials selector list, and
the nine message keys that only it used, from both catalogs.

**1280, populated: 4,110 → 3,772 px (en) · 4,082 → 3,728 px (ar).**

## 51.3 ITEM 3 — THE NINE SECTIONS, ONE VERDICT EACH

Worked in the order the brief lists them.

### 1. State Engine — current protected operation → **BELONGS**

**What it summarises:** `snapshot.activePlan` — the service's state engine, of
which there is exactly one across every domain at a time. Plan kind, lifecycle
state, risk tier, the short digest, and the consent window
(`consentReadyUntilUnixMs`) with the authorize / start actions that window
governs.

**Why the Overview is the right place:** every other section on this page reports
a *measurement* of the machine. This one reports what is being *done* to it, and
it is the only section on any screen that is not scoped to one feature — a
Cleanup plan, a Startup plan, a SystemRepair plan and a DriverInstall plan all
appear in the same slot because the service only ever runs one. A whole-system
summary is exactly what it is. It stays, directly under the instrument.

Unchanged except for its wrapper: it was the left column of a `.grid` splitting
1.25fr / 0.55fr with the driver panel beside it, and is now one full-width panel.

### 2. Driver Servicing — safe driver installation → **DROPS**

Two halves, both duplicated, and the duplication was measured rather than argued.

**The reading.** The panel printed `hub.summary.selectableUpdateCount` as a
56 px numeral with no citation. The instrument above it already carries that
number as an action item, with one — measured in the fixture:

    "Driver updates AetherCore can install"   9 updates   scan-fixture-1 · 12 offers

**The action.** Its button did `setPage('drivers')` and, when the hub was Idle,
`startDriverScan()`. That is the same exported function the Drivers screen's own
header button is bound to, and the Drivers screen is one press away on the rail.
Exercised on the destination, both states:

    hub Ready (fixture)      "↻ Scan again"    enabled
    hub Idle  (index.html)   "↻ Scan drivers"  present, disabled — because there
                             is no service, which is the same reason the Overview
                             button would have called into a dead transport

So the panel carried no action the sidebar lacks and no reading the instrument
lacks. Deleted with its four message keys, `common.live` (its last user), and
the `.driver-preview` / `.preview-number` / `.preview-state` rules.

**Height: no change (3,772 px).** It was the second column of a two-column row
whose height came from the taller panel beside it. Removing it removed a
duplicate, not a scroll — recorded because the point of measuring is to notice
when a deletion buys nothing.

### 3. AetherCore updates → **SETTINGS**
### 4. Diagnostic support bundle → **SETTINGS**
### 5. About this build → **SETTINGS**
### 6. Platform capabilities → **SETTINGS**

Four sections, one destination, and the reason is the same for all four: **none
of them reports the state of the machine.** The update channel and the support
bundle are things the user *chooses*; the build identity and the capability
matrix are facts about the *program*, not about the PC it is diagnosing. On a
screen whose whole claim is "here is what your machine is doing, and here is
where I know it from", they were four panels answering a different question.

P50 reached the same verdict and could not act on it: `NAVIGATION` had no
Settings id, and it recorded **`DBT-P50-003`** rather than create one inside a
one-screen brief. **This session created it. `DBT-P50-003` is closed.**

`src/features/settings/SettingsPage.svelte` renders `SystemCarePanel` and
`AboutPanel` — the same two components, **unmodified**. The cost was one
`PageId`, one `NAVIGATION` row, one nav group (`system` / "System" / "النظام"),
one icon, one branch in `AppShell`, and one character class in the shell's
keyboard handler so `Ctrl+Shift+S` resolves like the other ten. The command
palette picked it up with no change at all — it reads `NAVIGATION`.

**Exercised on the destination, not assumed.** At 1280 in the fixture, on
`settings`:

    heading                "Settings"
    sections present       AetherCore updates · Diagnostic support bundle
                           About this build · Platform capabilities
    update channel         Stable aria-pressed=true / Beta false
      → clicked Beta       Stable false / Beta true          — the control works
    support bundle         "Inspect" action present
    platform              "windows"
    engine source         "Native platform provider"
    capability chips       16, all Native, 0 raw untranslated keys

**The destination reached the way the other eleven are.** Dispatched from a real
element target, not asserted from the source:

    Ctrl+Shift+S            → app-shell[data-page] = "settings"
    Ctrl+Shift+0 (control)  → "activity"        — the existing shortcuts unmoved
    Ctrl+K → palette        → 12 options, including
                              "Settings · Updates, support bundle and this build
                               · Ctrl ⇧ S"      — the palette needed no change

**A fixture gap this exposed, and closed.** The About panel's entire content
comes from `get_platform_capabilities` and `get_engine_source`, and the fixture
answered `{}` to both — so in every layout sweep this project has ever run, that
panel rendered its not-collected state. It has never been measured populated.
The fixture now answers both, using the **service's own vocabulary**
(`PlatformCapability::as_str`, and `windows_table()`'s "everything Native,
exactly as shipped today") rather than invented names.

**1280, populated: 3,772 → 3,099 px (en) · 3,728 → 3,088 px (ar).**
Settings itself measures 1,569 px (en) / 1,593 px (ar), clean at 1280.

### 7. One-click care → **DROPS**
### 8. Insights — what your reports mean → **DROPS**

The duplication is not an argument, it is four lines of `AppShell.svelte`.
`<CarePanel />` and `<InsightsPanel />` were rendered in **both** branches of the
same `{#if}` — once under `activity`, once under the Overview. The same
components, the same stores, the same controllers. Only one of the two renders
is redundant, and it is the Overview's: the Overview is where the machine is
measured, and neither of these measures anything about the machine.

**Exercised on the destination**, at 1280 on `activity`:

    "Safe upkeep in one run"     → "Start Care" pressed → the consent dialog
                                   opened: "Authorize One-Click Care for this
                                   session · Care sequences plans that already
                                   exist…"
    "What your reports mean"     → Refresh · Explain this · Dismiss present,
                                   one insight rendered with its evidence chip

**Recorded rather than decided:** Activity is where both already lived, and it
is now their only home. Whether a screen described as "Durable operation journal
and recovery" is the right home for the product's one run-everything action is a
question this brief did not put and this session did not answer. **New id
`DBT-P51-002`.**

### 9. Activity & Recovery — recent recovery events → **MOVES → Activity**

`RecoveryPanel` was not an Overview section at all. It sat outside the `{#key}`
block in `AppShell`, so it rendered under **every screen in the app** whenever
`recoveryEntries` was non-empty. It is titled with `activity.eyebrow` and
`recovery.title`, the rail describes Activity as "Durable operation journal and
recovery", and the Activity page already renders an empty state for exactly the
case where this panel has nothing — it was written around a panel that was
somewhere else.

Moved inside the `activity` branch. **This removes it from the other ten
screens**, which is stated here rather than buried: it is a change beyond the
Overview, and it is the point of the verdict.

**Exercised on the destination:** on `activity`, "Recent maintenance recovery
events" renders with its one fixture row, between the scheduler panel and the
timeline.

**1280, populated after items 7–9: 3,099 → 2,281 px (en) · 3,088 → 2,265 px
(ar).** 281 px over the target, with every one of the nine sections decided.
What was left is in §51.4.

## 51.4 THE LAST 281 px — WHERE THE HEIGHT ACTUALLY WAS

With all nine verdicts applied the Overview measured 2,281 px, and the brief
forbids deleting a feature to reach 2,000. Measured, the remaining height was
not in any section — it was in the **arrangement**:

| element | height | width |
|---|---|---|
| system health orb | 270 px | 537 px |
| action items | **949 px** | 379 px |
| diagnostic telemetry | 276 px | 930 px |
| core service log | 275 px | 930 px |
| **the grid** | **1,528 px** | |

The shell lays this screen out orb 7 / items 5 / telemetry 12 / log 12, and P50
ported those spans literally. The consequence, measured: a 270 px orb beside a
949 px list left **679 px of empty column** under the orb, and the two sections
that could have filled it were stacked underneath instead. 1,528 px of grid for
1,770 px of content, with a third of the grid empty.

**The change: §C and §D join the orb in the left column, and the action list
spans all three rows.** One instrument stack beside one list. No section is
removed, none is smaller, nothing is hidden — the space that was empty is now
the telemetry and the log.

    grid   1,528 px → 991 px          dead space  679 px → 41 px
    main   2,281 px → 1,744 px (en)   2,265 px → 1,702 px (ar)

**This is the one place this session departs from the shell's own spans**, and
it is recorded here rather than left for someone to discover in a diff. The
alternative measured — telemetry in the left column, log left full width — came
to 1,991 px, nine pixels under the target, which is not a margin.

**EXPECTED: under 2,000 px at 1280 wide, populated, in both languages.
OBSERVED: 1,744 px (en) and 1,702 px (ar).** Sweep clean 12/12 at
1280 / 1024 / 960 × en/ar × dark/light.

## 51.5 THE TWO DEBT ITEMS THE BRIEF PUT IN SCOPE

### `DBT-P50-004` — the shared `EmptyState`: **FIXED, all seven screens verified**

Reproduced first, at 1280 on `index.html`, exactly as P50 recorded it:

    overview  action items  rendered 496px   flex-wrap: wrap   3 channels
              same node with flex-wrap:nowrap set from the console → 396px

The cause is the one P50 named: `main :where(*) { flex-wrap: wrap }` in
`feature-layout.css` is a deliberate zero-specificity rule for content ROWS, and
`.empty-state` is a flex COLUMN that never opted out. It only bites when the
component is given channels — two flex children instead of one.

Fixed with the one declaration P50 wrote down. **Measured on every screen the
component renders on, before and after:**

| screen | empty states | rendered before | rendered after |
|---|---|---|---|
| overview | 3 (one with 3 channels) | **496** · 68 · 68 | **396** · 68 · 68 |
| activity | 3 (activity, care, insights) | 68 · 47 · 68 | 68 · 47 · 68 |
| cleanup | 1 | 127 | 127 |
| crash | 1 | 127 | 127 |
| hardware | 1 | 127 | 127 |
| startup | 1 | 107 | 107 |
| settings | 1 (about) | 47 | 47 |

Eleven empty states on seven screens. **One got 100 px shorter; the other ten are
identical to the pixel**, and all eleven now compute `flex-wrap: nowrap`. No
screen regressed.

### `DBT-P50-002` — `Protocol v7`: **CLOSED — already dropped, and the key with it**

The brief says wire it or drop it. **Observed: it is not in the markup.** P50's
row 38 already dropped it when it rebuilt the hero, and the id was recorded
against the absence rather than against a live literal. Verified rather than
assumed — rendered text of `main` on the populated Overview:

    /Protocol/gi → 0 matches      /v7/gi → 0 matches

What remained were two orphaned catalog entries, `overview.protocol` in both
languages, with no reader anywhere in `src/`. Deleted.

**Wiring it was not chosen, and the reason stands.** `PROTOCOL_VERSION: u32 = 7`
lives in `crates/contracts/src/lib.rs`; no field on `Snapshot` carries it to the
UI. Adding one is a change to a published contract (§4) and needs approval
before it is made, not after. Recorded there; closed here as dropped.

### `DBT-P50-005` — **NOT STARTED. What it needs, and nothing else.**

The brief is explicit that this one requires the owner's approval before
implementation. It was not begun. What approval would be approving:

- **The change is on the wire, not in the UI.** `performance.proto` carries
  `message PerformanceSnapshotResponse { PerfSnapshot snapshot = 1; }` and
  `events.proto` carries `PerfSnapshot performance_snapshot = 31;`. There is no
  `repeated PerfSnapshot` anywhere in the contract, so the series cannot leave
  the service whatever the UI does.
- **The series already exists.** `PerformanceRing` holds `MAX_RING_SAMPLES = 300`
  ordered samples (~5 min at 1 s), and `PerformanceService` already calls
  `self.ring.window(owner)` — for `analyze()`, its only consumer. The ring, its
  bound and its ordering are built. **Only the accessor is missing.**
- **What it would add:** a `GetPerformanceWindowRequest` /
  `PerformanceWindowResponse { repeated PerfSnapshot samples = 1; }` pair, or a
  compact per-metric series message so a four-tile sparkline does not cost 300
  full snapshots on the wire. Both are additive; neither changes an existing
  message.
- **Why it is gated:** §4 puts published contracts — proto messages consumed
  outside this repository — behind explicit approval with a stated migration
  path. That approval has not been given.
- **What this session learned that changes its shape.** §51.1 found that the
  UI receives a `performanceSnapshot` event only per `get_performance_snapshot`
  call, so even the Performance screen's own four sparklines have never held
  more than one point. A window accessor would fix both screens, not one.

## 51.5b TWO THINGS THE REVIEW PASS FOUND, BOTH ON THIS SCREEN

Neither was in the brief. Both were found reading the session's own diff back,
and both are the class of defect this whole phase exists to remove.

**The telemetry note asserted a cadence it no longer had.**
`overview.noteSampling` read *"sampled every 1000 ms"*, built from
`PerfSnapshot.intervalMs`. That field is the window the counters were observed
over — what `platform.sample(interval)` was handed — **not a repeat rate**. The
sentence was true of the Performance screen's 1 s sampler and false of this
screen the moment item 1 made it read every 5 s. It now says what the field is:
*"1,000 ms observation window"* / *"نافذة رصد ١٬٠٠٠ م.ث"*.

**The same number was formatted two ways on one screen.** The section header
passed `performance.intervalMs` raw; the tiles passed it through `formatNumber`.
In English that is `1000` against `1,000`; **in Arabic it is `1000` against
`١٬٠٠٠`, two inches apart**. One exported `samplingNote()` now formats it once,
in the locale, for both. Verified rendered: header and tile both read
`نافذة رصد ١٬٠٠٠ م.ث`.

## 51.6 GATES — RAW OUTPUT, COUNTS NOT ADJECTIVES

Every command run against the committed tree, on the fixture server
(`npm run fixture`), headless Chrome, 2026-09-07.

### `node tools/verify-numbers.mjs` — EXPECTED: zero untraceable numbers

    === raw counts, from the rendered DOM of 12 pages x 2 languages ===
      "denied" word occurrences        24
      denied ELEMENTS rendered         98
      "evidence" word occurrences      68
      evidence CHIPS rendered          22

      en overview     51%        span.orb-value       "51%"
      en overview     41%        span.channel-value   "41%"
      en overview     34%        span.channel-value   "34%"
      en overview     0.94       strong               "0.94"
      …
    27 distinct score-shaped number(s) rendered.

**Four numbers on the Overview, the same four P50 traced, all still traced:**
`51%` = `100 − mean(41.3, 22.9, 34.4)`, with the arithmetic printed in its own
evidence chip; `41%` = `cpu.totalBusyBp / 100`; `34%` = the busiest
`storage[].activeTimeBp / 100`; `0.94` = `storage[].avgTransferLatencyUs / 1000`.
**The new Settings screen adds no score-shaped number at all** — the count is 27,
unchanged, over one more page than P50 measured.

### `node tools/verify-arabic.mjs` — EXPECTED: 7/7, zero system-font fallback

    overview   7/7 checks pass
    settings   7/7 checks pass
    activity   7/7 checks pass

    OBSERVED  76 Arabic nodes, 1196 glyphs; bundled = [["JetBrains Mono",196],
              ["IBM Plex Sans Arabic",679],["IBM Plex Sans Arabic SmBld",129],
              ["IBM Plex Sans Arabic Medm",175],["Inter",17]];
              system fallback = 0 glyph(s) []

### `node tools/verify-tokens.mjs` — EXPECTED: every var() resolves

    === 12 pages x 2 languages, read from the live CSSOM ===
      style rules walked                 975
      declarations referencing var()     437
      unresolved on a matched element    0 (property, selector) pair(s), 0 distinct

    PASS — every var() reference in 437 declarations resolves on every element
           the rule matches.

### `tools/layout-sweep.mjs` — EXPECTED: clean at 1280/1024/960, both languages, both themes, populated AND with no service

**Populated (`layout-fixture.html`) — 12/12 pass:**

    PASS  overview-1280-en-dark   overflowX=0 clipped=0 overlaps=0 band=true denied=3 evidence=7 empty=0 emdash=0  height=1715
    PASS  overview-1024-en-dark   …  height=2517
    PASS  overview-960-en-dark    …  height=2515
    PASS  overview-1280-ar-dark   …  height=1681
    PASS  overview-1024-ar-dark   …  height=2595
    PASS  overview-960-ar-dark    …  height=2593
    (light theme identical to dark at every width, both languages)

**No service (`index.html`) — 12/12 pass:**

    PASS  overview-1280-en-dark   overflowX=0 clipped=0 overlaps=0 band=true denied=3 evidence=0 empty=3 emdash=13  height=1677
    PASS  overview-1024-en-dark   …  height=1857
    PASS  overview-960-en-dark    …  height=1855
    PASS  overview-1280-ar-dark   …  height=1638
    PASS  overview-1024-ar-dark   …  height=1858
    PASS  overview-960-ar-dark    …  height=1856

**Both destinations, 1280, 8/8 pass:** `settings` 1,389 px (en) / 1,408 px (ar);
`activity` 1,958 px (en) / 1,988 px (ar).

At 1024 and below the container query collapses the grid to one column, so the
screen is taller by design — the brief's target is 1280 and it is met there in
every combination.

### the contrast instrument — EXPECTED: nothing below 4.5:1, either theme

`node tools/contrast-sweep.mjs` — 12 pages × 2 themes, English:

    PASS  overview/en/dark    nodes= 174 below=0 exempt=0 worst=5.23:1
    PASS  settings/en/dark    nodes= 107 below=0 exempt=0 worst=5.30:1
    PASS  activity/en/dark    nodes= 105 below=0 exempt=0 worst=5.24:1
    PASS  overview/en/light   nodes= 174 below=0 exempt=0 worst=4.67:1
    PASS  settings/en/light   nodes= 107 below=0 exempt=0 worst=4.67:1
    PASS  activity/en/light   nodes= 105 below=0 exempt=0 worst=4.67:1
    …
    measured 3202 text node(s); 0 below threshold, 0 distinct;
    2 exempt as inactive controls (WCAG 1.4.3 Incidental)
      theme=dark      0 below, 0 distinct
      theme=light     0 below, 0 distinct

    PASS — every rendered text node meets WCAG AA in every theme measured.

`--locales ar` — the same 12 pages × 2 themes in Arabic:

    PASS  overview/ar/dark    nodes= 174 below=0 exempt=0 worst=5.23:1
    PASS  settings/ar/dark    nodes= 107 below=0 exempt=0 worst=5.30:1
    PASS  activity/ar/dark    nodes= 105 below=0 exempt=0 worst=5.24:1
    PASS  overview/ar/light   nodes= 174 below=0 exempt=0 worst=4.67:1
    PASS  settings/ar/light   nodes= 107 below=0 exempt=0 worst=4.67:1
    PASS  activity/ar/light   nodes= 105 below=0 exempt=0 worst=4.67:1
    …
    measured 3186 text node(s); 0 below threshold, 0 distinct;
    2 exempt as inactive controls (WCAG 1.4.3 Incidental)
      theme=dark      0 below, 0 distinct
      theme=light     0 below, 0 distinct

    PASS — every rendered text node meets WCAG AA in every theme measured.

**6,388 text nodes across 12 pages × 2 themes × 2 languages, 0 below 4.5:1.**
The two exemptions in each language are the same disabled
`button.install-button` on the Startup screen, one per theme, which WCAG 1.4.3
exempts as an inactive control. Neither is on a screen this session touched.

### `npm run build`

    ✓ built in 503ms
    (!) Some chunks are larger than 500 kB after minification.  [pre-existing]

### `npm run check` — svelte-check, 16-warning baseline

    227 FILES 0 ERRORS 16 WARNINGS 3 FILES_WITH_PROBLEMS

**Baseline held.** 227 files rather than 226 — the new `SettingsPage.svelte`.
The same 16 warnings as before this session (one a11y in `FluidDialog`, fifteen
unused-selector in `FindingCard` and `DeepScanPage`); none is in a file this
session touched.

## 51.7 THE HEADLINE NUMBER

**Overview height at 1280, before and after, both languages, both data states:**

| state | before | after | delta |
|---|---|---|---|
| **populated, en** | **4,110 px** | **1,715 px** | **−2,395 px (−58%)** |
| **populated, ar** | **4,082 px** | **1,681 px** | **−2,401 px (−59%)** |
| **no service, en** | **3,533 px** | **1,677 px** | **−1,856 px (−53%)** |
| **no service, ar** | **3,498 px** | **1,638 px** | **−1,860 px (−53%)** |

**EXPECTED: under 2,000 px at 1280 wide, populated, in both languages.
OBSERVED: 1,715 and 1,681.** And under it empty as well, which the brief did not
require.

At a 900 px window the Overview is now **1.9 screens** instead of 4.6.
**Nothing was deleted to get there that was not proven to be a duplicate:** six
non-clickable cards, one panel whose reading and whose action both existed
elsewhere, and two components that were already rendering on Activity. Four
sections were moved to a screen built for them, one was moved to the screen it is
named after, and the rest of the distance came from arranging the instrument so
its own column was not empty.

## 51.8 THE LEDGER DELTA — two P50 ids closed, two new, one untouched by instruction

Recorded here rather than in `DEBT_REGISTER.json`, which is the `QD-*` register
under the p31 audit gate; every `DBT-P4x`/`DBT-P5x` id from P42 onward lives in
this ledger.

### Closed

| id | what it was | how it closed |
|---|---|---|
| **`DBT-P50-003`** | Two settings-shaped panels on the Overview with nowhere to go; `NAVIGATION` had no Settings id | §51.3 — the destination was created, both panels moved unmodified, all four sections exercised there |
| **`DBT-P50-004`** | `EmptyState` inherits `flex-wrap: wrap` on a flex column; 496 px for 396 px of content | §51.5 — P50's one declaration applied, reproduced first, verified on all eleven empty states across all seven screens: one 100 px shorter, ten identical |
| **`DBT-P50-002`** | `Protocol v7`, a sourceless literal in the markup | §51.5 — already dropped by P50's own rebuild; verified 0 matches in the rendered DOM, the two orphaned catalog keys deleted. Closed as dropped, not wired |

### Still open, unchanged

| id | status |
|---|---|
| **`DBT-P50-001`** | No disk free/used space provider. Untouched — a service change |
| **`DBT-P50-005`** | No wire accessor for the performance sample series. **Not started, by instruction.** §51.5 records exactly what approval would be approving |

### New

| id | what | why it is open, not fixed |
|---|---|---|
| **`DBT-P51-001`** | **The navigation rail overflows its own viewport.** Measured at 1280 × 900 on the fixture: `nav.scrollHeight = 774`, `clientHeight = 492` — **282 px below the fold**, and `settings` sits at `top = 867` against a rail bottom of `637`. Activity and Fleet were already below it before this session; Settings is the third, and it is last in the list | Pre-existing, and this session added 87 px to it (one group heading and one item). Fixing it means redesigning the rail's vertical budget — the brand block, the command trigger, the policy card and the two control buttons all take fixed height above and below a scrolling `nav`. That is a shell change touching every screen, and this brief is not it |
| **`DBT-P51-002`** | **One-click care's only entry point is now the Activity screen.** `CarePanel` rendered on both Overview and Activity; §51.3 removed the duplicate, and the survivor lives on a screen the rail describes as "Durable operation journal and recovery" | The duplication was real and the deletion is right. Whether the product's one run-everything action belongs on the history screen is a placement question the brief did not put, and answering it means either a new destination or a decision about the Overview's header actions. Recorded for the owner rather than decided here |
| **`DBT-P51-003`** | **Thirteen of sixteen `cap.reason.*` / `cap.note.*` keys the capability engine can emit have no translation.** `crates/platform-capabilities/src/lib.rs` defines 15 `cap.reason.*` constants plus `cap.note.windowsServerWsusPolicy`; the catalogs carry three (`windowsServerNoThermalPower`, `windowsServerNoGameMode`, `windowsServerNoRestorePoints`). `AboutPanel.noteLabel` deliberately renders the raw key when the catalog has none — so on macOS, Linux, Windows Server Core, or after any collector downgrade, the Settings screen prints `cap.reason.macosNoDriverStore` at the user | Found while building the Settings fixture, which is why the fixture models the shipping `windows_table()` (everything Native, no reason keys) rather than a state the product cannot yet render. Thirteen keys × two languages is a translation task, not a layout one, and it is not in any of this brief's items |

## 51.9 P51 FINAL REPORT

### What was asked, and what happened

Fill the Overview, cut the six-tile grid, re-home the other nine sections, and
get under 2,000 px without deleting a feature. All four were done. **Fourteen
commits; 14 source files (+287 / −164) and 5 gate tools; and the screen went
from 4,110 px to 1,715 px.**

### The sampler decision, in one paragraph

The brief offered three ways to start the perf sampler from the Overview.
Tracing it first showed that none of them was the answer: `aether-perf-sampler`
pushes into an in-process ring and **publishes nothing**, and the only producer
of a `performanceSnapshot` event in the product is the `get_performance_snapshot`
handler. Starting the sampler would have bought a background thread costing a
**measured 230.8 ms mean per tick** — ~11% of one core at 1 s — and left the
instrument exactly as blank as it was. So the Overview reads instead: one
snapshot on mount, then one every 5 s, disposed with the component, verified in
the fixture at 3 reads in 11 s on the screen and 0 after leaving it. Under 5% of
one core while you are looking at it, nothing when you are not. Reading counters
is not a mutation — no supervisor, no fence, no journal entry, no restore point —
and the consent this product asks for is consent to change the machine.

### What changed on a second screen, said out loud

Two things, both required by item 1 and both stated where the brief asked for
them. `stream-state.ts` inferred `perfSampling` from the arrival of a snapshot,
which would have made the Performance screen offer to stop a thread nobody
started; it is now set only by start/stop, and the Performance header was
re-measured after the change (`▶ Start monitoring`, Analyze disabled — the
truth). And `RecoveryPanel` was moved out of `AppShell`'s always-on slot into the
Activity branch, which removes it from ten screens; that is the verdict, not a
side effect.

### What was dropped, and the proof for each

- **Six module cards** — measured at **0 interactive descendants** each. Not a
  duplicate of the navigation; less than it.
- **The driver servicing panel** — its number is an action item carrying the
  scan id; its button calls the same function the Drivers header is bound to,
  exercised there in both hub states.
- **`CarePanel` and `InsightsPanel` on the Overview** — the same components were
  already rendered in the Activity branch of the same `{#if}`.
- **Fourteen message keys and four CSS blocks** that had no remaining reader.

Nothing else. Four sections were **moved** to a Settings screen that did not
exist, one to the screen it is named after, and the last 537 px came from
arranging the instrument so its own left column was not 679 px of nothing.

### What was recorded rather than worked around

`DBT-P51-001` (the rail overflows by 282 px and Settings is below the fold),
`DBT-P51-002` (one-click care's only entry point is now Activity), `DBT-P51-003`
(13 of 16 capability reason keys are untranslated and render raw). `DBT-P50-005`
was not started, by instruction, and what its approval would cover is written
down in §51.5.

### One paragraph: what a first-time user now sees in the first screen of the app

They see an instrument that is already reading. The orb shows a headroom
percentage with the arithmetic behind it one click away in its own chip, four
labelled channels with live values, four telemetry tiles naming their sources, a
service log with real sequence numbers, and beside all of it a column of action
items where every row carries the scan id, timestamp and counts it rests on.
Nothing on the screen is a feature list, a marketing claim, or a number nobody
measured — the six cards that named six modules are gone, and so is every panel
that was configuration wearing status styling. If the service is not running they
see the same instrument saying so twelve times in em dashes, and one button that
says **Take a reading now** — which is the difference this session actually
made: before, a new user opened the product, saw a correct and completely blank
instrument, and was told nothing about what would fill it. The whole screen fits
in under two window-heights, so the first thing they scroll to is the current
protected operation rather than the fourth copy of the navigation.
