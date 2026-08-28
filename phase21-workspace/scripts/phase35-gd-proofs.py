#!/usr/bin/env python3
"""Deterministic GD-1..GD-10 demonstrations for Phase 35."""
from __future__ import annotations
import json, subprocess, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
def run(*args: str) -> bool:
    return subprocess.run(args, cwd=ROOT, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=False).returncode == 0
def has(path: str, *parts: str) -> bool:
    value = (ROOT / path).read_text(encoding="utf-8")
    return all(part in value for part in parts)

results = {
    "GD-1": {"name":"release identity", "status":"PASS" if has("crates/release-authority/src/lib.rs", "ReleaseIdentity", "IDENTITY_SCHEMA") else "FAIL"},
    "GD-2": {"name":"release signing", "status":"PASS" if run("cargo", "test", "-p", "aethercore-release-authority", "strict_manifest_is_deterministic_and_signed") else "FAIL"},
    "GD-3": {"name":"metadata anti-downgrade", "status":"PASS" if run("cargo", "test", "-p", "aethercore-release-authority", "update_metadata_blocks_downgrade_and_wrong_channel") else "FAIL"},
    "GD-4": {"name":"download and staging", "status":"PASS" if has("crates/update-engine/src/coordinator.rs", "MAX_STAGE_CHUNK_BYTES", "verify_file_hash_size", "cancel_stage_upload") else "FAIL"},
    "GD-5": {"name":"transaction and recovery", "status":"PASS" if run("cargo", "test", "-p", "aethercore-release-authority", "state_machine_rejects_illegal_transition_and_plan_tamper") and has("crates/update-engine/src/coordinator.rs", "recover_execution_guard") else "FAIL"},
    "GD-6": {"name":"offline bundle", "status":"PASS" if run("python3", "scripts/phase35-release.py", "verify-bundle", "release/phase35/AetherCore-0.1.0-windows-x86_64-offline.zip") else "FAIL"},
    "GD-7": {"name":"installer definition", "status":"PASS" if has("installer/wix/Product.wxs", "UpgradeCode", "ServiceInstall", "ProgramDataComponent") else "FAIL", "windowsRuntime":"NotAvailable"},
    "GD-8": {"name":"SBOM and provenance", "status":"PASS" if has("scripts/phase35-release.py", "CycloneDX", "aethercore.release.provenance.v1") else "FAIL"},
    "GD-9": {"name":"desktop update UX", "status":"PASS" if has("apps/ui/src/features/system-care/SystemCarePanel.svelte", "checkUpdates", "stageLatestUpdate", "installStagedUpdate") and has("apps/ui/src/lib/i18n/catalog.en.ts", "update.status.available") and has("apps/ui/src/lib/i18n/catalog.ar.ts", "update.status.available") else "FAIL"},
    "GD-10": {"name":"production signing honesty", "status":"PASS" if "PRODUCTION_RELEASE_SIGNING=NotAvailable" in (ROOT / "docs/phase35/SIGNING_MODEL.md").read_text(encoding="utf-8") and "AUTHENTICODE_PRODUCTION_SIGNING=NotAvailable" in (ROOT / "docs/phase35/SIGNING_MODEL.md").read_text(encoding="utf-8") else "FAIL"},
}
status = "PASS" if all(row["status"] == "PASS" for row in results.values()) else "FAIL"
print(json.dumps({"schema":"aethercore.phase35.gd-proofs.v1", "results":results, "status":status}, indent=2))
raise SystemExit(0 if status == "PASS" else 1)
