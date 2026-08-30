# Phase 36 Locked Decisions

These decisions are the authority for the Windows qualification session. A
result that cannot be reconciled with them is recorded as
`ARCHITECTURAL_DECISION_REQUIRED`; it is not silently reinterpreted.

| ID | Decision | Consequence | Status |
|---|---|---|---|
| P36-D001 | The sealed P35 archive SHA and external pointer are the sole input authority. | No live-tree drift or alternate archive may enter the Windows build. | LOCKED |
| P36-D002 | The `.zip`-named P35 bytes are consumed by signature-aware GZip/TAR extraction after SHA verification. | Do not recompress, rename, or use a ZIP-only extractor that ignores the bytes. | LOCKED |
| P36-D003 | The Parallels VM is a Windows-native ARM64 qualification target when its OS reports ARM64. | Record `VM_WINDOWS_NATIVE_ARM64=True`; never label it native x86_64. | LOCKED |
| P36-D004 | Physical x86_64 Windows qualification remains mandatory for full P36 closure. | Record `PHYSICAL_X64_WINDOWS_QUALIFICATION_STILL_REQUIRED=True`; reserve hardware gates for a real PC. | LOCKED |
| P36-D005 | The Windows qualification root is `C:\\AetherCore-P36` on local NTFS. | Shared folders are transport only; all builds, installers, locks, services, ACLs, pipes, and state use local NTFS. | LOCKED |
| P36-D006 | WiX v4 `installer/wix/Product.wxs` plus the existing Tauri bundling path is installer authority. | UpgradeCode, component GUIDs, service definition, ProgramData policy, and custom-action boundaries are inspected from that source. | LOCKED |
| P36-D007 | Service registration and hardening authority is the authored WiX service plus `aethercore-install-hardener`; the maintenance service runs as LocalSystem with its authored Service SID/ACL policy. | Runtime evidence must inspect the registered service, SID type, binary path, and ACLs; a source-only claim is not a runtime pass. | LOCKED |
| P36-D008 | Named-pipe IPC authority is the Windows implementation in `crates/ipc`, exercised through the typed broker/service boundary. | Pipe identity, impersonation level, ACL, framing, and denial behavior are tested without inventing alternate IPC. | LOCKED |
| P36-D009 | `prlctl exec` SYSTEM evidence is limited to bootstrap and machine-level inspection. | UAC prompts, standard-user denial, desktop normal-user launch, and user-AppData semantics require explicit Administrator/StandardUser contexts. | LOCKED |
| P36-D010 | Update authority remains the typed release/update contracts and existing mutation broker/lease boundary. | Windows tests may stage and inspect safely; apply, rollback, and authority failures require their defined context and restore point. | LOCKED |
| P36-D011 | Rollback/recovery authority is the P35 signed rollback contract plus the clean Parallels restore point. | No destructive failure injection or rollback claim is valid without a usable `P36-CLEAN-BASELINE` snapshot and evidence. | LOCKED |
| P36-D012 | Defender, UAC, Firewall, SmartScreen, and standard Windows security posture remain enabled. | Green results cannot be purchased by disabling protections; blockers are recorded honestly. | LOCKED |
| P36-D013 | Evidence is sanitized, append-only by workstream, and context-labeled. | Retain reproducibility-relevant hashes/versions/SIDs/ACLs; redact identity, secrets, certificates, and full environment dumps. | LOCKED |
| P36-D014 | Non-destructive gates precede destructive lifecycle/corruption/driver testing. | Build, metadata, registration inspection, IPC, staging, and query paths may proceed; destructive loops wait for the snapshot gate. | LOCKED |
| P36-D015 | P36 is not sealed by ARM VM results alone. | Remaining physical x64/hardware/driver gates stay open and must receive a later PC + Hermes handoff. | LOCKED |
| P36-D016 | Phase 37 is outside this session. | No Phase 37 implementation, seal, or inferred closure is permitted. | LOCKED |
| P36-D017 | The sealed P35 archive does not contain the approved `release/dependency-freeze.json`, `dependency-locks.sha256`, or `dependency-manifests.sha256` inputs required by the release bootstrap script. | For non-release smoke only, `pnpm install --frozen-lockfile --trust-lockfile` may use the sealed lockfile while preserving package-integrity checks; dependency-freeze regeneration and release packaging remain blocked until a trusted freeze workstation supplies those files. | LOCKED |
| P36-D018 | A WiX compile using explicitly synthetic smoke payloads is source/schema evidence only; MSI ICE validation remains an acceptance gate. | Preserve the compile artifact only as non-release evidence, record ICE38/ICE43/ICE57, and do not install or promote the MSI until the authored per-user/per-machine component semantics are corrected and revalidated. | LOCKED |
| P36-D019 | A fresh ARM64 red gate proved Windows-rs API drift in the Windows Update and IPC sources; only compatibility corrections that preserve existing error, trust, framing, and mutation semantics are authorized in this session. | The four generated callback `*_Impl` targets, primitive HRESULT handling, `Error::from_thread()` last-error path, explicit pending-map type, owned worker joins, and Unix test platform gate may be changed. No protocol, ACL, service, or update-policy redesign is implied. | LOCKED |
