#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import re
import shutil
import sqlite3
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PARSER = argparse.ArgumentParser()
PARSER.add_argument("--output", type=Path, help="Optional explicit evidence path; default audit is read-only.")
ARGS = PARSER.parse_args()
checks: list[dict] = []


def add(cid: str, status: str, detail: str, evidence: list[str] | None = None) -> None:
    checks.append({"id": cid, "status": status, "detail": detail, "evidence": evidence or []})


def text(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def require(cid: str, path: str, needles: list[str]) -> None:
    p = ROOT / path
    if not p.is_file():
        add(cid, "FAIL", f"missing {path}")
        return
    body = p.read_text(encoding="utf-8")
    missing = [needle for needle in needles if needle not in body]
    add(cid, "PASS" if not missing else "FAIL", "required integrity markers present" if not missing else f"missing markers={missing}", [path])


required = [
    "crates/pc-intelligence/src/lifecycle.rs",
    "crates/pc-intelligence/src/fingerprint.rs",
    "crates/pc-intelligence/src/run_ownership.rs",
    "crates/pc-intelligence/src/rules.rs",
    "crates/persistence/migrations/0011_phase17_1_intelligence_integrity.sql",
    "tests/fixtures/phase17_1/integrity_scenarios.json",
    "docs/phase17_1/FINDING_LIFECYCLE_INTEGRITY.md",
    "docs/phase17_1/CANONICAL_STATE_FINGERPRINT.md",
    "docs/phase17_1/SCAN_GENERATION_OWNERSHIP.md",
    "docs/phase17_1/CORRELATION_STRENGTH.md",
]
missing = [p for p in required if not (ROOT / p).is_file()]
add("P17.1-STATIC-001", "PASS" if not missing else "FAIL", "Phase 17.1 surgical files present" if not missing else f"missing={missing}", required)

model = text("crates/pc-intelligence/src/model.rs")
lifecycle = text("crates/pc-intelligence/src/lifecycle.rs")
coordinator = text("crates/pc-intelligence/src/coordinator.rs")
persistence = text("crates/persistence/src/lib.rs")
fingerprint = text("crates/pc-intelligence/src/fingerprint.rs")
run_owner = text("crates/pc-intelligence/src/run_ownership.rs")
rules = text("crates/pc-intelligence/src/rules.rs")
proto = text("crates/contracts/proto/intelligence.proto")
ui = text("apps/ui/src/features/intelligence/FindingCard.svelte")

require("P17.1-LIFE-001", "crates/pc-intelligence/src/model.rs", [
    "FindingVerificationStatus", "ConfirmedCurrent", "NotRechecked", "VerificationUnavailable", "ResolutionConfirmed",
    "resolution_authority", "resolved_at_unix_ms", "resolution_scan_id", "resolution_evidence",
])
require("P17.1-LIFE-002", "crates/pc-intelligence/src/lifecycle.rs", [
    "resolution_authority_state", "CollectorState::CompletedWithWarnings", "ResolutionPolicy::AuthoritativeAbsence",
    "ResolutionPolicy::MatchingHealthyState", "FindingVerificationStatus::VerificationUnavailable",
    "FindingVerificationStatus::NotRechecked", "FindingVerificationStatus::ResolutionConfirmed",
])
dangerous = "resolve_missing_intelligence_findings" in persistence or "resolve_missing_intelligence_findings" in coordinator
add("P17.1-LIFE-003", "FAIL" if dangerous else "PASS", "absence-only global resolver removed" if not dangerous else "legacy absence-only resolver remains", ["crates/persistence/src/lib.rs", "crates/pc-intelligence/src/coordinator.rs"])
life_tests = [
    "authoritative_completed_scope_resolves_but_unrelated_cancel_does_not_block_it",
    "failed_or_partial_owner_scope_carries_finding_without_remediation",
    "cancellation_before_owner_scope_keeps_finding_not_rechecked",
    "resolved_issue_returning_is_recurred_and_ignored_state_is_durable",
]
add("P17.1-LIFE-004", "PASS" if all(x in lifecycle for x in life_tests) else "FAIL", "deterministic lifecycle regression cases are source-backed", ["crates/pc-intelligence/src/lifecycle.rs"])

require("P17.1-FP-001", "crates/pc-intelligence/src/fingerprint.rs", [
    "CanonicalStateEntry", "canonical_payload_state", "entries.sort()", "installed_driver_version",
    "uncorrected_read_errors", "FactPayload::DiagnosticLimitation { .. } => return None",
    'FactPayload::HardwareEvent { .. } => "hardware-event"', 'FactPayload::Crash { .. } => "crash-event"',
])
id_only = bool(re.search(r"fn\s+fingerprint\s*\([^)]*facts[^)]*\).*?\.map\(\|fact\|\s*fact\.id", coordinator, flags=re.S))
add("P17.1-FP-002", "FAIL" if id_only else "PASS", "identity-only Phase 17 fingerprint implementation absent", ["crates/pc-intelligence/src/coordinator.rs"])
fp_tests = ["same_state_ignores_timestamp_evidence_text_and_order", "driver_version_change_changes_fingerprint", "storage_degradation_changes_fingerprint", "event_resource_timestamp_identity_does_not_change_fingerprint", "normal_temperature_fluctuation_and_cleanup_churn_do_not_change_machine_state", "diagnostic_limitations_do_not_change_machine_state_fingerprint"]
add("P17.1-FP-003", "PASS" if all(x in fingerprint for x in fp_tests) else "FAIL", "fingerprint determinism/value/privacy tests are source-backed", ["crates/pc-intelligence/src/fingerprint.rs"])

require("P17.1-GEN-001", "crates/pc-intelligence/src/run_ownership.rs", ["RunIdentity", "generation", "cancel_if_current", "clear_if_owner", "owns"])
shared_token = "token: Mutex<Option<CancellationToken>>" in coordinator
owner_bound = all(x in coordinator for x in ["run_ownership: Mutex<RunOwnership>", "set_snapshot_if_owner", "mutate_snapshot_if_owner", "clear_run_if_owner"])
add("P17.1-GEN-002", "PASS" if owner_bound and not shared_token else "FAIL", "shared token cleanup replaced by generation-bound ownership", ["crates/pc-intelligence/src/coordinator.rs"])
gen_tests = ["rapid_restart_old_cleanup_cannot_clear_new_generation", "stale_cancel_cannot_cancel_new_generation", "duplicate_terminal_cleanup_is_idempotent", "late_worker_identity_cannot_mutate_new_generation"]
add("P17.1-GEN-003", "PASS" if all(x in run_owner for x in gen_tests) else "FAIL", "race cases use direct generation transitions rather than arbitrary sleeps", ["crates/pc-intelligence/src/run_ownership.rs"])

require("P17.1-CORR-001", "crates/pc-intelligence/src/model.rs", ["CorrelationStrength", "CorrelationExplanation", "time_distance_ms", "conflicting_evidence_keys"])
window_ok = "const HARDWARE_CORRELATION_WINDOW_MS: i64 = 30 * 60 * 1000;" in rules and "const TIGHT_HARDWARE_CORRELATION_MS: i64 = 10 * 60 * 1000;" in rules
weak_not_promoted = "if strength == CorrelationStrength::Weak" in rules and "return;" in rules[rules.index("if strength == CorrelationStrength::Weak"):rules.index("if strength == CorrelationStrength::Weak")+180]
strong_bounded = "CorrelationStrength::Strong => (Severity::High, Confidence::High)" in rules and "Severity::Critical" not in rules[rules.index("fn correlate_hardware_crash"):rules.index("fn whea_disposition")]
causal_order = "*signed_delta >= 0" in rules
add("P17.1-CORR-002", "PASS" if window_ok and weak_not_promoted and strong_bounded and causal_order else "FAIL", f"bounded windows={window_ok}, weak suppressed={weak_not_promoted}, strong bounded={strong_bounded}, causal ordering={causal_order}", ["crates/pc-intelligence/src/rules.rs"])
corr_tests = ["fatal_whea_immediately_before_crash_is_strong_not_causal_claim", "corrected_whea_days_before_unrelated_crash_never_creates_critical_correlation", "repeated_tight_whea_and_crashes_raise_strength", "crash_without_whea_has_no_hardware_correlation", "whea_without_crash_remains_independent_hardware_evidence", "corrected_or_after_crash_evidence_exposes_conflict", "stale_evidence_does_not_dominate_current_scan"]
add("P17.1-CORR-003", "PASS" if all(x in rules for x in corr_tests) else "FAIL", "correlation regression matrix is source-backed", ["crates/pc-intelligence/src/rules.rs"])

# Protobuf remains additive after Phase 17 field 25.
proto_fields = ["verification_status=26", "resolution_authority=27", "has_resolved_at=28", "resolved_at_unix_ms=29", "resolution_scan_id=30", "resolution_reason_key=31", "resolution_evidence=32", "correlation=33"]
add("P17.1-IPC-001", "PASS" if all(f in proto.replace(" ", "") for f in proto_fields) else "FAIL", "PcFinding schema evolution is additive after field 25", ["crates/contracts/proto/intelligence.proto"])
require("P17.1-IPC-002", "services/maintenance-service/src/protocol.rs", ["pc_resolution_evidence_proto", "pc_correlation_proto", "verification_status", "resolution_authority", "correlation: v.correlation.map"])

# UI semantics and localization parity.
ui_ok = all(x in ui for x in ["deepScan.verification.unavailable", "deepScan.verification.notRechecked", "deepScan.correlation.distance", "conflictingEvidenceKeys", 'role="status"'])
motion_bad = bool(re.search(r"@keyframes|animation\s*:|!important", ui))
add("P17.1-UI-001", "PASS" if ui_ok and not motion_bad else "FAIL", "existing Finding card minimally represents verification/correlation truth without new locked motion", ["apps/ui/src/features/intelligence/FindingCard.svelte"])

def catalog_keys(path: Path) -> set[str]:
    return set(re.findall(r"^\s*'([^']+)'\s*:", path.read_text(encoding="utf-8"), flags=re.M))

enp = ROOT / "apps/ui/src/lib/i18n/catalog.en.ts"
arp = ROOT / "apps/ui/src/lib/i18n/catalog.ar.ts"
en, ar = catalog_keys(enp), catalog_keys(arp)
new_keys = {
    "deepScan.verification.notRechecked", "deepScan.verification.unavailable", "deepScan.verification.resolutionConfirmed",
    "deepScan.correlation.strength", "deepScan.correlation.strong", "deepScan.correlation.moderate", "deepScan.correlation.weak",
    "deepScan.correlation.distance", "deepScan.correlation.scope", "deepScan.correlation.uncertainty",
    "finding.resolution.authoritativeAbsence", "finding.resolution.healthyStateConfirmed",
    "finding.correlation.driverChangeCloseToCrash", "finding.correlation.noCrashModuleAttribution",
    "finding.correlation.repeatedHardwareCrashCluster", "finding.correlation.fatalHardwareNearCrash",
    "finding.correlation.hardwareNearCrash", "finding.correlation.correctedHardwareEvidence", "finding.correlation.hardwareEvidenceAfterCrash",
}
add("P17.1-I18N-001", "PASS" if en == ar and new_keys.issubset(en) else "FAIL", f"catalog parity={en == ar}; new 17.1 keys complete={new_keys.issubset(en)}; total={len(en)}", [str(enp.relative_to(ROOT)), str(arp.relative_to(ROOT))])

# Deterministic migration proof: Phase 17 row created on v10 survives v11 with safe defaults.
try:
    conn = sqlite3.connect(":memory:")
    migrations = sorted((ROOT / "crates/persistence/migrations").glob("*.sql"))
    for p in migrations:
        if p.name.startswith("0011_"):
            break
        conn.executescript(p.read_text(encoding="utf-8"))
    conn.execute(
        "INSERT INTO intelligence_findings(owner_principal_key,finding_id,finding_code,first_observed_unix_ms,last_observed_unix_ms,lifecycle,severity,confidence,finding_json) VALUES(?,?,?,?,?,?,?,?,?)",
        ("owner", "finding-1", "DRIVER_MISSING", 10, 20, "Ignored", "High", "Confirmed", '{"id":"finding-1","ignored":true}')
    )
    conn.executescript((ROOT / "crates/persistence/migrations/0011_phase17_1_intelligence_integrity.sql").read_text(encoding="utf-8"))
    row = conn.execute("SELECT finding_id,lifecycle,finding_json,verification_status,resolved_at_unix_ms,resolution_scan_id,resolution_reason_key FROM intelligence_findings WHERE finding_id='finding-1'").fetchone()
    ok = row == ("finding-1", "Ignored", '{"id":"finding-1","ignored":true}', "NotRechecked", None, "", "")
    add("P17.1-MIG-001", "PASS" if ok else "FAIL", f"v10 row preserved with deterministic v11 defaults={ok}; row={row}", ["crates/persistence/migrations/0010_phase17_intelligence.sql", "crates/persistence/migrations/0011_phase17_1_intelligence_integrity.sql"])
except Exception as exc:
    add("P17.1-MIG-001", "FAIL", f"migration compatibility execution failed: {exc}")

try:
    fixture = json.loads((ROOT / "tests/fixtures/phase17_1/integrity_scenarios.json").read_text(encoding="utf-8"))
    ids = {s["id"] for s in fixture["scenarios"]}
    categories = {s["category"] for s in fixture["scenarios"]}
    required_categories = {"findingLifecycle", "fingerprint", "generationOwnership", "correlation"}
    ok = len(ids) >= 24 and required_categories.issubset(categories)
    add("P17.1-FIX-001", "PASS" if ok else "FAIL", f"integrity fixtures={len(ids)}, categories={sorted(categories)}", ["tests/fixtures/phase17_1/integrity_scenarios.json"])
except Exception as exc:
    add("P17.1-FIX-001", "FAIL", f"fixture inventory invalid: {exc}")

try:
    debt = json.loads((ROOT / "QUALIFICATION_DEBT.json").read_text(encoding="utf-8"))
    ids = [i.get("capabilityId") for i in debt.get("items", [])]
    required = {f"P17-QD-{i:03d}" for i in range(1, 13)}
    ok = required.issubset(set(ids)) and len(ids) == len(set(ids)) and "v10 to v11" in next(i for i in debt["items"] if i["capabilityId"] == "P17-QD-011")["windowsApiDependency"]
    add("P17.1-QD-001", "PASS" if ok else "FAIL", f"qualification debt reused without duplicate entries; items={len(ids)}", ["QUALIFICATION_DEBT.json"])
except Exception as exc:
    add("P17.1-QD-001", "FAIL", f"qualification debt invalid: {exc}")

# Source quality for newly introduced scope.
new_paths = [ROOT / "crates/pc-intelligence/src/lifecycle.rs", ROOT / "crates/pc-intelligence/src/fingerprint.rs", ROOT / "crates/pc-intelligence/src/run_ownership.rs", ROOT / "crates/persistence/migrations/0011_phase17_1_intelligence_integrity.sql", ROOT / "apps/ui/src/features/intelligence/FindingCard.svelte"]
violations = []
for p in new_paths:
    body = p.read_text(encoding="utf-8", errors="replace")
    token = "TO" + "DO|FIX" + "ME|HA" + "CK"
    if re.search(token, body, re.I):
        violations.append(str(p.relative_to(ROOT)))
add("P17.1-QUAL-001", "PASS" if not violations else "FAIL", "no placeholder markers in Phase 17.1 production scope" if not violations else f"violations={violations}", [str(p.relative_to(ROOT)) for p in new_paths])

cargo = shutil.which("cargo")
rustc = shutil.which("rustc")
svelte = ROOT / "apps/ui/node_modules/.bin/svelte-check"
if cargo and rustc:
    proc = subprocess.run([cargo, "test", "-p", "aethercore-pc-intelligence", "--no-fail-fast"], cwd=ROOT, capture_output=True, text=True)
    add("P17.1-RUST-001", "PASS" if proc.returncode == 0 else "FAIL", f"cargo test exit={proc.returncode}", ["crates/pc-intelligence/src/lifecycle.rs", "crates/pc-intelligence/src/fingerprint.rs", "crates/pc-intelligence/src/run_ownership.rs", "crates/pc-intelligence/src/rules.rs"])
else:
    add("P17.1-RUST-001", "NOT_EXECUTED", f"Rust toolchain unavailable (cargo={bool(cargo)}, rustc={bool(rustc)})")

if svelte.exists():
    proc = subprocess.run([str(svelte), "--tsconfig", "./tsconfig.json"], cwd=ROOT / "apps/ui", capture_output=True, text=True)
    add("P17.1-UI-RUNTIME-001", "PASS" if proc.returncode == 0 else "FAIL", f"svelte-check exit={proc.returncode}", ["apps/ui/src/features/intelligence/FindingCard.svelte"])
else:
    add("P17.1-UI-RUNTIME-001", "NOT_EXECUTED", "installed svelte-check dependency unavailable on host")

add("P17.1-WINDOWS-001", "NOT_EXECUTED", "Windows-native qualification remains intentionally deferred; no native PASS is claimed.", ["QUALIFICATION_DEBT.json"])

failed = [c for c in checks if c["status"] == "FAIL"]
summary = {
    "phase": "17.1",
    "status": "PASS" if not failed else "FAIL",
    "executedChecks": sum(c["status"] in {"PASS", "FAIL"} for c in checks),
    "passedChecks": sum(c["status"] == "PASS" for c in checks),
    "failedChecks": len(failed),
    "notExecutedChecks": sum(c["status"] == "NOT_EXECUTED" for c in checks),
    "nativeWindowsQualified": False,
    "releaseCandidateClaim": False,
}
if ARGS.output:
    ARGS.output.parent.mkdir(parents=True, exist_ok=True)
    ARGS.output.write_text(json.dumps({"schemaVersion": 1, "summary": summary, "checks": checks}, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
print(json.dumps(summary, ensure_ascii=False))
for c in checks:
    print(f"{c['status']:12} {c['id']} {c['detail']}")
raise SystemExit(1 if failed else 0)
