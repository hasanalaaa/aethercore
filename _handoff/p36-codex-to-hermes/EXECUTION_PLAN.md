# Phase 36 Execution Plan

Phase 36 is the Windows-native qualification session for the sealed Phase 35
delivery. The execution host is macOS, the execution target is the running
`Windows 11` Parallels VM, and the bridge is `prlctl exec`. The bridge currently
runs as `NT AUTHORITY\\SYSTEM`; that context is valid for bootstrap and
machine-level inspection, but it is not evidence for standard-user, UAC, or
normal desktop-user behavior.

## Authority and non-goals

- The sealed input is `AetherCore-Phase35-Master-Delivery.zip` with SHA-256
  `a6d8ac72c9c2f5321fd8f29e4082a908468134af98584108c53cf647e514f46e`.
- The external pointer `PHASE35_FINAL_SHA256.txt` and the archive digest are
  the pre-mutation P35 authority. Live source is used only to identify commands
  and architecture; the Windows build tree comes from the verified archive.
- The filename ends in `.zip`, but the verified bytes are a deterministic
  GZip-compressed TAR (the same archive convention used by the checked-in P34
  builder). Windows extraction must use signature-aware `tar -xzf`/equivalent,
  never a renamed or recompressed copy.
- Parallels shared folders are transport only. Compilation, installation,
  service, ACL, IPC, state, and recovery tests run below `C:\\AetherCore-P36` on
  local NTFS.
- This session must not install Codex in Windows, copy the macOS development
  workspace, begin Phase 37, or claim full P36 closure while physical x64 gates
  remain.

## Acceptance criteria (pass/fail)

1. P35 precheck is fresh: archive SHA and external pointer match exactly, the
   read-only P35 adversarial audit reports `998 checks / 0 failures / PASS`, and
   no unexplained drift is present.
2. Windows evidence records edition, version, build, architecture, PowerShell,
   and the execution context. `VM_WINDOWS_NATIVE_ARM64=True` is recorded only
   if the VM reports ARM64; `PHYSICAL_X64_WINDOWS_QUALIFICATION_STILL_REQUIRED=True`
   is always recorded for this VM session.
3. `C:\\AetherCore-P36\\incoming`, `workspace`, `evidence`, and `logs` exist on
   local NTFS. Only the sealed P35 archive (and optional pointer) is transported.
4. Windows-native `Get-FileHash` equals the authoritative P35 SHA. A mismatch
   blocks extraction and all execution.
5. The verified archive is extracted to local NTFS and the extracted root is
   proven to contain the expected P35 manifest and source files without
   `target`, `node_modules`, caches, private keys, or shared-folder build paths.
6. Exact toolchain evidence is captured: `rustc -vV`, Cargo, Node, pnpm,
   MSVC/Windows SDK, PowerShell, and OpenSSH state. Evidence is sanitized and
   contains no secrets or user-identifying values.
7. Safe gates pass or are recorded with a concrete failure: repository read,
   Cargo metadata, Windows-target compilation/no-run, UI/config validation,
   and WiX/Tauri source validation. Destructive tests do not run before the
   snapshot gate.
8. A usable Parallels snapshot named `P36-CLEAN-BASELINE` exists before any
   installer mutation, ACL corruption, driver, database-corruption, or rollback
   failure-injection test.
9. Non-destructive native qualification produces evidence for installer build,
   service definition/registration, Service SID, ACLs, named-pipe IPC, desktop
   launch, update staging/authority, DB/state persistence, DISM/SFC/CBS query
   paths, Windows Update query paths, and OpenSSH detection.
10. Every security-sensitive result is labeled `SYSTEM`, `Administrator`, or
    `StandardUser`. SYSTEM results never satisfy UAC, standard-user denial,
    desktop launch, or user-AppData criteria.
11. The final checkpoint contains a physical x64 Windows PC handoff for native
    x86_64, hardware/driver/GPU telemetry, and final driver rollback smoke
    tests. Phase 36 remains open until those gates are reviewed; Phase 37 does
    not start in this session.

## Execution order and stop conditions

### P36-00 — baseline and controls

Verify the P35 digest, pointer, and read-only audit. Create this plan, the
locked decision register, and the progress checkpoint. Any digest mismatch or
unexplained drift is a hard stop.

### P36-01 — Windows VM and local workspace

Use `prlctl exec "Windows 11"` to record OS facts and create the four local
directories. Locate the Parallels-visible host archive only for transport. Do
not use the shared path as a working directory.

### P36-02 — transport, verify, and extract

Copy only the sealed archive and optional pointer into `incoming`. Run
PowerShell `Get-FileHash -Algorithm SHA256`; stop on any mismatch. Extract the
verified GZip/TAR payload into `workspace` with Windows-native tooling, then
record the extracted root and manifest checks.

### P36-03 — toolchain and safe smoke gates

Inspect the repository requirements before installing anything. Prefer the
checked-in `scripts/bootstrap.ps1 -InstallPrerequisites` and official winget
IDs. Preserve Defender, UAC, Firewall, and SmartScreen. Generate Windows-native
dependency caches; never copy macOS `target` or `node_modules`. Run only
read-only metadata, compile/no-run, UI, Tauri, and WiX validation gates.

### P36-04 — clean snapshot gate

Create or verify the Parallels snapshot `P36-CLEAN-BASELINE`. If CLI snapshot
creation is not safe or available, stop and request only the minimal manual
Parallels action. No destructive qualification starts without a restore point.

### P36-05 — non-destructive native qualification

Run the prioritized gates in the acceptance criteria. Preserve raw command
output in `C:\\AetherCore-P36\\logs` and sanitized summaries in `evidence`.
Record the execution context beside every result. Destructive lifecycle,
corruption, driver, and rollback-injection work is a later controlled stage
after snapshot verification and is not inferred from smoke success.

### P36-06 — handoff and boundary review

Update `PROGRESS.md` after each workstream with exact results, failures, fixes,
decisions, and the next action. Leave explicit physical-x64 instructions and
the remaining P36 debt. Do not seal P36 or begin P37.

## Evidence and redaction

Evidence is append-only per workstream and names the VM, architecture, command,
timestamp, context, exit code, and result. Redact usernames, home paths,
machine names, access tokens, certificates/private-key material, and full
environment dumps. Hashes, product paths, service names, SIDs, ACL principals,
and build/tool versions are retained when needed to reproduce a gate.

## Current checkpoint

See `docs/phase36/PROGRESS.md`. The next action is always the exact command or
minimal human gate recorded there; a usage-limit interruption must resume from
that file without restarting the session.
