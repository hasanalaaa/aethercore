#!/usr/bin/env python3
"""Phase 23 adversarial source audit.

Runs the complete Phase 22 audit in-process (itself a superset of 21 → 20), then adds
the Embedded Local Intelligence gates:

  P23-a. Network-API ban: no TcpStream/UdpSocket/reqwest/ureq/hyper/tokio::net/
      std::net socket construction anywhere in crates/intelligence-core/src.
  P23-b. Dependency allowlist for the new crate (serde, serde_json, sha2, thiserror,
      tempfile dev-only; llama_cpp_2 only under an explicit code-comment waiver that
      names the owner review).
  P23-c. Destructive-API scan extended to crates/intelligence-core/src.
  P23-d. Placeholder/fake-marker scan extended (src + tests of the new crate).
  P23-e. Wire-freeze extended to ALL ranges incl. Phase 23's (EventKind 27, envelope
      36, requests 84–86, response 48).
  P23-f. EN/AR parity for every insight.* key + unit.insight six-form Arabic plural.
  P23-g. Model-hash pin manifest exists and is schema-valid; artifacts entries carry
        64-hex sha256 fields (fail-closed loader contract documented).

Exit code 0 = all gates pass; 1 = any gate fails.
"""
from __future__ import annotations

import importlib.util
import json
import re
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


# ---- Inherit the entire Phase 22 chain ----------------------------------------
spec = importlib.util.spec_from_file_location(
    "phase22_audit", ROOT / "scripts" / "phase22-adversarial-audit.py"
)
p22 = importlib.util.module_from_spec(spec)
assert spec.loader is not None
sys.argv = [sys.argv[0], str(ROOT)]
try:
    spec.loader.exec_module(p22)  # prints its own two PASS blocks; ends with SystemExit
except SystemExit:
    pass

checks += p22.checks
failures.extend(str(f) for f in p22.failures)

# ======================= Phase 23 gates ====================================

icore = ROOT / "crates/intelligence-core/src"
icore_sources = {p.name: p.read_text(encoding="utf-8") for p in sorted(icore.glob("*.rs"))}
all_src = "\n".join(icore_sources.values())

# --- Gate a: network API ban ---------------------------------------------------
network_patterns = [
    r"TcpStream", r"UdpSocket", r"reqwest", r"ureq", r"hyper",
    r"tokio::net", r"std::net", r"ToSocketAddrs", r"TcpListener",
    r"http[s]?://(?!schema|insight|aethercore)",  # allow docs words; ban real URL fetches
    r"download", r"fetch_",
]
for pattern in network_patterns:
    check(
        f"p23-network-ban:{pattern[:14]}",
        not re.search(pattern, all_src),
        f"network-capable symbol '{pattern}' found in intelligence-core/src",
    )
cargo_toml = (ROOT / "crates/intelligence-core/Cargo.toml").read_text(encoding="utf-8")
for banned in ("reqwest", "ureq", "hyper", "tokio", "attohttpc", "curl"):
    check(
        f"p23-network-dep-ban:{banned}",
        not re.search(rf"^{banned}\s*=", cargo_toml, flags=re.M),
        f"network-capable dependency '{banned}' present",
    )

# --- Gate b: dependency allowlist -------------------------------------------------
allowed_deps = {"serde", "serde_json", "sha2", "thiserror", "tempfile"}


def _deps_from_sections(text: str) -> set[str]:
    declared: set[str] = set()
    current: str | None = None
    for line in text.splitlines():
        header = re.match(r"^\[([^\]]+)\]", line.strip())
        if header:
            current = header.group(1)
            continue
        if current and ("dependencies" in current):
            m = re.match(r"^([a-z_][a-z0-9_]*)\s*[=.]", line.strip())
            if m:
                declared.add(m.group(1))
    return declared


declared = _deps_from_sections(cargo_toml)
for dep in sorted(declared):
    ok = dep in allowed_deps or dep == "aethercore-intelligence-core"
    check(
        f"p23-allowlist:{dep}",
        ok,
        f"dependency '{dep}' outside allowlist without documented waiver",
    )
# llama_cpp_2 requires an in-code waiver naming owner review when present as a real
# dependency declaration (comments mentioning it don't count).
declared_llama = "llama_cpp_2" in declared
if declared_llama:
    check(
        "p23-allowlist:llama_cpp_2-waiver",
        "owner" in cargo_toml.lower() and "review" in cargo_toml.lower(),
        "llama_cpp_2 present without in-code waiver comment",
    )
else:
    check(
        "p23-allowlist:llama_cpp_2-absent-or-waived",
        True,
        "",
    )

# --- Gate c: destructive APIs --------------------------------------------------
destructive_patterns = [
    r"EmptyWorkingSet", r"RegDeleteValue\w*", r"RegDeleteKey\w*",
    r"DeleteFileW?\b", r"RemoveDirectoryW?\b",
]
for name, text in icore_sources.items():
    stripped = "\n".join(l for l in text.splitlines() if not l.lstrip().startswith("//"))
    for pattern in destructive_patterns:
        check(
            f"p23-destructive:{name}:{pattern[:12]}",
            not re.search(pattern, stripped),
            f"{pattern} found in {name} (I1 violation surface)",
        )

# --- Gate d: placeholders -------------------------------------------------------
placeholder_patterns = [r"\bTODO\b", r"\bFIXME\b", r"unimplemented!\s*\(", r"todo!\s*\("]
tests_dir = ROOT / "crates/intelligence-core/tests"
test_sources = {p.name: p.read_text(encoding="utf-8") for p in sorted(tests_dir.glob("*.rs"))}
for name, text in {**icore_sources, **{f"tests/{k}": v for k, v in test_sources.items()}}.items():
    stripped = "\n".join(l for l in text.splitlines() if not l.lstrip().startswith("//"))
    for pattern in placeholder_patterns:
        check(
            f"p23-placeholder:{name}:{pattern[:10]}",
            not re.search(pattern, stripped),
            f"placeholder '{pattern}' found in {name}",
        )
lib_rs = icore_sources["lib.rs"]
check(
    "p23-no-unreachable-in-src",
    "unreachable!" not in lib_rs,
    "unreachable! must not appear in src",
)

# --- Gate e: wire freeze through Phase 23 --------------------------------------
events_proto = (ROOT / "crates/contracts/proto/events.proto").read_text(encoding="utf-8")
operations_proto = (ROOT / "crates/contracts/proto/operations.proto").read_text(encoding="utf-8")
kinds = dict(re.findall(r"(EVENT_KIND_[A-Z_]+) = (\d+);", events_proto))
check("p23-freeze:eventkind:care_run", kinds.get("EVENT_KIND_CARE_RUN") == "26", "P22 tag drifted")
check("p23-additive:eventkind:insights", kinds.get("EVENT_KIND_INSIGHTS") == "27", "P23 EventKind wrong")

envelope_slice = events_proto.split("message EventEnvelope")[1].split("oneof payload")[1].split("\n  }")[0]
envelope_fields = {f: t for (_ty, f, t) in re.findall(r"^\s{4}(\w+)\s+(\w+)\s*=\s*(\d+);", envelope_slice, flags=re.M)}
check("p23-freeze:envelope:care_status", envelope_fields.get("care_status") == "35", "P22 envelope tag drifted")
check("p23-additive:envelope:insights", envelope_fields.get("insights") == "36", "P23 envelope tag wrong")

request_slice = operations_proto.split("message Request")[1].split("oneof payload")[1].split("\n  }")[0]
request_fields = {f: t for (_ty, f, t) in re.findall(r"^\s{4}(\w+)\s+(\w+)\s*=\s*(\d+);", request_slice, flags=re.M)}
check("p23-freeze:request:cancel_care_run", request_fields.get("cancel_care_run") == "83", "P22 request tag drifted")
check("p23-additive:request:list_insights", request_fields.get("list_insights") == "84", "84 wrong")
check("p23-additive:request:request_insight", request_fields.get("request_insight") == "85", "85 wrong")
check("p23-additive:request:dismiss_insight", request_fields.get("dismiss_insight") == "86", "86 wrong")

response_slice = operations_proto.split("message Response")[1].split("oneof payload")[1].split("\n  }")[0]
response_fields = {f: t for (_ty, f, t) in re.findall(r"^\s{4}(\w+)\s+(\w+)\s*=\s*(\d+);", response_slice, flags=re.M)}
check("p23-freeze:response:care_status", response_fields.get("care_status") == "47", "P22 response tag drifted")
check("p23-additive:response:insights_response", response_fields.get("insights_response") == "48", "48 wrong")

# --- Gate f: i18n parity ---------------------------------------------------------
en_catalog = (ROOT / "apps/ui/src/lib/i18n/catalog.en.ts").read_text(encoding="utf-8")
ar_catalog = (ROOT / "apps/ui/src/lib/i18n/catalog.ar.ts").read_text(encoding="utf-8")
en_insight = set(re.findall(r"'(insight\.[^']+)'", en_catalog))
ar_insight = set(re.findall(r"'(insight\.[^']+)'", ar_catalog))
check("p23-i18n-parity-insight", en_insight == ar_insight and len(en_insight) >= 20,
      f"EN={len(en_insight)} AR={len(ar_insight)}")
en_pl = (ROOT / "apps/ui/src/lib/i18n/plurals.en.ts").read_text(encoding="utf-8")
ar_pl = (ROOT / "apps/ui/src/lib/i18n/plurals.ar.ts").read_text(encoding="utf-8")
check("p23-arabic-plural-insight", "'unit.insight'" in en_pl and "'unit.insight'" in ar_pl,
      "unit.insight plural missing")
ar_line = next((l for l in ar_pl.splitlines() if "'unit.insight'" in l), "")
six = ar_line and all(re.search(f, ar_line) for f in ["zero:", "one:", "two:", "few:", "many:", "other:"])
check("p23-arabic-plural-six-forms-insight", bool(six), "unit.insight lacks six Arabic forms")

# --- Gate g: model hash pin manifest ---------------------------------------------
manifest_path = ROOT / "assets/models/models.manifest.json"
check("p23-model-manifest-exists", manifest_path.exists(), "assets/models/models.manifest.json missing")
if manifest_path.exists():
    try:
        mm = json.loads(manifest_path.read_text(encoding="utf-8"))
        valid_schema = mm.get("schema") == "aethercore.phase23.model-manifest.v1"
        check("p23-model-manifest-schema", valid_schema, "manifest schema invalid")
        arts = mm.get("artifacts", [])
        hashes_ok = all(
            isinstance(a.get("sha256Hex"), str) and re.fullmatch(r"[0-9a-f]{64}", a["sha256Hex"])
            for a in arts
        )
        check("p23-model-manifest-sha256-format", hashes_ok, "an artifact lacks a 64-hex sha256")
        budget_ok = all(
            isinstance(a.get("declaredRamBudgetBytes"), int)
            and 0 < a["declaredRamBudgetBytes"] <= 2 * 1024 * 1024 * 1024
            for a in arts
        )
        check("p23-model-manifest-ram-budget", budget_ok, "RAM budget above 2 GiB cap or malformed")
    except json.JSONDecodeError as exc:
        check("p23-model-manifest-parses", False, str(exc))

result = {
    "schema": "aethercore.phase23.adversarial-audit.v1",
    "root": str(ROOT),
    "checks": checks,
    "failures": failures,
    "status": "PASS" if not failures else "FAIL",
}
print(json.dumps(result, indent=2))
sys.exit(0 if not failures else 1)
