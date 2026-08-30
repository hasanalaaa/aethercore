# Phase 36 Progress

STATUS: EXECUTING
CURRENT_WORKSTREAM: P36-04 native qualification gates and installer smoke validation
EXECUTION_CONTEXT: Host macOS -> Parallels `Windows 11` via `prlctl exec`; bridge context is currently `SYSTEM`
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
  AetherCore service is correctly reported `NOT_INSTALLED` because MSI install
  is deferred. Evidence: `C:\\AetherCore-P36\\evidence\\native-queries.json`.
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

- Reconcile the workspace Cargo gate after the Windows Update and IPC source
  corrections: `cargo check --workspace --locked` is the next compile gate.
- Complete non-destructive crate/test and installer-source validation where
  the corrected native crates are included.
- Use the verified `P36-CLEAN-BASELINE` as the rollback boundary for any later
  installation, service, ACL, UAC, update, or rollback mutation.
- Record explicit context labels; the current bridge evidence is SYSTEM only
  and is not Administrator/StandardUser/UAC evidence.

## NOT_STARTED

- Runtime MSI installation, service registration/SID/ACL inspection, and
  named-pipe broker/service exercise (installer ICE validation remains open).
- Desktop launch under a normal interactive user, update staging/apply/rollback,
  and destructive driver/repair loops.
- Controlled Administrator/StandardUser UAC and user-AppData tests.
- Physical x64 Windows PC handoff and final P36 boundary review.

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

## FAILURES

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

Continue with targeted non-destructive tests and source review. Retain
`P36-CLEAN-BASELINE` as the rollback boundary; do not install or mutate until
the MSI/IPC source blockers are resolved in a separately authorized change.
Keep all bridge output context-labeled as `SYSTEM` until explicit Administrator
and StandardUser sessions are available.
