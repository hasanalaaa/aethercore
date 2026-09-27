#!/usr/bin/env python3
from __future__ import annotations

import argparse, json, re, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PARSER = argparse.ArgumentParser()
PARSER.add_argument("--output", type=Path)
ARGS = PARSER.parse_args()
checks: dict[str, dict[str, object]] = {}


# P59 / DBT-P58-005: one shared reader that raises instead of substituting "".
# P58 fixed where this reader looked - `.github/` resolves against the
# repository root - but not what it did when the look failed, so a check
# asserting something is ABSENT still passed against a file never opened.
# No bytecode: `omega-evidence.py` runs each gate against a disposable clone
# and treats ANY new file in it as a source mutation, so a `__pycache__`
# entry for this import would be reported as the gate rewriting the tree.
sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
from gate_reader import SourceReader, contains, count, ordered as _gate_ordered  # noqa: E402

_READER = SourceReader(ROOT)
read = _READER.read
read_module = _READER.read_module


def check(name: str, ok: bool, **details: object) -> None:
    checks[name] = {"ok": bool(ok), **details}


def has(text: str, *tokens: str) -> bool:
    # `contains`, not `in`: the comparison ignores whitespace and is defined
    # once in gate_reader. `DBT-P61-001`.
    return all(contains(text, token) for token in tokens)


# `ordered` is gate_reader's too: same whitespace rule as `has`, one definition.
# `DBT-P61-001`.
ordered = _gate_ordered


def section(text: str, start: str, end: str) -> str:
    """Return a bounded source section; missing delimiters fail closed to an empty string."""
    begin = text.find(start)
    if begin < 0:
        return ""
    finish = text.find(end, begin + len(start))
    if finish < 0:
        return ""
    return text[begin:finish]


def mask_rust_noncode(text: str) -> str:
    """Mask comments and literals while preserving offsets/braces for lightweight structural checks."""
    out = list(text)
    i = 0
    n = len(text)
    while i < n:
        if text.startswith("//", i):
            j = text.find("\n", i + 2)
            if j == -1:
                j = n
            for k in range(i, j):
                out[k] = " "
            i = j
            continue
        if text.startswith("/*", i):
            depth = 1
            j = i + 2
            while j < n and depth:
                if text.startswith("/*", j):
                    depth += 1
                    j += 2
                elif text.startswith("*/", j):
                    depth -= 1
                    j += 2
                else:
                    j += 1
            for k in range(i, min(j, n)):
                if out[k] != "\n":
                    out[k] = " "
            i = j
            continue
        raw = re.match(r'r(?P<h>#{0,16})"', text[i:])
        if raw:
            hashes = raw.group("h")
            terminator = '"' + hashes
            start = i
            j = text.find(terminator, i + raw.end())
            j = n if j == -1 else j + len(terminator)
            for k in range(start, j):
                if out[k] != "\n":
                    out[k] = " "
            i = j
            continue
        if text[i] == '"':
            start = i
            i += 1
            escaped = False
            while i < n:
                ch = text[i]
                if escaped:
                    escaped = False
                elif ch == "\\":
                    escaped = True
                elif ch == '"':
                    i += 1
                    break
                i += 1
            for k in range(start, i):
                if out[k] != "\n":
                    out[k] = " "
            continue
        # Mask character literals, but not Rust lifetimes such as 'a.
        if text[i] == "'" and i + 2 < n:
            char_match = re.match(r"'(?:\\.|[^'\\])'", text[i:])
            if char_match:
                j = i + char_match.end()
                for k in range(i, j):
                    out[k] = " "
                i = j
                continue
        i += 1
    return "".join(out)


def duplicate_impl_methods(text: str) -> list[str]:
    """Return duplicate top-level method names, per cfg predicate, in one Rust impl block."""
    masked = mask_rust_noncode(text)
    duplicates: list[str] = []
    for match in re.finditer(r"(?m)^\s*impl\b[^\n{]*\{", masked):
        brace = masked.find("{", match.start(), match.end())
        if brace == -1:
            continue
        depth = 1
        end = brace + 1
        while end < len(masked) and depth:
            if masked[end] == "{":
                depth += 1
            elif masked[end] == "}":
                depth -= 1
            end += 1
        if depth != 0:
            continue
        body = masked[brace + 1:end - 1]
        local_depth = 0
        methods: list[str] = []
        cursor = 0
        method_re = re.compile(r"(?m)^\s*(?:pub(?:\([^)]*\))?\s+)?(?:(?:async|unsafe|const)\s+)*(?:extern\s+\"[^\"]+\"\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)\s*(?:<[^>{}]*>)?\s*\(")
        for method in method_re.finditer(body):
            segment = body[cursor:method.start()]
            local_depth += segment.count("{") - segment.count("}")
            cursor = method.start()
            if local_depth == 0:
                # Two `fn handshake` in one impl are a duplicate; a `#[cfg(unix)]` one and a
                # `#[cfg(windows)]` one are not - they never coexist, and the compiler would
                # reject them if they did. The cfg predicates attached to THIS item (every
                # attribute after the previous item's `}` or `;`) are part of the key.
                # `DBT-P61-001`.
                attached = re.split(r"[};]", body[:method.start()])[-1]
                cfgs = "".join(sorted(re.findall(r"#\[cfg[^\]]*\]", attached)))
                methods.append((cfgs, method.group(1)))
        seen: set[tuple[str, str]] = set()
        for key in methods:
            if key in seen:
                header = masked[match.start():brace].strip().replace("\n", " ")
                duplicates.append(f"{header}::{key[0]}{key[1]}")
            seen.add(key)
    return duplicates


engine = read("crates/operation-engine/src/lib.rs")
event_bus = read("crates/operation-kernel/src/event_bus.rs")
update = read("crates/update-engine/src/coordinator.rs")
server = read("services/maintenance-service/src/server.rs")
# `DBT-P63-004`: `main.rs` was 397 lines, 244 of them three inline `mod x { }`
# blocks, and `phase10-architecture-audit.ps1:153` throws above 220. The blocks
# are files beside it now. What each check below asserts did not move; the file
# that holds it did.
service_main = read("services/maintenance-service/src/main.rs")
service_scm_host = read("services/maintenance-service/src/windows_service_host.rs")
security_core = read("crates/security/src/lib.rs")
streaming = read("services/maintenance-service/src/streaming.rs")
# `DBT-P63-004`: `router.rs` plus `router/*.rs`. The leased-route counts below
# count verbs, and the verbs moved into the tree.
router = read_module("services/maintenance-service/src/router.rs")
mutation = read("crates/operation-kernel/src/mutation.rs")
work_budget = read("crates/operation-kernel/src/work_budget.rs")
driver_install = read("crates/driver-install/src/lib.rs")
system_repair = read("crates/system-repair/src/lib.rs")
cleaner = read("crates/cleaner/src/lib.rs")
startup = read("crates/startup-manager/src/lib.rs")
driver_hub = read("crates/driver-hub/src/lib.rs")
diagnostics = read("crates/diagnostic-engine/src/lib.rs")
ipc_windows = read("crates/ipc/src/windows_impl.rs")
ipc_lib = read("crates/ipc/src/lib.rs")
repair_windows = read("crates/system-repair/src/windows_impl.rs")
desktop = read("apps/desktop/src/main.rs")
driver_hub_toml = read("crates/driver-hub/Cargo.toml")
diagnostics_toml = read("crates/diagnostic-engine/Cargo.toml")
press = read("apps/ui/src/design/motion/fluid-press.ts")
motion_css = read("apps/ui/src/design/styles/motion.css")
verify_zenith = read("scripts/verify-zenith.ps1")
ci = read(".github/workflows/ci.yml")
release = read(".github/workflows/release.yml")
production = read("scripts/verify-production.ps1")
ipc_pipe_probe = read("scripts/verify-ipc-pipe-security.ps1")
service_token_probe = read("scripts/verify-maintenance-service-token.ps1")
enterprise_stress = read("scripts/enterprise-stress-matrix.ps1")
windows_foundation = read("crates/windows-foundation/src/lib.rs")
install_hardener = read("apps/install-hardener/src/main.rs")
installer_verify = read("scripts/verify-installer-security.ps1")
update_broker = read("apps/update-broker/src/main.rs")
startup_windows = read("crates/startup-manager/src/windows_impl.rs")
cleaner_windows = read("crates/cleaner/src/windows_impl.rs")
windows_update = read("crates/windows-update/src/execution_windows.rs")

required = [
    "ZENITH_RECURSIVE_EXECUTIVE_REPORT.md",
    "ZENITH_RECURSIVE_ARCHITECTURE_MAP.md",
    "ZENITH_RECURSIVE_TRANSFORMATION_MATRIX.md",
    "ZENITH_RECURSIVE_ADVERSARIAL_REVIEW.md",
    "ZENITH_RECURSIVE_VERIFICATION.md",
    "ZENITH_RECURSIVE_ISSUE_LEDGER.md",
    "docs/adr/0021-recursive-adversarial-hardening.md",
    "scripts/zenith-recursive-audit.py",
    "scripts/zenith-recursive-audit.ps1",
    "scripts/verify-zenith-recursive.ps1",
    "scripts/verify-ipc-pipe-security.ps1",
    "scripts/verify-maintenance-service-token.ps1",
]
check("recursive_artifact_set", all((ROOT / rel).is_file() for rel in required), missing=[rel for rel in required if not (ROOT / rel).is_file()])

# ZR-001: hashed immutable material is the consent/execution presentation authority.
coherence_tokens = [
    "material.id != p.id",
    "material.title != p.title",
    "material.risk != p.risk",
    "material.created_unix_ms != p.created_unix_ms",
    "material.owner_principal_key != p.owner_principal_key",
    "p.updated_unix_ms < p.created_unix_ms",
]
check("plan_duplicate_columns_fail_closed", has(engine, *coherence_tokens))
view = section(engine, "fn view_from", "fn material_from")
check("plan_view_uses_hashed_material", has(view, "id: material.id", "title: material.title", "risk: material.risk", "created_unix_ms: material.created_unix_ms", "owner_principal_key: material.owner_principal_key"))
check("plan_corruption_regression_test", has(engine, "duplicated_plan_columns_cannot_diverge_from_hashed_material", "Install 0 harmless updates", "EngineError::IntegrityMismatch"))
check("consent_path_inherits_integrity_gate", has(engine, "begin_consent_intent", "let plan = self.get_plan_for_owner(id, owner_principal_key)?;"))

# ZR-002: long-lived per-owner replay memory is bounded without evicting live subscribers.
check("event_bus_owner_retention_cap", has(event_bus, "const MAX_OWNER_STREAMS: usize = 128;", "last_activity_tick", "activity_clock"))
check("event_bus_only_evicts_inactive", has(event_bus, ".filter(|(_, stream)| stream.subscribers.is_empty())", ".min_by_key(|(_, stream)| stream.last_activity_tick)"))
check("event_bus_reuses_bounded_owner_helper", event_bus.count("owner_stream_mut(&mut state, owner_principal_key)") == 2)
check("event_bus_eviction_reconnect_test", has(event_bus, "inactive_owner_replay_state_is_bounded_and_reconnect_resets_safely", "assert!(!replay.complete)"))
check("event_bus_active_owner_protection_test", "active_owner_stream_is_never_evicted_under_retention_pressure" in event_bus)

# ZR-003: update owner-state cache does not retain historical logon principals forever.
check("update_owner_state_retention_cap", has(update, "const MAX_OWNER_STATE_CACHE:usize=128;", "fn admit_owner_state", "while states.len()>=MAX_OWNER_STATE_CACHE"))
check("update_owner_state_protects_mutation_states", has(update, "fn owner_state_is_evictable", "UpdateState::Staging|UpdateState::Staged|UpdateState::AwaitingConsent|UpdateState::Installing"))
check("update_owner_state_admission_on_creation", count(update, "admit_owner_state(&mut states,owner);") == 3)
check("update_owner_state_bounded_regression", "inactive_update_owner_state_cache_is_bounded" in update)
check("update_owner_state_critical_regression", "mutation_critical_update_state_is_never_evicted" in update)

# ZR-004: request worker admission remains bounded across client reconnect churn.
check("service_global_request_cap", has(server, "const MAX_GLOBAL_INFLIGHT_REQUESTS: usize = 64;", "static ACTIVE_REQUESTS: AtomicUsize"))
check("service_global_slot_is_atomic", has(server, "fn try_acquire_slot", "compare_exchange_weak", "Ordering::AcqRel"))
check("service_global_slot_raii", has(server, "struct GlobalRequestGuard", "ACTIVE_REQUESTS.fetch_sub(1, Ordering::AcqRel)"))
check("service_global_admission_fails_closed", has(server, "GlobalRequestGuard::try_acquire()", "service request worker limit reached", "ErrorCode::Busy"))
check("service_worker_owns_global_guard", "let _global_request_guard = global_request_guard;" in server)
check("service_global_cap_regression_test", "global_request_slot_admission_is_strictly_bounded" in server)

# ZR-005: mutation authority is owned by the actual execution worker, never a UI watcher.
check("mutation_lease_identity_is_exact", has(mutation, "pub fn matches(", "self.snapshot.workload == workload", "self.snapshot.plan_id == plan_id", "self.snapshot.owner_principal_key == owner_principal_key"))
check("mutation_lease_identity_regression", "lease_handoff_identity_is_exact" in mutation)
mutation_domains = [
    ("driver", driver_install, "MutationWorkload::DriverInstall"),
    ("repair", system_repair, "MutationWorkload::SystemRepair"),
    ("cleanup", cleaner, "MutationWorkload::Cleanup"),
    ("startup", startup, "MutationWorkload::Startup"),
]
check("mutation_workers_own_lease", all("let _mutation_lease" in text and "start_with_lease" in text and workload in text for _, text, workload in mutation_domains))
check("mutation_entrypoints_cannot_bypass_lease", all("Option<MutationLease>" not in text and "pub fn start(" not in text for _, text, _ in mutation_domains))
# `DBT-P63-004`: the verb handlers name `principal_key` as a `&String` borrowed
# from the dispatch state, where the match arms held it as an owned `String`, so
# the call sites lost one `&` that the compiler was dereferencing through
# anyway - `clippy::needless_borrow`, and step 18 runs clippy with `-D warnings`.
# Same call, same argument, one fewer ampersand; the count is still 4.
check("service_routes_use_leased_mutation_start", count(router, "start_with_lease(principal_key,&v.plan_id,lease)") == 4)
check("mutation_watchers_are_not_authority_holders", "MutationLease" not in streaming and "_mutation_lease" not in streaming)
check("mutation_crates_expose_no_unleased_start", all("pub fn start(&self" not in text and "pub fn start(&self," not in text for _, text, _ in mutation_domains))
check("mutation_private_boundary_requires_lease", all("Option<MutationLease>" not in text and ("mutation_lease: MutationLease" in text or "mutation_lease:MutationLease" in text) for _, text, _ in mutation_domains))

# ZR-006: worker-side persistence/transition failures must not disappear silently.
check("repair_worker_errors_are_failed_closed", has(system_repair, "if let Err(error) = run_worker", "fail_repair(&engine, &db, &id, error)"))
check("cleanup_worker_errors_are_failed_closed", has(cleaner, "if let Err(error) = run_cleanup", "fail_cleanup(&engine, &db, &id, error)"))
check("mutation_worker_spawn_failures_are_typed", all("thread::Builder::new()" in text and "worker creation failed" in text for _, text, _ in mutation_domains))

# ZR-007: service thread admission and failure cleanup are explicit instead of panic-based.
check("session_spawn_is_non_panicking", has(server, 'name("aether-ipc-session"', "ACTIVE_SESSIONS.fetch_sub(1, Ordering::SeqCst)"))
check("event_pump_spawn_is_non_panicking", has(server, 'name("aether-ipc-event-pump"', ".map_err("))
check("request_spawn_failure_cleans_cancellation", has(server, 'name("aether-ipc-request"', "cancels.remove(&scoped)", "service request worker unavailable"))

# ZR-008: read-only resource leases follow the expensive worker, not the observer thread.
check("read_budget_lease_identity_is_exact", has(work_budget, "pub fn matches(&self, kind: ReadWorkload)", "self.kind == kind", "read_budget_lease_identity_is_exact"))
read_domains = [
    (driver_hub, "ReadWorkload::DriverDiscovery", "start_scan_with_lease"),
    (system_repair, "ReadWorkload::RepairAssessment", "start_assessment_with_lease"),
    (cleaner, "ReadWorkload::CleanupDiscovery", "start_scan_with_lease"),
    (startup, "ReadWorkload::StartupDiscovery", "start_scan_with_lease"),
    (diagnostics, "ReadWorkload::Diagnostics", "start_scan_with_lease"),
]
check("read_workers_own_budget_lease", all("let _read_budget_lease" in text and workload in text and method in text for text, workload, method in read_domains))
check("read_entrypoints_cannot_bypass_budget", all("Option<ReadBudgetLease>" not in text and "pub fn start_scan(" not in text and "pub fn start_assessment(" not in text for text, _, _ in read_domains))
check("service_routes_use_leased_read_start", count(router, "start_scan_with_lease(principal_key,lease)") == 4 and contains(router, "start_assessment_with_lease(principal_key,lease)"))
check("read_watchers_are_not_budget_holders", "ReadBudgetLease" not in streaming and "_read_budget_lease" not in streaming)
check("read_worker_spawn_failure_is_recoverable", all("thread::Builder::new()" in text for text, _, _ in read_domains))
check("read_crates_expose_no_unleased_start", all("pub fn start_scan(&self" not in text and "pub fn start_assessment(&self" not in text for text, _, _ in read_domains))
check("read_private_boundary_requires_lease", all("Option<ReadBudgetLease>" not in text and ("read_budget_lease: ReadBudgetLease" in text or "read_budget_lease:ReadBudgetLease" in text) for text, _, _ in read_domains))

# ZR-009: watcher creation cannot panic a request after backend work has already started.
check("stream_watchers_use_non_panicking_builder", has(streaming, "fn spawn_watcher", "thread::Builder::new()", "stream watcher creation failed") and "thread::spawn" not in streaming)
check("all_stream_watchers_use_helper", streaming.count('spawn_watcher("') == 10)  # Phase 17 adds the Deep Scan watcher.

# ZR-010: remaining runtime support threads fail explicitly instead of panicking the caller.
check("ipc_client_reader_spawn_is_fallible", has(ipc_windows, 'name("aether-ipc-client-reader"', ").map_err(IpcError::Io)?;"))
check("repair_pipe_readers_are_fallible", has(repair_windows, 'name("aether-repair-stdout"', 'name("aether-repair-stderr"', "failed to create repair stdout reader", "failed to create repair stderr reader"))
check("repair_timeout_joins_pipe_readers", has(repair_windows, "let _ = stdout_thread.join();", "let _ = stderr_thread.join();", 'format!("{title} timed out")'))
check("desktop_reconnect_spawn_is_fallible", has(desktop, 'name("aether-desktop-ipc-reconnect"', ").map_err(|error| -> Box<dyn std::error::Error>"))
check("new_read_budget_dependencies_are_declared", "aethercore-operation-kernel" in driver_hub_toml and "aethercore-operation-kernel" in diagnostics_toml)

# ZR-013: pipe pumps have explicit cross-thread cancellation on teardown/backpressure.
#
# `482d480` replaced the whole mechanism and these six checks named the old one, which is
# why they read as eight lost safety properties and are not. Before it, both ends did
# BLOCKING I/O from two threads on one SYNCHRONOUS kernel file object and cancellation had
# to target a THREAD: `OpenThread(THREAD_TERMINATE)` + `CancelSynchronousIo`, a
# `SyncIoCancellation` registry to hold those thread handles, and
# `bind_reader_to_current_thread` so the registry knew which thread to shoot at. Its own
# commit message: "Windows request/response has never worked ... every ServiceJob verb
# hung." The pipes are overlapped now, each direction has its own event and OVERLAPPED, and
# cancellation targets the FILE OBJECT - `CancelIoEx(handle, None)` aborts every in-flight
# operation on the handle from any thread, which is strictly broader than the per-thread
# call and needs no registry and no registration handshake.
#
# Each check below asserts the SAME PROPERTY against the mechanism that now provides it,
# and three of them additionally assert the old mechanism cannot come back - satisfying the
# tokens as written would have reintroduced the hang.
# Property: teardown cancels through a handle it legitimately owns, not by reaching at a
# thread. That is now the pipe handle itself. The name is kept so this row stays traceable
# to `DBT-P61-001`; the `asserts` detail says what it measures.
check("ipc_sync_io_cancellation_uses_documented_thread_handle",
      has(ipc_windows, "struct PipeCancel(Arc<SendableHandle>);", "let _ = CancelIoEx(self.0.get(), None);")
      and "OpenThread(THREAD_TERMINATE" not in ipc_windows
      and "CancelSynchronousIo(" not in ipc_windows
      and "SyncIoCancellation" not in ipc_windows,
      asserts="CancelIoEx on the owned pipe handle; the pre-482d480 thread-handle mechanism is absent and must stay absent")
# Property unchanged: the writer's shutdown must abort the session loop's pending read too.
# One `PipeCancel` over the one file object both directions share does it in one call.
check("ipc_server_writer_shutdown_cancels_both_directions",
      has(ipc_windows, "pub struct PipeServerWriter", "cancel: PipeCancel",
          "let mut writer_io = reader.split_direction()?;", "let cancel = reader.cancel();",
          "fn shutdown(&self)", "self.cancel.cancel();"),
      asserts="writer and session reader share one file object and one PipeCancel; shutdown cancels it")
# Property: a server read in progress must be abortable by the writer's shutdown. The
# binding handshake existed ONLY so a thread-targeted cancel knew which thread to name;
# cancelling the handle needs no such registration, so the handshake is gone and asserting
# it would be asserting dead scaffolding. What is asserted is the guarantee it existed for.
check("ipc_server_reader_is_bound_to_session_worker",
      has(ipc_windows, "pub fn read(&mut self) -> Result<ClientFrame>", "if !self.writer.is_alive() {",
          "return Err(IpcError::Disconnected);", "read_client_frame(&mut self.reader)")
      and "bind_reader_to_current_thread" not in ipc_windows
      and "bind_reader_to_current_thread" not in server,
      asserts="the session read is fenced by writer liveness and cancelled through the shared handle; no thread-registration handshake remains")
# Property unchanged, and now literal: both client pumps hold a clone of the SAME
# `PipeCancel` and each cancels it on the way out, so either thread's exit aborts the other.
check("ipc_client_reader_writer_are_cross_cancelled",
      has(ipc_windows, "let writer_cancel = cancel.clone();", "let reader_cancel = cancel.clone();",
          "writer_cancel.cancel();", "reader_cancel.cancel();"),
      asserts="client reader and writer each cancel the shared pipe handle on exit")
check("ipc_client_backpressure_is_fail_closed", has(ipc_windows, "client_outbound_queue_saturates_fail_closed_and_requests_transport_cancellation", "self.shutdown();", "assert!(!alive.load(Ordering::Acquire));"))
check("ipc_client_pump_fences_post_shutdown_writes", has(ipc_windows, "if !writer_alive.load(Ordering::Acquire){", "while let Ok(discarded)=writer_rx.try_recv()", "self.writer.shutdown();"))
# Property: no lock is held across the Win32 cancel call. There is no registry to lock any
# more - `PipeCancel` holds an `Arc<PipeHandle>` and calls straight through - so this is now
# asserted structurally: the cancel body takes no lock at all.
pipe_cancel_body = section(ipc_windows, "impl PipeCancel {", "/// One direction of overlapped I/O")
check("ipc_cancellation_does_not_hold_registry_lock_across_win32",
      has(pipe_cancel_body, "fn cancel(&self)", "let _ = CancelIoEx(self.0.get(), None);")
      and "lock()" not in pipe_cancel_body,
      asserts="PipeCancel::cancel acquires no lock before the Win32 call")
# Property unchanged: the RAII drop releases the handle and does NOT cancel. Cancellation is
# an explicit act; a drop that cancelled would abort live I/O whenever a clone went out of
# scope. The owner is `windows-foundation`'s `SendableHandle` now rather than
# `SyncIoCancellation`, and it lives there because `CloseHandle` belongs in one place - see
# `common_windows_release_pairs_are_centralized`.
pipe_handle_drop = section(windows_foundation, "impl Drop for SendableHandle {", "/// Owns a Win32 kernel HANDLE and closes it exactly once.")
check("ipc_cancellation_raii_drop_is_side_effect_free",
      has(pipe_handle_drop, "let _ = CloseHandle(self.get());")
      and "CancelIoEx" not in pipe_handle_drop
      and "CancelSynchronousIo" not in pipe_handle_drop,
      asserts="Drop closes the handle and cancels nothing")
# Property: a failed wait or a failed thread spawn surfaces WHICH failure it was, instead of
# collapsing into one generic error. There is no registration handshake to fail any more, so
# the two surfaces left are the response wait and the pump spawns.
check("ipc_thread_registration_errors_preserve_cause",
      has(ipc_windows, "Err(mpsc::RecvTimeoutError::Timeout)", "Err(IpcError::DeadlineExceeded)",
          "Err(mpsc::RecvTimeoutError::Disconnected)", "Err(IpcError::Disconnected)",
          ".map_err(IpcError::Io)?;", "return Err(IpcError::Io(error));"),
      asserts="timeout and disconnect map to distinct typed errors; thread-spawn failure carries its io::Error")

# ZR-014 / ZR-021: installed clients authenticate a service-SID-owned pipe and a running
# own-process service before protocol exchange. Production server-instance authority is scoped to
# NT SERVICE\AetherCoreMaintenance rather than LocalSystem/Administrators as broad principals.
# Debug console mode uses a separate per-launch 128-bit rendezvous name and never weakens the fixed
# production endpoint policy.
check("ipc_client_authenticates_service_sid_owned_pipe_before_hello", has(ipc_windows, "fn verify_connected_server", "verify_pipe_owned_by_trusted_service", "GetSecurityInfo", "SE_FILE_OBJECT", "OWNER_SECURITY_INFORMATION", "trusted_service_sid_string") and ordered(ipc_windows, "verify_connected_server(&file, &pipe_name)?;", "write_client_frame(&mut reader"))
check("ipc_trusted_service_sid_resolution_is_bounded_and_cached", has(ipc_windows, "service_principal()", r"NT SERVICE\AetherCoreMaintenance", "LookupAccountNameW", "ERROR_INSUFFICIENT_BUFFER", "MAX_ACCOUNT_SID_BYTES", "MAX_ACCOUNT_DOMAIN_CHARS", "ConvertSidToStringSidW", "OnceLock<String>", "TRUSTED_SERVICE_SID"))
check("ipc_trusted_service_sid_storage_is_aligned_bounded_and_validated", has(ipc_windows, "fn lookup_account_sid(account_name: &str) -> Result<Vec<u64>>", "checked_add(std::mem::size_of::<u64>() - 1)", "checked_mul(std::mem::size_of::<u64>())", "sid_capacity_bytes", "IsValidSid(sid_ptr).as_bool()", "length changed outside its allocated buffer"))
check("ipc_registered_service_state_is_fail_closed", has(ipc_lib, "UntrustedPeer(String)") and has(ipc_windows, "verify_registered_service_running", "SERVICE_RUNNING", "SERVICE_WIN32_OWN_PROCESS", "status.dwProcessId == 0", "TRUSTED_SERVICE_NAME", "AetherCoreMaintenance"))
check("ipc_client_does_not_use_server_only_pid_query", "GetNamedPipeServerProcessId" not in ipc_windows)
pipe_security = section(ipc_windows, "const PIPE_CLIENT_ACCESS_MASK", "/// Authenticate")
# The Authenticated Users grant is spelled with NAMED SDDL rights, FR plus hex 0x2, and the hex
# form 0x00120003 is FORBIDDEN here - see the comment above ZR-016 below for why. The two checks
# that still demanded the hex form were the last two unresolved rows of `DBT-P61-001`.
check("ipc_production_pipe_owner_and_server_authority_are_service_scoped", has(pipe_security, "fn production_pipe_security_descriptor", '"O:{service_sid}D:P(A;;GA;;;{service_sid})(A;;FR;;;AU)(A;;0x00000002;;;AU)"') and has(ipc_windows, "security_descriptor_for_pipe(&pipe_name)?"))
check("ipc_production_pipe_does_not_grant_system_or_admin_server_ace", has(ipc_windows, 'assert!(!descriptor.contains(";;;BA)"))', 'assert!(!descriptor.contains(";;;SY)"))'))
check("maintenance_service_sid_is_unrestricted_for_mutation_compatibility", has(install_hardener, 'run_checked(&sc, ["sidtype", SERVICE_NAME, "unrestricted"])') and has(installer_verify, "qsidtype", "UNRESTRICTED"))
check("ipc_native_probe_requires_unrestricted_service_sid", has(ipc_pipe_probe, "qsidtype $ServiceName", "SID-type drift", "SERVICE_SID_TYPE_UNRESTRICTED", "service_sid_type = 'UNRESTRICTED'"))
mutation_surface = startup_windows + cleaner_windows + repair_windows + windows_update
check(
    "maintenance_service_does_not_write_restrict_arbitrary_windows_mutations",
    has(install_hardener, 'run_checked(&sc, ["sidtype", SERVICE_NAME, "unrestricted"])')
    and has(startup_windows, "RegSetValueExW", "ChangeServiceConfigW")
    and has(cleaner_windows, "SetFileInformationByHandle")
    and has(repair_windows, '"/RestoreHealth"', '"/scannow"')
    and has(windows_update, "BeginInstall")
    and 'run_checked(&sc, ["sidtype", SERVICE_NAME, "restricted"])' not in install_hardener,
)
check("ipc_debug_pipe_token_is_strict_and_compiled_for_debug", has(ipc_lib, "#[cfg(debug_assertions)]", "pipe_name_with_dev_token", "token.len() != 32", "DEV_PIPE_TOKEN_ARG", "DEV_PIPE_TOKEN_ENV", "invalid development pipe token"))
run_dev = read("scripts/run-dev.ps1")
check("ipc_debug_runner_uses_fresh_private_rendezvous", has(run_dev, "[Guid]::NewGuid().ToString('N').ToLowerInvariant()", "$env:AETHERCORE_DEV_PIPE_TOKEN = $devPipeToken", "--dev-pipe-token", "Remove-Item Env:AETHERCORE_DEV_PIPE_TOKEN"))

# ZR-015 / ZR-018: endpoint squatting is fail-closed at service startup and namespace ownership
# remains continuous across session churn. The successor listener is created before the accepted
# session can be dropped or moved to a worker. If an accept consumes its listener and fails,
# recovery uses FIRST_INSTANCE again rather than joining a potentially foreign namespace.
check("ipc_service_claims_first_pipe_instance_on_startup", has(ipc_windows, "FILE_FLAG_FIRST_PIPE_INSTANCE", "pub struct PipeServerListener", "pub fn claim_first()", "Self::create_with_mode(true)") and has(server, "PipeServerListener::claim_first()"))
check("ipc_service_creates_successor_before_session_handoff", has(ipc_windows, "pub fn create_successor()", "Self::create_with_mode(false)") and ordered(server, "match listener.accept()", "listener = aethercore_ipc::PipeServerListener::create_successor()", "ACTIVE_SESSIONS.fetch_add(1, Ordering::SeqCst)") and ordered(server, "PipeServerListener::create_successor()", 'name("aether-ipc-session"'))
check("ipc_accept_failure_reclaims_namespace_with_first_instance", server.count("PipeServerListener::claim_first()") >= 2 and has(server, "reclaim IPC pipe namespace after accept failure", "had no live pipe object"))
open_pipe_body = section(ipc_windows, "fn open_pipe", "// Count limits")
# ZR-019: connection retry is restricted to documented transient namespace states. Access-denied,
# invalid-access and other open failures are terminal instead of being hidden behind the retry loop.
check("ipc_client_retries_only_absent_or_busy_pipe", has(ipc_windows, "fn retryable_pipe_open_error", "ERROR_FILE_NOT_FOUND", "ERROR_PIPE_BUSY") and has(open_pipe_body, "Err(error) if retryable_pipe_open_error(&error)=>last=Some(error)", "Err(error)=>return Err(IpcError::Io(error))"))
check("ipc_untrusted_server_fails_without_identity_retry", has(open_pipe_body, "Ok(file)=>{", "verify_connected_server(&file, &pipe_name)?;", "return Ok(file)") and not contains(open_pipe_body, "Ok(file)=>match verify_connected_server"))
check("ipc_nonretryable_open_errors_have_regression", has(ipc_windows, "pipe_open_retry_policy_retries_only_absence_or_busy_conditions", "ERROR_FILE_NOT_FOUND.0 as i32", "ERROR_PIPE_BUSY.0 as i32", "assert!(!retryable_pipe_open_error"))

# ZR-020: replace the predictable Global mutex with an installer-provisioned byte-range lock
# inside the protected machine data tree. The runtime never creates the authority file.
check("machine_mutation_lock_uses_protected_programdata_authority", has(windows_foundation, "pub struct MachineMutationGuard", "SHGetKnownFolderPath", "FOLDERID_ProgramData", "machine-mutation.lock", "OPEN_EXISTING", "FILE_FLAG_OPEN_REPARSE_POINT", "LockFileEx", "LOCKFILE_EXCLUSIVE_LOCK", "LOCKFILE_FAIL_IMMEDIATELY", "ERROR_LOCK_VIOLATION", "UnlockFileEx"))
check("machine_mutation_runtime_never_creates_authority_file", "OPEN_ALWAYS" not in section(windows_foundation, "pub struct MachineMutationGuard", "/// Owns a Service Control Manager") and "CreateMutexW" not in windows_foundation and "Global\\AetherCore.WindowsUpdateMutation.v1" not in windows_foundation)
mutation_callers = [startup_windows, cleaner_windows, repair_windows, windows_update, update_broker]
check("machine_mutation_guard_is_centralized_across_all_mutators", all("MachineMutationGuard::try_acquire()" in text for text in mutation_callers) and all("LockFileEx(" not in text for text in mutation_callers))
check("machine_mutation_authority_is_installer_provisioned_and_acl_hardened", has(install_hardener, "MACHINE_MUTATION_LOCK_RELATIVE_PATH", "ensure_mutation_lock_file", "machine-mutation.lock", "let principal = service_principal();", '&["*S-1-5-18:F", "*S-1-5-32-544:F", &format!("{principal}:F")]') and has(installer_verify, "Assert-MutationLockAcl", "Assert-MutationLockContention", "Machine mutation byte-range lock allowed overlapping exclusive ownership", "Unexpected machine mutation authority allow trustee(s)", "ReparsePoint", "AreAccessRulesProtected"))
check("update_broker_holds_machine_lock_before_service_claim", ordered(update_broker, "UpdateMutationGuard::acquire", "claim(&intent_id)"))

# ZR-016: authenticated clients may read/write data but can never create additional server instances.
#
# WHY THE HEX FORM IS FORBIDDEN, so that no later phase re-raises it as drift. Until the P36
# Tranche 1 fix the AU ACE was `(A;;0x00120003;;;AU)`. Windows' SDDL parser SILENTLY DROPS
# SYNCHRONIZE (0x00100000) from a HEX access mask, so the DACL that materialized granted AU
# 0x00020003 - no SYNCHRONIZE - while every client, the shipped desktop app included, opens the
# pipe with PIPE_CLIENT_ACCESS_MASK = 0x00120003, and synchronous I/O requires SYNCHRONIZE. The
# production pipe was unopenable as encoded. `crates/ipc/src/windows_impl.rs:268-288` records the
# A/B live DACL dump that proved it and `:1211` asserts, in the product's own test, that the
# broken encoding must never return.
#
# The cure is an ENCODING change, not a policy change. Named rights survive the parser:
#   FR   = FILE_GENERIC_READ = READ_DATA|READ_EA|READ_ATTRIBUTES|READ_CONTROL|SYNCHRONIZE = 0x120089
#   0x2  = FILE_WRITE_DATA, which FR does not include
#   FR|0x2 materializes as 0x12008B
# 0x12008B COVERS the client's requested 0x120003 (0x120003 & ~0x12008B == 0) and WITHHOLDS
# FILE_CREATE_PIPE_INSTANCE (0x4), FILE_APPEND_DATA and every generic server right - which is the
# whole of what ZR-016 asserts. So the check below is repointed at the named form and, unlike the
# version it replaces, is scoped to the production builder: `pipe_security` also spans
# DEV_PIPE_SECURITY_DESCRIPTOR, whose debug-only `(A;;0x00120003;;;AU)` is what the old token was
# really matching. It passed without ever reading the production descriptor.
production_descriptor = section(ipc_windows, "fn production_pipe_security_descriptor", "fn security_descriptor_for_pipe")
check("ipc_authenticated_users_lack_pipe_create_instance", has(pipe_security, "const PIPE_CLIENT_ACCESS_MASK: u32 = 0x0012_0003;") and has(production_descriptor, "(A;;FR;;;AU)(A;;0x00000002;;;AU)") and "(A;;0x00120003;;;AU)" not in production_descriptor and "(A;;GRGW;;;AU)" not in pipe_security and "(A;;GW;;;AU)" not in pipe_security and "(A;;GA;;;AU)" not in pipe_security)
check("ipc_clients_request_specific_non_generic_access", has(open_pipe_body, "access_mode(PIPE_CLIENT_ACCESS_MASK)", "share_mode(0)") and ".read(true).write(true)" not in open_pipe_body)
check("ipc_pipe_create_instance_regression_test", has(ipc_windows, "authenticated_client_access_cannot_create_pipe_instances", "FILE_CREATE_PIPE_INSTANCE", "EXPECTED_CLIENT_ACCESS: u32 = 0x0012_0003", "assert_eq!(PIPE_CLIENT_ACCESS_MASK, EXPECTED_CLIENT_ACCESS)", "PIPE_CLIENT_ACCESS_MASK & FILE_CREATE_PIPE_INSTANCE", "O:S-1-5-80-1-2-3-4-5D:P(A;;GA;;;S-1-5-80-1-2-3-4-5)(A;;FR;;;AU)(A;;0x00000002;;;AU)", 'assert!(!descriptor.contains("0x00120003"))'))

# ZR-017: named-pipe client opens constrain server impersonation before endpoint authentication.
check("ipc_client_limits_named_pipe_impersonation", has(open_pipe_body, ".security_qos_flags(PIPE_CLIENT_SECURITY_QOS)") and has(pipe_security, "const PIPE_CLIENT_SECURITY_QOS: u32 = 0x0001_0000;") and has(ipc_windows, "named_pipe_client_limits_server_impersonation_to_identification"))
check("ipc_native_probe_verifies_owner_and_kernel_dacl", has(ipc_pipe_probe, "GetSecurityInfo", "SE_FILE_OBJECT", "OWNER_SECURITY_INFORMATION", "OwnerSid", "AuthenticatedUsersAllowMask", "OffendingCreateInstanceSids", "$CreatePipeInstance", "$RequiredClientMask"))
check("ipc_runtime_and_native_probe_pin_same_security_object_type", has(ipc_windows, "GetSecurityInfo(", "SE_FILE_OBJECT", "qualification probe") and has(ipc_pipe_probe, "private const int SE_FILE_OBJECT = 1", "mirrors the Rust runtime endpoint-authentication contract"))
check("ipc_native_probe_uses_exact_production_client_rights", has(ipc_pipe_probe, "PIPE_CLIENT_ACCESS = 0x00120003", "$RequiredClientMask = [uint32]0x00120003") and "PIPE_CLIENT_ACCESS | READ_CONTROL" not in ipc_pipe_probe)
check("ipc_native_probe_requires_service_sid_owner_and_service_identity", has(ipc_pipe_probe, "NT SERVICE", "$serviceSid", "trusted service SID", "Named-pipe owner drift", "$service.StartName", "Own Process"))
check("ipc_native_probe_understands_generic_create_instance_rights", has(ipc_pipe_probe, "GENERIC_ALL = 0x10000000", "GENERIC_WRITE = 0x40000000", "GrantsPipeCreateInstance", "TrustedServiceCanCreatePipeInstance"))
check("ipc_native_probe_rejects_administrator_server_authority", has(ipc_pipe_probe, "AdministratorsAllowMask", "$administratorsAllowMask -ne 0", "Builtin Administrators must not have a named-pipe allow ACE"))
# DBT-P63-002: the probe compared the AU ALLOW MASK against $RequiredClientMask, which is the mask
# a CLIENT ASKS FOR at open time, not what the DACL grants. Since the named-rights encoding the AU
# grant is 0x0012008B. Exactness is kept, against the right constant, plus a coverage assertion -
# a grant that stops covering the client mask is exactly the SYNCHRONIZE regression returning.
check("ipc_native_probe_requires_exact_au_acl", has(ipc_pipe_probe, "$ExpectedAuthenticatedUsersGrant = [uint32]0x0012008B", "$allowMask -ne $ExpectedAuthenticatedUsersGrant", "($allowMask -band $RequiredClientMask) -ne $RequiredClientMask", "$denyMask -ne 0", "Authenticated Users pipe allow mask drift", "Authenticated Users pipe deny mask drift"))
check("ipc_native_probe_requires_protected_expected_dacl", has(ipc_pipe_probe, "ControlFlags.DiscretionaryAclProtected", "UnexpectedAllowSids", "Named-pipe DACL is not protected from inheritance", "Unexpected named-pipe allow ACE trustee(s)"))
check("ipc_native_probe_rejects_unparsed_ace_types", has(ipc_pipe_probe, "CommonAce ace = genericAce as CommonAce", "UnexpectedAceTypes", "Unrecognized named-pipe ACE type(s)"))
check("ipc_native_probe_rejects_all_deny_aces", has(ipc_pipe_probe, "UnexpectedDenySids", "unexpectedDenies.Add", "Unexpected named-pipe deny ACE trustee(s)", "$denyMask -ne 0"))
# Three allow ACEs, two of them AU: FR and 0x2 are separate ACEs. Their masks are unioned before
# the exact check above, so the split hides nothing.
check("ipc_native_probe_requires_exact_allow_ace_cardinality", has(ipc_pipe_probe, "AllowAceCount", "AuthenticatedUsersAllowAceCount", "TrustedServiceAllowAceCount", "$result.AllowAceCount -ne 3", "$result.AuthenticatedUsersAllowAceCount -ne 2", "exactly two AU allow ACEs (FR and 0x2) and one trusted-service allow"))
check("ipc_native_probe_rejects_callback_object_opaque_or_flagged_aces", has(ipc_pipe_probe, "CommonAce ace = genericAce as CommonAce", "ace.AceFlags != AceFlags.None", "ace.IsCallback", "ace.OpaqueLength != 0"))
check("ipc_native_probe_evidence_schema_v4", has(ipc_pipe_probe, "aethercore.ipc-pipe-security.v4", "allow_ace_count", "unexpected_deny_sids"))
check("ipc_native_probe_uses_identification_sqos", has(ipc_pipe_probe, "SECURITY_IDENTIFICATION = 0x00010000", "SECURITY_SQOS_PRESENT = 0x00100000", "PIPE_SECURITY_QOS", "OPEN_EXISTING,", "PIPE_SECURITY_QOS"))
check("ipc_native_probe_is_in_enterprise_runtime_matrix", has(enterprise_stress, "verify-ipc-pipe-security.ps1", "ipc_pipe_security", "Native IPC pipe security verification failed"))
check("maintenance_service_token_probe_reads_live_process_token", has(service_token_probe, "OpenProcessToken", "GetTokenInformation", "TokenGroups", "TokenRestrictedSids", "SE_GROUP_ENABLED_BY_DEFAULT", "SE_GROUP_OWNER", "restricting_sid_count = 0"))
check("maintenance_service_token_probe_requires_unrestricted_sid_policy", has(service_token_probe, "SERVICE_SID_TYPE_UNRESTRICTED", "service_sid_type = 'UNRESTRICTED'", r"NT SERVICE\$ServiceName"))
check("maintenance_service_token_probe_is_in_enterprise_runtime_matrix", has(enterprise_stress, "verify-maintenance-service-token.ps1", "maintenance_service_token", "Native maintenance service token verification failed"))
check("maintenance_service_self_verifies_effective_sid_before_running", has(security_core, "pub fn verify_maintenance_service_token", "CheckTokenMembership(None", "service SID is not enabled in effective token", "TokenRestrictedSids", "maintenance service token contains restricting SIDs") and ordered(service_scm_host, "fn service_main_impl()", "verify_maintenance_service_token(SERVICE_NAME)", "ServiceState::Running"))
check("installer_lifecycle_requires_live_service_token_evidence", has(installer_verify, "verify-maintenance-service-token.ps1", "maintenance-service-token.json", "Maintenance service token verifier produced no evidence artifact"))
check("maintenance_service_token_self_check_is_bounded", has(security_core, "MAX_ACCOUNT_SID_BYTES", "MAX_ACCOUNT_DOMAIN_CHARS", "MAX_TOKEN_GROUP_BUFFER_BYTES", "service SID size outside safety bound", "restricted SID list size outside safety bound"))

# ZR-011: direct press feedback stays immediate and cancellation cannot leak activation.
press_rule = section(motion_css, "[data-fluid-press] {", "}")
check("press_feedback_has_no_fixed_transition", "transition:" not in press_rule)
check("pointer_cancel_fences_click", re.search(r"const cancelPointer[\s\S]*?suppressNextClick = true;[\s\S]*?setArmed\(false\)", press) is not None)
check("pointer_feedback_starts_on_down", has(press, "addEventListener('pointerdown', pointerdown)", "setArmed(true)"))

# Product sanitation and additive release governance.
source_ext = {".rs", ".ts", ".svelte", ".css", ".proto", ".ps1", ".py", ".wxs", ".toml", ".yml", ".yaml"}
violations: list[str] = []
# A placeholder DETECTOR has to name the tokens it detects. Two were exempted; six more
# gates search for the same words and were not, so this check was reporting the alarms
# rather than the placeholders. The list stays explicit rather than becoming a glob over
# `scripts/*audit*.py`: a new gate has to be added here deliberately, and a real `TODO` in
# one of these files is still worth failing on if it is ever the point. `DBT-P61-001`.
AUDIT_SOURCES = {
    "scripts/zenith-recursive-audit.py",
    "scripts/enterprise-adversarial-audit.py",
    "scripts/phase21-adversarial-audit.py",
    "scripts/phase18-driver-authority-audit.py",
    "scripts/phase18_1-driver-truth-audit.py",
    "scripts/phase19-windows-repair-audit.py",
}
# `node_modules`, `target` and `dist` are fetched or generated, not delivered: 8 of the 13
# hits were upstream `.d.ts` files. `source_seal.py` draws the same line at git-tracked.
GENERATED_PARTS = {"out", "node_modules", "target", "dist", ".git", "__pycache__"}
for path in ROOT.rglob("*"):
    rel = path.relative_to(ROOT).as_posix() if path.is_file() else ""
    if (
        not path.is_file()
        or path.suffix.lower() not in source_ext
        or GENERATED_PARTS.intersection(path.parts)
        or rel in AUDIT_SOURCES
        or any(rel.endswith("/" + name) for name in AUDIT_SOURCES)
    ):
        continue
    text = path.read_text(encoding="utf-8", errors="ignore")
    if re.search(r"\b(?:TODO|FIXME|HACK|XXX)\b|\btodo!\s*\(|\bunimplemented!\s*\(", text):
        violations.append(path.relative_to(ROOT).as_posix())
check("source_has_no_placeholders", not violations, files=violations[:20])

# Static-gate helpers must themselves fail closed when delimiters/tokens disappear.
check("audit_ordering_helper_fails_closed", not ordered("second", "missing", "second") and ordered("first second", "first", "second"))
check("audit_section_helper_fails_closed", section("alpha omega", "missing", "omega") == "" and section("alpha omega", "alpha", "omega") == "alpha ")

# Meta-regression: reject accidental patch corruption that can fool token-only gates.
# This now covers duplicate top-level methods inside each impl, including private methods.
malformed_rust: list[str] = []
duplicate_methods: list[str] = []
for path in ROOT.rglob("*.rs"):
    if "out" in path.parts:
        continue
    text = path.read_text(encoding="utf-8", errors="ignore")
    rel = path.relative_to(ROOT).as_posix()
    if re.search(r"\bpub\s+(?:async\s+)?fn\b[^\n{};]*\bpub\s+(?:async\s+)?fn\b", text):
        malformed_rust.append(rel)
    duplicate_methods.extend(f"{rel}:{item}" for item in duplicate_impl_methods(text))
check("rust_source_rejects_repeated_public_function_declarations", not malformed_rust, files=malformed_rust[:20])
check("rust_impls_reject_duplicate_method_names", not duplicate_methods, methods=duplicate_methods[:20])
check("diagnostic_refresh_signature_is_singular", diagnostics.count("pub fn passive_hardware_refresh") == 1)
check("recursive_gate_in_verify_zenith", "zenith-recursive-audit.ps1" in verify_zenith)
check("recursive_gate_in_ci", "zenith-recursive-audit.ps1" in ci)
check("recursive_gate_before_release_packaging", ordered(release, "zenith-recursive-audit.ps1", "verify-enterprise.ps1 -ReleasePackaging"))
check("recursive_gate_in_ga_seal", "zenith-recursive-audit.ps1" in production)

failed = [name for name, value in checks.items() if not value.get("ok")]
report = {
    "schema": "aethercore.zenith-recursive-audit.v1",
    "ok": not failed,
    "check_count": len(checks),
    "failed": failed,
    "checks": checks,
    "qualification_boundary": [
        "This is a platform-neutral structural regression gate for the recursive hardening pass.",
        "Windows Rust compilation, Win32/COM/WMI/SCM/WebView2 behavior, WiX lifecycle, Authenticode, signing, fault injection, stress and soak remain native qualification requirements.",
        "Synchronous named-pipe pumps now request cross-thread CancelSynchronousIo on teardown/backpressure/disconnect; Windows slow-peer fault injection must prove cancellation timing and decide whether overlapped I/O is still required.",
    ],
}
if ARGS.output:
    ARGS.output.parent.mkdir(parents=True, exist_ok=True)
    ARGS.output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
print(json.dumps({"ok": report["ok"], "checks": len(checks), "failed": failed}, indent=2))
sys.exit(0 if report["ok"] else 1)
