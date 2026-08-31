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
