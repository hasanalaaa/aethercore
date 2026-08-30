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
| A1 | Recorded canonical build script | NOT-STARTED | |
| A2 | Rebuild artifacts hashed + sized | NOT-STARTED | |
| A3 | wix build + validate, zero ICE | NOT-STARTED | |
| A4 | Reinstall-vs-upgrade decision justified | NOT-STARTED | |
| A5 | Install from realigned MSI verified | NOT-STARTED | |
| GATE A | Package == installed files, security intact, 8 verb runs | NOT-STARTED | |
| B1 | Repair preserves everything | NOT-STARTED | |
| B2 | Uninstall: what goes, what survives | NOT-STARTED | |
| B3 | Clean reinstall on empty box | NOT-STARTED | |
| B4 | Upgrade v1 -> v2 | NOT-STARTED | |
| C1 | Install interrupted mid-transaction | NOT-STARTED | |
| C2 | Service fails to start during install | NOT-STARTED | |
| C3 | Required file missing/corrupt at start | NOT-STARTED | |
| C4 | Rollback via failing custom action | NOT-STARTED | |
| D1 | `aetherctl update stage` real path | NOT-STARTED | |
| D2 | stage -> apply -> rollback | NOT-STARTED | |
| D3 | update trust disabled by default, no network | NOT-STARTED | |
| E | Seal, snapshot P36-VM-QUALIFIED | NOT-STARTED | |

## 8. PACE LOG

(one line per ~10 shell commands, naming the current gate)

- cmd ~6 — Stage 0, writing session brain.
