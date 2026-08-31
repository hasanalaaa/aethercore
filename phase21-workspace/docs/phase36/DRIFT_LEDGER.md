# Phase 36 — Post-Seal Drift Ledger

Closes the P36-D001 obligation: every file in the live working tree that differs
from the sealed Phase 35 full-tree ledger is classified here with its evidence.

## Method

```
python3 PHASE_35_BINARY_SAFE_PATCH/verify_phase35.py .
python3 PHASE_35_BINARY_SAFE_PATCH/verify_phase35.py . --full-tree
```

Scoped (patch mode): `checked 53 / expected 53`, 1 problem — `hash mismatch: Cargo.toml`.
Full-tree: `checked 1179 / expected 1114`, `problem_count 92`, status FAIL.

The verifier truncates its `problems` array to the first 50 entries
(`verify_phase35.py`, full-tree branch: `"problems": problems[:50]`), so the
complete set was recomputed directly against
`PHASE_35_BINARY_SAFE_PATCH/PHASE_35_EXPECTED_FULL_SHA256.json`:

| Class | Count |
|---|---|
| Added (in tree, not in ledger) | 65 |
| Removed (in ledger, not in tree) | 0 |
| Modified (in both, hash differs) | 26 |
| **Distinct drifting files** | **91** |

`problem_count 92` = these 91 plus the patch-manifest pass re-reporting
`hash mismatch: Cargo.toml`.

## Totals

| Bucket | Files |
|---|---|
| AUTHORIZED | 81 |
| DIAGNOSTIC | 10 |
| **UNKNOWN** | **0** |

---

## AUTHORIZED (81)

### A1 — Build enablement consequent to P36-D019 (1 file)

| File | Evidence |
|---|---|
| `Cargo.toml` | Adds two `windows` 0.62.2 crate features. `Win32_System_Rpc` is directly required by the D019-authorized import moves of `RPC_C_AUTHN_WINNT`/`RPC_C_AUTHZ_NONE` from `System::Com` to `System::Rpc` (`crates/idle-scheduler`, `crates/hardware-telemetry`, `crates/restore-point`). `Win32_System_Com_StructuredStorage` is a transitive requirement of the **pre-existing** `Win32_System_Ole` feature: `windows-0.62.2/src/Windows/Win32/System/Ole/mod.rs` names `super::Com::StructuredStorage::{OLESTREAM, IStorage}` in 33 signatures. No dependency added, no version changed. |

### A2 — P36-D019 windows-rs 0.62.2 API-drift corrections (16 files)

Items 1–10 are the itemised "HERMES DELTA (2026-08-29)" inventory in
`docs/phase36/PROGRESS.md`; the rest are the same class, verified by diff
against tag `p35`.

| File | Evidence |
|---|---|
| `crates/crash-diagnostics/src/windows_impl.rs` | PROGRESS item 1 — `EVT_QUERY_FLAGS` lost `BitOr`; `EvtQueryChannelPath.0 \| EvtQueryReverseDirection.0` |
| `crates/startup-manager/src/windows_impl.rs` | PROGRESS item 2 — `VARIANT`/`BOOL` module moves, import-path only |
| `crates/security-audit/src/filesystem.rs` | PROGRESS item 3 — `#[cfg(windows)]` fallback for `mode_bits()` |
| `crates/security/src/lib.rs` | PROGRESS item 4 — `BOOL` and `OpenProcessToken` module moves |
| `crates/idle-scheduler/src/windows_state.rs` | PROGRESS item 5 — `RPC_C_*` moved `System::Com` → `System::Rpc` |
| `apps/aetherctl/src/transport.rs` | PROGRESS item 6 — `CliError::Rejected` `detail` field completion (E0063) |
| `apps/desktop/src/main.rs` | PROGRESS item 7 — exhaustive match gains `PlatformCapabilities`/`SecurityAudit` |
| `services/maintenance-service/src/main.rs` | PROGRESS item 8 — module gates `#[cfg(any(unix, windows))]`. **SECURITY-SENSITIVE** |
| `services/maintenance-service/src/protocol.rs` | PROGRESS item 9 — `snapshot()` un-gated. **SECURITY-SENSITIVE** |
| `services/maintenance-service/src/router.rs` | PROGRESS item 10 — broker trust gates un-gated. **SECURITY-SENSITIVE** |
| `crates/hardware-telemetry/src/windows_impl.rs` | `Rpc`/`SystemInformation` module moves; `prop_u8` drops the removed `u8: TryFrom<&VARIANT>` impl and keeps the `u16`→`u8` narrowing path |
| `crates/performance-telemetry/src/windows_impl.rs` | `unsafe extern "system"` (2024 edition); `#[link_name = "PdhExpandWildCardPathW"]` fixes an ARM64 LNK2019 against a non-existent `…WW` export; `POWER_INFORMATION_LEVEL(11)` newtype; `PCWSTR::null()` |
| `crates/restore-point/src/windows_impl.rs` | `BOOL`, `FreeLibrary`, `RPC_C_*` module moves, import-path only |
| `crates/windows-pnp/src/windows_impl.rs` | `DICS_FLAG_GLOBAL` → `.0` (newtype to primitive) |
| `crates/windows-update/src/execution_windows.rs` | D019 verbatim: "the four generated callback `*_Impl` targets" and "primitive HRESULT handling" |
| `crates/fleet/src/trust.rs` | `ssh_binary()` also probes `ssh.exe` under `#[cfg(windows)]`; the extension-less-only check made detection always `None` on Windows. Trust semantics unchanged (D019: "preserve existing error, trust, framing, and mutation semantics") |

### A3 — P36-D019 Unix test platform gate (1 file)

| File | Evidence |
|---|---|
| `crates/ipc/tests/unix_adversarial.rs` | `#![cfg(unix)]` added. D019 verbatim: "Unix test platform gate" |

### A4 — The three tranche-1 security fixes (3 files)

| File | Evidence |
|---|---|
| `installer/wix/Product.wxs` | Launch condition `WindowsBuild >= 22621` → `OSCURRENTBUILD >= 22621` via `RegistrySearch` on `HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\CurrentBuildNumber`. MSI compat-caps `WindowsBuild` at 9600 for unmanifested packages, so the authored guard could never pass (observed: `WindowsBuild = 9600` on build 26200). Guard intent unchanged. Also carries the P36-D018 ICE38/ICE43/ICE57 component-authoring remedy |
| `apps/install-hardener/src/main.rs` | `run_icacls`: `/T` removed from the `/inheritance:r` + `(OI)(CI)` grant pass. With `/T`, files receive a protected empty DACL (`D:PAI`) because inheritance flags are invalid on files, leaving executables unreadable even to SYSTEM (service start failure, MSI 1920). Declared ACL policy unchanged |
| `crates/ipc/src/windows_impl.rs` | Pipe SDDL: `(A;;0x00120003;;;AU)` → `(A;;FR;;;AU)(A;;0x00000002;;;AU)`. Windows' SDDL parser silently drops `SYNCHRONIZE` from hex access masks, so the materialized DACL denied every synchronous client, including the desktop app. Named rights materialize as `0x12008B`, still withholding `FILE_CREATE_PIPE_INSTANCE (0x4)` and `FILE_APPEND_DATA`. Encoding-only change. Also carries two D019 items: `Error::from_thread()` and the explicit pending-map type |

### A5 — Icon set (54 files)

`docs/phase36/PROGRESS.md` item 11 and the "Icon placeholder status" section.
The sealed P35 `apps/desktop/icons/icon.png` is a truncated zlib stream (IHDR
promises 4128 scanline bytes, stream yields 3104) — an inherited P20-era defect
present identically in every sealed archive 30–35, never caught on macOS where
`icon.ico` is not required. Windows `tauri-build` hard-requires `icon.ico`.

53 added + `apps/desktop/icons/icon.png` modified. Generated by the project's own
pinned Tauri CLI 2.11.4 from the sealed lockfile (`tauri icon`) over a new
1024×1024 placeholder.

Historical placeholder marker; superseded by the reviewed evidence-shield
master and regenerated cross-platform set in design-elevation commit `9f07df5`.

### A6 — P36 control and evidence records (6 files)

| File | Evidence |
|---|---|
| `docs/phase36/DECISIONS.md`, `EXECUTION_PLAN.md`, `PROGRESS.md` | PROGRESS.md COMPLETED: "Phase 36 control files created: `EXECUTION_PLAN.md`, `DECISIONS.md`, and this checkpoint" |
| `docs/phase36/SNAPSHOT_EVIDENCE.json`, `PRE_NATIVE_MUTATION_EVIDENCE.json`, `NATIVE_MUTATION_TRANCHE1_EVIDENCE.json` | P36-D013 append-only, context-labelled evidence |

---

## DIAGNOSTIC (10)

Not product behaviour. Every one carries an in-file `P36`/`Hermes` provenance
comment. Tracked as debt below.

| File | What it is |
|---|---|
| `tools/p36-probes/ipc_probe.rs` | Named-pipe connect probe (was `crates/ipc/examples/`) |
| `tools/p36-probes/ipc_invalid_probe.rs` | Malformed-frame probe |
| `tools/p36-probes/ipc_rawwire_probe.rs` | Raw-wire framing probe |
| `tools/p36-probes/ipc_request_probe.rs` | Typed-request probe |
| `tools/p36-probes/ipc_two_client_probe.rs` | Concurrent-client probe |
| `tools/p36-probes/ipc_verb_probe.rs` | Per-verb probe |
| `crates/ipc/src/lib.rs` | Frame codec widened `pub(crate)` → `pub` plus `#[cfg(windows)] pub mod probe`, self-described "Diagnostic-only surface", existing solely to serve the probes above |
| `crates/cleaner/tests/coordinator.rs` | Test-local `CLEANUP_EXECUTION_SERIALIZER` mutex; the Windows machine-mutation lock is machine-wide and the parallel test harness contends for it. Test-only |
| `crates/collector-runtime/src/lib.rs` | Inside `mod tests`: fixed 60 ms sleep replaced by a 5 s poll for worker exit (ARM64 VM scheduling jitter). Test-only |
| `crates/fleet/src/transport.rs` | Inside `mod tests`: `/usr/bin/true` ssh stub gains a Windows branch. Test-only |

---

## Debt register

| ID | Item | Action |
|---|---|---|
| DBT-P36-001 | Six diagnostic probes relocated `crates/ipc/examples/` → `tools/p36-probes/`. They were compiled by `cargo build --examples` from inside product source; they are now inert (the workspace `members` list is explicit, no globs). | Delete when Windows named-pipe qualification is sealed |
| DBT-P36-002 | `crates/ipc/src/lib.rs` widens the frame codec to `pub` and adds `pub mod probe` for DBT-P36-001. This is a real public-API surface increase in a product crate. | Revert to `pub(crate)` with DBT-P36-001 |
| DBT-P36-003 | `crates/fleet/src/transport.rs` test hard-codes the VM-local path `C:\AetherCore-P36\incoming\ssh-true.cmd`. The test cannot pass on a Windows host that is not the qualification VM. | Create the stub in the test, or gate the test on its presence |
| DBT-P36-004 | Historical P36 icon set used a compile-only placeholder. | Resolved by the reviewed evidence-shield master and regenerated cross-platform set in `9f07df5`; retain this row as historical provenance. |
| DBT-P36-005 | maintenance-service module un-gating (`main.rs`, `protocol.rs`, `router.rs`) compiles router/protocol/streaming/performance/support and the broker trust gates into the Windows service binary for the first time. Recorded SECURITY-SENSITIVE. | Codex review before release packaging; do not expand |
| DBT-P36-006 | `crates/security-audit/src/filesystem.rs` `mode_bits()` Windows fallback is a POSIX-style mapping, **not** Windows ACL evidence. | Native ACL qualification remains open |
| DBT-P36-007 | The P35 full-tree ledger is now permanently stale (91 files). | Re-baseline at the next seal, not before |

## Closed

| ID | Item | Resolution |
|---|---|---|
| DBT-P36-008 | `ipc_probe.exe` was present in the **installed** product directory `C:\Program Files\AetherCore\` on the qualification VM (observed 2026-08-30 by `dir /b`). Origin: hand-copied from `target\release\examples` during tranche-1 probing; no build script or deploy step references it — it never was an MSI component. | Confirmed not an MSI component. Removed from the install image, backed up to `C:\AetherCore-P36\backup-p36-overlapped`. Service remained RUNNING throughout, verbs still return. Closed 2026-08-30. |

## Result

`UNKNOWN = 0`. Every one of the 91 drifting files is attributed to a locked P36
decision, a documented tranche-1 fix, or an explicitly provenance-marked
diagnostic.

## Tranche 3 (2026-08-31) — VM qualification

Items found during Phase 36 VM qualification. Recorded, not remediated, per the
brief's stop rule. Each names the evidence file that holds the raw observation.

| # | item | evidence | why it was not fixed here |
|---|---|---|---|
| 1 | `aetherctl.exe` sits in `C:\Program Files\AetherCore` but is **not authored in `installer/wix/Product.wxs`**. No MSI component owns it, so no MSI action installs, repairs or removes it. It survives uninstall and keeps INSTALLFOLDER alive; a genuinely bare machine gets seven files, not eight, and no `aetherctl`. | `STAGE_A_EVIDENCE.md` A5; `evidence/B2-survival.txt` | Adding a component is a product change beyond this brief's authorization. Decide deliberately whether the CLI is meant to ship in the MSI. |
| 2 | A rebuilt package of the **same version** carries a new PackageCode, so `msiexec /i … REINSTALLMODE=amus` is refused **1638** (`PackagecodeChanging=1`, error 1729). `vamus` is required. | `evidence/A5-install-key.txt` | Behaviour of Windows Installer, not a product defect. Worth pinning in whatever runbook describes same-version reinstall. |
| 3 | Killing the installer engine mid-`FileCopy` runs **no rollback at all** and leaves orphaned payload files with no registration. | `evidence/C1b-injection.txt` | Inherent to Windows Installer — the rollback executor is the process killed. Recorded so the recovery runbook says "run the installer again", which does clean it. |
| 4 | `C:\Windows\Installer\MSICD74.tmp` created by that hard kill is **never removed** by any later successful or rolled-back transaction. | `evidence/C1b-injection.txt`, `evidence/C2-injection.txt` | Cleaning `C:\Windows\Installer` by hand is outside authorization. |
| 5 | A plain `msiexec /i` onto a box holding the C1 orphans failed **1603 / Error 1920** at `StartServices`, where the identical command had passed on a clean box. Windows Installer rolled the failure back completely, including the orphans. Cause **not diagnosed** per the stop rule. | `STAGE_C_EVIDENCE.md` C1 aftermath | The stop rule forbids diagnosing a deviation. The observation is the deliverable. |
| 6 | `tauri.conf.json`'s `beforeBuildCommand` (`pnpm --dir ../ui build`) is run by the Tauri CLI from its own discovered app directory, not from the config's directory, so `../ui` resolves to `<root>\ui` and fails ENOENT. | `STAGE_A_EVIDENCE.md` A1; `evidence/A2-build.log` | Worked around for ARM64 with a recorded config overlay rather than editing the shared config. The x64 pipeline calls the same hook and may hit the same thing. |
| 7 | `aethercore-desktop.exe` is **not byte-reproducible** across Tauri rebuilds of identical sources (`5f8d771f…` -> `c351ccf0…`, same size). | `evidence/A2-build.log`, `evidence/B4-build.log` | Expected; byte-reproducibility is explicitly not a Phase 36 criterion. Noted because `RELEASE-METADATA.json` already sets `msi_byte_reproducible_claim = false`. |
| 8 | The Parallels host volume is at **100% capacity, ~6.8 GiB free**, which blocked snapshot creation from B4 through Stage C. | `SESSION_CONTEXT.md` §13 | Human action. Deleting snapshots is forbidden; the large host directories are the user's data. |
| 9 | The only `libomp140.aarch64.dll` available on this VM is from the VS redist **`debug_nonredist`** tree. It is the file tranche 1 shipped and the one the recorded recipe stages. | `scripts/build-arm64-msi.cmd` step [4] | Recorded, not changed. A production ARM64 package should source the redistributable OpenMP runtime, not the `debug_nonredist` copy. |
