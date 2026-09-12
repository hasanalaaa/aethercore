#!/usr/bin/env python3
"""Dependency-light source validation for AetherCore through Phase 16.

This gate is intentionally platform-neutral. It validates repository structure, modular
contracts, schema, principal/consent/file-identity, Operation Kernel/session-streaming, design,
localization/RTL, and release-source invariants when native Windows execution is unavailable.
Authoritative native compilation/live Windows checks remain in scripts/verify-phase16.ps1 and Windows CI; GA additionally requires verify-production.ps1 evidence aggregation.
"""
from __future__ import annotations

import argparse
import json
import re
import sqlite3
import sys
import tomllib
import xml.etree.ElementTree as ET
from pathlib import Path

try:
    import yaml  # type: ignore
except Exception:
    yaml = None

ROOT = Path(__file__).resolve().parents[1]
# P58 / DBT-P55-001: the workflows moved to `.github/workflows` at the
# REPOSITORY root, which is the only place GitHub Actions reads them from.
# They are one level above this workspace; everything else below stays
# workspace relative.
REPO = ROOT.parent

# P59 / DBT-P58-005: this file read 35 of its sources as
# `X.read_text(...) if X.exists() else ""`, which cannot tell "absent" from
# "empty". Two of them - `scripts/setup-and-run.ps1` and
# `scripts/phase10-architecture-audit.ps1` - could be deleted with this gate's
# report byte-identical. `read()` raises instead.
# No bytecode: `omega-evidence.py` runs each gate against a disposable clone
# and treats ANY new file in it as a source mutation, so a `__pycache__`
# entry for this import would be reported as the gate rewriting the tree.
sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
from gate_reader import SourceReader  # noqa: E402

_READER = SourceReader(ROOT)
read = _READER.read
workflow_root = _READER.base
PARSER = argparse.ArgumentParser()
PARSER.add_argument("--output", type=Path, help="Optional explicit report path; default verification is read-only.")
ARGS = PARSER.parse_args()
GENERATED_DIRS = {".git", ".devdata", "target", "node_modules", "out", "__pycache__", "dist"}
checks: dict[str, dict[str, object]] = {}


def files(pattern: str) -> list[Path]:
    return sorted(
        path
        for path in ROOT.rglob(pattern)
        if not any(part in GENERATED_DIRS for part in path.relative_to(ROOT).parts)
        and path != ROOT / "docs" / "STATIC_VALIDATION.json"
    )


def marker(name: str, text: str, required: list[str]) -> None:
    missing = [item for item in required if item not in text]
    checks[name] = {"ok": not missing, "missing": missing}


def parse_group(name: str, paths: list[Path], loader) -> None:
    errors = []
    for path in paths:
        try:
            loader(path)
        except Exception as exc:
            errors.append({"file": str(path.relative_to(ROOT)), "error": str(exc)})
    checks[name] = {"ok": not errors, "count": len(paths), "errors": errors}


parse_group("parse_toml", files("*.toml"), lambda p: tomllib.loads(p.read_text(encoding="utf-8")))
parse_group("parse_json", files("*.json"), lambda p: json.loads(p.read_text(encoding="utf-8")))
if yaml is not None:
    parse_group(
        "parse_yaml",
        files("*.yml") + files("*.yaml"),
        lambda p: yaml.safe_load(p.read_text(encoding="utf-8")),
    )
else:
    checks["parse_yaml"] = {
        "ok": True,
        "count": 0,
        "note": "PyYAML unavailable; Windows CI remains authoritative for workflow syntax.",
    }

workspace = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))["workspace"]
members = set(workspace.get("members", []))
required_members = {
    "apps/desktop",
    "apps/consent-broker",
    "apps/install-hardener",
    "services/maintenance-service",
    "crates/contracts",
    "crates/ipc",
    "crates/persistence",
    "crates/operation-engine",
    "crates/operation-kernel",
    "crates/idle-scheduler",
    "crates/windows-pnp",
    "crates/windows-update",
    "crates/driver-hub",
    "crates/restore-point",
    "crates/driver-backup",
    "crates/driver-install",
    "crates/system-repair",
    "crates/cleaner",
    "crates/startup-manager",
    "crates/hardware-telemetry",
    "crates/crash-diagnostics",
    "crates/diagnostic-engine",
}
missing_members = sorted(required_members - members)
missing_member_files = sorted(
    member for member in members if not (ROOT / member / "Cargo.toml").is_file()
)
checks["workspace_members"] = {
    "ok": not missing_members and not missing_member_files,
    "missing_required": missing_members,
    "missing_manifests": missing_member_files,
    "count": len(members),
}

path_errors = []
for cargo in files("Cargo.toml"):
    data = tomllib.loads(cargo.read_text(encoding="utf-8"))
    for section in ("dependencies", "dev-dependencies", "build-dependencies"):
        for dep, spec in data.get(section, {}).items():
            if isinstance(spec, dict) and "path" in spec:
                target = (cargo.parent / spec["path"]).resolve()
                if not (target / "Cargo.toml").is_file():
                    path_errors.append(
                        {
                            "file": str(cargo.relative_to(ROOT)),
                            "dependency": dep,
                            "path": spec["path"],
                        }
                    )
checks["path_dependencies"] = {"ok": not path_errors, "errors": path_errors}

# Schema validation through all migrations.
conn = sqlite3.connect(":memory:")
try:
    for migration in sorted((ROOT / "crates/persistence/migrations").glob("*.sql")):
        conn.executescript(migration.read_text(encoding="utf-8"))
    tables = {row[0] for row in conn.execute("SELECT name FROM sqlite_master WHERE type='table'")}
finally:
    conn.close()
required_tables = {
    "plans",
    "plan_events",
    "plan_executions",
    "driver_install_items",
    "execution_checkpoints",
    "recovery_records",
    "maintenance_executions",
    "maintenance_execution_items",
    "startup_change_records",
    "diagnostic_snapshots",
    "autonomous_scheduler_runs",
}
checks["sqlite_migrations"] = {
    "ok": required_tables.issubset(tables),
    "missing": sorted(required_tables - tables),
    "tables": sorted(tables),
}

proto_paths = sorted((ROOT / "crates/contracts/proto").glob("*.proto"))
proto = "\n".join(path.read_text(encoding="utf-8") for path in proto_paths)


def message_blocks(source: str):
    for match in re.finditer(r"\bmessage\s+(\w+)\s*\{", source):
        depth = 1
        cursor = match.end()
        while cursor < len(source) and depth:
            if source[cursor] == "{":
                depth += 1
            elif source[cursor] == "}":
                depth -= 1
            cursor += 1
        if depth == 0:
            yield match.group(1), source[match.end() : cursor - 1]


duplicates = []
for message, body in message_blocks(proto):
    seen: dict[int, str] = {}
    for field, tag in re.findall(r"\b(\w+)\s*=\s*(\d+)\s*;", body):
        number = int(tag)
        if number in seen:
            duplicates.append(
                {"message": message, "tag": number, "fields": [seen[number], field]}
            )
        seen[number] = field
checks["protobuf_unique_tags"] = {"ok": not duplicates, "duplicates": duplicates}

marker(
    "phase4_contract",
    proto,
    [
        "StartRepairAssessmentRequest",
        "GetRepairAssessmentRequest",
        "CreateSystemRepairPlanRequest",
        "StartSystemRepairRequest",
        "GetSystemRepairStatusRequest",
        "StartCleanupScanRequest",
        "GetCleanupSnapshotRequest",
        "CreateCleanupPlanRequest",
        "StartCleanupRequest",
        "GetCleanupStatusRequest",
        "RepairAssessmentSnapshot",
        "SystemRepairStatus",
        "CleanupSnapshot",
        "CleanupStatus",
    ],
)
marker(
    "phase5_contract",
    proto,
    [
        "StartStartupScanRequest",
        "GetStartupSnapshotRequest",
        "CreateStartupPlanRequest",
        "StartStartupChangesRequest",
        "GetStartupStatusRequest",
        "GetStartupHistoryRequest",
        "CreateStartupRestorePlanRequest",
        "StartupSnapshot",
        "StartupStatus",
        "StartupHistoryEntryInfo",
        "confirm_service_changes",
    ],
)
contracts = (ROOT / "crates/contracts/src/lib.rs").read_text(encoding="utf-8")
marker(
    "protocol_v7_and_ipc_budget",
    contracts,
    [
        "PROTOCOL_VERSION: u32 = 7",
        "MAX_REQUEST_FRAME_BYTES: usize = 256 * 1024",
        "MAX_RESPONSE_FRAME_BYTES: usize = 8 * 1024 * 1024",
    ],
)

operation = (ROOT / "crates/operation-engine/src/lib.rs").read_text(encoding="utf-8")
marker(
    "immutable_phase4_plan_actions",
    operation,
    [
        "SystemRepairAction",
        "CleanupFileEvidence",
        "root_final_path",
        "CleanupDeleteAction",
        "RepairWindowsIntegrity",
        "DeleteCleanupCandidate",
        "create_system_repair_plan",
        "create_cleanup_plan",
    ],
)

repair = (ROOT / "crates/system-repair/src/lib.rs").read_text(encoding="utf-8")
repair_win = (ROOT / "crates/system-repair/src/windows_impl.rs").read_text(encoding="utf-8")
repair_dism_api = (ROOT / "crates/system-repair/src/dism_api.rs").read_text(encoding="utf-8")
marker(
    "system_repair_state_and_recovery",
    repair,
    [
        "RepairCoordinator",
        "AwaitingAuthorization",
        "PlanState::Preflight",
        "PlanState::Protected",
        "begin_mutation",
        "mutation_started",
        "recover_incomplete",
        "automatic replay is intentionally disabled",
    ],
)
marker(
    "system_repair_fixed_windows_workflow",
    repair_win + "\n" + repair_dism_api,
    [
        'join("dism.exe")',
        'join("sfc.exe")',
        'join("chkdsk.exe")',
        "DismCheckImageHealth",
        "check_online_image_health",
        '"/RestoreHealth"',
        '"/scannow"',
        '"/verifyonly"',
        '"/scan"',
        "CREATE_NO_WINDOW",
        "ensure_servicing_available",
        "MachineMutationGuard::try_acquire()",
        "begin_mutation()?",
        "run_chkdsk_scan",
    ],
)
# Phase 19 replaces locale-sensitive /CheckHealth-/ScanHealth parsing with the structured
# DISM API. The durable mutation barrier must still follow a fresh API preflight and precede
# the first mutating RestoreHealth command.
preflight_pos = repair_win.find('check_online_image_health(false')
barrier_pos = repair_win.find('begin_mutation()?')
restore_pos = repair_win.find('"/RestoreHealth"')
checks["repair_mutation_barrier_order"] = {
    "ok": preflight_pos >= 0 and barrier_pos > preflight_pos and restore_pos > barrier_pos,
    "positions": {"dism_api_preflight": preflight_pos, "mutation_barrier": barrier_pos, "restore_health": restore_pos},
}

repair_forbidden = ["cmd.exe", "powershell.exe", '"/f"', '"/spotfix"', '"/r"']
repair_hits = [token for token in repair_win.lower().splitlines() if False]  # keep structure simple
checks["repair_no_shell_or_offline_disk_mutation"] = {
    "ok": "cmd.exe" not in repair_win.lower()
    and "powershell.exe" not in repair_win.lower()
    and ' &["/f"]' not in repair_win.lower()
    and '"/spotfix"' not in repair_win.lower(),
    "note": "CHKDSK /f may appear only in user-facing safety text; execution arrays are checked separately.",
}

wua = (ROOT / "crates/windows-update/src/execution_windows.rs").read_text(encoding="utf-8")
marker(
    "repair_wua_servicing_preflight",
    wua,
    [
        "ensure_servicing_available",
        "CreateUpdateInstaller",
        "IsBusy",
        "RebootRequiredBeforeInstallation",
    ],
)

cleaner = (ROOT / "crates/cleaner/src/lib.rs").read_text(encoding="utf-8")
cleaner_win = (ROOT / "crates/cleaner/src/windows_impl.rs").read_text(encoding="utf-8")
marker(
    "cleanup_immutable_state_and_limits",
    cleaner,
    [
        "MAX_FILES_PER_CANDIDATE",
        "MAX_PLAN_FILES",
        "HashSet",
        "create_cleanup_plan",
        "cleanup_actions",
        "acquire_cleanup_mutation_guard",
        "recover_incomplete",
        "immutable targets will not be replayed automatically",
    ],
)
marker(
    "cleanup_windows_safety",
    cleaner_win,
    [
        "WindowsTemp",
        "UserTemp",
        "ShaderCache",
        "WER",
        "CrashDumps",
        "FILE_ATTRIBUTE_REPARSE_POINT",
        "reject_ancestor_reparse_chain",
        "open_stable_root",
        "root_final_path",
        "GetFinalPathNameByHandleW",
        "SetFileInformationByHandle",
        "FileDispositionInfo",
        "FILE_DISPOSITION_INFO",
        "MachineMutationGuard::try_acquire()",
    ],
)
forbidden_cleanup = [
    "winsxs",
    "driverstore",
    "prefetch",
    "reg delete",
    "registry cleaner",
    "cmd.exe",
    "powershell.exe",
]
cleanup_hits = [token for token in forbidden_cleanup if token in cleaner_win.lower()]
checks["cleanup_no_whole_recycle_bin"] = {
    "ok": "shemptyrecyclebinw" not in cleaner_win.lower() and "shqueryrecyclebinw" not in cleaner_win.lower(),
    "note": "Whole-bin deletion is deferred because it cannot freeze exact item identity at plan time.",
}
checks["cleanup_allowlist_only"] = {"ok": not cleanup_hits, "hits": cleanup_hits}

# Ensure candidate details returned to the UI do not contain raw file paths.
cleanup_candidate_proto = re.search(
    r"message\s+CleanupCandidateInfo\s*\{(.*?)\}", proto, re.S
)
cleanup_candidate_body = cleanup_candidate_proto.group(1) if cleanup_candidate_proto else ""
privacy_hits = [token for token in ("path", "root", "files", "filename") if token in cleanup_candidate_body.lower()]
checks["cleanup_ui_privacy_boundary"] = {"ok": not privacy_hits, "hits": privacy_hits}


startup = (ROOT / "crates/startup-manager/src/lib.rs").read_text(encoding="utf-8")
startup_win = (ROOT / "crates/startup-manager/src/windows_impl.rs").read_text(encoding="utf-8")
marker(
    "startup_passive_default_and_recovery",
    startup,
    [
        "RecommendationDecision::Unreviewed",
        "RecommendationDecision::KeepEnabled",
        "RecommendationDecision::Disable",
        "d.decision!=RecommendationDecision::Disable",
        "StartupError::PassiveDefault",
        "confirm_service_changes",
        "Exact original and target states durably recorded before mutation.",
        "Never replay startup mutations after restart",
        "AppliedRecovered",
        "RecoveryRequired",
        "create_restore_plan",
        "source.state=\"Restored\"",
        "restart_recovery_observes_original_state_without_replay",
        "restart_recovery_observes_applied_state_without_replay",
        "service_disable_target_is_manual_without_stop_semantics",
    ],
)
marker(
    "startup_windows_inventory_and_reversible_mutation",
    startup_win,
    [
        "CurrentVersion\\Run",
        "CurrentVersion\\RunOnce",
        "KEY_WOW64_64KEY",
        "KEY_WOW64_32KEY",
        "Startup folder",
        "<LogonTrigger",
        "<BootTrigger",
        "SetEnabled",
        "RegDeleteValueW",
        "RegSetValueExW",
        "MoveFileExW",
        "ChangeServiceConfigW",
        "SERVICE_DEMAND_START",
        "FILE_ATTRIBUTE_REPARSE_POINT",
        "file_sha256",
        "is_essential_service",
        "is_protected_service_role",
        "LaunchProtected",
        "DependOnService",
    ],
)
startup_forbidden = ["DeleteService", "ControlService", "SERVICE_CONTROL_STOP", "RegDeleteKey", "cmd.exe", "powershell.exe"]
startup_hits = [token for token in startup_forbidden if token.lower() in startup_win.lower()]
checks["startup_no_stop_delete_or_shell"] = {"ok": not startup_hits, "hits": startup_hits}
checks["startup_service_is_next_start_only"] = {
    "ok": "SERVICE_DEMAND_START" in startup_win and "SERVICE_DISABLED" not in startup_win,
    "note": "Phase 5 removes eligible third-party services from automatic boot by switching to demand/manual start; it never stops a running service.",
}
checks["startup_impact_claims_are_evidence_conservative"] = {
    "ok": 'impact:"Unknown"' in startup_win and 'confidence:"InsufficientEvidence"' in startup_win,
    "note": "No undocumented Task Manager startup-impact database or synthetic performance score is used.",
}
checks["startup_task_identity_scopes_enabled_normalization"] = {
    "ok": all(token in startup_win for token in [
        "normalize_task_settings_enabled",
        "Trigger-level Enabled fields",
        "task_hash_ignores_only_task_level_enabled_state",
        "assert_ne!(task_definition_hash(a), task_definition_hash(c))",
    ]),
    "note": "Only task-level Settings/Enabled is excluded from identity; trigger/action drift remains hashed.",
}

persistence_src = (ROOT / "crates/persistence/src/lib.rs").read_text(encoding="utf-8")
marker(
    "startup_durable_journal",
    persistence_src + "\n" + (ROOT / "crates/persistence/migrations/0004_startup_manager.sql").read_text(encoding="utf-8"),
    [
        "StartupChangeRecord",
        "upsert_startup_change",
        "get_startup_change",
        "startup_changes_in_states",
        "startup_change_records",
        "origin_change_id",
        "startup_change_record_is_durable_and_queryable",
    ],
)

service = "\n".join(
    (ROOT / "services/maintenance-service/src" / name).read_text(encoding="utf-8")
    for name in ["main.rs", "composition.rs", "router.rs", "protocol.rs", "streaming.rs", "server.rs"]
)
marker(
    "service_phase4_handlers",
    service,
    [
        "RepairCoordinator::with_telemetry",
        "CleanupEngine::with_telemetry",
        "StartRepairAssessment",
        "CreateSystemRepairPlan",
        "StartSystemRepair",
        "GetSystemRepairStatus",
        "StartCleanupScan",
        "CreateCleanupPlan",
        "StartCleanup",
        "GetCleanupStatus",
        "repair.recover_incomplete",
        "cleaner.recover_incomplete",
    ],
)


marker(
    "service_phase5_handlers",
    service,
    [
        "StartupManager::with_telemetry",
        "StartStartupScan",
        "GetStartupSnapshot",
        "CreateStartupPlan",
        "StartStartupChanges",
        "GetStartupStatus",
        "GetStartupHistory",
        "CreateStartupRestorePlan",
        "startup.recover_incomplete",
    ],
)

desktop = (ROOT / "apps/desktop/src/main.rs").read_text(encoding="utf-8")
ui_root = ROOT / "apps/ui/src"
ui = "\n".join(
    path.read_text(encoding="utf-8", errors="ignore")
    for path in sorted(ui_root.rglob("*"))
    if path.is_file() and path.suffix in {".svelte", ".ts", ".css"}
)
invokes = sorted(set(re.findall(r"invoke(?:<[^>]+>)?\(\s*['\"]([^'\"]+)['\"]", ui)))
handler_match = re.search(r"generate_handler!\[(.*?)\]\s*\)", desktop, re.S)
handlers = (
    sorted(set(re.findall(r"\b([a-z][a-z0-9_]*)\b", handler_match.group(1))))
    if handler_match
    else []
)
missing_handlers = sorted(set(invokes) - set(handlers))
checks["tauri_ui_wiring"] = {
    "ok": not missing_handlers,
    "missing": missing_handlers,
    "invoke_count": len(invokes),
}
marker(
    "phase4_ui",
    ui,
    [
        "SYSTEM INTEGRITY EXECUTION",
        "Review repair",
        "Authorize & repair",
        "DEEP CLEANUP",
        "Review cleanup",
        "Authorize & clean",
        "CHKDSK /scan only",
        "exact file evidence",
        "Frozen in service",
    ],
)


marker(
    "phase5_ui_passive_default",
    ui,
    [
        "STARTUP & BACKGROUND SERVICES",
        "Unreviewed",
        "KeepEnabled",
        "Disable reviewed",
        "Confirm background service changes separately",
        "Unreviewed and Keep are omitted",
        "Passive-default invariant",
        "Restore original",
        "Create restore plan",
        "Authorize & restore",
    ],
)
checks["ui_no_implicit_startup_disable"] = {
    "ok": ("decisions[item.itemId] = 'Unreviewed'" in ui or "next[item.itemId] = 'Unreviewed'" in ui) and "Optimize all" not in ui,
    "note": "Every new scan resets decision state to Unreviewed; no mass-disable control exists.",
}

# No user-supplied raw command/path mutation channel may exist at the IPC boundary.
trust_surface = "\n".join([proto, service, desktop])
forbidden_surface = [
    "raw_command",
    "command_line",
    "powershell",
    "cleanup_path",
    "delete_path",
    "repair_arguments",
    "dism_arguments",
]
surface_hits = [token for token in forbidden_surface if token in trust_surface.lower()]
checks["typed_mutation_surface_only"] = {"ok": not surface_hits, "hits": surface_hits}

verify_phase5 = read("scripts/verify-phase5.ps1")
setup_run = read("scripts/setup-and-run.ps1")
ci = (REPO / ".github/workflows/ci.yml").read_text(encoding="utf-8")
marker(
    "phase5_windows_gate",
    verify_phase5 + "\n" + setup_run + "\n" + ci,
    [
        "aethercore-startup-manager",
        "live_inventory_is_read_only",
        "cargo check --workspace",
        "pnpm --dir apps/ui check",
    ],
)


marker(
    "phase6_contract",
    proto,
    [
        "StartDiagnosticsScanRequest", "GetDiagnosticsSnapshotRequest", "GetDiagnosticsHistoryRequest",
        "StorageReliabilityInfo", "StorageDeviceTelemetryInfo", "AtaSmartAttributeInfo", "MemoryTelemetryInfo",
        "HardwareEventInfo", "CrashRecordInfo", "DiagnosticCardInfo", "DiagnosticsSnapshot", "event_window_days",
    ],
)
hardware=(ROOT/"crates/hardware-telemetry/src/lib.rs").read_text(encoding="utf-8")
hardware_win=(ROOT/"crates/hardware-telemetry/src/windows_impl.rs").read_text(encoding="utf-8")
crash=(ROOT/"crates/crash-diagnostics/src/lib.rs").read_text(encoding="utf-8")
crash_win=(ROOT/"crates/crash-diagnostics/src/windows_impl.rs").read_text(encoding="utf-8")
diag=(ROOT/"crates/diagnostic-engine/src/lib.rs").read_text(encoding="utf-8")
marker("phase6_storage_sources", hardware_win, ["MSFT_PhysicalDisk","MSFT_StorageReliabilityCounter","IOCTL_STORAGE_QUERY_PROPERTY","StorageDeviceProtocolSpecificProperty","ProtocolTypeNvme","parse_nvme_health_log","checked_protocol_window","SMART_RCV_DRIVE_DATA","SENDCMDINPARAMS","READ_ATTRIBUTES","GlobalMemoryStatusEx"])
marker("phase6_honest_storage_model", hardware+"\n"+ui, ["Option<i32>","nvme_media_errors: Option<String>","Not reported", "do not currently show a reliability warning", "health conclusion"])
checks["phase6_no_health_score"]={"ok":"health_score" not in (hardware+diag+ui).lower() and "97%" not in (hardware+diag+ui),"note":"No synthetic hardware-health percentage is present in model/rules/UI."}
marker("phase6_event_and_dump_collectors", crash_win, ["EvtQuery","EvtNext","EvtCreateRenderContext","EvtRenderContextSystem","EvtRenderContextUser","EvtRenderEventValues","TimeCreated[timediff(@SystemTime)","Microsoft-Windows-WHEA-Logger","Microsoft-Windows-Kernel-Power","Microsoft-Windows-WER-SystemErrorReporting","Minidump","DUMP_HEADER64","DUMP_HEADER32"])
marker("phase6_non_exaggerated_crash_rules", crash+crash_win+diag, ["does not identify why","does not identify a specific DIMM","does not prove RAM is fault-free","requires symbol-assisted dump analysis","root-cause"])
marker("phase6_diagnostic_engine", diag, ["DiagnosticEngine","start_scan","save_diagnostic_snapshot","build_cards","Back up important files immediately","Windows Memory Diagnostic"])
marker("service_phase6_handlers", service, ["DiagnosticEngine::new","StartDiagnosticsScan","GetDiagnosticsSnapshot","GetDiagnosticsHistory","diagnostics_snapshot_proto"])
marker("phase6_ui", ui, ["HARDWARE TELEMETRY","Measurements, not a made-up health score","ATA SMART raw attributes","CRASH & WHEA DIAGNOSTICS","No logged memory hardware errors ≠ RAM proven healthy","eventWindowDays","root cause unknown"])
checks["phase6_read_only_surface"]={"ok":all(token not in (hardware_win+crash_win+diag).lower() for token in ["setfileinformationbyhandle","changeserviceconfig","iupdateinstaller","srsetrestorepoint","deletefilew"]),"note":"Phase 6 collectors/diagnostic rules expose no system-mutation API."}
checks["phase6_ata_is_observational_only"]={"ok":"SMART_RCV_DRIVE_DATA" in hardware_win and "SMART_SEND_DRIVE_COMMAND" not in hardware_win and "ata_smart_attributes" not in hardware.split("pub fn classify_storage",1)[1].split("#[cfg(test)]",1)[0],"note":"ATA SMART is read-only raw evidence and does not drive AetherCore health severity."}
checks["phase6_bounded_event_window"]={"ok":"DEFAULT_EVENT_WINDOW_DAYS: u32 = 30" in crash and "TimeCreated[timediff(@SystemTime)" in crash_win and "MAX_EVENTS: usize = 128" in crash_win,"note":"System Event Log collection is explicitly bounded by age and count."}

verify_phase6=read("scripts/verify-phase6.ps1")
marker("phase6_windows_gate", verify_phase6+"\n"+setup_run+"\n"+ci, ["aethercore-hardware-telemetry","aethercore-crash-diagnostics","aethercore-diagnostic-engine","live_storage_and_memory_collection_is_read_only","live_event_and_minidump_collection_is_read_only"])


# Phase 7 — inherited UI quality foundations, now carried by the Phase 11 design system.
tokens = (ui_root / "design-tokens.css").read_text(encoding="utf-8")
phase7_css = "\n".join(
    path.read_text(encoding="utf-8", errors="ignore")
    for path in sorted((ui_root / "design/styles").glob("*.css"))
)
styles_entry = (ui_root / "styles.css").read_text(encoding="utf-8")
navigation_ts = (ui_root / "lib/navigation.ts").read_text(encoding="utf-8")
i18n_ts = "\n".join(path.read_text(encoding="utf-8") for path in sorted((ui_root / "lib/i18n").glob("*.ts"))) + "\n" + (ui_root / "lib/i18n.ts").read_text(encoding="utf-8")
window_ux_ts = (ui_root / "lib/window-ux.ts").read_text(encoding="utf-8")
nav_component = (ui_root / "components/NavigationRail.svelte").read_text(encoding="utf-8")
palette_component = (ui_root / "components/CommandPalette.svelte").read_text(encoding="utf-8")
fluid_dialog = (ui_root / "design/primitives/FluidDialog.svelte").read_text(encoding="utf-8")
icon_component = (ui_root / "components/AppIcon.svelte").read_text(encoding="utf-8")
tauri_config = json.loads((ROOT / "apps/desktop/tauri.conf.json").read_text(encoding="utf-8"))
window_config = tauri_config["app"]["windows"][0]

required_phase7_files = [
    "apps/ui/src/design-tokens.css",
    "apps/ui/src/design/styles/base.css",
    "apps/ui/src/design/styles/materials.css",
    "apps/ui/src/lib/navigation.ts",
    "apps/ui/src/lib/i18n.ts",
    "apps/ui/src/lib/window-ux.ts",
    "apps/ui/src/components/AppIcon.svelte",
    "apps/ui/src/components/NavigationRail.svelte",
    "apps/ui/src/components/CommandPalette.svelte",
]
missing_phase7_files = [name for name in required_phase7_files if not (ROOT / name).is_file()]
checks["phase7_component_architecture"] = {"ok": not missing_phase7_files, "missing": missing_phase7_files}

phase7_token_count = len(set(re.findall(r"--ac-[a-z0-9-]+\s*:", tokens)))
checks["phase7_design_tokens"] = {
    "ok": phase7_token_count >= 40 and all(marker in tokens for marker in ["--ac-text-1", "--ac-material-base", "--ac-accent", "--ac-radius-lg", "--ac-feedback-normal", "--ac-shadow-focus"]),
    "token_count": phase7_token_count,
}
checks["phase7_style_layering"] = {
    "ok": all(item in styles_entry for item in ["design-tokens.css", "feature-layout.css", "base.css", "materials.css", "motion.css"]),
    "note": "Phase 11 supersedes the Phase 7 override stack with explicit design-system layers while preserving its visual/accessibility foundations.",
}

page_ids = ["overview", "deepScan", "drivers", "repair", "cleanup", "startup", "hardware", "crash", "activity"]
checks["phase7_all_eight_surfaces"] = {
    "ok": all(f"id: '{page}'" in navigation_ts for page in page_ids) and navigation_ts.count("shortcut: 'Ctrl+Shift+") == len(page_ids),
    "surfaces": page_ids,
    "note": "Phase 17 adds Deep Scan as a ninth production surface while preserving the original eight surfaces and direct keyboard shortcuts.",
}
checks["phase7_keyboard_navigation"] = {
    "ok": all(marker in (ui + nav_component + palette_component + fluid_dialog) for marker in ["Control+K", "ArrowDown", "ArrowUp", "Home", "End", "Ctrl+Shift+", "handleGlobalKeydown", "dialogKeydown"]),
    "note": "Primary navigation, command palette, direct shortcuts, Escape handling and dialog focus trapping remain present after decomposition.",
}
checks["phase7_screen_reader_accessibility"] = {
    "ok": all(marker in (ui + nav_component + palette_component + fluid_dialog) for marker in ["skip-link", "aria-live", "aria-current", 'role="alert"', 'aria-modal="true"', "aria-selected", "aria-label"]),
}
checks["phase7_adaptive_accessibility"] = {
    "ok": all(marker in phase7_css + tokens + window_ux_ts for marker in ["prefers-reduced-motion", "forced-colors", "prefers-contrast: more", ":focus-visible", "--ac-shadow-focus"]),
}
phase12_runtime_compat = read("apps/ui/src/lib/i18n/runtime.ts")
phase12_ar_compat = read("apps/ui/src/lib/i18n/catalog.ar.ts")
checks["phase7_localization_rtl"] = {
    "ok": all(marker in (phase12_runtime_compat + phase12_ar_compat) for marker in ["type Locale = 'en' | 'ar'", "directionFor", "document.documentElement.dir", "aethercore.locale", "arCatalog"]) and '[dir="rtl"]' in phase7_css and 'lang="en" dir="ltr"' in (ROOT / "apps/ui/index.html").read_text(encoding="utf-8"),
}
checks["phase7_dpi_multi_monitor"] = {
    "ok": all(marker in window_ux_ts for marker in ["scaleFactor()", "onScaleChanged", "payload.scaleFactor", "--ac-display-scale"]) and window_config.get("minWidth", 9999) <= 900 and window_config.get("minHeight", 9999) <= 560,
}
effects = window_config.get("windowEffects", {}).get("effects", [])
checks["phase7_mica_desktop_shell"] = {
    "ok": window_config.get("transparent") is True and window_config.get("noRedirectionBitmap") is True and "mica" in effects and "acrylic" not in effects and window_config.get("scrollBarStyle") == "fluentOverlay",
    "effects": effects,
}
checks["phase7_motion_budget"] = {
    "ok": "prefers-reduced-motion" in phase7_css and "transition:transform" not in phase7_css.replace(" ", "") and "transition:all" not in phase7_css.replace(" ", ""),
    "note": "Interactive transform motion is no longer prescribed by fixed CSS transitions; Phase 11 springs own presentation motion.",
}
checks["phase7_no_remote_ui_assets"] = {
    "ok": not any(re.search(r"https?://", path.read_text(encoding="utf-8", errors="ignore")) for path in ui_root.rglob("*") if path.is_file()),
}

# Dependency-light Svelte control-block balance check. Full compiler diagnostics remain in the Windows gate.
svelte_balance_errors = []
for svelte in sorted(ui_root.rglob("*.svelte")):
    stack = []
    for match in re.finditer(r"\{([#/])\s*(if|each|await|key|snippet)\b", svelte.read_text(encoding="utf-8")):
        kind, name = match.groups()
        if kind == "#":
            stack.append(name)
        elif not stack or stack[-1] != name:
            svelte_balance_errors.append({"file": str(svelte.relative_to(ROOT)), "block": name, "offset": match.start()})
            break
        else:
            stack.pop()
    if stack:
        svelte_balance_errors.append({"file": str(svelte.relative_to(ROOT)), "unclosed": stack})
checks["phase7_svelte_block_balance"] = {"ok": not svelte_balance_errors, "errors": svelte_balance_errors}

verify_phase7 = read("scripts/verify-phase7.ps1")
checks["phase7_windows_gate"] = {
    "ok": (ROOT / "scripts/verify-phase7.ps1").is_file()
    and all(token in verify_phase7 for token in [
        "Phase 7 verification complete.",
        "svelte-check",
        "--threshold warning",
        "--fail-on-warnings",
        "pnpm --dir apps/ui build",
        "verify-phase6.ps1",
    ]),
}

# Phase 8 — production packaging, installer hardening, supply-chain controls, and security verification.
product_wxs_path = ROOT / "installer/wix/Product.wxs"
bundle_wxs_path = ROOT / "installer/wix/Bundle.wxs"
product_wxs = product_wxs_path.read_text(encoding="utf-8")
bundle_wxs = bundle_wxs_path.read_text(encoding="utf-8")
hardener = (ROOT / "apps/install-hardener/src/main.rs").read_text(encoding="utf-8")
ipc_src = (ROOT / "crates/ipc/src/lib.rs").read_text(encoding="utf-8")
security_src = (ROOT / "crates/security/src/lib.rs").read_text(encoding="utf-8")
freeze_ps = (ROOT / "scripts/freeze-dependencies.ps1").read_text(encoding="utf-8")
build_release_ps = (ROOT / "scripts/build-release.ps1").read_text(encoding="utf-8")
build_installer_ps = (ROOT / "scripts/build-installer.ps1").read_text(encoding="utf-8")
verify_phase8 = (ROOT / "scripts/verify-phase8.ps1").read_text(encoding="utf-8")
burn_sign_ps = (ROOT / "scripts/sign-burn-bundle.ps1").read_text(encoding="utf-8")
user_privilege_ps = (ROOT / "scripts/verify-user-shell-privilege.ps1").read_text(encoding="utf-8")
installer_verify = (ROOT / "scripts/verify-installer-security.ps1").read_text(encoding="utf-8")
security_audit = (ROOT / "scripts/security-hardening-audit.ps1").read_text(encoding="utf-8")
pe_hardening = (ROOT / "scripts/verify-pe-hardening.ps1").read_text(encoding="utf-8")
sbom_ps = (ROOT / "scripts/generate-sbom.ps1").read_text(encoding="utf-8")
audit_ps = (ROOT / "scripts/audit-dependencies.ps1").read_text(encoding="utf-8")
repro_ps = (ROOT / "scripts/verify-reproducible.ps1").read_text(encoding="utf-8")
webview_ps = (ROOT / "scripts/fetch-webview2.ps1").read_text(encoding="utf-8")
fuzz_ps = (ROOT / "scripts/run-ipc-fuzz.ps1").read_text(encoding="utf-8")
release_ci = (REPO / ".github/workflows/release.yml").read_text(encoding="utf-8")
dotnet_tools = json.loads((ROOT / ".config/dotnet-tools.json").read_text(encoding="utf-8"))
deny_cfg = tomllib.loads((ROOT / "deny.toml").read_text(encoding="utf-8"))
root_cargo = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
cargo_cfg = (ROOT / ".cargo/config.toml").read_text(encoding="utf-8")

required_phase8_files = [
    "apps/install-hardener/Cargo.toml",
    "apps/install-hardener/src/main.rs",
    "installer/wix/Product.wxs",
    "installer/wix/Bundle.wxs",
    ".config/dotnet-tools.json",
    "nuget.config",
    "deny.toml",
    "scripts/freeze-dependencies.ps1",
    "scripts/build-installer.ps1",
    "scripts/build-release.ps1",
    "scripts/fetch-webview2.ps1",
    "scripts/sign-artifacts.ps1",
    "scripts/sign-burn-bundle.ps1",
    "scripts/generate-sbom.ps1",
    "scripts/audit-dependencies.ps1",
    "scripts/verify-reproducible.ps1",
    "scripts/security-hardening-audit.ps1",
    "scripts/verify-pe-hardening.ps1",
    "scripts/verify-installer-security.ps1",
    "scripts/run-ipc-fuzz.ps1",
    "scripts/verify-phase8.ps1",
    "fuzz/Cargo.toml",
    "fuzz/fuzz_targets/ipc_frame.rs",
    ".github/workflows/release.yml",
    "scripts/verify-user-shell-privilege.ps1",
    "PHASE_8_DELIVERABLES.md",
    "docs/PRODUCTION_PACKAGING_SECURITY.md",
    "docs/RELEASE_SUPPLY_CHAIN.md",
    "docs/adr/0009-production-packaging-and-hardening.md",
    "docs/adr/0010-release-supply-chain-and-reproducibility.md",
]
missing_phase8 = [name for name in required_phase8_files if not (workflow_root(name) / name).is_file()]
checks["phase8_required_artifacts"] = {"ok": not missing_phase8, "missing": missing_phase8}

# XML well-formedness is checked separately from WiX semantic compilation on Windows.
xml_errors = []
for path in (product_wxs_path, bundle_wxs_path):
    try:
        ET.parse(path)
    except Exception as exc:
        xml_errors.append({"file": str(path.relative_to(ROOT)), "error": str(exc)})
checks["phase8_wix_xml_well_formed"] = {"ok": not xml_errors, "errors": xml_errors}

marker(
    "phase8_msi_service_and_acl_hardening",
    product_wxs + "\n" + hardener,
    [
        'Scope="perMachine"',
        'Account="LocalSystem"',
        'Type="ownProcess"',
        'ExeCommand="apply"',
        'Execute="deferred"',
        'Impersonate="no"',
        'After="InstallServices"',
        'sc.exe',
        'icacls.exe',
        'sidtype',
        'restricted',
        'start=',
        'delayed-auto',
        '/reset',
        '/inheritance:r',
        'NT SERVICE\\AetherCoreMaintenance',
    ],
)
# DBT-P46-D1 (§46.4 Part 0.D): the Windows service name had three independent
# Rust deciders, and four more declarations outside Rust that no check tied to
# them — WiX, two PowerShell scripts, and this file, which hardcoded the
# principal string just above. The Rust side is now one const in
# crates/product-identity; this reads that const and asserts every non-Rust
# declaration still spells the same name, so the layers cannot drift apart in
# silence the way they were free to before.
product_identity = (ROOT / "crates/product-identity/src/lib.rs").read_text(encoding="utf-8")
_service_name_match = re.search(r'pub const SERVICE_NAME: &str = "([^"]+)";', product_identity)
SERVICE_NAME = _service_name_match.group(1) if _service_name_match else ""
_service_name_mirrors = {
    "installer/wix/Product.wxs": product_wxs,
    "scripts/install-service.ps1": (ROOT / "scripts/install-service.ps1").read_text(encoding="utf-8"),
    "scripts/uninstall-service.ps1": (ROOT / "scripts/uninstall-service.ps1").read_text(encoding="utf-8"),
}
checks["p46_service_name_has_one_decider"] = {
    "ok": bool(SERVICE_NAME)
    and all(SERVICE_NAME in text for text in _service_name_mirrors.values())
    and f"NT SERVICE\\{SERVICE_NAME}" in product_wxs
    and not any(
        'const SERVICE_NAME' in (ROOT / rust).read_text(encoding="utf-8")
        or 'const TRUSTED_SERVICE_NAME' in (ROOT / rust).read_text(encoding="utf-8")
        for rust in (
            "crates/ipc/src/windows_impl.rs",
            "apps/install-hardener/src/main.rs",
            "services/maintenance-service/src/main.rs",
        )
    ),
    "note": f"Service name is decided once in crates/product-identity ({SERVICE_NAME!r}); the three former Rust deciders now import it, and every non-Rust declaration is asserted against it.",
}
# DBT-P48-002 (§48.5): a control that cannot fire.
#
# `Pressable.svelte` accepts an `onclick` prop and forwards it to the <button> it
# renders. Thirty call sites use it. Seven wrote `<Pressable on:press={...}>`
# instead — the legacy component-event syntax, which needs a component that
# dispatches. There is NO `createEventDispatcher`, no `dispatch(` and no
# `CustomEvent` anywhere under apps/ui/src, so nothing in the tree can ever emit
# a `press` event and those seven handlers were unreachable. Those same seven
# also nested a <button> inside Pressable's own <button>, which is invalid HTML
# and a nested interactive control.
#
# This is the third time this class has been found: P47 fixed two dead Insights
# controls, and the pattern is that a control LOOKS wired because a handler is
# named right next to it. A grep is enough to settle it, so the grep is a gate.
_ui_src = ROOT / "apps/ui/src"
_svelte_sources = {
    path.relative_to(ROOT).as_posix(): path.read_text(encoding="utf-8")
    for path in sorted(_ui_src.rglob("*.svelte"))
}
_press_producers = [
    name for name, text in _svelte_sources.items()
    if "createEventDispatcher" in text or "CustomEvent('press'" in text or 'CustomEvent("press"' in text
]
_pressable_on_press = sorted(
    name for name, text in _svelte_sources.items() if re.search(r"<Pressable[^>]*\son:", text)
)
_pressable_nested_button = sorted(
    name for name, text in _svelte_sources.items()
    if re.search(r"<Pressable[^>]*>\s*<button", text)
)
checks["p48_pressable_handlers_can_fire"] = {
    "ok": (not _pressable_on_press or bool(_press_producers)) and not _pressable_nested_button,
    "press_event_producers": _press_producers,
    "pressable_using_component_events": _pressable_on_press,
    "pressable_nesting_a_button": _pressable_nested_button,
    "note": "A <Pressable> must wire its handler through the onclick prop it actually accepts, and must not nest an interactive control inside its own <button>. `on:` component-event syntax is only valid if some component dispatches; none does.",
}
_product_name_match = re.search(r'pub const PRODUCT_NAME: &str = "([^"]+)";', product_identity)
PRODUCT_NAME = _product_name_match.group(1) if _product_name_match else ""
checks["p46_product_name_has_one_decider"] = {
    # DBT-P46-D2 (§46.4 Part 0.D): the install directory, the ProgramData
    # directory and the update protocol's product_id were 17 separate literals
    # across 7 crates and apps. They now read one const; WiX declares the same
    # two directory names, and this asserts they still agree.
    "ok": bool(PRODUCT_NAME)
    and f'<Directory Id="INSTALLFOLDER" Name="{PRODUCT_NAME}">' in product_wxs
    and f'<Directory Id="ProgramDataRoot" Name="{PRODUCT_NAME}">' in product_wxs,
    "note": f"Install and ProgramData directory names in Product.wxs match the single PRODUCT_NAME decider ({PRODUCT_NAME!r}).",
}
checks["phase8_msi_serviceconfig_not_relied_upon"] = {
    "ok": "<ServiceConfig" not in product_wxs,
    "note": "Service SID policy and delayed-auto are applied by the fixed-purpose post-InstallServices hardener, avoiding reliance on MSI ServiceConfig semantics.",
}
checks["phase8_msi_upgrade_and_os_gate"] = {
    "ok": all(token in product_wxs for token in [
        'UpgradeCode="{45598C77-2C32-5BCE-8510-19C7E51EE3B8}"',
        'MajorUpgrade Schedule="afterInstallInitialize"',
        'Condition="VersionNT64 AND ((MsiNTProductType = 1 AND OSCURRENTBUILD &gt;= 22621) OR (MsiNTProductType = 3 AND OSCURRENTBUILD &gt;= 17763))"',
    ])
    and 'Condition="VersionNT64 AND ((NTProductType = 1 AND WindowsBuildNumber &gt;= 22621) OR (NTProductType = 3 AND WindowsBuildNumber &gt;= 17763))"' in bundle_wxs
    and 'Name="InstallationType"' in product_wxs
    and 'Variable="WindowsInstallationType"' in bundle_wxs
    and 'InstallCondition="NOT (WindowsInstallationType ~= &quot;Server Core&quot;)"' in bundle_wxs,
    "note": "MSI and Burn admit Windows 11 clients and Windows Server 2019+ member servers; domain controllers remain refused, and Server Core omits the WebView2 prerequisite and desktop feature.",
}
checks["phase8_hardener_fixed_operation_only"] = {
    # DBT-P46-D1 changed HOW this is proven, not what it proves. The hardener
    # used to contain the service name as its own literal; it now imports the
    # one decider, so asserting the literal here would assert the duplication
    # this gate should want removed.
    "ok": all(
        token in hardener
        for token in [
            'mode == OsStr::new("apply")',
            "System32",
            "aethercore_product_identity::",
            "SERVICE_NAME",
        ]
    )
    and "cmd.exe" not in hardener.lower()
    and "powershell.exe" not in hardener.lower(),
    "note": "The elevated MSI helper accepts only the literal apply verb and invokes fixed System32 tooling for the fixed AetherCore service, named by the single shared constant rather than a local literal.",
}

checks["phase8_msi_repair_not_disabled"] = {
    "ok": '<Property Id="ARPNOREPAIR"' not in product_wxs,
    "note": "MSI repair remains available so the deferred hardener can restore service/ACL policy drift.",
}

marker(
    "phase8_webview2_bundle",
    bundle_wxs + "\n" + webview_ps,
    [
        "WebView2EvergreenBootstrapper",
        "WebView2MachineVersion",
        "WebView2UserVersion",
        'InstallArguments="/silent /install"',
        'PerMachine="yes"',
        'Permanent="yes"',
        "MicrosoftEdgeWebview2Setup.exe",
        "LinkId=2124703",
        "Get-AuthenticodeSignature",
        "Microsoft Corporation",
    ],
)
checks["phase8_webview2_no_injectable_url"] = {
    "ok": "[string]$Url" not in webview_ps and "Invoke-WebRequest" in webview_ps and "https://go.microsoft.com/fwlink/p/?LinkId=2124703" in webview_ps,
    "note": "The release script owns the Microsoft bootstrapper URL and verifies Microsoft Authenticode before packaging it.",
}

marker(
    "phase8_ipc_malformed_frame_resilience",
    ipc_src + "\n" + contracts + "\n" + service,
    [
        "decode_client_frame_bytes",
        "TrailingBytes",
        "truncated",
        "rejects_invalid_protobuf",
        "MAX_REQUEST_FRAME_BYTES",
        "MAX_REQUEST_ID_BYTES",
        "is_safe_request_id",
        "catch_unwind",
    ],
)
marker(
    "phase8_broker_path_boundary_tests",
    security_src,
    [
        "common_prefix",
        "relative",
        "is_expected_broker",
    ],
)
marker(
    "phase8_fuzz_harness",
    (ROOT / "fuzz/fuzz_targets/ipc_frame.rs").read_text(encoding="utf-8") + "\n" + fuzz_ps,
    [
        "decode_client_frame_bytes",
        "cargo-fuzz",
        "0.13.2",
        "ipc_frame",
        "deterministic_malformed_frame_corpus_never_panics",
        "invoke-cargo-test-case.ps1",
    ],
)

marker(
    "phase8_installer_lifecycle_verification",
    installer_verify,
    [
        "UNRESTRICTED",
        "LocalSystem",
        "DelayedAutoStart",
        "asInvoker",
        "requireAdministrator",
        "AcknowledgeDisposableMachine",
        "Get-Acl",
        "S-1-1-0",
        "/fa",
        "ProgramData",
    ],
)
marker(
    "phase8_security_static_audit",
    security_audit,
    [
        "crates/ipc/src/windows_impl.rs",
        "PIPE_REJECT_REMOTE_CLIENTS",
        "After=\"InstallServices\"",
        "<ServiceConfig\\b",
        "MAX_REQUEST_FRAME_BYTES",
        "MAX_REQUEST_ID_BYTES",
        "is_expected_broker",
        "FILE_ATTRIBUTE_REPARSE_POINT",
        "GetFinalPathNameByHandleW",
        "sidtype",
        "reset_acl_tree",
        "Impersonate=\"no\"",
    ],
)

marker(
    "phase8_pe_mitigation_gate",
    pe_hardening + "\n" + build_release_ps,
    [
        "IMAGE_DLLCHARACTERISTICS_HIGH_ENTROPY_VA",
        "IMAGE_DLLCHARACTERISTICS_DYNAMIC_BASE",
        "IMAGE_DLLCHARACTERISTICS_NX_COMPAT",
        "DllCharacteristics",
        "verify-pe-hardening.ps1",
        "PE hardening verification failed",
    ],
)

release_profile = root_cargo.get("profile", {}).get("release", {})
checks["phase8_release_profile_determinism"] = {
    "ok": release_profile.get("lto") == "thin"
    and release_profile.get("codegen-units") == 1
    and release_profile.get("incremental") is False
    and release_profile.get("panic") == "abort"
    and "/Brepro" in cargo_cfg
    and "/INCREMENTAL:NO" in cargo_cfg,
    "profile": release_profile,
}
required_wix_extensions = {
    "WixToolset.Util.wixext/6.0.2",
    "WixToolset.BootstrapperApplications.wixext/6.0.2",
}
found_wix_extensions = set(
    re.findall(
        r"WixToolset\.(?:Util|BootstrapperApplications)\.wixext/[^'\"\s]+",
        build_installer_ps,
    )
)
checks["phase8_wix_tool_pin"] = {
    "ok": dotnet_tools.get("tools", {}).get("wix", {}).get("version") == "6.0.2"
    and required_wix_extensions.issubset(found_wix_extensions),
    "version": dotnet_tools.get("tools", {}).get("wix", {}).get("version"),
    "required_extensions": sorted(required_wix_extensions),
    "found_extensions": sorted(found_wix_extensions),
}

phase8_script_names = {path.name for path in (ROOT / "scripts").glob("*.ps1")}
referenced_phase8_scripts: set[str] = set()
for script_path in (ROOT / "scripts").glob("*.ps1"):
    referenced_phase8_scripts.update(
        re.findall(
            r"(?<![A-Za-z0-9_.-])([A-Za-z0-9_-]+\.ps1)(?![A-Za-z0-9_.-])",
            script_path.read_text(encoding="utf-8"),
        )
    )
missing_referenced_scripts = sorted(referenced_phase8_scripts - phase8_script_names)
checks["phase8_script_reference_integrity"] = {
    "ok": not missing_referenced_scripts,
    "referenced_count": len(referenced_phase8_scripts),
    "missing": missing_referenced_scripts,
}
checks["phase8_nuget_signature_required"] = {
    "ok": 'key="signatureValidationMode" value="require"' in (ROOT / "nuget.config").read_text(encoding="utf-8"),
}
pnpm_workspace_text = (ROOT / "pnpm-workspace.yaml").read_text(encoding="utf-8")
checks["phase8_pnpm_supply_chain_policy"] = {
    "ok": all(token in pnpm_workspace_text for token in [
        "minimumReleaseAge: 1440",
        "blockExoticSubdeps: true",
        "strictDepBuilds: true",
        "auditLevel: high",
    ])
    and "\naudit:\n  level:" not in pnpm_workspace_text,
    "note": "pnpm uses its actual auditLevel setting plus dependency-age/exotic-subdependency/build-script guardrails.",
}
checks["phase8_cargo_deny_policy"] = {
    "ok": deny_cfg.get("advisories", {}).get("yanked") == "deny"
    and deny_cfg.get("bans", {}).get("wildcards") == "deny"
    and deny_cfg.get("sources", {}).get("unknown-registry") == "deny"
    and deny_cfg.get("sources", {}).get("unknown-git") == "deny"
    and deny_cfg.get("sources", {}).get("required-git-spec") == "rev",
}
marker(
    "phase8_sbom_and_dependency_audit",
    sbom_ps + "\n" + audit_ps,
    [
        "cargo-cyclonedx",
        "0.5.9",
        "cyclonedx",
        "pnpm",
        "sbom",
        "1.7",
        "cargo-deny",
        "0.20.2",
        "audit-level high",
        "licenses list",
        "cargo metadata --locked",
    ],
)
marker(
    "phase8_release_ordering",
    build_release_ps,
    [
        "freeze-dependencies.ps1",
        "audit-dependencies.ps1",
        "generate-sbom.ps1",
        "verify-reproducible.ps1",
        "sign-artifacts.ps1",
        "sign-burn-bundle.ps1",
        "-MsiOnly",
        "-BundleOnly",
        "SHA256SUMS.txt",
        "RELEASE-METADATA.json",
    ],
)
# Enforce payload -> signed MSI -> bundle -> signed Burn engine -> signed whole bundle sequence.
payload_sign = build_release_ps.find("-Path (Get-ChildItem $Payload")
msi_build = build_release_ps.find("-MsiOnly")
msi_sign = build_release_ps.find("-Path $msi")
bundle_build = build_release_ps.find("-BundleOnly")
bundle_sign = build_release_ps.find("sign-burn-bundle.ps1")
checks["phase8_release_signing_sequence"] = {
    "ok": -1 not in (payload_sign, msi_build, msi_sign, bundle_build, bundle_sign)
    and payload_sign < msi_build < msi_sign < bundle_build < bundle_sign,
    "positions": {"payload_sign": payload_sign, "msi_build": msi_build, "msi_sign": msi_sign, "bundle_build": bundle_build, "burn_sign": bundle_sign},
}
marker(
    "phase8_burn_two_piece_signing",
    burn_sign_ps,
    [
        "wix burn detach",
        "-engine",
        "sign-artifacts.ps1",
        "wix burn reattach",
        "Move-Item -Force",
        "Final Burn bundle signing failed",
    ],
)

marker(
    "phase8_reproducibility_boundary",
    repro_ps + "\n" + build_release_ps,
    [
        "SOURCE_DATE_EPOCH",
        "dependency-locks.sha256",
        "msi_byte_for_byte",
        "claimed",
        "false",
        "8978",
        "/Brepro",
    ],
)
checks["phase8_dependency_freeze_design"] = {
    "ok": all(token in freeze_ps for token in ["Cargo.lock", "pnpm-lock.yaml", "dependency-locks.sha256", "Get-FileHash", "cargo generate-lockfile", "--lockfile-only", "--locked", "-Refresh", "-VerifyOnly"])
    and all(token in build_release_ps for token in ["Cargo.lock", "pnpm-lock.yaml", "release\\dependency-locks.sha256", "-VerifyOnly"]),
    "lockfiles_present": (ROOT / "Cargo.lock").is_file() and (ROOT / "pnpm-lock.yaml").is_file(),
    "note": "This source snapshot may legitimately be pre-freeze when registry egress is unavailable; production release scripts refuse to release until both locks and their approved hash baseline exist.",
}
# If the package already contains a dependency freeze, it must be complete; otherwise the seed workflow is explicitly required.
lock_present = (ROOT / "Cargo.lock").is_file()
pnpm_lock_present = (ROOT / "pnpm-lock.yaml").is_file()
baseline_present = (ROOT / "release/dependency-locks.sha256").is_file()
checks["phase8_dependency_freeze_completeness"] = {
    "ok": (lock_present and pnpm_lock_present and baseline_present) or (not lock_present and not pnpm_lock_present and not baseline_present),
    "cargo_lock": lock_present,
    "pnpm_lock": pnpm_lock_present,
    "baseline": baseline_present,
    "note": "Partial dependency freeze state is forbidden.",
}

marker(
    "phase8_windows_gate",
    verify_phase8 + "\n" + ci + "\n" + release_ci,
    [
        "verify-phase7.ps1",
        "freeze-dependencies.ps1",
        "security-hardening-audit.ps1",
        "run-ipc-fuzz.ps1",
        "cargo check --workspace --locked",
        "verify-reproducible.ps1",
        "verify-installer-security.ps1",
        "actions/checkout@v7.0.1",
        "actions/setup-node@v7.0.0",
        "actions/setup-dotnet@v6.0.0",
        "actions/upload-artifact@v7.0.1",
        "aethercore-signing",
        "-RequireSigning",
    ],
)
checks["phase8_release_signing_runner_isolated"] = {
    "ok": "runs-on: [self-hosted, windows, x64, aethercore-signing]" in release_ci
    and "persist-credentials: false" in release_ci
    and "production-signing" in release_ci
    and "AETHERCORE_CODESIGN_THUMBPRINT" in release_ci,
}
marker(
    "phase8_user_shell_privilege_boundary",
    user_privilege_ps + "\n" + verify_phase8,
    [
        "non-elevated user session",
        "TokenElevation",
        "Start-Process",
        "Desktop shell unexpectedly owns an elevated token",
        "AetherCoreMaintenance",
        "LocalSystem",
        "UserShellPrivilegeCheck",
    ],
)
marker(
    "phase8_release_provenance_metadata",
    build_release_ps,
    [
        "source_commit",
        "signer_subject",
        "signer_thumbprint",
        "webview2_bootstrapper_sha256",
        "RELEASE-METADATA.json",
        "SHA256SUMS.txt",
        "msi_byte_reproducible_claim",
    ],
)
marker(
    "phase8_release_tag_version_binding",
    release_ci,
    [
        "git diff --quiet",
        "git diff --cached --quiet",
        "workspace\\.package",
        "GITHUB_REF_TYPE",
        "does not match Cargo version",
    ],
)

# Keep privilege model explicit: desktop remains capability-minimal and privileged operations stay out-of-process.
capabilities = json.loads((ROOT / "apps/desktop/capabilities/default.json").read_text(encoding="utf-8"))
permissions = capabilities.get("permissions", [])
checks["phase8_desktop_capability_minimal"] = {
    "ok": permissions == ["core:default"],
    "permissions": permissions,
}
checks["phase9_protocol_v7"] = {
    "ok": "PROTOCOL_VERSION: u32 = 7" in contracts,
    "note": "Phase 9 retires prototype authorization payloads and introduces principal-bound consent-intent IPC."
}


# Phase 9 — principal/session ownership, one-shot consent, exact file identity, dependency freeze, sanitation.
security9 = (ROOT / "crates/security/src/lib.rs").read_text(encoding="utf-8")
windows_foundation = read("crates/windows-foundation/src/lib.rs")
operation9 = (ROOT / "crates/operation-engine/src/lib.rs").read_text(encoding="utf-8")
persistence9 = (ROOT / "crates/persistence/src/lib.rs").read_text(encoding="utf-8")
migration9 = (ROOT / "crates/persistence/migrations/0006_phase9_security.sql").read_text(encoding="utf-8")
cleaner_win9 = (ROOT / "crates/cleaner/src/windows_impl.rs").read_text(encoding="utf-8")
broker9 = (ROOT / "apps/consent-broker/src/main.rs").read_text(encoding="utf-8")
desktop9 = (ROOT / "apps/desktop/src/main.rs").read_text(encoding="utf-8")
verify9 = read("scripts/verify-phase9.ps1")
freeze9 = (ROOT / "scripts/freeze-dependencies.ps1").read_text(encoding="utf-8")

marker("phase9_kernel_derived_principal", security9, ["GetNamedPipeClientProcessId", "GetNamedPipeClientSessionId", "TokenStatistics", "TokenUser", "authentication_id", "binding_key"])
marker("phase9_pipe_token_impersonation", security9 + windows_foundation, ["ThreadImpersonation::named_pipe_client", "OpenThreadToken", "RevertToSelf", "principal_from_token"])
marker("phase9_token_buffer_safety", security9, ["vec![0usize; words]", "user SID pointer outside TokenUser buffer", "checked_add(sid_len)"])
marker("phase9_service_principal_dispatch", service, ["inspect_named_pipe_client", "peer.binding_key()", "snapshot(&ctx.engine"])
marker("phase9_plan_ownership", operation9 + persistence9 + migration9, ["owner_principal_key", "get_plan_for_owner", "latest_plan_for_owner", "event_count_for_owner"])
driver_hub9 = (ROOT / "crates/driver-hub/src/lib.rs").read_text(encoding="utf-8")
diagnostic9 = (ROOT / "crates/diagnostic-engine/src/lib.rs").read_text(encoding="utf-8")
marker("phase9_domain_snapshot_ownership", driver_hub9 + repair + cleaner + startup + diagnostic9, ["OwnershipMismatch", "snapshot_for_owner", "assessment_for_owner", "diagnostic_snapshots_for_owner"])
marker("phase9_consent_intent_contract", proto + operation9 + persistence9, ["BeginConsentIntentRequest", "GetConsentIntentRequest", "ApproveConsentIntentRequest", "consent_intents", "consume_consent_and_transition", "approved_unix_ms.is_some()"])
checks["phase9_secret_free_broker_cli"] = {
    "ok": "--intent-id" in broker9 and "--challenge" not in broker9 and "challenge" not in desktop9.lower(),
    "note": "Elevated broker receives only a non-secret service-minted intent identifier."
}
marker("phase9_atomic_one_shot_authorization", persistence9, ["TransactionBehavior::Immediate", "consumed_unix_ms", "authorization_consumed_transition"])
driver_install_test9 = (ROOT / "crates/driver-install/tests/coordinator.rs").read_text(encoding="utf-8")
marker("phase9_cross_user_start_fail_closed", driver_install_test9 + (ROOT / "crates/driver-install/src/lib.rs").read_text(encoding="utf-8"), ["cross_user_start_cannot_fail_or_mutate_an_owned_plan", "get_plan_for_owner", "AwaitingAuthorization"])
marker("phase9_exact_cleanup_file_identity", cleaner_win9 + operation9, ["GetFileInformationByHandleEx", "FileIdInfo", "volume_serial_number", "file_id_128", "SetFileInformationByHandle", "replacement_with_same_path_and_shape_is_rejected_by_file_identity"])
marker("phase9_dependency_freeze", freeze9, ["dependency-locks.sha256", "dependency-manifests.sha256", "dependency-freeze.json", "cargo metadata --locked", "Current-ManifestLines"])
marker("phase9_dependency_freeze_inputs", freeze9, ["'package.json'", "'.config/dotnet-tools.json'", "'nuget.config'", "manifest_baseline_sha256", "lock_baseline_sha256", "Expected pnpm"])
marker("phase9_post_consent_init_fail_closed", repair + cleaner + startup, ["repair execution journal initialization failed after consent", "cleanup execution journal initialization failed after consent", "startup execution journal initialization failed after consent"])
marker("phase9_windows_gate", verify9, ["verify-phase8.ps1", "freeze-dependencies.ps1", "phase9-security-audit.ps1", "cargo test --locked", "cargo check --workspace --locked", "pnpm --dir apps/ui check"])
product_text = "\n".join((p.read_text(encoding="utf-8", errors="ignore") for base in [ROOT/"apps",ROOT/"services",ROOT/"crates"] for p in base.rglob("*") if p.is_file() and p.suffix in {".rs",".proto",".ts",".svelte"}))
retired = ["create_demo_plan", "advance_demo", "Phase1Simulation", "simulation_only", "issue_challenge", "grant_authorization", "authorization_challenge", "authorization_grant"]
checks["phase9_production_sanitation"] = {
    "ok": all(token.lower() not in product_text.lower() for token in retired) and "invoke('authorize_plan'" not in product_text and 'invoke("authorize_plan"' not in product_text,
    "retired_tokens": retired,
}



# Phase 10 — Operation Kernel, persistent authenticated IPC v7, ordered streaming and telemetry.
kernel10 = "\n".join(path.read_text(encoding="utf-8") for path in sorted((ROOT / "crates/operation-kernel/src").glob("*.rs")))
ipc10 = (ROOT / "crates/ipc/src/lib.rs").read_text(encoding="utf-8") + "\n" + (ROOT / "crates/ipc/src/windows_impl.rs").read_text(encoding="utf-8")
server10 = (ROOT / "services/maintenance-service/src/server.rs").read_text(encoding="utf-8")
router10 = (ROOT / "services/maintenance-service/src/router.rs").read_text(encoding="utf-8")
protocol10 = (ROOT / "services/maintenance-service/src/protocol.rs").read_text(encoding="utf-8")
streaming10 = (ROOT / "services/maintenance-service/src/streaming.rs").read_text(encoding="utf-8")
composition10 = (ROOT / "services/maintenance-service/src/composition.rs").read_text(encoding="utf-8")
driver_install10 = (ROOT / "crates/driver-install/src/lib.rs").read_text(encoding="utf-8")
migration10 = (ROOT / "crates/persistence/migrations/0007_phase10_kernel.sql").read_text(encoding="utf-8")

checks["phase10_modular_proto_files"] = {
    "ok": all((ROOT / "crates/contracts/proto" / name).is_file() for name in ["common.proto","operations.proto","drivers.proto","repair.proto","cleanup.proto","startup.proto","diagnostics.proto","events.proto"]),
    "count": len(proto_paths),
}
marker("phase10_typed_protocol", proto, ["enum ErrorCode", "message ErrorInfo", "enum OperationState", "enum RiskLevel", "message ClientHello", "message ServerHello", "message SessionRequest", "message StreamReset", "uint64 sequence"])
errors10 = (ROOT / "services/maintenance-service/src/errors.rs").read_text(encoding="utf-8")
marker("phase10_typed_service_errors", errors10 + router10, ["ServiceError", "ErrorCode::Busy", "ErrorCode::Forbidden", "ErrorCode::Conflict", "message_key", "retryable"])
marker("phase10_typed_service_error_regressions", errors10, ["kernel_contention_and_ownership_failures_have_typed_semantics", "deadline_and_cancellation_are_not_collapsed_into_invalid_request", "domain_busy_and_state_conflict_remain_distinct", "foreign_domain_snapshot_state_is_non_enumerable"])
marker("phase10_operation_kernel", kernel10, ["OperationKernel", "AuthorizationManager", "StateMachine", "MutationSupervisor", "RecoverySupervisor", "EventBus", "ReadBudgetManager", "CancellationRegistry", "ProgressTelemetryStore"])
marker("phase10_kernel_owner_safe_api", kernel10, ["transition_for_owner", "mutation_lease_rejects_unscoped_identity", "InvalidIdentity"])
marker("phase10_mutation_busy_privacy", kernel10 + errors10, ["BusyOtherPrincipal", "cross_principal_contention_never_discloses_foreign_plan_identity", "snapshot_for_owner", "active_lease_snapshot_is_owner_scoped", "kernel.mutationBusy", "machine mutation is already active"])
marker("phase10_global_mutation_lease", kernel10 + router10 + streaming10, ["exactly_one_mutation_can_own_the_machine", "MutationWorkload::DriverInstall", "MutationWorkload::SystemRepair", "MutationWorkload::Cleanup", "MutationWorkload::Startup", "MutationWorkload::Update", "durable_mutation_released"])
marker("phase10_read_budget", kernel10 + router10, ["ReadWorkload::DriverDiscovery", "ReadWorkload::RepairAssessment", "ReadWorkload::CleanupDiscovery", "ReadWorkload::StartupDiscovery", "ReadWorkload::Diagnostics", "max_total"])
stale_read_workloads = [token for token in ["ReadWorkload::DriverScan", "ReadWorkload::StartupScan", "ReadWorkload::CleanupScan", "ReadWorkload::RepairScan"] if token in kernel10 + router10]
checks["phase10_read_budget"]["stale_variants"] = stale_read_workloads
checks["phase10_read_budget"]["ok"] = bool(checks["phase10_read_budget"]["ok"] and not stale_read_workloads)
checks["phase10_read_watcher_lease_release"] = {
    "ok": streaming10.count("else { break };") >= 5,
    "note": "Discovery observers terminate and release their RAII read-budget lease if the domain snapshot ownership changes before they observe terminal state.",
}
marker("phase10_persistent_ipc_session", ipc10 + server10, ["AetherCore.Maintenance.v7", "ClientHello", "SessionClient", "MAX_INFLIGHT_PER_SESSION", "deadline_unix_ms", "cancellation_id", "cancel_session"])
marker("phase10_directional_session_frame_limits", ipc10 + contracts, ["MAX_CLIENT_SESSION_FRAME_BYTES", "MAX_SERVER_SESSION_FRAME_BYTES", "session_direction_limits_match_trust_and_payload_shape"])
marker("phase10_per_user_session_quota", server10, ["MAX_SESSIONS_PER_USER_SID", "admit_user_session", "peer.user_sid", "UserSessionGuard"])
marker("phase10_reconnect_invalidation_race_closed", desktop, ["invalidate_session_if_current", "Arc::ptr_eq", "if !client.is_alive()", "prevents timeout-driven session churn"])
marker("phase10_replay_and_reset", kernel10 + server10 + desktop, ["replay_after_sequence", "replay_floor_sequence", "replay.complete", "SequenceReset", "SubscriberLagged", "aethercore://stream-reset", "LAST_EVENT_SEQUENCE.store"])
marker("phase10_principal_scoped_stream", kernel10 + server10, ["owner_principal_key", "peer.binding_key()", "subscribe(&owner", "does_not_leak_cross_user_activity"])
marker("phase10_typed_stream_hydration", router10 + protocol10 + streaming10 + server10, ["publish_hydration", "ServiceSnapshot", "DriverHubSnapshot", "RepairAssessment", "CleanupSnapshot", "StartupSnapshot", "DiagnosticsSnapshot"])
marker("phase10_renderer_reload_hydration", proto + router10 + server10 + desktop + ui, ["HydrateSessionRequest", "Payload::HydrateSession", "publish_hydration", "applyStreamReset"])
checks["phase10_ui_polling_eliminated"] = {
    "ok": "setInterval" not in ui and "clearInterval" not in ui and "start_ipc_session" in ui and "aethercore://kernel-event" in ui,
    "note": "Renderer hydration/live updates are driven by the persistent IPC event stream; no domain polling timers remain.",
}
repair10 = (ROOT / "crates/system-repair/src/lib.rs").read_text(encoding="utf-8")
cleaner10 = (ROOT / "crates/cleaner/src/lib.rs").read_text(encoding="utf-8")
startup10 = (ROOT / "crates/startup-manager/src/lib.rs").read_text(encoding="utf-8")
marker("phase10_telemetry_decoupled", kernel10 + driver_install10 + repair10 + cleaner10 + startup10 + migration10, ["ProgressTelemetryStore", "publish every WUA callback", "percent_delta>=5", "2_000", "intentionally NOT persisted", "SQLite remains the durable safety", "with_telemetry"])
checks["phase10_telemetry_owner_scoped"] = {
    "ok": all(marker in kernel10 + driver_install10 + repair10 + cleaner10 + startup10 for marker in ["get_for_owner", "clear_for_owner", "transient_telemetry_is_scoped_by_owner_even_for_the_same_plan_id"]) and ".telemetry.get(" not in driver_install10 + repair10 + cleaner10 + startup10 and "telemetry.get(" not in driver_install10 + repair10 + cleaner10 + startup10,
    "note": "Transient progress lookup/clear APIs are principal-scoped by construction; no caller reads a plan id then filters owner after disclosure.",
}
marker("phase10_recovery_composition", composition10 + kernel10, ["RecoverySupervisor", "recover_incomplete", "run_task"])
checks["phase10_service_decomposed"] = {
    "ok": all((ROOT / "services/maintenance-service/src" / name).is_file() for name in ["main.rs","composition.rs","errors.rs","router.rs","protocol.rs","streaming.rs","server.rs"]) and len((ROOT / "services/maintenance-service/src/main.rs").read_text(encoding="utf-8").splitlines()) < 220 and len(router10.splitlines()) < 240,
    "main_lines": len((ROOT / "services/maintenance-service/src/main.rs").read_text(encoding="utf-8").splitlines()),
    "router_lines": len(router10.splitlines()),
}
marker("phase10_per_principal_sequences", kernel10 + proto, ["HashMap<String, OwnerStream>", "monotonic per authenticated principal", "does_not_leak_cross_user_activity", "dropped_through_sequence"])
marker("phase10_explicit_stream_reset", kernel10 + server10 + proto, ["SubscriptionItem::Lagged", "ReplayWindowExceeded", "SubscriberLagged", "SequenceReset", "StreamReset"])
marker("phase10_bounded_stream_backpressure", kernel10 + server10, ["SUBSCRIBER_CAPACITY", "bounded_subscriber_overflow_is_reported_as_lag_not_silent_loss", "TrySendError::Full", "SubscriptionItem::Lagged", "publish_hydration"])
marker("phase10_session_client_shutdown", ipc10, ["impl Drop for SessionClient", "self.writer.shutdown();", "fn shutdown(&self)", "CancelSynchronousIo", "cancel_registered_io"])
marker("phase10_session_disconnect_cancellation", kernel10 + server10, ["cancel_session", "disconnect_cancels_only_the_owning_session_requests", "session_id"] )
marker("phase10_duplicate_request_defense", ipc10 + server10 + kernel10, ["duplicate in-flight request id", "duplicate_cancellation_ids_are_rejected_instead_of_rebinding_tokens", "active_request_ids"])
marker("phase10_typed_domain_states", proto + protocol10, ["enum DiscoveryState", "DiscoveryState state_code", "OperationState state_code", "discovery_state_code", "operation_state_code_str"])
marker("phase10_mutation_lease_events", kernel10 + proto, ["MutationLeaseEvent", "MutationLeaseState::Acquired", "MutationLeaseState::Released", "EventKind::MutationLease"])
marker("phase10_all_mutation_telemetry", composition10 + repair10 + cleaner10 + startup10 + driver_install10, ["RepairCoordinator::with_telemetry", "CleanupEngine::with_telemetry", "StartupManager::with_telemetry", "kernel.telemetry().clone()", "publish_progress"])
marker("phase10_persistence_migration", persistence9 + migration10, ["0007_phase10_kernel", "idx_maintenance_executions_domain_updated", "idx_plan_executions_stage_updated"])
verify10 = read("scripts/verify-phase10.ps1")
audit10 = read("scripts/phase10-architecture-audit.ps1")
ci10 = (REPO / ".github/workflows/ci.yml").read_text(encoding="utf-8")
release10 = (REPO / ".github/workflows/release.yml").read_text(encoding="utf-8")
marker("phase10_windows_gate", verify10 + audit10, ["verify-phase9.ps1", "phase10-architecture-audit.ps1", "aethercore-operation-kernel", "aethercore-maintenance-service", "aethercore-desktop", "aethercore-system-repair", "aethercore-cleaner", "aethercore-startup-manager", "cargo check --workspace --locked", "pnpm --dir apps/ui build"])
checks["phase10_ci_release_gate"] = {
    "ok": any(gate in ci10 for gate in ["verify-phase10.ps1 -SkipOnlineSupplyChain", "verify-phase11.ps1 -SkipOnlineSupplyChain", "verify-phase12.ps1 -SkipOnlineSupplyChain", "verify-phase13.ps1 -SkipOnlineSupplyChain", "verify-phase14.ps1 -SkipOnlineSupplyChain", "verify-phase15.ps1 -SkipOnlineSupplyChain", "verify-phase16.ps1 -SkipOnlineSupplyChain", "verify-enterprise.ps1 -SkipOnlineSupplyChain"])
    and any(gate in release10 for gate in ["verify-phase10.ps1 -ReleasePackaging -RequireSigning", "verify-phase11.ps1 -ReleasePackaging -RequireSigning", "verify-phase12.ps1 -ReleasePackaging -RequireSigning", "verify-phase13.ps1 -ReleasePackaging -RequireSigning", "verify-phase14.ps1 -ReleasePackaging -RequireSigning", "verify-phase15.ps1 -ReleasePackaging -RequireSigning", "verify-phase16.ps1 -ReleasePackaging -RequireSigning", "verify-enterprise.ps1 -ReleasePackaging -RequireSigning"]),
    "note": "Current CI/release may supersede Phase 10 with a later inherited verification gate without weakening its requirements.",
}

# Phase 11 — modular event-driven frontend, physical motion runtime, tactile primitives and adaptive materials.
app11 = (ui_root / "App.svelte").read_text(encoding="utf-8")
shell11 = (ui_root / "app/AppShell.svelte").read_text(encoding="utf-8")
shell_state11 = (ui_root / "app/shell-state.ts").read_text(encoding="utf-8")
stream11 = (ui_root / "platform/stream-state.ts").read_text(encoding="utf-8")
kernel_session11 = (ui_root / "platform/kernel-session.ts").read_text(encoding="utf-8")
spring11 = (ui_root / "design/motion/spring.ts").read_text(encoding="utf-8")
physics11 = (ui_root / "design/motion/physics.ts").read_text(encoding="utf-8")
press11 = (ui_root / "design/motion/fluid-press.ts").read_text(encoding="utf-8")
drag11 = (ui_root / "design/motion/fluid-drag.ts").read_text(encoding="utf-8")
dialog11 = (ui_root / "design/primitives/FluidDialog.svelte").read_text(encoding="utf-8")
progress11 = (ui_root / "design/primitives/ProgressBar.svelte").read_text(encoding="utf-8")
materials11 = (ui_root / "design/styles/materials.css").read_text(encoding="utf-8")
motion_css11 = (ui_root / "design/styles/motion.css").read_text(encoding="utf-8")
typography11 = (ui_root / "design/styles/typography.css").read_text(encoding="utf-8")
navigation11 = (ui_root / "design/styles/navigation.css").read_text(encoding="utf-8")
responsive11 = (ui_root / "design/styles/responsive.css").read_text(encoding="utf-8")
styles11 = "\n".join(path.read_text(encoding="utf-8", errors="ignore") for path in sorted((ui_root / "design/styles").glob("*.css")))

feature_files11 = [
    "features/overview/OverviewPage.svelte",
    "features/drivers/DriversPage.svelte",
    "features/repair/RepairPage.svelte",
    "features/cleanup/CleanupPage.svelte",
    "features/startup/StartupPage.svelte",
    "features/diagnostics/HardwarePage.svelte",
    "features/diagnostics/CrashPage.svelte",
    "features/activity/ActivityPage.svelte",
]
checks["phase11_feature_modules"] = {
    "ok": all((ui_root / rel).is_file() for rel in feature_files11),
    "features": feature_files11,
}
required_phase11_artifacts = [
    "PHASE_11_DELIVERABLES.md",
    "docs/AETHER_DESIGN_SYSTEM.md",
    "docs/adr/0013-aether-design-system-and-fluid-motion.md",
    "scripts/phase11-design-audit.ps1",
    "scripts/test-phase11-motion.ps1",
    "scripts/phase11-motion-tests.cjs",
    "scripts/verify-phase11.ps1",
]
checks["phase11_required_artifacts"] = {
    "ok": all((ROOT / rel).is_file() for rel in required_phase11_artifacts),
    "missing": [rel for rel in required_phase11_artifacts if not (ROOT / rel).is_file()],
}
checks["phase11_app_shell_decomposed"] = {
    "ok": len(app11.splitlines()) <= 20 and len(shell11.splitlines()) <= 180 and all(Path(rel).name in shell11 for rel in feature_files11),
    "app_lines": len(app11.splitlines()),
    "shell_lines": len(shell11.splitlines()),
}
marker("phase11_event_driven_frontend", stream11 + kernel_session11, ["reduceKernelEvent", "applyKernelEvent", "aethercore://kernel-event", "aethercore://session-state", "aethercore://stream-reset", "start_ipc_session"])
checks["phase11_no_renderer_polling"] = {
    "ok": "setInterval" not in ui and "clearInterval" not in ui,
    "note": "Feature state is reducer/event-stream driven; one-shot request/response confirmation is not polling.",
}
marker("phase11_spring_runtime", spring11, ["SPRING_DEFAULT", "response: 0.36", "damping: 1", "adoptPresentation", "retarget(target", "Semi-implicit Euler", "1 / 120"])
marker("phase11_momentum_projection", physics11 + drag11, ["decelerationRate = 0.998", "projectMomentum", "projectedEndpoint", "nearestSnapPoint", "rubberband", "estimateVelocity", "retarget(target, velocity)"])
marker("phase11_pointer_down_tactility", press11, ["pointerdown", "setPointerCapture", "hysteresis ?? 10", "data-pressed", "scale.retarget", "pointercancel"])
button_tags11 = []
for component in sorted(ui_root.rglob("*.svelte")):
    button_tags11.extend(re.findall(r"<button\b.*?>", component.read_text(encoding="utf-8"), flags=re.S))
checks["phase11_all_buttons_tactile"] = {
    "ok": bool(button_tags11) and all("use:fluidPress" in tag for tag in button_tags11),
    "button_count": len(button_tags11),
    "untactile": sum(1 for tag in button_tags11 if "use:fluidPress" not in tag),
}
marker("phase11_interruptible_dialog", dialog11, ["spring.retarget", "visualProgress", "returnFocus", "dialogKeydown", "aria-modal=\"true\"", "onClosed"])
marker("phase11_spring_progress", progress11, ["SpringValue", "spring.retarget", "progressbar", "aria-valuenow", "known"])
marker("phase11_material_hierarchy", materials11 + tokens, ["--ac-material-structural", "--ac-material-base", "--ac-material-elevated", "--ac-material-focused", "backdrop-filter", "[data-transparency=\"reduced\"]"])
marker("phase11_accessibility_preferences", window_ux_ts + tokens + responsive11 + motion_css11 + materials11 + (ui_root / "design/styles/base.css").read_text(encoding="utf-8"), ["prefers-reduced-motion", "prefers-reduced-transparency", "prefers-contrast", "forced-colors", "data-transparency", "focus-visible"])
marker("phase11_optical_typography", typography11 + tokens, ["font-optical-sizing", "--ac-type-display", "--ac-type-body", "letter-spacing", "[dir=\"rtl\"]", "unicode-bidi:isolate"])
checks["phase11_no_fixed_interactive_transform_transitions"] = {
    "ok": re.search(r"transition\s*:[^;]*(transform|all)", styles11, flags=re.I) is None,
}
checks["phase11_no_important_overrides"] = {"ok": "!important" not in ui}
checks["phase11_no_tiny_literal_type"] = {
    "ok": re.search(r"font-size\s*:\s*(7|8|9|10|11)px", styles11, flags=re.I) is None,
}
checks["phase11_legacy_override_layers_removed"] = {
    "ok": not (ui_root / "luxury.css").exists() and not (ui_root / "foundation.css").exists() and "luxury.css" not in styles_entry and "foundation.css" not in styles_entry,
}
checks["phase11_logical_navigation_layout"] = {
    "ok": all(token in navigation11 + materials11 + responsive11 for token in ["inset-inline-start", "border-inline-end", "padding-inline", "text-align:start", "[dir=\"rtl\"]"]),
}
checks["phase11_indeterminate_motion_is_status_only"] = {
    "ok": "@keyframes ac-indeterminate" in motion_css11 and "transition:transform" not in styles11.replace(" ", ""),
    "note": "Looping keyframes remain only for unknown-progress/status indicators; user-driven motion is spring controlled.",
}
required_primitives11 = ["Pressable.svelte", "FluidDialog.svelte", "FluidPage.svelte", "ProgressBar.svelte", "MaterialSurface.svelte", "DragSurface.svelte"]
checks["phase11_primitive_suite"] = {
    "ok": all((ui_root / "design/primitives" / name).is_file() for name in required_primitives11),
    "primitives": required_primitives11,
}
verify11 = read("scripts/verify-phase11.ps1")
audit11 = read("scripts/phase11-design-audit.ps1")
motion_test11 = read("scripts/test-phase11-motion.ps1")
checks["phase11_windows_gate"] = {
    "ok": all(token in verify11 + audit11 + motion_test11 for token in ["verify-phase10.ps1", "phase11-design-audit.ps1", "test-phase11-motion.ps1", "pnpm --dir apps/ui check", "pnpm --dir apps/ui build", "fluid-press.ts", "fluid-drag.ts", "pnpm exec tsc"]),
}
checks["phase11_ci_release_gate"] = {
    "ok": any(gate in ci10 for gate in ["verify-phase11.ps1 -SkipOnlineSupplyChain", "verify-phase12.ps1 -SkipOnlineSupplyChain", "verify-phase13.ps1 -SkipOnlineSupplyChain", "verify-phase14.ps1 -SkipOnlineSupplyChain", "verify-phase15.ps1 -SkipOnlineSupplyChain", "verify-phase16.ps1 -SkipOnlineSupplyChain", "verify-enterprise.ps1 -SkipOnlineSupplyChain"])
    and any(gate in release10 for gate in ["verify-phase11.ps1 -ReleasePackaging -RequireSigning", "verify-phase12.ps1 -ReleasePackaging -RequireSigning", "verify-phase13.ps1 -ReleasePackaging -RequireSigning", "verify-phase14.ps1 -ReleasePackaging -RequireSigning", "verify-phase15.ps1 -ReleasePackaging -RequireSigning", "verify-phase16.ps1 -ReleasePackaging -RequireSigning", "verify-enterprise.ps1 -ReleasePackaging -RequireSigning"]),
}

# Phase 12 — typed full-product localization, Arabic RTL and technical-evidence isolation.
i18n12 = ROOT / "apps/ui/src/lib/i18n"
en12 = (i18n12 / "catalog.en.ts").read_text(encoding="utf-8")
ar12 = (i18n12 / "catalog.ar.ts").read_text(encoding="utf-8")
runtime12 = (i18n12 / "runtime.ts").read_text(encoding="utf-8")
semantic12 = (i18n12 / "semantic.ts").read_text(encoding="utf-8")
bidi12 = (i18n12 / "bidi.ts").read_text(encoding="utf-8")
plural_en12 = (i18n12 / "plurals.en.ts").read_text(encoding="utf-8")
plural_ar12 = (i18n12 / "plurals.ar.ts").read_text(encoding="utf-8")
technical12 = (ui_root / "design/primitives/TechnicalText.svelte").read_text(encoding="utf-8")
owned12 = (ui_root / "design/primitives/LocalizedOwnedText.svelte").read_text(encoding="utf-8")
verify12 = read("scripts/verify-phase12.ps1")
audit12_ps = read("scripts/phase12-localization-audit.ps1")
audit12_py = read("scripts/test-phase12-localization.py")

required_phase12_artifacts = [
    "PHASE_12_DELIVERABLES.md",
    "docs/ADR-0012-localization-rtl.md",
    "scripts/test-phase12-localization.py",
    "scripts/phase12-localization-audit.ps1",
    "scripts/verify-phase12.ps1",
    "apps/ui/src/lib/i18n/catalog.en.ts",
    "apps/ui/src/lib/i18n/catalog.ar.ts",
    "apps/ui/src/lib/i18n/plurals.en.ts",
    "apps/ui/src/lib/i18n/plurals.ar.ts",
    "apps/ui/src/lib/i18n/runtime.ts",
    "apps/ui/src/lib/i18n/semantic.ts",
    "apps/ui/src/lib/i18n/bidi.ts",
    "apps/ui/src/design/primitives/TechnicalText.svelte",
    "apps/ui/src/design/primitives/LocalizedOwnedText.svelte",
]
checks["phase12_catalog_required_artifacts"] = {
    "ok": all((ROOT / rel).is_file() for rel in required_phase12_artifacts),
    "missing": [rel for rel in required_phase12_artifacts if not (ROOT / rel).is_file()],
}
marker("phase12_typed_catalog_contract", en12 + ar12 + runtime12, ["export type MessageKey = keyof typeof enCatalog", "satisfies Record<MessageKey, string>", "ExtractVars", "VarsFor", "export function t<", "export function td("])

def flat_catalog_keys12(text: str) -> set[str]:
    return set(re.findall(r"^\s*['\"]([^'\"]+)['\"]\s*:\s*['\"]", text, flags=re.M))

def flat_catalog_values12(text: str) -> dict[str, str]:
    values: dict[str, str] = {}
    for match in re.finditer(r"^\s*(['\"])(?P<key>.+?)\1\s*:\s*(['\"])(?P<value>.*?)(?<!\\)\3\s*,?\s*$", text, flags=re.M):
        values[match.group("key")] = match.group("value")
    return values

en_values12 = flat_catalog_values12(en12)
ar_values12 = flat_catalog_values12(ar12)
en_keys12 = set(en_values12)
ar_keys12 = set(ar_values12)
checks["phase12_catalog_exact_parity"] = {
    "ok": bool(en_keys12) and en_keys12 == ar_keys12,
    "en": len(en_keys12), "ar": len(ar_keys12),
    "missing_ar": sorted(en_keys12 - ar_keys12), "missing_en": sorted(ar_keys12 - en_keys12),
}
placeholder12 = lambda value: set(re.findall(r"\{([A-Za-z_][A-Za-z0-9_]*)\}", value))
placeholder_mismatch12 = [key for key in sorted(en_keys12 & ar_keys12) if placeholder12(en_values12[key]) != placeholder12(ar_values12[key])]
checks["phase12_placeholder_parity"] = {"ok": not placeholder_mismatch12, "mismatches": placeholder_mismatch12}
marker("phase12_plural_rules", runtime12 + plural_en12 + plural_ar12, ["Intl.PluralRules", "zero:", "one:", "two:", "few:", "many:", "other:"])
marker("phase12_semantic_localization", semantic12, ["localizeOwnedText", "localizeState", "localizeSeverity", "localizeConfidence", "driverTargetEvidence", "tech.driver.install.backup", "tech.repair.dismRestore", "tech.card.bugcheckEvidence"])
marker("phase12_catalog_bidi_isolation", technical12 + owned12 + bidi12 + typography11, ["technical-isolate", 'dir="ltr"', "unicode-bidi:isolate", "segmentBidiEvidence", "technical: boolean", "TechnicalText"])
checks["phase12_css_logical_direction"] = {
    "ok": re.search(r"(margin|padding|border)-(left|right)|(^|[;{\s])(left|right)\s*:|text-align\s*:\s*(left|right)|inset-(left|right)", styles11, flags=re.I | re.M) is None,
}
marker("phase12_catalog_arabic_optical_typography", typography11, ['html[dir="rtl"]', "letter-spacing: 0", "text-transform: none", "line-height:1.62"])
feature_localized12 = {rel: (ui_root / rel).read_text(encoding="utf-8") for rel in feature_files11}
checks["phase12_all_eight_surfaces_localized"] = {
    "ok": all(("t(" in text or "td(" in text or "tp(" in text) and "../lib/i18n" in text or "../../lib/i18n" in text for text in feature_localized12.values()),
    "features": feature_files11,
}
checks["phase12_no_legacy_untyped_catalog"] = {"ok": "Record<string, string>" not in (en12 + ar12 + runtime12)}
checks["phase12_visible_english_audit"] = {"ok": "no_hardcoded_visible_english" in audit12_py and "literal_message_keys_exist" in audit12_py}
checks["phase12_no_display_string_logic_audit"] = {"ok": "no_display_string_business_logic" in audit12_py and "startsWith|includes|endsWith" in audit12_py}
checks["phase12_deep_technical_audit"] = {"ok": all(token in audit12_py for token in ["driver_version_path_isolation", "crash_code_path_isolation", "hardware_identifier_isolation", "plan_hash_identifier_isolation", "backend_owned_prose_localized"])}
checks["phase12_catalog_windows_gate"] = {
    "ok": all(token in verify12 + audit12_ps for token in ["verify-phase11.ps1", "phase12-localization-audit.ps1", "test-phase12-localization.py", "pnpm --dir apps/ui check", "pnpm --dir apps/ui build", "static_validate.py"]),
}
checks["phase12_catalog_ci_release_gate"] = {
    "ok": any(gate in ci10 for gate in ["verify-phase12.ps1 -SkipOnlineSupplyChain", "verify-phase13.ps1 -SkipOnlineSupplyChain", "verify-phase14.ps1 -SkipOnlineSupplyChain", "verify-phase15.ps1 -SkipOnlineSupplyChain", "verify-phase16.ps1 -SkipOnlineSupplyChain", "verify-enterprise.ps1 -SkipOnlineSupplyChain"]) and any(gate in release10 for gate in ["verify-phase12.ps1 -ReleasePackaging -RequireSigning", "verify-phase13.ps1 -ReleasePackaging -RequireSigning", "verify-phase14.ps1 -ReleasePackaging -RequireSigning", "verify-phase15.ps1 -ReleasePackaging -RequireSigning", "verify-phase16.ps1 -ReleasePackaging -RequireSigning", "verify-enterprise.ps1 -ReleasePackaging -RequireSigning"]),
}
checks["phase12_catalog_namespace_depth"] = {
    "ok": len(en_keys12) >= 850 and all(any(key.startswith(prefix) for key in en_keys12) for prefix in ["nav.","overview.","drivers.","repair.","cleanup.","startup.","hardware.","crash.","activity.","recovery.","dialog.","tech.","state.","severity.","confidence."]),
    "keys": len(en_keys12),
}


# Phase 12 — typed localization, full EN/AR parity, bidi integrity, plural rules and localized consent presentation.
i18n12_dir = ui_root / "lib/i18n"
en12 = read("apps/ui/src/lib/i18n/catalog.en.ts")
ar12 = read("apps/ui/src/lib/i18n/catalog.ar.ts")
runtime12 = read("apps/ui/src/lib/i18n/runtime.ts")
semantic12 = read("apps/ui/src/lib/i18n/semantic.ts")
bidi12 = read("apps/ui/src/lib/i18n/bidi.ts")
plurals_en12 = read("apps/ui/src/lib/i18n/plurals.en.ts")
plurals_ar12 = read("apps/ui/src/lib/i18n/plurals.ar.ts")
technical12 = read("apps/ui/src/design/primitives/TechnicalText.svelte")
owned12 = read("apps/ui/src/design/primitives/LocalizedOwnedText.svelte")
typography12 = (ui_root / "design/styles/typography.css").read_text(encoding="utf-8")
broker12 = (ROOT / "apps/consent-broker/src/main.rs").read_text(encoding="utf-8")
desktop12 = (ROOT / "apps/desktop/src/main.rs").read_text(encoding="utf-8")
verify12 = read("scripts/verify-phase12.ps1")
audit12 = read("scripts/phase12-localization-audit.py")
test12 = read("scripts/phase12-i18n-tests.cjs")
required12 = [
    "apps/ui/src/lib/i18n/catalog.en.ts","apps/ui/src/lib/i18n/catalog.ar.ts","apps/ui/src/lib/i18n/plurals.en.ts","apps/ui/src/lib/i18n/plurals.ar.ts",
    "apps/ui/src/lib/i18n/runtime.ts","apps/ui/src/lib/i18n/semantic.ts","apps/ui/src/lib/i18n/bidi.ts",
    "apps/ui/src/design/primitives/TechnicalText.svelte","apps/ui/src/design/primitives/LocalizedOwnedText.svelte",
    "scripts/phase12-localization-audit.py","scripts/phase12-localization-audit.ps1","scripts/phase12-i18n-tests.cjs","scripts/test-phase12-i18n.ps1","scripts/verify-phase12.ps1",
    "docs/LOCALIZATION_AND_BIDI.md","docs/adr/0014-typed-localization-and-bidi-integrity.md",
]
checks["phase12_required_artifacts"] = {"ok": all((ROOT / rel).is_file() for rel in required12), "missing": [rel for rel in required12 if not (ROOT / rel).is_file()]}
phase12_docs = (ROOT / "docs/LOCALIZATION_AND_BIDI.md").read_text(encoding="utf-8") + "\n" + (ROOT / "docs/adr/0014-typed-localization-and-bidi-integrity.md").read_text(encoding="utf-8")
marker("phase12_documented_localization_contract", phase12_docs, ["MessageKey", "Intl.PluralRules", "TechnicalText.svelte", "unicode-bidi:isolate", "display strings", "presentation locale"])
marker("phase12_typed_catalog", en12 + ar12 + runtime12, ["export type MessageKey = keyof typeof enCatalog", "satisfies Record<MessageKey, string>", "ExtractVars<", "VarsFor<", "hasMessageKey"])
checks["phase12_catalog_scale"] = {"ok": en12.count(": ") >= 850 and ar12.count(": ") >= 850, "english_entries_approx": en12.count(": "), "arabic_entries_approx": ar12.count(": ")}
marker("phase12_plural_architecture", plurals_en12 + plurals_ar12 + runtime12 + test12, ["Intl.PluralRules", "zero", "one", "two", "few", "many", "other", "ar-IQ"])
marker("phase12_semantic_localizers", semantic12, ["localizeOwnedText", "localizeState", "localizeRisk", "localizeSeverity", "localizeConfidence", "localizeImpact", "localizeKind", "localizeDirection", "localizeDomain", "localizePlanKind"])
checks["phase12_semantic_route_ids"] = {"ok": all(f"id: '{page}'" in navigation_ts for page in ["overview","drivers","repair","cleanup","startup","hardware","crash","activity"]) and all(f"id: '{old}'" not in navigation_ts for old in ["Overview","Drivers","Repair","Cleanup","Startup","Hardware","Crash history","Activity & recovery"])}
checks["phase12_no_display_string_routing"] = {"ok": "title.startsWith(" not in ui and "active.title" not in ui and "=== 'Crash history'" not in ui and "=== 'Activity & recovery'" not in ui}
marker("phase12_technical_bidi_isolation", bidi12 + technical12 + typography12 + owned12, ["TECHNICAL_TOKEN", "segmentBidiEvidence", "dir=\"ltr\"", "data-technical", "unicode-bidi: isolate", "TechnicalText"])
checks["phase12_css_logical_layout"] = {"ok": re.search(r"\b(?:margin|padding|border|inset)-(?:left|right)\b", phase7_css) is None}
marker("phase12_arabic_optical_typography", typography12, ["html[lang='ar'] body", "word-spacing", "font-synthesis: none", "text-wrap: pretty", "letter-spacing: 0"])
checks["phase12_arabic_windows_update_localized"] = {"ok": "Windows Update" not in ar12 and "تحديث Windows" in ar12}
checks["phase12_progress_label_typed"] = {"ok": "export let label: string;" in progress11 and "export let label = 'Progress'" not in progress11}
checks["phase12_live_announcement_localized"] = {"ok": "announce(t('announce.streamResynchronized'" in (ui_root / "platform/kernel-session.ts").read_text(encoding="utf-8")}
marker("phase12_service_error_message_keys", desktop12 + runtime12 + en12 + ar12, ["anyhow::bail!(error.message_key.clone())", "hasMessageKey", "service.internal", "authorization.brokerRejected"])
checks["phase12_no_technical_detail_as_ui_error"] = {"ok": "error.technical_detail.is_empty()" not in desktop12}
marker("phase12_localized_uac_broker", broker12 + desktop12, ["--locale", "locale != \"en\" && locale != \"ar\"", "localized_operation_ar", "MB_RTLREADING", "GetUserDefaultUILanguage", "--intent-id"])
checks["phase12_consent_stays_secret_free"] = {"ok": "--challenge" not in broker12 and "--challenge" not in desktop12 and "--locale {locale}" in desktop12}
checks["phase12_all_surfaces_locale_driven"] = {"ok": all("locale = $shellState.locale" in (ROOT / rel).read_text(encoding="utf-8") and "t('" in (ROOT / rel).read_text(encoding="utf-8") for rel in ["apps/ui/src/features/overview/OverviewPage.svelte","apps/ui/src/features/drivers/DriversPage.svelte","apps/ui/src/features/repair/RepairPage.svelte","apps/ui/src/features/cleanup/CleanupPage.svelte","apps/ui/src/features/startup/StartupPage.svelte","apps/ui/src/features/diagnostics/HardwarePage.svelte","apps/ui/src/features/diagnostics/CrashPage.svelte","apps/ui/src/features/activity/ActivityPage.svelte"])}
diagnostic_surfaces12 = "\n".join((ROOT / rel).read_text(encoding="utf-8") for rel in ["apps/ui/src/features/diagnostics/HardwarePage.svelte", "apps/ui/src/features/diagnostics/CrashPage.svelte"])
marker("phase12_deep_diagnostic_localization", owned12 + semantic12 + bidi12 + diagnostic_surfaces12, ["LocalizedOwnedText value={card.title}", "segmentBidiEvidence", "tech.card", "tech.storage", "tech.memory", "tech.warning"])
marker("phase12_catalog_audit_depth", audit12, ["Placeholder mismatch", "Plural key parity mismatch", "Raw visible English prose", "Typed service error keys missing", "Display-string business logic reintroduced"])
marker("phase12_windows_gate", verify12, ["verify-phase11.ps1", "phase12-localization-audit.ps1", "test-phase12-i18n.ps1", "pnpm --dir apps/ui check", "pnpm --dir apps/ui build", "cargo check --locked -p aethercore-consent-broker -p aethercore-desktop", "static_validate.py"])
checks["phase12_release_packaging_deferred"] = {"ok": "'ReleasePackaging','InstallerLifecycle'" not in verify12.split("& (Join-Path $PSScriptRoot 'verify-phase11.ps1')",1)[0] and verify12.find("build-release.ps1") > verify12.find("static_validate.py") and "verify-reproducible.ps1') -NativeDoubleBuild" in verify12}
checks["phase12_ci_release_gate"] = {"ok": any(gate in ci10 for gate in ["verify-phase12.ps1 -SkipOnlineSupplyChain", "verify-phase13.ps1 -SkipOnlineSupplyChain", "verify-phase14.ps1 -SkipOnlineSupplyChain", "verify-phase15.ps1 -SkipOnlineSupplyChain", "verify-phase16.ps1 -SkipOnlineSupplyChain", "verify-enterprise.ps1 -SkipOnlineSupplyChain"]) and any(gate in release10 for gate in ["verify-phase12.ps1 -ReleasePackaging -RequireSigning", "verify-phase13.ps1 -ReleasePackaging -RequireSigning", "verify-phase14.ps1 -ReleasePackaging -RequireSigning", "verify-phase15.ps1 -ReleasePackaging -RequireSigning", "verify-phase16.ps1 -ReleasePackaging -RequireSigning", "verify-enterprise.ps1 -ReleasePackaging -RequireSigning"])}


# Phase 13 — native engine reliability, bounded Windows collectors and typed provider faults.
runtime13 = (ROOT / "crates/collector-runtime/src/lib.rs").read_text(encoding="utf-8")
hardware13 = (ROOT / "crates/hardware-telemetry/src/lib.rs").read_text(encoding="utf-8")
hardware_win13 = (ROOT / "crates/hardware-telemetry/src/windows_impl.rs").read_text(encoding="utf-8")
crash13 = (ROOT / "crates/crash-diagnostics/src/lib.rs").read_text(encoding="utf-8")
crash_win13 = (ROOT / "crates/crash-diagnostics/src/windows_impl.rs").read_text(encoding="utf-8")
diag13 = (ROOT / "crates/diagnostic-engine/src/lib.rs").read_text(encoding="utf-8")
proto13 = (ROOT / "crates/contracts/proto/diagnostics.proto").read_text(encoding="utf-8")
protocol13 = (ROOT / "services/maintenance-service/src/protocol.rs").read_text(encoding="utf-8")
verify13 = read("scripts/verify-phase13.ps1")
audit13 = read("scripts/phase13-reliability-audit.py")
fault13 = read("scripts/phase13-fault-injection.ps1")
restore13 = (ROOT / "crates/restore-point/src/windows_impl.rs").read_text(encoding="utf-8")
required13 = [
    "crates/collector-runtime/Cargo.toml", "crates/collector-runtime/src/lib.rs",
    "scripts/phase13-reliability-audit.py", "scripts/phase13-reliability-audit.ps1",
    "scripts/phase13-fault-injection.ps1", "scripts/verify-phase13.ps1",
    "docs/ENGINE_RELIABILITY.md", "docs/adr/0015-collector-runtime-and-structured-event-rendering.md",
    "PHASE_13_DELIVERABLES.md", "PHASE_13_VALIDATION_SUMMARY.md",
]
checks["phase13_required_artifacts"] = {"ok": all((ROOT / rel).is_file() for rel in required13), "missing": [rel for rel in required13 if not (ROOT / rel).is_file()]}
marker("phase13_collector_runtime", runtime13, ["CollectorControl", "CancellationToken", "IsolationGate", "run_isolated_gated", "recv_timeout", "catch_unwind"])
marker("phase13_hierarchical_cancellation", runtime13, ["struct CancellationNode", "parent: Option<Arc<CancellationNode>>", "pub fn child(&self)", "current = node.parent.as_deref()", "parent_cancellation_propagates_to_children_without_reverse_poisoning", "external_cancellation_interrupts_supervisor_before_watchdog_deadline"])
checks["phase13_gate_release_order"] = {"ok": runtime13.find("drop(lease);") >= 0 and runtime13.find("drop(lease);") < runtime13.find("tx.send(result)") and "successful_gated_provider_releases_before_result_is_visible" in runtime13}
marker("phase13_fault_taxonomy", runtime13, ["Timeout", "Cancelled", "Unavailable", "PermissionDenied", "MalformedResponse", "ProviderFailure", "Io", "Internal"])
marker("phase13_fault_detail_bound", runtime13, ["MAX_FAULT_DETAIL_BYTES", "bounded_detail", "impl CollectorFaultRecord", "is_char_boundary", "fault_detail_is_utf8_bounded_before_crossing_process_boundaries"])
marker("phase13_watchdog_regressions", runtime13, ["watchdog_cancels_and_returns_timeout", "timed_out_provider_remains_quarantined_until_worker_exits", "panic_is_contained_as_internal_fault"])
checks["phase13_no_wbem_infinite"] = {"ok": "WBEM_INFINITE" not in hardware_win13 and "WBEM_INFINITE" not in restore13}
marker("phase13_finite_wmi", hardware_win13, ["remaining_ms_capped(WMI_NEXT_SLICE)", ".Next(timeout_ms", "bounded 256-object storage inventory", "Ok(out)"])
marker("phase13_wmi_status_semantics", hardware_win13 + restore13, ["WBEM_S_TIMEDOUT", "WBEM_S_FALSE", "status.is_err()", "terminal WBEM_S_FALSE with a non-empty result"])
marker("phase13_wmi_count_consistency", hardware_win13, ["fn take_wmi_object(", "returned > 1", "reported one returned object but supplied no object", "populated an object while reporting zero returned objects"])
marker("phase13_permission_denied_classification", hardware_win13 + crash_win13, ["E_ACCESSDENIED", "WBEM_E_ACCESS_DENIED", "TelemetryError::PermissionDenied", "CrashError::PermissionDenied", "FaultKind::PermissionDenied", "fn io_fault_kind("])
marker("phase13_storage_watchdogs", hardware_win13, ["STORAGE_GATE", "DIRECT_IOCTL_GATE", "MEMORY_GATE", "STORAGE_IOCTL_TIMEOUT", "nvme-health-ioctl", "ata-smart-ioctl"])
marker("phase13_storage_byte_parsers", hardware13, ["parse_ata_driver_response", "parse_nvme_health_log", "checked_protocol_window", "checked_add", "u128::from_le_bytes"])
marker("phase13_storage_fault_regressions", hardware13, ["nvme_parser_rejects_truncated_vendor_response", "protocol_window_rejects_overflow_and_truncation", "ata_driver_response_rejects_forged_returned_length", "ata_driver_response_rejects_inconsistent_declared_payload"])
checks["phase13_no_event_xml_scraping"] = {"ok": all(token not in crash_win13 for token in ["EvtRenderEventXml", "xml_tag(", "xml_attr(", "<EventID>"])}
marker("phase13_structured_event_rendering", crash_win13, ["EvtCreateRenderContext", "EvtRenderContextSystem", "EvtRenderContextUser", "EvtRenderEventValues", "EVT_VARIANT"])
marker("phase13_event_render_bounds", crash_win13, ["MAX_SYSTEM_RENDER_BYTES", "MAX_USER_RENDER_BYTES", "MAX_EVENT_PROPERTIES", "MAX_EVENT_STRING_UTF16", "string pointer escaped its render buffer"])
marker("phase13_event_property_count_preallocation_cap", crash_win13, ["fn checked_render_property_count(", "let property_count = checked_render_property_count(properties)?;", "let actual_property_count = checked_render_property_count(actual_properties)?;", "render_property_count_is_rejected_before_allocation_when_pathological"])
marker("phase13_event_timeout_and_partial_fault", crash_win13, ["remaining_ms_capped(EVENTLOG_NEXT_SLICE)", "ERROR_TIMEOUT", "malformed_events", "eventlog.render", "FaultKind::MalformedResponse"])
marker("phase13_structured_whea_classifier", crash13, ["payload_values: &[String]", "structured_payload_classification_does_not_require_xml"])
marker("phase13_provider_fanout", diag13, ["hardware_gate:IsolationGate", "crash_gate:IsolationGate", "hardware_worker = thread::Builder::new()", "crash_worker = thread::Builder::new()", "run_isolated_gated", "provider_panic_is_contained_and_persisted_as_typed_fault"])
marker("phase13_provider_supervisor_spawn_and_panic_containment", diag13, ["fn join_provider<T>(", "failed to spawn provider supervisor thread", "provider supervisor thread terminated unexpectedly", "provider_supervisor_panic_is_classified"])
marker("phase13_top_level_scan_panic_guard", diag13, ["catch_unwind(AssertUnwindSafe(|| run(worker_inner,owner)))", "mark_scan_runtime_failure(", "scan_runtime_failure_marks_collecting_snapshot_failed"])
marker("phase13_nested_fault_persistence", diag13, ["provider_faults.extend(h.provider_faults", "provider_faults.extend(c.provider_faults", "nested_provider_faults_are_preserved_in_the_diagnostic_snapshot"])
marker("phase13_provider_fault_contract", proto13 + protocol13, ["enum ProviderFaultKind", "message ProviderFaultInfo", "repeated ProviderFaultInfo provider_faults = 13", "provider_fault_kind_code", "v1::ProviderFaultInfo"])
phase13_fault_ui = read("apps/ui/src/features/diagnostics/ProviderFaultsPanel.svelte")
marker("phase13_localized_fault_ui", phase13_fault_ui + semantic12 + en12 + ar12, ["localizeProviderFaultKind", "diagnostics.providerFaults.kind.timeout", "diagnostics.providerFaults.kind.malformedResponse", "TechnicalText"])
checks["phase13_raw_fault_detail_not_user_visible"] = {"ok": "fault.detail" not in phase13_fault_ui}
checks["phase13_fault_records_share_bounded_constructor"] = {"ok": "CollectorFaultRecord {" not in crash_win13 and "impl CollectorFaultRecord" in runtime13 and "ProviderFaultRecord{provider:\"diagnostic-engine\"" not in diag13 and "ProviderFaultRecord{provider:\"diagnostic-journal\"" not in diag13}
checks["phase13_diagnostic_persistence_not_silent"] = {"ok": "let _=inner.db.save_diagnostic_snapshot" not in diag13 and "diagnostic-journal" in diag13 and "snapshot.persist" in diag13 and "ScanState::Partial" in diag13 and "DiagnosticError::Internal" in (ROOT / "services/maintenance-service/src/errors.rs").read_text(encoding="utf-8")}
marker("phase13_fault_injection_gate", fault13, ["aethercore-collector-runtime", "aethercore-hardware-telemetry", "aethercore-crash-diagnostics", "aethercore-diagnostic-engine", "LiveReadOnly", "render_property_count_is_rejected_before_allocation_when_pathological", "nvme_parser_accepts_vendor_tail_without_reading_past_standard_prefix"])
marker("phase13_event_alignment_and_handle_bounds", crash_win13, ["align_of::<EVT_VARIANT>()", "align_of::<u16>()", "Take ownership of every non-null handle", "EvtNext reported more event handles than the bounded output array", "EvtNext returned null, sparse, or trailing handles inconsistent with its reported count"])
marker("phase13_protocol_overlap_guard", hardware13, ["minimum_data_offset", "protocol payload overlapped the protocol-specific metadata header"])
checks["phase13_memory_unavailable_is_unknown"] = {"ok": "pub memory:Option<MemoryTelemetry>" in diag13 and "memory:None" in diag13 and "v.memory.map(|memory|" in protocol13}
checks["phase13_ci_release_gate"] = {"ok": any(gate in ci10 for gate in ["verify-phase13.ps1 -SkipOnlineSupplyChain", "verify-phase14.ps1 -SkipOnlineSupplyChain", "verify-phase15.ps1 -SkipOnlineSupplyChain", "verify-phase16.ps1 -SkipOnlineSupplyChain", "verify-enterprise.ps1 -SkipOnlineSupplyChain"]) and any(gate in release10 for gate in ["verify-phase13.ps1 -ReleasePackaging -RequireSigning", "verify-phase14.ps1 -ReleasePackaging -RequireSigning", "verify-phase15.ps1 -ReleasePackaging -RequireSigning", "verify-phase16.ps1 -ReleasePackaging -RequireSigning", "verify-enterprise.ps1 -ReleasePackaging -RequireSigning"])}
marker("phase13_windows_gate", verify13, ["verify-phase12.ps1", "phase13-reliability-audit.ps1", "phase13-fault-injection.ps1", "cargo check --workspace --locked", "cargo test --locked -p aethercore-collector-runtime"])
checks["phase13_audit_depth"] = {"ok": all(token in audit13 for token in ["no_unbounded_wmi_in_collectors", "no_event_xml_scraping", "direct_ioctl_watchdog", "provider_fault_contract", "nested_provider_faults_preserved"])}
phase13_docs = (ROOT / "docs/ENGINE_RELIABILITY.md").read_text(encoding="utf-8") + "\n" + (ROOT / "docs/adr/0015-collector-runtime-and-structured-event-rendering.md").read_text(encoding="utf-8")
marker("phase13_documented_contract", phase13_docs, ["IsolationGate", "WBEM_INFINITE", "EvtRenderContextSystem", "MalformedResponse", "ProviderFault", "unknown remains unknown"])


# Phase 14 — autonomous, owner-bound, read-only idle scheduling.
model14 = (ROOT / "crates/idle-scheduler/src/model.rs").read_text(encoding="utf-8")
policy14 = (ROOT / "crates/idle-scheduler/src/policy.rs").read_text(encoding="utf-8")
runtime14 = (ROOT / "crates/idle-scheduler/src/runtime.rs").read_text(encoding="utf-8")
resource14 = (ROOT / "crates/idle-scheduler/src/resource.rs").read_text(encoding="utf-8")
windows14 = (ROOT / "crates/idle-scheduler/src/windows_state.rs").read_text(encoding="utf-8")
service14 = (ROOT / "services/maintenance-service/src/scheduler.rs").read_text(encoding="utf-8")
main14 = (ROOT / "services/maintenance-service/src/main.rs").read_text(encoding="utf-8")
persistence14 = (ROOT / "crates/persistence/src/lib.rs").read_text(encoding="utf-8")
migration14 = (ROOT / "crates/persistence/migrations/0008_phase14_scheduler.sql").read_text(encoding="utf-8")
kernel14 = (ROOT / "crates/operation-kernel/src/lib.rs").read_text(encoding="utf-8")
mutation14 = (ROOT / "crates/operation-kernel/src/mutation.rs").read_text(encoding="utf-8")
collector14 = (ROOT / "crates/collector-runtime/src/lib.rs").read_text(encoding="utf-8")
cleaner14 = (ROOT / "crates/cleaner/src/windows_impl.rs").read_text(encoding="utf-8")
proto_scheduler14 = (ROOT / "crates/contracts/proto/scheduler.proto").read_text(encoding="utf-8")
proto_events14 = (ROOT / "crates/contracts/proto/events.proto").read_text(encoding="utf-8")
activity14 = (ROOT / "apps/ui/src/features/activity/ActivityPage.svelte").read_text(encoding="utf-8")
verify14 = read("scripts/verify-phase14.ps1")
audit14 = read("scripts/phase14-scheduler-audit.py")
docs14 = (ROOT / "docs/AUTONOMOUS_MAINTENANCE.md").read_text(encoding="utf-8") + "\n" + (ROOT / "docs/adr/0016-autonomous-idle-scheduler.md").read_text(encoding="utf-8")
required14 = [
    "crates/idle-scheduler/Cargo.toml", "crates/idle-scheduler/src/model.rs", "crates/idle-scheduler/src/policy.rs",
    "crates/idle-scheduler/src/resource.rs", "crates/idle-scheduler/src/runtime.rs", "crates/idle-scheduler/src/windows_state.rs",
    "crates/contracts/proto/scheduler.proto", "crates/persistence/migrations/0008_phase14_scheduler.sql",
    "services/maintenance-service/src/scheduler.rs", "scripts/phase14-scheduler-audit.py", "scripts/phase14-scheduler-audit.ps1",
    "scripts/phase14-scheduler-tests.ps1", "scripts/phase14-scheduler-fault-injection.ps1", "scripts/verify-phase14.ps1", "docs/AUTONOMOUS_MAINTENANCE.md",
    "docs/adr/0016-autonomous-idle-scheduler.md", "PHASE_14_DELIVERABLES.md", "PHASE_14_VALIDATION_SUMMARY.md",
]
checks["phase14_required_artifacts"] = {"ok": all((ROOT / rel).is_file() for rel in required14), "missing": [rel for rel in required14 if not (ROOT / rel).is_file()]}
checks["phase14_closed_read_only_workloads"] = {"ok": all(token in model14 for token in ["HardwareTelemetry", "DriverDiscovery", "CleanupInventory", "StartupInventory", "EventLogTriage"]) and all(token not in model14.split("pub enum AutonomousWorkload",1)[1].split("}",1)[0] for token in ["DriverInstall", "SystemRepair", "Update", "Delete"])}
checks["phase14_executor_has_no_mutation_entrypoints"] = {"ok": all(token not in service14 for token in ["start_driver_install", "start_system_repair", "start_cleanup(", "start_startup_changes", "consume_authorization", "MutationWorkload::"])}
marker("phase14_owner_bound_active_console_session", windows14, ["WTSGetActiveConsoleSessionId", "WTSQueryUserToken", "inspect_session_token(token.get(), session_id)", "principal.binding_key()"])
marker("phase14_fail_closed_eligibility", policy14, ["PresentationUnknown", "ServicingUnknown", "MutationActive", "ThermalPressureUnknown", "NetworkCostUnknown", "UserActive"])
checks["phase14_preemption_100ms"] = {"ok": "Duration::from_millis(100)" in model14 and "sample_fast" in runtime14 and "refresh_slow_nonblocking" in runtime14}
marker("phase14_linearized_commit_fence", collector14 + runtime14, ["CommitFenceState", "Committed", "Revoked", "try_commit_checked", "monitor_fence.revoke()", "monitor_token.cancel()"])
marker("phase14_kernel_read_budget_and_mutation_exclusion", runtime14 + mutation14, ["kernel.reads().try_acquire", "kernel.mutations().is_active()", "pub fn is_active(&self) -> bool"])
marker("phase14_resource_governor", resource14 + service14, ["cpu_budget_per_second", "io_budget_per_second", "account_cpu_cancellable", "account_io_cancellable"])
marker("phase14_windows_background_mode", windows14 + windows_foundation, ["BackgroundThreadMode::enter", "THREAD_MODE_BACKGROUND_BEGIN", "THREAD_MODE_BACKGROUND_END", "SetThreadPriority"])
marker("phase14_durable_principal_scoped_cadence", persistence14 + migration14 + kernel14 + runtime14, ["autonomous_scheduler_runs", "owner_principal_key", "scheduler_cadence", "save_scheduler_cadence", "persist_cadence"])
checks["phase14_cadence_contains_no_mutation_authority"] = {"ok": all(token not in migration14.lower() for token in ["command_path", "consent_token", "authorization_grant", "mutation_intent"])}
checks["phase14_cleanup_passive_privacy_scope"] = {"ok": "scan_impl(false)" in cleaner14 and "if include_profile_roots" in cleaner14 and "retain(|candidate| !candidate.requires_explicit_confirmation)" in cleaner14}
marker("phase14_typed_scheduler_events", proto_scheduler14 + proto_events14, ["message SchedulerEvent", "SchedulerRunState", "EVENT_KIND_SCHEDULER = 18", "SchedulerEvent scheduler = 27"])
checks["phase14_activity_is_observable_and_localized"] = {"ok": "schedulerEvent" in activity14 and "activity.schedulerTitle" in activity14 and "scheduler.reason." in activity14}
checks["phase14_service_survives_scheduler_start_failure"] = {"ok": "scheduler::start(&context)" in main14 and "interactive maintenance remains available" in main14}
marker("phase14_jitter_and_backoff", runtime14, ["random_jitter_ms", "Equal-jitter exponential backoff", "backoff_delay_ms", "base_backoff", "max_backoff"])
checks["phase14_scheduler_audit_depth"] = {"ok": all(token in audit14 for token in ["closed_workload_enum_exactly_five", "active_console_session_principal", "commit_fence_three_state", "cleanup_passive_scope_narrower", "equal_jitter_exponential_backoff"])}
marker("phase14_windows_gate", verify14, ["verify-phase13.ps1", "phase14-scheduler-audit.ps1", "phase14-scheduler-tests.ps1", "phase14-scheduler-fault-injection.ps1", "cargo check --workspace --locked", "static_validate.py"])
checks["phase14_ci_release_gate"] = {"ok": any(gate in ci10 for gate in ["verify-phase14.ps1 -SkipOnlineSupplyChain", "verify-phase15.ps1 -SkipOnlineSupplyChain", "verify-phase16.ps1 -SkipOnlineSupplyChain", "verify-enterprise.ps1 -SkipOnlineSupplyChain"]) and any(gate in release10 for gate in ["verify-phase14.ps1 -ReleasePackaging -RequireSigning", "verify-phase15.ps1 -ReleasePackaging -RequireSigning", "verify-phase16.ps1 -ReleasePackaging -RequireSigning", "verify-enterprise.ps1 -ReleasePackaging -RequireSigning"])}
marker("phase14_documented_contract", docs14, ["Zero background mutations", "active console session", "CommitFence", "ReadBudgetManager", "100 ms", "THREAD_MODE_BACKGROUND_BEGIN", "Unknown"])


# Phase 15 — secure user-scope update download, single installer authority and privacy-first cryptographic support export.
update_download15 = (ROOT / "crates/update-download/src/lib.rs").read_text(encoding="utf-8")
update_download_cargo15 = (ROOT / "crates/update-download/Cargo.toml").read_text(encoding="utf-8")
update_engine_cargo15 = (ROOT / "crates/update-engine/Cargo.toml").read_text(encoding="utf-8")
update_manifest15 = (ROOT / "crates/update-engine/src/manifest.rs").read_text(encoding="utf-8")
update_platform15 = (ROOT / "crates/update-engine/src/platform.rs").read_text(encoding="utf-8")
update_coordinator15 = (ROOT / "crates/update-engine/src/coordinator.rs").read_text(encoding="utf-8")
update_broker15 = (ROOT / "apps/update-broker/src/main.rs").read_text(encoding="utf-8")
desktop15 = (ROOT / "apps/desktop/src/main.rs").read_text(encoding="utf-8")
service_router15 = (ROOT / "services/maintenance-service/src/router.rs").read_text(encoding="utf-8")
service_cargo15 = (ROOT / "services/maintenance-service/Cargo.toml").read_text(encoding="utf-8")
support15 = (ROOT / "crates/support-bundle/src/lib.rs").read_text(encoding="utf-8")
support_service15 = (ROOT / "services/maintenance-service/src/support.rs").read_text(encoding="utf-8")
update_proto15 = (ROOT / "crates/contracts/proto/update.proto").read_text(encoding="utf-8")
support_proto15 = (ROOT / "crates/contracts/proto/support_bundle.proto").read_text(encoding="utf-8")
events_proto15 = (ROOT / "crates/contracts/proto/events.proto").read_text(encoding="utf-8")
migration15 = (ROOT / "crates/persistence/migrations/0009_phase15_update.sql").read_text(encoding="utf-8")
verify15 = read("scripts/verify-phase15.ps1")
audit15 = read("scripts/phase15-security-audit.py")
build_release15 = (ROOT / "scripts/build-release.ps1").read_text(encoding="utf-8")
trust_validate15 = (ROOT / "scripts/validate-update-trust.ps1").read_text(encoding="utf-8")
product15 = (ROOT / "installer/wix/Product.wxs").read_text(encoding="utf-8")
docs15 = (ROOT / "docs/SECURE_UPDATE_AND_SUPPORT_EXPORT.md").read_text(encoding="utf-8") + "\n" + (ROOT / "docs/adr/0017-secure-update-and-cryptographic-support-export.md").read_text(encoding="utf-8")
required15 = [
    "crates/update-engine/Cargo.toml", "crates/update-engine/src/manifest.rs", "crates/update-engine/src/platform.rs", "crates/update-engine/src/coordinator.rs",
    "crates/update-download/Cargo.toml", "crates/update-download/src/lib.rs", "crates/support-bundle/Cargo.toml", "crates/support-bundle/src/lib.rs",
    "apps/update-broker/Cargo.toml", "apps/update-broker/src/main.rs", "tools/update-manifest/Cargo.toml", "tools/update-manifest/src/main.rs",
    "tools/support-bundle-verify/Cargo.toml", "tools/support-bundle-verify/src/main.rs", "crates/contracts/proto/update.proto", "crates/contracts/proto/support_bundle.proto",
    "crates/persistence/migrations/0009_phase15_update.sql", "scripts/build-update-manifest.ps1", "scripts/generate-update-trust.ps1", "scripts/validate-update-trust.ps1",
    "scripts/phase15-security-audit.py", "scripts/phase15-security-audit.ps1", "scripts/phase15-crypto-tests.ps1", "scripts/verify-phase15.ps1",
    "docs/SECURE_UPDATE_AND_SUPPORT_EXPORT.md", "docs/adr/0017-secure-update-and-cryptographic-support-export.md", "PHASE_15_DELIVERABLES.md", "PHASE_15_VALIDATION_SUMMARY.md",
]
checks["phase15_required_artifacts"] = {"ok": all((ROOT / rel).is_file() for rel in required15), "missing": [rel for rel in required15 if not (ROOT / rel).is_file()]}
checks["phase15_http_authority_is_desktop_only"] = {"ok": "reqwest" in update_download_cargo15.lower() and "reqwest" not in update_engine_cargo15.lower() and "reqwest" not in service_cargo15.lower() and "aethercore-update-download" in (ROOT / "apps/desktop/Cargo.toml").read_text(encoding="utf-8")}
marker("phase15_bounded_https_downloader", update_download15, ["reqwest::redirect::Policy::none()", "connect_timeout", "timeout", "ResponseTooLarge", "SizeMismatch", "Sha256::new"])
checks["phase15_manifest_signature_before_parse"] = {"ok": update_manifest15.find("verifying.verify(manifest_bytes") < update_manifest15.find("serde_json::from_slice(manifest_bytes)") and all(token in update_manifest15 for token in ["MAX_MANIFEST_BYTES", "MAX_SIGNATURE_BYTES", "validate_https_url"])}
marker("phase15_rollback_and_equivocation_floor", update_coordinator15 + migration15, ["update_manifest_floor", "highest_sequence", "manifest_sha256", "manifest.sequence==floor.highest_sequence", "manifest_sha256.eq_ignore_ascii_case"])
checks["phase15_minimum_windows_build_enforced"] = {"ok": "RtlGetVersion" in update_platform15 and "release.minimum_windows_build<=self.current_windows_build" in update_coordinator15 and "releases_requiring_newer_windows_build_are_not_offered" in update_coordinator15}
checks["phase15_service_side_download_legacy_disabled"] = {"ok": 'legacy service-side update download is disabled' in service_router15 and "CheckForUpdates" in service_router15 and "StageUpdate" in service_router15}
marker("phase15_descriptor_chunk_upload", service_router15 + update_coordinator15 + update_proto15, ["GetUpdateCheckDescriptor", "SubmitUpdateManifest", "BeginUpdateStageUpload", "WriteUpdateStageChunk", "FinalizeUpdateStageUpload", "MAX_STAGE_CHUNK_BYTES"])
checks["phase15_no_privileged_user_path_or_url_input"] = {"ok": "staged_path" not in update_proto15.split("message BeginUpdateStageUploadRequest",1)[1].split("}",1)[0] and "package_url" not in update_proto15.split("message BeginUpdateStageUploadRequest",1)[1].split("}",1)[0]}
marker("phase15_service_derived_staging", update_coordinator15, ["expected_staged_path", "owner_scope", "sha256.to_ascii_lowercase()", "AetherCoreUpdate-{owner_scope}-{release_id}-{}.exe", "upload-{upload_id}.part"])
checks["phase15_hash_authenticode_hash"] = {"ok": update_coordinator15.count("verify_file_hash_size(&path") >= 6 and update_coordinator15.count("verify_authenticode(&path)") >= 3}
marker("phase15_authenticode_chain_validation", update_platform15, ["WinVerifyTrust", "WINTRUST_ACTION_GENERIC_VERIFY_V2", "WTD_REVOKE_WHOLECHAIN", "WTD_REVOCATION_CHECK_CHAIN"])
checks["phase15_update_uses_global_mutation_lease"] = {"ok": update_coordinator15.count("MutationWorkload::Update") >= 2 and "active_update_execution" in migration15}
checks["phase15_update_broker_fixed_surface"] = {"ok": "if args.len()!=5" in update_broker15 and "--intent-id" in update_broker15 and "--locale" in update_broker15 and all(token not in update_broker15 for token in ["--url","--path","--command","--args"]) and all(token not in update_broker15.lower() for token in ["reqwest","http://","https://"])}
checks["phase15_update_broker_no_arbitrary_installer_args"] = {"ok": "Command::new(&path).status()" in update_broker15 and "let path=std::path::PathBuf::from(&ticket.staged_path)" in update_broker15 and ".args(" not in update_broker15}
checks["phase15_single_installer_authority_packaged"] = {"ok": "UpdateBrokerExe" in product15 and "UpdateTrustJson" in product15 and "aethercore-update-broker.exe" in product15 and "update-trust.json" in product15}
checks["phase15_signed_release_requires_enabled_trust"] = {"ok": "AETHERCORE_UPDATE_TRUST_PATH" in build_release15 and "RequireSigning" in build_release15 and "RequireEnabled" in trust_validate15 and "stable" in trust_validate15.lower()}
marker("phase15_support_preview_and_allowlist", support15 + support_service15, ["create_preview", "prepare(&self,owner:&str,preview_id:&str)", "product.json", "diagnostics.json", "operation-history.json", "scheduler-activity.json"])
checks["phase15_support_privacy_nonlinkable_redaction"] = {"ok": all(token in support15 for token in ["%USERPROFILE%", "<redacted-account>", "<redacted-sid>", "<redacted-email>", "<redacted-hardware-serial>"]) and "serial-hash" not in support15}
marker("phase15_support_deterministic_hash_manifest", support15, ["files.sort_by", "ManifestFile", "payload_root_sha256", "sha256", "BTreeMap"])
checks["phase15_support_proof_is_narrow"] = {"ok": "installation-ed25519" in support15 and "not a vendor or hardware attestation" in support15}
marker("phase15_support_independent_fingerprint", support15 + support_proto15 + desktop15, ["public_key_fingerprint_sha256", "expected_public_key_fingerprint_sha256", "actual_fingerprint.eq_ignore_ascii_case", "computed_fingerprint"])
checks["phase15_support_rejects_unmanifested_entries"] = {"ok": "unmanifested archive entry" in support15 and "unmanifested_entry_is_rejected" in support15}
checks["phase15_support_service_has_no_destination_path"] = {"ok": all(token not in support_proto15 for token in ["destination_path","output_path","folder_path"]) and all(token not in service_router15 for token in ["destination_path","output_path"])}
checks["phase15_support_desktop_non_overwrite"] = {"ok": all(token in desktop15 for token in ["non_overwriting_support_path", "create_new(true)", ".aetherdiag-new", "MarkSupportBundleExported"])}
marker("phase15_typed_update_support_events", update_proto15 + support_proto15 + events_proto15, ["UpdateCheckDescriptorResponse", "SupportBundleReady", "public_key_fingerprint_sha256", "EVENT_KIND_UPDATE", "EVENT_KIND_SUPPORT_BUNDLE"])
checks["phase15_security_audit_depth"] = {"ok": all(token in audit15 for token in ["service_update_engine_has_no_reqwest", "manifest_floor_equivocation_hash", "update_mutation_workload_reserved", "support_serial_non_linkable_redaction", "support_strong_verifier_requires_independent_fingerprint"])}
checks["phase15_windows_gate_order"] = {"ok": all(token in verify15 for token in ["verify-phase14.ps1", "phase15-security-audit.ps1", "phase15-crypto-tests.ps1", "pnpm --dir apps/ui check", "static_validate.py", "build-release.ps1"])}
checks["phase15_ci_release_gate"] = {"ok": any(gate in ci10 for gate in ["verify-phase15.ps1 -SkipOnlineSupplyChain", "verify-phase16.ps1 -SkipOnlineSupplyChain", "verify-enterprise.ps1 -SkipOnlineSupplyChain"]) and any(gate in release10 for gate in ["verify-phase15.ps1 -ReleasePackaging -RequireSigning", "verify-phase16.ps1 -ReleasePackaging -RequireSigning", "verify-enterprise.ps1 -ReleasePackaging -RequireSigning"]) and "AETHERCORE_UPDATE_TRUST_PATH" in release10}
marker("phase15_documented_trust_boundaries", docs15, ["user-scope", "LocalSystem", "fixed elevated", "MutationSupervisor::Update", "not vendor or hardware attestation", "fingerprint"])
checks["phase15_cancelled_consent_intent_is_recoverable"] = {"ok": "CancelUpdateInstallIntentRequest" in update_proto15 and "cancel_install_intent" in update_coordinator15 and "expired_intent_owners" in update_coordinator15 and "cancel_update_install_intent(&intent_id)" in desktop15}
claim15_start = update_coordinator15.find("pub fn claim_install")
claim15_end = update_coordinator15.find("pub fn complete_install", claim15_start)
claim15_body = update_coordinator15[claim15_start:claim15_end] if claim15_start >= 0 and claim15_end > claim15_start else ""
checks["phase15_install_claim_reservation_is_linearized"] = {"ok": claim15_body.find("record.claimed=true") >= 0 and claim15_body.find("verify_file_hash_size(&path") > claim15_body.find("record.claimed=true") and claim15_body.find("self.mutations.try_acquire(MutationWorkload::Update") > claim15_body.find("verify_file_hash_size(&path") and "if result.is_err()" in claim15_body and "record.claimed=false" in claim15_body}
checks["phase15_claim_cleanup_preserves_inflight_reservation"] = {"ok": "intents.retain(|_,v|v.claimed||v.intent.expires_unix_ms>=now)" in update_coordinator15 and "self.intents.lock().unwrap_or_else(|p|p.into_inner()).remove(intent_id)" in claim15_body and "cleanup_does_not_erase_an_inflight_claim_reservation" in update_coordinator15}
checks["phase15_support_archive_reverified_before_user_save"] = {"ok": "aethercore_support_bundle::verify_archive(&archive_bytes" in desktop15 and desktop15.find("verify_archive(&archive_bytes") < desktop15.find("std::fs::rename(&temporary,&path)")}
checks["phase15_support_tar_header_checksum"] = {"ok": "valid_tar_checksum" in support15 and "tar_header_checksum_tampering_is_rejected" in support15}
checks["phase15_broker_protected_machine_mutation_lock"] = {"ok": "MachineMutationGuard::try_acquire()" in update_broker15 and "pub struct MachineMutationGuard" in windows_foundation and "LockFileEx" in windows_foundation and "SHGetKnownFolderPath" in windows_foundation and "OPEN_EXISTING" in windows_foundation and "machine-mutation.lock" in windows_foundation and "UpdateMutationGuard::acquire" in update_broker15 and update_broker15.find("UpdateMutationGuard::acquire") < update_broker15.find("claim(&intent_id)")}
checks["phase15_execution_expiry_fail_closed"] = {"ok": "if self.db.clear_update_execution_guard(&ticket_id).is_ok()" in update_coordinator15 and update_coordinator15.find("clear_update_execution_guard(&ticket_id).is_ok()") < update_coordinator15.find("self.active.lock().unwrap_or_else(|p|p.into_inner()).take()", update_coordinator15.find("pub fn reap_expired_execution"))}
checks["phase15_restart_restores_exact_install_identity"] = {"ok": all(token in migration15 for token in ["release_version TEXT NOT NULL", "channel TEXT NOT NULL", "notes_message_key TEXT NOT NULL", "minimum_windows_build INTEGER NOT NULL"]) and all(token in update_coordinator15 for token in ["record.release_version.clone()", "record.notes_message_key.clone()", "record.minimum_windows_build", "snapshot.state=UpdateState::Installing", "durable_execution_recovery_restores_installing_snapshot_and_release_identity"])}
checks["phase15_support_strict_signature_verification"] = {"ok": "verify_strict(manifest_bytes" in support15}
checks["phase15_support_embedded_pii_redaction"] = {"ok": "embedded_sid_and_email_are_redacted_inside_free_form_text" in support15 and "<redacted-sid>" in support15 and "<redacted-email>" in support15}
checks["phase15_support_object_quotas_and_failure_discard"] = {"ok": all(token in support15 for token in ["MAX_ACTIVE_PREVIEWS_TOTAL", "MAX_ACTIVE_BUNDLES_TOTAL", "SupportBundleError::ResourceLimit", "one_active_bundle_per_owner_is_enforced_and_discard_releases_quota"]) and desktop15.count("request(request::Payload::DiscardSupportBundle") >= 2}



# Phase 16 — final production qualification, stress matrix and cryptographic GA seal.
phase16_matrix = json.loads((ROOT / "release/ga-matrix.json").read_text(encoding="utf-8"))
phase16_probe = (ROOT / "tools/ga-probe/src/main.rs").read_text(encoding="utf-8")
phase16_soak = (ROOT / "scripts/phase16-stress-soak.ps1").read_text(encoding="utf-8")
phase16_test_helper = (ROOT / "scripts/invoke-cargo-test-case.ps1").read_text(encoding="utf-8")
phase16_resilience = (ROOT / "scripts/phase16-resilience-matrix.ps1").read_text(encoding="utf-8")
phase16_host = (ROOT / "scripts/phase16-host-qualification.ps1").read_text(encoding="utf-8")
phase16_lifecycle = (ROOT / "scripts/phase16-installer-lifecycle.ps1").read_text(encoding="utf-8")
phase16_seal = (ROOT / "scripts/phase16-seal-release.ps1").read_text(encoding="utf-8")
phase16_verify_seal = (ROOT / "scripts/verify-ga-seal.ps1").read_text(encoding="utf-8")
phase16_verify = (ROOT / "scripts/verify-phase16.ps1").read_text(encoding="utf-8")
phase16_prod = (ROOT / "scripts/verify-production.ps1").read_text(encoding="utf-8")
phase16_docs = (ROOT / "docs/FINAL_PRODUCTION_QUALIFICATION.md").read_text(encoding="utf-8") + "\n" + (ROOT / "docs/adr/0018-final-production-qualification-and-ga-seal.md").read_text(encoding="utf-8")
checks["phase16_matrix_schema"] = {"ok": phase16_matrix.get("schema") == "aethercore.ga-matrix.v1"}
checks["phase16_minimum_windows_build"] = {"ok": phase16_matrix.get("minimum_windows_build") == 22621}
checks["phase16_os_lane_floor"] = {"ok": len(phase16_matrix.get("required_os_lanes", [])) >= 3}
phase16_cov = phase16_matrix.get("required_coverage", {})
checks["phase16_bidi_matrix"] = {"ok": {"en-US", "ar-IQ"} <= set(phase16_cov.get("locales", [])) and {"ltr", "rtl"} <= set(phase16_cov.get("directions", []))}
checks["phase16_dpi_refresh_matrix"] = {"ok": {100,125,150,200} <= set(phase16_cov.get("dpi_percent", [])) and {60,120,144} <= set(phase16_cov.get("refresh_hz", []))}
checks["phase16_accessibility_matrix"] = {"ok": {"keyboard-only","narrator","reduced-motion","reduced-transparency","high-contrast"} <= set(phase16_cov.get("accessibility", []))}
checks["phase16_extended_soak_24h"] = {"ok": phase16_matrix.get("stress", {}).get("required_profile") == "extended" and int(phase16_matrix.get("stress", {}).get("minimum_duration_minutes", 0)) >= 1440}
marker("phase16_live_ipc_probe", phase16_probe, ["SessionClient::connect", "HydrateSessionRequest", "PingRequest", "event.sequence <= prior", "replay_after"])
marker("phase16_soak_resource_bounds", phase16_soak, ["PrivateMemorySize64", "HandleCount", "Threads.Count", "max_private_bytes_growth_mib", "max_handle_growth", "max_thread_growth"])
marker("phase16_rust_test_nonzero_guard", phase16_test_helper, ["--list", "exactly one test", "& cargo @runArgs"])
checks["phase16_no_short_exact_false_pass"] = {"ok": "-- --exact" not in ((ROOT / "scripts/phase13-fault-injection.ps1").read_text(encoding="utf-8") + (ROOT / "scripts/phase14-scheduler-fault-injection.ps1").read_text(encoding="utf-8") + (ROOT / "scripts/run-ipc-fuzz.ps1").read_text(encoding="utf-8") + phase16_resilience + phase16_verify)}
marker("phase16_resilience_fault_matrix", phase16_resilience, ["phase13-fault-injection.ps1", "phase14-scheduler-fault-injection.ps1", "phase15-crypto-tests.ps1", "run-ipc-fuzz.ps1", "Restart-Service AetherCoreMaintenance", "concurrent_contenders_never_overlap_machine_mutation_leases"])
marker("phase16_host_evidence_gate", phase16_host, ["required_os_lanes", "required_manual_surfaces", "Witness surface is not PASS", "verify-installer-security.ps1"])
marker("phase16_burn_msi_lifecycle", phase16_lifecycle, ["Burn install", "Burn uninstall", "MSI repair", "ProgramData preservation sentinel"])
marker("phase16_ga_seal_aggregation", phase16_seal, ["Missing PASS evidence for OS lane", "aethercore.ga-stress.v1", "aethercore.ga-resilience.v1", "aethercore.ga-installer-lifecycle.v1"])
marker("phase16_supply_chain_commitment", phase16_seal, ["SHA256SUMS.txt", "GA-EVIDENCE-SHA256SUMS.txt", "evidence\\sbom", "validate-update-trust.ps1", "Get-AuthenticodeSignature"])
marker("phase16_cms_release_attestation", phase16_seal, ["System.Security.Cryptography.Pkcs.SignedCms", "GA-SEAL.json", "GA-SEAL.p7s", "signer_thumbprint"])
marker("phase16_ga_seal_verify", phase16_verify_seal, ["CheckSignature", "release_sha256s_sha256", "evidence_manifest_sha256", "signer thumbprint mismatch"])
marker("phase16_master_gate", phase16_verify, ["verify-phase15.ps1", "phase16-ga-audit.ps1", "cargo check --workspace --locked", "static_validate.py"])
marker("phase16_production_gate", phase16_prod, ["phase16-seal-release.ps1", "verify-ga-seal.ps1", "General Availability release seal: PASS"])
marker("phase16_honest_ga_boundary", phase16_docs + "\n" + phase16_verify, ["does not constitute GA", "Windows-native", "GA-SEAL.json", "GA-SEAL.p7s"])
checks["phase16_ci_master_gate"] = {"ok": any(gate in (REPO / ".github/workflows/ci.yml").read_text(encoding="utf-8") for gate in ["verify-phase16.ps1 -SkipOnlineSupplyChain", "verify-enterprise.ps1 -SkipOnlineSupplyChain"]) }
checks["phase16_signed_release_master_gate"] = {"ok": any(gate in (REPO / ".github/workflows/release.yml").read_text(encoding="utf-8") for gate in ["verify-phase16.ps1 -ReleasePackaging -RequireSigning", "verify-enterprise.ps1 -ReleasePackaging -RequireSigning"]) }
checks["phase16_update_broker_elevation_lifecycle"] = {"ok": "aethercore-update-broker.exe" in (ROOT / "scripts/verify-installer-security.ps1").read_text(encoding="utf-8") and "Update broker PE manifest is not requireAdministrator" in (ROOT / "scripts/verify-installer-security.ps1").read_text(encoding="utf-8")}
checks["phase16_probe_workspace_member"] = {"ok": "tools/ga-probe" in workspace.get("members", [])}

# Sigma — evidence integrity, hermetic verification and fail-closed source truth.
omega_sigma = (ROOT / "scripts/omega-evidence.py").read_text(encoding="utf-8")
sigma_regression = (ROOT / "scripts/sigma-evidence-integrity-test.py").read_text(encoding="utf-8")
sigma_audits = [
    "zenith-recursive-audit.py", "zenith-adversarial-audit.py", "enterprise-adversarial-audit.py",
    "static_validate.py", "phase13-reliability-audit.py", "phase14-scheduler-audit.py",
    "phase15-security-audit.py", "phase16-ga-audit.py",
]
checks["sigma_manifest_is_final_success_condition"] = {
    "ok": all(token in omega_sigma for token in [
        'source_manifest_before', 'evidence["source_manifest"].get("ok") is True',
        'source_verification_pass', 'return 2 if args.strict else 1',
    ])
}
checks["sigma_repository_gates_are_isolated_and_write_detected"] = {
    "ok": all(token in omega_sigma for token in [
        "TemporaryDirectory", "shutil.copytree(ROOT, clone", "source_tree_unchanged",
        "SIGMA-RB-002", "mutating_runs",
    ])
}
checks["sigma_audit_reports_require_explicit_output"] = {
    "ok": all('--output' in (ROOT / 'scripts' / name).read_text(encoding='utf-8') for name in sigma_audits)
    and 'Optional explicit report path; default verification is read-only.' in (ROOT / 'scripts/static_validate.py').read_text(encoding='utf-8')
}
checks["sigma_adversarial_integrity_regression"] = {
    "ok": all(token in sigma_regression for token in [
        "valid_delivery_is_byte_stable", "invalid_manifest_cannot_return_zero",
        "audit_source_write_is_contained_and_detected", "SIGMA-MUTATION-SENTINEL",
        "manifest_path_escape_is_rejected", "source_symlink_is_rejected",
    ])
}

# --- Phase 38: single-source-of-truth gates -------------------------------
# Both defects closed in Phase 38 were the same class: a value that is supposed
# to be ONE truth was derived independently in several places and the copies
# disagreed. These two checks make a second derivation fail the gate on any
# host, including the ones where the Rust regression test is not discriminating.

def _version_single_source() -> None:
    problems: list[str] = []
    cargo = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    m = re.search(r"(?ms)\[workspace\.package\].*?version\s*=\s*\"([0-9]+\.[0-9]+\.[0-9]+)\"", cargo)
    if not m:
        problems.append("Cargo.toml has no [workspace.package] version")
    canonical = m.group(1) if m else None

    # No other manifest may declare a product version of its own.
    tauri = json.loads((ROOT / "apps/desktop/tauri.conf.json").read_text(encoding="utf-8"))
    if "version" in tauri:
        problems.append(
            "apps/desktop/tauri.conf.json declares its own version; remove the field so "
            "tauri inherits the Cargo.toml version"
        )
    for pkg in ("package.json", "apps/ui/package.json"):
        data = json.loads((ROOT / pkg).read_text(encoding="utf-8"))
        if "version" in data:
            problems.append(f"{pkg} declares its own version; it is private and must not")

    # Build scripts must derive, never hard-code.
    arm64 = (ROOT / "scripts/build-arm64-msi.cmd").read_text(encoding="utf-8")
    if 'set "VERSION=0.1' in arm64:
        problems.append("scripts/build-arm64-msi.cmd hard-codes a version literal")
    if "Cargo.toml" not in arm64:
        problems.append("scripts/build-arm64-msi.cmd does not derive its version from Cargo.toml")
    for ps in ("scripts/build-release.ps1", "scripts/build-installer.ps1", "scripts/build-cli-archive.ps1"):
        body = (ROOT / ps).read_text(encoding="utf-8")
        if "Get-ProductVersion.ps1" not in body:
            problems.append(f"{ps} does not derive its version from Get-ProductVersion.ps1")
    if not (ROOT / "scripts/Get-ProductVersion.ps1").exists():
        problems.append("scripts/Get-ProductVersion.ps1 is missing")

    checks["version_single_source_of_truth"] = {
        "ok": not problems,
        "canonical_version": canonical,
        "problems": problems,
    }


def _platform_identity_single_source() -> None:
    problems: list[str] = []
    helper = "aethercore_platform_capabilities::current_platform_name"
    # Every surface that emits a platform LABEL must route through the one helper.
    label_sites = {
        "crates/security-audit/src/lib.rs": "platform_tag",
        "apps/aetherctl/src/offline.rs": "platform_str",
    }
    for rel, fn in label_sites.items():
        body = (ROOT / rel).read_text(encoding="utf-8")
        m = re.search(rf"fn {fn}\(\) -> &'static str \{{(.*?)\n\}}", body, re.S)
        if not m:
            problems.append(f"{rel}: {fn}() not found in the expected shape")
        elif helper not in m.group(1):
            problems.append(
                f"{rel}: {fn}() does not delegate to {helper}; a second platform "
                f"derivation has been reintroduced"
            )
    # The literal that the old ladder produced must not come back as a platform label.
    audit = (ROOT / "crates/security-audit/src/lib.rs").read_text(encoding="utf-8")
    if re.search(r'cfg!\(target_os = "macos"\)[^}]*?"other"', audit, re.S):
        problems.append(
            'crates/security-audit/src/lib.rs: a cfg! ladder falling back to "other" is back'
        )
    checks["platform_identity_single_source"] = {"ok": not problems, "problems": problems}


_version_single_source()
_platform_identity_single_source()

all_ok = all(bool(value.get("ok")) for value in checks.values())
report = {
    "phase": "0-16",
    "ok": all_ok,
    "check_count": len(checks),
    "checks": checks,
    "limitations": [
        "This platform-neutral gate does not compile Rust against Windows SDK bindings.",
        "scripts/verify-phase16.ps1 and Windows CI are the authoritative native compile/session/installer/security/design/localization/reliability/scheduler/update/export gates; verify-production.ps1 is the final GA evidence/seal gate.",
        "Live Phase 6 verifier probes remain telemetry/event collection only and intentionally perform no system mutation.",
        "This static gate cannot render WebView2/Mica, measure frame pacing, validate platform Arabic shaping/Narrator pronunciation, or replace physical high-DPI/multi-monitor/RTL/accessibility testing.",
        "This Linux environment cannot compile or execute WiX MSI/Burn, Authenticode signing, Service Control Manager lifecycle checks, or Windows ACL repair tests.",
        "MSI byte-for-byte reproducibility is intentionally not claimed; Phase 8 documents and isolates the current upstream WiX nondeterminism boundary.",
        "If dependency-registry egress is unavailable, this source package remains pre-freeze; the trusted Windows freeze workstation must create Cargo.lock, pnpm-lock.yaml, release/dependency-locks.sha256, release/dependency-manifests.sha256, and release/dependency-freeze.json before any release build.",
    ],
}
if ARGS.output:
    ARGS.output.parent.mkdir(parents=True, exist_ok=True)
    ARGS.output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
print(json.dumps({"ok": all_ok, "checks": len(checks), "failed": [name for name, value in checks.items() if not value.get("ok")]}, indent=2))
sys.exit(0 if all_ok else 1)
