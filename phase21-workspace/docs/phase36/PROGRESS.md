# Phase 36 Progress

STATUS: EXECUTING — TRANCHE 1 COMPLETE, VM IS IN A MUTATED (INSTALLED) STATE
CURRENT_WORKSTREAM: P36-04 native qualification gates — tranche 1 sealed; ServiceJob IPC defect under investigation
EXECUTION_CONTEXT: Host macOS -> Parallels `Windows 11` via `prlctl exec`; bridge context is `SYSTEM`

INSTALL_STATE: INSTALLED — the real ARM64 MSI is installed and the service is RUNNING.
Verified 2026-08-30 by `sc query` / `sc qc` over the `prlctl exec` bridge:

```
SERVICE_NAME: AetherCoreMaintenance
STATE              : 4  RUNNING
START_TYPE         : 2  AUTO_START (DELAYED)
BINARY_PATH_NAME   : "C:\Program Files\AetherCore\aethercore-maintenance-service.exe"
SERVICE_START_NAME : LocalSystem
```

`P36-PRE-NATIVE-MUTATION` was NOT restored. `P36-CLEAN-BASELINE` remains the
rollback boundary. The earlier "do not install or mutate" instruction is
SUPERSEDED — the install already happened under tranche-1 authorization.
VM_WINDOWS_NATIVE_ARM64: True
PHYSICAL_X64_WINDOWS_QUALIFICATION_STILL_REQUIRED: True

## COMPLETED

- Graphify-first navigation completed against the external
  `_graphify/aethercore-lean/graph.json`. The authoritative source set is the
  P35 release/audit scripts, package/toolchain manifests, Tauri/WiX definition,
  update authority, and Windows IPC implementation.
- P35 archive SHA-256 precheck passed:
  `a6d8ac72c9c2f5321fd8f29e4082a908468134af98584108c53cf647e514f46e`.
- `PHASE35_FINAL_SHA256.txt` contains the same SHA and filename.
- Read-only `scripts/phase35-adversarial-audit.py` passed `998 checks`,
  `0 failures`, status `PASS`.
- Archive bytes were independently identified as GZip-compressed TAR with
  `1126` file members and no absolute, parent-traversal, or backslash member
  names. The `.zip` suffix is retained; extraction will be signature-aware.
- Phase 36 control files created: `EXECUTION_PLAN.md`, `DECISIONS.md`, and
  this checkpoint.
- VM facts recorded by a SYSTEM-context command: Windows 11 Pro, version
  `10.0.26200`, build `26200`, OS architecture `ARM 64-bit Processor`, process
  architecture `ARM64`, PowerShell `5.1.26100.9168`.
- `C:\\AetherCore-P36\\incoming`, `workspace`, `evidence`, and `logs` were
  created and verified on `C:` with filesystem `NTFS`.
- Windows transport/hash/extraction passed: the VM-local copy matches the P35
  SHA, and Windows TAR extraction produced `1126` file members under the
  expected archive root with no forbidden target/node_modules/_graphify/private
  key paths.
- Toolchain bootstrap passed for Rustup/rustc `1.97.1` ARM64 MSVC, Node.js
  `24.20.0` ARM64, npm `11.19.0`, pnpm `11.22.0`, .NET `8.0.424` ARM64,
  Visual Studio Build Tools `17.14.37614.0` with ARM64 MSVC tools, WiX
  `6.0.2`, and OpenSSH client capability. Evidence:
  `C:\\AetherCore-P36\\evidence\\toolchain.json` and
  `C:\\AetherCore-P36\\evidence\\native-tools.json`.
- Cargo metadata smoke passed (`50` packages / `50` workspace members),
  evidence: `C:\\AetherCore-P36\\evidence\\cargo-metadata.json`.
- UI smoke passed directly with `0` errors and `17` pre-existing warnings;
  the repository wrapper exits non-zero because it intentionally fails on
  warnings. The UI production build passed (Vite `8.2.1`, `195` modules).
- Targeted native tests passed for persistence (`31`), release authority (`9`),
  update engine (`18`), and DB diagnostics (`10`); evidence:
  `C:\\AetherCore-P36\\evidence\\targeted-tests.json`.
- WiX `6.0.2` synthetic MSI compile passed using an explicitly marked smoke
  payload (not a release payload). `wix msi validate` then failed ICE38,
  ICE43, and ICE57 on the existing ProgramMenu/Desktop shortcut component
  authoring; no install was attempted. Evidence:
  `C:\\AetherCore-P36\\evidence\\wix-smoke.json`.
- Snapshot gate passed before any system mutation: Parallels created and
  verified `P36-CLEAN-BASELINE` (snapshot ID
  `{6b721a10-8ac1-4339-9699-bd5145efda0a}`, state `poweron`, current `true`) on
  `2026-08-28 23:26:48`.
- Read-only native servicing/security queries passed: DISM current edition
  `Professional`, `sfc /verifyonly` reported no integrity violations, CBS log
  is present, Windows Update services are running, OpenSSH 9.5p2 is present,
  Firewall profiles and Defender real-time/tamper protection are enabled. The
  AetherCore service was reported `NOT_INSTALLED` at that time because MSI
  install was still deferred. Evidence:
  `C:\\AetherCore-P36\\evidence\\native-queries.json`. **SUPERSEDED 2026-08-29
  by tranche 1** — the real ARM64 MSI is now installed and
  `AetherCoreMaintenance` is RUNNING; see `INSTALL_STATE` at the top of this
  file. This bullet is retained as a dated pre-mutation observation only.
- UAC is enabled (`EnableLUA=1`, administrator consent level `5`); the VM's
  existing `PromptOnSecureDesktop=0` value was observed and left unchanged.
- Installer source inspection confirms the authored per-machine scope,
  LocalSystem `AetherCoreMaintenance` service, and deferred non-impersonated
  hardening action in `installer/wix/Product.wxs`; runtime registration,
  Service SID, and ACL evidence remain intentionally open until a valid MSI is
  available.
- Windows ARM64 `aethercore-windows-update` compile and safe unit-test gates
  now pass under the pinned toolchain. The Windows-rs 0.62 callback contract
  requires implementations on generated `*_Impl` types, and `HResult()` is
  already an `i32`; the four callback impl targets and two stale `.0` accesses
  were corrected. `cargo check -p aethercore-windows-update --locked` and
  `cargo test -p aethercore-windows-update --locked` passed (3 tests; live WUA
  applicability test remains intentionally ignored).
- Windows ARM64 `aethercore-ipc` compile and tests now pass under the pinned
  toolchain. `Error::from_thread()` preserves the immediately-following
  `GetLastError()` semantics of `CreateNamedPipeW`; the `pending` map received
  its declared `SyncSender<Response>` value type. A Windows-only test join
  ownership issue and the Unix adversarial test's missing platform gate were
  also corrected. `cargo test -p aethercore-ipc --locked` passed (17 tests;
  Unix adversarial binary is correctly empty on Windows).

## IN_PROGRESS

- Workspace ARM64 compile gate **PASSED (Hermes, 2026-08-29)**:
  `cargo check --workspace --locked` EXIT=0 on Windows ARM64 under the pinned
  Codex-established toolchain environment (VsDevCmd -arch=arm64 + ARM64
  CMake/Ninja/clang-cl + LIBCLANG_PATH), evidence:
  `C:\AetherCore-P36\incoming\cargo-workspace2.{log,status}`.
- `cargo test --workspace --locked` launched (detached, log+status pattern).
- Complete non-destructive crate/test and installer-source validation where the
  corrected native crates are included.
- Use the verified `P36-CLEAN-BASELINE` as the rollback boundary for any later
  installation, service, ACL, UAC, update, or rollback mutation.
- Record explicit context labels; the current bridge evidence is SYSTEM only
  and is not Administrator/StandardUser/UAC evidence.

## HERMES DELTA (2026-08-29) — files changed by Hermes, separately recorded

All changes are minimal Windows-rs 0.62.2 / cfg-gating compatibility fixes in the
P36-D019 authorized scope. Each file synced Mac->VM with SHA-256 verification.

1. `crates/crash-diagnostics/src/windows_impl.rs` — `EvtQuery` flags: windows-rs
   0.62.2 `EVT_QUERY_FLAGS` no longer implements `BitOr` and `EvtQuery` takes raw
   `u32`; changed to `EvtQueryChannelPath.0 | EvtQueryReverseDirection.0`.
   Targeted `cargo check -p aethercore-crash-diagnostics --locked` EXIT=0.
2. `crates/startup-manager/src/windows_impl.rs` — `VARIANT` moved from
   `windows::core` to `Win32::System::Variant`; `BOOL` moved to
   `windows::core` (re-export of windows-result). Import-path-only fix.
3. `crates/security-audit/src/filesystem.rs` — `mode_bits()`: `std::os::unix`
   import was unconditional; added `#[cfg(windows)]` fallback mapping
   (read-only vs writable) preserving the consumed security bits.
   Targeted check EXIT=0.
4. `crates/security/src/lib.rs` — `BOOL` → `windows::core`; `OpenProcessToken`
   moved `Win32::Security` → `Win32::System::Threading`. Import-path-only.
5. `crates/idle-scheduler/src/windows_state.rs` — `RPC_C_AUTHN_WINNT`/
   `RPC_C_AUTHZ_NONE` moved `System::Com` → `System::Rpc`. Import-only.
6. `apps/aetherctl/src/transport.rs` — Windows lane `CliError::Rejected` now
   fills the `detail` field (E0063). Minimal field completion.
7. `apps/desktop/src/main.rs` — event normalization match added the two missing
   contract payload variants `PlatformCapabilities`/`SecurityAudit` (exhaustive
   match; wire mapping mirrors the unix side).
8. `services/maintenance-service/src/main.rs` — module gates: `errors`,
   `performance`, `protocol`, `router`, `streaming`, `support` now
   `#[cfg(any(unix, windows))]` (platform-neutral wire/routing modules; their
   genuinely unix-only items keep internal gates). `ctrlc_handler` BOOL import
   moved to `windows::core` per 0.62 move.
9. `services/maintenance-service/src/protocol.rs` — `snapshot()` un-gated
   (platform-neutral mapping).
10. `services/maintenance-service/src/router.rs` — `require_broker`,
    `expected_broker_path`, `require_update_broker`,
    `expected_update_broker_path` un-gated (platform-neutral PrincipalContext
    trust checks; no behavior change).
11. `apps/desktop/icons/` — the sealed P35 `icon.png` was corrupt (truncated
    zlib stream: IHDR promises 32x32xRGBA=4128 scanline bytes, stream yields
    3104; Tauri CLI decode fails; identical corrupt bytes present in ALL sealed
    phase archives 30-35, an inherited P20-era defect never caught on macOS
    where icon.ico is not required). Windows tauri-build hard-requires
    `apps/desktop/icons/icon.ico`. Disposition: generated a NEW 1024x1024
    placeholder mark (`_p36-icon-source-1024.png`, SHA-256 F50A2BFD...114F7C0E,
    AI-generated, NOT the product mark) and derived the full icon set
    (icon.ico/icon.icns/PNG set) with the project's own pinned Tauri CLI 2.11.4
    from the sealed lockfile (`tauri icon`). Owner decision required on final
    artwork before any release packaging; this unblocks compile-only
    qualification only. Corrupt original replaced on both trees.
12. All of the above compiled clean together: workspace check EXIT=0.

## NOT_STARTED

- Runtime MSI installation, service registration/SID/ACL inspection, and
  named-pipe broker/service exercise (installer ICE validation remains open).
- Desktop launch under a normal interactive user, update staging/apply/rollback,
  and destructive driver/repair loops.
- Controlled Administrator/StandardUser UAC and user-AppData tests.
- Physical x64 Windows PC handoff and final P36 boundary review.

---

# P36_ARM_VM_PRE_MUTATION_READY (Hermes checkpoint — 2026-08-29, session 2)

**P36_ARM_VM_PRE_MUTATION_READY=True**

Snapshot `P36-PRE-NATIVE-MUTATION` created and verified:
- ID `{e9434b5f-ba68-452d-ac4c-cbcc863ffb4b}`, 2026-08-29 16:08:11, poweron, **current=true**,
  parent = `P36-CLEAN-BASELINE` {6b721a10-8ac1-4339-9699-bd5145efda0a}.
- Evidence: `docs/phase36/PRE_NATIVE_MUTATION_EVIDENCE.json`.
- This snapshot is the rollback boundary for all subsequent native mutation
  gates (MSI install, service registration, Service SID, NTFS ACLs, named-pipe
  IPC, StandardUser denial, repair/uninstall/upgrade).

## Gate status after Hermes continuation (2026-08-29, session 2)

| Gate | Result |
|---|---|
| `cargo check --workspace --locked` (ARM64) | **EXIT=0** (re-verified this session) |
| `cargo test --workspace --locked` (ARM64, --no-fail-fast) | **EXIT=0 — 562 passed / 0 failed across 209 test binaries** |
| WiX ICE38/ICE43/ICE57 | in progress (root-cause authoring fixes) |

## Windows ADK installation evidence (toolchain gap closure)

- Installer: official Microsoft fwlink `https://go.microsoft.com/fwlink/?linkid=2289980`
  (ADK 10.1.26100.2454, December 2024) → `C:\AetherCore-P36\incoming\adksetup-10.1.26100.2454.exe`,
  SHA-256 `7F61E29F2314BCDD7E0ABF67A8367D83A05AA4A7B9223F85C5FD2582A35CC6F4`,
  Authenticode **Valid, CN=Microsoft Corporation**.
- Installed silently with ONLY `OptionId.DeploymentTools` (feature id verified
  against learn.microsoft.com ADK offline-install docs; the first attempt with
  the wrong id `OptionId.WindowsDeploymentTools` failed with "features not
  available"; two earlier runs failed 1001/1618 due to orphaned adksetup
  processes holding the global setup mutex — killed and reinstalled clean).
- Install root: `C:\Program Files (x86)\Windows Kits\10\` (pre-registered path).
- **DismApi.lib**: `C:\Program Files (x86)\Windows Kits\10\Assessment and Deployment Kit\Deployment Tools\SDKs\DismApi\Lib\arm64\dismapi.lib`
  (61,956 bytes, llvm-readobj: `Machine: IMAGE_FILE_MACHINE_ARM64 (0xAA64)`).
- **dismapi.h**: `C:\Program Files (x86)\Windows Kits\10\Assessment and Deployment Kit\Deployment Tools\SDKs\DismApi\Include\dismapi.h`.
- Additional arm64/x86/amd64 libs under the same `SDKs\DismApi\Lib\` tree.
- Link-time env addition (invocation-only, no source change): `LIB` extended
  with the arm64 DismApi dir above.

## cargo test gate — fixes applied during this session (all targeted-verified)

1. **OpenMP link (LLK2019 ×13 `__kmpc_*`)** — clang-cl's ARM64 OpenMP lowering
   requires the MSVC OpenMP import lib; added `RUSTFLAGS=-C link-arg=libomp.lib`
   (env-only) and the matching runtime DLL dir
   `...\VC\Redist\MSVC\14.44.35112\debug_nonredist\arm64\Microsoft.VC143.OpenMP.LLVM`
   to PATH (the test exes import `libomp140.aarch64.dll`; STATUS_DLL_NOT_FOUND
   without it).
2. **`crates/fleet/src/trust.rs` `ssh_binary()`** — Windows binaries are
   `ssh.exe`; the extension-less lookup always returned None (SshMissing).
   Added `#[cfg(windows)] ssh.exe` candidate. Honest-detection bug, not security.
3. **`crates/fleet/src/transport.rs` (test)** — trust_1 used unix-only stub
   `/usr/bin/true`; on Windows a batch stub `C:\AetherCore-P36\incoming\ssh-true.cmd`
   (`@exit 0`) is used instead (test-infra only, cfg-split).
4. **`crates/cleaner/tests/coordinator.rs`** — the two execution-driving tests
   contended for the REAL machine-wide mutation lock when the harness ran them
   in parallel threads (passes serially). Serialized on a test-local mutex;
   no product change.
5. **`crates/collector-runtime/src/lib.rs` (test)** — fixed 60 ms sleep raced
   VM thread scheduling; replaced with a bounded 5 s poll for gate release.
   Same contract proven, no busy-assumption.

## cargo check exact result

`cargo check --workspace --locked` on Windows ARM64 (VsDevCmd -arch=arm64
-host_arch=arm64 + ARM64 CMake 4.4.2/Ninja 1.13.2/clang-cl 22.1.8 +
LIBCLANG_PATH + /EHsc): **EXIT=0**
Evidence: `C:\AetherCore-P36\incoming\cargo-workspace2.{log,status}`.

## Dependency-freeze disposition (recorded 2026-08-29, per P36-D017)

Verified live on the VM workspace:
- `Cargo.lock` (152,186 bytes, SHA-256 `54DA8A3D9F41159D…`) present, byte-identical
  between Mac and VM.
- `pnpm-lock.yaml` present. UI smoke earlier used `pnpm install --frozen-lockfile
  --trust-lockfile` (bounded exception, integrity checks preserved).
- **Absent from the sealed P35 archive** (blocker states it; verified): 
  `release/dependency-freeze.json`, `release/dependency-locks.sha256`,
  `release/dependency-manifests.sha256`.
- `release/dependency-freeze.blocker.json` (OMEGA-RB-001, severity
  release_blocker) remains **OPEN** in the sealed tree — by design it is only
  closed by a successful `scripts/freeze-dependencies.ps1 -Refresh` on the
  trusted dependency-freeze workstation.

**Disposition:** P36 ARM-VM qualification proceeds under the P36-D017 bounded
exception (sealed lockfiles + `--frozen-lockfile` + `--trust-lockfile`; no
dependency graph changes, no version bumps, no lockfile edits). Release
packaging remains BLOCKED: the freeze set must be regenerated and reviewed on
the trusted freeze workstation, and this gate cannot be closed from the VM.
The blocker file stays OPEN and unchanged. Every `--locked` cargo gate in this
session is deterministic against the sealed `Cargo.lock` bytes above.

## WiX validation exact result — **RESOLVED (2026-08-29, Hermes)**

Root-cause authoring fixes in `installer/wix/Product.wxs` (no ICE suppression):

1. **ICE43/ICE57 (shortcut KeyPath mixing):** the Start-Menu shortcut was
   non-advertised (`Advertise="no"`) inside `DesktopComponent` whose KeyPath is
   a file under ProgramFiles64Folder, while the shortcut landed under
   ProgramMenuFolder (All-Users profile under per-machine scope). Remedy per
   Microsoft docs: author the shortcut **advertised** (`Advertise="yes"`, child
   of the File element) plus `DISABLEADVTSHORTCUTS=1` so Windows Installer
   materializes an ordinary .lnk at install time. Advertised shortcuts may
   legally use the file KeyPath; no per-user KeyPath is introduced.
2. **ICE38 (ProgramMenuComponent):** the folder-owner component installed into
   ProgramMenuFolder but used an HKLM registry KeyPath. Changed its KeyPath
   registry entry to **HKCU** (`Software\AetherCore\ProgramMenu`), the required
   per-user KeyPath for a component whose directory is under the user profile.
   It owns only folder create/remove; the HKLM `InstallVersion` entry stays in
   `ProgramDataComponent` (per-machine, correct location).

Validation (synthetic smoke payload, clearly marked NOT-release):
- `dotnet wix build -arch arm64` → EXIT=0 (WiX 6.0.2 via pinned
  `.config/dotnet-tools.json`, `dotnet tool restore` EXIT=0).
- `dotnet wix msi validate` → **EXIT=0, zero ICE38/ICE43/ICE57 findings**.
- Smoke MSI: `C:\AetherCore-P36\incoming\wix-smoke-fixed.msi` (40,960 bytes,
  SHA-256 `51A0AC9E5A72979D21B03C416BDC42841F958F80A11051F2DE8D9362F031088F`),
  synthetic ProductCode `{A36C0DE0-1111-4000-8000-000000000036}`, synthetic
  75-byte placeholder payload files explicitly marked NOT FOR RELEASE.
- Logs: `wix-build-fixed.{log,status}`, `wix-validate-fixed.{log,status}`.
- The **synthetic smoke MSI** was NOT installed and NOT promoted to release
  packaging (P36-D018 respected); the synthetic compile/validate is
  source/schema evidence only. This bullet scopes the synthetic smoke artifact
  only — it says nothing about the real ARM64 product MSI, which **is**
  installed (see `INSTALL_STATE`).

## Administrator/StandardUser/UAC evidence method (established 2026-08-29)

Machine-level test accounts created for evidence purposes (names only — no
passwords anywhere in repo/logs/evidence; each password was random, shown once
at creation, and reset per probe invocation):

- `P36Admin` — local Administrator-group member (UAC High when elevated).
- `P36StandardUser` — plain Users member (UAC Medium), never elevated.

UAC posture (unchanged, P36-D012): `EnableLUA=1`, `ConsentPromptBehaviorAdmin=5`,
`ConsentPromptBehaviorUser=3`.

**Evidence method (headless-safe, no interactive logon):** one-shot Scheduled
Tasks registered per probe with `-User/-Password` (Password logon type) and
`-RunLevel Limited`; the task writes `whoami /groups` output under the target
user's own temp, then the evidence file is copied into
`C:\AetherCore-P36\evidence\`. Verified results:

- `p36admin-groups.txt`: `Mandatory Label\High Mandatory Level (S-1-16-12288)`
  plus `BUILTIN\Administrators` membership.
- `p36std-groups.txt`: `Mandatory Label\Medium Mandatory Level (S-1-16-8192)`,
  **no** Administrators membership.

Note: task scheduling for P36StandardUser required granting
`SeBatchLogonRight` to that account (this VM's baseline grants batch logon only
to Administrators/Backup Ops/Perf Log Users); applied via a minimal secedit INF
scoped to the single account, recorded in
`C:\AetherCore-P36\evidence\batch-right3.inf` (+ secedit export copies). This
is machine-level test-context preparation, not a product mutation, and is
reversible by re-running the same INF without the account SID.

SYSTEM-context evidence never satisfies Administrator/StandardUser criteria
(P36-D009): all future UAC/StandardUser-denial gates must run through this
task method (or a future interactive session), with the context label recorded
beside each result.

- `P36-CLEAN-BASELINE` {6b721a10-8ac1-4339-9699-bd5145efda0a} current. NOT restored.
- `P36-PRE-NATIVE-MUTATION`: **all four pre-mutation gates PASS** (check EXIT=0,
  tests EXIT=0 562/0, WiX validate EXIT=0, freeze disposition + context method
  recorded) → creation AUTHORIZED; executed and verified below.

## Remaining blockers (in order)

1. **DismApi.lib / dismapi.h missing from VM SDK** — install the missing
   Windows SDK feature (or ADK DISM component) on the VM; NO source change
   authorized to work around a missing toolchain component.
2. WiX ICE38/ICE43/ICE57 authoring defects (P36-D018).
3. dependency-freeze disposition (P36-D017).
4. SYSTEM-context-only evidence (P36-D009).
5. Physical x64 qualification (P36-D004/D015).

## SECURITY-SENSITIVE DELTA (Codex must review before any release packaging)

The maintenance-service un-gating below is SECURITY-SENSITIVE: it changes which
modules compile into the Windows service binary. No behavior change was
intended or detected, but the Windows named-pipe service path now compiles
router/protocol/streaming/performance/support and the broker trust gates for
the first time. Review scope:
- `services/maintenance-service/src/main.rs` (module gates + ctrlc BOOL import)
- `services/maintenance-service/src/protocol.rs` (`snapshot()` un-gated)
- `services/maintenance-service/src/router.rs` (`require_broker`,
  `expected_broker_path`, `require_update_broker`,
  `expected_update_broker_path` un-gated)
Authorization basis: P36-D019 compatibility scope; the named-pipe transport
enforces the same broker contract on Windows. Recorded as
SECURITY-SENSITIVE DELTA — DO NOT EXPAND further without owner/Codex review.
Security-audit `mode_bits()` Windows fallback is POSIX-style mapping and is
NOT Windows ACL evidence; native ACL qualification remains open.

## Icon placeholder status

`COMPILE_ONLY_PLACEHOLDER=True`. The sealed P35 icon.png was corrupt
(truncated zlib stream, identical in ALL sealed archives 30-35, inherited
P20-era defect). Replaced on both trees with an AI-generated placeholder
source + full Tauri-CLI-derived icon set. NOT final product branding; owner
must approve/release real artwork before any release packaging.

## NEXT ACTION (exact, for the next executor)

1. Sync this PROGRESS.md to the VM workspace (SHA-256 verify).
2. Continue the P36 sequence **under the P36-PRE-NATIVE-MUTATION rollback
   boundary**: native ARM VM MSI qualification (real payload build), service
   registration, Service SID, NTFS ACL qualification, native named-pipe IPC,
   StandardUser denial / Administrator behavior (via the established task-probe
   method), repair/uninstall/upgrade preparation.
3. No physical-driver qualification yet. No Phase 37.
4. Windows runtime qualification of the corrected Product.wxs shortcut
   authoring (advertised + DISABLEADVTSHORTCUTS) happens during the MSI
   qualification gate; desktop-launch evidence under P36StandardUser must use
   the established task method.

## WINDOWS EVIDENCE GENERATED

- Host: Parallels snapshot list verified `P36-CLEAN-BASELINE`; captured in
  `docs/phase36/SNAPSHOT_EVIDENCE.json`.
- VM: `toolchain.json`, `native-tools.json`, `cargo-metadata.json`,
  `cargo-check.json`, `targeted-tests.json`, `wix-smoke.json`,
  `native-queries.json`, and `native-gates.json`.
- Raw/sanitized command logs: `C:\\AetherCore-P36\\logs\\` (`cargo-check`,
  `targeted-tests`, `ui-smoke`, `dism-currentedition`, `sfc-verifyonly`,
  `windows-services`, `wix-validation`, and `native-gates`).
- Evidence root: `C:\\AetherCore-P36\\evidence`.
- Log root: `C:\\AetherCore-P36\\logs`.

---

# HERMES PRE-MUTATION REVIEW COMPLETE (2026-08-29, session 3 — Codex interrupted-review finish)

## EVIDENCE_GAP_CLOSED=True — StandardUser/Admin token evidence (actual-token, bounded probes)

Codex finding resolved: the documented `p36std-groups.txt` was missing. Both
contexts were re-probed with the established one-shot Scheduled Task method
(Password logon, random per-probe password reset on the VM, never printed or
persisted; probe scripts in `_handoff/p36-codex-to-hermes/` and
`C:\AetherCore-P36\evidence\{std,admin}-token-probe-inner.ps1` + drivers):

- `C:\AetherCore-P36\evidence\P36StandardUser-token-groups.txt` (NEW, 2026-08-29T13:48:09Z,
  task `P36StdTokenProbe`, RunLevel=Limited, LastTaskResult=0):
  - `whoami` = `hasanalaaa3a44\p36standarduser`; SID `S-1-5-21-...-1003`
  - `whoami /groups` = Medium Mandatory Level (S-1-16-8192); group list has NO
    `S-1-5-32-544` entry
  - machine checks: `MACHINE_CHECK S-1-5-32-544=ABSENT_FROM_TOKEN`;
    `IsInRole_BUILTIN_Administrators=False`
- `C:\AetherCore-P36\evidence\P36Admin-token-evidence.txt` (NEW, 2026-08-29T13:48:56Z,
  task `P36AdminTokenProbe`, RunLevel=Highest, LastTaskResult=0):
  - `whoami` = `hasanalaaa3a44\p36admin`; SID `S-1-5-21-...-1002`
  - High Mandatory Level (S-1-16-12288); `BUILTIN\Administrators` present,
    `Enabled by default, Enabled group, Group owner`;
    `IsInRole_BUILTIN_Administrators=True`
- Prior `p36admin-groups.txt` (unchanged) remains consistent with the new
  Admin probe. SYSTEM-side group configuration evidence (secpol exports,
  batch-right INFs) unchanged and remains supplementary only.

## Targeted security review results (Codex continuation; no source files modified)

- **maintenance-service=ACCEPT.** `require_broker`/`require_update_broker`
  fail closed when the expected path cannot be resolved
  (`authorization.brokerPathUnavailable`/`update.error.brokerPath` → forbidden);
  both gates check `aethercore_security::is_expected_broker`, which requires
  `elevated=true`, absolute expected path, and exact full-path (case-/
  separator-normalized, `\\?\`-aware) equality — prefix attacks and relative
  paths are rejected by unit test. Executable/path authority is bounded to
  `<service dir>\aethercore-{consent,update}-broker.exe`. The Windows named-pipe
  composition preserves locked trust semantics: `server.rs` captures the
  principal once from the real connected pipe token
  (`inspect_named_pipe_client` → `GetNamedPipeClientProcessId` + client token,
  crates/security — delta was import-path moves only) with per-SID session
  admission; `windows_service_host::service_main_impl` fails before
  SERVICE_RUNNING if `verify_maintenance_service_token` does not match the
  authored SID policy. Un-gated router/protocol modules are platform-neutral
  wire mapping/authorization plumbing; no authorization bypass introduced; the
  unix un-gating did not weaken Windows security (unix modules keep internal
  cfg(unix) gates).
- **WiX=ACCEPT.** `installer/wix/Product.wxs`: `Scope="perMachine"`; all
  executables + `update-trust.json` machine-owned under ProgramFiles64Folder
  (no private key installed); service LocalSystem ownProcess with deferred
  non-impersonated hardener invoked with the literal verb `apply` and fixed
  System32 paths; the HKCU `Software\AetherCore\ProgramMenu` KeyPath exists
  only as the folder-owner marker required by ICE38/ICE43 for a component
  whose directory is under the (All-Users) profile — it owns only folder
  create/remove and is consumed by no trust check, so it is not an authority
  boundary; advertised Start-Menu shortcut + `DISABLEADVTSHORTCUTS=1`
  materializes an ordinary .lnk and moves no privileged state; repair is not
  suppressed (no ARPNOREPAIR) and ServiceControl removes the service on
  uninstall. Windows runtime qualification of this authoring remains a
  separate MSI gate.
- **security-audit=ACCEPT-WITH-BOUNDARY.** `crates/security-audit/src/
  filesystem.rs` Windows `mode_bits()` is a read-only compatibility mapping
  (0o444 | 0o222-if-writable). BOUNDARY: it is NOT NTFS ACL evidence. On
  Windows the world-writable/SUID/ssh-permission heuristics (SEC-FS-001..004)
  are POSIX-style approximations only; native P36 ACL qualification
  (service dir, ProgramData, pipe ACLs) must use Windows-native ACL
  inspection (Get-Acl/icacls/SecurityDescriptor APIs) and remains open.

## Build reproducibility review (Codex continuation)

**build-reproducibility=ACTION_REQUIRED.** The successful Windows ARM64
qualification currently depends on requirements that exist ONLY as manual
invocation state (handoff §7 command / VM evidence); none are encoded in
`scripts/bootstrap.ps1`, `scripts/build-release.ps1`,
`scripts/build-installer.ps1`, `.cargo/config.toml`, or `.github/workflows/`
(grep-verified: zero occurrences). Must eventually be encoded in official
build/bootstrap/CI tooling (NOT implemented in this review):
1. ADK DeploymentTools component providing
   `...\Assessment and Deployment Kit\Deployment Tools\SDKs\DismApi\Lib\<arch>\DismApi.lib`
   + `Include\dismapi.h` (installed 10.1.26100.2454; LIB extended invocation-only).
2. `RUSTFLAGS=-C link-arg=libomp.lib` (ARM64 clang-cl OpenMP lowering needs the
   MSVC OpenMP import lib for `__kmpc_*`).
3. `...\VC\Redist\MSVC\14.44.35112\debug_nonredist\arm64\Microsoft.VC143.OpenMP.LLVM`
   on runtime PATH (`libomp140.aarch64.dll`; STATUS_DLL_NOT_FOUND without it).
4. `LIBCLANG_PATH=C:\AetherCore-P36\toolchain\llvm-22.1.8\bin\libclang.dll`.
5. clang-cl ARM64 as CC/CXX (`CC/CXX_aarch64_pc_windows_msvc=clang-cl`).
6. ARM64 CMake 4.4.2 (`CMAKE_GENERATOR=Ninja`, `CMAKE_{C,C}_COMPILER=clang-cl`) —
   MSVC hard-rejects ARM for llama.cpp.
7. ARM64 Ninja 1.13.2.
8. `/EHsc` via `CXXFLAGS` + `CXXFLAGS_aarch64_pc_windows_msvc`.
(bootstrap.ps1 rejects ARM64 by design; x64 CI/physical gates are a separate
path — this encoding debt applies to any future ARM64 official tooling.)

## Final decision block

- maintenance-service=ACCEPT
- WiX=ACCEPT
- security-audit=ACCEPT-WITH-BOUNDARY (Windows mode_bits() is NOT NTFS ACL evidence; native ACL qualification still required)
- Admin-StandardUser=PASS (both token evidences fresh, machine-readable, bounded)
- build-reproducibility=ACTION_REQUIRED (8 ARM64 requirements manual-state only; list above)
- EVIDENCE_GAP_CLOSED=True
- FILES_CHANGED_BY_REVIEW= (none — no source/config file modified; new evidence + probe scripts only)
- TARGETED_PROBES=P36StdTokenProbe(one-shot task, RunLevel=Limited)→P36StandardUser-token-groups.txt; P36AdminTokenProbe(one-shot task, RunLevel=Highest)→P36Admin-token-evidence.txt
- ARCHITECTURAL_DECISION_REQUIRED=False
- NATIVE_MUTATION_RECOMMENDATION=AUTHORIZED

HERMES_PRE_MUTATION_REVIEW_COMPLETE=True

STOP — no MSI install, no service registration, no product ACL change, no
named-pipe mutation, no Phase 37.

> **SUPERSEDED 2026-08-29.** The above was the end-of-session-3 boundary, before
> native mutation tranche 1 was authorized and executed. The real ARM64 MSI is
> now installed, `AetherCoreMaintenance` is registered and RUNNING, and tranche 1
> is COMPLETE. Do not read this STOP line as a current instruction; the current
> state and the current boundary are the `INSTALL_STATE` block at the top of this
> file. Phase 37 remains out of scope.

## FAILURES

---

# HERMES NATIVE MUTATION TRANCHE 1 COMPLETE (2026-08-29, session 4)

**P36_NATIVE_MUTATION_TRANCHE1=PASS**

All workstreams A–G executed against the REAL ARM64 payload inside the VM under the
`P36-PRE-NATIVE-MUTATION` rollback boundary (never restored; snapshot verified
current before mutation). Full machine-readable evidence:
`docs/phase36/NATIVE_MUTATION_TRANCHE1_EVIDENCE.json`. Raw logs/evidence:
`C:\AetherCore-P36\{logs,evidence}\tranche1-*`.

## Workstream results

| Workstream | Result | Key evidence |
|---|---|---|
| A — REAL ARM64 payload | PASS | 5 exes + OpenMP DLL, all PE `IMAGE_FILE_MACHINE_ARM64 (0xAA64)` via llvm-readobj; SHA-256 per file in evidence JSON; no synthetic bytes |
| B — Real MSI | PASS | `wix build -arch arm64` + `wix msi validate` both EXIT=0, zero ICE findings, no suppression; ProductCode `{FC8A3841-759D-B452-1864-161F84F56C03}`, UpgradeCode `{45598C77-2C32-5BCE-8510-19C7E51EE3B8}`, v0.1.0; final MSI SHA-256 `1C99CB1A2FD268652BA570ADD5A33C15007EE78FC3C8AF93BBEA424BFC2CD230` |
| C — Controlled install | PASS | msiexec EXIT=0 with `/l*v` log; all files hash-verified in `C:\Program Files\AetherCore`; Start-Menu .lnk materialized (advertised + DISABLEADVTSHORTCUTS=1); HKCU ProgramMenu marker written to installer-context hive only, consumed by no trust check; **no private key material** (PEM scan 0 hits); `update-trust.json` = template (enabled=false, 0 channels) |
| D — Service registration | PASS | `AetherCoreMaintenance`: WIN32_OWN_PROCESS, LocalSystem, AUTO_START (DELAYED), NORMAL error control, binary in `C:\Program Files\AetherCore`; clean start; RUNNING |
| D — Service SID | PASS | `SERVICE_SID_TYPE: UNRESTRICTED`; SID `S-1-5-80-4285065559-3530017622-2858480679-3751456793-1187574229` **Active** (sc showsid); service SD = exact hardener SDDL; runtime token check (`CheckTokenMembership` on service SID before SERVICE_RUNNING) passes |
| E — Native ACL | PASS | Get-Acl SDDL + icacls only (`mode_bits()` NOT used): install dir + exes = Users/service-SID `0x1200a9` RX, SYSTEM/BA FA, no Users write; ProgramData dir has NO Users ACE; `machine-mutation.lock` protected DACL (SY/BA/service-SID only). StandardUser write denials runtime-verified (error 5) |
| F — Named-pipe IPC | PASS (after defect fix) | v7 handshake `CONNECT=OK` as SYSTEM; StandardUser raw open with exact production mask = SUCCESS (intended client path); pipe serviced by the service (vanishes on stop — verified); update authority fail-closed with trust disabled |
| G — StandardUser denial | PASS | exe write / trust write / create-in-install-dir / mutation-lock write: ALL `Access denied (0x5)`; `sc stop` + `sc config`: `[SC] OpenService FAILED 5`; harmless probes only, artifacts cleaned |
| Admin behavior | PASS | pipe connect OK; service-exe write DENIED while running (in-use lock); trust JSON + mutation lock Admin-FA per locked policy; `sc stop` OK then clean restart — matches locked policy, no ACL "fixes" applied |

## Defects found, root-caused, fixed (3 — all smallest-correction, all retested)

1. **Product.wxs launch condition could never pass.** `WindowsBuild` is
   compat-capped at 9600 (Windows 8.1 values) for unmanifested MSI packages on
   ALL modern Windows (log: `Property(S): WindowsBuild = 9600` on build 26200).
   Fix: RegistrySearch of `HKLM\...\CurrentVersion\CurrentBuildNumber`
   (`OSCURRENTBUILD >= 22621`). First fix attempt needed a WiX v4 rename
   (`Win64` → `Bitness="always64"`).
2. **Hardener `run_icacls` /T defect (security-relevant).** `icacls <dir>
   /inheritance:r /grant:r <(OI)(CI) ACEs> /T` applies the whole argument set to
   every descendant: on FILES, `/inheritance:r` leaves a protected EMPTY DACL
   (`D:PAI`) and the `(OI)(CI)` grant silently no-ops → service exe unreadable
   even to SYSTEM → SCM "Access is denied" → MSI Error 1920/1603. Proven by
   controlled icacls characterization (grants on files are silent no-ops;
   `D:PAI` after `/inheritance:r` on a file) and by needing `takeown` to recover
   the residual dir. Fix: drop `/T` from the grant pass (the preceding
   `reset_acl_tree /T` pass has already restored children to pure inheritance, so
   the `(OI)(CI)` directory ACEs propagate — the declared policy).
3. **Production pipe DACL unusable as encoded.** `production_pipe_security_descriptor`
   granted AU the hex mask `0x00120003`, but the Windows SDDL parser **silently
   drops SYNCHRONIZE (0x00100000) from hex masks** → materialized DACL = `0x120003`
   → every client (SYSTEM/Admin/StandardUser/desktop app) denied; pipe unusable.
   Proven by A/B lab pipes + live DACL dumps. Fix: AU ACE now `FR` + hex `0x00000002`
   (materializes `0x12008B`: covers the client mask including SYNCHRONIZE, still NO
   FILE_CREATE_PIPE_INSTANCE/append/generic authority — negative property re-proven
   in lab). Unit test updated to pin the encoding.
   Additionally: the service exe imports `libomp140.aarch64.dll` (clang-cl ARM64
   OpenMP) — `STATUS_DLL_NOT_FOUND (0xC0000135)` without it; now shipped beside the
   service exe as an MSI File component.

## Evidence artifacts (VM)

`C:\AetherCore-P36\logs\tranche1-before-state.txt`,
`tranche1-workspace-verify.txt`, `tranche1-cargo-release.log`,
`tranche1-tauri-build.log`, `tranche1-wix-build3.log`,
`tranche1-wix-validate3.log`, `tranche1-msi-final-install.log`,
`tranche1-msi-final-uninstall2.log`, `tranche1-ipc-test2.log`;
`C:\AetherCore-P36\evidence\tranche1-{install-verify,service-registration,acl-evidence,ipc-pipe-security-attempt,install-verify,userprobe2-*,final-std-probe}.json/.txt`,
`C:\AetherCore-P36\incoming\payload\payload-manifest.json`,
`tranche1-msi-evidence.json`, `tranche1-install-verify.json`.

## Files changed by this session (SECURITY-SENSITIVE — Codex review required before release packaging)

1. `installer/wix/Product.wxs` (launch condition + OpenMP DLL component)
2. `apps/install-hardener/src/main.rs` (run_icacls /T defect)
3. `crates/ipc/src/windows_impl.rs` (pipe SD encoding; unit test updated)
4. `crates/ipc/examples/ipc_probe.rs` (NEW diagnostic example)
All synced Mac⇄VM with SHA-256 verification at each step.

## Tranche 1 boundary

DONE: real MSI → install → service → Service SID → native ACL → named pipe →
StandardUser denial / Admin behavior.
NOT DONE (per tranche authorization): repair/uninstall/upgrade cycles,
rollback/recovery fault injection, physical x64 qualification, driver
qualification, Phase 37. `P36-PRE-NATIVE-MUTATION` was NOT restored.

## FAILURES (cont.)

- The macOS ZIP utility's format rejection was explained by the archive's
  verified GZip/TAR signature; it is not a digest mismatch.
- `release/dependency-freeze.json`, `release/dependency-locks.sha256`, and
  `release/dependency-manifests.sha256` are absent from the sealed P35 archive.
  The checked-in release bootstrap therefore cannot be run as-is. This is a
  release-packaging blocker, not an archive digest failure; P36 non-release
  smoke may use the sealed lockfile with the bounded `--trust-lockfile` mode.
- The Windows Update and IPC source-qualification blockers were resolved with
  minimal API-compatibility fixes and fresh ARM64 compile/test evidence. They
  are no longer open blockers; workspace-wide compilation is still pending.
- WiX MSI validation is blocked by existing ICE38/ICE43/ICE57 component
  authoring findings. The synthetic compile proves the pinned WiX toolchain can
  parse and link the source, but it is not installable evidence and must not be
  promoted to release packaging.

## CODE FIXES

- `crates/windows-update/src/execution_windows.rs`: target Windows-rs
  generated callback `*_Impl` types and consume primitive `HResult()` values.
- `crates/ipc/src/windows_impl.rs`: use `Error::from_thread()` for the
  `CreateNamedPipeW` failure path, type the pending response map explicitly,
  consume worker join handles by value, and make the byte-budget test use a
  non-empty frame.
- `crates/ipc/tests/unix_adversarial.rs`: gate Unix-only integration tests with
  `#![cfg(unix)]` so ARM64 Windows test compilation is portable.

## ARCHITECTURAL DECISION REQUIRED

- None at this checkpoint.

## NEXT ACTION

SUPERSEDED (the MSI/IPC source blockers named below were resolved and the
install was performed under tranche-1 authorization; see INSTALL_STATE above).

Current next action: close the open ServiceJob IPC defect — every `aetherctl`
ServiceJob verb hangs on Windows while OfflineJob verbs return. Retain
`P36-CLEAN-BASELINE` as the rollback boundary; tranche-2 destructive work
(repair/uninstall/upgrade cycles, rollback fault injection) stays unauthorized.
Keep all bridge output context-labeled as `SYSTEM`; StandardUser and
Administrator results must come from the established one-shot Scheduled Task
probes, never from the SYSTEM bridge.

Post-seal drift is now fully classified in `docs/phase36/DRIFT_LEDGER.md`
(91 files, 0 UNKNOWN). The six diagnostic IPC probes were moved out of product
source to `tools/p36-probes/`.
