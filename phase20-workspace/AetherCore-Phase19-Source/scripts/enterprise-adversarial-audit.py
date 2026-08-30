#!/usr/bin/env python3
from __future__ import annotations

import argparse, json, re, sys, tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PARSER = argparse.ArgumentParser()
PARSER.add_argument("--output", type=Path)
ARGS = PARSER.parse_args()
checks: dict[str, dict[str, object]] = {}


def read(rel: str) -> str:
    path = ROOT / rel
    return path.read_text(encoding="utf-8") if path.is_file() else ""


def check(name: str, ok: bool, **details: object) -> None:
    checks[name] = {"ok": bool(ok), **details}


def has(text: str, *tokens: str) -> bool:
    return all(token in text for token in tokens)


def ordered(text: str, *tokens: str) -> bool:
    """True only when every token exists and appears in the requested order."""
    cursor = 0
    for token in tokens:
        position = text.find(token, cursor)
        if position < 0:
            return False
        cursor = position + len(token)
    return True


def section(text: str, start: str, end: str) -> str:
    """Return a bounded source section; missing delimiters fail closed to an empty string."""
    begin = text.find(start)
    if begin < 0:
        return ""
    finish = text.find(end, begin + len(start))
    if finish < 0:
        return ""
    return text[begin:finish]


def product_rust_sources() -> list[Path]:
    paths: list[Path] = []
    for root in (ROOT / "crates", ROOT / "services", ROOT / "apps"):
        if not root.exists():
            continue
        for path in root.rglob("*.rs"):
            if "tests" in path.parts or path.name == "build.rs":
                continue
            paths.append(path)
    return paths


workspace = tomllib.loads(read("Cargo.toml"))
members = set(workspace.get("workspace", {}).get("members", []))
foundation = read("crates/windows-foundation/src/lib.rs")
security = read("crates/security/src/lib.rs")
scheduler_win = read("crates/idle-scheduler/src/windows_state.rs")
hardware_win = read("crates/hardware-telemetry/src/windows_impl.rs")
startup_win = read("crates/startup-manager/src/windows_impl.rs")
restore_win = read("crates/restore-point/src/windows_impl.rs")
wua_discovery = read("crates/windows-update/src/windows_impl.rs")
wua_execution = read("crates/windows-update/src/execution_windows.rs")
cleaner_win = read("crates/cleaner/src/windows_impl.rs")
repair_win = read("crates/system-repair/src/windows_impl.rs")
update_broker = read("apps/update-broker/src/main.rs")
service_server = read("services/maintenance-service/src/server.rs")
ipc_lib = read("crates/ipc/src/lib.rs")
ipc_win = read("crates/ipc/src/windows_impl.rs")
desktop_cargo = read("apps/desktop/Cargo.toml")
desktop_main = read("apps/desktop/src/main.rs")
support = read("crates/support-bundle/src/lib.rs")
support_service = read("services/maintenance-service/src/support.rs")
update = read("crates/update-engine/src/coordinator.rs")
event_bus = read("crates/operation-kernel/src/event_bus.rs")
kernel_lib = read("crates/operation-kernel/src/lib.rs")
contracts = read("apps/ui/src/lib/contracts.ts")
stream = read("apps/ui/src/platform/stream-state.ts")
preferences = read("apps/ui/src/design/motion/preferences.ts")
fluid_press = read("apps/ui/src/design/motion/fluid-press.ts")
drivers_page = read("apps/ui/src/features/drivers/DriversPage.svelte")
ci = read(".github/workflows/ci.yml")
release = read(".github/workflows/release.yml")
verify_enterprise = read("scripts/verify-enterprise.ps1")
stress_enterprise = read("scripts/enterprise-stress-matrix.ps1")
production_gate = read("scripts/verify-production.ps1")
selector = read("scripts/invoke-cargo-test-case.ps1")

required = [
    "crates/windows-foundation/Cargo.toml",
    "crates/windows-foundation/src/lib.rs",
    "scripts/enterprise-adversarial-audit.py",
    "scripts/enterprise-adversarial-audit.ps1",
    "scripts/enterprise-stress-matrix.ps1",
    "scripts/verify-maintenance-service-token.ps1",
    "scripts/enterprise-soak-analyze.py",
    "scripts/verify-enterprise.ps1",
    "docs/ENTERPRISE_ADVERSARIAL_AUDIT.md",
    "docs/adr/0019-enterprise-convergence-refactor.md",
    "ENTERPRISE_TRANSFORMATION_MATRIX.md",
    "ENTERPRISE_DELIVERABLES.md",
    "release/enterprise-stress-matrix.json",
]
check("required_enterprise_artifacts", all((ROOT / p).is_file() for p in required), missing=[p for p in required if not (ROOT / p).is_file()])
check("enterprise_runtime_verifies_live_service_token_policy", has(stress_enterprise, "verify-maintenance-service-token.ps1", "maintenance_service_token", "Native maintenance service token verification failed"))

# Windows FFI ownership and privilege-context guards.
check("windows_foundation_workspace_member", "crates/windows-foundation" in members)
check("windows_foundation_raii_kernel_handle", has(foundation, "pub struct OwnedHandle", "CloseHandle", "impl Drop for OwnedHandle"))
check("windows_foundation_raii_service_handle", has(foundation, "pub struct OwnedServiceHandle", "CloseServiceHandle", "impl Drop for OwnedServiceHandle"))
check("windows_foundation_raii_com_apartment", has(foundation, "pub struct ComApartment", "CoInitializeEx", "CoUninitialize", "impl Drop for ComApartment"))
check("windows_foundation_raii_impersonation", has(foundation, "ThreadImpersonation", "named_pipe_client", "logged_on_user", "RevertToSelf", "pub fn revert"))
check("windows_foundation_background_mode", has(foundation, "BackgroundThreadMode", "THREAD_MODE_BACKGROUND_BEGIN", "THREAD_MODE_BACKGROUND_END"))
check("windows_foundation_protected_machine_mutation_guard", has(foundation, "pub struct MachineMutationGuard", "SHGetKnownFolderPath", "FOLDERID_ProgramData", "machine-mutation.lock", "OPEN_EXISTING", "FILE_FLAG_OPEN_REPARSE_POINT", "LockFileEx", "LOCKFILE_FAIL_IMMEDIATELY", "ERROR_LOCK_VIOLATION", "UnlockFileEx"))
mutation_domains = (cleaner_win, repair_win, startup_win, wua_execution, update_broker)
check("machine_mutation_authority_is_centralized", all("MachineMutationGuard::try_acquire()" in text for text in mutation_domains) and all("LockFileEx(" not in text and "UnlockFileEx(" not in text for text in mutation_domains))
check("update_broker_acquires_machine_lock_before_claim", ordered(update_broker, "UpdateMutationGuard::acquire", "claim(&intent_id)"))
check("background_mode_admission_is_fail_closed", has(foundation, "pub fn enter() -> windows::core::Result<Self>", "SetThreadPriority(GetCurrentThread(), THREAD_MODE_BACKGROUND_BEGIN)?") and has(scheduler_win, "THREAD_MODE_BACKGROUND_BEGIN failed", "BackgroundThreadMode::enter()"))
check("security_uses_raii_impersonation_and_handles", has(security, "ThreadImpersonation::named_pipe_client", "OwnedHandle::new", "impersonation.revert()") and "CloseHandle(" not in security)
check("security_explicit_revert_runs_after_failed_token_inspection", has(security, "let result = (|| {", "OpenThreadToken", "impersonation.revert().map_err(winerr)?;", "returning while still impersonating"))
check("scheduler_uses_raii_user_token_and_impersonation", has(scheduler_win, "OwnedHandle::new", "ThreadImpersonation::logged_on_user", "impersonation.revert()"))
check("scheduler_uses_raii_com_and_service_handles", has(scheduler_win, "ComApartment::mta", "OwnedServiceHandle::new", "BackgroundThreadMode::enter"))
check("hardware_uses_raii_com_and_storage_handles", has(hardware_win, "ComApartment::mta", "OwnedHandle::new") and "CoUninitialize(" not in hardware_win and "CloseHandle(" not in hardware_win)
check("startup_uses_raii_com_and_service_handles", has(startup_win, "ComApartment::mta", "OwnedServiceHandle::new") and "CoUninitialize(" not in startup_win and "CloseServiceHandle(" not in startup_win)
check("restore_point_uses_raii_thread_com", has(restore_win, "ComApartment::mta", "CoInitializeSecurity") and "CoUninitialize(" not in restore_win)
check("windows_update_uses_raii_com", all("CoUninitialize(" not in text and "ComApartment::mta" in text for text in (wua_discovery, wua_execution)))
check("desktop_shell_execute_process_handles_are_raii", "aethercore-windows-foundation" in desktop_cargo and "OwnedHandle::new" in desktop_main and "CloseHandle(" not in desktop_main)
check("desktop_reconnect_is_transport_failure_scoped", has(desktop_main, "if !client.is_alive()", "invalidate_session_if_current(&client)") and "Request-level deadline/backpressure errors do not invalidate" in desktop_main)
manual_common_release_hits: list[str] = []
for path in product_rust_sources():
    if path.relative_to(ROOT).as_posix() == "crates/windows-foundation/src/lib.rs":
        continue
    text = path.read_text(encoding="utf-8", errors="ignore").split("#[cfg(test)]", 1)[0]
    for token in ("CloseHandle(", "CloseServiceHandle(", "CoUninitialize(", "RevertToSelf("):
        if token in text:
            manual_common_release_hits.append(f"{path.relative_to(ROOT)}:{token[:-1]}")
check("common_windows_release_pairs_are_centralized", not manual_common_release_hits, hits=manual_common_release_hits)

# IPC write-side backpressure must never trap request workers behind a synchronous pipe write.
check("ipc_outbound_backpressure_error_is_typed", "OutboundBackpressure" in ipc_lib)
check("ipc_server_outbound_queue_is_bounded", has(ipc_win, "SERVER_OUTBOUND_QUEUE_CAPACITY: usize = 32", "SERVER_OUTBOUND_BYTE_BUDGET: usize = 16 * 1024 * 1024", "mpsc::sync_channel::<QueuedServerFrame>(SERVER_OUTBOUND_QUEUE_CAPACITY)", "mpsc::SyncSender<QueuedServerFrame>"))
check("ipc_client_outbound_queue_is_bounded", has(ipc_win, "CLIENT_OUTBOUND_QUEUE_CAPACITY: usize = 32", "CLIENT_OUTBOUND_BYTE_BUDGET: usize = 4 * 1024 * 1024", "mpsc::sync_channel::<QueuedClientFrame>(CLIENT_OUTBOUND_QUEUE_CAPACITY)", "mpsc::SyncSender<QueuedClientFrame>"))
check("ipc_producers_use_nonblocking_try_send", has(ipc_win, "try_send(QueuedServerFrame", "try_send(QueuedClientFrame", "TrySendError::Full", "IpcError::OutboundBackpressure"))
check("ipc_outbound_queues_are_byte_bounded", has(ipc_win, "fn try_reserve_bytes", "compare_exchange_weak", "release_bytes", "server_outbound_byte_budget_is_fail_closed_even_when_frame_slots_remain", "byte_reservation_never_exceeds_limit_under_contention"))
check("ipc_bootstrap_replay_wait_is_bounded", has(ipc_win, "pub fn write_bootstrap", "checked_add(max_wait)", "Instant::now()>=deadline", "thread::sleep(Duration::from_millis(1))") and has(service_server, "IPC_BOOTSTRAP_ENQUEUE_TIMEOUT", ".write_bootstrap("))
check("ipc_live_writes_remain_nonblocking", service_server.count(".write(&") >= 2 and "write_bootstrap" in service_server)
check("ipc_has_dedicated_server_writer_thread", has(ipc_win, "aether-ipc-server-writer", "write_server_frame(&mut writer_file", "pump_alive.store(false"))
check("ipc_has_dedicated_client_writer_thread", has(ipc_win, "aether-ipc-client-writer", "write_client_frame(&mut writer_file", "writer_pending.lock()"))
check("ipc_session_fails_closed_after_writer_failure", has(ipc_win, "if !self.writer.is_alive()", "return Err(IpcError::Disconnected)"))
check("ipc_outbound_backpressure_regression_present", has(ipc_win, "server_outbound_queue_saturates_fail_closed_without_blocking_request_workers", "client_outbound_queue_saturates_fail_closed_and_requests_transport_cancellation", "assert!(!writer.is_alive())", "assert!(!alive.load(Ordering::Acquire))"))
check("ipc_client_inflight_matches_server_hello", has(ipc_win, "ClientInflightGuard", "try_acquire_inflight", "max_inflight", "client_inflight_admission_matches_server_advertised_limit"))
check("ipc_client_shutdown_is_nonblocking_and_cancellable", has(ipc_win, "fn shutdown(&self)", "self.cancellation.cancel()", "cancel_registered_io(&self.peer_reader)", "self.writer.shutdown();") and "fn close_with" not in ipc_win)
check("ipc_disconnect_notification_is_at_most_once", has(ipc_win, "fn notify_disconnect_once", "disconnect_notified", "writer_disconnect", "disconnect_notification_is_at_most_once_across_reader_and_writer_paths"))

# Update-engine publication and state-machine reentrancy.
check("update_observer_never_emits_state_reference", "self.emit(owner,&state.snapshot)" not in update)
check("update_observer_regression_present", has(update, "observer_publication_never_runs_while_state_mutex_is_held", "states.try_lock().is_ok()"))
check("update_upload_registry_is_per_upload", has(update, "struct UploadState", "records:HashMap<String,Arc<Mutex<UploadRecord>>>", "owner_upload:HashMap<String,String>", "starting_owners:HashSet<String>"))
write_stage = section(update, "pub fn write_stage_chunk", "pub fn finalize_stage_upload")
check("update_chunk_io_uses_per_upload_mutex", has(write_stage, "self.upload_record(upload_id)?", "record.lock()", "OpenOptions", "write_all") and "self.uploads.lock" not in write_stage)
check("update_upload_start_is_owner_reserved", has(update, "reserve_upload_start", "release_upload_start", "publish_upload_start", "upload_start_reservations_are_owner_scoped_not_global"))
check("update_upload_finalize_fences_late_chunks", has(update, "closed:bool", "if upload.closed", "upload.closed=true", "take_upload_record"))
check("update_upload_per_record_regressions", has(update, "upload_start_reservations_are_owner_scoped_not_global", "upload_records_use_independent_per_upload_mutexes"))
check("update_staged_paths_are_owner_scoped_and_content_addressed", has(update, "owner_scope", "Sha256::digest(owner.as_bytes())", "sha256.to_ascii_lowercase()", "staged_paths_are_owner_scoped_and_content_addressed"))
check("update_stale_cleanup_protects_active_execution", has(update, "should_remove_stale_staging", "protected.insert(active.ticket.staged_path.clone())", "file_type.is_file()", "stale_staging_cleanup_never_removes_protected_execution_artifact"))
check("update_set_snapshot_publish_after_lock", has(update, "fn set_snapshot", "self.emit(owner,&snapshot)") and "let snapshot={" in update)
mutate_body = section(update, "fn mutate_snapshot", "fn update_progress")
check("update_mutate_snapshot_publish_after_lock", "fn mutate_snapshot" in mutate_body and re.search(r"let\s+published\s*=\s*\{", mutate_body) is not None and "self.emit(owner,&published)" in mutate_body)

# Support-bundle quota and privacy invariants.
check("support_preview_reservation_state", has(support, "struct PreviewState", "reserve_preview", "release_preview"))
check("support_preview_reserves_before_sanitization", ordered(support, "self.reserve_preview(owner)?", "sanitize_value(section.value)") and has(support, "preview_reservations_count_against_global_quota_before_sanitization", "preview_reservation_is_linearized_per_owner"))
check("support_preparation_reservation_state", has(support, "preparing_owners:HashSet<String>", "reserve_preparation", "release_preparation"))
prepare_pos = support.find("pub fn prepare")
reserve_pos = support.find("self.reserve_preparation(owner)?", prepare_pos)
build_pos = support.find("build_archive", prepare_pos)
check("support_reserves_before_crypto_and_archive_io", prepare_pos >= 0 and prepare_pos < reserve_pos < build_pos, reserve_pos=reserve_pos, build_pos=build_pos)
check("support_pending_counts_against_global_quota", has(support, "bundles.ready.len().saturating_add(bundles.preparing_owners.len())", "occupied>=MAX_ACTIVE_BUNDLES_TOTAL"))
check("support_reservation_regressions", has(support, "preparation_reservations_count_against_global_quota_before_archive_io", "preparation_reservation_is_linearized_per_owner"))
check("support_test_lifecycle_discards_after_read", has(support, "engine.discard(\"owner\",&ready.bundle_id).unwrap()"))
check("support_runtime_metrics_are_owner_scoped", "metrics_for_owner(owner)" in support_service and ".events().metrics()" not in support_service)
check("support_still_redacts_principal_and_serial_keys", has(support, "ownerprincipalkey", "usersid", "serialnumber", "<redacted-hardware-serial>", "<redacted-sid>", "<redacted-email>"))
check("support_no_raw_minidump_eventlog_authority", all(token not in support_service for token in ["MEMORY.DMP", "Minidump", "EvtExportLog", "EventLog XML"]))

# Event-stream backpressure observability without cross-principal leakage.
check("event_bus_global_operational_metrics", has(event_bus, "pub struct EventBusMetrics", "published_total", "subscriber_lag_total", "subscriber_disconnect_total"))
check("event_bus_owner_scoped_metrics", has(event_bus, "pub struct OwnerEventBusMetrics", "pub fn metrics_for_owner", "state.owners.get(owner_principal_key)"))
check("event_bus_lag_transition_counted_once", has(event_bus, "lagged.swap(true, Ordering::AcqRel)", "lag_transitions"))
check("event_bus_metrics_regression", has(event_bus, "metrics_expose_backpressure_without_principal_content", "owner-other"))
check("event_bus_subscription_drop_is_eager", has(event_bus, "impl Drop for EventSubscription", "subscriber.id != self.subscriber_id", "dropping_quiet_subscription_releases_registry_immediately"))
check("event_bus_metrics_exported", "OwnerEventBusMetrics" in kernel_lib and "EventBusMetrics" in kernel_lib)

# UI strict event typing, exhaustive reducers and zero-hot-path preference queries.
check("ui_kernel_event_is_discriminated_union", has(contracts, "type KernelEvent<K extends string", "export type UiKernelEvent", "KernelEvent<'serviceSnapshot'"))
check("ui_kernel_event_unknown_payload_is_closed", "kind: string; payload: unknown" not in contracts and "KernelEvent<'unknown', null>" in contracts)
check("ui_stream_reducer_exhaustive", has(stream, "assertNeverEvent(event)", "function assertNeverEvent(event: never)"))
ui_text = "\n".join(read(str(p.relative_to(ROOT))) for p in (ROOT / "apps/ui/src").rglob("*") if p.suffix in {".ts", ".svelte"})
check("ui_no_any_escape_hatches", " as any" not in ui_text and "as unknown as" not in ui_text and "@ts-ignore" not in ui_text and "@ts-expect-error" not in ui_text)
check("drivers_state_index_is_typed", "as any" not in drivers_page and "scanStateIndex(hub.state)" in drivers_page)
check("motion_preferences_matchmedia_cached", preferences.count("window.matchMedia(") == 3 and "subscribeMotionPreferences" in preferences and "MutationObserver" in preferences)
check("fluid_press_hot_path_has_no_matchmedia", "matchMedia(" not in fluid_press and fluid_press.count("readMotionPreferences()") == 1 and "subscribeMotionPreferences" in fluid_press)

# Production panic/placeholder discipline.
panic_pattern = re.compile(r"\b(?:unwrap|expect|panic!|unreachable!)\s*\(")
panic_hits: list[str] = []
for path in product_rust_sources():
    text = path.read_text(encoding="utf-8", errors="ignore").split("#[cfg(test)]", 1)[0]
    for line_no, line in enumerate(text.splitlines(), 1):
        if panic_pattern.search(line):
            panic_hits.append(f"{path.relative_to(ROOT)}:{line_no}")
check("production_rust_has_no_panic_shortcuts", not panic_hits, hits=panic_hits)
placeholder_hits: list[str] = []
for root in (ROOT / "crates", ROOT / "services", ROOT / "apps"):
    for path in root.rglob("*"):
        if path.is_file() and path.suffix.lower() in {".rs", ".ts", ".svelte", ".css", ".proto"}:
            text = path.read_text(encoding="utf-8", errors="ignore")
            if re.search(r"\b(?:TODO|FIXME|HACK)\b", text):
                placeholder_hits.append(str(path.relative_to(ROOT)))
check("product_has_zero_todo_fixme_hack", not placeholder_hits, hits=placeholder_hits)

# Verification cannot silently pass a zero-match targeted Rust test.
check("targeted_rust_test_selector_enumerates_first", has(selector, "--list", "matched $($matches.Count)", "-ne 1"))
raw_exact_hits: list[str] = []
for path in (ROOT / "scripts").glob("*.ps1"):
    text = path.read_text(encoding="utf-8", errors="ignore")
    if "-- --exact" in text:
        raw_exact_hits.append(path.name)
check("no_raw_exact_zero_match_test_gates", not raw_exact_hits, hits=raw_exact_hits)

# Master verification / CI / release convergence.
check("enterprise_gate_inherits_phase16", has(verify_enterprise, "verify-phase16.ps1", "enterprise-adversarial-audit.ps1", "cargo clippy", "-D", "warnings"))
check("enterprise_gate_runs_full_workspace_tests", has(verify_enterprise, "cargo test", "--workspace", "--locked"))
check("enterprise_gate_runs_ui_check_and_build", has(verify_enterprise, "pnpm --dir apps/ui check", "pnpm --dir apps/ui build"))
check("enterprise_stress_reuses_ga_soak_and_resilience", has(stress_enterprise, "phase16-stress-soak.ps1", "phase16-resilience-matrix.ps1", "enterprise-soak-analyze.py", "observer_publication_never_runs_while_state_mutex_is_held", "upload_start_reservations_are_owner_scoped_not_global", "upload_records_use_independent_per_upload_mutexes", "staged_paths_are_owner_scoped_and_content_addressed", "stale_staging_cleanup_never_removes_protected_execution_artifact", "preparation_reservations_count_against_global_quota_before_archive_io", "server_outbound_queue_saturates_fail_closed_without_blocking_request_workers", "dropping_quiet_subscription_releases_registry_immediately", "server_outbound_byte_budget_is_fail_closed_even_when_frame_slots_remain", "byte_reservation_never_exceeds_limit_under_contention", "client_inflight_admission_matches_server_advertised_limit"))
check("ci_runs_enterprise_master_gate", "verify-enterprise.ps1 -SkipOnlineSupplyChain" in ci)
check("release_runs_enterprise_master_gate", "verify-enterprise.ps1 -ReleasePackaging" in release)
check("ga_seal_requires_enterprise_audit", "enterprise-adversarial-audit.ps1" in production_gate)
seal_release = read("scripts/phase16-seal-release.ps1")
check("ga_seal_requires_enterprise_runtime_evidence", has(seal_release, "aethercore.enterprise-stress-evidence.v1", "aethercore.enterprise-resource-trend.v1", "targeted_regressions -ge 17", "enterprise-resource-trend-pass", "enterprise-targeted-regressions-pass"))

# Historical gates remain intact.
for name, rel in [
    ("phase13", "scripts/phase13-reliability-audit.py"),
    ("phase14", "scripts/phase14-scheduler-audit.py"),
    ("phase15", "scripts/phase15-security-audit.py"),
    ("phase16", "scripts/phase16-ga-audit.py"),
    ("aggregate", "scripts/static_validate.py"),
]:
    check(f"inherited_{name}_gate_present", (ROOT / rel).is_file())

failed = [name for name, value in checks.items() if not value.get("ok")]
report = {
    "schema": "aethercore.enterprise-adversarial-audit.v1",
    "ok": not failed,
    "check_count": len(checks),
    "failed": failed,
    "checks": checks,
    "qualification_boundary": [
        "This audit proves source-level architectural invariants only.",
        "Windows compilation, clippy, runtime FFI behavior, Authenticode, SCM/UAC, WebView2 rendering, accessibility and soak evidence remain native gates.",
        "No finite test suite can mathematically guarantee zero future defects or zero memory leaks; the enterprise gate uses fail-closed regression, stress and evidence thresholds.",
    ],
}
if ARGS.output:
    ARGS.output.parent.mkdir(parents=True, exist_ok=True)
    ARGS.output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
print(json.dumps({"ok": report["ok"], "checks": len(checks), "failed": failed}, indent=2))
sys.exit(0 if report["ok"] else 1)
