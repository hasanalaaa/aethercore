//! Phase 23 — Embedded Local Intelligence Core (air-gapped, advisory-only).
//!
//! Invariants enforced by construction + tests + audit gates:
//! I1 advisory-only output vocabulary ([`model::Insight`]) — no mutation surfaces in
//!    this crate's dependency tree;
//! I2 mandatory citations resolving against the current evidence pack (unresolvable
//!    insights are dropped before emission);
//! I3 deterministic fallback reasoner, silently engaged on any model-path failure;
//! I4 resource budgets: bounded pack, inference timeout, single-flight lane, observer-
//!    effect guard while mutations run;
//! I5 air-gapped: artifact manifest with pinned sha256, fail-closed loader, zero
//!    network dependencies (audit gate enforces).

pub mod engine;
pub mod llama;
pub mod model;

pub use engine::{
    DeterministicFallbackReasoner, INFERENCE_TIMEOUT, IntelligenceError, LocalReasoner,
    MAX_MODEL_RAM_BUDGET_BYTES, ReasonerSelector,
};
pub use llama::{LlamaCppReasoner, ModelManifestEntry, verify_model_hash};
pub use model::{
    Citation, EvidenceItem, EvidenceSurface, INSIGHT_SCHEMA_V1, Insight, InsightConfidence,
    InsightEngineKind, MAX_EVIDENCE_ITEMS, MAX_INSIGHTS_PER_CALL, TypedEvidencePack,
};
