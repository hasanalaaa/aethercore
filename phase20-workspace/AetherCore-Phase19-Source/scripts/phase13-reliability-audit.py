#!/usr/bin/env python3
from pathlib import Path
import argparse, json, sys

ROOT = Path(__file__).resolve().parents[1]
PARSER = argparse.ArgumentParser()
PARSER.add_argument("--output", type=Path)
ARGS = PARSER.parse_args()
checks = {}

def read(rel):
    p = ROOT / rel
    return p.read_text(encoding='utf-8') if p.exists() else ''

def check(name, ok, **detail):
    checks[name] = {'ok': bool(ok), **detail}

def has(text, *tokens):
    return all(token in text for token in tokens)

runtime = read('crates/collector-runtime/src/lib.rs')
hardware = read('crates/hardware-telemetry/src/lib.rs')
hardware_win = read('crates/hardware-telemetry/src/windows_impl.rs')
crash = read('crates/crash-diagnostics/src/lib.rs')
crash_win = read('crates/crash-diagnostics/src/windows_impl.rs')
diag = read('crates/diagnostic-engine/src/lib.rs')
proto = read('crates/contracts/proto/diagnostics.proto')
protocol = read('services/maintenance-service/src/protocol.rs')
contracts = read('apps/ui/src/lib/contracts.ts')
stream = read('apps/ui/src/platform/stream-state.ts')
fault_ui = read('apps/ui/src/features/diagnostics/ProviderFaultsPanel.svelte')
hardware_ui = read('apps/ui/src/features/diagnostics/HardwarePage.svelte')
crash_ui = read('apps/ui/src/features/diagnostics/CrashPage.svelte')
semantic = read('apps/ui/src/lib/i18n/semantic.ts')
en = read('apps/ui/src/lib/i18n/catalog.en.ts')
ar = read('apps/ui/src/lib/i18n/catalog.ar.ts')
restore = read('crates/restore-point/src/windows_impl.rs')
verify = read('scripts/verify-phase13.ps1')
fault_ps = read('scripts/phase13-fault-injection.ps1')
ci = read('.github/workflows/ci.yml')
release = read('.github/workflows/release.yml')
docs = read('docs/ENGINE_RELIABILITY.md') + '\n' + read('docs/adr/0015-collector-runtime-and-structured-event-rendering.md')

required = [
    'crates/collector-runtime/Cargo.toml', 'crates/collector-runtime/src/lib.rs',
    'scripts/phase13-reliability-audit.py', 'scripts/phase13-reliability-audit.ps1',
    'scripts/phase13-fault-injection.ps1', 'scripts/verify-phase13.ps1',
    'docs/ENGINE_RELIABILITY.md',
    'docs/adr/0015-collector-runtime-and-structured-event-rendering.md',
    'PHASE_13_DELIVERABLES.md', 'PHASE_13_VALIDATION_SUMMARY.md',
    'apps/ui/src/features/diagnostics/ProviderFaultsPanel.svelte',
]
check('required_artifacts', all((ROOT/p).is_file() for p in required), missing=[p for p in required if not (ROOT/p).is_file()])

# Shared runtime / cancellation / isolation.
check('collector_runtime_deadlines', has(runtime, 'CollectorControl', 'CancellationToken', 'remaining_ms_capped', 'DEFAULT_COLLECTOR_TIMEOUT'))
check('hierarchical_cancellation', has(runtime, 'struct CancellationNode', 'parent: Option<Arc<CancellationNode>>', 'pub fn child(&self)', 'current = node.parent.as_deref()', 'parent_cancellation_propagates_to_children_without_reverse_poisoning'))
check('external_cancellation_interrupts_watchdog', has(runtime, 'run_isolated_gated_with_token', 'external_cancellation_interrupts_supervisor_before_watchdog_deadline', 'FaultKind::Cancelled'))
check('collector_watchdog_isolation', has(runtime, 'run_isolated_gated', 'IsolationGate', 'catch_unwind', 'recv_timeout', 'previous collector invocation is still active'))
check('fault_taxonomy', has(runtime, 'Timeout', 'Cancelled', 'Unavailable', 'PermissionDenied', 'MalformedResponse', 'ProviderFailure', 'Io', 'Internal'))
check('fault_detail_bounded', has(runtime, 'MAX_FAULT_DETAIL_BYTES', 'bounded_detail', 'CollectorFaultRecord::new', 'is_char_boundary', 'fault_detail_is_utf8_bounded_before_crossing_process_boundaries'))
check('watchdog_fault_injection_tests', has(runtime, 'watchdog_cancels_and_returns_timeout', 'timed_out_provider_remains_quarantined_until_worker_exits', 'successful_gated_provider_releases_before_result_is_visible', 'panic_is_contained_as_internal_fault'))
check('lease_release_precedes_result_publication', runtime.find('drop(lease);') < runtime.find('tx.send(result)') and 'successful_gated_provider_releases_before_result_is_visible' in runtime)

# WMI discipline.
check('no_unbounded_wmi_in_collectors', 'WBEM_INFINITE' not in hardware_win and 'WBEM_INFINITE' not in restore)
check('wmi_finite_next', has(hardware_win, 'remaining_ms_capped(WMI_NEXT_SLICE)', '.Next(timeout_ms', 'Ok(out)'))
check('wmi_semantic_status_handling', has(hardware_win, 'WBEM_S_TIMEDOUT', 'WBEM_S_FALSE', 'status.is_err()', 'terminal WBEM_S_FALSE with a non-empty result', 'returned == 0'))
check('wmi_provider_object_cap', 'bounded 256-object storage inventory' in hardware_win)
check('wmi_return_count_consistency', has(hardware_win, 'fn take_wmi_object(', 'returned > 1', 'reported one returned object but supplied no object', 'populated an object while reporting zero returned objects'))
check('wmi_permission_denied_classification', has(hardware_win, 'E_ACCESSDENIED', 'WBEM_E_ACCESS_DENIED', 'TelemetryError::PermissionDenied', 'FaultKind::PermissionDenied'))
check('restore_point_wmi_finite_status', has(restore, 'WBEM_S_TIMEDOUT', 'WBEM_S_FALSE', '.Next(5_000'))
check('restore_point_wmi_return_consistency', has(restore, 'terminal WBEM_S_FALSE with a non-empty result', 'reported more objects than the supplied output slice', 'reported an object without supplying one', 'populated an object while reporting zero returned objects'))

# Storage / vendor boundary.
check('storage_outer_watchdog', has(hardware_win, 'STORAGE_GATE', 'run_isolated_gated_with_token(', 'DEFAULT_COLLECTOR_TIMEOUT', 'parent.child()'))
check('direct_ioctl_watchdog', has(hardware_win, 'DIRECT_IOCTL_GATE', 'STORAGE_IOCTL_TIMEOUT', 'nvme-health-ioctl', 'ata-smart-ioctl', 'run_isolated_gated_with_token'))
check('memory_isolation', has(hardware_win, 'MEMORY_GATE', 'memory_gate()', 'parent.child()'))
check('memory_unavailable_is_optional', 'pub memory:Option<MemoryTelemetry>' in diag and 'memory:None' in diag and 'v.memory.map(|memory|' in protocol)
check('nvme_byte_parser', has(hardware, 'parse_nvme_health_log', 'REQUIRED: usize = 192', 'u128::from_le_bytes'))
check('nvme_protocol_window_bounds', has(hardware, 'checked_protocol_window', 'checked_add', 'escaped the bytes returned by the driver'))
check('nvme_protocol_header_overlap_rejected', has(hardware, 'protocol payload overlapped the protocol-specific metadata header', 'minimum_data_offset'))
check('ata_driver_response_bounds', has(hardware, 'parse_ata_driver_response', 'inconsistent cBufferSize', 'reported more bytes than the supplied output buffer'))
check('storage_malformed_regressions', has(hardware, 'nvme_parser_rejects_truncated_vendor_response', 'protocol_window_rejects_overflow_and_truncation', 'ata_driver_response_rejects_forged_returned_length', 'ata_driver_response_rejects_inconsistent_declared_payload'))
check('live_hardware_probe_exists', has(hardware, '#[ignore =', 'live_storage_and_memory_collection_is_read_only'))

# Event Log / WHEA / minidumps.
check('no_event_xml_scraping', all(x not in crash_win for x in ['EvtRenderEventXml','xml_tag(','xml_attr(','<EventID>','SystemTime=']))
check('structured_event_render', has(crash_win, 'EvtCreateRenderContext', 'EvtRenderContextSystem', 'EvtRenderContextUser', 'EvtRenderEventValues', 'EVT_VARIANT'))
check('event_render_caps', has(crash_win, 'MAX_SYSTEM_RENDER_BYTES', 'MAX_USER_RENDER_BYTES', 'MAX_EVENT_PROPERTIES', 'MAX_EVENT_STRING_UTF16'))
check('event_property_count_rejected_before_allocation', has(crash_win, 'fn checked_render_property_count(', 'let property_count = checked_render_property_count(properties)?;', 'let actual_property_count = checked_render_property_count(actual_properties)?;', 'render_property_count_is_rejected_before_allocation_when_pathological'))
check('event_permission_denied_classification', has(crash_win, 'E_ACCESSDENIED', 'CrashError::PermissionDenied', 'std::io::ErrorKind::PermissionDenied', 'FaultKind::PermissionDenied', 'fn io_fault_kind('))
check('event_pointer_containment', has(crash_win, 'string pointer escaped its render buffer', 'not null terminated inside its bounded render buffer'))
check('event_alignment_validation', has(crash_win, 'align_of::<EVT_VARIANT>()', 'align_of::<u16>()', 'UTF-16 string pointer was misaligned'))
check('event_handle_count_bounds', has(crash_win, 'EvtNext reported more event handles than the bounded output array', 'Take ownership of every non-null handle', 'EvtNext returned null, sparse, or trailing handles inconsistent with its reported count', 'EvtNext succeeded without returning an event handle'))
check('event_finite_next', has(crash_win, 'remaining_ms_capped(EVENTLOG_NEXT_SLICE)', 'ERROR_TIMEOUT'))
check('event_malformed_partial_fault', has(crash_win, 'malformed_events', 'FaultKind::MalformedResponse', 'eventlog.render'))
check('event_classification_structured_payload', 'payload_values: &[String]' in crash and 'structured_payload_classification_does_not_require_xml' in crash)
check('minidump_bounds', has(crash_win, 'MAX_DUMPS', 'take(header_bytes as u64)', 'header_data.len() >= size_of::<DUMP_HEADER64>()', 'header_data.len() >= size_of::<DUMP_HEADER32>()'))
check('minidump_partial_faults', has(crash_win, 'minidump.enumerate', 'minidump.metadata', 'minidump.parse', 'ProviderFailure', 'CollectorFaultRecord::new'))
check('live_crash_probe_exists', has(crash, '#[ignore =', 'live_event_and_minidump_collection_is_read_only'))

# Engine fanout and fault preservation.
check('provider_fanout_isolation', has(diag, 'hardware_worker = thread::Builder::new()', 'crash_worker = thread::Builder::new()', 'hardware-provider', 'crash-provider', 'hardware_gate:IsolationGate', 'crash_gate:IsolationGate', 'run_isolated_gated'))
check('provider_supervisor_spawn_and_panic_are_contained', has(diag, 'fn join_provider<T>(', 'failed to spawn provider supervisor thread', 'provider supervisor thread terminated unexpectedly', 'provider_supervisor_panic_is_classified'))
check('top_level_scan_panic_cannot_leave_collecting', has(diag, 'catch_unwind(AssertUnwindSafe(|| run(worker_inner,owner)))', 'mark_scan_runtime_failure(', 'scan_runtime_failure_marks_collecting_snapshot_failed'))
check('provider_control_propagation', has(diag, 'fn hardware(&self, control: CollectorControl)', 'fn crashes(&self, control: CollectorControl)', 'collect_with_cancellation(control.cancellation())'))
check('provider_panic_containment', 'provider_panic_is_contained_and_persisted_as_typed_fault' in diag)
check('fault_records_share_bounded_constructor', 'CollectorFaultRecord {' not in crash_win and has(runtime, 'impl CollectorFaultRecord', 'Self::new(fault.provider') and 'ProviderFaultRecord{provider:"diagnostic-engine"' not in diag and 'ProviderFaultRecord{provider:"diagnostic-journal"' not in diag)
check('nested_provider_faults_preserved', 'nested_provider_faults_are_preserved_in_the_diagnostic_snapshot' in diag and 'provider_faults.extend(h.provider_faults' in diag and 'provider_faults.extend(c.provider_faults' in diag)
check('diagnostic_persistence_failure_is_visible', 'let _=inner.db.save_diagnostic_snapshot' not in diag and 'diagnostic-journal' in diag and 'snapshot.persist' in diag and 'Diagnostic history persistence was unavailable' in diag)

# Contract and localized UI exposure. Raw details stay out of normal UI.
check('provider_fault_contract', has(proto, 'enum ProviderFaultKind', 'message ProviderFaultInfo', 'repeated ProviderFaultInfo provider_faults = 13'))
check('provider_fault_service_mapping', has(protocol, 'provider_faults:v.provider_faults', 'provider_fault_kind_code', 'v1::ProviderFaultInfo'))
check('provider_fault_ui_contract', has(contracts, 'export type ProviderFault', 'providerFaults:ProviderFault[]') and 'providerFaults: []' in stream)
check('provider_fault_localized_ui', has(fault_ui, 'localizeProviderFaultKind', '<TechnicalText value={fault.provider}', '<TechnicalText value={fault.operation}') and 'ProviderFaultsPanel' in hardware_ui and 'ProviderFaultsPanel' in crash_ui)
check('provider_fault_raw_detail_not_rendered', 'fault.detail' not in fault_ui and '{fault.detail}' not in (hardware_ui + crash_ui))
check('provider_fault_catalog_parity_surface', has(en, 'diagnostics.providerFaults.title', 'diagnostics.providerFaults.kind.timeout', 'diagnostics.providerFaults.kind.malformedResponse') and has(ar, 'diagnostics.providerFaults.title', 'diagnostics.providerFaults.kind.timeout', 'diagnostics.providerFaults.kind.malformedResponse') and 'localizeProviderFaultKind' in semantic)

# Verification / CI integration.
check('fault_injection_windows_gate', has(fault_ps, 'aethercore-collector-runtime', 'aethercore-hardware-telemetry', 'aethercore-crash-diagnostics', 'aethercore-diagnostic-engine', '$LiveReadOnly', 'parent_cancellation_propagates_to_children_without_reverse_poisoning', 'external_cancellation_interrupts_supervisor_before_watchdog_deadline', 'provider_supervisor_panic_is_classified', 'scan_runtime_failure_marks_collecting_snapshot_failed'))
check('fault_injection_covers_event_preallocation_and_vendor_tail', has(fault_ps, 'render_property_count_is_rejected_before_allocation_when_pathological', 'nvme_parser_accepts_vendor_tail_without_reading_past_standard_prefix'))
check('phase13_windows_gate', has(verify, 'verify-phase12.ps1', 'phase13-reliability-audit.ps1', 'phase13-fault-injection.ps1', 'cargo check --workspace --locked', 'aethercore-collector-runtime -p aethercore-hardware-telemetry'))
check('release_packaging_deferred_until_reliability', verify.find('build-release.ps1') > verify.find('phase13-fault-injection.ps1') > verify.find('verify-phase12.ps1'))
check('phase13_ci_release_gate', any(g in ci for g in ['verify-phase13.ps1 -SkipOnlineSupplyChain','verify-phase14.ps1 -SkipOnlineSupplyChain','verify-phase15.ps1 -SkipOnlineSupplyChain','verify-phase16.ps1 -SkipOnlineSupplyChain','verify-enterprise.ps1 -SkipOnlineSupplyChain']) and any(g in release for g in ['verify-phase13.ps1 -ReleasePackaging -RequireSigning','verify-phase14.ps1 -ReleasePackaging -RequireSigning','verify-phase15.ps1 -ReleasePackaging -RequireSigning','verify-phase16.ps1 -ReleasePackaging -RequireSigning','verify-enterprise.ps1 -ReleasePackaging -RequireSigning']))
check('documented_reliability_contract', has(docs, 'hierarchical', 'IsolationGate', 'WBEM_S_TIMEDOUT', 'EvtRenderContextSystem', 'MalformedResponse', 'ProviderFault'))

ok = all(v['ok'] for v in checks.values())
report = {'phase': 13, 'ok': ok, 'check_count': len(checks), 'checks': checks}
failed = [k for k,v in checks.items() if not v['ok']]
print(json.dumps({'ok': ok, 'checks': len(checks), 'failed': failed}, indent=2))
if ARGS.output:
    ARGS.output.parent.mkdir(parents=True, exist_ok=True)
    ARGS.output.write_text(json.dumps(report, indent=2, sort_keys=True)+'\n', encoding='utf-8')
sys.exit(0 if ok else 1)
