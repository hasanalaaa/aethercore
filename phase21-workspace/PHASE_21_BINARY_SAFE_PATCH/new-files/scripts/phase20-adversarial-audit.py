#!/usr/bin/env python3
"""Phase 20 adversarial source audit.

Static checks that attack the Phase 20 safety contract at the source level:
  1. No destructive API usage in the optimization surface (EmptyWorkingSet, RegDeleteValue,
     DeleteFile inside the perf/optimization crates).
  2. Sampling interval floor enforced (no sub-250 ms observer-effect path).
  3. Ring capacity bound present and referenced by both push paths.
  4. Bottleneck rules always cite evidence (every RuleOutput carries >=1 EvidenceRef).
  5. Optimization candidates always carry reversibility != Unspecified.
  6. Wire contract stays additive: EventKind new tags only, no renumbering of 0..21.

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


# --- Gate 1: destructive APIs must not appear in Phase 20 crates -------------------------
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
        # Strip // comments before scanning so the safety contract *documentation* cannot
        # trip the code-level gate. Only real API usage is a failure.
        text = "\n".join(
            line for line in source.read_text(encoding="utf-8").splitlines()
            if not line.lstrip().startswith("//")
        )
        for pattern in destructive_patterns:
            check(
                f"destructive-api:{source.name}",
                not re.search(pattern, text),
                f"{pattern} found in {source.name}",
            )

# The test that asserts absence of destructive verbs is itself fine; exclude tests from gate 1.
# (Already excluded: only src/ trees scanned.)

# --- Gate 2: sampling floor -------------------------------------------------------------
telemetry = (ROOT / "crates/performance-telemetry/src/lib.rs").read_text(encoding="utf-8")
check("interval-floor", "MIN_INTERVAL_MS: u32 = 250" in telemetry, "250 ms floor constant missing")
check(
    "interval-clamp-used",
    "clamped_interval_ms" in telemetry and "clamp(MIN_INTERVAL_MS, MAX_INTERVAL_MS)" in telemetry,
    "start() must clamp through the documented helper",
)

# --- Gate 3: ring bound referenced by push paths -----------------------------------------
check("ring-bound-const", "MAX_RING_SAMPLES: usize = 300" in telemetry, "bound constant missing")
check(
    "ring-bound-push",
    len(re.findall(r"samples\.len\(\) >= MAX_RING_SAMPLES", telemetry)) >= 2,
    "both worker and external push must enforce the bound",
)

# --- Gate 4: every rule output cites evidence --------------------------------------------
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

# --- Gate 5: candidate construction fixes reversibility ----------------------------------
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

# --- Gate 6: wire contract additive -------------------------------------------------------
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

# --- i18n parity --------------------------------------------------------------------------
en = (ROOT / "apps/ui/src/lib/i18n/catalog.en.ts").read_text(encoding="utf-8")
ar = (ROOT / "apps/ui/src/lib/i18n/catalog.ar.ts").read_text(encoding="utf-8")
en_perf = set(re.findall(r"'(perf\.[^']+|nav\.performance[^']*)'", en))
ar_perf = set(re.findall(r"'(perf\.[^']+|nav\.performance[^']*)'", ar))
check("i18n-parity-en-ar", en_perf == ar_perf and len(en_perf) >= 30,
      f"EN={len(en_perf)} AR={len(ar_perf)}")

result = {
    "schema": "aethercore.phase20.adversarial-audit.v1",
    "root": str(ROOT),
    "checks": checks,
    "failures": failures,
    "status": "PASS" if not failures else "FAIL",
}
print(json.dumps(result, indent=2))
sys.exit(0 if not failures else 1)
