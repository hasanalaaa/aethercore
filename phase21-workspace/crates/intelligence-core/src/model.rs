//! Phase 23 — Embedded Local Intelligence Core (advisory-only) domain model.
//!
//! INVARIANT I1 (ADVISORY-ONLY): this crate's entire output vocabulary is
//! [`Insight`]. It carries summary keys, explanations over EXISTING evidence ids,
//! confidence, and citations. Nothing here can express a mutation, an action kind,
//! a registry write, a file operation, or process control: the crate does not
//! depend on any mutation surface (compile-level proof, see Cargo.toml + audit).
//!
//! INVARIANT I2 (MANDATORY CITATIONS): every insight references at least one
//! [`Citation`] resolving against the evidence pack it was built from. The engine
//! drops unresolvable insights before they ever leave this crate.
//!
//! INVARIANT I3/I4 types live in `selector`/`fallback`; schema versioning here.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Wire/prompt schema version. Unknown fields are rejected on parse.
pub const INSIGHT_SCHEMA_V1: u32 = 1;

/// Upper bound on evidence items per pack (I4 bounded size).
pub const MAX_EVIDENCE_ITEMS: usize = 64;

/// Upper bound on insights per inference call.
pub const MAX_INSIGHTS_PER_CALL: usize = 8;

/// Confidence class for an insight. Same precision discipline as Phase 17.1:
/// no numeric pseudo-precision; a class only when its evidence supports it.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum InsightConfidence {
    Weak,
    Moderate,
    Strong,
}

/// A reference to EXISTING evidence the insight is grounded in.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Citation {
    /// Stable evidence handle: finding id / report digest / timeline pattern id.
    pub evidence_id: String,
    /// Which domain surface the handle belongs to.
    pub surface: EvidenceSurface,
}

/// Domain surfaces whose PUBLIC read APIs feed evidence packs.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub enum EvidenceSurface {
    /// Performance bottleneck report item (id = report digest + role).
    BottleneckReport,
    /// System-repair diagnosis entry (id = diagnosis code anchor).
    RepairDiagnosis,
    /// Timeline recurrence pattern (id = pattern semantic identity hash).
    TimelinePattern,
    /// Maintenance history row (id = plan/execution id).
    MaintenanceHistory,
}

/// One advisory insight. THE ONLY output type of the reasoner (I1).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Insight {
    pub schema_version: u32,
    /// Localized message key resolved in renderer catalogs (never raw prose identity).
    pub summary_key: String,
    /// Explanation composed strictly over cited evidence handles.
    pub explanation: String,
    pub confidence: InsightConfidence,
    /// ≥1 citation required; enforced at construction and again at emission.
    pub citations: Vec<Citation>,
    /// Which engine produced this (selector labels it; renderer shows the badge).
    pub engine: InsightEngineKind,
}

/// Which mode served an insight (I3 visibility without error surfaces).
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum InsightEngineKind {
    /// On-device model behind the feature flag.
    LocalModel,
    /// Deterministic rule-based fallback (always available).
    RuleFallback,
}

impl Insight {
    /// Constructs an insight only from resolvable inputs. Enforces I2 at the type
    /// boundary: zero citations → None; over-cap citations truncated deterministically.
    pub fn build(
        summary_key: impl Into<String>,
        explanation: impl Into<String>,
        confidence: InsightConfidence,
        mut citations: Vec<Citation>,
        engine: InsightEngineKind,
    ) -> Option<Self> {
        if citations.is_empty() {
            return None;
        }
        // Deterministic dedup by (surface, evidence_id).
        citations.sort_by(|a, b| (&a.surface, &a.evidence_id).cmp(&(&b.surface, &b.evidence_id)));
        citations.dedup();
        Some(Self {
            schema_version: INSIGHT_SCHEMA_V1,
            summary_key: summary_key.into(),
            explanation: explanation.into(),
            confidence,
            citations,
            engine,
        })
    }
}

/// Bounded, read-only evidence pack assembled from PUBLIC read APIs of existing
/// domains (I1/I4). Items are pre-typed structured data — never raw attacker
/// controlled prose (threat-model mitigation, see docs/phase23/ARCHITECTURE.md).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypedEvidencePack {
    pub items: Vec<EvidenceItem>,
}

/// One structured evidence item. `payload` is a bounded key:value summary rendered
/// from typed domain structs — no free-form host strings beyond bounded detail text.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceItem {
    pub evidence_id: String,
    pub surface: EvidenceSurface,
    /// Short bounded detail (≤256 chars enforced at ingestion).
    pub detail: String,
}

impl TypedEvidencePack {
    pub fn push(&mut self, item: EvidenceItem) {
        if self.items.len() >= MAX_EVIDENCE_ITEMS {
            return;
        }
        let mut item = item;
        if item.detail.len() > 256 {
            item.detail.truncate(253);
            item.detail.push_str("...");
        }
        self.items.push(item);
    }

    /// Resolves a citation handle against current pack content (I2).
    pub fn resolves(&self, citation: &Citation) -> bool {
        self.items.iter().any(|item| {
            item.evidence_id == citation.evidence_id && item.surface == citation.surface
        })
    }

    /// Deterministic pack digest for logging/echo (no PII beyond existing findings).
    pub fn digest_sha256(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update((self.items.len() as u64).to_le_bytes());
        for item in &self.items {
            hasher.update(item.evidence_id.as_bytes());
            hasher.update([match item.surface {
                EvidenceSurface::BottleneckReport => 0,
                EvidenceSurface::RepairDiagnosis => 1,
                EvidenceSurface::TimelinePattern => 2,
                EvidenceSurface::MaintenanceHistory => 3,
            }]);
            hasher.update(item.detail.as_bytes());
        }
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let bytes = hasher.finalize();
        bytes.iter().fold(String::new(), |mut out, &b| {
            out.push(HEX[(b >> 4) as usize] as char);
            out.push(HEX[(b & 0x0f) as usize] as char);
            out
        })
    }
}
