#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
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


def add(check_id: str, status: str, detail: str, evidence: list[str] | None = None) -> None:
    checks.append({"id": check_id, "status": status, "detail": detail, "evidence": evidence or []})


def require_text(check_id: str, path: str, needles: list[str]) -> None:
    p = ROOT / path
    if not p.is_file():
        add(check_id, "FAIL", f"missing {path}")
        return
    text = p.read_text(encoding="utf-8")
    missing = [needle for needle in needles if needle not in text]
    add(check_id, "PASS" if not missing else "FAIL", "all required integration markers present" if not missing else f"missing markers: {missing}", [path])


def catalog_keys(path: Path) -> set[str]:
    return set(re.findall(r"^\s*'([^']+)'\s*:", path.read_text(encoding="utf-8"), flags=re.M))


required_files = [
    "crates/pc-intelligence/Cargo.toml",
    "crates/pc-intelligence/src/model.rs",
    "crates/pc-intelligence/src/rules.rs",
    "crates/pc-intelligence/src/normalize.rs",
    "crates/pc-intelligence/src/sources.rs",
    "crates/pc-intelligence/src/coordinator.rs",
    "crates/pc-intelligence/tests/scenarios.rs",
    "crates/contracts/proto/intelligence.proto",
    "crates/persistence/migrations/0010_phase17_intelligence.sql",
    "apps/ui/src/features/intelligence/DeepScanPage.svelte",
    "apps/ui/src/features/intelligence/FindingCard.svelte",
    "QUALIFICATION_DEBT.json",
    "tests/fixtures/phase17/scenarios.json",
]
missing = [p for p in required_files if not (ROOT / p).is_file()]
add("P17-STATIC-001", "PASS" if not missing else "FAIL", "required Phase 17 source files present" if not missing else f"missing: {missing}", required_files)

require_text("P17-STATIC-002", "Cargo.toml", ['"crates/pc-intelligence"'])
require_text("P17-STATIC-003", "crates/pc-intelligence/src/model.rs", [
    "pub struct SystemFact", "pub enum FactPayload", "pub struct Finding", "pub enum Severity", "pub enum Confidence",
    "pub struct RemediationCandidate", "pub struct RemediationPlan", "immutable: true", "private_id",
])
require_text("P17-STATIC-004", "crates/pc-intelligence/src/coordinator.rs", [
    "DeepScanCoordinator", "CancellationToken", "execute_batch", "ScanState::Partial", "set_finding_ignored",
    "RemediationPlan::seal", "CollectorState::TimedOut", "collector worker unavailable",
])

require_text("P17-STATIC-004B", "crates/pc-intelligence/src/sources.rs", ["ReadBudgetManager", "DISCOVERY_TIMEOUT", "token.is_cancelled()"])

require_text("P17-STATIC-004C", "crates/pc-intelligence/src/coordinator.rs", [
    "recent_verified_driver_install_items_for_owner", "normalize::driver_change", "publish_interim_intelligence",
    "ScanMetrics", "record_stream_event", "persistence_write_count",
])
require_text("P17-STATIC-004D", "crates/pc-intelligence/src/normalize.rs", [
    "FactPayload::HardwareDevice", "driver-install-journal", "InstalledDriverEvidence", "FaultKind::PermissionDenied",
])
rules_text = (ROOT / "crates/pc-intelligence/src/rules.rs").read_text(encoding="utf-8")
rule_ids = sorted(set(re.findall(r'id:\s*"(P17-[A-Z]+-\d{3})"', rules_text)))
corr_ids = [r for r in rule_ids if "CORR" in r]
invalid_corr = "P17-CORR-002" in rules_text or "SERVICING_REPAIR_LIKELY" in rules_text
add("P17-STATIC-005", "PASS" if len(rule_ids) >= 12 and len(corr_ids) >= 2 and not invalid_corr else "FAIL", f"{len(rule_ids)} versioned rules, {len(corr_ids)} cross-domain correlations; unsupported self-update/servicing correlation absent", ["crates/pc-intelligence/src/rules.rs"])

model_and_rules = (ROOT / "crates/pc-intelligence/src/model.rs").read_text(encoding="utf-8") + rules_text
score_bad = bool(re.search(r"healthScore|pc_health_score|PC Health\s*=|health\s*percentage", model_and_rules, re.I))
add("P17-STATIC-006", "PASS" if not score_bad else "FAIL", "no generic PC health percentage/score in intelligence core", ["crates/pc-intelligence/src/model.rs", "crates/pc-intelligence/src/rules.rs"])

ui_paths = [ROOT / "apps/ui/src/features/intelligence/DeepScanPage.svelte", ROOT / "apps/ui/src/features/intelligence/FindingCard.svelte", ROOT / "apps/ui/src/features/intelligence/controller.ts"]
ui_text = "\n".join(p.read_text(encoding="utf-8") for p in ui_paths)
fake_motion = bool(re.search(r"setInterval\s*\(|@keyframes|animation\s*:", ui_text))
primitives_ok = all(x in ui_text for x in ["Pressable", "ProgressBar", "MaterialSurface", "aria-label"])
add("P17-STATIC-007", "PASS" if not fake_motion and primitives_ok else "FAIL", "Deep Scan uses production interaction primitives and no time/keyframe fake-progress loop", [str(p.relative_to(ROOT)) for p in ui_paths])

require_text("P17-STATIC-008", "crates/contracts/proto/operations.proto", ["StartDeepScanRequest", "CancelDeepScanRequest", "GetDeepScanSnapshotRequest", "GetDeepScanHistoryRequest", "SealRemediationPlanRequest"])
require_text("P17-STATIC-008B", "crates/contracts/proto/intelligence.proto", ["PcScanMetrics", "streamed_event_count", "persistence_write_count", "DeepScanSnapshot"])
require_text("P17-STATIC-009", "crates/contracts/proto/events.proto", ["EVENT_KIND_DEEP_SCAN", "DeepScanSnapshot"])
require_text("P17-STATIC-010", "services/maintenance-service/src/router.rs", ["StartDeepScan", "CancelDeepScan", "GetDeepScanSnapshot", "GetDeepScanHistory", "SealRemediationPlan", "watch_deep_scan"])
require_text("P17-STATIC-011", "services/maintenance-service/src/streaming.rs", ["watch_deep_scan", "DeepScanSnapshot", "publish_hydration"])
require_text("P17-STATIC-012", "apps/desktop/src/main.rs", ["start_deep_scan", "cancel_deep_scan", "get_deep_scan_snapshot", "get_deep_scan_history", "seal_remediation_plan", '"deepScanSnapshot"'])
require_text("P17-STATIC-013", "apps/ui/src/platform/stream-state.ts", ["deepScan", "deepScanHistory", "deepScanSnapshot"])
require_text("P17-STATIC-014", "apps/ui/src/app/AppShell.svelte", ["DeepScanPage", "activePage === 'deepScan'"])

# Localization parity and source-used keys.
en_path = ROOT / "apps/ui/src/lib/i18n/catalog.en.ts"
ar_path = ROOT / "apps/ui/src/lib/i18n/catalog.ar.ts"
en, ar = catalog_keys(en_path), catalog_keys(ar_path)
parity = en == ar
phase_keys = {k for k in en if k.startswith(("deepScan.", "finding.", "remediation."))} | {"nav.deepScan", "nav.deepScanDescription", "overview.intelligenceEyebrow", "overview.intelligenceTitle", "overview.intelligenceCopy", "overview.scanMyPc"}
source_files = [
    ROOT / "apps/ui/src/features/intelligence/DeepScanPage.svelte",
    ROOT / "apps/ui/src/features/intelligence/FindingCard.svelte",
    ROOT / "apps/ui/src/features/intelligence/controller.ts",
    ROOT / "crates/pc-intelligence/src/model.rs",
    ROOT / "crates/pc-intelligence/src/rules.rs",
]
used = set()
for p in source_files:
    text = p.read_text(encoding="utf-8")
    used.update(re.findall(r"['\"]((?:deepScan|finding|remediation)\.[A-Za-z0-9_.-]+)['\"]", text))
used.update(["nav.deepScan", "nav.deepScanDescription", "overview.intelligenceEyebrow", "overview.intelligenceTitle", "overview.intelligenceCopy", "overview.scanMyPc"])
# generated remediation keys from executable finding codes
finding_codes = set(re.findall(r'finding\(\s*fact,\s*"([A-Z0-9_]+)"', rules_text))
for code in finding_codes:
    if code in {"HIGH_MEMORY_PRESSURE"}:  # no remediation candidate is generated for this finding
        continue
    used.add("remediation." + code.lower())
missing_en, missing_ar = sorted(used - en), sorted(used - ar)
add("P17-STATIC-015", "PASS" if parity and not missing_en and not missing_ar else "FAIL", f"catalog parity={parity}; phase keys={len(phase_keys)}; missing EN={missing_en}; missing AR={missing_ar}", [str(en_path.relative_to(ROOT)), str(ar_path.relative_to(ROOT))])

# SQLite migrations and FK behavior.
try:
    conn = sqlite3.connect(":memory:")
    conn.execute("PRAGMA foreign_keys=ON")
    migration_files = sorted((ROOT / "crates/persistence/migrations").glob("*.sql"))
    for p in migration_files:
        conn.executescript(p.read_text(encoding="utf-8"))
    tables = {r[0] for r in conn.execute("SELECT name FROM sqlite_master WHERE type='table'")}
    required_tables = {"intelligence_scans", "intelligence_findings", "intelligence_overrides", "intelligence_remediation_plans"}
    conn.execute("INSERT INTO intelligence_scans VALUES(?,?,?,?,?,?,?,?,?,?,?,?)", ("scan","owner","Completed","Healthy",1,2,"fp","phase17-rules-v1","0.1",0,0,"{}"))
    conn.execute("INSERT INTO intelligence_remediation_plans VALUES(?,?,?,?,?,?)", ("plan","owner","scan","digest","{}",3))
    fk_restrict = False
    try:
        conn.execute("DELETE FROM intelligence_scans WHERE scan_id='scan'")
    except sqlite3.IntegrityError:
        fk_restrict = True
    ok = required_tables.issubset(tables) and fk_restrict and len(migration_files) >= 10
    add("P17-STATIC-016", "PASS" if ok else "FAIL", f"executed {len(migration_files)} migrations in-memory; Phase17 tables present={required_tables.issubset(tables)}; sealed-plan FK RESTRICT={fk_restrict}", ["crates/persistence/migrations/0010_phase17_intelligence.sql"])
except Exception as exc:
    add("P17-STATIC-016", "FAIL", f"SQLite migration execution failed: {exc}")

# Qualification debt schema.
try:
    debt = json.loads((ROOT / "QUALIFICATION_DEBT.json").read_text(encoding="utf-8"))
    items = debt.get("items", [])
    req = {"capabilityId", "subsystem", "windowsApiDependency", "requiredNativeScenario", "expectedEvidence", "severityIfUnqualified", "qualificationHook"}
    unique = len({i.get("capabilityId") for i in items}) == len(items)
    complete = all(req.issubset(i) for i in items)
    add("P17-STATIC-017", "PASS" if len(items) >= 10 and unique and complete else "FAIL", f"qualification debt items={len(items)}, unique={unique}, required fields complete={complete}", ["QUALIFICATION_DEBT.json"])
except Exception as exc:
    add("P17-STATIC-017", "FAIL", f"qualification debt invalid: {exc}")

# Fixture inventory.
try:
    fixture = json.loads((ROOT / "tests/fixtures/phase17/scenarios.json").read_text(encoding="utf-8"))
    ids = {s["id"] for s in fixture["scenarios"]}
    expected = {"healthy-pc", "missing-driver", "storage-concern", "recent-driver-regression", "windows-corruption", "slow-startup", "partial-diagnostics-failure"}
    add("P17-STATIC-018", "PASS" if expected.issubset(ids) else "FAIL", f"synthetic scenarios={len(ids)}; mandatory scenarios present={expected.issubset(ids)}", ["tests/fixtures/phase17/scenarios.json", "crates/pc-intelligence/tests/scenarios.rs"])
except Exception as exc:
    add("P17-STATIC-018", "FAIL", f"fixture inventory invalid: {exc}")

# Phase 17 source-quality scan. Keep audit implementation itself out of the scan because it names the forbidden tokens.
quality_paths = [
    ROOT / "crates/pc-intelligence",
    ROOT / "apps/ui/src/features/intelligence",
    ROOT / "crates/contracts/proto/intelligence.proto",
    ROOT / "crates/persistence/migrations/0010_phase17_intelligence.sql",
    ROOT / "crates/persistence/migrations/0011_phase17_1_intelligence_integrity.sql",
]
violations = []
for target in quality_paths:
    files = target.rglob("*") if target.is_dir() else [target]
    for p in files:
        if not p.is_file():
            continue
        text = p.read_text(encoding="utf-8", errors="replace")
        token = "TO" + "DO|FIX" + "ME|HA" + "CK"
        if re.search(token, text, re.I):
            violations.append(str(p.relative_to(ROOT)))
        if re.search(r"mock\s+logic", text, re.I):
            violations.append(str(p.relative_to(ROOT)) + ":mock-logic")
add("P17-STATIC-019", "PASS" if not violations else "FAIL", "no placeholder markers or production mock-logic markers in Phase17 source" if not violations else f"violations={violations}", [str(p.relative_to(ROOT)) for p in quality_paths])

# Strong typing / no opaque payload JSON in the critical model.
typed_ok = "#[serde(tag = \"kind\"" in (ROOT / "crates/pc-intelligence/src/model.rs").read_text(encoding="utf-8") and "serde_json::Value" not in model_and_rules
add("P17-STATIC-020", "PASS" if typed_ok else "FAIL", "critical fact/finding core uses closed typed payloads rather than serde_json::Value", ["crates/pc-intelligence/src/model.rs"])

# Toolchain evidence. Never turn absence into PASS.
cargo = shutil.which("cargo")
rustc = shutil.which("rustc")
svelte_check = ROOT / "apps/ui/node_modules/.bin/svelte-check"
if cargo and rustc:
    proc = subprocess.run([cargo, "test", "-p", "aethercore-pc-intelligence", "--no-fail-fast"], cwd=ROOT, capture_output=True, text=True)
    add("P17-RUST-001", "PASS" if proc.returncode == 0 else "FAIL", f"cargo test exit={proc.returncode}", ["crates/pc-intelligence/tests/scenarios.rs"])
else:
    add("P17-RUST-001", "NOT_EXECUTED", f"Rust toolchain unavailable on host (cargo={bool(cargo)}, rustc={bool(rustc)})", ["crates/pc-intelligence/tests/scenarios.rs"])

if svelte_check.exists():
    proc = subprocess.run([str(svelte_check), "--tsconfig", "./tsconfig.json"], cwd=ROOT / "apps/ui", capture_output=True, text=True)
    add("P17-UI-001", "PASS" if proc.returncode == 0 else "FAIL", f"svelte-check exit={proc.returncode}", ["apps/ui/src/features/intelligence/DeepScanPage.svelte"])
else:
    add("P17-UI-001", "NOT_EXECUTED", "installed svelte-check dependency unavailable on host", ["apps/ui/src/features/intelligence/DeepScanPage.svelte"])

add("P17-WINDOWS-001", "NOT_EXECUTED", "Windows-native qualification is intentionally deferred by program strategy and this host is not used to claim native PASS.", ["QUALIFICATION_DEBT.json", "scripts/phase17-windows-qualification.ps1"])

failed = [c for c in checks if c["status"] == "FAIL"]
executed = [c for c in checks if c["status"] in {"PASS", "FAIL"}]
summary = {
    "phase": 17,
    "status": "PASS" if not failed else "FAIL",
    "executedChecks": len(executed),
    "passedChecks": sum(c["status"] == "PASS" for c in checks),
    "failedChecks": len(failed),
    "notExecutedChecks": sum(c["status"] == "NOT_EXECUTED" for c in checks),
    "nativeWindowsQualified": False,
    "releaseCandidateClaim": False,
}
report = {"schemaVersion": 1, "summary": summary, "checks": checks}
if ARGS.output:
    ARGS.output.parent.mkdir(parents=True, exist_ok=True)
    ARGS.output.write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
print(json.dumps(summary, ensure_ascii=False))
for check in checks:
    print(f"{check['status']:12} {check['id']} {check['detail']}")
raise SystemExit(1 if failed else 0)
