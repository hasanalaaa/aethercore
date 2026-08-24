//! Phase 21 — read-only ingestion of persisted history into typed timeline events.
//!
//! This module consumes four existing tables (`plan_events`, `maintenance_executions`,
//! `repair_timeline_events`, `intelligence_scans`) through the public `aethercore-persistence`
//! API and maps them into [`TimelineEvent`] values. It performs **no writes**, adds **no
//! schema objects** and never rewrites historical truth (PCD-TIMELINE-INTELLIGENCE
//! completionTruth).
//!
//! Mapping is intentionally conservative:
//! - journal rows (`state_transition`) become `Operation` events; transitions whose target
//!   state is a recovery/failed state carry `Outcome::Failed`, all others stay neutral;
//! - maintenance executions with outcome `Failed`/`RecoveryRequired` become failure-shaped
//!   `Operation` events keyed by domain;
//! - repair timeline events map their persisted outcome verbatim;
//! - deep scans with warnings or unavailable collectors become `Finding` events.

use aethercore_persistence::{
    Database, IntelligenceScanRecord, MaintenanceExecutionRecord, RepairTimelineEventRecord,
    SupportJournalEventRecord,
};

use crate::model::{EventClass, Outcome, TimelineEvent};

/// Upper bound applied to every read query so one hostile principal cannot request
/// unbounded history. Matches the retention ceiling of the timeline itself.
const INGEST_READ_LIMIT: usize = 2000;

/// States that mark an operation run as failure-shaped for recurrence purposes.
fn failed_state(state: &str) -> bool {
    matches!(
        state,
        "RecoveryRequired" | "Failed" | "Cancelled" | "RolledBack"
    )
}

/// Persisted outcomes that mark a maintenance execution as failure-shaped.
fn failed_outcome(outcome: &str) -> bool {
    matches!(outcome, "Failed" | "RecoveryRequired" | "Escalated")
}

/// Maps a journal row to an operation event. `to_state` carries the transition
/// target; failure-shaped targets feed recurrence detection.
pub fn journal_event(record: &SupportJournalEventRecord) -> TimelineEvent {
    let failed = failed_state(record.to_state.as_str());
    let code = format!("journal.transition:{}", record.to_state);
    TimelineEvent::new(
        format!("journal:{}", record.seq),
        EventClass::Operation,
        "operationJournal",
        &code,
        if failed {
            Outcome::Failed
        } else {
            Outcome::Neutral
        },
        record.created_unix_ms,
    )
}

/// Maps one maintenance-execution completion to an operation event.
///
/// The observation timestamp is the execution's own completion time when present,
/// otherwise its last update — never wall-clock time at mapping.
pub fn maintenance_execution(record: &MaintenanceExecutionRecord) -> TimelineEvent {
    let observed = record.completed_unix_ms.unwrap_or(record.updated_unix_ms);
    let failed = failed_outcome(&record.outcome);
    let code = format!("execution.outcome:{}", record.outcome);
    TimelineEvent::new(
        format!("execution:{}", record.plan_id),
        EventClass::Operation,
        &record.domain,
        &code,
        if failed {
            Outcome::Failed
        } else {
            Outcome::Neutral
        },
        observed,
    )
}

/// Verbatim mapping of a persisted repair-timeline event.
pub fn repair_timeline_event(record: &RepairTimelineEventRecord) -> TimelineEvent {
    let failed = failed_outcome(&record.outcome);
    let code = format!("{}.{}", record.event_kind, record.action_id);
    TimelineEvent::new(
        format!("repair-event:{}", record.event_id),
        EventClass::Verification,
        &record.domain,
        &code,
        if failed {
            Outcome::Failed
        } else {
            Outcome::Neutral
        },
        record.created_unix_ms,
    )
}

/// Maps a completed deep scan to a finding event when it surfaced problems.
/// Fully clean scans are recorded as neutral findings so the timeline shows cadence.
pub fn intelligence_scan(record: &IntelligenceScanRecord) -> TimelineEvent {
    let finding_count = record.finding_count as usize;
    let outcome = if finding_count > 0 || record.unavailable_collector_count > 0 {
        Outcome::Failed
    } else {
        Outcome::Neutral
    };
    let code = format!("scan.completed:findings={finding_count}");
    TimelineEvent::new(
        format!("scan:{}", record.scan_id),
        EventClass::Finding,
        "deepScan",
        &code,
        outcome,
        record.completed_unix_ms,
    )
}

/// Reads the owner-scoped history tables and returns candidate events in arbitrary
/// (per-table) order; construction applies the deterministic total order afterwards.
///
/// Every query is bounded server-side; no table can push more than
/// [`INGEST_READ_LIMIT`] rows into the builder. Rows stamped after `watermark_unix_ms`
/// are quarantined here (counted, never silently mixed into the timeline) so hostile
/// future timestamps cannot poison ordering downstream.
pub fn ingest_owner_history(
    db: &Database,
    owner_principal_key: &str,
) -> Result<Vec<TimelineEvent>, aethercore_persistence::PersistenceError> {
    ingest_owner_history_with_watermark(db, owner_principal_key, i64::MAX)
}

/// As [`ingest_owner_history`], with an explicit freshness watermark.
pub fn ingest_owner_history_with_watermark(
    db: &Database,
    owner_principal_key: &str,
    watermark_unix_ms: i64,
) -> Result<Vec<TimelineEvent>, aethercore_persistence::PersistenceError> {
    let mut candidates = Vec::new();

    // 1. Operation journal (plan lifecycle events), newest-first, clamped.
    for record in db.support_journal_events_for_owner(owner_principal_key, INGEST_READ_LIMIT)? {
        push_if_fresh(&mut candidates, journal_event(&record), watermark_unix_ms);
    }

    // 2. Repair-domain structured timeline events (Phase 19 surface).
    for record in db.repair_timeline_events_for_owner(owner_principal_key, INGEST_READ_LIMIT)? {
        push_if_fresh(
            &mut candidates,
            repair_timeline_event(&record),
            watermark_unix_ms,
        );
    }

    // 3. Maintenance executions across domains, owner-visible plans only.
    for record in db.maintenance_executions_for_owner(owner_principal_key, INGEST_READ_LIMIT)? {
        push_if_fresh(
            &mut candidates,
            maintenance_execution(&record),
            watermark_unix_ms,
        );
    }

    // 4. Deep-scan history (bounded per persistence retention).
    for record in db.intelligence_scans_for_owner(owner_principal_key, 100)? {
        push_if_fresh(
            &mut candidates,
            intelligence_scan(&record),
            watermark_unix_ms,
        );
    }

    Ok(candidates)
}

/// Appends the candidate only when its timestamp is at or before the watermark.
fn push_if_fresh(
    candidates: &mut Vec<TimelineEvent>,
    event: TimelineEvent,
    watermark_unix_ms: i64,
) {
    if event.observed_unix_ms <= watermark_unix_ms {
        candidates.push(event);
    }
}
