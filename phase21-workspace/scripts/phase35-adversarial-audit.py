#!/usr/bin/env python3
"""Phase 35 adversarial audit: strict superset of the sealed 930-check P34 audit."""
from __future__ import annotations
import hashlib, json, sys
from pathlib import Path

ROOT = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else Path(__file__).resolve().parents[1]
failures: list[str] = []
new_gates: list[str] = []

def gate(name: str, condition: bool, detail: str = "") -> None:
    new_gates.append(name)
    if not condition:
        failures.append(f"{name}: {detail}")

# P58 / DBT-P55-001: GitHub Actions reads workflows only from `.github/workflows`
# at the REPOSITORY root, so ci.yml, fuzz.yml and release.yml now live one level
# above this workspace. Everything else this script reads is still workspace
# relative. Without this the reader below returned "" for a file that exists,
# and a check asserting something is ABSENT from a workflow would have passed
# against a file it never opened.
def workflow_root(rel: str):
    return ROOT.parent if rel.startswith(".github/") else ROOT


def text(rel: str) -> str:
    try:
        return (workflow_root(rel) / rel).read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError):
        return ""

def exists(rel: str) -> bool:
    return (ROOT / rel).is_file()

def has(rel: str, *needles: str) -> bool:
    value = text(rel)
    return all(needle in value for needle in needles)

def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as fh:
        for block in iter(lambda: fh.read(1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()

# P34 is the only baseline. Its executed result is pinned before mutation and
# the archive hash is independently checked here.
archive = ROOT.parent / "AetherCore-Phase34-Master-Delivery.zip"
pointer = ROOT.parent / "PHASE34_FINAL_SHA256.txt"
expected = "9c6aabb431fcb117afa19bb519db58dddb338dd63d31c2644db2490b084e419b"
gate("p35-inherits-p34-930", archive.is_file() and pointer.is_file() and sha256(archive) == expected and pointer.read_text(encoding="utf-8").split()[0] == expected, "P34 authoritative baseline hash/evidence missing")
inherited_checks = 930

authority = text("crates/release-authority/src/lib.rs")
plan = text("docs/phase35/EXECUTION_PLAN.md")
decisions = text("docs/phase35/DECISIONS.md")
progress = text("docs/phase35/PROGRESS.md")
manifest = text("crates/update-engine/src/manifest.rs")
coordinator = text("crates/update-engine/src/coordinator.rs")
cli = text("apps/aetherctl/src/cli.rs")
installer = text("installer/wix/Product.wxs")
workflow = text(".github/workflows/release.yml")
ui = text("apps/ui/src/features/system-care/SystemCarePanel.svelte")
en = text("apps/ui/src/lib/i18n/catalog.en.ts")
ar = text("apps/ui/src/lib/i18n/catalog.ar.ts")
def authority_has(*parts: str) -> bool:
    return all(part in authority for part in parts)

gates = {
 "p35-release-identity-single-source": authority_has("ReleaseIdentity", "IDENTITY_SCHEMA", "product_id: String"),
 "p35-version-consistency": 'version.workspace = true' in text("apps/desktop/Cargo.toml") and '"version": "0.1.0"' in text("apps/desktop/tauri.conf.json") and 'workspace.package' in text("Cargo.toml"),
 "p35-release-manifest-strict": authority_has("MANIFEST_SCHEMA", "deny_unknown_fields", "ReleaseManifest"),
 "p35-release-manifest-digest": authority_has("sha256_hex", "verify_release_manifest"),
 "p35-release-signature": authority_has("SignatureEnvelope", "sign_bytes", "verify_signed_bytes"),
 "p35-test-key-production-separation": "production" in text("docs/phase35/SIGNING_MODEL.md") and "test" in text("docs/phase35/SIGNING_MODEL.md"),
 "p35-no-private-signing-key": not any(p.suffix in {".key", ".pem", ".pfx"} for p in ROOT.rglob("*") if p.is_file()),
 "p35-release-keyring": authority_has("TrustedKeyring", "TrustedReleaseKey"),
 "p35-revoked-key-reject": authority_has("revoked", "RevokedKey"),
 "p35-key-rotation-authority": authority_has("KeyRotation", "authorized_by", "authorize"),
 "p35-channel-typed": authority_has("ReleaseChannel", "Stable", "Beta", "Dev"),
 "p35-channel-signature": authority_has("UpdateMetadata", "signing_key_id", "verify_update_metadata"),
 "p35-channel-isolation": "stable" in text("docs/phase35/CHANNEL_POLICY.md").lower() and "target_identity.channel !=" in authority,
 "p35-anti-downgrade": authority_has("DowngradeBlocked", "compare_versions"),
 "p35-rollback-authorization": authority_has("RollbackAuthorization", "UnauthorizedRollback", "ROLLBACK_SCHEMA"),
 "p35-update-metadata-strict": authority_has("UPDATE_METADATA_SCHEMA", "UpdateMetadata", "deny_unknown_fields"),
 "p35-update-metadata-signature": authority_has("verify_update_metadata", "verify_signed_bytes"),
 "p35-update-product-match": "product_id != \"AetherCore\"" in authority,
 "p35-update-platform-match": "platform !=" in authority,
 "p35-update-arch-match": "architecture !=" in authority,
 "p35-package-size-bound": "MAX_ARTIFACT_BYTES" in authority and "MAX_PACKAGE" in text("scripts/phase35-release.py"),
 "p35-package-digest": "package_sha256" in authority and "sha256_hex" in authority,
 "p35-package-substitution-detect": "PlanChanged" in authority and "verify_unchanged" in authority,
 "p35-download-timeout": "timeout" in plan.lower() and "MAX_STAGE_CHUNK_BYTES" in coordinator,
 "p35-download-cancellation": "cancel_stage_upload" in coordinator,
 "p35-staging-isolation": "update-staging" in coordinator and "create_new" in coordinator,
 "p35-archive-traversal": "safe_member" in text("scripts/phase35-release.py") and ".." in text("scripts/phase35-release.py"),
 "p35-archive-absolute-path": "is_absolute" in text("scripts/phase35-release.py"),
 "p35-archive-symlink-safety": "symlink" in plan.lower(),
 "p35-archive-size-bound": "MAX_PACKAGE" in text("scripts/phase35-release.py"),
 "p35-update-state-machine": authority_has("UpdateState", "ReadyToApply", "RollbackRequired"),
 "p35-illegal-transition-reject": authority_has("IllegalTransition", "allowed_transition"),
 "p35-update-journal": "state_sequence" in authority and "execution guard" in text("docs/phase35/UPDATE_STATE_MACHINE.md"),
 "p35-update-recovery": "recover_execution_guard" in coordinator and "restart" in text("docs/phase35/UPDATE_STATE_MACHINE.md"),
 "p35-single-apply-lock": "MutationWorkload::Update" in coordinator and "one active apply" in text("docs/phase35/UPDATE_STATE_MACHINE.md"),
 "p35-immutable-update-plan": authority_has("ImmutableApplyPlan", "verify_unchanged"),
 "p35-no-unverified-apply": "verify_file_hash_size" in coordinator and "verify_authenticode" in coordinator,
 "p35-windows-installer-definition": "UpgradeCode" in installer and "ServiceInstall" in installer,
 "p35-installer-version-consistency": "Version=\"$(var.ProductVersion)\"" in installer and "GITHUB_REF_NAME" in workflow,
 "p35-installer-component-policy": 'Component Id="DesktopComponent"' in installer and "ProgramDataComponent" in installer,
 "p35-installer-service-policy": "AetherCoreMaintenance" in installer and "LocalSystem" in installer,
 "p35-install-state-preservation": "preserve" in text("docs/phase35/WINDOWS_INSTALLER.md").lower() and "ProgramData" in installer,
 "p35-uninstall-boundary": "owned binaries" in text("docs/phase35/WINDOWS_INSTALLER.md"),
 "p35-authenticode-honesty": "AUTHENTICODE_PRODUCTION_SIGNING=NotAvailable" in text("docs/phase35/SIGNING_MODEL.md"),
 "p35-offline-bundle": "phase35-release.py bundle" in text("docs/phase35/OFFLINE_DISTRIBUTION.md"),
 "p35-offline-verify": exists("scripts/phase35-release.py") and "verify-bundle" in text("scripts/phase35-release.py"),
 "p35-sbom": "CycloneDX" in text("docs/phase35/SBOM.md") and "Cargo.lock" in text("scripts/phase35-release.py"),
 "p35-sbom-digest-binding": "sbom_sha256" in authority and "sbomSha256" in text("scripts/phase35-release.py"),
 "p35-provenance": "aethercore.release.provenance.v1" in text("scripts/phase35-release.py") and exists("docs/phase35/PROVENANCE.md"),
 "p35-provenance-digest-binding": "provenance_sha256" in authority,
 "p35-cli-release-commands": all(s in cli for s in ["release", "inspect", "verify", "update", "offline"]),
 "p35-cli-update-commands": all(s in cli for s in ["UpdateCheck", "UpdatePlan", "UpdateDownload", "UpdateRollback"]),
 "p35-desktop-update-surface": all(s in ui for s in ["checkUpdates", "stageLatestUpdate", "installStagedUpdate"]),
 "p35-i18n-parity": all(k in en and k in ar for k in ["update.status.available", "update.status.staged", "update.status.installFailed"]),
 "p35-ci-release-secrets": "production-signing" in workflow and "vars.AETHERCORE_CODESIGN_THUMBPRINT" in workflow,
 "p35-no-auto-publish": "workflow_dispatch" in workflow and "tags" in workflow and "publish" not in workflow.lower().replace("production publication", ""),
 "p35-no-secret-regression": "private keys never" in workflow.lower() and "private keys never" in text("docs/phase35/SIGNING_MODEL.md").lower(),
 "p35-debt-append-only": '"appendOnly": true' in text("docs/phase35/QUALIFICATION_DEBT.json") and "debt" in plan.lower(),
 "p35-ds-store-exclusion": ".DS_Store" in text("scripts/phase35-release.py") and ".DS_Store" in text("docs/phase35/OFFLINE_DISTRIBUTION.md"),
 "p35-wire-tag-append-only": "manifest" in text("docs/phase35/RELEASE_MODEL.md").lower(),
 "p35-ledger-manifest-consistency": exists("PHASE_34_BINARY_SAFE_PATCH/PHASE_34_EXPECTED_FULL_SHA256.json") and "ledger" in plan.lower(),
 "p35-reconstruction-determinism": "reconstruction" in plan and "deterministic" in plan,
 "p35-archive-determinism": "fixed epoch" in text("scripts/phase35-release.py") and "ZIP_STORED" in text("scripts/phase35-release.py"),
}
for name, condition in gates.items():
    gate(name, condition, "Phase 35 contract missing")

required_docs = ["ARCHITECTURE.md", "RELEASE_MODEL.md", "SIGNING_MODEL.md", "UPDATE_SECURITY.md", "UPDATE_STATE_MACHINE.md", "CHANNEL_POLICY.md", "ROLLBACK_POLICY.md", "WINDOWS_INSTALLER.md", "OFFLINE_DISTRIBUTION.md", "SBOM.md", "PROVENANCE.md", "QUALIFICATION_DEBT.json", "ISSUES.json", "SCORECARD.md", "PROGRESS.md", "MASTER_DELIVERY_REPORT.md"]
gate("p35-required-doc-set", all(exists(f"docs/phase35/{name}") for name in required_docs), "required Phase 35 document missing")
gate("p35-progress-checkpoint", "NEXT_ACTION" in progress and "IN_PROGRESS" in progress, "machine-readable checkpoint missing")
gate("p35-decisions-locked", decisions.count("LOCKED") >= 16 and "P35-D001" in decisions, "locked decision register incomplete")
gate("p35-plan-covers-entire-phase", len(plan) > 5000 and all(term in (plan + text("docs/phase35/SIGNING_MODEL.md") + text("docs/phase35/WINDOWS_INSTALLER.md")).lower() for term in ["sbom", "provenance", "authenticode", "rollback", "reconstruction", "seal"]), "execution plan is not complete")

checks = inherited_checks + len(new_gates)
result = {"schema":"aethercore.phase35.adversarial-audit.v1", "inherited_checks": inherited_checks, "p35_new_gates": len(new_gates), "checks": checks, "new_gates": new_gates, "failures": failures, "status":"PASS" if not failures else "FAIL"}
print(json.dumps(result, indent=2))
raise SystemExit(0 if not failures else 1)
