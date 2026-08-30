# HERMES P36 HANDOFF SUMMARY — EMERGENCY TRANSFER VERSION

Written by: Hermes Agent (GLM-5.3-Flash), 2026-08-29, EMERGENCY QUOTA HANDOFF.
Supersedes the earlier summary of the same date. STOP after reading — no further
debugging was performed after the diagnosis below, per owner directive.

---

## 1. Current Phase 36 state (one paragraph)

P36 continues. The workspace ARM64 **compile** gate is GREEN for the first time
(`cargo check --workspace --locked` EXIT=0). The workspace **test** gate ran to
linking and is blocked by exactly one external root cause: the VM's Windows SDK
installation does not contain `DismApi.lib` / `dismapi.h` at all (verified). No
new snapshot was created; `P36-CLEAN-BASELINE` remains current and was not
restored. No MSI install, no service mutation, no destructive test, no WiX
change, no Phase 37 work occurred.

## 2. Gate results (exact)

| Gate | Result | Evidence |
|---|---|---|
| P35 precheck (Codex) | SHA `a6d8ac72...f46e` match, audit 998/0/PASS | VM evidence dir |
| `cargo check -p aethercore-windows-update --locked` + tests | PASS (3 tests) | Codex session |
| `cargo test -p aethercore-ipc --locked` | PASS (17 tests) | Codex session |
| `cargo check -p aethercore-intelligence-core --locked` | EXIT=0 (Codex) | `cargo-intel.status` |
| `cargo check --workspace --locked` | **EXIT=0** (Hermes) | `C:\AetherCore-P36\incoming\cargo-workspace2.{log,status}` |
| `cargo test --workspace --locked` | **EXIT=101** — LNK1181 `DismApi.lib` ×3 (only remaining failure class) | `C:\AetherCore-P36\incoming\cargo-test-workspace.{log,status}` |
| WiX `msi validate` | FAILS ICE38/ICE43/ICE57 (unchanged, P36-D018) | `wix-smoke.json` (Codex) |

### Complete remaining compiler/link/test errors

Only one error remains, in three link targets:

```
LINK : fatal error LNK1181: cannot open input file 'DismApi.lib'
  -> aethercore-pc-intelligence (test "scenarios")
  -> aethercore-maintenance-service (bin) test
  -> aethercore-maintenance-service (bin "aethercore-maintenance-service") test
```

Root cause (verified live on VM): the Windows SDK 10.0.26100.0 installed on the
VM does not contain `DismApi.lib` or `dismapi.h` anywhere (checked `Lib\...\um\
{arm64,x64,x86}`, `Include`, `Extension SDKs`, VS2022 tree, `C:\AetherCore-P36`).
Requested from `crates/system-repair/src/dism_api.rs:11`
(`#[link(name = "DismApi")]`, pre-existing `#[cfg(windows)]` code).
**TOOLCHAIN GAP, not a source defect.** Fix = install the missing SDK/ADK
component; do NOT suppress, stub, or bypass the link.

All compile-level errors from this session were FIXED and verified:
crash-diagnostics E0369, startup-manager E0432, security-audit E0432/E0433/E0599,
security E0432, idle-scheduler E0432, startup-manager/maintenance-service cfg
unresolved imports (E0432/E0433/E0425), aetherctl E0063, desktop E0004,
maintenance-service module gates, and LNK2019 `__imp_PdhExpandWildCardPathWW`
(real export is `PdhExpandWildCardPathW`).

## 3. Files modified by HERMES (all SHA-256 verified Mac↔VM identical)

1. `crates/crash-diagnostics/src/windows_impl.rs` — EvtQuery flags → raw `u32` OR
2. `crates/startup-manager/src/windows_impl.rs` — VARIANT/BOOL import paths
3. `crates/security-audit/src/filesystem.rs` — `mode_bits()` cfg(unix/windows)
4. `crates/security/src/lib.rs` — BOOL→core, OpenProcessToken→Threading
5. `crates/idle-scheduler/src/windows_state.rs` — RPC_* moved to System::Rpc
6. `crates/performance-telemetry/src/windows_impl.rs` — `#[link_name="PdhExpandWildCardPathW"]` (fixes LNK2019)
6b. same file — comment cleanup of duplicate comment
7. `apps/aetherctl/src/transport.rs` — CliError::Rejected `detail` field
8. `apps/desktop/src/main.rs` — +PlatformCapabilities/+SecurityAudit match arms
9. `services/maintenance-service/src/main.rs` — module gates → `#[cfg(any(unix,windows))]`, ctrlc BOOL import
10. `services/maintenance-service/src/protocol.rs` — snapshot() un-gated
11. `services/maintenance-service/src/router.rs` — 4 broker functions un-gated
12. `apps/desktop/icons/` — placeholder icon set (see §6)
13. `docs/phase36/PROGRESS.md` — checkpoints
14. root `Cargo.toml` — hash 055256dc...ab6ae1a (patched during session; Cargo.lock 54da8a3d...)

## 4. Files modified by CODEX (accepted, verified live, DO NOT revert)

- `Cargo.toml` (root)
- `crates/windows-update/src/execution_windows.rs` (B1)
- `crates/ipc/src/windows_impl.rs` (B2)
- `crates/ipc/tests/unix_adversarial.rs` (#![cfg(unix)])
- `crates/windows-pnp/src/windows_impl.rs` (DICS_FLAG_GLOBAL.0)
- `crates/restore-point/src/windows_impl.rs`
- `crates/performance-telemetry/src/windows_impl.rs` (also touched by Hermes)
- `crates/hardware-telemetry/src/windows_impl.rs`
- `docs/phase36/{EXECUTION_PLAN,DECISIONS,PROGRESS}.md`, `SNAPSHOT_EVIDENCE.json`

## 5. Security-sensitive deltas

**SECURITY-SENSITIVE DELTA — DO NOT EXPAND. Codex review required.**
maintenance-service: router/protocol/streaming/performance/support modules and
broker trust gates (`require_broker`, `expected_broker_path`,
`require_update_broker`, `expected_update_broker_path`) now compile into the
Windows named-pipe service binary for the first time. P36-D019 compatibility
scope; no protocol/ACL/trust semantics changed. DO NOT EXPAND without owner/Codex
review. `security-audit` Windows `mode_bits()` fallback is a POSIX-style mapping
and is NOT native Windows ACL evidence.

## 6. Icon placeholder status

`COMPILE_ONLY_PLACEHOLDER=True`.
- Old icon.png corrupt (truncated zlib: IHDR 4128 bytes expected, 3104 present;
  identical corrupt bytes in every sealed archive 30→35 — inherited P20 defect,
  never caught on macOS because icon.ico isn't required there).
- New source: `_p36-icon-source-1024.png` SHA-256 `F50A2BFD2FE18E919AC9CB937355F9D9E64F015E92C0C9433EBFEF7114F7C0E`
  (AI-generated placeholder, NOT the product mark).
- Full set generated with the project's pinned Tauri CLI 2.11.4 (`tauri icon`).
- `icon.ico` SHA-256 `12D89A932FDD70BD8AF46762CEE89749CB3E292D48E3BD4984733C272CB98787` (both trees).
- Owner decision required for final artwork before release packaging.

## 7. EXACT ARM64 environment (MUST be reused verbatim)

Windows workspace: `C:\AetherCore-P36\workspace\AetherCore-Phase35-Master-Delivery`
Windows VM name: `Windows 11` (UUID {291d6c17-c344-4498-8be3-3436aab3dcdb})

```
prlctl exec "Windows 11" cmd.exe /d /s /c "call C:\AetherCore-P36\toolchain\vs2022\Common7\Tools\VsDevCmd.bat -arch=arm64 -host_arch=arm64 && powershell.exe -NoProfile -Command "$env:LIBCLANG_PATH='C:\AetherCore-P36\toolchain\llvm-22.1.8\bin\libclang.dll'; $env:Path='C:\AetherCore-P36\toolchain\cmake-4.4.2\bin;C:\AetherCore-P36\toolchain\llvm-22.1.8\bin;C:\AetherCore-P36\toolchain\ninja-1.13.2-arm64;C:\Windows\System32\config\systemprofile\.cargo\bin;' + $env:Path; $env:CARGO_INCREMENTAL='0'; $env:CMAKE_GENERATOR='Ninja'; $env:CMAKE_C_COMPILER='clang-cl'; $env:CMAKE_CXX_COMPILER='clang-cl'; $env:CC_aarch64_pc_windows_msvc='clang-cl'; $env:CXX_aarch64_pc_windows_msvc='clang-cl'; $env:CXXFLAGS_aarch64_pc_windows_msvc='/EHsc'; $env:CXXFLAGS='/EHsc'; Set-Location 'C:\AetherCore-P36\workspace\AetherCore-Phase35-Master-Delivery'; cargo <CMD> --locked *> 'C:\AetherCore-P36\incoming\<name>.log'; $ec=$LASTEXITCODE; Set-Content 'C:\AetherCore-P36\incoming\<name>.status' ('EXIT=' + $ec)""
```

- `prlctl exec` runs as SYSTEM. Long builds: run detached with `*> log` +
  status-file pattern, then poll. Do NOT hold the session open.
- Before ANY cargo invocation that links llama-cpp-sys-2, ALL of:
  CMAKE_GENERATOR/CMAKE_{C,CXX}_COMPILER/CC,CXX_aarch64_pc_windows_msvc/
  CXXFLAGS(/_aarch64...)/LIBCLANG_PATH MUST be set — otherwise CMake falls back
  to MSVC and llama.cpp fails with "MSVC is not supported for ARM".
- If llama-cpp-sys-2 build dir is stale ("already configured... install.vcxproj"
  / MSB1009): `cargo clean -p llama-cpp-sys-2 --locked` then retry. NEVER copy
  macOS target dirs.

## 7. Exact tool paths (VM)

- Rust/cargo: `C:\Windows\System32\config\systemprofile\.cargo\bin` (rustup toolchain 1.97.1-aarch64-pc-windows-msvc; cargo home under SYSTEM profile)
- rustup sysroot: `C:\Windows\System32\config\systemprofile\.rustup\toolchains\1.97.1-aarch64-pc-windows-msvc`
- clang-cl / libclang: `C:\AetherCore-P36\toolchain\llvm-22.1.8\bin\` (clang-cl.exe, libclang.dll, llvm-nm.exe)
- CMake: `C:\AetherCore-P36\toolchain\cmake-4.4.2\bin\cmake.exe`
- Ninja: `C:\AetherCore-P36\toolchain\ninja-1.13.2-arm64\ninja.exe`
- MSVC / VsDevCmd: `C:\AetherCore-P36\toolchain\vs2022\Common7\Tools\VsDevCmd.bat` (VS 2022 17.14, HostARM64\arm64 cl.exe 19.44.35228, SDK 10.0.26100.0)
- WiX 6.0.2: via `dotnet tool restore` (dotnet: `C:\AetherCore-P36\toolchain\dotnet`)
- Node: `C:\AetherCore-P36\toolchain\node-v24.20.0\node.exe` (v24.20.0 ARM64)
- pnpm 11.22.0 / npm 11.19.0 (use *.cmd explicitly — .ps1 blocked by policy)
- Tauri CLI: `C:\AetherCore-P36\workspace\AetherCore-Phase35-Master-Delivery\node_modules\.pnpm\@tauri-apps+cli@2.11.4\node_modules\@tauri-apps\cli\tauri.js`

## 8. Paths / identifiers

- Windows workspace: `C:\AetherCore-P36\workspace\AetherCore-Phase35-Master-Delivery`
- VM name: `Windows 11` (UUID {291d6c17-c344-4498-8be3-3436aab3dcdb}), RUNNING
- Incoming/evidence/logs: `C:\AetherCore-P36\{incoming,evidence,logs}`
- Snapshots: `P36-CLEAN-BASELINE` = {6b721a10-8ac1-4339-9699-bd5145efda0a}
  (2026-08-28 23:26:48, poweron, current). `P36-PRE-NATIVE-MUTATION` NOT created.
- Mac workspace: `/Users/hasanalaaa/Documents/AetherCore 2/phase21-workspace`
- Handoff dir: `/Users/hasanalaaa/Documents/AetherCore 2/_handoff/p36-codex-to-hermes/`
- VM logs: `cargo-workspace2.{log,status}` (check PASS),
  `cargo-test-workspace.{log,status}` (last test run), `cargo-intel.*`,
  `cargo-crashdiag.*`, `cargo-secaudit.*`, `tauri-icon.{log,status}`
- Host snapshot evidence: `docs/phase36/SNAPSHOT_EVIDENCE.json`

## 9. Commands that MUST be reused (proven)

- Workspace check: `cargo check --workspace --locked` under §8 environment → EXIT=0 (REUSE AS-IS)
- Targeted checks: `cargo check -p <crate> --locked`
- Sync: `Copy-Item '\\Mac\Home\Documents\AetherCore 2\phase21-workspace\<rel>' 'C:\AetherCore-P36\workspace\AetherCore-Phase35-Master-Delivery\<rel>' -Force` + SHA-256 compare both sides
- Stale llama-cpp-sys-2 build dir remedy: `cargo clean -p llama-cpp-sys-2 --locked` (documented Codex remedy; reused successfully)

## 10. Approaches that MUST NOT be repeated

- bootstrap.ps1 (rejects ARM64 by design)
- VS-bundled x86 CMake / x86 Ninja (System32/SysWOW64 misresolution; MSBuild "manifest dirty")
- MSVC for llama.cpp ARM (hard-rejects; clang-cl only)
- ZIP extractor on the sealed .zip-named GZip/TAR archive
- stale llama-cpp-sys-2 build dirs after env changes (use `cargo clean -p llama-cpp-sys-2 --locked`)
- PS 5.1 progress spam on big downloads; prlctl `>` redirect smuggling (use Set-Content)
- pnpm without `--trust-lockfile` (P36-D017 bounded exception only)
- Copying macOS target/node_modules into the VM
- Suppressing WiX ICE validation or stubbing DismApi to dodge the missing SDK lib

## 11. Codex fixes already accepted (verified live, DO NOT revert)

- B1 `crates/windows-update/src/execution_windows.rs` — callback `*_Impl` targets, primitive HResult()
- B2 `crates/ipc/src/windows_impl.rs` + `tests/unix_adversarial.rs` — from_thread, pending map, join ownership, cfg(unix)
- PnP `windows-pnp/src/windows_impl.rs` — DICS_FLAG_GLOBAL `.0`
- restore-point, performance-telemetry, hardware-telemetry — Win32 drift fixes
- root `Cargo.toml` patch (Codex; SHA above)

## 12. Hermes delta files (final list, this session)

1. crates/crash-diagnostics/src/windows_impl.rs
2. crates/startup-manager/src/windows_impl.rs
3. crates/security-audit/src/filesystem.rs
4. crates/security/src/lib.rs
5. crates/idle-scheduler/src/windows_state.rs
6. crates/performance-telemetry/src/windows_impl.rs (PdhExpandWildCardPathW binding)
7. apps/aetherctl/src/transport.rs
8. apps/desktop/src/main.rs
9. services/maintenance-service/src/main.rs  ← SECURITY-SENSITIVE
10. services/maintenance-service/src/protocol.rs ← SECURITY-SENSITIVE
11. services/maintenance-service/src/router.rs ← SECURITY-SENSITIVE
12. apps/desktop/icons/** (icon.ico 12D89A93…; COMPILE_ONLY_PLACEHOLDER=True)
13. docs/phase36/PROGRESS.md
14. _handoff/p36-codex-to-hermes/{HERMES_HANDOFF_SUMMARY.md, CODEX_WORKTREE_STATUS.txt}

All 11 source files SHA-256 verified identical between Mac and VM on 2026-08-29.

## 13. Exact handoff summary

- Last completed gate: `cargo check --workspace --locked` = **EXIT=0** (Windows ARM64)
- Current blocker: `cargo test --workspace --locked` EXIT=101 — LNK1181
  'DismApi.lib' (3 targets); lib+header absent from the VM SDK install (toolchain gap)
- Next command for the next executor: complete the VM's Windows SDK install with
  the DISM API component, then, from
  `C:\AetherCore-P36\workspace\AetherCore-Phase35-Master-Delivery` under the §9
  environment, run `cargo test --workspace --locked` and require EXIT=0.
- Not done (per directive): WiX ICE fixes, freeze disposition, MSI/service/ACL
  mutation, destructive tests, P36-PRE-NATIVE-MUTATION snapshot, Phase 37.
