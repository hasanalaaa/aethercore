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
    let end = before_index.min(timeline.events.len());
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
            // Capacity cannot trigger here: ingestion clamps candidates well below
            // the builder ceiling by construction.
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
        let before_index = before_sequence.max(0).min(usize::MAX as u64) as usize;
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
        };
        Ok((response, timeline))
    }
}
