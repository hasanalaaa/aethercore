//! Phase 34 — fleet compliance scheduling semantics with an injectable clock.
//!
//! Deterministic, testable scheduling: no wall-clock reads inside the decision
//! functions, no tight-loop possibility (structural cadence floor of 1 hour),
//! explicit missed-run catch-up, single-overlap policy (a schedule never runs
//! twice concurrently — enforced by the `in_flight` marker in the caller),
//! cancellation, and per-host isolation inherited from the orchestrator.
//!
//! A background service that keeps this running continuously on Windows is a
//! Windows-native qualification item (QD-034-*); on macOS the semantics are
//! proven deterministically through this module and the CLI `schedule due`
//! evaluation path.

use crate::domain::FleetSchedule;

/// Injected clock: unix milliseconds. Test-friendly.
pub trait Clock: Send + Sync {
    fn now_unix_ms(&self) -> i64;
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now_unix_ms(&self) -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_millis() as i64)
            .unwrap_or(0)
    }
}

/// Fixed clock for deterministic proofs.
pub struct FixedClock(pub i64);

impl Clock for FixedClock {
    fn now_unix_ms(&self) -> i64 {
        self.0
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ScheduleError {
    #[error("schedule is disabled")]
    Disabled,
    #[error("schedule is already running (overlap policy: single-run)")]
    OverlapLocked,
    #[error("schedule is not due")]
    NotDue,
}

/// Is the schedule due at `now`? Pure function of state + clock.
pub fn is_due(schedule: &FleetSchedule, now_unix_ms: i64) -> bool {
    schedule.enabled && now_unix_ms >= schedule.next_run_unix_ms
}

/// Begins a run: computes the next-run mark and returns the cadence period.
/// The caller must hold the overlap lock before calling this; a disabled
/// schedule is a typed refusal. Missed runs are caught up exactly once: the
/// next run is anchored at `now + period`, never re-anchored into the past
/// (this prevents catch-up storm loops after downtime).
pub fn begin_run(schedule: &mut FleetSchedule, now_unix_ms: i64) -> Result<i64, ScheduleError> {
    if !schedule.enabled {
        return Err(ScheduleError::Disabled);
    }
    if now_unix_ms < schedule.next_run_unix_ms {
        return Err(ScheduleError::NotDue);
    }
    let period_ms = (schedule
        .cadence
        .period_secs()
        .map_err(|_| ScheduleError::OverlapLocked)?
        * 1000) as i64;
    schedule.next_run_unix_ms = now_unix_ms + period_ms;
    Ok(period_ms)
}

/// Records a finished run into `last_result` metadata. Failure does NOT erase
/// prior history beyond updating the summary — the caller appends run records
/// to persistence; this field is the latest-run digest only.
pub fn record_result(
    schedule: &mut FleetSchedule,
    finished_unix_ms: i64,
    hosts_attempted: u32,
    hosts_ok: u32,
    hosts_failed: u32,
    outcome_summary: &str,
) {
    schedule.last_result = Some(crate::domain::FleetScheduleLastResult {
        finished_unix_ms,
        hosts_attempted,
        hosts_ok,
        hosts_failed,
        outcome_summary: outcome_summary.to_string(),
    });
}

/// A simple overlap lock the caller holds for the duration of a run.
#[derive(Default)]
pub struct OverlapLock {
    in_flight: std::sync::Mutex<std::collections::BTreeSet<String>>,
}

impl OverlapLock {
    /// Try to acquire exclusive run rights for a schedule. `None` = another
    /// run of the same schedule is already active (single-run policy).
    pub fn try_acquire(&self, schedule_id: &str) -> Option<OverlapGuard<'_>> {
        let mut guard = self.in_flight.lock().ok()?;
        if guard.contains(schedule_id) {
            return None;
        }
        guard.insert(schedule_id.to_string());
        Some(OverlapGuard {
            schedule_id: schedule_id.to_string(),
            lock: self,
        })
    }
}

pub struct OverlapGuard<'a> {
    schedule_id: String,
    lock: &'a OverlapLock,
}

impl Drop for OverlapGuard<'_> {
    fn drop(&mut self) {
        if let Ok(mut guard) = self.lock.in_flight.lock() {
            guard.remove(&self.schedule_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{FleetCadence, MIN_CADENCE_SECS};

    fn schedule(cadence: FleetCadence, start: i64) -> FleetSchedule {
        FleetSchedule::new(
            "sched-weekly",
            vec!["group:edge".to_string()],
            "cis-l1",
            cadence,
            start,
        )
        .unwrap()
    }

    #[test]
    fn due_and_not_due_with_fixed_clock() {
        let start = 1_000_000;
        let mut sched = schedule(FleetCadence::EveryHours(2), start);
        let period_ms = 7200 * 1000;
        // not due before the anchor
        assert!(!is_due(&sched, start));
        assert!(!is_due(&sched, start + period_ms - 1));
        assert!(is_due(&sched, start + period_ms));
        // begin_run advances the anchor
        begin_run(&mut sched, start + period_ms).unwrap();
        assert_eq!(sched.next_run_unix_ms, start + 2 * period_ms);
        assert!(!is_due(&sched, start + 2 * period_ms - 1));
        assert!(is_due(&sched, start + 2 * period_ms));
    }

    #[test]
    fn disabled_schedule_refuses() {
        let start = 1_000_000;
        let mut sched = schedule(FleetCadence::DailyAtUtcHour(5), start);
        sched.enabled = false;
        assert_eq!(
            begin_run(&mut sched, start + 86_400_000),
            Err(ScheduleError::Disabled)
        );
        assert!(!is_due(&sched, start + 86_400_000));
    }

    #[test]
    fn missed_run_catches_up_once_without_storm() {
        let start = 1_000_000;
        let mut sched = schedule(FleetCadence::EveryHours(1), start);
        let period_ms = 3600 * 1000;
        // 10 periods of downtime: next run anchored ONCE at now+period
        let after_downtime = start + 10 * period_ms;
        begin_run(&mut sched, after_downtime).unwrap();
        assert_eq!(sched.next_run_unix_ms, after_downtime + period_ms);
    }

    #[test]
    fn overlap_policy_is_single_run() {
        let lock = OverlapLock::default();
        let guard = lock.try_acquire("sched-a").unwrap();
        assert!(lock.try_acquire("sched-a").is_none());
        assert!(lock.try_acquire("sched-b").is_some()); // other schedules unaffected
        drop(guard);
        assert!(lock.try_acquire("sched-a").is_some());
    }

    #[test]
    fn failure_updates_summary_without_losing_history_shape() {
        let start = 1_000_000;
        let mut sched = schedule(FleetCadence::EveryHours(2), start);
        record_result(&mut sched, start + 100, 3, 1, 2, "partial: 1 ok, 2 failed");
        let last = sched.last_result.clone().unwrap();
        assert_eq!(last.hosts_attempted, 3);
        assert_eq!(last.hosts_failed, 2);
        record_result(&mut sched, start + 200, 3, 3, 0, "all ok");
        assert_eq!(sched.last_result.unwrap().hosts_ok, 3);
    }

    #[test]
    fn no_tight_loop_floor_is_structural() {
        // The smallest legal cadence is >= MIN_CADENCE_SECS.
        assert_eq!(MIN_CADENCE_SECS, 3600);
        assert!(FleetCadence::EveryHours(0).period_secs().is_err());
        let start = 1;
        let mut sched = schedule(FleetCadence::EveryHours(1), start);
        // begin_run twice in the same instant is NotDue the second time —
        // a caller looping on is_due cannot spin faster than one period.
        begin_run(&mut sched, start + 3600 * 1000).unwrap();
        assert_eq!(
            begin_run(&mut sched, start + 3600 * 1000),
            Err(ScheduleError::NotDue)
        );
    }
}
