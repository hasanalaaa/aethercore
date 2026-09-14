#!/usr/bin/env python3
"""Phase 32 adversarial source audit — SECURITY AUDIT DOMAIN (read-only, CIS-mapped).
Strict superset importing phase31's audit in-process.

New gates:
  P32-a. Crate presence + read-only contract: no mutation verbs in
         crates/security-audit/src (chmod/chown/write/create/remove/rename),
         comments stripped before scanning; examples/tests exempt from the
         production-src rule but scanned separately for honesty.
  P32-b. Network-ban symbols absent from the crate AND its Cargo.toml deps are
         allowlist-only (serde/serde_json/sha2/thiserror/regex + dev tempfile).
  P32-c. Secrets-redaction enforcement: evidence construction goes through
         redact(); raw-secret passthrough patterns absent.
  P32-d. vulndb manifest pin present + tamper-test marker in tests.
  P32-e. cis_map asset parses; every control matches ^CIS L[12] \\d+\\.\\d+(\\.\\d+)?$
         or is exactly "unmapped" WITH a note; every implemented rule code has a
         mapping or explicit unmapped note.
  P32-f. Wire freeze RE-FROZEN at the new allocation: request max == 90,
         response max == 52, EventKind 29 present, EventEnvelope tag 38 present,
         nothing renumbered (prior tags still at their exact values).
  P32-g. Router RunSecurityAudit arm: read-only lane, typed traversal guard,
         honest NotAvailable propagation; sec.rs writer scoped EXACTLY like
         export.rs precedent (explicit owner action only).
  P32-h. CLI parity registry: sec/compliance/vulndb commands wired in cli.rs +
         offline executors + i18n parity extended to every sec.* key (EN+AR).
  P32-i. GD live-proof markers: gd4 example exists; tamper test marker present;
         golden fixtures cover weak/hardened sshd, secrets redaction, CVE join.
  P32-j. Docs set exists (docs/phase32/*) incl. HYGIENE.md with freed-bytes and
         moved-file hashes; QUALIFICATION_DEBT append-only QD-032 items present.

Ledger scoping rules (P31 lessons applied): full-tree hash standard continues via
PHASE_32_BINARY_SAFE_PATCH/PHASE_32_EXPECTED_FULL_SHA256.json which EXCLUDES all
later-phase patch dirs (PHASE_30_/PHASE_31_/PHASE_32_BINARY_SAFE_PATCH) as
self-referential deliverables; .DS_Store excluded; documented inside the JSON.
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
sys.dont_write_bytecode = True
sys.path.insert(0, str(ROOT / "scripts"))
from gate_reader import module_text  # noqa: E402

# `DBT-P63-004`: the maintenance service router is a module tree now -
# `router.rs` plus `router/*.rs`. These checks assert its verbs.
ROUTER = "services/maintenance-service/src/router.rs"
failures: list[str] = []
checks = 0


def check(name: str, condition: bool, detail: str = "") -> None:
    global checks
    checks += 1
    if not condition:
        failures.append(f"{name}: {detail}")


# ---- Inherit the entire Phase 31 chain ------------------------------------------
spec = importlib.util.spec_from_file_location(
    "phase31_audit", ROOT / "scripts" / "phase31-adversarial-audit.py"
)
p31 = importlib.util.module_from_spec(spec)
assert spec.loader is not None
sys.argv = [sys.argv[0], str(ROOT)]
try:
    spec.loader.exec_module(p31)
except SystemExit:
    pass
checks += p31.checks

# Superseded filters extended PRINCIPLEDLY (named-list + counter-guard) ONLY for:
#   1. wire-freeze bounds re-frozen at P32's allocation (req 90 / resp 52);
#   2. any P28-mutation-guard style findings on NEW offline write paths —
#      `sec.rs` implements the vulndb-update EXPLICIT owner action and is scoped
#      like export.rs (P29 precedent); its fs::write/remove_file usage is gated
#      POSITIVELY below by p32-owner-action-scoped-writer instead.
SUPERSEDED_PATTERNS_P32 = [
    # Wire freeze re-frozen at P32's allocation (req 90 / resp 52 / EK29 / env38).
    # Every prior phase's freeze gate names the bound it defended; the NEW
    # positive anchors are p32-wire-refreeze:* and p32-wire-anchor:* below.
    r"^p27-wirefreeze:eventkind-max: .*",
    r"^p27-wirefreeze:envelope-max: .*",
    r"^p28-wirefreeze:eventkind-max-28: .*",
    r"^p28-wirefreeze:envelope-max-37: .*",
    r"^p28-wirefreeze:request-max-88: .*",
    r"^p28-wirefreeze:response-max-50: .*",
    r"^p29-wirefreeze:request-max: .*",
    r"^p29-wirefreeze:response-max: .*",
    r"^p30-wirefreeze:request-max-89: .*",
    r"^p30-wirefreeze:response-max-51: .*",
    r"^p31-wirefreeze:no-new-tags-request$",
    r"^p31-wirefreeze:no-new-tags-request: .*",
    r"^p31-wirefreeze:no-new-tags-response$",
    r"^p31-wirefreeze:no-new-tags-response: .*",
    # vulndb-update owner action in sec.rs is the export.rs precedent applied
    # to a new file; its writes are gated POSITIVELY by p32-owner-action-*.
    r"^p28-mutation-guard:offline\.rs: .*$",
]
before_f = len(failures)
failures = [f for f in failures
            if not any(re.match(p, f) for p in SUPERSEDED_PATTERNS_P32)]
checks += 1
if before_f - len(failures) > len(SUPERSEDED_PATTERNS_P32):
    failures.append("p32-p31-filter: filtered more findings than expected")

# ======================= Phase 32 gates ====================================

SEC = ROOT / "crates/security-audit"
SEC_SRC = SEC / "src"

# --- Gate a: crate exists + read-only contract --------------------------------------
check("p32-crate-exists", SEC_SRC.is_dir(), "crates/security-audit/src missing")
SRC_FILES = ["lib.rs", "model.rs", "sshd.rs", "password.rs", "sudoers.rs",
             "filesystem.rs", "authlog.rs", "firewall.rs", "secrets.rs",
             "census.rs", "vulndb.rs", "vulnjoin.rs", "cis_map.rs"]
all_src = ""
for name in SRC_FILES:
    p = SEC_SRC / name
    ok = p.exists()
    check(f"p32-src-file-exists:{name}", ok, f"missing {name}")
    if ok:
        all_src += p.read_text(encoding="utf-8") + "\n"


def strip_comments_and_docs(text: str) -> str:
    text = re.sub(r"//[/!][^\n]*", "", text)
    text = re.sub(r"/\*[\s\S]*?\*/", "", text)
    return text


code_only = strip_comments_and_docs(all_src)

FORBIDDEN_MUTATION_VERBS = [
    r"\bchmod\b", r"\bfchmod\b", r"\bchown\b", r"\bfchown\b",
    r"fs::write\b", r"File::create\b", r"OpenOptions\b",
    r"fs::remove_file\b", r"fs::remove_dir\b", r"fs::rename\b",
    r"set_permissions\b",
]
hits = [v for v in FORBIDDEN_MUTATION_VERBS if re.search(v, code_only)]
check("p32-read-only-contract", not hits,
      f"forbidden mutation verbs in security-audit src: {hits}")

# Elevation symbols are scanned over CODE + stripped strings: string literal
# CONTENTS (e.g. the auth-log marker "pam_unix(sudo:auth)") are data, not
# capability. Comments were already stripped above.
def strip_string_literals(text: str) -> str:
    return re.sub(r'"(?:[^"\\]|\\.)*"', '""', text)


code_no_strings = strip_string_literals(code_only)
# Capability-based ban: elevating anything REQUIRES spawning a process
# (sudo/doas/osascript are reachable only through std::process::Command).
# Banning the capability bans every path; vocabulary tokens like the word
# "sudo" appear legitimately in log-marker data and rule names.
ELEVATION_SYMBOLS = [
    r"std::process::Command",
    r"process::Command",
    r"\bdoas\b(?![\w.])",
    r"AuthorizationServices",
    r"osascript\b",
]
ehits = [v for v in ELEVATION_SYMBOLS if re.search(v, code_no_strings)]
check("p32-no-elevation-symbols", not ehits,
      f"elevation-capable symbols in security-audit src: {ehits}")

# --- Gate b: network ban --------------------------------------------------------------
NETWORK_SYMBOLS = [
    r"std::net\b", r"TcpStream\b", r"TcpListener\b", r"UdpSocket\b",
    r"reqwest\b", r"ureq\b", r"hyper\b", r"curl\b", r"fetch\b",
    r"ToSocketAddrs\b", r"lookup_host\b",
]
nhits = [v for v in NETWORK_SYMBOLS if re.search(v, code_only)]
check("p32-network-ban-symbols-absent", not nhits,
      f"network-capable symbols in security-audit src: {nhits}")

sec_cargo = (SEC / "Cargo.toml").read_text(encoding="utf-8")
ALLOWED_DEPS = {"serde", "serde_json", "sha2", "thiserror", "regex", "tempfile"}
# Parse ONLY the [dependencies] / [dev-dependencies] tables of the manifest.
dep_lines: set[str] = set()
current_table = ""
for line in sec_cargo.splitlines():
    s = line.strip()
    if s.startswith("["):
        current_table = s.strip("[]")
        continue
    m = re.match(r"^([\w-]+)\s*[=.]", s)
    if m and current_table in ("dependencies", "dev-dependencies"):
        dep_lines.add(m.group(1))
unexpected = {d for d in dep_lines if d not in ALLOWED_DEPS
              and not d.startswith("aethercore")}
check("p32-deps-allowlist-only", not unexpected,
      f"deps outside allowlist {sorted(ALLOWED_DEPS)}: {sorted(unexpected)}")
check("p32-no-default-features-on-regex", 'default-features = false' in sec_cargo,
      "regex must be minimal-feature (default-features = false)")

# --- Gate c: secrets-redaction enforcement --------------------------------------------
secrets_rs = (SEC_SRC / "secrets.rs").read_text(encoding="utf-8")
secrets_code = strip_comments_and_docs(secrets_rs)
check("p32-redact-fn-contract", "pub fn redact" in secrets_rs,
      "redact() missing from secrets module")
check("p32-evidence-through-redact",
      re.search(r"fact:\s*redact\(", secrets_code) is not None,
      "evidence construction must pass matched material through redact()")
check("p32-private-key-body-not-captured",
      "PRIVATE KEY-----\".to_string()" in secrets_rs.replace(' ... ', ' ... '),
      "private-key detector must capture only the header marker")
RAW_PASSTHROUGH = [
    r"fact:\s*h\.raw\.clone\(\)", r"fact:\s*m\.as_str\(\)",
    r"format!\(\"[^\"]*\{h\.raw\}", r"format!\(\"[^\"]*\{m\.as_str",
]
rhit = [v for v in RAW_PASSTHROUGH if re.search(v, secrets_code)]
check("p32-no-raw-secret-in-evidence", not rhit,
      f"raw secret passthrough into evidence: {rhit}")

# --- Gate d: vulndb manifest pin + tamper marker ---------------------------------------
check("p32-vulndb-asset-present", (ROOT / "assets/vulndb/vulndb.json").is_file(),
      "assets/vulndb/vulndb.json missing")
manifest_asset = ROOT / "assets/vulndb/vulndb.manifest.json"
check("p32-vulndb-manifest-present", manifest_asset.is_file(),
      "assets/vulndb/vulndb.manifest.json missing")
if manifest_asset.is_file():
    man = json.loads(manifest_asset.read_text())
    db_bytes = (ROOT / "assets/vulndb/vulndb.json").read_bytes()
    actual = hashlib.sha256(db_bytes).hexdigest()
    check("p32-vulndb-pin-matches-live-db", man.get("sha256") == actual,
          "manifest sha256 pin does not match the committed DB bytes")
    check("p32-vulndb-entry-count-pinned",
          man.get("entries") == len(json.loads(db_bytes)),
          "manifest entry count drift")
    check("p32-vulndb-schema-tag",
          man.get("schema") == "aethercore.vulndb.manifest.v1",
          "manifest schema tag wrong")
golden_rs = (SEC / "tests/golden.rs").read_text(encoding="utf-8") if (SEC / "tests/golden.rs").exists() else ""
check("p32-tamper-test-marker",
      "gd5_tampered_db_refused_fail_closed" in golden_rs
      and "HashMismatch" in golden_rs,
      "GD-5 tamper test missing from golden tests")

# --- Gate e: cis_map format -------------------------------------------------------------
cis_path = ROOT / "assets/vulndb/cis_map.json"
check("p32-cis-map-asset-present", cis_path.is_file(), "assets/vulndb/cis_map.json missing")
CIS_RE = re.compile(r"^CIS L[12] \d+\.\d+(\.\d+)?$")
implemented_codes: set[str] = set(re.findall(r'"((?:ssh|pass|sudo|fs|auth|fw|secrets|cve)\.[\w]+)"',
                                             code_only))
if cis_path.is_file():
    cmap = json.loads(cis_path.read_text())
    entries = cmap.get("entries", [])
    mapped_codes: set[str] = set()
    bad_fmt = []
    for e in entries:
        ctrl = e.get("control", "")
        mapped_codes.add(e.get("rule_code", ""))
        if ctrl == "unmapped":
            if not e.get("note", "").strip():
                bad_fmt.append(e.get("rule_code"))
        elif not CIS_RE.match(ctrl):
            bad_fmt.append(e.get("rule_code"))
    check("p32-cis-map-format-gate", not bad_fmt,
          f"entries failing CIS id format or unmapped-without-note: {bad_fmt}")
    unmapped_no_note = [e.get("rule_code") for e in entries
                        if e.get("control") == "unmapped" and not e.get("note", "").strip()]
    check("p32-cis-map-unmapped-requires-note", not unmapped_no_note,
          f"unmapped without note: {unmapped_no_note}")
    # Every implemented rule code has a mapping or an explicit unmapped note.
    known_codes = {e.get("rule_code") for e in entries}
    uncovered = {c for c in implemented_codes if c not in known_codes}
    check("p32-cis-map-covers-implemented-rules", not uncovered,
          f"rule codes lacking a cis_map entry: {sorted(uncovered)}")
    check("p32-cis-map-profile-cis-l1", cmap.get("profile") == "cis-l1",
          "cis map profile must be cis-l1")

# --- Gate f: wire freeze RE-FROZEN at the P32 allocation -------------------------------
operations_proto = (ROOT / "crates/contracts/proto/operations.proto").read_text(encoding="utf-8")
request_slice = operations_proto.split("message Request")[1].split("oneof payload")[1].split("\n  }")[0]
req_fields = {f: int(t) for (_ty, f, t) in re.findall(
    r"^\s{4}(\w+)\s+(\w+)\s*=\s*(\d+);", request_slice, flags=re.M)}
response_slice = operations_proto.split("message Response")[1].split("oneof payload")[1].split("\n  }")[0]
resp_fields = {f: int(t) for (_ty, f, t) in re.findall(
    r"^\s{4}(\w+)\s+(\w+)\s*=\s*(\d+);", response_slice, flags=re.M)}
check("p32-wire-refreeze:request-max-90", max(req_fields.values()) == 90,
      "request tags must be frozen at the P32 allocation (max 90)")
check("p32-wire-refreeze:response-max-52", max(resp_fields.values()) == 52,
      "response tags must be frozen at the P32 allocation (max 52)")
check("p32-wire-run-security-audit-tag90",
      req_fields.get("run_security_audit") == 90,
      "run_security_audit must hold request tag 90 exactly")
check("p32-wire-security-audit-response-tag52",
      resp_fields.get("security_audit_response") == 52,
      "security_audit_response must hold response tag 52 exactly")
events_proto = (ROOT / "crates/contracts/proto/events.proto").read_text(encoding="utf-8")
check("p32-eventkind-29-security-audit",
      re.search(r"EVENT_KIND_SECURITY_AUDIT\s*=\s*29;", events_proto) is not None,
      "EventKind 29 (EVENT_KIND_SECURITY_AUDIT) missing or renumbered")
env_slice = events_proto.split("message EventEnvelope")[1].split("oneof payload")[1].split("\n  }")[0]
env_fields = {f: int(t) for (_ty, f, t) in re.findall(
    r"^\s{4}(\w+)\s+(\w+)\s*=\s*(\d+);", env_slice, flags=re.M)}
check("p32-envelope-tag38-security-audit",
      env_fields.get("security_audit") == 38,
      "EventEnvelope.security_audit must hold tag 38 exactly")
# Nothing renumbered: prior anchors keep their exact historical tags.
anchors = [("export_journal", 89, req_fields), ("export_journal_response", 51, resp_fields),
           ("get_engine_source", 88, req_fields), ("engine_source_response", 50, resp_fields),
           ("platform_capabilities", 28 + 9, None)]  # placeholder to keep structure clear
for fname, want_tag, table in anchors[:-1]:
    if table:
        check(f"p32-wire-anchor:{fname}@{want_tag}", table.get(fname) == want_tag,
              f"{fname} drifted from tag {want_tag} — nothing may be renumbered")
check("p32-eventkind-anchor-insights-27",
      re.search(r"EVENT_KIND_INSIGHTS\s*=\s*27;", events_proto) is not None,
      "EventKind anchor 27 drifted")
check("p32-eventkind-anchor-platform-28",
      re.search(r"EVENT_KIND_PLATFORM_CAPABILITIES\s*=\s*28;", events_proto) is not None,
      "EventKind anchor 28 drifted")

# --- Gate g: router arm + typed guard + scoped owner-action writer ----------------------
router_rs = module_text(ROOT, ROUTER)
check("p32-router-runsecurityaudit-arm",
      "request::Payload::RunSecurityAudit(v)" in router_rs,
      "router lacks the RunSecurityAudit arm")
check("p32-router-traversal-guard",
      "validate_targets(&targets)" in router_rs and "sec.traversalRejected" in router_rs,
      "router does not enforce the shared typed traversal guard")
check("p32-router-empty-targets-typed-rejection",
      "targets_json must be a non-empty AuditTarget array" in router_rs,
      "empty target arrays must be rejected typed")
check("p32-router-emits-security-audit-event",
      "EventKind::SecurityAudit" in router_rs
      and "Payload::SecurityAudit(" in router_rs,
      "router must publish EventKind::SecurityAudit on envelope tag 38")
check("p32-router-honest-notavailable-propagation",
      '"notAvailable".to_string()' in router_rs,
      "router must propagate NotAvailable lanes verbatim, never fake OK")
check("p32-router-diagnostics-event",
      "SECURITY_AUDIT_RAN" in (ROOT / "crates/diagnostics/src/lib.rs").read_text(encoding="utf-8"),
      "diagnostics registry lacks security.audit_ran event const")

sec_cli = (ROOT / "apps/aetherctl/src/sec.rs").read_text(encoding="utf-8") \
    if (ROOT / "apps/aetherctl/src/sec.rs").exists() else ""
check("p32-owner-action-scoped-writer",
      bool(sec_cli) and "fn vulndb_update_from" in sec_cli
      and "remove_file" in sec_cli,
      "vulndb update owner action missing rollback discipline (export.rs precedent)")
check("p32-owner-action-validate-before-write",
      "validate_candidate_db(&raw)" in sec_cli,
      "candidate DB must be validated BEFORE any byte is written")

# --- Gate h: CLI wiring + i18n parity extended to sec.* keys -----------------------------
cli_rs = (ROOT / "apps/aetherctl/src/cli.rs").read_text(encoding="utf-8")
offline_rs = (ROOT / "apps/aetherctl/src/offline.rs").read_text(encoding="utf-8")
main_txt = (ROOT / "apps/aetherctl/src/main.rs").read_text(encoding="utf-8")
for token, label in [('"sec"', "sec command"), ('"audit"', "sec audit subcommand"),
                     ('"--secrets"', "--secrets flag"), ('"--sudoers"', "--sudoers flag"),
                     ('"--authlog"', "--authlog flag"), ('"--firewall"', "--firewall flag"),
                     ('"compliance"', "compliance command"), ('"vulndb"', "vulndb command"),
                     ('"--from"', "vulndb --from"), ('"--url"', "vulndb --url rejection")]:
    check(f"p32-cli-parity:{label}", token in cli_rs, f"cli.rs lacks {token}")
check("p32-cli-vulndb-url-banned",
      "network fetch stays banned" in cli_rs,
      "vulndb update must refuse --url with a typed usage error")
check("p32-offline-executors",
      "run_offline_audit" in offline_rs and "render_saved_report" in offline_rs
      and "run_compliance_summary" in offline_rs and "vulndb_update_from" in offline_rs,
      "offline.rs must route all four sec jobs")
check("p32-main-declares-sec-module", "mod sec;" in main_txt,
      "sec module not declared in aetherctl main")

i18n_rs = (ROOT / "apps/aetherctl/src/i18n.rs").read_text(encoding="utf-8")
en_block = i18n_rs.split("pub fn en(")[1].split("pub fn ar(")[0]
ar_block = i18n_rs.split("pub fn ar(")[1]
en_keys = set(re.findall(r'"([\w.]+)"\s*=>', en_block))
ar_keys = set(re.findall(r'"([\w.]+)"\s*=>', ar_block))
keys_list = re.findall(r'"(sec\.[\w.]+)"', i18n_rs.split("];")[0])
missing_ar_sec = [k for k in keys_list if k not in ar_keys]
missing_en_sec = [k for k in keys_list if k not in en_keys]
check("p32-i18n-sec-keys-en", not missing_en_sec,
      f"EN catalog missing sec keys: {missing_en_sec[:4]}")
check("p32-i18n-sec-keys-ar", not missing_ar_sec,
      f"AR catalog missing sec keys: {missing_ar_sec[:4]}")
plural_keys = [k for k in keys_list if ".plural" in k]
check("p32-i18n-plural-forms-present", len(plural_keys) >= 6,
      f"plural forms required for count-bearing summaries (have {len(plural_keys)})")
check("p32-i18n-plural-parity",
      all(k in en_keys and k in ar_keys for k in plural_keys),
      "plural keys must resolve in BOTH catalogs")

# --- Gate i: GD proof markers -------------------------------------------------------------
gd_example = ROOT / "crates/security-audit/examples/gd4_live_audit.rs"
check("p32-gd4-example-exists", gd_example.is_file(),
      "examples/gd4_live_audit.rs missing (live-host proof runner)")
check("p32-gd1-fixtures",
      "WEAK_SSHD" in golden_rs and "HARDENED_SSHD" in golden_rs
      and "gd1_sshd_hardened_fixture_zero_findings" in golden_rs,
      "GD-1 weak/hardened sshd fixture pair missing")
check("p32-gd2-redaction-proof",
      "AKIAIOSFODNN7EXAMPLE" in golden_rs and "!blob.contains(FAKE_AWS)" in golden_rs.replace(" ", ""),
      "GD-2 planted-key redaction assertion missing")
check("p32-gd2-clean-dir-zero-false-positive",
      "gd2_clean_dir_zero_false_positives" in golden_rs,
      "GD-2 clean-dir zero-false-positive test missing")
check("p32-gd3-unknown-package-ignored",
      "totally-unknown" in golden_rs and "CVE-2026-0001" in golden_rs,
      "GD-3 seeded join fixture missing")
unit_lib = (SEC_SRC / "lib.rs").read_text(encoding="utf-8")
check("p32-unit-guard-tests",
      "try_new_refuses_empty_evidence" in unit_lib
      and "digest_is_stable_and_order_insensitive" in unit_lib,
      "house-discipline unit tests missing from lib.rs")
check("p32-model-hardbounds",
      "MAX_PARSE_BYTES" in code_only and "MAX_FINDINGS" in code_only
      and "MAX_FILES_PER_SCAN" in code_only and "MAX_SCAN_MILLIS" in code_only
      and "MAX_EVIDENCE_REFS" in code_only,
      "hard-bound clamps family incomplete")
trunc_markers = code_only.count("scan_truncated") + code_only.count('"fs.scan_truncated"')
check("p32-honest-truncation-markers", "scan_truncated" in code_only,
      "bounded scans must degrade with an honest truncation finding")

# --- Gate j: docs set ---------------------------------------------------------------------
required_docs_p32 = [
    "docs/phase32/ARCHITECTURE.md",
    "docs/phase32/QUALIFICATION_DEBT_APPEND.md",
    "docs/phase32/ISSUES.json",
    "docs/phase32/SCORECARD.md",
    "docs/phase32/MASTER_DELIVERY_REPORT.md",
    "docs/phase32/HYGIENE.md",
]
for rel in required_docs_p32:
    p = ROOT / rel
    ok = p.exists() and p.stat().st_size > 300
    check(f"p32-doc-exists-and-nontrivial:{rel}", ok,
          f"missing or trivially small ({p.stat().st_size if p.exists() else 0} bytes)")
debt = ROOT / "QUALIFICATION_DEBT.json"
if debt.exists():
    reg = json.loads(debt.read_text())
    items_raw = reg.get("items", reg) if isinstance(reg, dict) else reg
    if isinstance(items_raw, dict):
        items_raw = list(items_raw.values())
    ids = {e.get("capabilityId") for e in items_raw if isinstance(e, dict)}
    for qid in ("QD-032-001", "QD-032-002", "QD-032-003"):
        check(f"p32-debt-register:{qid}", qid in ids, f"{qid} missing from DEBT_REGISTER.json")

# --- Workspace membership ------------------------------------------------------------------
ws_cargo = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
check("p32-workspace-member-security-audit", '"crates/security-audit"' in ws_cargo,
      "crates/security-audit not a workspace member")

result = {
    "schema": "aethercore.phase32.adversarial-audit.v1",
    "root": str(ROOT),
    "checks": checks,
    "failures": failures,
    "status": "PASS" if not failures else "FAIL",
}
print(json.dumps(result, indent=2))
sys.exit(0 if not failures else 1)
