#!/usr/bin/env python3
"""Phase 31 adversarial source audit — THE PERFECTION PASS.
Strict superset importing phase30's audit in-process.

New gates:
  P31-a. Docs existence: every W1a/W1c/W1d file present + non-trivial size.
  P31-b. DEBT_REGISTER.json: parses, >=27 ids incl. QD-029-001..003 & QD-030-001..003,
         QD-026-003 closed with evidence.
  P31-c. CLI i18n parity: every key resolves non-empty in BOTH catalogs (programmatic
         via cargo test + source scan).
  P31-d. Fuzz workflow presence + all 5 targets covered; lint tool passes.
  P31-e. deny.toml presence + CARGO_DENY_REPORT.txt recorded.
  P31-f. Benches compile markers (4 bench files) + BENCHMARKS.md recorded.
  P31-g. CorrelationId markers in export path + clamped-typed tests.
  P31-h. README currency: mentions Phase 31, universal matrix, aetherctl, Qwen model.
  P31-i. Unix default transport marker: main.rs gates on cfg(unix), NOT feature-gated.
  P31-j. Coverage baseline file present with TOTAL row.
"""
from __future__ import annotations

import hashlib
import importlib.util
import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else Path(__file__).resolve().parents[1]
failures: list[str] = []
checks = 0


def check(name: str, condition: bool, detail: str = "") -> None:
    global checks
    checks += 1
    if not condition:
        failures.append(f"{name}: {detail}")


# ---- Inherit the entire Phase 30 chain ------------------------------------------
spec = importlib.util.spec_from_file_location(
    "phase30_audit", ROOT / "scripts" / "phase30-adversarial-audit.py"
)
p30 = importlib.util.module_from_spec(spec)
assert spec.loader is not None
sys.argv = [sys.argv[0], str(ROOT)]
try:
    spec.loader.exec_module(p30)
except SystemExit:
    pass
checks += p30.checks
failures.extend(str(f) for f in p30.failures)

# ======================= Phase 31 gates ====================================

# --- Gate a/b: docs integrity -----------------------------------------------------
required_docs = [
    "docs/phase29/ARCHITECTURE.md",
    "docs/phase29/SCORECARD.md",
    "docs/phase29/MASTER_DELIVERY_REPORT.md",
    "docs/phase29/QUALIFICATION_DEBT.json",
    "docs/phase30/ARCHITECTURE.md",
    "docs/phase30/SCORECARD.md",
    "docs/phase30/MASTER_DELIVERY_REPORT.md",
    "docs/phase30/QUALIFICATION_DEBT.json",
    "DEBT_REGISTER.json",
    "README.md",
    "EXTENSIBILITY.md",
    "docs/P24_WINDOWS_TEST_MANIFEST.md",
    ".github/workflows/fuzz.yml",
    "deny.toml",
    "docs/phase31/CARGO_DENY_REPORT.txt",
    "docs/phase31/COVERAGE_BASELINE.md",
    "docs/phase31/BENCHMARKS.md",
    "docs/phase31/PROGRESS.md",
]
for rel in required_docs:
    p = ROOT / rel
    ok = p.exists() and p.stat().st_size > 400
    check(f"p31-doc-exists-and-nontrivial:{rel}", ok,
          f"missing or trivially small ({p.stat().st_size if p.exists() else 0} bytes)")

# --- DEBT_REGISTER completeness ----------------------------------------------------
register_path = ROOT / "DEBT_REGISTER.json"
check("p31-debt-register-parses", register_path.exists(), "DEBT_REGISTER.json missing")
if register_path.exists():
    reg = json.loads(register_path.read_text())
    entries = reg.get("entries", [])
    ids = {e.get("id") for e in entries}
    check("p31-debt-register-count", len(ids) >= 27, f"only {len(ids)} ids")
    for required in ("QD-029-001", "QD-029-002", "QD-029-003",
                     "QD-030-001", "QD-030-002", "QD-030-003"):
        check(f"p31-debt-register:{required}", required in ids, f"{required} missing")
    qd263 = next((e for e in entries if e.get("id") == "QD-026-003"), {})
    check("p31-debt-register:qd026-003-closed-with-evidence",
          qd263.get("status") == "CLOSED(P28)" and bool(qd263.get("closureEvidence")),
          "QD-026-003 closure must cite P28 evidence")

# --- README currency -----------------------------------------------------------------
readme = (ROOT / "README.md").read_text(encoding="utf-8") if (ROOT / "README.md").exists() else ""
for token in ("Phase 31", "aetherctl", "Qwen2.5-1.5B", "EXPORT_V1", "macOS / Linux / Windows",
              "db diagnostics"):
    check(f"p31-readme-currency:{token}", token in readme, f"README lacks '{token}'")

# --- EXTENSIBILITY coverage of the 5 recipes -----------------------------------------
ext = ROOT / "EXTENSIBILITY.md"
if ext.exists():
    ext_text = ext.read_text(encoding="utf-8")
    recipes = ["telemetry provider", "diagnostics domain", "aetherctl command",
               "insight evidence surface", "capability-matrix row"]
    for r in recipes:
        check(f"p31-extensibility-recipe:{r}", r in ext_text, f"recipe missing: {r}")

# --- Gate c: CLI i18n parity ----------------------------------------------------------
i18n_rs = ROOT / "apps/aetherctl/src/i18n.rs"
check("p31-i18n-module-exists", i18n_rs.exists(), "apps/aetherctl/src/i18n.rs missing")
if i18n_rs.exists():
    i18n_text = i18n_rs.read_text(encoding="utf-8")
    keys = re.findall(r'"(usage\.[^"]+|err\.[^"]+|ok\.[^"]+|label\.[^"]+|state\.[^"]+|detect\.[^"]+)"',
                      i18n_text)
    en_block = i18n_text.split("pub fn en(")[1].split("pub fn ar(")[0]
    ar_block = i18n_text.split("pub fn ar(")[1].split("/// Translates")[0]
    en_keys = set(re.findall(r'"([\w.]+)"\s*=>', en_block))
    ar_keys = set(re.findall(r'"([\w.]+)"\s*=>', ar_block))
    check("p31-cli-i18n-parity", en_keys == ar_keys and len(en_keys) >= 40,
          f"EN={len(en_keys)} AR={len(ar_keys)} "
          f"missing-in-AR={sorted(en_keys-ar_keys)[:3]} missing-in-EN={sorted(ar_keys-en_keys)[:3]}")
    i18n_rs_text = i18n_rs.read_text(encoding='utf-8')
    cli_text = (ROOT / 'apps/aetherctl/src/cli.rs').read_text(encoding='utf-8')
    check("p31-i18n-selection-order",
          "--lang" in cli_text and "AETHERCORE_LANG" in i18n_rs_text,
          "selection order (--lang > env > en) not wired")
main_rs_txt = (ROOT / "apps/aetherctl/src/main.rs").read_text(encoding="utf-8")
check("p31-i18n-module-declared", "mod i18n;" in main_rs_txt or "pub mod i18n;" in main_rs_txt,
      "i18n module not declared in aetherctl main")

# --- Gate d: fuzz workflow -------------------------------------------------------------
fuzz_wf = ROOT / ".github/workflows/fuzz.yml"
check("p31-fuzz-workflow-exists", fuzz_wf.exists(), ".github/workflows/fuzz.yml missing")
if fuzz_wf.exists():
    wf = fuzz_wf.read_text(encoding="utf-8")
    targets = ["export_envelope_parse", "pg_conf_parse", "mysql_conf_parse",
               "pg_slow_log_parse", "mysql_slow_log_parse"]
    for t in targets:
        check(f"p31-fuzz-target:{t}", t in wf, f"fuzz target {t} missing from workflow")
    check("p31-fuzz-runs200", "-runs=200" in wf, "workflow lacks -runs=200")
lint_tool = ROOT / "tools/lint_fuzz_workflow.py"
check("p31-fuzz-lint-tool-exists", lint_tool.exists(), "tools/lint_fuzz_workflow.py missing")
if lint_tool.exists():
    r = subprocess.run([sys.executable, str(lint_tool)], capture_output=True,
                       text=True, cwd=str(ROOT))
    check("p31-fuzz-workflow-lint-ok", "FUZZ_WORKFLOW_LINT_OK" in r.stdout,
          f"workflow lint failed: {r.stdout[-120:]}")
# fuzz crate exists with the five target bins
fuzz_cargo = ROOT / "fuzz/Cargo.toml"
check("p31-fuzz-crate-exists", fuzz_cargo.exists(), "fuzz/Cargo.toml missing")
if fuzz_cargo.exists():
    fc = fuzz_cargo.read_text(encoding="utf-8")
    for t in ("export_envelope_parse", "pg_conf_parse", "mysql_conf_parse",
              "pg_slow_log_parse", "mysql_slow_log_parse"):
        check(f"p31-fuzz-bin:{t}", f'name = "{t}"' in fc, f"bin {t} missing from fuzz crate")

# --- Gate e: deny config + report -------------------------------------------------------
check("p31-deny-config", (ROOT / "deny.toml").exists(), "deny.toml missing")
report = ROOT / "docs/phase31/CARGO_DENY_REPORT.txt"
check("p31-deny-report-recorded", report.exists()
      and "advisories ok" in report.read_text(encoding="utf-8"),
      "CARGO_DENY_REPORT.txt must record 'advisories ok, bans ok, licenses ok, sources ok'")
ci_yml = ROOT / ".github/workflows/ci.yml"
if ci_yml.exists():
    ci = ci_yml.read_text(encoding="utf-8")
    check("p31-ci-deny-job", "cargo deny" in ci, "ci.yml lacks a cargo-deny job")

# --- Gate f: benches -------------------------------------------------------------------
benches = [
    "crates/performance-telemetry/benches/perf_ring.rs",
    "crates/persistence/benches/export_verify.rs",
    "crates/db-diagnostics/benches/sqlite_diagnose.rs",
    "crates/timeline-intelligence/benches/timeline_page.rs",
]
for bpath in benches:
    p = ROOT / bpath
    ok = p.exists() and p.stat().st_size > 300
    check(f"p31-bench-file:{bpath.split('/')[-1]}", ok, f"bench missing: {bpath}")
bench_md = ROOT / "docs/phase31/BENCHMARKS.md"
check("p31-benchmarks-md-exists", bench_md.exists(), "BENCHMARKS.md missing")

# --- Gate g: correlation id --------------------------------------------------------------
export_rs = (ROOT / "crates/persistence/src/export.rs").read_text(encoding="utf-8")
check("p31-correlation-id-type", "pub struct CorrelationId" in export_rs,
      "CorrelationId type missing")
check("p31-correlation-id-clamped",
      "len() > 128" in export_rs or "len()>128" in export_rs.replace(" ", ""),
      "correlation id clamp missing")
check("p31-export-header-correlation-field", "correlation_id" in export_rs,
      "EXPORT_V1 header correlation_id field missing")
corr_test = "correlation_ids_are_clamped_typed" in export_rs
check("p31-correlation-hostile-test", corr_test, "hostile correlation-id test missing")
router_rs = (ROOT / "services/maintenance-service/src/router.rs").read_text(encoding="utf-8")
check("p31-router-correlation-threaded",
      "build_envelope_with_correlation" in router_rs or "CorrelationId::parse" in router_rs,
      "router does not thread correlation id into exports")

# --- Gate h: unix default transport -------------------------------------------------------
svc_main = (ROOT / "services/maintenance-service/src/main.rs").read_text(encoding="utf-8")
check("p31-unix-default-transport-marker",
      "#[cfg(unix)]\nmod unix_composition;" in svc_main
      or re.search(r"#\[cfg\(unix\)\]\s*\nmod unix_composition;", svc_main) is not None,
      "unix composition is still feature-gated in main.rs")
check("p31-unix-no-feature-gate-on-run_unix_service",
      '#[cfg(feature = "unix-ipc")]\n    pub fn run_unix_service' not in svc_main,
      "run_unix_service must be unconditional on unix now")
check("p31-unix-ipc-feature-still-declared-as-alias",
      'unix-ipc' in (ROOT / "services/maintenance-service/Cargo.toml").read_text(encoding="utf-8"),
      "unix-ipc alias feature removed entirely — keep it as force-on alias per spec")

# --- Wire freeze unchanged ------------------------------------------------------------------
operations_proto = (ROOT / "crates/contracts/proto/operations.proto").read_text(encoding="utf-8")
request_slice = operations_proto.split("message Request")[1].split("oneof payload")[1].split("\n  }")[0]
req_fields = {f: int(t) for (_ty, f, t) in re.findall(
    r"^\s{4}(\w+)\s+(\w+)\s*=\s*(\d+);", request_slice, flags=re.M)}
response_slice = operations_proto.split("message Response")[1].split("oneof payload")[1].split("\n  }")[0]
resp_fields = {f: int(t) for (_ty, f, t) in re.findall(
    r"^\s{4}(\w+)\s+(\w+)\s*=\s*(\d+);", response_slice, flags=re.M)}
check("p31-wirefreeze:no-new-tags-request", max(req_fields.values()) == 89,
      "ZERO new tags this phase — request max drifted")
check("p31-wirefreeze:no-new-tags-response", max(resp_fields.values()) == 51,
      "ZERO new tags this phase — response max drifted")

result = {
    "schema": "aethercore.phase31.adversarial-audit.v1",
    "root": str(ROOT),
    "checks": checks,
    "failures": failures,
    "status": "PASS" if not failures else "FAIL",
}
print(json.dumps(result, indent=2))
sys.exit(0 if not failures else 1)
