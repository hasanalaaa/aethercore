#!/usr/bin/env python3
"""Phase 33 compliance-reporting audit: strict 29-gate superset of Phase 32."""
from __future__ import annotations

import hashlib
import importlib.util
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else Path(__file__).resolve().parents[1]
failures: list[str] = []
new_gates: list[str] = []


def gate(name: str, condition: bool, detail: str = "") -> None:
    new_gates.append(name)
    if not condition:
        failures.append(f"{name}: {detail}")


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


# Import and execute the entire P32 audit. No P33 superseded filters are added.
# One inherited P30 gate regenerates its old patch directory as a side effect.
# Run the inherited chain against a hard-linked disposable clone so all 817
# checks see current P33 bytes while the delivered tree remains read-only.
audit_temp = Path(tempfile.mkdtemp(prefix="aethercore-p33-inherited-"))
audit_root = audit_temp / "root"
shutil.copytree(
    ROOT,
    audit_root,
    copy_function=os.link,
    ignore=shutil.ignore_patterns(
        "target",
        "node_modules",
        ".git",
        "dist",
        "state",
        "support-staging",
        "__pycache__",
    ),
)
spec = importlib.util.spec_from_file_location(
    "phase32_audit", audit_root / "scripts" / "phase32-adversarial-audit.py"
)
p32 = importlib.util.module_from_spec(spec)
assert spec.loader is not None
sys.argv = [sys.argv[0], str(audit_root)]
try:
    try:
        spec.loader.exec_module(p32)
    except SystemExit:
        pass
    inherited_checks = p32.checks
    failures.extend(p32.failures)
finally:
    shutil.rmtree(audit_temp, ignore_errors=True)
    sys.argv = [sys.argv[0], str(ROOT)]

compliance_path = ROOT / "crates/security-audit/src/compliance.rs"
compliance = compliance_path.read_text(encoding="utf-8") if compliance_path.is_file() else ""
compliance_tests_path = ROOT / "crates/security-audit/tests/compliance_red.rs"
compliance_tests = (
    compliance_tests_path.read_text(encoding="utf-8") if compliance_tests_path.is_file() else ""
)
sec_cli = (ROOT / "apps/aetherctl/src/sec.rs").read_text(encoding="utf-8")
export_rs = (ROOT / "crates/persistence/src/export.rs").read_text(encoding="utf-8")
i18n = (ROOT / "apps/aetherctl/src/i18n.rs").read_text(encoding="utf-8")

# 1-4: engine and strict profile assets.
gate("p33-01-compliance-module-exists", compliance_path.is_file(), "missing compliance.rs")
gate("p33-02-profile-schema-pinned", 'PROFILE_SCHEMA: &str = "COMPLIANCE_PROFILE_V1"' in compliance)
gate(
    "p33-03-strict-profile-deserialization",
    compliance.count("deny_unknown_fields") >= 7,
    "all profile/report structs must reject unknown fields",
)
profile_paths = [
    ROOT / "assets/compliance/profiles/cis-l1.json",
    ROOT / "assets/compliance/profiles/cis-l2.json",
]
gate("p33-04-bundled-cis-profiles", all(path.is_file() for path in profile_paths))

# 5: exact coverage of every P32-implemented rule code in each bundled profile.
p32_code = "\n".join(
    path.read_text(encoding="utf-8")
    for path in sorted((ROOT / "crates/security-audit/src").glob("*.rs"))
    if path.name != "compliance.rs"
)
implemented = set(
    re.findall(r'"((?:ssh|pass|sudo|fs|auth|fw|secrets|cve)\.[\w]+)"', p32_code)
)
coverage_failures: list[str] = []
for path in profile_paths:
    if not path.is_file():
        continue
    profile = json.loads(path.read_text(encoding="utf-8"))
    mapped = {
        code
        for family in profile.get("control_families", [])
        for control in family.get("controls", [])
        for code in control.get("rule_codes", [])
    }
    if mapped != implemented:
        coverage_failures.append(
            f"{path.name}: missing={sorted(implemented - mapped)} extra={sorted(mapped - implemented)}"
        )
gate("p33-05-complete-p32-rule-coverage", not coverage_failures, "; ".join(coverage_failures))

# 6-12: typed honesty, totals, score and deterministic report envelope.
gate(
    "p33-06-adversarial-rule-reference-tests",
    "ComplianceError::UnknownRule" in compliance_tests
    and "profile_rejects_invalid_rule_and_control_shape" in compliance_tests,
)
gate(
    "p33-07-not-verified-first-class",
    "NotVerified" in compliance and "not_verified_is_not_pass" in compliance_tests,
)
gate(
    "p33-08-total-invariant",
    "report.controls.len()" in compliance_tests
    and "report.score.total()" in compliance_tests
    and "report.score.total() != report.controls.len()" in compliance,
)
gate(
    "p33-09-score-excludes-na-nv",
    "pass + fail == 0" in compliance
    and "pass as f64 * 100.0 / (pass + fail) as f64" in compliance,
)
gate(
    "p33-10-zero-verifiable-typed-state",
    "NoVerifiableControls" in compliance
    and "zero_verifiable_controls_has_typed_state" in compliance_tests,
)
gate("p33-11-report-schema-pinned", 'REPORT_SCHEMA: &str = "aethercore.compliance.v1"' in compliance)
gate(
    "p33-12-deterministic-digest",
    "canonical.generated_unix_ms = 0" in compliance
    and "canonical.controls" in compliance
    and "sort_by(|left, right| left.control_id" in compliance
    and "digest_sorts_semantically_and_excludes_timestamp" in compliance_tests,
)

# 13-18: Phase29 signing reuse, unsigned honesty, tamper, air-gap, i18n, redaction.
gate(
    "p33-13-phase29-signature-primitive-reused",
    "pub fn sign_digest" in export_rs
    and "pub fn verify_digest_signature" in export_rs
    and "aethercore_persistence::export::sign_digest" in sec_cli
    and "aethercore_persistence::export::verify_digest_signature" in sec_cli
    and "ed25519_dalek" not in compliance,
)
gate(
    "p33-14-unsigned-report-honesty",
    "signed != report.signature.is_some()" in compliance
    and "unsigned integrity: PASS signed=false signature=absent" in sec_cli,
)
gate(
    "p33-15-typed-tamper-rejection",
    "digest mismatch" in sec_cli
    and "signature mismatch" in sec_cli
    and "signature tamper must fail" in sec_cli,
)
render_slice = compliance.split("pub fn render_html", 1)[1] if "pub fn render_html" in compliance else ""
gate(
    "p33-16-self-contained-bilingual-html",
    "inline CSS" in compliance
    and 'dir=\\\"rtl\\\"' in compliance
    and "http://" not in render_slice
    and "https://" not in render_slice,
)
new_i18n_keys = set(
    re.findall(r'"((?:compliance|cli\.usage\.compliance|sec\.signatureMismatch)[\w.]*)"', i18n.split("];", 1)[0])
)
en_block = i18n.split("pub fn en(", 1)[1].split("pub fn ar(", 1)[0]
ar_block = i18n.split("pub fn ar(", 1)[1]
en_keys = set(re.findall(r'"([\w.]+)"\s*=>', en_block))
ar_keys = set(re.findall(r'"([\w.]+)"\s*=>', ar_block))
gate(
    "p33-17-en-ar-i18n-parity",
    bool(new_i18n_keys) and new_i18n_keys <= en_keys and new_i18n_keys <= ar_keys
    and "compliance_help_is_present_in_both_languages" in i18n,
    f"missing EN={sorted(new_i18n_keys-en_keys)} AR={sorted(new_i18n_keys-ar_keys)}",
)
gate(
    "p33-18-no-raw-secret-regression",
    "evidence_refs.push(finding.id.clone())" in compliance
    and "finding.evidence" not in compliance
    and "sensitive raw fact" in compliance_tests,
)

# 19-22: seal closure and wire freeze.
archive_builders = sorted((ROOT / "scripts").glob("_build_p*_archive.py"))
gate(
    "p33-19-all-archive-builders-exclude-ds-store",
    bool(archive_builders)
    and all("DS_Store" in path.read_text(encoding="utf-8") for path in archive_builders),
    f"missing marker: {[p.name for p in archive_builders if 'DS_Store' not in p.read_text(encoding='utf-8')]}",
)
p32_mdr = (ROOT / "docs/phase32/MASTER_DELIVERY_REPORT.md").read_text(encoding="utf-8")
p32_authoritative_hash = (ROOT.parent / "PHASE32_FINAL_SHA256.txt").read_text(encoding="utf-8").split()[0]
gate(
    "p33-20-p32-mdr-hash-pointer-only",
    "Authoritative archive hash: see PHASE32_FINAL_SHA256.txt" in p32_mdr
    and p32_authoritative_hash not in p32_mdr,
)
p33_mdr = (ROOT / "docs/phase33/MASTER_DELIVERY_REPORT.md").read_text(encoding="utf-8")
gate(
    "p33-21-p33-mdr-hash-pointer-only",
    "Authoritative archive hash: see PHASE33_FINAL_SHA256.txt" in p33_mdr
    and not re.search(r"\b[0-9a-f]{64}\b", p33_mdr),
)
p32_ledger = json.loads(
    (ROOT / "PHASE_32_BINARY_SAFE_PATCH/PHASE_32_EXPECTED_FULL_SHA256.json").read_text(encoding="utf-8")
)["files"]
proto_paths = sorted((ROOT / "crates/contracts/proto").glob("*.proto"))
proto_unchanged = all(p32_ledger.get(str(path.relative_to(ROOT))) == sha256(path) for path in proto_paths)
operations = (ROOT / "crates/contracts/proto/operations.proto").read_text(encoding="utf-8")
request = operations.split("message Request", 1)[1].split("oneof payload", 1)[1].split("\n  }", 1)[0]
response = operations.split("message Response", 1)[1].split("oneof payload", 1)[1].split("\n  }", 1)[0]
request_tags = [int(tag) for tag in re.findall(r"^\s{4}\w+\s+\w+\s*=\s*(\d+);", request, re.M)]
response_tags = [int(tag) for tag in re.findall(r"^\s{4}\w+\s+\w+\s*=\s*(\d+);", response, re.M)]
gate(
    "p33-22-zero-new-wire-tags-freeze-90-52",
    proto_unchanged and max(request_tags) == 90 and max(response_tags) == 52,
)

# 23-26: capability bans, docs, and exact append-only debt proof against sealed P32.
mutation_symbols = ["fs::write", "File::create", "OpenOptions", "remove_file", "remove_dir", "rename("]
gate(
    "p33-23-compliance-engine-read-only",
    not any(symbol in compliance for symbol in mutation_symbols),
)
network_symbols = ["std::net", "TcpStream", "reqwest", "ureq", "hyper", "curl", "fetch("]
runtime_text = compliance + sec_cli.split("/// Offline `sec report", 1)[0]
gate(
    "p33-24-runtime-renderer-verifier-network-free",
    not any(symbol in runtime_text for symbol in network_symbols),
)
required_docs = [
    "ARCHITECTURE.md",
    "QUALIFICATION_DEBT.json",
    "ISSUES.json",
    "SCORECARD.md",
    "MASTER_DELIVERY_REPORT.md",
    "PROGRESS.md",
]
docs_dir = ROOT / "docs/phase33"
gate(
    "p33-25-doc-set-complete-nontrivial",
    all((docs_dir / name).is_file() and (docs_dir / name).stat().st_size > 300 for name in required_docs),
)
base_debt_path = Path("/tmp/p33_p32_seal/base/DEBT_REGISTER.json")
if base_debt_path.is_file():
    base_debt = json.loads(base_debt_path.read_text(encoding="utf-8"))
else:
    archive = ROOT.parent / "AetherCore-Phase32-Master-Delivery.zip"
    member = "AetherCore-Phase32-Master-Delivery/DEBT_REGISTER.json"
    extracted = subprocess.run(["tar", "-xOf", str(archive), member], capture_output=True, check=True)
    base_debt = json.loads(extracted.stdout)
current_debt = json.loads((ROOT / "DEBT_REGISTER.json").read_text(encoding="utf-8"))
base_entries = base_debt.get("entries", [])
current_entries = current_debt.get("entries", [])
gate(
    "p33-26-debt-register-append-only",
    current_entries[: len(base_entries)] == base_entries
    and [entry.get("id") for entry in current_entries[len(base_entries) :]]
    == ["QD-033-001", "QD-033-002", "QD-033-003"],
)

# 27-28: current artifact coverage and deterministic read-only verification x2.
verify_command = [sys.executable, str(ROOT / "PHASE_33_BINARY_SAFE_PATCH/verify_phase33.py"), str(ROOT)]
first_patch = subprocess.run(verify_command, capture_output=True, text=True)
first_full = subprocess.run(verify_command + ["--full-tree"], capture_output=True, text=True)
manifest = json.loads((ROOT / "PHASE_33_BINARY_SAFE_PATCH/MANIFEST.json").read_text(encoding="utf-8"))
ledger = json.loads(
    (ROOT / "PHASE_33_BINARY_SAFE_PATCH/PHASE_33_EXPECTED_FULL_SHA256.json").read_text(encoding="utf-8")
)
manifest_post = {
    entry[1:] if entry.startswith("+") else entry
    for entry in manifest.get("files", [])
    if not entry.startswith("-")
}
gate(
    "p33-27-ledger-manifest-full-tree-consistency",
    first_patch.returncode == 0
    and first_full.returncode == 0
    and bool(manifest_post)
    and manifest_post <= set(ledger.get("files", {}))
    and not any(rel.startswith("PHASE_33_BINARY_SAFE_PATCH/") for rel in ledger.get("files", {})),
)
second_patch = subprocess.run(verify_command, capture_output=True, text=True)
second_full = subprocess.run(verify_command + ["--full-tree"], capture_output=True, text=True)
gate(
    "p33-28-verifier-determinism-x2",
    first_patch.returncode == second_patch.returncode == 0
    and first_full.returncode == second_full.returncode == 0
    and first_patch.stdout == second_patch.stdout
    and first_full.stdout == second_full.stdout,
)

# 29: exact count guard. This also prevents accidental omission/addition of a gate.
gate(
    "p33-29-exact-audit-total-above-817",
    inherited_checks == 817 and inherited_checks + 29 > 817 and len(new_gates) == 28,
    f"inherited={inherited_checks} new={len(new_gates)}",
)

checks = inherited_checks + len(new_gates)
result = {
    "schema": "aethercore.phase33.adversarial-audit.v1",
    "root": str(ROOT),
    "inherited_p32_checks": inherited_checks,
    "new_gates": new_gates,
    "checks": checks,
    "failures": failures,
    "status": "PASS" if not failures else "FAIL",
}
print(json.dumps(result, indent=2))
raise SystemExit(0 if not failures else 1)
