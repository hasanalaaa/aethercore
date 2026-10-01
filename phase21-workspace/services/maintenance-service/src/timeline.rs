//! Phase 21 — protocol mappings and the service-side timeline coordinator.
//!
//! Timeline pages and recurrence patterns are computed on demand from persisted
//! history, principal-scoped by the caller. Every page is bounded server-side;
//! nothing here trusts client-supplied sizes.

use aethercore_contracts::v1;
use aethercore_persistence::Database;
use aethercore_timeline_intelligence::{
    EventClass, MAX_PAGE_SIZE, Outcome, RecurrenceConfidence, Timeline,
};

// ---------------------------------------------------------------------------
// Domain -> wire mappings
// ---------------------------------------------------------------------------

pub(crate) fn event_class_proto(value: EventClass) -> v1::TimelineEventClass {
    match value {
        EventClass::Operation => v1::TimelineEventClass::Operation,
        EventClass::Finding => v1::TimelineEventClass::Finding,
        EventClass::Verification => v1::TimelineEventClass::Verification,
        EventClass::Recovery => v1::TimelineEventClass::Recovery,
        EventClass::Escalation => v1::TimelineEventClass::Escalation,
    }
}

pub(crate) fn outcome_proto(value: Outcome) -> v1::TimelineOutcome {
    match value {
        Outcome::Succeeded => v1::TimelineOutcome::Succeeded,
        Outcome::Failed => v1::TimelineOutcome::Failed,
        Outcome::Neutral => v1::TimelineOutcome::Neutral,
    }
}

fn confidence_proto(value: RecurrenceConfidence) -> v1::RecurrenceConfidence {
    match value {
        RecurrenceConfidence::Weak => v1::RecurrenceConfidence::Weak,
        RecurrenceConfidence::Moderate => v1::RecurrenceConfidence::Moderate,
        RecurrenceConfidence::Strong => v1::RecurrenceConfidence::Strong,
    }
}

pub(crate) fn timeline_entry_proto(
    event: &aethercore_timeline_intelligence::TimelineEvent,
) -> v1::TimelineEntry {
    v1::TimelineEntry {
        source_id: event.source_id.clone(),
        class: event_class_proto(event.class) as i32,
        domain: event.domain.clone(),
        code: event.code.clone(),
        outcome: outcome_proto(event.outcome) as i32,
        observed_unix_ms: event.observed_unix_ms,
        semantic_identity_sha256: event.semantic_identity_sha256.clone(),
    }
}

pub(crate) fn recurrence_pattern_proto(
    pattern: &aethercore_timeline_intelligence::RecurrencePattern,
) -> v1::RecurrencePattern {
    v1::RecurrencePattern {
        semantic_identity_sha256: pattern.semantic_identity_sha256.clone(),
        class: event_class_proto(pattern.class) as i32,
        domain: pattern.domain.clone(),
        code: pattern.code.clone(),
        confidence: confidence_proto(pattern.confidence) as i32,
        occurrence_count: pattern.occurrence_count.min(u32::MAX as usize) as u32,
        first_observed_unix_ms: pattern.first_observed_unix_ms,
        last_observed_unix_ms: pattern.last_observed_unix_ms,
        mean_gap_ms: pattern.mean_gap_ms,
        evidence: pattern
            .evidence
            .iter()
            .map(|citation| v1::RecurrenceEvidence {
                source_id: citation.source_id.clone(),
                observed_unix_ms: citation.observed_unix_ms,
                gap_from_previous_ms: citation.gap_from_previous_ms,
            })
            .collect(),
    }
}

/// Slices an ordered timeline into a bounded wire page. `before_sequence` is the
/// exclusive upper index into the ordered event list (0 = newest page).
fn timeline_page_proto(
    timeline: &Timeline,
    page_size: usize,
    before_index: usize,
) -> v1::TimelineResponse {
    // 0 is the newest page (timeline.proto, and what every client sends). It was taken as
    // the end index, so the newest page was always empty (P76, DBT-P76-006).
    let end = match before_index {
        0 => timeline.events.len(),
        index => index.min(timeline.events.len()),
    };
    let start = end.saturating_sub(page_size);
    let entries: Vec<v1::TimelineEntry> = timeline.events[start..end]
        .iter()
        .map(timeline_entry_proto)
        .collect();
    let has_more = start > 0;
    v1::TimelineResponse {
        entries,
        has_more,
        next_before_sequence: if has_more { start as u64 } else { 0 },
        digest_sha256: timeline.digest_sha256.clone(),
        duplicates_collapsed: timeline.duplicates_collapsed.min(u32::MAX as usize) as u32,
        history_window_limit: aethercore_timeline_intelligence::MAX_TIMELINE_EVENTS as u32,
    }
}

// ---------------------------------------------------------------------------
// Service-side coordinator
// ---------------------------------------------------------------------------

/// Computes timelines from persisted owner history on demand. Stateless beyond the
/// database handle; every request re-derives deterministic content.
pub struct TimelineCoordinator {
    db: std::sync::Arc<Database>,
}

impl TimelineCoordinator {
    pub fn new(db: std::sync::Arc<Database>) -> Self {
        Self { db }
    }

    /// Builds the full owner timeline once per request pair (page + patterns share it).
    fn build_timeline(
        &self,
        owner_principal_key: &str,
    ) -> Result<Timeline, aethercore_persistence::PersistenceError> {
        // Freshness watermark is the service's own wall-clock now: rows stamped in the
        // future are quarantined during ingestion and can never poison ordering.
        let watermark = chrono::Utc::now().timestamp_millis();
        let candidates =
            aethercore_timeline_intelligence::ingest::ingest_owner_history_with_watermark(
                &self.db,
                owner_principal_key,
                watermark,
            )?;
        let mut builder =
            aethercore_timeline_intelligence::TimelineBuilder::new().watermark(watermark);
        builder.ingest_all(candidates).map_err(|err| match err {
            aethercore_timeline_intelligence::TimelineError::FutureTimestamp { .. } => {
                // Ingestion quarantines future rows already; reaching this arm means
                // an invariant broke. Fail loudly instead of emitting a wrong page.
                aethercore_persistence::PersistenceError::Poisoned
            }
            // Ingestion globally clamps after merging the independently bounded
            // sources. Capacity here would mean that invariant broke.
            aethercore_timeline_intelligence::TimelineError::CapacityExceeded { .. } => {
                aethercore_persistence::PersistenceError::Poisoned
            }
        })?;
        Ok(builder.build())
    }

    /// Principal-scoped, server-bounded page over the owner's timeline.
    pub fn page_for_owner(
        &self,
        owner_principal_key: &str,
        requested_page_size: u32,
        before_sequence: u64,
    ) -> Result<(v1::TimelineResponse, Timeline), aethercore_persistence::PersistenceError> {
        let page_size = requested_page_size.clamp(1, MAX_PAGE_SIZE as u32) as usize;
        let before_index = before_sequence.min(usize::MAX as u64) as usize;
        let timeline = self.build_timeline(owner_principal_key)?;
        let page = timeline_page_proto(&timeline, page_size, before_index);
        Ok((page, timeline))
    }

    /// Principal-scoped recurrence patterns with their full evidence matrices.
    pub fn patterns_for_owner(
        &self,
        owner_principal_key: &str,
    ) -> Result<(v1::RecurrencePatternsResponse, Timeline), aethercore_persistence::PersistenceError>
    {
        let timeline = self.build_timeline(owner_principal_key)?;
        let response = v1::RecurrencePatternsResponse {
            patterns: timeline
                .patterns
                .iter()
                .map(recurrence_pattern_proto)
                .collect(),
            digest_sha256: timeline.digest_sha256.clone(),
            history_window_limit: aethercore_timeline_intelligence::MAX_TIMELINE_EVENTS as u32,
        };
        Ok((response, timeline))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aethercore_persistence::PlanRecord;
    use aethercore_timeline_intelligence::{TimelineBuilder, TimelineEvent};
    use std::sync::Arc;

    fn timeline_of(count: usize) -> Timeline {
        let mut builder = TimelineBuilder::new().watermark(1_000_000);
        for index in 0..count {
            builder
                .ingest(TimelineEvent::new(
                    format!("event:{index}"),
                    EventClass::Operation,
                    "cleanup",
                    &format!("execution.outcome:{index}"),
                    Outcome::Neutral,
                    1_000 + index as i64,
                ))
                .expect("event");
        }
        builder.build()
    }

    fn ids(page: &v1::TimelineResponse) -> Vec<&str> {
        page.entries.iter().map(|e| e.source_id.as_str()).collect()
    }

    /// P79-01: `before_sequence = 0` is the newest page (timeline.proto), which is what every client
    /// sends for the first page. The cursor then walks back to the oldest events without repeating any.
    #[test]
    fn the_first_page_is_the_newest_and_the_cursor_walks_back_without_repeats() {
        let timeline = timeline_of(3);
        let first = timeline_page_proto(&timeline, 2, 0);
        assert_eq!(
            ids(&first),
            ["event:1", "event:2"],
            "the two newest events, oldest of them first"
        );
        assert!(first.has_more);
        assert_eq!(first.next_before_sequence, 1);
        let second = timeline_page_proto(&timeline, 2, first.next_before_sequence as usize);
        assert_eq!(
            ids(&second),
            ["event:0"],
            "the cursor returns the rest, none of the first page"
        );
        assert!(!second.has_more);
        assert_eq!(second.next_before_sequence, 0);
    }

    #[test]
    fn an_empty_timeline_and_an_oversized_cursor_or_page_stay_in_bounds() {
        let empty = timeline_page_proto(&timeline_of(0), 50, 0);
        assert!(empty.entries.is_empty() && !empty.has_more && empty.next_before_sequence == 0);
        let three = timeline_of(3);
        assert_eq!(
            ids(&timeline_page_proto(&three, 50, 0)).len(),
            3,
            "a page larger than the history is all of it"
        );
        assert_eq!(
            ids(&timeline_page_proto(&three, 2, usize::MAX)),
            ["event:1", "event:2"],
            "a cursor past the end is the newest page"
        );
    }

    fn database(label: &str) -> (Arc<Database>, std::path::PathBuf) {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "aethercore-timeline-{label}-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).expect("temp root");
        (
            Arc::new(Database::open(root.join("state.db")).expect("db")),
            root,
        )
    }

    fn seed_plan(db: &Database, id: &str, owner: &str, at: i64) {
        db.insert_plan(
            &PlanRecord {
                id: id.into(),
                title: "seed".into(),
                state: "Completed".into(),
                digest: format!("digest-{id}"),
                risk: "Low".into(),
                immutable_json: "{}".into(),
                created_unix_ms: at,
                updated_unix_ms: at,
                owner_principal_key: owner.into(),
            },
            "created",
        )
        .expect("plan");
    }

    /// The coordinator over a real database: the caller's own history only, and a request for a huge
    /// page is cut to the server's bound, not honoured.
    #[test]
    fn a_page_is_the_callers_own_history_and_never_larger_than_the_server_bound() {
        let (db, root) = database("page");
        for index in 0..(MAX_PAGE_SIZE + 5) {
            seed_plan(
                &db,
                &format!("mine-{index}"),
                "owner-a",
                1_000 + index as i64,
            );
        }
        seed_plan(&db, "theirs", "owner-b", 1_000);
        let coordinator = TimelineCoordinator::new(db);
        let (page, _) = coordinator
            .page_for_owner("owner-a", u32::MAX, 0)
            .expect("page");
        assert_eq!(
            page.entries.len(),
            MAX_PAGE_SIZE,
            "a huge page size is bounded"
        );
        assert!(page.has_more);
        assert!(
            page.entries.iter().all(|e| !e.source_id.contains("theirs")),
            "another owner's history leaked"
        );
        let (other, _) = coordinator.page_for_owner("owner-b", 10, 0).expect("page");
        assert_eq!(
            other.entries.len(),
            1,
            "the other owner sees only its own event"
        );
        let _ = std::fs::remove_dir_all(root);
    }
}
