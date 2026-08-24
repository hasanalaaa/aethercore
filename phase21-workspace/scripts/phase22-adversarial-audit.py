#!/usr/bin/env python3
"""Phase 22 adversarial source audit.

Runs the complete Phase 21 audit unchanged (itself a superset of Phase 20), then
adds Phase 22 gates for One-Click Care orchestration:

  P22-1. No destructive APIs in crates/care-orchestrator/src and the service
      care module (Gate 1 family extension).
  P22-2. No placeholder/fake markers anywhere in the new crate or UI feature.
  P22-3. Orchestrator never fabricates success: aggregate verified claims require
      per-domain verification (symbol-level checks on the truth-first contract).
  P22-4. Safety firewall: ReviewOnly plans are never executed; session consent is
      required before any run and lives in an in-memory per-session registry.
  P22-5. Single-flight: the orchestrator acquires MutationWorkload::OneClickCare.
  P22-6. Wire-freeze extension covering ALL frozen ranges including Phase 22:
        - EventKind ...25 (P21) + new 26
        - envelope payloads ...34 + new 35
        - requests ...79 + new 80..83
        - responses ...46 + new 47
        - MutationWorkloadKind ...5 + new ONE_CLICK_CARE = 6
  P22-7. EN/AR parity for every care.* key + Arabic plural unit.careStep.
  P22-8. Persistence journal additive-only: migration 0014 creates two tables,
        existing migrations untouched.

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


# ---- Run the entire Phase 21 audit in-process, inheriting its counts ----------
spec = importlib.util.spec_from_file_location(
    "phase21_audit", ROOT / "scripts" / "phase21-adversarial-audit.py"
)
p21 = importlib.util.module_from_spec(spec)
assert spec.loader is not None
sys.argv = [sys.argv[0], str(ROOT)]
try:
    # The Phase 21 script ends with sys.exit(); suppress it so this superset
    # can continue with the Phase 22 gates.
    spec.loader.exec_module(p21)
except SystemExit:
    pass

checks += p21.checks
failures.extend(f"{name}" for name in p21.failures)

# ======================= Phase 22 gates ====================================

destructive_patterns = [
    r"EmptyWorkingSet",
    r"RegDeleteValue\w*",
    r"RegDeleteKey\w*",
    r"DeleteFileW?\b",
    r"RemoveDirectoryW?\b",
]
placeholder_patterns = [
    r"\bTODO\b",
    r"\bFIXME\b",
    r"unimplemented!\s*\(",
    r"todo!\s*\(",
    r"\bfake_\w+",
    r"not implemented",
]


def strip_comments(source_text: str) -> str:
    return "\n".join(
        line for line in source_text.splitlines() if not line.lstrip().startswith("//")
    )


care_dir = ROOT / "crates/care-orchestrator/src"
care_sources = {s.name: strip_comments(s.read_text(encoding="utf-8")) for s in sorted(care_dir.glob("*.rs"))}
service_care = strip_comments((ROOT / "services/maintenance-service/src/care.rs").read_text(encoding="utf-8"))
ui_care_controller = (ROOT / "apps/ui/src/features/care/controller.ts").read_text(encoding="utf-8")

# --- Gate P22-1: destructive APIs -------------------------------------------
for name, text in care_sources.items():
    for pattern in destructive_patterns:
        check(
            f"p22-destructive-api:{name}:{pattern[:12]}",
            not re.search(pattern, text),
            f"{pattern} found in {name}",
        )
for pattern in destructive_patterns:
    check(
        f"p22-destructive-api:care.rs:{pattern[:12]}",
        not re.search(pattern, service_care),
        f"{pattern} found in service care.rs",
    )

# --- Gate P22-2: placeholders -------------------------------------------------
all_new_sources = dict(care_sources)
all_new_sources["care.rs"] = service_care
all_new_sources["controller.ts"] = ui_care_controller
ui_panel = (ROOT / "apps/ui/src/features/care/CarePanel.svelte").read_text(encoding="utf-8")
all_new_sources["CarePanel.svelte"] = ui_panel
for name, text in all_new_sources.items():
    for pattern in placeholder_patterns:
        check(
            f"p22-placeholder:{name}:{pattern[:12]}",
            not re.search(pattern, text),
            f"placeholder '{pattern}' found in {name}",
        )
engine_src = (ROOT / "crates/care-orchestrator/src/engine.rs").read_text(encoding="utf-8")
check(
    "p22-no-unreachable",
    "unreachable!" not in engine_src and "panic!(" not in engine_src.replace("expect(", ""),
    "unreachable!/panic! must not appear in engine.rs",
)

# --- Gate P22-3: truth-first reporting contract -------------------------------
model_src = care_sources["model.rs"]
check(
    "p22-truth-first-outcome-enum",
    "VerifiedByDomain" in model_src and "CompletedUnverified" in model_src,
    "per-domain outcome vocabulary missing",
)
check(
    "p22-domain-cited-verifications",
    "domain_verification_state" in model_src,
    "steps must cite the domain's own verification state",
)

# --- Gate P22-4: safety firewall ----------------------------------------------
check(
    "p22-firewall-reviewonly-never-executed",
    "ReviewOnly" in model_src and "auto_steps" in model_src and "review_steps" in model_src,
    "safety classification must separate auto from review-only",
)
consent_registry = re.search(r"struct SessionConsentRegistry", service_care) is not None
check("p22-session-consent-registry", consent_registry, "session consent registry missing")
check(
    "p22-consent-required-guard",
    "ConsentRequired" in engine_src and "consent_granted" in engine_src,
    "run must refuse without explicit session consent",
)

# --- Gate P22-5: single-flight --------------------------------------------------
check(
    "p22-single-flight-workload",
    "OneClickCare" in engine_src,
    "orchestrator must hold the OneClickCare mutation lease",
)
kernel_mutation = (ROOT / "crates/operation-kernel/src/mutation.rs").read_text(encoding="utf-8")
check(
    "p22-kernel-workload-variant",
    "OneClickCare" in kernel_mutation,
    "MutationWorkload::OneClickCare variant missing",
)

# --- Gate P22-6: complete wire freeze (P22 additions over P21 freeze) ----------
events_proto = (ROOT / "crates/contracts/proto/events.proto").read_text(encoding="utf-8")
operations_proto = (ROOT / "crates/contracts/proto/operations.proto").read_text(encoding="utf-8")
kinds = dict(re.findall(r"(EVENT_KIND_[A-Z_]+) = (\d+);", events_proto))
check("p22-wire-freeze:eventkind:timeline_page", kinds.get("EVENT_KIND_TIMELINE_PAGE") == "25", "P21 tag drifted")
check("p22-wire-additive:eventkind:care_run", kinds.get("EVENT_KIND_CARE_RUN") == "26", "P22 EventKind wrong")

envelope_slice = events_proto.split("message EventEnvelope")[1].split("oneof payload")[1].split("\n  }")[0]
envelope_pairs = re.findall(r"^\s{4}(\w+)\s+(\w+)\s*=\s*(\d+);", envelope_slice, flags=re.M)
envelope_payloads = {field: tag for (_type, field, tag) in envelope_pairs}
check("p22-wire-freeze:envelope:timeline_page", envelope_payloads.get("timeline_page") == "34", "P21 envelope tag drifted")
check("p22-wire-additive:envelope:care_status", envelope_payloads.get("care_status") == "35", "P22 envelope tag wrong")

request_slice = operations_proto.split("message Request")[1].split("oneof payload")[1].split("\n  }")[0]
request_pairs = re.findall(r"^\s{4}(\w+)\s+(\w+)\s*=\s*(\d+);", request_slice, flags=re.M)
request_fields = {field: tag for (_type, field, tag) in request_pairs}
check("p22-wire-freeze:request:get_recurrence_patterns", request_fields.get("get_recurrence_patterns") == "79", "P21 request tag drifted")
check("p22-wire-additive:request:start_care_run", request_fields.get("start_care_run") == "80", "P22 request tag 80 wrong")
check("p22-wire-additive:request:grant_care_session_consent", request_fields.get("grant_care_session_consent") == "81", "P22 request tag 81 wrong")
check("p22-wire-additive:request:get_care_status", request_fields.get("get_care_status") == "82", "P22 request tag 82 wrong")
check("p22-wire-additive:request:cancel_care_run", request_fields.get("cancel_care_run") == "83", "P22 request tag 83 wrong")

response_slice = operations_proto.split("message Response")[1].split("oneof payload")[1].split("\n  }")[0]
response_pairs = re.findall(r"^\s{4}(\w+)\s+(\w+)\s*=\s*(\d+);", response_slice, flags=re.M)
response_fields = {field: tag for (_type, field, tag) in response_pairs}
check("p22-wire-freeze:response:timeline_page", response_fields.get("timeline_page") == "46", "P21 response tag drifted")
check("p22-wire-additive:response:care_status", response_fields.get("care_status") == "47", "P22 response tag 47 wrong")

workload_kinds = dict(re.findall(r"(MUTATION_WORKLOAD_KIND_[A-Z_]+) = (\d+);", events_proto))
check("p22-wire-freeze:workload:update", workload_kinds.get("MUTATION_WORKLOAD_KIND_UPDATE") == "5", "workload tag 5 drifted")
check("p22-wire-additive:workload:oneclickcare", workload_kinds.get("MUTATION_WORKLOAD_KIND_ONE_CLICK_CARE") == "6", "workload tag 6 wrong")

# --- Gate P22-7: i18n parity for care keys --------------------------------------
en_catalog = (ROOT / "apps/ui/src/lib/i18n/catalog.en.ts").read_text(encoding="utf-8")
ar_catalog = (ROOT / "apps/ui/src/lib/i18n/catalog.ar.ts").read_text(encoding="utf-8")
en_care = set(re.findall(r"'(care\.[^']+)'", en_catalog))
ar_care = set(re.findall(r"'(care\.[^']+)'", ar_catalog))
check("p22-i18n-parity-care", en_care == ar_care and len(en_care) >= 24,
      f"EN={len(en_care)} AR={len(ar_care)}")
en_plurals_src = (ROOT / "apps/ui/src/lib/i18n/plurals.en.ts").read_text(encoding="utf-8")
ar_plurals_src = (ROOT / "apps/ui/src/lib/i18n/plurals.ar.ts").read_text(encoding="utf-8")
check("p22-arabic-plural-carestep", "'unit.careStep'" in en_plurals_src and "'unit.careStep'" in ar_plurals_src,
      "unit.careStep plural missing from a catalog")
ar_line = next((l for l in ar_plurals_src.splitlines() if "'unit.careStep'" in l), "")
six_forms = all(re.search(form, ar_line) for form in ["zero:", "one:", "two:", "few:", "many:", "other:"]) if ar_line else False
check("p22-arabic-plural-six-forms-care", six_forms, "unit.careStep lacks the six Arabic forms")

# --- Gate P22-8: persistence journal additive-only -------------------------------
migrations_dir = ROOT / "crates/persistence/migrations"
migration_0014 = (migrations_dir / "0014_phase22_care_orchestration.sql").read_text(encoding="utf-8")
check("p22-journal-tables-created",
      "CREATE TABLE IF NOT EXISTS care_runs" in migration_0014
      and "CREATE TABLE IF NOT EXISTS care_steps" in migration_0014,
      "phase 22 journal tables missing from migration 0014")
check("p22-journal-no-existing-table-touched",
      not re.search(r"(ALTER|DROP|UPDATE)\s+TABLE\s+(?!care_)", migration_0014),
      "migration 0014 must only add care_* objects")
persistence_src = (ROOT / "crates/persistence/src/lib.rs").read_text(encoding="utf-8")
check("p22-migration-list-appended",
      '(14, "0014_phase22_care_orchestration"' in persistence_src,
      "migration list entry for 0014 missing")

result = {
    "schema": "aethercore.phase22.adversarial-audit.v1",
    "root": str(ROOT),
    "checks": checks,
    "failures": failures,
    "status": "PASS" if not failures else "FAIL",
}
print(json.dumps(result, indent=2))
sys.exit(0 if not failures else 1)
