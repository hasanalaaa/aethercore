//! Phase 23 — insight engine: citation enforcement, fallback, selector, budgets.
//!
//! I3 (deterministic fallback): if the model runtime is unavailable/slow/unparseable,
//! [`DeterministicFallbackReasoner`] builds rule-based summary insights from the SAME
//! evidence pack. Fallback output satisfies I2 identically.
//!
//! I4 (resource budget): single in-flight request via an internal supervisor lane
//! (NOT MutationWorkload), hard inference timeout, on-demand only.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::model::{
    Citation, EvidenceItem, Insight, InsightConfidence, InsightEngineKind, MAX_INSIGHTS_PER_CALL,
    TypedEvidencePack,
};

/// Hard inference ceiling (I4). The model path must return inside this budget or the
/// selector degrades to fallback for that call.
pub const INFERENCE_TIMEOUT: Duration = Duration::from_secs(10);

/// Requested model RAM budget constant (I4). The loader refuses models whose manifest
/// declares more than this.
pub const MAX_MODEL_RAM_BUDGET_BYTES: u64 = 2 * 1024 * 1024 * 1024; // 2 GiB

/// Errors are typed and advisory-scoped: none of them can imply a mutation failure.
#[derive(Debug, thiserror::Error)]
pub enum IntelligenceError {
    #[error("another inference request is already in flight")]
    Busy,
    #[error("inference is paused while a care run or mutation is active")]
    MutationActive,
    #[error("no evidence is available for this request")]
    EmptyEvidence,
    #[error("model runtime unavailable: {0}")]
    ModelUnavailable(String),
}

/// The reasoner contract (B1). Implementations: LlamaCppReasoner (feature-gated) and
/// DeterministicFallbackReasoner (always available).
pub trait LocalReasoner: Send + Sync {
    fn load(&mut self, model_path: &std::path::Path) -> Result<(), String>;
    fn is_loaded(&self) -> bool;
    /// Returns raw candidate insights; the ENGINE enforces citations before emission.
    fn infer(
        &self,
        pack: &TypedEvidencePack,
        question: &str,
        deadline: std::time::Instant,
    ) -> Result<Vec<Insight>, String>;
}

/// Deterministic rule-based reasoner over structured evidence roles (I3).
///
/// Rules read ONLY the typed fields of the pack: bottleneck role codes, repair
/// verification states, recurrence pattern counts. Output is stable for identical
/// packs (byte-identical explanation strings).
pub struct DeterministicFallbackReasoner {
    loaded: bool,
}

impl DeterministicFallbackReasoner {
    pub fn new() -> Self {
        Self { loaded: true }
    }
}

impl Default for DeterministicFallbackReasoner {
    fn default() -> Self {
        Self::new()
    }
}

impl LocalReasoner for DeterministicFallbackReasoner {
    fn load(&mut self, _model_path: &std::path::Path) -> Result<(), String> {
        self.loaded = true;
        Ok(())
    }

    fn is_loaded(&self) -> bool {
        self.loaded
    }

    fn infer(
        &self,
        pack: &TypedEvidencePack,
        question: &str,
        _deadline: std::time::Instant,
    ) -> Result<Vec<Insight>, String> {
        let _ = question; // fallback summarizes; it does not answer open questions
        let mut out = Vec::new();

        // Rule 1: dominant bottleneck role summary.
        let bottleneck_ids: Vec<&EvidenceItem> = pack
            .items
            .iter()
            .filter(|item| {
                matches!(
                    item.surface,
                    crate::model::EvidenceSurface::BottleneckReport
                )
            })
            .collect();
        if !bottleneck_ids.is_empty() {
            let anchor = bottleneck_ids[0];
            if let Some(insight) = Insight::build(
                "insight.summary.bottleneck",
                format!(
                    "{} performance finding(s) reported; primary: {}.",
                    bottleneck_ids.len(),
                    anchor.evidence_id
                ),
                InsightConfidence::Moderate,
                bottleneck_ids
                    .iter()
                    .take(4)
                    .map(|item| Citation {
                        evidence_id: item.evidence_id.clone(),
                        surface: item.surface,
                    })
                    .collect(),
                InsightEngineKind::RuleFallback,
            ) {
                out.push(insight);
            }
        }

        // Rule 2: repair verification state summary.
        let repair_items: Vec<&EvidenceItem> = pack
            .items
            .iter()
            .filter(|item| matches!(item.surface, crate::model::EvidenceSurface::RepairDiagnosis))
            .collect();
        if !repair_items.is_empty() {
            if let Some(insight) = Insight::build(
                "insight.summary.repairState",
                format!(
                    "{} repair diagnosis entr(ies) present with their own verification states.",
                    repair_items.len()
                ),
                InsightConfidence::Weak,
                repair_items
                    .iter()
                    .take(4)
                    .map(|item| Citation {
                        evidence_id: item.evidence_id.clone(),
                        surface: item.surface,
                    })
                    .collect(),
                InsightEngineKind::RuleFallback,
            ) {
                out.push(insight);
            }
        }

        // Rule 3: recurrence pattern summary.
        let pattern_items: Vec<&EvidenceItem> = pack
            .items
            .iter()
            .filter(|item| matches!(item.surface, crate::model::EvidenceSurface::TimelinePattern))
            .collect();
        if pattern_items.len() >= 1 {
            if let Some(insight) = Insight::build(
                "insight.summary.recurrence",
                format!(
                    "{} recurring pattern(s) detected by timeline intelligence with full evidence matrices.",
                    pattern_items.len()
                ),
                InsightConfidence::Strong,
                pattern_items
                    .iter()
                    .take(4)
                    .map(|item| Citation {
                        evidence_id: item.evidence_id.clone(),
                        surface: item.surface,
                    })
                    .collect(),
                InsightEngineKind::RuleFallback,
            ) {
                out.push(insight);
            }
        }

        // Rule 4 (Phase 32): security-finding posture summary. Cites the
        // SecFinding evidence ids verbatim; the surface stays citation-
        // resolvable through the audit report lane.
        let security_items: Vec<&EvidenceItem> = pack
            .items
            .iter()
            .filter(|item| matches!(item.surface, crate::model::EvidenceSurface::SecurityFinding))
            .collect();
        if !security_items.is_empty()
            && let Some(insight) = Insight::build(
                "insight.summary.securityPosture",
                format!(
                    "{} security finding(s) in the current posture snapshot; review the cited evidence.",
                    security_items.len()
                ),
                InsightConfidence::Moderate,
                security_items
                    .iter()
                    .take(4)
                    .map(|item| Citation {
                        evidence_id: item.evidence_id.clone(),
                        surface: item.surface,
                    })
                    .collect(),
                InsightEngineKind::RuleFallback,
            )
        {
            out.push(insight);
        }

        Ok(out)
    }
}

/// Budget-aware selection + health tracking (B1). Labels every emitted insight with
/// the engine that served it (renderer badge source of truth).
pub struct ReasonerSelector {
    /// Optional on-device model reasoner (feature-flag gated at construction).
    model_reasoner: Option<Box<dyn LocalReasoner>>,
    fallback: DeterministicFallbackReasoner,
    /// Single in-flight inference lane (I4) — NOT a mutation workload.
    in_flight: AtomicBool,
    health: Mutex<SelectorHealth>,
}

#[derive(Default)]
struct SelectorHealth {
    served_by_model: u32,
    served_by_fallback: u32,
    last_mode: Option<InsightEngineKind>,
}

impl ReasonerSelector {
    /// Builds a selector. Since Phase 23.1 the model reasoner is provided whenever the
    /// embedded artifact verified at startup; `None` only ever reflects a runtime fault
    /// path (I3), never a supported default configuration.
    pub fn new(model: Option<Box<dyn LocalReasoner>>) -> Self {
        Self {
            model_reasoner: model,
            fallback: DeterministicFallbackReasoner::new(),
            in_flight: AtomicBool::new(false),
            health: Mutex::new(SelectorHealth::default()),
        }
    }

    pub fn engine_label(&self) -> &'static str {
        if self.model_reasoner.as_ref().is_some_and(|r| r.is_loaded()) {
            "localModel"
        } else {
            "ruleFallback"
        }
    }

    pub fn stats(&self) -> (u32, u32) {
        let health = self.health.lock().unwrap_or_else(|p| p.into_inner());
        (health.served_by_model, health.served_by_fallback)
    }

    fn record(&self, engine: InsightEngineKind) {
        let mut health = self.health.lock().unwrap_or_else(|p| p.into_inner());
        match engine {
            InsightEngineKind::LocalModel => health.served_by_model += 1,
            InsightEngineKind::RuleFallback => health.served_by_fallback += 1,
        }
        health.last_mode = Some(engine);
    }

    /// On-demand inference with observer-effect guard (I4): refuses while a mutation
    /// flag is raised, enforces single-flight, degrades to fallback on any model-path
    /// failure, and drops unresolvable citations before returning (I2).
    pub fn request_insights(
        &self,
        pack: &TypedEvidencePack,
        question: &str,
        mutation_active: bool,
    ) -> Result<Vec<Insight>, IntelligenceError> {
        if mutation_active {
            return Err(IntelligenceError::MutationActive);
        }
        if pack.items.is_empty() {
            return Err(IntelligenceError::EmptyEvidence);
        }
        // Single-flight CAS: compare_exchange as the lane token.
        if self
            .in_flight
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err(IntelligenceError::Busy);
        }
        let result = self.dispatch(pack, question);
        self.in_flight.store(false, Ordering::SeqCst);
        result
    }

    fn dispatch(
        &self,
        pack: &TypedEvidencePack,
        question: &str,
    ) -> Result<Vec<Insight>, IntelligenceError> {
        let deadline = std::time::Instant::now() + INFERENCE_TIMEOUT;

        let mut candidates: Vec<Insight> = Vec::new();
        let mut used_engine = InsightEngineKind::RuleFallback;

        if let Some(model) = &self.model_reasoner {
            if model.is_loaded() {
                match model.infer(pack, question, deadline) {
                    Ok(mut insights) if !insights.is_empty() => {
                        candidates.clear();
                        candidates.append(&mut insights);
                        used_engine = InsightEngineKind::LocalModel;
                    }
                    _ => {
                        // Unavailable / unparseable / empty → silent degrade (I3).
                        candidates.clear();
                    }
                }
            }
        }

        if candidates.is_empty() {
            candidates = self
                .fallback
                .infer(pack, question, deadline)
                .unwrap_or_default();
            used_engine = InsightEngineKind::RuleFallback;
        }

        // I2 gate: drop anything whose citations do not resolve against THIS pack;
        // relabel surviving insights with the engine that actually served them.
        let mut emitted: Vec<Insight> = Vec::new();
        for mut insight in candidates {
            let all_resolve =
                insight.citations.iter().all(|c| pack.resolves(c)) && !insight.citations.is_empty();
            if !all_resolve || insight.schema_version != crate::model::INSIGHT_SCHEMA_V1 {
                continue;
            }
            insight.engine = used_engine;
            emitted.push(insight);
            if emitted.len() >= MAX_INSIGHTS_PER_CALL {
                break;
            }
        }

        if !emitted.is_empty() {
            self.record(used_engine);
        } else {
            // Even zero-insight outcomes count as fallback service when the model failed.
            self.record(used_engine);
        }
        Ok(emitted)
    }
}
