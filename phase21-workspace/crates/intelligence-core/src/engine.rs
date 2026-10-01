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
    Citation, Insight, InsightConfidence, InsightEngineKind, Locale, MAX_INSIGHTS_PER_CALL,
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
    #[error("the on-device model is still loading")]
    ModelLoading,
}

/// The reasoner contract (B1). Implementations: LlamaCppReasoner (feature-gated) and
/// DeterministicFallbackReasoner (always available).
pub trait LocalReasoner: Send + Sync {
    fn load(&mut self, model_path: &std::path::Path) -> Result<(), String>;
    fn is_loaded(&self) -> bool;
    /// Whether the model is still loading; a request that needs it then answers
    /// loading rather than being served by the rule engine.
    fn is_loading(&self) -> bool {
        false
    }
    /// Returns candidate insights in `locale`; the engine accepts only exact
    /// owned fact templates with matching registered citations.
    fn infer(
        &self,
        pack: &TypedEvidencePack,
        question: &str,
        locale: Locale,
        deadline: std::time::Instant,
    ) -> Result<Vec<Insight>, String>;
}

/// Deterministic rule-based reasoner over structured evidence roles (I3).
///
/// Typed packs use the shared owned fact templates. Untyped packs prove only
/// the count/availability of evidence items. Neither path reads raw detail prose.
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
        locale: Locale,
        _deadline: std::time::Instant,
    ) -> Result<Vec<Insight>, String> {
        let _ = question;
        if !pack.propositions.is_empty() {
            let ids = (1..=pack.propositions.len().min(4))
                .map(|id| id.to_string())
                .collect::<Vec<_>>()
                .join(",");
            let mut out =
                insights_from_fact_selection(&format!("{{\"facts\":[{ids}]}}"), pack, locale);
            for insight in &mut out {
                insight.engine = InsightEngineKind::RuleFallback;
            }
            return Ok(out);
        }
        // Untyped observations prove availability/count only, never health, success or recurrence.
        let explanation = match locale {
            Locale::En => format!("{} evidence item(s) available", pack.items.len()),
            Locale::Ar => format!("عدد عناصر الأدلة المتاحة: {}", pack.items.len()),
        };
        Ok(Insight::build(
            "insight.summary.observation",
            explanation,
            InsightConfidence::Weak,
            pack.items
                .iter()
                .take(4)
                .map(|item| Citation {
                    evidence_id: item.evidence_id.clone(),
                    surface: item.surface,
                })
                .collect(),
            InsightEngineKind::RuleFallback,
        )
        .into_iter()
        .collect())
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
        if self.model_loading() {
            return "loading";
        }
        let health = self.health.lock().unwrap_or_else(|p| p.into_inner());
        match health.last_mode {
            Some(InsightEngineKind::LocalModel) => "localModel",
            Some(InsightEngineKind::RuleFallback) => "ruleFallback",
            None if self.model_reasoner.as_ref().is_some_and(|r| r.is_loaded()) => "localModel",
            None => "ruleFallback",
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
        locale: Locale,
        mutation_active: bool,
    ) -> Result<Vec<Insight>, IntelligenceError> {
        if mutation_active {
            return Err(IntelligenceError::MutationActive);
        }
        if pack.items.is_empty() {
            return Err(IntelligenceError::EmptyEvidence);
        }
        if self.model_loading() {
            return Err(IntelligenceError::ModelLoading);
        }
        let Some(_lane) = Lane::acquire(&self.in_flight) else {
            return Err(IntelligenceError::Busy);
        };
        self.dispatch(pack, question, locale)
    }

    fn model_loading(&self) -> bool {
        self.model_reasoner.as_ref().is_some_and(|r| r.is_loading())
    }

    fn dispatch(
        &self,
        pack: &TypedEvidencePack,
        question: &str,
        locale: Locale,
    ) -> Result<Vec<Insight>, IntelligenceError> {
        let deadline = std::time::Instant::now() + INFERENCE_TIMEOUT;

        if let Some(model) = &self.model_reasoner
            && model.is_loaded()
            && !pack.propositions.is_empty()
        {
            // Unavailable / busy / unparseable / over budget → silent degrade (I3).
            let cited = cited_only(
                pack,
                model
                    .infer(pack, question, locale, deadline)
                    .unwrap_or_default(),
            );
            let canonical = canonical_fact_candidates(pack, cited, locale);
            if !canonical.is_empty() && std::time::Instant::now() <= deadline {
                return Ok(self.serve(canonical, InsightEngineKind::LocalModel));
            }
        }

        // P75: decided AFTER the citation gate. It used to run only when the
        // model returned zero RAW candidates, so a model whose every insight
        // failed the gate left the user with nothing, recorded as served by the
        // model. Nothing cited survived, so the model served nothing.
        let cited = cited_only(
            pack,
            self.fallback
                .infer(pack, question, locale, deadline)
                .unwrap_or_default(),
        );
        Ok(self.serve(cited, InsightEngineKind::RuleFallback))
    }

    /// Labels what is emitted with the engine that actually served it, and
    /// records that engine for `engine_label`, the renderer badge's source.
    fn serve(&self, mut insights: Vec<Insight>, engine: InsightEngineKind) -> Vec<Insight> {
        for insight in &mut insights {
            insight.engine = engine;
        }
        self.record(engine);
        insights
    }
}

/// I2 gate: keeps only insights whose citations ALL resolve against THIS pack.
/// Converts strict model-selected IDs using the same product templates as the assistant.
pub fn insights_from_fact_selection(
    raw: &str,
    pack: &TypedEvidencePack,
    locale: Locale,
) -> Vec<Insight> {
    crate::assistant::selected_fact_ids(raw, pack)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|id| canonical_fact(&pack.propositions[id - 1], locale))
        .collect()
}

fn canonical_fact(proposition: &crate::model::Proposition, locale: Locale) -> Option<Insight> {
    Insight::build(
        "insight.summary.observation",
        proposition.fact.sentence(locale),
        InsightConfidence::Moderate,
        vec![proposition.citation.clone()],
        InsightEngineKind::LocalModel,
    )
}

fn canonical_fact_candidates(
    pack: &TypedEvidencePack,
    candidates: Vec<Insight>,
    locale: Locale,
) -> Vec<Insight> {
    candidates
        .into_iter()
        .filter_map(|candidate| {
            pack.propositions
                .iter()
                .find(|proposition| {
                    candidate.citations == [proposition.citation.clone()]
                        && candidate.explanation == proposition.fact.sentence(locale)
                })
                .and_then(|proposition| canonical_fact(proposition, locale))
        })
        .take(MAX_INSIGHTS_PER_CALL)
        .collect()
}

fn cited_only(pack: &TypedEvidencePack, candidates: Vec<Insight>) -> Vec<Insight> {
    candidates
        .into_iter()
        .filter(|insight| {
            insight.schema_version == crate::model::INSIGHT_SCHEMA_V1
                && !insight.citations.is_empty()
                && insight.citations.iter().all(|c| pack.resolves(c))
        })
        .take(MAX_INSIGHTS_PER_CALL)
        .collect()
}

/// A held single-flight lane (I4), released on drop. A reasoner that panics
/// mid-call therefore frees it on unwind; the plain store after the call that
/// this replaced was skipped by a panic, and every later request read `Busy`
/// until the service restarted.
pub(crate) struct Lane<'a>(&'a AtomicBool);

impl<'a> Lane<'a> {
    pub(crate) fn acquire(flag: &'a AtomicBool) -> Option<Self> {
        flag.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .ok()
            .map(|_| Self(flag))
    }
}

impl Drop for Lane<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}
