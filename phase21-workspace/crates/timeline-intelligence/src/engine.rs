//! Phase 21 — deterministic timeline construction and recurrence reasoning.
//!
//! Construction is a pure function of its input rows: stable total ordering,
//! semantic deduplication, bounded retention and evidence-complete recurrence
//! detection. Identical input yields a byte-identical timeline digest.

use std::collections::HashMap;

use sha2::{Digest, Sha256};

use crate::model::{
    EventClass, MAX_PATTERN_EVIDENCE, MAX_PATTERNS, MAX_RECURRENCE_GAP_MS, MAX_TIMELINE_EVENTS,
    MIN_OCCURRENCES_FOR_PATTERN, Outcome, PatternEvidence, RecurrenceConfidence, RecurrencePattern,
    Timeline, TimelineError, TimelineEvent,
};

/// Builder that ingests persisted history rows and produces a `Timeline`.
#[derive(Debug, Default)]
pub struct TimelineBuilder {
    events: Vec<TimelineEvent>,
    seen: HashMap<String, ()>,
    duplicates_collapsed: usize,
    watermark_unix_ms: Option<i64>,
}

impl TimelineBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Pins the freshness watermark up front. Events with timestamps after it are
    /// rejected with [`TimelineError::FutureTimestamp`].
    pub fn watermark(mut self, unix_ms: i64) -> Self {
        self.watermark_unix_ms = Some(unix_ms);
        self
    }

    /// Ingests one candidate event. Exact duplicates (same semantic identity AND same
    /// observation timestamp) collapse silently; everything else is retained in order.
    pub fn ingest(&mut self, event: TimelineEvent) -> Result<&mut Self, TimelineError> {
        if let Some(watermark) = self.watermark_unix_ms {
            if event.observed_unix_ms > watermark {
                return Err(TimelineError::FutureTimestamp {
                    observed_unix_ms: event.observed_unix_ms,
                    watermark_unix_ms: watermark,
                });
            }
        }
        if self.events.len() >= MAX_TIMELINE_EVENTS {
            return Err(TimelineError::CapacityExceeded {
                limit: MAX_TIMELINE_EVENTS,
            });
        }
        let identity = format!(
            "{}:{}:{}",
            event.semantic_identity_sha256,
            event.observed_unix_ms,
            event.class.ordinal()
        );
        if self.seen.insert(identity, ()).is_some() {
            self.duplicates_collapsed += 1;
            return Ok(self);
        }
        self.events.push(event);
        Ok(self)
    }

    /// Ingests many candidates at once.
    pub fn ingest_all(
        &mut self,
        events: impl IntoIterator<Item = TimelineEvent>,
    ) -> Result<&mut Self, TimelineError> {
        for event in events {
            self.ingest(event)?;
        }
        Ok(self)
    }

    /// Number of accepted (non-duplicate) events held so far.
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// True when no event has been accepted yet.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Finalizes construction: orders deterministically, runs recurrence reasoning,
    /// computes the timeline digest.
    pub fn build(mut self) -> Timeline {
        // Stable total order: (timestamp, semantic identity, source id). The source id
        // tiebreak keeps two distinct events sharing one timestamp in a fixed order.
        self.events.sort_by(|a, b| {
            (
                a.observed_unix_ms,
                &a.semantic_identity_sha256,
                &a.source_id,
            )
                .cmp(&(
                    b.observed_unix_ms,
                    &b.semantic_identity_sha256,
                    &b.source_id,
                ))
        });
        let patterns = detect_recurrence_patterns(&self.events);
        let digest = timeline_digest(&self.events, &patterns);
        let watermark_unix_ms = self.watermark_unix_ms.unwrap_or_else(|| {
            self.events
                .last()
                .map(|event| event.observed_unix_ms)
                .unwrap_or(0)
        });
        Timeline {
            events: self.events,
            duplicates_collapsed: self.duplicates_collapsed,
            watermark_unix_ms,
            patterns,
            digest_sha256: digest,
        }
    }
}

/// Deterministic digest over sorted event content and emitted pattern content.
fn timeline_digest(events: &[TimelineEvent], patterns: &[RecurrencePattern]) -> String {
    let outcome_ordinal = |outcome: Outcome| match outcome {
        Outcome::Failed => 0u8,
        Outcome::Neutral => 1,
        Outcome::Succeeded => 2,
    };
    let mut hasher = Sha256::new();
    hasher.update((events.len() as u64).to_le_bytes());
    for event in events {
        hasher.update(event.semantic_identity_sha256.as_bytes());
        hasher.update(event.source_id.as_bytes());
        hasher.update(event.observed_unix_ms.to_le_bytes());
        hasher.update([outcome_ordinal(event.outcome)]);
    }
    hasher.update((patterns.len() as u64).to_le_bytes());
    for pattern in patterns {
        hasher.update(pattern.semantic_identity_sha256.as_bytes());
        hasher.update((pattern.occurrence_count as u64).to_le_bytes());
        hasher.update(pattern.first_observed_unix_ms.to_le_bytes());
        hasher.update(pattern.last_observed_unix_ms.to_le_bytes());
        hasher.update(pattern.mean_gap_ms.to_le_bytes());
        hasher.update([confidence_ordinal(pattern.confidence)]);
    }
    hex_lower(&hasher.finalize())
}

fn confidence_ordinal(confidence: RecurrenceConfidence) -> u8 {
    match confidence {
        RecurrenceConfidence::Weak => 1,
        RecurrenceConfidence::Moderate => 2,
        RecurrenceConfidence::Strong => 3,
    }
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

/// Groups failure-shaped events by semantic identity and emits a pattern per group
/// whose full evidence matrix is satisfiable:
///
/// 1. at least [`MIN_OCCURRENCES_FOR_PATTERN`] distinct occurrences;
/// 2. strictly increasing observation times (causal-ordering evidence);
/// 3. every consecutive gap inside `[0, MAX_RECURRENCE_GAP_MS]` — negative distances
///    (unsorted/hostile clocks) and over-window distances disqualify the whole run,
///    so hostile timestamps never become evidence.
///
/// A group failing ANY clause emits nothing. Confidence classes:
/// - Strong: >= 5 occurrences AND every pair of consecutive gaps within ±25 % of the mean;
/// - Moderate: >= 4 occurrences AND every gap within ±50 % of the mean;
/// - Weak: minimum-occurrence run with irregular spacing.
///
/// Detection correlates repetition over time only; nothing here asserts causation.
fn detect_recurrence_patterns(events: &[TimelineEvent]) -> Vec<RecurrencePattern> {
    let mut groups: HashMap<&str, Vec<&TimelineEvent>> = HashMap::new();
    for event in events {
        if event.outcome != Outcome::Failed {
            continue;
        }
        groups
            .entry(event.semantic_identity_sha256.as_str())
            .or_default()
            .push(event);
    }

    let mut patterns: Vec<RecurrencePattern> = groups
        .into_iter()
        .filter_map(|(identity, group)| build_pattern(identity, group))
        .collect();

    patterns.sort_by(|a, b| {
        (
            a.semantic_identity_sha256.as_str(),
            a.first_observed_unix_ms,
        )
            .cmp(&(
                b.semantic_identity_sha256.as_str(),
                b.first_observed_unix_ms,
            ))
    });
    patterns.truncate(MAX_PATTERNS);
    patterns
}

/// Attempts to assemble one pattern from an occurrence group. Returns `None` whenever
/// the evidence matrix is incomplete — partial patterns are never emitted.
fn build_pattern(identity: &str, mut group: Vec<&TimelineEvent>) -> Option<RecurrencePattern> {
    // Causal-ordering evidence: occurrences must arrive strictly ascending in time.
    group.sort_by_key(|event| event.observed_unix_ms);

    let occurrence_count = group.len();
    if occurrence_count < MIN_OCCURRENCES_FOR_PATTERN {
        return None;
    }

    // Hostile-timestamp guard: negative or over-window distances disqualify the run.
    let gaps: Vec<i64> = group
        .windows(2)
        .map(|pair| {
            pair[1]
                .observed_unix_ms
                .saturating_sub(pair[0].observed_unix_ms)
        })
        .collect();
    if gaps
        .iter()
        .any(|gap| *gap > MAX_RECURRENCE_GAP_MS || *gap < 0)
    {
        return None;
    }

    let first_observed_unix_ms = group[0].observed_unix_ms;
    let last_observed_unix_ms = group[occurrence_count - 1].observed_unix_ms;
    let gap_sum: i64 = gaps.iter().sum();
    let mean_gap_ms = gap_sum / gaps.len() as i64;

    // Regularity classification against the mean distance.
    let max_deviation_permille: i64 = gaps
        .iter()
        .map(|gap| {
            if mean_gap_ms == 0 {
                if *gap == 0 { 0 } else { i64::MAX }
            } else {
                (gap.abs_diff(mean_gap_ms) as i64 * 1000) / mean_gap_ms
            }
        })
        .max()
        .unwrap_or(i64::MAX);

    let confidence = if occurrence_count >= 5 && max_deviation_permille <= 250 {
        RecurrenceConfidence::Strong
    } else if occurrence_count >= 4 && max_deviation_permille <= 500 {
        RecurrenceConfidence::Moderate
    } else {
        RecurrenceConfidence::Weak
    };

    // Bounded citation list: first and last always cited, middle evenly sampled.
    let evidence = bounded_evidence(&group);

    let first = group[0];
    Some(RecurrencePattern {
        semantic_identity_sha256: identity.to_string(),
        class: first.class,
        domain: first.domain.clone(),
        code: first.code.clone(),
        confidence,
        occurrence_count,
        first_observed_unix_ms,
        last_observed_unix_ms,
        mean_gap_ms,
        evidence,
    })
}

/// Evenly samples at most [`MAX_PATTERN_EVIDENCE`] citations from the run, always
/// including the first and last occurrence. Pure arithmetic — no randomness.
fn bounded_evidence(group: &[&TimelineEvent]) -> Vec<PatternEvidence> {
    let count = group.len();
    let step = if count <= MAX_PATTERN_EVIDENCE {
        1usize
    } else {
        (count + MAX_PATTERN_EVIDENCE - 1) / MAX_PATTERN_EVIDENCE
    };
    let mut indices: Vec<usize> = (0..count).step_by(step).collect();
    if indices.last() != Some(&(count - 1)) {
        if indices.len() >= MAX_PATTERN_EVIDENCE {
            indices.pop();
        }
        indices.push(count - 1);
    }

    indices
        .into_iter()
        .enumerate()
        .map(|(position, index)| {
            let observed_unix_ms = group[index].observed_unix_ms;
            let gap_from_previous_ms = if position == 0 {
                0
            } else {
                observed_unix_ms.saturating_sub(group[index - step].observed_unix_ms)
            };
            PatternEvidence {
                source_id: group[index].source_id.clone(),
                observed_unix_ms,
                gap_from_previous_ms,
            }
        })
        .collect()
}
