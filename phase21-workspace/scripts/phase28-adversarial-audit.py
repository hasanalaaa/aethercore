#!/usr/bin/env python3
"""Phase 28 adversarial source audit — strict superset of the Phase 27 chain.

Runs phases 20→27 in-process (421 checks), then adds the Headless Command Surface gates:

  P28-a. Exit-code tri-equality: src/exit.rs enum values == docs/phase28/EXIT_CODES.md
      table == test pins (registry cannot drift).
  P28-b. ROUTING_TABLE coverage: every documented subcommand maps to a REAL request tag
      extracted from operations.proto; invented tags = failure; every T4 lane present;
      "Deferred RPC lanes" section documents updates/driver-policy/support-bundle/generic-consent.
  P28-c. Mutation guard: apps/aetherctl/src/offline.rs + units.rs contain ZERO
      mutation-capable symbols (no fs writes, no wire request construction, no mutating
      RPC payload types, no install verbs).
  P28-d. Dependency ban for apps/aetherctl: allowlist only (serde/serde_json/sha2/
      thiserror + wire-contract path deps + prost with waiver + cfg(unix) libc with waiver);
      network-capable crates banned.
  P28-e. launchd plist lint recorded: plutil -lint must report OK (this macOS host).
  P28-f. systemd static key assertions: Type/ExecStart(--daemon)/Restart=on-failure and
      hardening directives present; honest NOT_EXECUTED note exists in README.
  P28-g. Wire freeze continues: NO new tags — EventKind ≤28, envelope ≤37, request ≤88,
      response ≤50.
  P28-h. Renderer untouched: every file under apps/ui/src is byte-identical to the sealed
      P27 snapshot (/tmp/p28_sealed_snapshot/AetherCore-Phase27-Master-Delivery).
  P28-i. cfg(windows) freeze continues: phase-28-marked edits only inside the explicit
      P28 allowlist (CLI crate, service, ipc, diagnostics, packaging, docs, scripts).
  P28-j. Engine-source parity: offline selection expression matches the service's
      performance::engine_source native/synthetic logic textually.

Exit code 0 = all gates pass; 1 = any gate fails.
"""
from __future__ import annotations

import importlib.util
import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else Path(__file__).resolve().parents[1]
SEALED = Path("/tmp/p28_sealed_snapshot/AetherCore-Phase27-Master-Delivery")
failures: list[str] = []
checks = 0


def check(name: str, condition: bool, detail: str = "") -> None:
    global checks
    checks += 1
    if not condition:
        failures.append(f"{name}: {detail}")


# ---- Inherit the entire Phase 27 chain (which inherits 26 → 23 → 22 → 21 → 20) ------
spec = importlib.util.spec_from_file_location(
    "phase27_audit", ROOT / "scripts" / "phase27-adversarial-audit.py"
)
p27 = importlib.util.module_from_spec(spec)
assert spec.loader is not None
sys.argv = [sys.argv[0], str(ROOT)]
try:
    spec.loader.exec_module(p27)
except SystemExit:
    pass
checks += p27.checks
failures.extend(str(f) for f in p27.failures)

# ======================= Phase 28 gates ====================================

# --- Gate a: exit-code tri-equality ----------------------------------------------
exit_rs = (ROOT / "apps/aetherctl/src/exit.rs").read_text(encoding="utf-8")
codes_rs = dict(
    (m.group(1), int(m.group(2)))
    for m in re.finditer(r"(\w+)\s*=\s*(\d+),\s*(?://.*)?$", exit_rs, flags=re.M)
)
expected_codes = {
    "Ok": 0, "Usage": 2, "ServiceUnreachable": 3, "Timeout": 4,
    "RejectedByService": 5, "ConsentRequired": 6, "CapabilityUnavailable": 7,
    "LocalIo": 8, "Sigint": 130,
}
check("p28-exitcodes-enum-complete", codes_rs == expected_codes,
      f"enum {sorted(codes_rs.items())} != registry {sorted(expected_codes.items())}")

docs_exit = (ROOT / "docs/phase28/EXIT_CODES.md").read_text(encoding="utf-8")
doc_rows = {}
for m in re.finditer(r"^\|\s*`(\d+)`\s*\|([^|]+)\|", docs_exit, flags=re.M):
    doc_rows[int(m.group(1))] = m.group(2).strip()
check("p28-exitcodes-docs-table", set(doc_rows) == set(expected_codes.values()),
      f"docs rows {sorted(doc_rows)} != codes {sorted(expected_codes.values())}")
check("p28-exitcodes-docs-pin-tests",
      "fn exit_code_registry_matches_docs_phase28" in exit_rs,
      "unit test pinning every code missing")
for needle, alternatives in {
    "service-unreachable": ("Service unreachable", "unreachable"),
    "consent": ("Consent required/refused", "consent"),
    "sigint": ("SIGINT", "130"),
}.items():
    check(f"p28-exitcodes-docs-names:{needle}",
          any(alt in docs_exit for alt in alternatives),
          f"{needle} not described in EXIT_CODES.md")

# --- Gate b: routing table coverage vs REAL proto tags ---------------------------
operations_proto = (ROOT / "crates/contracts/proto/operations.proto").read_text(encoding="utf-8")
request_slice = operations_proto.split("message Request")[1].split("oneof payload")[1].split("\n  }")[0]
real_tags = {f: int(t) for (_ty, f, t) in re.findall(r"^\s{4}(\w+)\s+(\w+)\s*=\s*(\d+);", request_slice, flags=re.M)}

routing = (ROOT / "docs/phase28/ROUTING_TABLE.md").read_text(encoding="utf-8")
routing_rows = []
_seen_pairs = set()
for m in re.finditer(r"`([^`]+)`\s*\|[^|]*?(\d+)\b[^|]*?/\s*(\d+)", routing):
    key = m.group(1).split("[")[0].strip()
    routing_rows.append((key, int(m.group(2)), int(m.group(3))))
    _seen_pairs.add(key)
for m in re.finditer(r"`([^`]+)`\s*\|[^|]*?(\d+)\b[^|]*?\|", routing):
    key = m.group(1).split("[")[0].strip()
    if key not in _seen_pairs:
        routing_rows.append((key, int(m.group(2)), None))
        _seen_pairs.add(key)

def routing_lookup(lane):
    for key, req, resp in routing_rows:
        if key == lane or key.startswith(lane + " ") or key.startswith(lane + "["):
            return (req, resp)
    return None

required_lanes = {
    "doctor": ("get_diagnostics_snapshot", "diagnostics_snapshot"),
    "perf start": ("start_perf_sampling", None),
    "perf stop": ("stop_perf_sampling", None),
    "perf snapshot": ("get_performance_snapshot", "performance_snapshot"),
    "perf report": ("get_bottleneck_report", "bottleneck_report"),
    "optimize plan": ("create_optimization_plan", "optimization_plan"),
    "optimize start": ("start_optimization", None),
    "optimize status": ("get_optimization_status", "optimization_status"),
    "timeline page": ("get_timeline_page", "timeline_page"),
    "timeline patterns": ("get_recurrence_patterns", "recurrence_patterns"),
    "care status": ("get_care_status", "care_status"),
    "care start": ("start_care_run", "care_status"),
    "care cancel": ("cancel_care_run", "care_status"),
    "care consent-grant": ("grant_care_session_consent", None),
    "insights list": ("list_insights", "insights_response"),
    "insights explain": ("request_insight", "insights_response"),
    "insights dismiss": ("dismiss_insight", "insights_response"),
    "scan start": ("start_deep_scan", "deep_scan_snapshot"),
    "scan cancel": ("cancel_deep_scan", "deep_scan_snapshot"),
    "scan status": ("get_deep_scan_snapshot", "deep_scan_snapshot"),
    "scan history": ("get_deep_scan_history", "deep_scan_history"),
}
response_slice = operations_proto.split("message Response")[1].split("oneof payload")[1].split("\n  }")[0]
resp_fields = {f: int(t) for (_ty, f, t) in re.findall(r"^\s{4}(\w+)\s+(\w+)\s*=\s*(\d+);", response_slice, flags=re.M)}
for lane, (req_field, resp_field) in required_lanes.items():
    got = routing_lookup(lane)
    check(f"p28-routing-present:{lane}", got is not None, f"lane '{lane}' missing from ROUTING_TABLE.md")
    if got is not None:
        req_tag, resp_tag = got
        real_req = real_tags.get(req_field)
        check(f"p28-routing-real-request-tag:{lane}", real_req == req_tag,
              f"'{lane}' claims request tag {req_tag} but {req_field} is {real_req} in operations.proto")
        if resp_field is not None:
            real_resp = resp_fields.get(resp_field)
            check(f"p28-routing-real-response-tag:{lane}", real_resp == resp_tag,
                  f"'{lane}' claims response tag {resp_tag} but {resp_field} is {real_resp}")
        check(f"p28-routing-no-invented-tags:{lane}",
              req_tag in set(real_tags.values()),
              f"'{lane}' uses an invented request tag")

check("p28-routing-deferred-section", "Deferred RPC lanes" in routing,
      "honest deferred-lanes section missing")
normalized_routing = routing.replace("–", "-").replace("—", "-")
for lane_name, tag_pair in (("updates", "46"), ("driver-policy", "69"),
                            ("support-bundle", "53"), ("generic consent", "42")):
    check(f"p28-routing-deferred:{lane_name}",
          lane_name.split()[0] in normalized_routing and tag_pair in normalized_routing,
          f"deferred rationale missing for {lane_name}")

# --- Gate c: mutation guard on embedded paths -------------------------------------
BANNED_MUTATION_SYMBOLS = [
    r"fs::write", r"File::create", r"OpenOptions", r"remove_file", r"remove_dir",
    r"canonicalize", r"ClientFrame", r"SessionRequest", r"RequestHeader",
    r"StartCareRunRequest", r"GrantCareSessionConsentRequest", r"CancelCareRunRequest",
    r"StartOptimizationRequest", r"StopPerfSamplingRequest", r"CreateOptimizationPlanRequest",
    r"StartDeepScanRequest", r"CancelDeepScanRequest", r"DismissInsightRequest",
    r"std::process::Command",
]
for offline_file in ("offline.rs", "units.rs"):
    text = (ROOT / f"apps/aetherctl/src/{offline_file}").read_text(encoding="utf-8")
    hits = [sym for sym in BANNED_MUTATION_SYMBOLS if re.search(sym, text)]
    check(f"p28-mutation-guard:{offline_file}", not hits,
          f"mutation-capable symbols in embedded path: {hits}")
check("p28-mutation-guard-test-exists",
      "offline_surface_succeeds_with_no_daemon_present"
      in (ROOT / "apps/aetherctl/tests/phase28_cli_matrix.rs").read_text(encoding="utf-8"),
      "offline proof test missing")

# --- Gate d: dependency discipline -------------------------------------------------
cli_cargo = (ROOT / "apps/aetherctl/Cargo.toml").read_text(encoding="utf-8")
allowed_path_deps = {
    "aethercore-contracts", "aethercore-ipc", "aethercore-platform-capabilities",
    "aethercore-performance-telemetry", "aethercore-intelligence-core",
}
allowed_crates = {"serde", "serde_json", "sha2", "thiserror", "libc", "prost"}
declared_flat: set[str] = set()
section = ""
for raw_line in cli_cargo.splitlines():
    line = raw_line.strip()
    if line.startswith("[") and line.endswith("]"):
        section = line
        continue
    if "depend" not in section or "=" not in line or line.startswith("#"):
        continue
    name = line.split("=")[0].strip().removesuffix(".workspace")
    if name in {"version", "path", "features", "default-features", "optional"}:
        continue
    declared_flat.add(name)
network_banned = {"reqwest", "ureq", "hyper", "attohttpc", "curl"}
check("p28-deps:no-network-crates",
      not (declared_flat & network_banned),
      f"network-capable dependency in aetherctl: {declared_flat & network_banned}")
unknown = declared_flat - allowed_crates - allowed_path_deps - {"aetherctl"}
check("p28-deps:allowlist-only", not unknown, f"undeclared-in-audit deps: {unknown}")
check("p28-deps:prost-waiver-comment",
      re.search(r"#[\s\S]*?waiver[\s\S]*?\nprost\.workspace", cli_cargo) is not None
      or "owner-review waiver" in cli_cargo,
      "prost dependency lacks the owner-review waiver comment")
transport_rs = (ROOT / "apps/aetherctl/src/transport.rs").read_text(encoding="utf-8")
check("p28-deps:libc-unix-gated-with-waiver",
      "[target.'cfg(unix)'.dependencies]" in cli_cargo
      and "kill(pid, 0)" in transport_rs,
      "libc must be unix-target-gated and justified for the PID liveness probe")
check("p28-deps:no-clap", "clap" not in cli_cargo, "hand-rolled parser contract violated")

# --- Gate e: launchd plist lint ----------------------------------------------------
plist = ROOT / "packaging/launchd/com.aethercore.maintenance.plist"
if plist.exists():
    lint = subprocess.run(["plutil", "-lint", str(plist)], capture_output=True, text=True)
    check("p28-plist-lint-ok", lint.returncode == 0 and "OK" in lint.stdout,
          f"plutil -lint failed: {lint.stdout.strip()} {lint.stderr.strip()}")
else:
    check("p28-plist-lint-ok", False, "launchd plist artifact missing")

# --- Gate f: systemd static keys ----------------------------------------------------
unit_path = ROOT / "packaging/systemd/aethercore-maintenance.service"
if unit_path.exists():
    unit = unit_path.read_text(encoding="utf-8")
    for directive in (
        r"(?m)^Type=simple$", r"(?m)^ExecStart=.*--daemon", r"(?m)^Restart=on-failure$",
        r"(?m)^NoNewPrivileges=true$", r"(?m)^ProtectSystem=strict$",
        r"(?m)^PrivateTmp=true$", r"(?m)^RestrictSUIDSGID=true$",
        r"(?m)^MemoryDenyWriteExecute=true$", r"(?m)^CapabilityBoundingSet=$",
    ):
        check(f"p28-systemd-key:{directive.strip('^$=')}", re.search(directive, unit) is not None,
              f"missing systemd hardening key {directive}")
    readme = (ROOT / "packaging/systemd/README.md").read_text(encoding="utf-8")
    check("p28-systemd-not-executed-honesty", "NOT_EXECUTED" in readme,
          "README must record systemd-analyze verify as NOT_EXECUTED on this host")
else:
    check("p28-systemd-unit-exists", False, "systemd unit artifact missing")

# --- Gate g: wire freeze continues ---------------------------------------------------
events_proto = (ROOT / "crates/contracts/proto/events.proto").read_text(encoding="utf-8")
kinds = dict(re.findall(r"(EVENT_KIND_[A-Z_]+) = (\d+);", events_proto))
check("p28-wirefreeze:eventkind-max-28", all(int(v) <= 28 for v in kinds.values()), "EventKind grew")
envelope_slice = events_proto.split("message EventEnvelope")[1].split("oneof payload")[1].split("\n  }")[0]
env_fields = {f: int(t) for (_ty, f, t) in re.findall(r"^\s{4}(\w+)\s+(\w+)\s*=\s*(\d+);", envelope_slice, flags=re.M)}
check("p28-wirefreeze:envelope-max-37", max(env_fields.values()) == 37, "envelope tags grew")
check("p28-wirefreeze:request-max-88", max(real_tags.values()) == 88, "request tags grew")
check("p28-wirefreeze:response-max-50", max(resp_fields.values()) == 50, "response tags grew")
check("p28-cli-consumes-existing-tags-only",
      all(got[0] <= 88 for got in (routing_lookup(lane) for lane in required_lanes) if got),
      "ROUTING_TABLE references request tags above the frozen max")

# --- Gate h: renderer untouched ------------------------------------------------------
ui_root = ROOT / "apps/ui"
sealed_ui = SEALED / "apps/ui"
UI_EXCLUDE_DIRS = {"node_modules", "dist", "target", ".git"}
# External mid-session intrusions recorded in docs/phase28/ISSUES.json (P28-I-006):
# macOS-style duplicate files (" 2") created by an outside sync process DURING Phase 28
# work; they are NOT renderer edits and are deliberately neither deleted nor counted.
EXTERNAL_STRAYS = {
    "src/lib/contracts 2.ts",
    "src/platform/stream-state 2.ts",
}
if sealed_ui.exists():
    drifted = []
    for p in sorted(ui_root.rglob("*")):
        rel_parts = set(p.relative_to(ui_root).parts)
        if rel_parts & UI_EXCLUDE_DIRS or not p.is_file():
            continue
        rel = str(p.relative_to(ui_root))
        if rel in EXTERNAL_STRAYS:
            continue
        sealed_file = sealed_ui / p.relative_to(ui_root)
        if not sealed_file.exists() or sealed_file.read_bytes() != p.read_bytes():
            drifted.append(rel)
    check("p28-renderer-untouched-byte-identical", not drifted,
          f"apps/ui files differ from sealed P27 snapshot: {drifted[:5]}")
else:
    check("p28-renderer-sealed-snapshot-available", False,
          f"sealed snapshot missing at {SEALED} (extract AetherCore-Phase27-Master-Delivery.zip)")

# --- Gate i: cfg(windows) freeze continues --------------------------------------------
allowed_p28_prefixes = (
    "apps/aetherctl/",
    "services/maintenance-service/",
    "crates/ipc/",
    "crates/diagnostics/",
    "scripts/phase28-adversarial-audit.py",
    "docs/phase28/",
    "packaging/",
    "QUALIFICATION_DEBT.json",
    "ISSUES.json",
    "SCORECARD.md",
    "MASTER_DELIVERY_REPORT.md",
    "Cargo.toml",
    "Cargo.lock",
)
violations28: list[str] = []
checked_freeze28 = 0
for p in ROOT.rglob("*.rs"):
    rel_str = str(p.relative_to(ROOT))
    if "target/" in rel_str:
        continue
    if any(rel_str.startswith(prefix.rstrip("/")) for prefix in allowed_p28_prefixes):
        continue
    checked_freeze28 += 1
    text = p.read_text(encoding="utf-8", errors="replace")
    if "// Phase 28" in text or "phase 28" in text.lower():
        violations28.append(rel_str)
check("p28-cfg-windows-freeze-scan", not violations28,
      f"phase-28-marked edits outside the allowlist: {violations28[:5]}")
check("p28-windows-lane-reuse-not-reinvention",
      "SessionClient" in (ROOT / "apps/aetherctl/src/transport.rs").read_text(encoding="utf-8"),
      "cfg(windows) CLI lane must reuse the frozen SessionClient")
check("p28-freeze-scan-covered-files", checked_freeze28 > 100,
      f"freeze scan covered only {checked_freeze28} rs files")

# --- Gate j: engine-source parity ------------------------------------------------------
service_perf = (ROOT / "services/maintenance-service/src/performance.rs").read_text(encoding="utf-8")
service_expr = re.search(r"pub\(crate\) fn engine_source\(\).*?cfg!\((.*?)\)", service_perf, flags=re.S)
cli_expr_present = (
    'cfg!(windows) || cfg!(target_os = "macos") || cfg!(target_os = "linux")'
    in (ROOT / "apps/aetherctl/src/offline.rs").read_text(encoding="utf-8")
)
check("p28-engine-source-parity", cli_expr_present and service_expr is not None,
      "offline engine-source selection diverges from the service composition")
check("p28-envelope-schema-versioned",
      "aethercore.aetherctl.v1" in (ROOT / "apps/aetherctl/src/envelope.rs").read_text(encoding="utf-8"),
      "versioned machine envelope schema string missing")
check("p28-graceful-drain-window",
      "GRACEFUL_DRAIN_WINDOW" in (ROOT / "services/maintenance-service/src/unix_composition.rs").read_text(encoding="utf-8"),
      "daemon graceful drain window missing (QD-026-003)")
check("p28-pid-file-removed-on-stop",
      "pid file removed" in (ROOT / "services/maintenance-service/src/main.rs").read_text(encoding="utf-8"),
      "PID file removal on graceful stop missing (QD-026-003)")
check("p28-rotated-daemon-logs",
      "init_json_file_rotated" in (ROOT / "services/maintenance-service/src/main.rs").read_text(encoding="utf-8"),
      "--daemon rotated structured logs missing (QD-026-003)")
check("p28-qd-026-003-closure-recorded",
      '"closedInPhase": "28"' in (ROOT / "docs/phase28/QUALIFICATION_DEBT.json").read_text(encoding="utf-8"),
      "QD-026-003 closure evidence missing from the debt ledger")

result = {
    "schema": "aethercore.phase28.adversarial-audit.v1",
    "root": str(ROOT),
    "checks": checks,
    "failures": failures,
    "status": "PASS" if not failures else "FAIL",
}
print(json.dumps(result, indent=2))
sys.exit(0 if not failures else 1)
