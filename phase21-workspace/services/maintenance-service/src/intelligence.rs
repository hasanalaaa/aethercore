//! Phase 23 — service-side intelligence coordinator (advisory-only, ephemeral).
//!
//! - Evidence packs are composed ONLY from public read APIs of existing domains
//!   (bottleneck report roles, repair diagnoses, timeline patterns, maintenance
//!   history) and are bounded.
//! - Insights are EPHEMERAL session state: never persisted, never republished as
//!   facts; the persisted history tables remain untouched by this module.
//! - Observer-effect guard (I4): inference is refused while a mutation or care run
//!   is active; on-demand only, never periodic.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use aethercore_contracts::v1;
use aethercore_intelligence_core::{
    EvidenceItem, EvidenceSurface, LlamaCppReasoner, LocalReasoner, ReasonerSelector,
    TypedEvidencePack,
};
use aethercore_persistence::Database;

/// True when the embedded model verified and loaded at startup. When false, the panel
/// shows the honest degraded-mode chip (ruleFallback) until the fault clears (I3) —
/// in the packaged product this is a defect condition, not a supported mode.
pub static EMBEDDED_ENGINE_ACTIVE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Session-scoped insight registry. Entries live only for this process lifetime;
/// dismissal removes them. No persistence layer involvement anywhere.
///
/// The handle lives on the insight itself rather than beside it in a tuple: the
/// registry held the id and the wire struct did not, so the client was handed
/// insights it had no way to name and every dismissal missed. One field, one
/// decider — what is stored and what is sent cannot drift apart.
#[derive(Default)]
pub struct EphemeralInsights {
    next_id: AtomicU32,
    items: Mutex<HashMap<String, Vec<v1::Insight>>>,
}

impl EphemeralInsights {
    /// Replaces the session set and returns it stamped with the handles the
    /// client must send back to dismiss.
    fn replace_all(&self, owner: &str, insights: Vec<v1::Insight>) -> Vec<v1::Insight> {
        let mut all = self.items.lock().unwrap_or_else(|p| p.into_inner());
        let store = all.entry(owner.to_owned()).or_default();
        store.clear();
        for mut insight in insights {
            insight.id = format!("insight-{}", self.next_id.fetch_add(1, Ordering::SeqCst));
            store.push(insight);
        }
        store.clone()
    }

    fn list(&self, owner: &str) -> Vec<v1::Insight> {
        self.items
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(owner)
            .cloned()
            .unwrap_or_default()
    }

    fn dismiss(&self, owner: &str, insight_id: &str) -> bool {
        let mut all = self.items.lock().unwrap_or_else(|p| p.into_inner());
        let Some(store) = all.get_mut(owner) else {
            return false;
        };
        let before = store.len();
        store.retain(|insight| insight.id != insight_id);
        store.len() != before
    }

    fn clear(&self, owner: &str) {
        self.items
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(owner);
    }
}

/// The bounded evidence pack, composed from PUBLIC read APIs of existing domains
/// and nothing else (I1/I4).
///
/// Extracted in P56 because the assistant answers from the SAME pack the insight
/// path summarises. Two compositions would mean two answers to "what has this
/// product measured?", and the whole grounding contract rests on there being one.
pub fn compose_evidence_pack(db: &Database, owner_principal_key: &str) -> TypedEvidencePack {
    let mut pack = TypedEvidencePack::default();

    // Surface 1: latest maintenance history rows (existing executions/plans).
    if let Ok(rows) = db.maintenance_executions_for_owner(owner_principal_key, 8) {
        for row in rows.into_iter().take(8) {
            pack.push(EvidenceItem {
                evidence_id: row.plan_id.clone(),
                surface: EvidenceSurface::MaintenanceHistory,
                detail: format!(
                    "plan {} domain {} stage {}",
                    row.plan_id,
                    if row.domain.is_empty() {
                        "n/a"
                    } else {
                        &row.domain
                    },
                    if row.stage.is_empty() {
                        "n/a"
                    } else {
                        &row.stage
                    }
                ),
            });
        }
    }

    // Surface 2: recent timeline events as history evidence (bounded read).
    if let Ok(timeline) =
        aethercore_timeline_intelligence::ingest::ingest_owner_history(db, owner_principal_key)
    {
        for event in timeline.into_iter().take(12) {
            pack.push(EvidenceItem {
                evidence_id: event.semantic_identity_sha256.clone(),
                surface: EvidenceSurface::TimelinePattern,
                detail: format!("event class {:?} code {}", event.class, event.code),
            });
        }
    }

    pack
}

pub struct IntelligenceCoordinator {
    db: Arc<Database>,
    selector: Arc<ReasonerSelector>,
    pub session: Arc<EphemeralInsights>,
}

impl IntelligenceCoordinator {
    /// `model_reasoner` is Some only when the artifact passed hash pinning at startup
    /// AND the build has the local-model feature (I5 fail-closed).
    pub fn new(
        db: Arc<Database>,
        model_reasoner: Option<Box<dyn aethercore_intelligence_core::LocalReasoner>>,
    ) -> Self {
        Self {
            db,
            selector: Arc::new(ReasonerSelector::new(model_reasoner)),
            session: Arc::new(EphemeralInsights::default()),
        }
    }

    /// Phase 23.1 startup sequence (M2): locate artifact → sha256 vs pinned+manifest →
    /// RAM budget → load within budget → typed log line. The embedded reasoner is the
    /// PERMANENT default; failure here is a defect logged loudly while insights degrade
    /// to ruleFallback (I3).
    pub fn activate_embedded_default(&mut self, product_root: &std::path::Path) {
        use std::sync::atomic::Ordering;
        let model_path =
            product_root.join(aethercore_intelligence_core::EMBEDDED_MODEL_RELATIVE_PATH);
        match aethercore_intelligence_core::verify_model_hash(
            &model_path,
            &aethercore_intelligence_core::embedded_model_entry(),
        ) {
            Ok(()) => {
                let mut reasoner = LlamaCppReasoner::new();
                match reasoner.load(&model_path) {
                    Ok(()) => {
                        self.selector = Arc::new(ReasonerSelector::new(Some(Box::new(reasoner))));
                        EMBEDDED_ENGINE_ACTIVE.store(true, Ordering::SeqCst);
                        // Typed startup log line (contract M2).
                        eprintln!(
                            "intelligence-core: embedded reasoner active (model=qwen2.5-1.5b-instruct-q4_k_m, sha256 ok)"
                        );
                    }
                    Err(e) => {
                        eprintln!(
                            "intelligence-core: embedded reasoner FAILED to load ({e}); degraded to rule fallback — defect"
                        );
                        EMBEDDED_ENGINE_ACTIVE.store(false, Ordering::SeqCst);
                    }
                }
            }
            Err(e) => {
                eprintln!(
                    "intelligence-core: embedded artifact verification FAILED ({e}); degraded to rule fallback — defect"
                );
                EMBEDDED_ENGINE_ACTIVE.store(false, Ordering::SeqCst);
            }
        }
    }

    pub fn engine_label(&self) -> &'static str {
        self.selector.engine_label()
    }

    /// Lists current session insights without running inference.
    pub fn list(&self, owner: &str) -> v1::InsightsResponse {
        v1::InsightsResponse {
            engine_label: self.engine_label().into(),
            insights: self.session.list(owner),
        }
    }

    /// Composes a bounded evidence pack from PUBLIC read APIs and runs advisory
    /// inference. `mutation_or_care_active` comes from the router's knowledge of
    /// machine state — the observer-effect guard is enforced here too (defense in
    /// depth with the selector's own check).
    pub fn request(
        &self,
        owner_principal_key: &str,
        mutation_or_care_active: bool,
        question: &str,
    ) -> Result<v1::InsightsResponse, String> {
        let pack = compose_evidence_pack(self.db.as_ref(), owner_principal_key);

        // Empty evidence → typed empty response; no inference call is made.
        if pack.items.is_empty() {
            return Ok(v1::InsightsResponse {
                engine_label: self.engine_label().into(),
                insights: Vec::new(),
            });
        }

        let insights = self
            .selector
            .request_insights(&pack, question, mutation_or_care_active)
            .map_err(|error| error.to_string())?;

        // Convert typed domain insights to wire structs (same vocabulary, I1).
        let wire: Vec<v1::Insight> = insights
            .iter()
            .map(|insight| v1::Insight {
                // Stamped by replace_all below; the registry is the one decider.
                id: String::new(),
                schema_version: insight.schema_version,
                summary_key: insight.summary_key.clone(),
                explanation: insight.explanation.clone(),
                confidence: match insight.confidence {
                    aethercore_intelligence_core::InsightConfidence::Weak => "Weak".into(),
                    aethercore_intelligence_core::InsightConfidence::Moderate => "Moderate".into(),
                    aethercore_intelligence_core::InsightConfidence::Strong => "Strong".into(),
                },
                citations: insight
                    .citations
                    .iter()
                    .map(|c| v1::InsightCitation {
                        evidence_id: c.evidence_id.clone(),
                        surface: match c.surface {
                            EvidenceSurface::BottleneckReport => "bottleneckReport".into(),
                            EvidenceSurface::RepairDiagnosis => "repairDiagnosis".into(),
                            EvidenceSurface::TimelinePattern => "timelinePattern".into(),
                            EvidenceSurface::MaintenanceHistory => "maintenanceHistory".into(),
                            EvidenceSurface::SecurityFinding => "securityFinding".into(),
                        },
                    })
                    .collect(),
                engine: match insight.engine {
                    aethercore_intelligence_core::InsightEngineKind::LocalModel => {
                        "localModel".into()
                    }
                    aethercore_intelligence_core::InsightEngineKind::RuleFallback => {
                        "ruleFallback".into()
                    }
                },
            })
            .collect();

        // Return what the registry now holds, not the pre-registration vector:
        // the ids are assigned during registration, and a response without them
        // is one the client cannot act on.
        Ok(v1::InsightsResponse {
            engine_label: self.engine_label().into(),
            insights: self.session.replace_all(owner_principal_key, wire),
        })
    }

    /// Dismisses one session insight. Returns true when it existed.
    pub fn dismiss(&self, owner: &str, insight_id: &str) -> bool {
        self.session.dismiss(owner, insight_id)
    }

    /// Clears session insights (e.g., when a care run starts — observer hygiene).
    pub fn clear_session(&self, owner: &str) {
        self.session.clear(owner);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn insight(summary: &str) -> v1::Insight {
        v1::Insight {
            id: String::new(),
            schema_version: 1,
            summary_key: summary.into(),
            explanation: "e".into(),
            confidence: "Moderate".into(),
            citations: vec![v1::InsightCitation {
                evidence_id: "fact-0".into(),
                surface: "repairDiagnosis".into(),
            }],
            engine: "ruleFallback".into(),
        }
    }

    /// The defect this registry shipped with, written as the assertion that
    /// would have caught it: the client was handed insights carrying no handle,
    /// so the only value it could send was the list index, and the registry
    /// matched on `insight-N`. Every Dismiss press was a no-op with no error.
    #[test]
    fn a_dismissal_by_list_index_removes_nothing() {
        let registry = EphemeralInsights::default();
        registry.replace_all("owner-a", vec![insight("a"), insight("b"), insight("c")]);

        for index in 0..3 {
            assert!(
                !registry.dismiss("owner-a", &index.to_string()),
                "list index {index} must not name an insight"
            );
        }
        assert_eq!(registry.list("owner-a").len(), 3);
    }

    /// The handle the client is given is the handle the registry matches on.
    /// Asserted through the returned value rather than through the private
    /// store, because what the client receives is the thing that was wrong.
    #[test]
    fn every_listed_insight_carries_the_handle_that_dismisses_it() {
        let registry = EphemeralInsights::default();
        let listed =
            registry.replace_all("owner-a", vec![insight("a"), insight("b"), insight("c")]);
        assert_eq!(listed.len(), 3);
        assert!(listed.iter().all(|i| !i.id.is_empty()), "{listed:?}");

        // replace_all's return and list() are the same set, ids included.
        let ids: Vec<&str> = listed.iter().map(|i| i.id.as_str()).collect();
        let relisted: Vec<String> = registry.list("owner-a").into_iter().map(|i| i.id).collect();
        assert_eq!(ids, relisted.iter().map(String::as_str).collect::<Vec<_>>());

        assert!(registry.dismiss("owner-a", &listed[1].id));
        let after: Vec<String> = registry.list("owner-a").into_iter().map(|i| i.id).collect();
        assert_eq!(after, vec![listed[0].id.clone(), listed[2].id.clone()]);
        assert_eq!(registry.list("owner-a").len(), 2);
        assert!(registry.list("owner-b").is_empty());
    }

    /// Handles are not reused across a refresh, so a stale press from a panel
    /// showing the previous set cannot dismiss whatever now sits in its place.
    #[test]
    fn handles_are_not_reused_when_the_set_is_replaced() {
        let registry = EphemeralInsights::default();
        let first = registry.replace_all("owner-a", vec![insight("a"), insight("b")]);
        let second = registry.replace_all("owner-a", vec![insight("c"), insight("d")]);

        for stale in &first {
            assert!(
                !registry.dismiss("owner-a", &stale.id),
                "handle {} from the replaced set must not match",
                stale.id
            );
        }
        assert_eq!(registry.list("owner-a").len(), 2);
        assert!(registry.dismiss("owner-a", &second[0].id));
        assert_eq!(registry.list("owner-a").len(), 1);
    }

    #[test]
    fn dismissing_an_unknown_handle_is_reported_rather_than_swallowed() {
        let registry = EphemeralInsights::default();
        registry.replace_all("owner-a", vec![insight("a")]);
        assert!(!registry.dismiss("owner-a", "insight-999"));
        assert_eq!(registry.list("owner-a").len(), 1);
    }
}
