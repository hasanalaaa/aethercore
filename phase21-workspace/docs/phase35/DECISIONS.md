# Phase 35 Locked Decisions

These decisions are architectural authority. A handoff executor must record `ARCHITECTURAL_DECISION_REQUIRED` rather than reinterpret a locked item.

| ID | Decision | Rationale / consequence | Status |
|---|---|---|---|
| P35-D001 | Canonical release identity is `crates/release-authority` (`ReleaseIdentity`), with workspace version as the source value and generated/verified projections for Tauri, installer, manifest, CLI and filenames. | Prevents conflicting version surfaces; runtime does not require Git. | LOCKED |
| P35-D002 | Release manifest schema is strict `aethercore.release.manifest.v1`, deterministic struct-ordered JSON; digest/signature cover exact canonical bytes without the signature field. | Removes self-reference ambiguity and rejects unknown fields. | LOCKED |
| P35-D003 | Metadata signing primitive is the existing `ed25519-dalek` Ed25519 implementation already used by P29/P33; no second crypto implementation. | Reuse reviewed primitive and verification behavior. | LOCKED |
| P35-D004 | Production private signing keys remain outside source, artifacts, logs, CLI, IPC and UI; production signing requires an explicit external key reference. Test seeds are deterministic and clearly test-only. | Prevents credential disclosure and fake production PASS. | LOCKED |
| P35-D005 | Trusted release keyring is typed by key ID, algorithm, public key, enabled/revoked status and validity metadata; unknown or revoked keys reject. | Deterministic key selection and revocation. | LOCKED |
| P35-D006 | Key rotation is authorized by an already-trusted key and cannot replace the trust root merely because a new release requests it. | Prevents trust-root substitution. | LOCKED |
| P35-D007 | Channels are typed `stable`, `beta`, `dev`; stable never silently consumes beta/dev; channel changes require explicit intent and signed metadata. | Prevents cross-channel confusion/downgrade. | LOCKED |
| P35-D008 | Anti-downgrade compares strict canonical semver and per-channel sequence floors; ordinary update cannot target an older/equal version. | Prevents replay and version ambiguity. | LOCKED |
| P35-D009 | Rollback is a distinct signed `aethercore.release.rollback-authorization.v1` binding current/target versions, channel, reason, expiry and signer. Generic force flags cannot bypass it. | Emergency recovery remains explicitly authorized. | LOCKED |
| P35-D010 | Verification order is metadata signature/keyring → identity/channel/platform/arch → package length/hash → release manifest/signature → SBOM/provenance binding → immutable plan. | No unverified bytes reach apply. | LOCKED |
| P35-D011 | Transaction states are typed and persisted: Discovered, Validated, Downloading, Downloaded, Verified, Staged, ReadyToApply, Applying, Applied, RebootRequired, RollbackRequired, RollingBack, RolledBack, Failed, Cancelled. | Illegal transitions reject and restart recovery is deterministic. | LOCKED |
| P35-D012 | Apply authority is the existing mutation supervisor/elevated broker; UI/CLI may request typed plans but never construct arbitrary installer commands. | Preserves consent, lease and IPC boundaries. | LOCKED |
| P35-D013 | Recovery retains journal and verified rollback artifact; interruption reconstructs a non-actionable state until revalidation, and rollback requires signed authorization and digest verification. | Prevents arbitrary or stale binary rollback. | LOCKED |
| P35-D014 | Installer architecture is WiX v4 MSI/Burn through the existing Tauri/WiX path, with stable UpgradeCode/component GUID policy and ProgramData preservation. | Matches current service/package architecture. | LOCKED |
| P35-D015 | Install/upgrade/repair/uninstall preserve user state by default; uninstall removes only AetherCore-owned binaries/transient files and does not delete fleet trust/data absent explicit data-removal mode. | Avoids destructive lifecycle behavior. | LOCKED |
| P35-D016 | Windows qualification boundary: macOS may prove source/config/composition and deterministic metadata only; MSI runtime, UAC, service ACLs, Authenticode and reboot recovery are P36 qualification debt. | Honest provenance outranks convenience. | LOCKED |
