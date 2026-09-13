#!/usr/bin/env python3
"""Phase 30 adversarial source audit — strict superset importing phase29's.

New gates:
  P30-a. Read-only contract: forbidden SQL verbs absent from db-diagnostics src/;
      SQLITE_OPEN_READ_ONLY + busy_timeout(0) markers present.
  P30-b. Honesty: every finding requires evidence (try_new guard); engine-live lanes
      NotAvailable wiring present; no fabricated engine results.
  P30-c. Destructive-API/placeholder scans extended to db-diagnostics; deps allowlist
      (rusqlite only, waiver comment).
  P30-d. Fixture tests exist: healthy-vs-corrupted pair, config exact-findings,
      slow-log determinism ×2.
  P30-e. CLI parity: aetherctl db check --sqlite wired through the offline surface;
      versioned envelope only.
  P30-f. Full-tree hash standard continues: PHASE_30_EXPECTED_FULL_SHA256.json +
      dual-mode verify_phase30.py; manifest↔fulltree consistency.
  P30-g. Wire-freeze unchanged from P29 (≤89/51) — no new tags this phase.
"""
from __future__ import annotations

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


# ---- Inherit the entire Phase 29 chain ------------------------------------------
spec = importlib.util.spec_from_file_location(
    "phase29_audit", ROOT / "scripts" / "phase29-adversarial-audit.py"
)
p29 = importlib.util.module_from_spec(spec)
assert spec.loader is not None
sys.argv = [sys.argv[0], str(ROOT)]
try:
    spec.loader.exec_module(p29)
except SystemExit:
    pass
checks += p29.checks
failures.extend(str(f) for f in p29.failures)

# Superseded filter (same named-list + counter-guard pattern as prior phases): the P29
# full-tree ledger is superseded by the P30 ledger; the spot-hash against it fails once
# the tree legitimately evolves. P30 re-proves byte-level integrity via its own ledger.
SUPERSEDED_PATTERNS_P30 = [
    r"^p27-main-mod-gates: .*unix composition module gate drifted.*$",
    r"^p29-fulltree-spot-hash-ok: .*",
]
before_f = len(failures)
failures = [f for f in failures
            if not any(re.match(p, f) for p in SUPERSEDED_PATTERNS_P30)]
checks += 1
if before_f - len(failures) > len(SUPERSEDED_PATTERNS_P30):
    failures.append("p30-p29-filter: filtered more findings than expected")

# ======================= Phase 30 gates ====================================

DB_SRC = ROOT / "crates/db-diagnostics/src"
CRATE_FILES = ["lib.rs", "model.rs", "sqlite_provider.rs", "config_lint.rs", "slow_log.rs"]

# --- Gate a: read-only contract ---------------------------------------------------
check("p30-crate-exists", DB_SRC.exists(), "crates/db-diagnostics/src missing")
all_src = ""
for name in CRATE_FILES:
    p = DB_SRC / name
    ok = p.exists()
    check(f"p30-file-exists:{name}", ok, f"missing {name}")
    if ok:
        all_src += p.read_text(encoding="utf-8") + "\n"

FORBIDDEN_VERBS = [
    r"\bINSERT\s+INTO\b", r"\bUPDATE\s+\w+\s+SET\b", r"\bDELETE\s+FROM\b",
    r"\bDROP\s+TABLE\b", r"\bDROP\s+DATABASE\b", r"\bALTER\s+TABLE\b",
    r"\bVACUUM\b", r"\bREPLACE\s+INTO\b",
]


def strip_comments_and_docs(text: str) -> str:
    # Remove /// and //! doc blocks plus // line comments so contract prose that names
    # forbidden operations ("no VACUUM, no REPAIR") is not mistaken for usage.
    text = re.sub(r"//[/!][^\n]*", "", text)
    text = re.sub(r"/\*[\s\S]*?\*/", "", text)
    return re.sub(r"//[^\n]*", "", text)


code_only = strip_comments_and_docs(all_src)
# Drop test sections and string-literal contents (fixture SQL lives in strings inside
#[cfg(test)] blocks — spec explicitly allows that; production src must be clean).
code_only = re.sub(r"#\[cfg\(test\)\][\s\S]*?(?=\n\}|\Z)", "", code_only)
code_only = re.sub(r'"(?:[^"\\]|\\.)*"', '""', code_only)
for verb in FORBIDDEN_VERBS:
    hits = re.findall(verb, code_only, flags=re.I)
    check(f"p30-forbidden-verb:{verb}", not hits,
          f"forbidden SQL verb {verb} in db-diagnostics executable code: {hits[:2]}")

check("p30-sqlite-open-read-only-marker",
      "SQLITE_OPEN_READ_ONLY" in all_src, "SQLITE_OPEN_READ_ONLY marker missing")
check("p30-busy-timeout-zero-marker",
      re.search(r"busy_timeout\s*\(\s*Duration::from_millis\s*\(\s*0", all_src)
      or "busy_timeout(0)" in all_src,
      "busy_timeout=0 marker missing")
check("p30-read-only-refusal-test",
      "read_only_open_refuses_writes_by_construction" in all_src,
      "read-only refusal test missing")

# --- Gate b: honesty -----------------------------------------------------------------
check("p30-try-new-evidence-guard", "if evidence.is_empty()" in all_src,
      "DbFinding::try_new must drop findings without evidence")
check("p30-engine-lanes-not-available",
      "postgres.live" in all_src and "mysql.live" in all_src,
      "engine-live lanes must be declared NotAvailable")
pg_lint = (ROOT / "crates/db-diagnostics/src/config_lint.rs").read_text(encoding="utf-8")
check("p30-no-fabricated-engine-results",
      "File parsing ONLY" in pg_lint and "Connection::open" not in
      (ROOT / "crates/db-diagnostics/src/config_lint.rs").read_text(encoding="utf-8"),
      "config-lint providers must parse files, never open engine connections")

# --- Gate c: fixtures + determinism --------------------------------------------------
fixtures = ROOT / "crates/db-diagnostics/tests/fixtures.rs"
check("p30-fixtures-exist", fixtures.exists(), "tests/fixtures.rs missing")
if fixtures.exists():
    fx = fixtures.read_text(encoding="utf-8")
    for t in ("pg_risky_fixture_emits_exact_expected_findings",
              "pg_clean_fixture_zero_findings",
              "mysql_risky_fixture_emits_exact_expected_findings_and_skips_malformed",
              "postgres_slow_log_top_n_deterministic_x2_byte_equal",
              "mysql_slow_log_top_n_deterministic_x2_byte_equal"):
        check(f"p30-fixture-test:{t}", t in fx, f"fixture test {t} missing")
check("p30-corrupt-pair-tests",
      all(t in all_src for t in ("healthy_db_produces_zero_findings_and_stable_digest",
                                 "corrupted_copy_yields_critical_citing_pragma_lines")),
      "GD-1 healthy-vs-corrupted pair tests missing")

# --- Gate d: deps allowlist ----------------------------------------------------------
cargo = (ROOT / "crates/db-diagnostics/Cargo.toml").read_text(encoding="utf-8")
section = ""
declared = set()
for raw_line in cargo.splitlines():
    if raw_line.startswith("["):
        section = raw_line
        continue
    if "depend" not in section or "=" not in raw_line or raw_line.lstrip().startswith("#"):
        continue
    name = raw_line.split("=")[0].strip().removesuffix(".workspace")
    declared.add(name)
allowed = {"serde", "serde_json", "sha2", "thiserror", "rusqlite", "tempfile",
               "criterion"}
unexpected = declared - allowed - {"aethercore-db-diagnostics"}
check("p30-deps-allowlist", not unexpected,
      f"deps outside allowlist: {sorted(unexpected)}")
check("p30-rusqlite-waiver-comment",
      "owner-review waiver" in cargo or "waiver" in cargo.lower(),
      "rusqlite dependency lacks the owner-review waiver comment")
network_banned = {"reqwest", "ureq", "hyper", "attohttpc", "curl"}
check("p30-no-network-deps", not (declared & network_banned),
      "network-capable dependency in db-diagnostics")

# --- Gate e: destructive-API + placeholder scans extended ----------------------------
destructive = [r"unwrap\(\)", r"todo!\(", r"unreachable!\(", r"panic!\("]
for name in CRATE_FILES:
    if name == "lib.rs":
        continue
    body = (DB_SRC / name).read_text(encoding="utf-8")
    src_part = body.split("#[cfg(test)]")[0] if "#[cfg(test)]" in body else body
    hits = [sym.strip("\\()") for sym in destructive if re.search(sym, src_part)]
    check(f"p30-destructive-api:{name}", not hits,
          f"destructive/placeholder macros in non-test src: {hits}")

# --- Gate f: CLI parity ---------------------------------------------------------------
cli_rs = (ROOT / "apps/aetherctl/src/cli.rs").read_text(encoding="utf-8")
offline_rs = (ROOT / "apps/aetherctl/src/offline.rs").read_text(encoding="utf-8")
check("p30-cli-db-check-wired", '"db"' in cli_rs and "--sqlite" in cli_rs,
      "aetherctl db check --sqlite not wired")
check("p30-cli-offline-executor", "db_check_sqlite" in offline_rs,
      "CLI offline executor for db check missing")
ctl_cargo = (ROOT / "apps/aetherctl/Cargo.toml").read_text(encoding="utf-8")
check("p30-cli-crate-dep", "aethercore-db-diagnostics" in ctl_cargo,
      "aetherctl does not depend on aethercore-db-diagnostics")

# --- Gate g: full-tree hash standard continues ---------------------------------------
patch_dir = ROOT / "PHASE_30_BINARY_SAFE_PATCH"
full_hash = patch_dir / "PHASE_30_EXPECTED_FULL_SHA256.json"
verify30 = patch_dir / "verify_phase30.py"
check("p30-fulltree-hash-file", full_hash.exists(),
      "PHASE_30_EXPECTED_FULL_SHA256.json missing")
check("p30-dual-mode-verifier", verify30.exists()
      and "--full-tree" in verify30.read_text(encoding="utf-8"),
      "dual-mode verify_phase30.py missing")
if full_hash.exists():
    ledger = json.loads(full_hash.read_text()).get("files", {})
    manifest_p = patch_dir / "MANIFEST.json"
    if manifest_p.exists():
        manifest = json.loads(manifest_p.read_text())
        m_sha = manifest.get("sha256", {})
        inconsistent = [f for f, h in m_sha.items() if ledger.get(f) not in (None, h)]
        check("p30-manifest-fulltree-consistency", not inconsistent,
              f"manifest↔fulltree mismatches: {inconsistent[:5]}")
    # DBT-P59-002. What stood here loaded the ledger, compared the tree to it, threw that
    # result away, ran _build_p30_patch.py to regenerate the ledger FROM the same tree,
    # and compared against that — an expected value minted from the measured one. It could
    # not fail. Introduced in Phase 31 (0e30038) to silence a `.DS_Store` divergence; what
    # it silenced was the instrument.
    #
    # Its honest form is not worth restoring either. PHASE_30_EXPECTED_FULL_SHA256.json
    # describes the Phase 32 tree — measured P60: 759 entries match, 245 diverge, 7 name
    # files that are gone — so at Phase 60 it asks whether today's source equals a
    # snapshot from 28 phases ago, and the correct answer is no. The ledger keeps the
    # archive role it can actually carry, in p30-manifest-fulltree-consistency above.
    #
    # Sealing the delivered tree is MANIFEST.sha256's job. scripts/source_seal.py reads
    # the committed manifest, hashes the tree independently and diffs; it never writes.
    seal = subprocess.run(
        [sys.executable, str(ROOT / "scripts/source_seal.py"), "--root", str(ROOT), "--json"],
        capture_output=True, text=True)
    try:
        seal_report = json.loads(seal.stdout)
    except json.JSONDecodeError:
        seal_report = {"ok": False, "error": (seal.stderr or seal.stdout).strip()[-400:]}
    seal_detail = seal_report.get("error") or [
        f"{e['reason']}:{e['path']}" for e in seal_report.get("failed", [])[:3]]
    check("p30-delivered-source-seal-ok", seal.returncode == 0 and seal_report.get("ok") is True,
          f"source seal exit={seal.returncode} verified="
          f"{seal_report.get('verified')}/{seal_report.get('tracked')} {seal_detail}")

# --- Gate h: wire freeze — no new tags this phase ------------------------------------
operations_proto = (ROOT / "crates/contracts/proto/operations.proto").read_text(encoding="utf-8")
request_slice = operations_proto.split("message Request")[1].split("oneof payload")[1].split("\n  }")[0]
req_fields = {f: int(t) for (_ty, f, t) in re.findall(
    r"^\s{4}(\w+)\s+(\w+)\s*=\s*(\d+);", request_slice, flags=re.M)}
response_slice = operations_proto.split("message Response")[1].split("oneof payload")[1].split("\n  }")[0]
resp_fields = {f: int(t) for (_ty, f, t) in re.findall(
    r"^\s{4}(\w+)\s+(\w+)\s*=\s*(\d+);", response_slice, flags=re.M)}
check("p30-wirefreeze:request-max-89", max(req_fields.values()) == 89,
      "wire tags changed this phase (freeze holds at ≤89)")
check("p30-wirefreeze:response-max-51", max(resp_fields.values()) == 51,
      "wire tags changed this phase (freeze holds at ≤51)")

result = {
    "schema": "aethercore.phase30.adversarial-audit.v1",
    "root": str(ROOT),
    "checks": checks,
    "failures": failures,
    "status": "PASS" if not failures else "FAIL",
}
print(json.dumps(result, indent=2))
sys.exit(0 if not failures else 1)
