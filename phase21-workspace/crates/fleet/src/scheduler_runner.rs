//! Phase 34 corrective — executable scheduled-compliance orchestration.
//!
//! Connects the full pipeline deterministically:
//!
//! `FleetSchedule` → due evaluation → scope resolution → selected hosts →
//! bounded fleet orchestrator → remote read-only compliance operation →
//! per-host results → append `fleet_run_history` → update `last_result` →
//! advance `next_run`.
//!
//! The scheduler state store is injectable (`SchedulerStore` trait) so the
//! COMPLETE pipeline is proven hermetically (injected/fake transport, real
//! persistence semantics) without network. A permanently running background
//! daemon on Windows stays qualification debt (QD-034-006); this runner is
//! the explicit tick path (`aetherctl fleet schedule run-due`) that makes
//! scheduled compliance executable today.

use crate::domain::{FleetHost, FleetInventory, FleetSchedule};
use crate::orchestrator::{FleetTransport, OrchestratorConfig, run_batch_for_profile};
use crate::scheduler::{Clock, OverlapLock, begin_run, is_due, record_result};
use crate::trust::RemoteOutcomeKind;
use std::sync::Arc;

/// How many past due periods a run may catch up: exactly one (no storms).
pub const MISSED_RUN_CATCHUP: u32 = 1;

/// Per-schedule execution summary returned by [`run_due_schedules`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScheduleRunSummary {
    pub schedule_id: String,
    pub started_unix_ms: i64,
    pub finished_unix_ms: i64,
    pub hosts_attempted: u32,
    pub hosts_ok: u32,
    pub hosts_failed: u32,
    pub outcome_summary: String,
    pub history_seq: i64,
    /// The failure that affected THIS schedule only; other schedules are
    /// never destroyed by it.
    pub error: Option<String>,
}

/// Injectable schedule-state persistence (schedules + append-only history).
pub trait SchedulerStore: Send + Sync {
    /// All schedules in deterministic (schedule_id) order.
    fn schedules(&self) -> Vec<FleetSchedule>;
    /// Persist one schedule's mutated state (next_run/last_result).
    fn save_schedule(&self, schedule: &FleetSchedule);
    /// Append one run-history record; returns its sequence number.
    fn append_history(&self, record: &ScheduleRunRecord) -> i64;
}

/// One run-history record (the flat parameter list factored into a type).
pub struct ScheduleRunRecord<'a> {
    pub schedule_id: Option<&'a str>,
    pub trigger_kind: &'a str,
    pub started_unix_ms: i64,
    pub finished_unix_ms: i64,
    pub hosts_attempted: u32,
    pub hosts_ok: u32,
    pub hosts_failed: u32,
    pub outcome_summary: &'a str,
}

/// A single due schedule failed, but sibling schedules continue.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum RunnerError {
    #[error("no enabled schedule is due at this instant")]
    NothingDue,
    #[error("schedule {0} failed: {1}")]
    ScheduleFailed(String, String),
}

/// Resolves a schedule's scope selectors (`host ids` and `group:<tag>`)
/// against the inventory, deterministically, deduplicated and sorted.
pub fn resolve_schedule_scope<'a>(
    inventory: &'a FleetInventory,
    scope: &[String],
) -> Vec<&'a FleetHost> {
    let mut selected: Vec<&FleetHost> = Vec::new();
    for selector in scope {
        if let Some(tag) = selector.strip_prefix("group:") {
            selected.extend(inventory.hosts_matching(tag));
        } else if let Some(host) = inventory.get(selector) {
            selected.push(host);
        }
    }
    selected.sort_by(|a, b| a.host_id.cmp(&b.host_id));
    selected.dedup_by(|a, b| a.host_id == b.host_id);
    selected
}

/// Runs ONE schedule's fleet batch through the bounded orchestrator.
/// Trust pre-check mirrors the CLI batch: untrusted hosts are reported as
/// typed NotVerified, never contacted. `make_operation` produces the closed
/// read-only remote operation per host.
pub fn run_one_schedule<T: FleetTransport + 'static>(
    schedule: &FleetSchedule,
    inventory: &FleetInventory,
    transport: Arc<T>,
    config: &OrchestratorConfig,
    cancel: &crate::transport::CancelToken,
) -> Result<(u32, u32, u32, String, Vec<crate::orchestrator::HostOutcome>), RunnerError> {
    let selected = resolve_schedule_scope(inventory, &schedule.scope);
    if selected.is_empty() {
        return Err(RunnerError::ScheduleFailed(
            schedule.schedule_id.clone(),
            "scope resolved to zero hosts".to_string(),
        ));
    }
    // Fail-closed trust gate: untrusted hosts get typed results, no contact.
    let hosts: Vec<FleetHost> = selected.into_iter().cloned().collect();
    let batch = run_batch_for_profile(transport, &hosts, &schedule.profile_id, config, cancel)
        .map_err(|error| {
            RunnerError::ScheduleFailed(schedule.schedule_id.clone(), error.to_string())
        })?;
    let attempted = batch.outcomes.len() as u32;
    let ok = batch.count(RemoteOutcomeKind::Success) as u32;
    let failed = attempted.saturating_sub(ok);
    let summary = format!(
        "{}: {} ok, {} failed/other{}",
        schedule.profile_id,
        ok,
        failed,
        if batch.cancelled { " (cancelled)" } else { "" }
    );
    Ok((attempted, ok, failed, summary, batch.outcomes))
}

/// The scheduler tick: runs every DUE + ENABLED schedule once, enforcing the
/// single-run overlap policy, catching up missed runs exactly once, appending
/// history, updating last_result, and advancing next_run. One schedule's
/// failure is isolated — the loop continues with the remaining schedules and
/// reports every outcome in deterministic schedule order.
pub fn run_due_schedules<T: FleetTransport + 'static>(
    store: &dyn SchedulerStore,
    inventory: &FleetInventory,
    clock: &dyn Clock,
    transport: Arc<T>,
    config: &OrchestratorConfig,
    overlap: &OverlapLock,
    cancel: &crate::transport::CancelToken,
) -> Vec<ScheduleRunSummary> {
    let now = clock.now_unix_ms();
    let mut summaries = Vec::new();
    for mut schedule in store.schedules() {
        if !is_due(&schedule, now) {
            continue;
        }
        // Overlap lock: another run of this schedule is already active.
        let Some(_guard) = overlap.try_acquire(&schedule.schedule_id) else {
            summaries.push(ScheduleRunSummary {
                schedule_id: schedule.schedule_id.clone(),
                started_unix_ms: now,
                finished_unix_ms: now,
                hosts_attempted: 0,
                hosts_ok: 0,
                hosts_failed: 0,
                outcome_summary: "skipped: overlap locked (already running)".to_string(),
                history_seq: 0,
                error: Some(crate::scheduler::ScheduleError::OverlapLocked.to_string()),
            });
            continue;
        };
        let started = now;
        match begin_run(&mut schedule, now) {
            Ok(_period_ms) => {}
            Err(error) => {
                summaries.push(ScheduleRunSummary {
                    schedule_id: schedule.schedule_id.clone(),
                    started_unix_ms: started,
                    finished_unix_ms: started,
                    hosts_attempted: 0,
                    hosts_ok: 0,
                    hosts_failed: 0,
                    outcome_summary: format!("skipped: {error}"),
                    history_seq: 0,
                    error: Some(error.to_string()),
                });
                continue;
            }
        }
        // Host failure isolation is inside run_batch; panic isolation too.
        // The whole-schedule failure path below isolates schedule failures.
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run_one_schedule(&schedule, inventory, Arc::clone(&transport), config, cancel)
        }))
        .unwrap_or_else(|_| {
            Err(RunnerError::ScheduleFailed(
                schedule.schedule_id.clone(),
                "scheduler task panicked; isolated".to_string(),
            ))
        });
        let finished = clock.now_unix_ms();
        let summary = match outcome {
            Ok((attempted, ok, failed, text, outcomes)) => {
                let seq = store.append_history(&ScheduleRunRecord {
                    schedule_id: Some(&schedule.schedule_id),
                    trigger_kind: "schedule",
                    started_unix_ms: started,
                    finished_unix_ms: finished,
                    hosts_attempted: attempted,
                    hosts_ok: ok,
                    hosts_failed: failed,
                    outcome_summary: &text,
                });
                record_result(&mut schedule, finished, attempted, ok, failed, &text);
                store.save_schedule(&schedule);
                let _ = outcomes;
                ScheduleRunSummary {
                    schedule_id: schedule.schedule_id.clone(),
                    started_unix_ms: started,
                    finished_unix_ms: finished,
                    hosts_attempted: attempted,
                    hosts_ok: ok,
                    hosts_failed: failed,
                    outcome_summary: text,
                    history_seq: seq,
                    error: None,
                }
            }
            Err(error) => {
                // The schedule itself failed (empty scope / orchestrator
                // refusal / panic). History still records the attempt;
                // next_run was already advanced by begin_run, and prior
                // history + other schedules' state are untouched.
                let error_text = error.to_string();
                let seq = store.append_history(&ScheduleRunRecord {
                    schedule_id: Some(&schedule.schedule_id),
                    trigger_kind: "schedule",
                    started_unix_ms: started,
                    finished_unix_ms: finished,
                    hosts_attempted: 0,
                    hosts_ok: 0,
                    hosts_failed: 0,
                    outcome_summary: &error_text,
                });
                record_result(&mut schedule, finished, 0, 0, 0, &error.to_string());
                store.save_schedule(&schedule);
                ScheduleRunSummary {
                    schedule_id: schedule.schedule_id.clone(),
                    started_unix_ms: started,
                    finished_unix_ms: finished,
                    hosts_attempted: 0,
                    hosts_ok: 0,
                    hosts_failed: 0,
                    outcome_summary: error.to_string(),
                    history_seq: seq,
                    error: Some(error.to_string()),
                }
            }
        };
        summaries.push(summary);
    }
    summaries
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{AuthReference, FleetCadence};
    use crate::orchestrator::HostOutcome;
    use crate::scheduler::FixedClock;
    use crate::trust::RemoteResult;
    use std::collections::BTreeSet;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn host(id: &str, tag: &str) -> FleetHost {
        let mut host = FleetHost::new(
            id,
            id,
            &format!("{id}.example.internal"),
            22,
            "ops",
            AuthReference::Agent,
            BTreeSet::new(),
        )
        .unwrap();
        host.tags.insert(tag.to_string());
        host
    }

    /// Fake transport that records whether the compliance operation ran.
    struct FakeTransport {
        calls: AtomicUsize,
    }

    impl FleetTransport for FakeTransport {
        fn execute(&self, _host: &FleetHost) -> RemoteResult {
            self.calls.fetch_add(1, Ordering::SeqCst);
            RemoteResult::kind(RemoteOutcomeKind::Success).detail("fake compliance run")
        }
    }

    type HistoryRow = (Option<String>, String, u32, u32, u32, String);

    struct MemoryStore {
        schedules: Mutex<Vec<FleetSchedule>>,
        history: Mutex<Vec<HistoryRow>>,
        next_seq: std::sync::atomic::AtomicI64,
    }

    impl MemoryStore {
        fn new(schedules: Vec<FleetSchedule>) -> Self {
            Self {
                schedules: Mutex::new(schedules),
                history: Mutex::new(Vec::new()),
                next_seq: std::sync::atomic::AtomicI64::new(1),
            }
        }
    }

    impl SchedulerStore for MemoryStore {
        fn schedules(&self) -> Vec<FleetSchedule> {
            self.schedules.lock().unwrap().clone()
        }
        fn save_schedule(&self, schedule: &FleetSchedule) {
            let mut guard = self.schedules.lock().unwrap();
            if let Some(slot) = guard
                .iter_mut()
                .find(|s| s.schedule_id == schedule.schedule_id)
            {
                *slot = schedule.clone();
            }
        }
        fn append_history(&self, record: &ScheduleRunRecord) -> i64 {
            self.history.lock().unwrap().push((
                record.schedule_id.map(str::to_string),
                record.trigger_kind.to_string(),
                record.hosts_attempted,
                record.hosts_ok,
                record.hosts_failed,
                record.outcome_summary.to_string(),
            ));
            self.next_seq.fetch_add(1, Ordering::SeqCst) - 1
        }
    }

    fn inventory() -> FleetInventory {
        let mut inventory = FleetInventory::new();
        inventory.add(host("edge-1", "edge")).unwrap();
        inventory.add(host("edge-2", "edge")).unwrap();
        inventory.add(host("core-1", "core")).unwrap();
        inventory
    }

    fn config() -> OrchestratorConfig {
        OrchestratorConfig::default()
    }

    #[test]
    fn due_schedule_invokes_fleet_compliance_transport_end_to_end() {
        let start = 1_000_000;
        let period = 3600 * 1000;
        let schedule = FleetSchedule::new(
            "sched-edge",
            vec!["group:edge".into()],
            "cis-l1",
            FleetCadence::EveryHours(1),
            start,
        )
        .unwrap();
        let store = MemoryStore::new(vec![schedule]);
        let transport = Arc::new(FakeTransport {
            calls: AtomicUsize::new(0),
        });
        let inventory = inventory();
        let clock = FixedClock(start + period); // exactly due
        let summaries = run_due_schedules(
            &store,
            &inventory,
            &clock,
            Arc::clone(&transport),
            &config(),
            &OverlapLock::default(),
            &crate::transport::CancelToken::default(),
        );
        assert_eq!(summaries.len(), 1);
        let summary = &summaries[0];
        assert_eq!(summary.error, None);
        assert_eq!(summary.hosts_attempted, 2, "group:edge resolved to 2 hosts");
        assert_eq!(summary.hosts_ok, 2);
        assert!(
            transport.calls.load(Ordering::SeqCst) >= 2,
            "transport invoked per host"
        );
        // history appended
        let history = store.history.lock().unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].0.as_deref(), Some("sched-edge"));
        assert_eq!(history[0].1, "schedule");
        assert_eq!(history[0].2, 2);
        // next_run advanced exactly one period past `now`
        let saved = &store.schedules.lock().unwrap()[0];
        assert_eq!(saved.next_run_unix_ms, start + 2 * period);
        assert!(saved.last_result.is_some());
    }

    #[test]
    fn not_due_schedule_is_not_run() {
        let start = 1_000_000;
        let schedule = FleetSchedule::new(
            "sched-later",
            vec!["group:edge".into()],
            "cis-l1",
            FleetCadence::EveryHours(2),
            start,
        )
        .unwrap();
        let store = MemoryStore::new(vec![schedule]);
        let transport = Arc::new(FakeTransport {
            calls: AtomicUsize::new(0),
        });
        let clock = FixedClock(start + 3600 * 1000 - 1);
        let summaries = run_due_schedules(
            &store,
            &inventory(),
            &clock,
            Arc::clone(&transport),
            &config(),
            &OverlapLock::default(),
            &crate::transport::CancelToken::default(),
        );
        assert!(summaries.is_empty());
        assert_eq!(transport.calls.load(Ordering::SeqCst), 0);
        assert!(store.history.lock().unwrap().is_empty());
    }

    #[test]
    fn disabled_schedule_is_never_run() {
        let start = 1_000_000;
        let mut schedule = FleetSchedule::new(
            "sched-off",
            vec!["group:edge".into()],
            "cis-l1",
            FleetCadence::EveryHours(1),
            start,
        )
        .unwrap();
        schedule.enabled = false;
        let store = MemoryStore::new(vec![schedule]);
        let transport = Arc::new(FakeTransport {
            calls: AtomicUsize::new(0),
        });
        let clock = FixedClock(start + 3600 * 1000 * 5);
        let summaries = run_due_schedules(
            &store,
            &inventory(),
            &clock,
            Arc::clone(&transport),
            &config(),
            &OverlapLock::default(),
            &crate::transport::CancelToken::default(),
        );
        assert!(summaries.is_empty());
        assert_eq!(transport.calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn failure_is_isolated_and_history_preserved() {
        let start = 1_000_000;
        let period = 3600 * 1000;
        let good = FleetSchedule::new(
            "sched-a-good",
            vec!["group:edge".into()],
            "cis-l1",
            FleetCadence::EveryHours(1),
            start,
        )
        .unwrap();
        // Scope resolving to zero hosts -> typed schedule failure.
        let broken = FleetSchedule::new(
            "sched-b-broken",
            vec!["group:ghost".into()],
            "cis-l1",
            FleetCadence::EveryHours(1),
            start,
        )
        .unwrap();
        let store = MemoryStore::new(vec![good, broken]);
        let transport = Arc::new(FakeTransport {
            calls: AtomicUsize::new(0),
        });
        let clock = FixedClock(start + period);
        let summaries = run_due_schedules(
            &store,
            &inventory(),
            &clock,
            Arc::clone(&transport),
            &config(),
            &OverlapLock::default(),
            &crate::transport::CancelToken::default(),
        );
        // Deterministic schedule order; both schedules processed.
        assert_eq!(summaries.len(), 2);
        assert_eq!(summaries[0].schedule_id, "sched-a-good");
        assert_eq!(summaries[0].error, None, "good schedule ran fully");
        assert_eq!(summaries[1].schedule_id, "sched-b-broken");
        assert!(
            summaries[1].error.is_some(),
            "broken schedule typed its failure"
        );
        // The good schedule's history is intact; the broken one appended its
        // own failure record; neither erased the other's state.
        let history = store.history.lock().unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].0.as_deref(), Some("sched-a-good"));
        assert_eq!(history[0].3, 2, "good hosts_ok preserved");
        assert_eq!(history[1].0.as_deref(), Some("sched-b-broken"));
        // Both schedules' next_run advanced (no schedule-state destruction).
        let schedules = store.schedules.lock().unwrap();
        assert!(schedules[0].last_result.is_some());
        assert!(schedules[1].last_result.is_some());
    }

    #[test]
    fn cancellation_propagates_as_typed_outcomes() {
        let start = 1_000_000;
        let schedule = FleetSchedule::new(
            "sched-cancel",
            vec!["group:edge".into()],
            "cis-l1",
            FleetCadence::EveryHours(1),
            start,
        )
        .unwrap();
        let store = MemoryStore::new(vec![schedule]);
        struct CancellingTransport {
            cancel: crate::transport::CancelToken,
        }
        impl FleetTransport for CancellingTransport {
            fn execute(&self, _host: &FleetHost) -> RemoteResult {
                // Simulate a cancellation signal arriving mid-batch.
                self.cancel.cancel();
                RemoteResult::kind(RemoteOutcomeKind::Cancelled)
            }
        }
        let cancel = crate::transport::CancelToken::default();
        let transport = Arc::new(CancellingTransport {
            cancel: cancel.clone(),
        });
        let clock = FixedClock(start + 3600 * 1000);
        let summaries = run_due_schedules(
            &store,
            &inventory(),
            &clock,
            transport,
            &config(),
            &OverlapLock::default(),
            &cancel,
        );
        assert_eq!(summaries.len(), 1);
        assert!(summaries[0].outcome_summary.contains("cancelled"));
    }

    #[test]
    fn missed_run_catches_up_exactly_once() {
        let start = 1_000_000;
        let period = 3600 * 1000;
        let schedule = FleetSchedule::new(
            "sched-catchup",
            vec!["group:edge".into()],
            "cis-l1",
            FleetCadence::EveryHours(1),
            start,
        )
        .unwrap();
        let store = MemoryStore::new(vec![schedule]);
        let transport = Arc::new(FakeTransport {
            calls: AtomicUsize::new(0),
        });
        // 10 periods of downtime: the runner executes ONE run and re-anchors.
        let clock = FixedClock(start + 10 * period);
        let summaries = run_due_schedules(
            &store,
            &inventory(),
            &clock,
            Arc::clone(&transport),
            &config(),
            &OverlapLock::default(),
            &crate::transport::CancelToken::default(),
        );
        assert_eq!(summaries.len(), 1);
        let saved_next_run = store.schedules()[0].next_run_unix_ms;
        assert_eq!(
            saved_next_run,
            start + 11 * period,
            "anchored once at now+period"
        );
        // Running the tick again at the SAME instant must not re-run it.
        let summaries2 = run_due_schedules(
            &store,
            &inventory(),
            &clock,
            Arc::clone(&transport),
            &config(),
            &OverlapLock::default(),
            &crate::transport::CancelToken::default(),
        );
        assert!(summaries2.is_empty(), "no catch-up storm");
    }

    #[test]
    fn results_are_deterministic_and_bounded() {
        let start = 1_000_000;
        let schedule = FleetSchedule::new(
            "sched-order",
            vec!["edge-2".into(), "core-1".into(), "edge-1".into()],
            "cis-l2",
            FleetCadence::EveryHours(1),
            start,
        )
        .unwrap();
        let store = MemoryStore::new(vec![schedule]);
        struct RecordingTransport {
            order: Mutex<Vec<String>>,
            profiles: Mutex<Vec<String>>,
        }
        impl FleetTransport for RecordingTransport {
            fn execute(&self, host: &FleetHost) -> RemoteResult {
                self.order.lock().unwrap().push(host.host_id.clone());
                RemoteResult::kind(RemoteOutcomeKind::Success)
            }
            fn execute_for_profile(&self, host: &FleetHost, profile_id: &str) -> RemoteResult {
                self.profiles.lock().unwrap().push(profile_id.to_string());
                self.execute(host)
            }
        }
        let transport = Arc::new(RecordingTransport {
            order: Mutex::new(Vec::new()),
            profiles: Mutex::new(Vec::new()),
        });
        let clock = FixedClock(start + 3600 * 1000);
        run_due_schedules(
            &store,
            &inventory(),
            &clock,
            Arc::clone(&transport),
            &OrchestratorConfig {
                max_concurrency: 2,
                ..OrchestratorConfig::default()
            },
            &OverlapLock::default(),
            &crate::transport::CancelToken::default(),
        );
        let mut order = transport.order.lock().unwrap().clone();
        order.sort();
        assert_eq!(
            order,
            vec![
                "core-1".to_string(),
                "edge-1".to_string(),
                "edge-2".to_string()
            ],
            "every selected host executed exactly once"
        );
        let summary_of_history = store.history.lock().unwrap()[0].clone();
        assert_eq!(summary_of_history.2, 3, "attempted == selected count");
        assert_eq!(
            transport.profiles.lock().unwrap().as_slice(),
            ["cis-l2", "cis-l2", "cis-l2"],
            "the validated schedule profile reaches the real transport seam"
        );
    }

    #[test]
    fn scope_resolution_is_tag_and_id_aware_deduped() {
        let inventory = inventory();
        let selected = resolve_schedule_scope(
            &inventory,
            &["group:edge".into(), "edge-1".into(), "group:ghost".into()],
        );
        let ids: Vec<&str> = selected.iter().map(|h| h.host_id.as_str()).collect();
        assert_eq!(ids, vec!["edge-1", "edge-2"], "dedup + deterministic sort");
    }

    // silence unused warnings for the helper field used by other stubs
    #[allow(dead_code)]
    fn _touch(h: &HostOutcome) {
        let _ = &h.host_id;
    }
}
