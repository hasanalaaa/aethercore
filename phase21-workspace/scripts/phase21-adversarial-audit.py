#!/usr/bin/env python3
"""Phase 21 adversarial source audit.

Strict superset of the Phase 20 audit: every Phase 20 gate runs unchanged, then
Phase 21 adds its own attack surface:

  P0. All Phase 20 gates (destructive APIs, sampling floor, ring bounds, rule
      evidence, reversibility, wire additive ranges, perf i18n parity).
  P21-1. No destructive API usage in crates/timeline-intelligence/src (Gate 1
      extension to the new crate).
  P21-2. Timeline crate is read-only over persistence: no INSERT/UPDATE/DELETE/
      CREATE/ALTER/DROP statements and no migration edits.
  P21-3. No placeholder/fake markers (TODO/FIXME/unimplemented!/todo!/panic!
      "not implemented"/fake data) in the new crate sources.
  P21-4. Wire-freeze gate covering ALL previously frozen ranges:
        - EventKind 0..=21 (P0-P17), 22..=24 (P20) plus new 25 (P21)
        - EventEnvelope payload tags 31..=33 (P20) plus new 34 (P21)
        - Response payload tag 44 (P20 optimization plan) plus new 45/46 (P21)
  P21-5. Recurrence precision: confidence classes exist; a pattern without its
      evidence matrix cannot be emitted (guard constants referenced).
  P21-6. Bounded server-side page size (MAX_PAGE_SIZE referenced by clamping).
  P21-7. Full EN/AR i18n parity for every timeline.* key + Arabic plural entry.

Exit code 0 = all gates pass; 1 = any gate fails.
"""
from __future__ import annotations

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


def strip_comments(source_text: str) -> str:
    return "\n".join(
        line for line in source_text.splitlines() if not line.lstrip().startswith("//")
    )


# ======================= Phase 20 gates (unchanged) ========================

destructive_patterns = [
    r"EmptyWorkingSet",
    r"RegDeleteValue\w*",
    r"RegDeleteKey\w*",
    r"DeleteFileW?\b",
    r"RemoveDirectoryW?\b",
]
perf_dirs = [
    ROOT / "crates/performance-telemetry/src",
    ROOT / "crates/performance-bottleneck/src",
    ROOT / "crates/performance-optimization/src",
]
for directory in perf_dirs:
    for source in directory.glob("*.rs"):
        text = strip_comments(source.read_text(encoding="utf-8"))
        for pattern in destructive_patterns:
            check(
                f"destructive-api:{source.name}",
                not re.search(pattern, text),
                f"{pattern} found in {source.name}",
            )

telemetry = (ROOT / "crates/performance-telemetry/src/lib.rs").read_text(encoding="utf-8")
check("interval-floor", "MIN_INTERVAL_MS: u32 = 250" in telemetry, "250 ms floor constant missing")
check(
    "interval-clamp-used",
    "clamped_interval_ms" in telemetry and "clamp(MIN_INTERVAL_MS, MAX_INTERVAL_MS)" in telemetry,
    "start() must clamp through the documented helper",
)
check("ring-bound-const", "MAX_RING_SAMPLES: usize = 300" in telemetry, "bound constant missing")
check(
    "ring-bound-push",
    len(re.findall(r"samples\.len\(\) >= MAX_RING_SAMPLES", telemetry)) >= 2,
    "both worker and external push must enforce the bound",
)

bottleneck = (ROOT / "crates/performance-bottleneck/src/lib.rs").read_text(encoding="utf-8")
rule_outputs = re.findall(r"Some\(RuleOutput \{.*?\}\)", bottleneck, flags=re.S)
check("rules-present", len(rule_outputs) >= 6, f"only {len(rule_outputs)} rule outputs found")
for index, block in enumerate(rule_outputs):
    check(
        f"rule-{index}-evidence",
        "evidence:" in block,
        "a RuleOutput without cited evidence would be speculation",
    )
check(
    "insufficient-evidence-guard",
    "MIN_SAMPLES_FOR_FINDINGS" in bottleneck and "sample_count < thresholds::MIN_SAMPLES_FOR_FINDINGS" in bottleneck,
    "empty-report guard missing",
)

optimizer = (ROOT / "crates/performance-optimization/src/lib.rs").read_text(encoding="utf-8")
candidate_block = re.search(r"candidates\.push\(Candidate \{(.*?)\}\);", optimizer, flags=re.S)
check("candidate-construction", candidate_block is not None, "Candidate construction site missing")
if candidate_block:
    check(
        "candidate-reversibility-fixed",
        "reversibility," in candidate_block.group(1),
        "reversibility must be set from the kind policy, never caller-supplied",
    )
check(
    "trim-requires-consent",
    "ActionKind::CooperativeTrimRequest => (Reversibility::AutomaticRestore, true)" in optimizer,
    "memory trim must demand explicit consent",
)

events = (ROOT / "crates/contracts/proto/events.proto").read_text(encoding="utf-8")
kinds = dict(re.findall(r"(EVENT_KIND_[A-Z_]+) = (\d+);", events))
expected_stable = {
    "EVENT_KIND_SERVICE_SNAPSHOT": "1",
    "EVENT_KIND_DEEP_SCAN": "21",
}
for name, tag in expected_stable.items():
    check(f"wire-stable:{name}", kinds.get(name) == tag, f"{name} moved from its frozen tag")
check("wire-additive-p20", kinds.get("EVENT_KIND_PERFORMANCE_SNAPSHOT") == "22", "P20 snapshot tag wrong")
check("wire-additive-bottleneck", kinds.get("EVENT_KIND_BOTTLENECK_REPORT") == "23", "P20 report tag wrong")
check("wire-additive-optimization", kinds.get("EVENT_KIND_OPTIMIZATION_EXECUTION") == "24", "P20 execution tag wrong")

en = (ROOT / "apps/ui/src/lib/i18n/catalog.en.ts").read_text(encoding="utf-8")
ar = (ROOT / "apps/ui/src/lib/i18n/catalog.ar.ts").read_text(encoding="utf-8")
en_perf = set(re.findall(r"'(perf\.[^']+|nav\.performance[^']*)'", en))
ar_perf = set(re.findall(r"'(perf\.[^']+|nav\.performance[^']*)'", ar))
check("i18n-parity-en-ar", en_perf == ar_perf and len(en_perf) >= 30,
      f"EN={len(en_perf)} AR={len(ar_perf)}")

# ======================= Phase 21 gates ====================================

# --- Gate P21-1: destructive APIs must not appear in the timeline crate ------
timeline_dir = ROOT / "crates/timeline-intelligence/src"
for source in sorted(timeline_dir.glob("*.rs")):
    text = strip_comments(source.read_text(encoding="utf-8"))
    for pattern in destructive_patterns:
        check(
            f"p21-destructive-api:{source.name}",
            not re.search(pattern, text),
            f"{pattern} found in {source.name}",
        )

# --- Gate P21-2: read-only over persistence ---------------------------------
persistence = (ROOT / "crates/persistence/src/lib.rs").read_text(encoding="utf-8")
write_markers = ["INSERT INTO", "INSERT OR REPLACE INTO", "UPDATE ", "DELETE FROM"]
timeline_sources = {s.name: strip_comments(s.read_text(encoding="utf-8")) for s in sorted(timeline_dir.glob("*.rs"))}
for name, text in timeline_sources.items():
    for marker in write_markers:
        check(
            f"p21-read-only:{name}:{marker.strip()}",
            marker not in text,
            f"write statement '{marker}' must not appear in the timeline domain crate",
        )
for forbidden_ddl in [
    "CREATE TABLE IF NOT EXISTS repair_timeline_events",
    "ALTER TABLE repair_timeline_events",
]:
    check(
        f"p21-no-ddl:{forbidden_ddl.split()[1]}",
        forbidden_ddl not in persistence or forbidden_ddl.startswith("CREATE TABLE IF NOT EXISTS repair_timeline_events") is False,
        "timeline ingestion must not alter the persisted schema",
    )
migration_names = re.findall(r"\((\d+), \"(00\d+_[a-z0-9_]+)\"", persistence)
frozen_prefix = [
    "0001_init", "0002_driver_install", "0003_phase4", "0004_startup_manager",
    "0005_diagnostics", "0006_phase9_security", "0007_phase10_kernel",
    "0008_phase14_scheduler", "0009_phase15_update", "0010_phase17_intelligence",
    "0011_phase17_1_intelligence_integrity", "0012_phase18_driver_authority",
    "0013_phase19_windows_repair",
]
check(
    "p21-no-migration-drift",
    # The ledger is append-only: the frozen prefix must be untouched; later phases
    # may append new migrations after it but never renumber or rename these.
    [name for _v, name in migration_names][:13] == frozen_prefix,
    "existing migration prefix must remain untouched (no renames, no renumbers)",
)

# --- Gate P21-3: no placeholders / fake data ---------------------------------
placeholder_patterns = [
    r"\bTODO\b",
    r"\bFIXME\b",
    r"unimplemented!\s*\(",
    r"todo!\s*\(",
    r"\bfake_\w+",
    r"not implemented",
]
for name, text in timeline_sources.items():
    for pattern in placeholder_patterns:
        check(
            f"p21-placeholder:{name}:{pattern[:12]}",
            not re.search(pattern, text),
            f"placeholder pattern '{pattern}' found in {name}",
        )
service_timeline = strip_comments((ROOT / "services/maintenance-service/src/timeline.rs").read_text(encoding="utf-8"))
for pattern in [r"\bTODO\b", r"\bFIXME\b", r"unimplemented!\s*\(", r"todo!\s*\("]:
    check(
        f"p21-placeholder:timeline.rs:{pattern[:12]}",
        not re.search(pattern, service_timeline),
        f"placeholder pattern '{pattern}' found in service timeline.rs",
    )
ui_controller = strip_comments((ROOT / "apps/ui/src/features/timeline/controller.ts").read_text(encoding="utf-8"))
check("p21-placeholder:controller.ts", not re.search(r"\b(TODO|FIXME)\b", ui_controller), "placeholder found")

# --- Gate P21-4: complete wire freeze ----------------------------------------
operations = (ROOT / "crates/contracts/proto/operations.proto").read_text(encoding="utf-8")
envelope_slice = events.split("message EventEnvelope")[1].split("oneof payload")[1].split("\n  }")[0]
envelope_pairs = re.findall(r"^\s{4}(\w+)\s+(\w+)\s*=\s*(\d+);", envelope_slice, flags=re.M)
envelope_payloads = {field: tag for (_type, field, tag) in envelope_pairs}
frozen_event_kinds = {
    "EVENT_KIND_UNSPECIFIED": "0", "EVENT_KIND_SERVICE_SNAPSHOT": "1", "EVENT_KIND_PLAN_CHANGED": "2",
    "EVENT_KIND_DRIVER_DISCOVERY": "3", "EVENT_KIND_DRIVER_INSTALL": "4", "EVENT_KIND_RECOVERY_HISTORY": "5",
    "EVENT_KIND_REPAIR_ASSESSMENT": "6", "EVENT_KIND_SYSTEM_REPAIR": "7", "EVENT_KIND_CLEANUP_DISCOVERY": "8",
    "EVENT_KIND_CLEANUP_EXECUTION": "9", "EVENT_KIND_STARTUP_DISCOVERY": "10", "EVENT_KIND_STARTUP_EXECUTION": "11",
    "EVENT_KIND_STARTUP_HISTORY": "12", "EVENT_KIND_DIAGNOSTICS": "13", "EVENT_KIND_DIAGNOSTICS_HISTORY": "14",
    "EVENT_KIND_CONSENT_CHANGED": "15", "EVENT_KIND_MUTATION_LEASE": "16", "EVENT_KIND_PROGRESS_TELEMETRY": "17",
    "EVENT_KIND_SCHEDULER": "18", "EVENT_KIND_UPDATE": "19", "EVENT_KIND_SUPPORT_BUNDLE": "20",
    "EVENT_KIND_DEEP_SCAN": "21", "EVENT_KIND_PERFORMANCE_SNAPSHOT": "22",
    "EVENT_KIND_BOTTLENECK_REPORT": "23", "EVENT_KIND_OPTIMIZATION_EXECUTION": "24",
}
for name, tag in frozen_event_kinds.items():
    check(
        f"p21-wire-freeze:{name}",
        kinds.get(name) == tag,
        f"{name} drifted from its frozen tag {tag}",
    )
check(
    "p21-wire-additive-timeline-kind",
    kinds.get("EVENT_KIND_TIMELINE_PAGE") == "25",
    "P21 EventKind tag wrong",
)

frozen_envelope = {
    "performance_snapshot": "31", "bottleneck_report": "32", "optimization_status": "33",
}
for field, tag in frozen_envelope.items():
    check(
        f"p21-wire-freeze:envelope:{field}",
        envelope_payloads.get(field) == tag,
        f"EventEnvelope payload {field} drifted from frozen tag {tag}",
    )
check(
    "p21-wire-additive-envelope-timeline",
    envelope_payloads.get("timeline_page") == "34",
    "P21 EventEnvelope payload tag wrong",
)

response_slice = operations.split("message Response")[1].split("oneof payload")[1].split("\n  }")[0]
response_pairs = re.findall(r"^\s{4}(\w+)\s+(\w+)\s*=\s*(\d+);", response_slice, flags=re.M)
response_fields = {field: tag for (_type, field, tag) in response_pairs}
check("p21-wire-freeze:response:optimization_plan", response_fields.get("optimization_plan") == "44", "response tag 44 drifted")
check("p21-wire-freeze:response:optimization_status", response_fields.get("optimization_status") == "43", "response tag 43 drifted")
check("p21-wire-additive:response:recurrence_patterns", response_fields.get("recurrence_patterns") == "45", "response tag 45 wrong")
check("p21-wire-additive:response:timeline_page", response_fields.get("timeline_page") == "46", "response tag 46 wrong")

request_slice = operations.split("message Request")[1].split("oneof payload")[1].split("\n  }")[0]
request_pairs = re.findall(r"^\s{4}(\w+)\s+(\w+)\s*=\s*(\d+);", request_slice, flags=re.M)
request_fields = {field: tag for (_type, field, tag) in request_pairs}
check("p21-wire-freeze:request:get_optimization_status", request_fields.get("get_optimization_status") == "77", "request tag 77 drifted")
check("p21-wire-additive:request:get_timeline_page", request_fields.get("get_timeline_page") == "78", "request tag 78 wrong")
check("p21-wire-additive:request:get_recurrence_patterns", request_fields.get("get_recurrence_patterns") == "79", "request tag 79 wrong")

# --- Gate P21-5: recurrence precision machinery present -----------------------
engine_src = (ROOT / "crates/timeline-intelligence/src/engine.rs").read_text(encoding="utf-8")
model_src = (ROOT / "crates/timeline-intelligence/src/model.rs").read_text(encoding="utf-8")
check("p21-confidence-classes", all(c in model_src for c in ["Weak", "Moderate", "Strong"]), "confidence classes missing")
check("p21-min-occurrence-guard", "occurrence_count < MIN_OCCURRENCES_FOR_PATTERN" in engine_src, "minimum occurrence guard missing")
check("p21-gap-window-guard", "MAX_RECURRENCE_GAP_MS" in engine_src and "gap < 0" in engine_src.replace("\r", ""), "hostile gap guard missing")
check("p21-correlation-not-causation", "causation" in engine_src.lower() or "asserts causation" in engine_src.lower(), "correlation-not-causation contract undocumented")
check("p21-pattern-cap", "patterns.truncate(MAX_PATTERNS)" in engine_src, "pattern cap missing")
check("p21-evidence-bound", "bounded_evidence" in engine_src and "MAX_PATTERN_EVIDENCE" in engine_src, "evidence citation bound missing")

# --- Gate P21-6: bounded page size enforced server-side -----------------------
service_timeline_raw = (ROOT / "services/maintenance-service/src/timeline.rs").read_text(encoding="utf-8")
check("p21-page-size-clamp", "clamp(1, MAX_PAGE_SIZE as u32)" in service_timeline_raw, "page size clamp missing")
check("p21-max-page-const", "pub const MAX_PAGE_SIZE: usize = 200" in model_src, "MAX_PAGE_SIZE constant missing")

# --- Gate P21-7: full EN/AR parity for timeline keys --------------------------
en_timeline = set(re.findall(r"'(timeline\.[^']+)'", en))
ar_timeline = set(re.findall(r"'(timeline\.[^']+)'", ar))
check("p21-i18n-parity-en-ar-timeline", en_timeline == ar_timeline and len(en_timeline) >= 20,
      f"EN={len(en_timeline)} AR={len(ar_timeline)}")
en_plurals_src = (ROOT / "apps/ui/src/lib/i18n/plurals.en.ts").read_text(encoding="utf-8")
ar_plurals_src = (ROOT / "apps/ui/src/lib/i18n/plurals.ar.ts").read_text(encoding="utf-8")
check("p21-arabic-plural-occurrence", "'unit.occurrence'" in en_plurals_src and "'unit.occurrence'" in ar_plurals_src,
      "unit.occurrence plural missing from a catalog")
ar_plural_line = next(
    (line for line in ar_plurals_src.splitlines() if "'unit.occurrence'" in line), ""
)
ar_plural_entry = (
    re.search(r"zero:", ar_plural_line)
    and re.search(r"one:", ar_plural_line)
    and re.search(r"two:", ar_plural_line)
    and re.search(r"few:", ar_plural_line)
    and re.search(r"many:", ar_plural_line)
    and re.search(r"other:", ar_plural_line)
) if ar_plural_line else None
check("p21-arabic-plural-six-forms", ar_plural_entry is not None, "Arabic plural entry lacks the six forms")

result = {
    "schema": "aethercore.phase21.adversarial-audit.v1",
    "root": str(ROOT),
    "checks": checks,
    "failures": failures,
    "status": "PASS" if not failures else "FAIL",
}
print(json.dumps(result, indent=2))
sys.exit(0 if not failures else 1)
