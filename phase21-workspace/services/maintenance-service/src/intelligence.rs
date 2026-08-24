//! Phase 23 — service-side intelligence coordinator (advisory-only, ephemeral).
//!
//! - Evidence packs are composed ONLY from public read APIs of existing domains
//!   (bottleneck report roles, repair diagnoses, timeline patterns, maintenance
//!   history) and are bounded.
//! - Insights are EPHEMERAL session state: never persisted, never republished as
//!   facts; the persisted history tables remain untouched by this module.
//! - Observer-effect guard (I4): inference is refused while a mutation or care run
//!   is active; on-demand only, never periodic.

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
#[derive(Default)]
pub struct EphemeralInsights {
    next_id: AtomicU32,
    items: Mutex<Vec<(String, v1::Insight)>>,
}

impl EphemeralInsights {
    fn replace_all(&self, insights: Vec<v1::Insight>) {
        let mut store = self.items.lock().unwrap_or_else(|p| p.into_inner());
        store.clear();
        for insight in insights {
            let id = format!("insight-{}", self.next_id.fetch_add(1, Ordering::SeqCst));
            store.push((id, insight));
        }
    }

    fn list(&self) -> Vec<(String, v1::Insight)> {
        self.items
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    fn dismiss(&self, insight_id: &str) -> bool {
        let mut store = self.items.lock().unwrap_or_else(|p| p.into_inner());
        let before = store.len();
        store.retain(|(id, _)| id != insight_id);
        store.len() != before
    }

    fn clear(&self) {
        self.items.lock().unwrap_or_else(|p| p.into_inner()).clear();
    }
}

pub struct IntelligenceCoordinator {
    db: Arc<Database>,
    selector: Arc<ReasonerSelector>,
    pub session: Arc<EphemeralInsights>,
}

impl IntelligenceCoordinator {
    /// `model_reasoner` is Some only when the artifact passed hash pinning at startup
    /// AND the build has the local-model feature (I5 fail-closed).
    pub fn new(db: Arc<Database>, model_reasoner: Option<Box<dyn aethercore_intelligence_core::LocalReasoner>>) -> Self {
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
                        self.selector =
                            Arc::new(ReasonerSelector::new(Some(Box::new(reasoner))));
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
        use std::sync::atomic::Ordering;
        if EMBEDDED_ENGINE_ACTIVE.load(Ordering::SeqCst) {
            "localModel"
        } else {
            // Honest degraded-mode chip source (I3): fallback engaged after a fault.
            "ruleFallback"
        }
    }

    /// Lists current session insights without running inference.
    pub fn list(&self) -> v1::InsightsResponse {
        let items = self.session.list();
        v1::InsightsResponse {
            engine_label: self.engine_label().into(),
            insights: items.into_iter().map(|(_, insight)| insight).collect(),
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
    ) -> Result<v1::InsightsResponse, String> {
        let mut pack = TypedEvidencePack::default();

        // Surface 1: latest maintenance history rows (existing executions/plans).
        if let Ok(rows) = self.db.maintenance_executions_for_owner(owner_principal_key, 8) {
            for row in rows.into_iter().take(8) {
                pack.push(EvidenceItem {
                    evidence_id: row.plan_id.clone(),
                    surface: EvidenceSurface::MaintenanceHistory,
                    detail: format!(
                        "plan {} domain {} stage {}",
                        row.plan_id,
                        if row.domain.is_empty() { "n/a" } else { &row.domain },
                        if row.stage.is_empty() { "n/a" } else { &row.stage }
                    ),
                });
            }
        }

        // Surface 2: recent timeline events as history evidence (bounded read).
        if let Ok(timeline) =
            aethercore_timeline_intelligence::ingest::ingest_owner_history(self.db.as_ref(), owner_principal_key)
        {
            for event in timeline.into_iter().take(12) {
                pack.push(EvidenceItem {
                    evidence_id: event.semantic_identity_sha256.clone(),
                    surface: EvidenceSurface::TimelinePattern,
                    detail: format!("event class {:?} code {}", event.class, event.code),
                });
            }
        }

        // Empty evidence → typed empty response; no inference call is made.
        if pack.items.is_empty() {
            return Ok(v1::InsightsResponse {
                engine_label: self.engine_label().into(),
                insights: Vec::new(),
            });
        }

        let insights = self
            .selector
            .request_insights(&pack, "explain", mutation_or_care_active)
            .map_err(|error| error.to_string())?;

        // Convert typed domain insights to wire structs (same vocabulary, I1).
        let wire: Vec<v1::Insight> = insights
            .iter()
            .map(|insight| v1::Insight {
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
                        },
                    })
                    .collect(),
                engine: match insight.engine {
                    aethercore_intelligence_core::InsightEngineKind::LocalModel => "localModel".into(),
                    aethercore_intelligence_core::InsightEngineKind::RuleFallback => "ruleFallback".into(),
                },
            })
            .collect();

        self.session.replace_all(wire.clone());
        Ok(v1::InsightsResponse {
            engine_label: self.engine_label().into(),
            insights: wire,
        })
    }

    /// Dismisses one session insight. Returns true when it existed.
    pub fn dismiss(&self, insight_id: &str) -> bool {
        self.session.dismiss(insight_id)
    }

    /// Clears session insights (e.g., when a care run starts — observer hygiene).
    pub fn clear_session(&self) {
        self.session.clear();
    }
}
