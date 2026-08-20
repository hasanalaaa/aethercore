use std::{
    collections::{hash_map::Entry, HashMap},
    sync::{atomic::{AtomicBool, Ordering}, Arc},
    thread,
    time::{Duration, Instant},
};

use aethercore_collector_runtime::{
    run_isolated_gated_with_token, CancellationToken, CollectorFault, CommitFence, FaultKind,
    IsolationGate,
};
use aethercore_contracts::v1::{self, event_envelope, EventKind, SchedulerRunState};
use aethercore_operation_kernel::{OperationKernel, ReadWorkload};
use chrono::Utc;
use rand::Rng;
use tracing::{info, warn};

use crate::{model::*, policy::EligibilityEngine, resource::ResourceGovernor};

pub trait SystemStateProbe: Send + Sync + 'static {
    /// Full eligibility sample. Slow platform signals may be refreshed here.
    fn sample(&self) -> Result<SystemState, String>;

    /// Fast preemption sample. Implementations should avoid slow COM/WMI/network work and return
    /// cached slow signals. The default keeps test/mock probes source-compatible.
    fn sample_fast(&self) -> Result<SystemState, String> { self.sample() }

    /// Requests a refresh of slow eligibility signals without blocking the preemption path.
    /// Platform implementations must single-flight this work; the default is a no-op for tests.
    fn refresh_slow_nonblocking(&self) {}
}

pub trait PassiveWorkExecutor: Send + Sync + 'static {
    fn execute(
        &self,
        workload: AutonomousWorkload,
        owner: &str,
        token: CancellationToken,
        commit_fence: CommitFence,
        governor: &ResourceGovernor,
    ) -> Result<PassiveWorkReport, String>;
}

#[derive(Clone)]
pub struct IdleScheduler {
    stop: Arc<AtomicBool>,
    wake: Arc<AtomicBool>,
}

pub struct SchedulerHandle {
    scheduler: IdleScheduler,
    worker: Option<thread::JoinHandle<()>>,
}

#[derive(Debug, thiserror::Error)]
#[error("{detail}")]
pub struct SchedulerStartError { detail: String }

#[derive(Clone, Copy)]
struct RunState {
    failures: u32,
    next_eligible_ms: i64,
    last_completed_ms: Option<i64>,
}

impl IdleScheduler {
    pub fn start(
        kernel: Arc<OperationKernel>,
        probe: Arc<dyn SystemStateProbe>,
        executor: Arc<dyn PassiveWorkExecutor>,
        config: SchedulerConfig,
    ) -> Result<SchedulerHandle, SchedulerStartError> {
        let scheduler = Self {
            stop: Arc::new(AtomicBool::new(false)),
            wake: Arc::new(AtomicBool::new(false)),
        };
        let worker_scheduler = scheduler.clone();
        let worker = thread::Builder::new()
            .name("aether-idle-scheduler".into())
            .spawn(move || run_loop(worker_scheduler, kernel, probe, executor, config))
            .map_err(|error| SchedulerStartError { detail: format!("failed to spawn idle scheduler: {error}") })?;
        Ok(SchedulerHandle { scheduler, worker: Some(worker) })
    }

    pub fn notify_state_change(&self) { self.wake.store(true, Ordering::Release); }

    pub fn stop(&self) {
        self.stop.store(true, Ordering::Release);
        self.notify_state_change();
    }
}

impl SchedulerHandle {
    pub fn scheduler(&self) -> IdleScheduler { self.scheduler.clone() }
}

impl Drop for SchedulerHandle {
    fn drop(&mut self) {
        self.scheduler.stop();
        if let Some(worker) = self.worker.take() { let _ = worker.join(); }
    }
}

fn map_read(workload: AutonomousWorkload) -> ReadWorkload {
    match workload {
        AutonomousWorkload::HardwareTelemetry | AutonomousWorkload::EventLogTriage => ReadWorkload::Diagnostics,
        AutonomousWorkload::DriverDiscovery => ReadWorkload::DriverDiscovery,
        AutonomousWorkload::CleanupInventory => ReadWorkload::CleanupDiscovery,
        AutonomousWorkload::StartupInventory => ReadWorkload::StartupDiscovery,
    }
}

fn event_state(outcome: RunOutcome) -> SchedulerRunState {
    match outcome {
        RunOutcome::Started => SchedulerRunState::Started,
        RunOutcome::Completed => SchedulerRunState::Completed,
        RunOutcome::Preempted => SchedulerRunState::Preempted,
        RunOutcome::Skipped => SchedulerRunState::Skipped,
        RunOutcome::Failed => SchedulerRunState::Failed,
    }
}

fn publish(
    kernel: &OperationKernel,
    owner: &str,
    workload: AutonomousWorkload,
    outcome: RunOutcome,
    reason: &str,
    report: PassiveWorkReport,
) {
    if owner.is_empty() { return; }
    kernel.events().publish(
        owner,
        EventKind::Scheduler,
        "",
        Some(event_envelope::Payload::Scheduler(v1::SchedulerEvent {
            workload: workload.as_str().into(),
            state: event_state(outcome) as i32,
            reason: reason.into(),
            changed_unix_ms: Utc::now().timestamp_millis(),
            evidence_count: report.evidence_count,
            warning_count: report.warning_count,
        })),
    );
}


fn preemption_required(
    eligibility: &EligibilityEngine,
    workload: AutonomousWorkload,
    expected_owner: &str,
    expected_session: u32,
    state: &SystemState,
    mutation_active: bool,
) -> bool {
    state.owner_principal_key != expected_owner
        || state.session_id != expected_session
        || !eligibility.evaluate(workload, state, mutation_active).is_empty()
}

fn classify_run_outcome(
    committed: bool,
    fence_valid: bool,
    stopping: bool,
    result: &Result<PassiveWorkReport, CollectorFault>,
) -> RunOutcome {
    if committed && result.is_ok() {
        RunOutcome::Completed
    } else if stopping
        || !fence_valid
        || result.as_ref().err().is_some_and(|error| matches!(error.kind, FaultKind::Cancelled))
    {
        RunOutcome::Preempted
    } else {
        // A watchdog timeout is a provider failure, not user preemption. Likewise an executor that
        // returned success without crossing its CommitFence is never reported as Completed.
        RunOutcome::Failed
    }
}

fn run_loop(
    scheduler: IdleScheduler,
    kernel: Arc<OperationKernel>,
    probe: Arc<dyn SystemStateProbe>,
    executor: Arc<dyn PassiveWorkExecutor>,
    config: SchedulerConfig,
) {
    let eligibility = EligibilityEngine::new(config.clone());
    let governor = ResourceGovernor::default();
    let mut runs = HashMap::<AutonomousWorkload, RunState>::new();
    let gates = HashMap::<AutonomousWorkload, IsolationGate>::from(
        AutonomousWorkload::ALL.map(|w| (w, IsolationGate::default())),
    );
    let mut last_owner = String::new();

    while !scheduler.stop.load(Ordering::Acquire) {
        let state = match probe.sample() {
            Ok(value) => value,
            Err(error) => {
                if error.starts_with("security-fatal:") {
                    warn!(%error, "idle scheduler terminated after security-fatal state probe failure");
                    return;
                }
                warn!(%error, "idle scheduler state probe unavailable");
                sleep_interruptible(&scheduler, config.idle_probe_interval);
                continue;
            }
        };

        if state.owner_principal_key != last_owner {
            last_owner = state.owner_principal_key.clone();
            runs.clear();
        }

        let mutation_active = kernel.mutations().is_active();
        let now = Utc::now().timestamp_millis();
        let mut ran = false;

        for workload in AutonomousWorkload::ALL {
            if let Entry::Vacant(entry) = runs.entry(workload) {
                let initial = match kernel.scheduler_cadence(&state.owner_principal_key, workload.as_str()) {
                    Ok(Some(saved)) => RunState {
                        failures: saved.failure_count,
                        next_eligible_ms: saved.next_eligible_unix_ms,
                        last_completed_ms: saved.last_completed_unix_ms,
                    },
                    Ok(None) => RunState {
                        failures: 0,
                        // Jitter the first eligible run so a fleet becoming idle together does not
                        // stampede WUA/EventLog/filesystem providers.
                        next_eligible_ms: now.saturating_add(random_jitter_ms(config.max_jitter)),
                        last_completed_ms: None,
                    },
                    Err(error) => {
                        warn!(%error, workload=workload.as_str(), "idle scheduler cadence ledger unavailable; autonomous work fails closed");
                        return;
                    }
                };
                entry.insert(initial);
            }
            let Some(run) = runs.get_mut(&workload) else {
                warn!(workload=workload.as_str(), "idle scheduler run state disappeared before admission; autonomous work fails closed");
                return;
            };
            if now < run.next_eligible_ms { continue; }

            let policy = eligibility.policy(workload);
            let blocked = eligibility.evaluate(workload, &state, mutation_active);
            if !blocked.is_empty() { continue; }

            let read = match kernel.reads().try_acquire(map_read(workload)) {
                Ok(value) => value,
                Err(_) => {
                    run.next_eligible_ms = now.saturating_add(config.resource_cooldown.as_millis().min(i64::MAX as u128) as i64);
                    publish(
                        &kernel,
                        &state.owner_principal_key,
                        workload,
                        RunOutcome::Skipped,
                        BlockReason::ReadBudgetBusy.as_str(),
                        PassiveWorkReport::default(),
                    );
                    continue;
                }
            };

            let token = CancellationToken::new();
            let commit_fence = CommitFence::new();
            let monitor_token = token.clone();
            let monitor_fence = commit_fence.clone();
            let monitor_scheduler = scheduler.clone();
            let monitor_probe = probe.clone();
            let owner = state.owner_principal_key.clone();
            let session_id = state.session_id;
            let kernel_monitor = kernel.clone();
            let eligibility_monitor = eligibility.clone();
            let active_probe_interval = config.active_probe_interval;
            let full_recheck_interval = config.full_recheck_interval;

            let monitor = match thread::Builder::new()
                .name("aether-idle-preemption".into())
                .spawn(move || {
                    let mut last_full = Instant::now();
                    loop {
                        if monitor_token.is_cancelled() { break; }
                        if monitor_scheduler.stop.load(Ordering::Acquire) {
                            monitor_fence.revoke();
                            monitor_token.cancel();
                            break;
                        }
                        thread::sleep(active_probe_interval);
                        if monitor_token.is_cancelled() { break; }

                        if last_full.elapsed() >= full_recheck_interval {
                            last_full = Instant::now();
                            monitor_probe.refresh_slow_nonblocking();
                        }
                        // Never block prompt activity/session/power preemption on a slow provider.
                        // Slow signals refresh in a single-flight background worker and are observed
                        // through this same fast cached sample as soon as they become available.
                        let sampled = monitor_probe.sample_fast();
                        let invalid = match sampled {
                            Ok(next) => preemption_required(
                                &eligibility_monitor,
                                workload,
                                &owner,
                                session_id,
                                &next,
                                kernel_monitor.mutations().is_active(),
                            ),
                            Err(_) => true,
                        };
                        if invalid {
                            // Revoke publication before signalling cancellation. A worker that exits
                            // a platform call late can observe the token too late, but it still cannot
                            // publish through the linearized commit fence.
                            monitor_fence.revoke();
                            monitor_token.cancel();
                            break;
                        }
                    }
                }) {
                Ok(worker) => worker,
                Err(error) => {
                    commit_fence.revoke();
                    token.cancel();
                    drop(read);
                    run.failures = run.failures.saturating_add(1);
                    run.next_eligible_ms = now.saturating_add(backoff_delay_ms(&config, run.failures));
                    if let Err(persist_error) = persist_cadence(&kernel, &state.owner_principal_key, workload, *run, RunOutcome::Failed) {
                        warn!(error=%persist_error, "idle scheduler cadence write failed; autonomous work disabled");
                        return;
                    }
                    publish(
                        &kernel,
                        &state.owner_principal_key,
                        workload,
                        RunOutcome::Failed,
                        "preemptionMonitorUnavailable",
                        PassiveWorkReport::default(),
                    );
                    warn!(%error, workload = workload.as_str(), "idle preemption monitor failed to start");
                    ran = true;
                    break;
                }
            };

            publish(
                &kernel,
                &state.owner_principal_key,
                workload,
                RunOutcome::Started,
                "started",
                PassiveWorkReport::default(),
            );

            let exec = executor.clone();
            let owner_exec = state.owner_principal_key.clone();
            let governor_exec = governor.clone();
            let work_token = token.clone();
            let work_fence = commit_fence.clone();
            let result = run_isolated_gated_with_token(
                &gates[&workload],
                "idle-scheduler",
                workload.as_str(),
                policy.timeout,
                work_token,
                move |control| {
                    crate::run_in_background_mode(|| exec.execute(
                        workload,
                        &owner_exec,
                        control.cancellation(),
                        work_fence,
                        &governor_exec,
                    ))
                    .map_err(|error| {
                        let kind = if control.is_cancelled() {
                            FaultKind::Cancelled
                        } else {
                            FaultKind::ProviderFailure
                        };
                        CollectorFault::new("idle-scheduler", workload.as_str(), kind, error)
                    })
                },
            );

            let committed = commit_fence.is_committed();
            let fence_valid = commit_fence.is_valid();
            let stopping = scheduler.stop.load(Ordering::Acquire);
            let outcome = classify_run_outcome(committed, fence_valid, stopping, &result);
            token.cancel();
            let _ = monitor.join();
            drop(read);

            let finished = Utc::now().timestamp_millis();
            match outcome {
                RunOutcome::Completed => {
                    run.failures = 0;
                    run.last_completed_ms = Some(finished);
                    run.next_eligible_ms = finished
                        .saturating_add(policy.minimum_interval.as_millis().min(i64::MAX as u128) as i64)
                        .saturating_add(random_jitter_ms(config.max_jitter));
                }
                RunOutcome::Preempted => {
                    run.next_eligible_ms = finished
                        .saturating_add(config.resource_cooldown.as_millis().min(i64::MAX as u128) as i64);
                }
                RunOutcome::Failed => {
                    run.failures = run.failures.saturating_add(1);
                    run.next_eligible_ms = finished.saturating_add(backoff_delay_ms(&config, run.failures));
                }
                RunOutcome::Started | RunOutcome::Skipped => {}
            }
            if let Err(error) = persist_cadence(&kernel, &state.owner_principal_key, workload, *run, outcome) {
                warn!(%error, "idle scheduler cadence write failed; autonomous work disabled");
                return;
            }

            let report = result.as_ref().ok().copied().unwrap_or_default();
            let reason = result
                .as_ref()
                .err()
                .map(|error| error.kind.as_str().to_owned())
                .unwrap_or_else(|| match outcome {
                    RunOutcome::Completed => "completed".into(),
                    RunOutcome::Preempted => "cancelled".into(),
                    RunOutcome::Failed => "publicationNotCommitted".into(),
                    RunOutcome::Started => "started".into(),
                    RunOutcome::Skipped => "skipped".into(),
                });
            publish(&kernel, &state.owner_principal_key, workload, outcome, &reason, report);
            ran = true;
            break;
        }

        sleep_interruptible(
            &scheduler,
            if ran { config.resource_cooldown } else { config.idle_probe_interval },
        );
    }
    info!("idle scheduler stopped");
}


fn persist_cadence(
    kernel: &OperationKernel,
    owner: &str,
    workload: AutonomousWorkload,
    run: RunState,
    outcome: RunOutcome,
) -> Result<(), String> {
    kernel.save_scheduler_cadence(
        owner,
        workload.as_str(),
        run.failures,
        run.next_eligible_ms,
        match outcome {
            RunOutcome::Started => "started",
            RunOutcome::Completed => "completed",
            RunOutcome::Preempted => "preempted",
            RunOutcome::Skipped => "skipped",
            RunOutcome::Failed => "failed",
        },
        run.last_completed_ms,
    )
}

fn backoff_ceiling_ms(config: &SchedulerConfig, failures: u32) -> i64 {
    let shift = failures.saturating_sub(1).min(16);
    let factor = 1u64 << shift;
    let base = config.base_backoff.as_millis() as u64;
    let cap = config.max_backoff.as_millis() as u64;
    base.saturating_mul(factor).min(cap).min(i64::MAX as u64) as i64
}

/// Equal-jitter exponential backoff: half the computed delay is guaranteed and the other half is
/// randomized. This prevents synchronized retry storms while preserving a monotonic backoff floor.
fn backoff_delay_ms(config: &SchedulerConfig, failures: u32) -> i64 {
    let ceiling = backoff_ceiling_ms(config, failures);
    let half = ceiling / 2;
    half.saturating_add(if ceiling <= half {
        0
    } else {
        rand::rng().random_range(0..=ceiling - half)
    })
}

fn random_jitter_ms(max: Duration) -> i64 {
    if max.is_zero() {
        0
    } else {
        rand::rng().random_range(0..=max.as_millis().min(i64::MAX as u128) as i64)
    }
}

fn sleep_interruptible(scheduler: &IdleScheduler, duration: Duration) {
    let start = Instant::now();
    while start.elapsed() < duration && !scheduler.stop.load(Ordering::Acquire) {
        if scheduler.wake.swap(false, Ordering::AcqRel) { break; }
        thread::sleep(Duration::from_millis(100).min(duration.saturating_sub(start.elapsed())));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eligible_state() -> SystemState {
        SystemState {
            sampled_unix_ms: 1,
            owner_principal_key: "owner".into(),
            user_sid: "S-1-5-21-test".into(),
            session_id: 7,
            idle_for: Duration::from_secs(10 * 60),
            session_unlocked: true,
            on_ac_power: true,
            battery_saver: false,
            thermal_pressure: ThermalPressure::Normal,
            network_cost: NetworkCost::Unmetered,
            presentation: PresentationState::Clear,
            servicing: ServicingState::Idle,
        }
    }

    #[test]
    fn exponential_backoff_ceiling_is_capped() {
        let config = SchedulerConfig {
            base_backoff: Duration::from_secs(2),
            max_backoff: Duration::from_secs(10),
            ..Default::default()
        };
        assert_eq!(backoff_ceiling_ms(&config, 1), 2_000);
        assert_eq!(backoff_ceiling_ms(&config, 2), 4_000);
        assert_eq!(backoff_ceiling_ms(&config, 4), 10_000);
        assert_eq!(backoff_ceiling_ms(&config, 30), 10_000);
        for failures in 1..=8 {
            let ceiling = backoff_ceiling_ms(&config, failures);
            let delay = backoff_delay_ms(&config, failures);
            assert!(delay >= ceiling / 2 && delay <= ceiling);
        }
    }

    #[test]
    fn scheduler_workload_enum_has_no_mutation_variants() {
        let names = AutonomousWorkload::ALL.map(|w| w.as_str()).join(",");
        for forbidden in ["Install", "Delete", "Repair", "Apply", "Update"] {
            assert!(!names.contains(forbidden));
        }
    }

    #[test]
    fn preemption_is_fail_closed_for_activity_owner_session_and_mutation_changes() {
        let eligibility = EligibilityEngine::new(SchedulerConfig::default());
        let mut state = eligible_state();
        assert!(!preemption_required(&eligibility, AutonomousWorkload::StartupInventory, "owner", 7, &state, false));
        state.idle_for = Duration::from_secs(1);
        assert!(preemption_required(&eligibility, AutonomousWorkload::StartupInventory, "owner", 7, &state, false));
        state = eligible_state(); state.owner_principal_key = "other".into();
        assert!(preemption_required(&eligibility, AutonomousWorkload::StartupInventory, "owner", 7, &state, false));
        state = eligible_state(); state.session_id = 8;
        assert!(preemption_required(&eligibility, AutonomousWorkload::StartupInventory, "owner", 7, &state, false));
        state = eligible_state();
        assert!(preemption_required(&eligibility, AutonomousWorkload::StartupInventory, "owner", 7, &state, true));
    }

    #[test]
    fn watchdog_timeout_is_failure_not_user_preemption() {
        let result: Result<PassiveWorkReport, CollectorFault> = Err(CollectorFault::timeout("idle-scheduler", "test"));
        assert_eq!(classify_run_outcome(false, true, false, &result), RunOutcome::Failed);
    }

    #[test]
    fn revoked_fence_is_preemption_and_uncommitted_success_is_never_completed() {
        let success = Ok(PassiveWorkReport::default());
        assert_eq!(classify_run_outcome(false, false, false, &success), RunOutcome::Preempted);
        assert_eq!(classify_run_outcome(false, true, false, &success), RunOutcome::Failed);
        assert_eq!(classify_run_outcome(true, true, false, &success), RunOutcome::Completed);
    }

    #[test]
    fn jitter_is_bounded_and_zero_configuration_is_deterministic() {
        assert_eq!(random_jitter_ms(Duration::ZERO), 0);
        for _ in 0..64 {
            let value = random_jitter_ms(Duration::from_millis(25));
            assert!((0..=25).contains(&value));
        }
    }

}
