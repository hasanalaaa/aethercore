//! Phase 21 — Timeline Intelligence & Recurrence Reasoning.
//!
//! Deterministic, bounded timeline over already-persisted operations, findings,
//! scans and repair outcomes, with evidence-complete recurrence detection. The
//! crate is read-only over `aethercore-persistence` (no schema change) and never
//! rewrites historical truth.

pub mod engine;
pub mod ingest;
pub mod model;

pub use engine::TimelineBuilder;
pub use model::{
    EventClass, MAX_PAGE_SIZE, MAX_PATTERN_EVIDENCE, MAX_PATTERNS, MAX_RECURRENCE_GAP_MS,
    MAX_TIMELINE_EVENTS, MIN_OCCURRENCES_FOR_PATTERN, Outcome, PatternEvidence,
    RecurrenceConfidence, RecurrencePattern, Timeline, TimelineError, TimelineEvent,
    semantic_identity,
};

/// Re-exported for service-layer convenience; ingestion itself lives in [`ingest`].
pub use aethercore_persistence as persistence;
