//! Phase 21 — Timeline Intelligence domain model.
//!
//! Typed event model over already-persisted operations, findings, scans and repair
//! outcomes. Ingestion is read-only over `aethercore-persistence`: historical truth is
//! consumed as written and never rewritten (PCD-TIMELINE-INTELLIGENCE completionTruth).
//!
//! Precision rules carried over from the Phase 17.1 correlation discipline:
//! - Every emitted object cites its full evidence matrix; a rule that cannot fill its
//!   matrix emits nothing.
//! - Correlation is temporal-ordering evidence only. Nothing here asserts causation.
//! - Identical input produces a byte-identical timeline digest.

use serde::Serialize;
use sha2::{Digest, Sha256};

/// Hard upper bound on events retained by one constructed timeline. Ingestion stops
/// accepting beyond this count instead of growing without limit.
pub const MAX_TIMELINE_EVENTS: usize = 2000;

/// Hard upper bound on page size any caller may request.
pub const MAX_PAGE_SIZE: usize = 200;

/// Hard upper bound on recurrence patterns reported per timeline build.
pub const MAX_PATTERNS: usize = 64;

/// Hard upper bound on evidence references cited per pattern.
pub const MAX_PATTERN_EVIDENCE: usize = 32;

/// Minimum distinct occurrences before a recurrence pattern may be emitted at all.
pub const MIN_OCCURRENCES_FOR_PATTERN: usize = 3;

/// Maximum allowed time distance between consecutive occurrences of a pattern.
pub const MAX_RECURRENCE_GAP_MS: i64 = 30 * 24 * 60 * 60 * 1000;

/// Semantic identity classes for deduplication. Two events with equal semantic identity
/// describe the same fact and collapse into one timeline entry.
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
pub enum EventClass {
    /// A maintenance operation ran (plan lifecycle, install, repair, cleanup, startup).
    Operation,
    /// An assessment/finding/scan produced a conclusion about machine state.
    Finding,
    /// A verification step proved or disproved an expected post-condition.
    Verification,
    /// The operation journal recorded a recovery / reboot-resume path.
    Recovery,
    /// An escalation was recorded (failure escalated to a broader remediation).
    Escalation,
}

impl EventClass {
    pub fn as_str(self) -> &'static str {
        match self {
            EventClass::Operation => "operation",
            EventClass::Finding => "finding",
            EventClass::Verification => "verification",
            EventClass::Recovery => "recovery",
            EventClass::Escalation => "escalation",
        }
    }

    /// Stable ordinal used inside the semantic-identity digest so two different
    /// classes can never collide on the same key text.
    pub fn ordinal(self) -> u8 {
        match self {
            EventClass::Operation => 1,
            EventClass::Finding => 2,
            EventClass::Verification => 3,
            EventClass::Recovery => 4,
            EventClass::Escalation => 5,
        }
    }
}

/// Outcome polarity used by recurrence reasoning. Only failure-shaped outcomes feed
/// failure-recurrence detection; successes are timeline entries but never pattern input.
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
pub enum Outcome {
    Succeeded,
    Failed,
    Neutral,
}

/// One typed timeline event ingested from persisted history.
///
/// `observed_unix_ms` is the authoritative ordering timestamp. Events carrying future
/// timestamps relative to the build watermark are quarantined, not ordered.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TimelineEvent {
    /// Persistence-layer identifier of the source row (event id, scan id, plan id...).
    pub source_id: String,
    pub class: EventClass,
    /// Domain string exactly as persisted (never rewritten by this crate).
    pub domain: String,
    /// Action / rule / workload code exactly as persisted.
    pub code: String,
    pub outcome: Outcome,
    /// Authoritative ordering timestamp (Unix ms).
    pub observed_unix_ms: i64,
    /// Semantic identity: identical identities across occurrences deduplicate.
    /// Computed as sha256 over canonical (class, domain, code) content, excluding all
    /// volatile identifiers and timestamps — the same discipline as the Phase 17.1
    /// machine-state fingerprint.
    pub semantic_identity_sha256: String,
}

impl TimelineEvent {
    /// Builds an event and derives its semantic identity deterministically.
    pub fn new(
        source_id: impl Into<String>,
        class: EventClass,
        domain: &str,
        code: &str,
        outcome: Outcome,
        observed_unix_ms: i64,
    ) -> Self {
        let identity = semantic_identity(class, domain, code);
        Self {
            source_id: source_id.into(),
            class,
            domain: domain.to_string(),
            code: code.to_string(),
            outcome,
            observed_unix_ms,
            semantic_identity_sha256: identity,
        }
    }
}

/// Deterministic semantic-identity digest over the stable meaning of an event.
/// Timestamps, source identifiers and free-text details are intentionally excluded.
pub fn semantic_identity(class: EventClass, domain: &str, code: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update([class.ordinal()]);
    // Length-prefix each field so ("ab","c") and ("a","bc") cannot collide.
    hasher.update((domain.len() as u64).to_le_bytes());
    hasher.update(domain.as_bytes());
    hasher.update((code.len() as u64).to_le_bytes());
    hasher.update(code.as_bytes());
    hex_lower(&hasher.finalize())
}

/// Confidence classes for recurrence patterns. Ordered weakest to strongest.
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub enum RecurrenceConfidence {
    Weak,
    Moderate,
    Strong,
}

/// One piece of evidence cited by a recurrence pattern.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PatternEvidence {
    /// Source id of the occurrence this citation points at.
    pub source_id: String,
    /// Observed timestamp of the occurrence (Unix ms).
    pub observed_unix_ms: i64,
    /// Time distance in milliseconds from the previous occurrence (0 for the first).
    pub gap_from_previous_ms: i64,
}

/// A detected repetition of one semantic identity.
///
/// Emitted only when the full evidence matrix is available: enough occurrences, all
/// gaps inside the admissible window, strictly causal ordering of observations, and
/// no clock-skew violation. A rule lacking any part of its matrix emits nothing.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RecurrencePattern {
    /// The repeated semantic identity.
    pub semantic_identity_sha256: String,
    pub class: EventClass,
    pub domain: String,
    pub code: String,
    pub confidence: RecurrenceConfidence,
    /// Number of distinct occurrences backing the pattern.
    pub occurrence_count: usize,
    /// First and last observation of the run (Unix ms), strictly increasing.
    pub first_observed_unix_ms: i64,
    pub last_observed_unix_ms: i64,
    /// Mean inter-occurrence distance over the run (Unix ms).
    pub mean_gap_ms: i64,
    pub evidence: Vec<PatternEvidence>,
}

/// Errors surfaced by timeline construction. Typed enum — never stringly state.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TimelineError {
    #[error(
        "timestamp {observed_unix_ms} is in the future relative to watermark {watermark_unix_ms}"
    )]
    FutureTimestamp {
        observed_unix_ms: i64,
        watermark_unix_ms: i64,
    },
    #[error("timeline capacity exceeded: {limit} events")]
    CapacityExceeded { limit: usize },
}

/// A deterministic, bounded timeline over ingested events.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Timeline {
    /// All accepted events, ascending by (observed_unix_ms, semantic_identity_sha256,
    /// source_id) — a total order, so identical input yields byte-identical output.
    pub events: Vec<TimelineEvent>,
    /// Number of input rows rejected as exact duplicates of an already-ingested event.
    pub duplicates_collapsed: usize,
    /// Watermark supplied at construction; every event satisfies ts <= watermark.
    pub watermark_unix_ms: i64,
    /// Recurrence patterns whose full evidence matrices were satisfiable.
    pub patterns: Vec<RecurrencePattern>,
    /// Deterministic digest over the sorted event content plus pattern content.
    pub digest_sha256: String,
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
