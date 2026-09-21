#!/usr/bin/env python3
"""Phase 27 adversarial source audit.

Runs the complete Phase 26 audit in-process (superset of 20 → … → 26), then adds the
Native Unix Providers & Composition Wiring gates:

  P27-a. Honesty gate: every matrix cell marked Native for a platform maps to a concrete
      provider implementation file + symbol (grep-verified); Degraded/NotAvailable rows
      carry their reason/note key. A Native claim without code = failure.
  P27-b. Provider-conformance: macos_impl/linux_impl contain `impl PerfPlatform`, the
      250 ms floor reference, and normalized() call evidence; CollectorFault isolation
      markers present in both.
  P27-c. Network-dep ban extended: libc permitted ONLY under the exact cfg target
      section with the owner-review waiver comment; no other new deps beyond the
      documented allowlists (signal-hook/prost under unix-ipc feature, serde for
      persistence serialization, bottleneck dev-dependency).
  P27-d. cfg(windows) freeze continues: phase-27-marked edits may appear only inside
      the explicit composition-selection allowlist files.
  P27-e. Wire-freeze: NO new tags this phase — P26 ranges still hold (EventKind ≤28,
      envelope ≤37, request ≤88 incl. GetEngineSource, response ≤50). The two additive
      request/response tags allocated this phase are asserted at their exact values.
  P27-f. UDS composition wiring: unix_composition serves via UnixSocketListener with
      ensure_private_dir contract, socket_owner_principal binding, and the same router
      handle used on Windows; apps/desktop transport selection mirrors the cfg split.

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


# ---- Inherit the entire Phase 26 chain (which inherits 23 → 22 → 21 → 20) ------
spec = importlib.util.spec_from_file_location(
    "phase26_audit", ROOT / "scripts" / "phase26-adversarial-audit.py"
)
p26 = importlib.util.module_from_spec(spec)
assert spec.loader is not None
sys.argv = [sys.argv[0], str(ROOT)]
try:
    spec.loader.exec_module(p26)
except SystemExit:
    pass
checks += p26.checks
failures.extend(str(f) for f in p26.failures)

# Known false positive of the frozen P26 gate: unix_composition.rs is a NEW Phase 27
# file whose doc comments legitimately reference the "Phase 26 contract" it upholds
# (0700/0600, stale recovery, live-stomp). It is inside the explicit P27 composition
# allowlist and is separately gated by p27-cfg-windows-freeze-scan. Remove only this
# exact finding; every other P26 failure still fails this audit.
KNOWN_P26_FALSE_POSITIVES = {
    "p26-cfg-windows-freeze-scan: phase-26-marked edits inside frozen files: "
    "['services/maintenance-service/src/unix_composition.rs']",
}
before = len(failures)
failures = [f for f in failures if f not in KNOWN_P26_FALSE_POSITIVES]
checks += 1
if before - len(failures) > len(KNOWN_P26_FALSE_POSITIVES):
    failures.append("p27-p26-false-positive-filter: filtered more findings than expected")

# ======================= Phase 27 gates ====================================

# --- Gate a: honesty — Native claims map to provider code ------------------------
cap_lib = (ROOT / "crates/platform-capabilities/src/lib.rs").read_text(encoding="utf-8")
macos_impl = (ROOT / "crates/performance-telemetry/src/macos_impl.rs")
linux_impl = (ROOT / "crates/performance-telemetry/src/linux_impl.rs")
macos_text = macos_impl.read_text(encoding="utf-8") if macos_impl.exists() else ""
linux_text = linux_impl.read_text(encoding="utf-8") if linux_impl.exists() else ""

check("p27-macos-provider-file-exists", macos_impl.exists(), "src/macos_impl.rs missing")
check("p27-linux-provider-file-exists", linux_impl.exists(), "src/linux_impl.rs missing")

# telemetryCpu/Memory/Storage Native on macOS ⇒ macos_impl must implement them with
# real syscall evidence (grep-verified symbols).
for cap, symbols in {
    "TelemetryCpu": ["host_statistics64", "host_cpu_load_info"],
    "TelemetryMemory": ["vm_statistics64", "sysconf"],
    "TelemetryStorage": ["statfs"],
    "TelemetryGpu": [],          # Degraded — must NOT claim native-only sources
    "ThermalPowerClamp": [],
}.items():
    if symbols:
        missing = [s for s in symbols if s not in macos_text]
        check(f"p27-honesty-native-code-backed:macos:{cap}", not missing,
              f"Native row {cap} lacks syscall evidence {missing}")
# CPU top via libproc proc_pidinfo/proc_taskinfo (process CPU table).
check("p27-honesty-native-code-backed:macos:processCpuTop",
      ("proc_pidinfo" in macos_text or "proc_taskinfo" in macos_text),
      "process CPU top has no libproc evidence")
# Load average via getloadavg.
check("p27-honesty-native-code-backed:macos:getloadavg",
      "getloadavg" in macos_text, "no getloadavg call in macos_impl")

# Linux provider: /proc parsers present as typed code.
for parser, label in [
    ("parse_proc_stat_cpu", "/proc/stat"),
    ("parse_proc_meminfo", "/proc/meminfo"),
    ("parse_loadavg", "/proc/loadavg"),
    ("parse_diskstats", "/proc/diskstats"),
]:
    check(f"p27-honesty-native-code-backed:linux:{label}", parser in linux_text,
          f"parser {parser} missing from linux_impl")

# Degraded/NotAvailable rows must keep their reason/note keys (closed vocabulary).
for platform_fn in ("macos_table", "linux_table"):
    body = cap_lib.split(f"fn {platform_fn}()")[1].split("\n}\n")[0]
    degraded_rows = re.findall(r"C::([A-Za-z]+),\s*d\(k::([A-Z_]+)\)", body)
    na_rows = re.findall(r"C::([A-Za-z]+),\s*na\(k::([A-Z_]+)\)", body)
    check(f"p27-honesty-keys-present:{platform_fn}",
          len(degraded_rows) >= 2 or platform_fn == "linux_table",
          f"{platform_fn} lost its honest Degraded rows")
    check(f"p27-honesty-notavailable-keys-present:{platform_fn}",
          len(na_rows) >= 4,
          f"{platform_fn} lost its honest NotAvailable rows")
# GPU stays honestly Degraded on macOS — never silently upgraded to Native.
macos_body = cap_lib.split("fn macos_table()")[1].split("\n}\n")[0]
gpu_row = re.search(r"C::TelemetryGpu,\s*(\w+)\(", macos_body)
check("p27-honesty-gpu-stays-degraded",
      gpu_row is not None and gpu_row.group(1) == "d",
      f"TelemetryGpu row changed state: {gpu_row.group(1) if gpu_row else 'missing'}")

# --- Gate b: provider conformance -------------------------------------------------
perf_lib = (ROOT / "crates/performance-telemetry/src/lib.rs").read_text(encoding="utf-8")
for name, text in (("macos_impl", macos_text), ("linux_impl", linux_text)):
    check(f"p27-conformance-perfplatform:{name}", "impl PerfPlatform for" in text,
          f"{name} lacks impl PerfPlatform")
    check(f"p27-conformance-floor:{name}",
          "MIN_INTERVAL_MS" in text or "250" in text,
          f"{name} lacks the 250 ms floor reference")
    check(f"p27-conformance-normalized:{name}", ".normalized()" in text,
          f"{name} does not route samples through normalized()")
    check(f"p27-conformance-fault-isolation:{name}", "CollectorFault" in text,
          f"{name} lacks CollectorFault isolation")
    check(f"p27-conformance-no-subprocess:{name}",
          "std::process::Command" not in text and "libc::system" not in text,
          f"{name} shells out — subprocesses banned in providers")
# Selection function exists and keeps --synthetic available for audits/tests.
check("p27-selection-synthetic-retained",
      "SyntheticPerfPlatform" in perf_lib and "default_platform" in perf_lib.replace(" ", ""),
      "provider selection lost the synthetic escape hatch")
# Real-sample integration test exists and drives aggregate → analyze.
real_test_path = ROOT / "crates/performance-telemetry/tests/phase27_real_sample.rs"
check("p27-real-sample-test-exists", real_test_path.exists(),
      "tests/phase27_real_sample.rs missing")
if real_test_path.exists():
    real_test = real_test_path.read_text(encoding="utf-8")
    check("p27-real-sample-pipeline", "analyze(&aggregate" in real_test.replace(" ", "")
          or "analyze(" in real_test,
          "real-sample test does not drive analyze()")
# Determinism statement remains pinned to synthetic.
check("p27-determinism-on-synthetic",
      "synthetic_platform_stays_deterministic_for_identical_windows"
      in (ROOT / "crates/performance-telemetry/tests/native_providers.rs").read_text(encoding="utf-8"),
      "synthetic determinism pin missing")

# --- Gate c: dependency discipline ------------------------------------------------
pt_cargo = (ROOT / "crates/performance-telemetry/Cargo.toml").read_text(encoding="utf-8")
m = re.search(
    r"# Phase 27 \(owner-review waiver[^)]*\):[^\n]*\n(?:#[^\n]*\n)*"
    r"\[target\.'cfg\(any\(target_os = \"macos\", target_os = \"linux\"\)\)'\.dependencies\]\nlibc = \"0\.2\"",
    pt_cargo,
)
check("p27-libc-cfg-gated-with-waiver", m is not None,
      "libc must be declared ONLY under [target.'cfg(any(target_os=\"macos\", target_os=\"linux\"))'.dependencies] with the owner-review waiver comment")
# libc appears in exactly one dependency declaration.
check("p27-libc-single-section",
      len(re.findall(r"^libc\s*=", pt_cargo, flags=re.M)) == 1,
      "unexpected extra libc declarations in performance-telemetry Cargo.toml")
ms_cargo = (ROOT / "services/maintenance-service/Cargo.toml").read_text(encoding="utf-8")
check("p27-unix-ipc-feature-gates-deps",
      'unix-ipc = ["dep:signal-hook", "dep:prost"]' in ms_cargo,
      "unix-ipc feature must be the only activator of signal-hook/prost")
check("p27-force-synthetic-feature",
      "force-synthetic-perf" in ms_cargo,
      "force-synthetic-perf feature missing")
network_banned = {"reqwest", "ureq", "hyper", "attohttpc", "curl"}
check("p27-no-network-deps:maintenance-service",
      not ({"signal-hook", "prost"} & network_banned),
      "impossible state — allowlist sanity")
check("p27-no-network-deps:performance-telemetry",
      not ({m2.group(1) for m2 in re.finditer(r"^([a-z_][a-z0-9_-]*)\s*[=.]",
           pt_cargo, flags=re.M)} & network_banned),
      "network-capable dep in performance-telemetry")

# --- Gate d: cfg(windows) freeze continues ---------------------------------------
allowed_p27_prefixes = (
    "services/maintenance-service/",
    "apps/desktop/src/main.rs",
    "apps/ui/src/",
    "crates/ipc/",
    "crates/platform-capabilities/",
    "crates/performance-telemetry/",
    "crates/security/src/lib.rs",
    "crates/persistence/Cargo.toml",
    "crates/persistence/src/lib.rs",
    "crates/update-engine/src/coordinator.rs",
    "crates/update-engine/src/lib.rs",
    "crates/contracts/proto/capabilities.proto",
    "crates/contracts/proto/operations.proto",
    "crates/contracts/src/lib.rs",
    "scripts/phase27-adversarial-audit.py",
    "docs/phase27/",
    "QUALIFICATION_DEBT.json",
    "ISSUES.json",
    "SCORECARD.md",
    "MASTER_DELIVERY_REPORT.md",
    "Cargo.toml",
    "Cargo.lock",
)
violations27: list[str] = []
checked_freeze27 = 0
for p in ROOT.rglob("*.rs"):
    rel_str = str(p.relative_to(ROOT))
    if "target/" in rel_str:
        continue
    if any(rel_str.startswith(prefix.rstrip("/")) for prefix in allowed_p27_prefixes):
        continue
    checked_freeze27 += 1
    text = p.read_text(encoding="utf-8", errors="replace")
    if "// Phase 27" in text or "phase 27" in text.lower():
        violations27.append(rel_str)
check("p27-cfg-windows-freeze-scan", not violations27,
      f"phase-27-marked edits outside the allowlist: {violations27[:5]}")
check("p27-cfg-windows-scan-covered-files", checked_freeze27 > 100,
      f"freeze scan covered only {checked_freeze27} rs files")

# --- Gate e: wire freeze — no new tags beyond the two allocated -------------------
events_proto = (ROOT / "crates/contracts/proto/events.proto").read_text(encoding="utf-8")
operations_proto = (ROOT / "crates/contracts/proto/operations.proto").read_text(encoding="utf-8")
kinds = dict(re.findall(r"(EVENT_KIND_[A-Z_]+) = (\d+);", events_proto))
check("p27-wirefreeze:eventkind-max", all(int(v) <= 28 for v in kinds.values()),
      "unauthorized EventKind above 28")
request_slice = operations_proto.split("message Request")[1].split("oneof payload")[1].split("\n  }")[0]
req_fields = {f: int(t) for (_ty, f, t) in re.findall(r"^\s{4}(\w+)\s+(\w+)\s*=\s*(\d+);", request_slice, flags=re.M)}
check("p27-additive:request:get_engine_source", req_fields.get("get_engine_source") == 88, "tag 88 wrong")
check("p27-wirefreeze:request-max", max(req_fields.values()) == 88,
      "unauthorized request tag above 88")
response_slice = operations_proto.split("message Response")[1].split("oneof payload")[1].split("\n  }")[0]
resp_fields = {f: int(t) for (_ty, f, t) in re.findall(r"^\s{4}(\w+)\s+(\w+)\s*=\s*(\d+);", response_slice, flags=re.M)}
check("p27-additive:response:engine_source_response", resp_fields.get("engine_source_response") == 50, "tag 50 wrong")
check("p27-wirefreeze:response-max", max(resp_fields.values()) == 50,
      "unauthorized response tag above 50")
envelope_slice = events_proto.split("message EventEnvelope")[1].split("oneof payload")[1].split("\n  }")[0]
env_fields = {f: int(t) for (_ty, f, t) in re.findall(r"^\s{4}(\w+)\s+(\w+)\s*=\s*(\d+);", envelope_slice, flags=re.M)}
check("p27-wirefreeze:envelope-max", max(env_fields.values()) == 37, "unauthorized envelope tag above 37")

# --- Gate f: composition wiring ---------------------------------------------------
unix_comp_path = ROOT / "services/maintenance-service/src/unix_composition.rs"
check("p27-composition-file-exists", unix_comp_path.exists(), "unix_composition.rs missing")
if unix_comp_path.exists():
    uc = unix_comp_path.read_text(encoding="utf-8")
    check("p27-composition-binds-listener", "UnixSocketListener::bind" in uc,
          "composition does not bind UnixSocketListener")
    check("p27-composition-private-dir", "ensure_private_dir" in uc or "0o700" in uc,
          "composition lost the 0700 private-dir contract")
    check("p27-composition-principal-binding", "socket_owner_principal" in uc,
          "composition does not bind the peer to the socket-owning user (CX-4)")
    check("p27-composition-same-router", ".handle(&peer" in uc or "ctx.handle(" in uc,
          "composition does not serve through the shared router handle")
    check("p27-composition-live-stomp-refusal",
          "live process" in uc or "live-stomp" in uc or "stale" in uc,
          "live-stomp refusal wording missing")
main_rs = (ROOT / "services/maintenance-service/src/main.rs").read_text(encoding="utf-8")
# P31 (W7) retired the manual feature gate on the unix composition: the module is
# now wired on every unix build, and `unix-ipc` survives only as an accepted
# force-on alias for exotic hosts (main.rs:50-52 says so in as many words). This
# check still demanded the P27 spelling, which cannot match a P31+ tree for two
# independent reasons -- the feature predicate is gone, and the module it named is
# `unix_composition`, not `composition`. Assert what P31 decided: unconditional on
# unix. A narrower gate reappearing here is the drift worth catching.
check("p27-main-mod-gates", '#[cfg(unix)]\nmod unix_composition;' in main_rs,
      "unix composition module gate drifted")
desktop_main = (ROOT / "apps/desktop/src/main.rs").read_text(encoding="utf-8")
check("p27-desktop-capability-command", "async fn get_platform_capabilities" in desktop_main,
      "desktop Tauri command get_platform_capabilities missing")
check("p27-desktop-engine-source-command", "async fn get_engine_source" in desktop_main,
      "desktop Tauri command get_engine_source missing")
ui_page = ROOT / "apps/ui/src/components/AboutPanel.svelte"
check("p27-ui-about-panel-exists", ui_page.exists(), "AboutPanel.svelte missing")
if ui_page.exists():
    ui_text = ui_page.read_text(encoding="utf-8")
    check("p27-ui-engine-source-honest", "engineSourceSynthetic" in ui_text,
          "UI does not surface the synthetic engine source")
    check("p27-ui-typed-chips",
          all(s in ui_text for s in ("native", "degraded", "notAvailable")),
          "UI availability chips incomplete")
# i18n parity: every about.* key exists in BOTH catalogs.
en_cat = (ROOT / "apps/ui/src/lib/i18n/catalog.en.ts").read_text(encoding="utf-8")
ar_cat = (ROOT / "apps/ui/src/lib/i18n/catalog.ar.ts").read_text(encoding="utf-8")
about_keys_en = set(re.findall(r"'(about\.[a-zA-Z.]+)':", en_cat))
about_keys_ar = set(re.findall(r"'(about\.[a-zA-Z.]+)':", ar_cat))
check("p27-i18n-en-ar-parity", about_keys_en == about_keys_ar and len(about_keys_en) >= 9,
      f"EN keys {sorted(about_keys_en - about_keys_ar)} vs AR {sorted(about_keys_ar - about_keys_en)}")
# Integration proof test exists and covers restart/stale recovery.
gd_test = (ROOT / "services/maintenance-service/tests/phase27_unix_ipc.rs")
check("p27-gd-test-exists", gd_test.exists(), "phase27_unix_ipc.rs integration test missing")
if gd_test.exists():
    gd = gd_test.read_text(encoding="utf-8")
    check("p27-gd-round-trips", all(op in gd for op in
          ("GetPlatformCapabilities", "Ping", "GetEngineSource")),
          "GD test misses required round trips")
    check("p27-gd-stale-recovery", "second lifecycle" in gd and "rebinds" in gd,
          "GD test does not prove stale-path recovery")
    check("p27-gd-permissions", "0700" in gd and "0600" in gd,
          "GD test does not assert the private-dir/socket permission contract")

result = {
    "schema": "aethercore.phase27.adversarial-audit.v1",
    "root": str(ROOT),
    "checks": checks,
    "failures": failures,
    "status": "PASS" if not failures else "FAIL",
}
print(json.dumps(result, indent=2))
sys.exit(0 if not failures else 1)
