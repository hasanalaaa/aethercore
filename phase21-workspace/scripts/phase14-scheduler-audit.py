#!/usr/bin/env python3
from pathlib import Path
import argparse, json, re, sys
ROOT=Path(__file__).resolve().parents[1]
PARSER=argparse.ArgumentParser();PARSER.add_argument("--output",type=Path);ARGS=PARSER.parse_args()
checks={}
# P59 / DBT-P58-005: one shared reader that raises instead of substituting "".
# P58 fixed where this reader looked - `.github/` resolves against the
# repository root - but not what it did when the look failed, so a check
# asserting something is ABSENT still passed against a file never opened.
# No bytecode: `omega-evidence.py` runs each gate against a disposable clone
# and treats ANY new file in it as a source mutation, so a `__pycache__`
# entry for this import would be reported as the gate rewriting the tree.
sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
from gate_reader import SourceReader, contains  # noqa: E402

read = SourceReader(ROOT).read
def check(name, ok, **detail): checks[name]={'ok':bool(ok),**detail}
# `contains`, not `in`: whitespace-insensitive, defined once. `DBT-P61-001`.
def has(text,*tokens): return all(contains(text,t) for t in tokens)

model=read('crates/idle-scheduler/src/model.rs')
policy=read('crates/idle-scheduler/src/policy.rs')
runtime=read('crates/idle-scheduler/src/runtime.rs')
resource=read('crates/idle-scheduler/src/resource.rs')
win=read('crates/idle-scheduler/src/windows_state.rs')
foundation=read('crates/windows-foundation/src/lib.rs')
service=read('services/maintenance-service/src/scheduler.rs')
main=read('services/maintenance-service/src/main.rs')
kernel_mut=read('crates/operation-kernel/src/mutation.rs')
kernel_budget=read('crates/operation-kernel/src/work_budget.rs')
collector=read('crates/collector-runtime/src/lib.rs')
driver=read('crates/driver-hub/src/lib.rs')
cleaner=read('crates/cleaner/src/lib.rs')
cleaner_win=read('crates/cleaner/src/windows_impl.rs')
startup=read('crates/startup-manager/src/lib.rs')
diag=read('crates/diagnostic-engine/src/lib.rs')
events=read('crates/contracts/proto/events.proto')
scheduler_proto=read('crates/contracts/proto/scheduler.proto')
desktop=read('apps/desktop/src/main.rs')
contracts=read('apps/ui/src/lib/contracts.ts')
stream=read('apps/ui/src/platform/stream-state.ts')
verify=read('scripts/verify-phase14.ps1')
ci=read('.github/workflows/ci.yml')
release=read('.github/workflows/release.yml')
docs=read('docs/AUTONOMOUS_MAINTENANCE.md')+'\n'+read('docs/adr/0016-autonomous-idle-scheduler.md')
persistence=read('crates/persistence/src/lib.rs')
migration=read('crates/persistence/migrations/0008_phase14_scheduler.sql')
kernel=read('crates/operation-kernel/src/lib.rs')
activity=read('apps/ui/src/features/activity/ActivityPage.svelte')
fault14=read('scripts/phase14-scheduler-fault-injection.ps1')

required=[
 'crates/idle-scheduler/Cargo.toml','crates/idle-scheduler/src/model.rs','crates/idle-scheduler/src/policy.rs',
 'crates/idle-scheduler/src/resource.rs','crates/idle-scheduler/src/runtime.rs','crates/idle-scheduler/src/windows_state.rs',
 'crates/contracts/proto/scheduler.proto','services/maintenance-service/src/scheduler.rs',
 'scripts/phase14-scheduler-audit.py','scripts/phase14-scheduler-audit.ps1','scripts/phase14-scheduler-tests.ps1','scripts/phase14-scheduler-fault-injection.ps1','scripts/verify-phase14.ps1',
 'docs/AUTONOMOUS_MAINTENANCE.md','docs/adr/0016-autonomous-idle-scheduler.md','PHASE_14_DELIVERABLES.md','PHASE_14_VALIDATION_SUMMARY.md',
]
check('required_artifacts',all((ROOT/p).is_file() for p in required),missing=[p for p in required if not (ROOT/p).is_file()])

# Closed read-only policy.
variants=re.findall(r'^\s{4}([A-Za-z][A-Za-z0-9_]*)\s*,\s*$', model.split('pub enum AutonomousWorkload',1)[1].split('}',1)[0], re.M)
check('closed_workload_enum_exactly_five', variants==['HardwareTelemetry','DriverDiscovery','CleanupInventory','StartupInventory','EventLogTriage'], variants=variants)
check('no_mutation_workload_representation', all(x not in variants for x in ['DriverInstall','SystemRepair','Cleanup','Startup','Update','Delete','Repair']))
for forbidden in ['start_driver_install','start_system_repair','start_cleanup(','start_startup_changes','create_driver_install_plan','create_cleanup_plan','create_startup_plan','consume_authorization','MutationWorkload::']:
    check('service_executor_forbids_'+re.sub(r'\W+','_',forbidden).strip('_'), forbidden not in service)
check('scheduler_executor_only_passive_entrypoints',has(service,'passive_hardware_refresh','passive_event_log_refresh','passive_scan_with_fence'))
check('cleanup_passive_scope_narrower',has(cleaner,'fn scan_passive','requires_explicit_confirmation') and has(cleaner_win,'scan_impl(false)','retain(|candidate| !candidate.requires_explicit_confirmation)'))

# Ownership / eligibility / active-user state.
check('active_console_session_principal',has(win,'WTSGetActiveConsoleSessionId','WTSQueryUserToken','inspect_session_token(token.get(), session_id)','principal.binding_key()'))
check('idle_uses_wts_session_time_not_session0_lastinput',has(win,'WTSQuerySessionInformationW','WTSSessionInfoEx','LastInputTime','CurrentTime') and 'GetLastInputInfo' not in win)
check('power_gate',has(win,'GetSystemPowerStatus','ACLineStatus == 1','SystemStatusFlag != 0'))
check('presentation_gate_impersonates_active_user',has(win,'ImpersonateLoggedOnUser','SHQueryUserNotificationState','RevertToSelf','security-fatal: RevertToSelf'))
check('presentation_unknown_fail_closed',has(policy,'matches!(state.presentation, PresentationState::Unknown) { blocked.push(BlockReason::PresentationUnknown); }'))
check('servicing_gate',has(win,'TrustedInstaller','UsoSvc','WaaSMedicSvc','QueryServiceStatusEx','ServicingState::Unknown'))
check('servicing_unknown_fail_closed',has(policy,'matches!(state.servicing, ServicingState::Unknown) { blocked.push(BlockReason::ServicingUnknown); }'))
check('network_cost_gate',has(win,'INetworkCostManager','GetCost(&mut cost, ptr::null())','NLM_CONNECTION_COST_FIXED','NetworkCost::Metered'))
check('metered_unknown_network_blocks_driver_discovery',has(policy,'NetworkCost::Metered','NetworkCost::Unknown','NetworkCostUnknown') and 'network_sensitive' in model)
check('thermal_acpi_probe',has(win,'MSAcpi_ThermalZoneTemperature','CurrentTemperature','CriticalTripPoint','THERMAL_WMI_DEADLINE','WBEM_S_TIMEDOUT','MAX_THERMAL_ZONES'))
check('thermal_unknown_not_fabricated',has(win,'unwrap_or(ThermalPressure::Unknown)','return ThermalPressure::Unknown') and has(policy,'ThermalPressureUnknown'))
check('thermal_uses_trip_point_margin',has(win,'classify_thermal_zone','margin <= 50','margin <= 150','thermal_zone_uses_trip_point_margin_without_invented_absolute_thresholds'))

# Preemption / commit linearization.
check('fast_preemption_probe',has(runtime,'sample_fast','active_probe_interval','full_recheck_interval') and 'Duration::from_millis(100)' in model)
check('preemption_covers_full_policy',has(runtime,'preemption_required(','kernel_monitor.mutations().is_active()','monitor_fence.revoke()','monitor_token.cancel()'))
check('probe_failure_preempts', 'Err(_) => true' in runtime)
check('preemption_monitor_spawn_fail_closed',has(runtime,'preemptionMonitorUnavailable','commit_fence.revoke()','idle preemption monitor failed to start'))
# rustfmt puts one variant per line and adds the trailing comma. `DBT-P61-001`.
check('commit_fence_three_state',has(collector,'CommitFenceState { Active, Committed, Revoked, }','try_commit_checked','is_committed'))
check('commit_vs_preemption_linearized',has(collector,'if *state == CommitFenceState::Active { *state = CommitFenceState::Revoked; }','*state = CommitFenceState::Committed') and has(runtime,'let committed = commit_fence.is_committed()'))
check('late_publication_rejected_in_domains',all('try_commit_checked' in text for text in [driver,cleaner,startup,diag]))
check('passive_domains_recheck_cancel_at_commit',all('token.is_cancelled()' in text for text in [driver,cleaner,startup,diag]))
check('passive_scans_no_intermediate_scanning_publish', 'pub fn passive_scan_with_fence' in driver and 'No intermediate `Scanning` state' in driver)
check('monotonic_passive_inventory_epochs', has(driver,'inventory_epoch.saturating_add(1)') and has(cleaner,'inventory_epoch.saturating_add(1).max') and has(startup,'inventory_epoch.saturating_add(1).max'))

# Kernel/resource integration.
check('mutation_supervisor_boolean_gate',has(kernel_mut,'pub fn is_active(&self) -> bool'))
check('scheduler_blocks_when_mutation_active',has(runtime,'kernel.mutations().is_active()') and has(policy,'MutationActive'))
check('read_budget_acquired_per_workload',has(runtime,'kernel.reads().try_acquire(map_read(workload))') and has(kernel_budget,'same_expensive_read_workload_is_single_flight'))
check('scheduler_single_flight_outer_loop', 'ran = true;' in runtime and 'break;' in runtime)
check('isolation_gate_per_workload',has(runtime,'HashMap::<AutonomousWorkload, IsolationGate>','run_isolated_gated_with_token'))
check('windows_background_thread_mode',has(win,'BackgroundThreadMode::enter') and has(foundation,'THREAD_MODE_BACKGROUND_BEGIN','THREAD_MODE_BACKGROUND_END','SetThreadPriority'))
check('cooperative_cpu_io_governor',has(resource,'cpu_budget_per_second','io_budget_per_second','account_cpu_cancellable','account_io_cancellable') and has(service,'account_cpu_cancellable','account_io_cancellable'))
check('jitter_first_and_repeat_runs',runtime.count('random_jitter_ms(config.max_jitter)')>=2)
check('equal_jitter_exponential_backoff',has(runtime,'Equal-jitter exponential backoff','backoff_ceiling_ms','backoff_delay_ms','random_range'))
check('durable_principal_scoped_cadence',has(migration,'autonomous_scheduler_runs','owner_principal_key','PRIMARY KEY(owner_principal_key, workload)') and has(persistence,'scheduler_cadence_is_durable_and_principal_scoped') and has(kernel,'scheduler_cadence','save_scheduler_cadence') and has(runtime,'persist_cadence'))
check('cadence_ledger_carries_no_mutation_authority',all(token not in migration.lower() for token in ['command_path','consent_token','authorization_grant','mutation_intent']))
check('cadence_failure_disables_autonomous_only',has(runtime,'cadence ledger unavailable; autonomous work fails closed','cadence write failed; autonomous work disabled') and has(main,'interactive maintenance remains available'))

# Event stream / UI integration.
check('typed_scheduler_proto',has(scheduler_proto,'enum SchedulerRunState','message SchedulerEvent','evidence_count','warning_count'))
check('scheduler_event_in_event_bus',has(events,'EVENT_KIND_SCHEDULER = 18','SchedulerEvent scheduler = 27') and has(runtime,'EventKind::Scheduler','Payload::Scheduler'))
check('domain_snapshots_stream_after_passive_commit',has(service,'EventKind::Diagnostics','EventKind::DriverDiscovery','EventKind::CleanupDiscovery','EventKind::StartupDiscovery'))
check('desktop_normalizes_scheduler_event','Payload::Scheduler(v)' in desktop and '"scheduler"' in desktop)
check('ui_tracks_scheduler_event',has(contracts,'export type SchedulerEvent') and has(stream,'schedulerEvent: SchedulerEvent | null',"case 'scheduler'"))
check('activity_surfaces_scheduler_observability',has(activity,'schedulerEvent','activity.schedulerTitle','scheduler.reason.','scheduler.workload.'))

# Lifecycle / tests / docs.
check('scheduler_start_failure_isolated_from_interactive_service',has(runtime,'SchedulerStartError','failed to spawn idle scheduler') and has(main,'scheduler::start(&context)','interactive maintenance remains available'))
check('scheduler_shutdown_join',has(runtime,'impl Drop for SchedulerHandle','worker.join()'))
check('policy_tests',has(policy,'mutations_block_every_autonomous_workload','user_activity_preempts_all_workloads','metered_and_unknown_network_only_block_network_sensitive_discovery','unknown_presentation_and_servicing_fail_closed'))
check('commit_fence_tests',has(collector,'revoked_commit_fence_rejects_late_publication','commit_and_revoke_are_linearized_by_one_boundary','checked_commit_can_decline_without_claiming_publication'))
check('thermal_tests',has(win,'thermal_zone_uses_trip_point_margin_without_invented_absolute_thresholds','worst_thermal_zone_wins'))
check('documented_zero_mutation_contract',('Zero background mutations' in docs and has(docs,'active console session','CommitFence','ReadBudgetManager','THREAD_MODE_BACKGROUND_BEGIN','Unknown')))
check('phase14_windows_gate',has(verify,'verify-phase13.ps1','phase14-scheduler-audit.ps1','phase14-scheduler-tests.ps1','phase14-scheduler-fault-injection.ps1','cargo check --workspace --locked') and 'aethercore-idle-scheduler' in read('scripts/phase14-scheduler-tests.ps1'))
check('phase14_fault_injection_gate',has(fault14,'revoked_commit_fence_rejects_late_publication','preemption_is_fail_closed_for_activity_owner_session_and_mutation_changes','cancellation_interrupts_governor_wait','scheduler_cadence_is_durable_and_principal_scoped','live_read_only_system_state_probe'))
check('phase14_ci_release_gate',any(g in ci for g in ['verify-phase14.ps1 -SkipOnlineSupplyChain','verify-phase15.ps1 -SkipOnlineSupplyChain','verify-phase16.ps1 -SkipOnlineSupplyChain','verify-enterprise.ps1 -SkipOnlineSupplyChain']) and any(g in release for g in ['verify-phase14.ps1 -ReleasePackaging -RequireSigning','verify-phase15.ps1 -ReleasePackaging -RequireSigning','verify-phase16.ps1 -ReleasePackaging -RequireSigning','verify-enterprise.ps1 -ReleasePackaging -RequireSigning']))

ok=all(v['ok'] for v in checks.values())
failed=[k for k,v in checks.items() if not v['ok']]
report={'phase':14,'ok':ok,'check_count':len(checks),'checks':checks}
if ARGS.output:
    ARGS.output.parent.mkdir(parents=True,exist_ok=True)
    ARGS.output.write_text(json.dumps(report,indent=2,sort_keys=True)+'\n',encoding='utf-8')
print(json.dumps({'ok':ok,'checks':len(checks),'failed':failed},indent=2))
sys.exit(0 if ok else 1)
