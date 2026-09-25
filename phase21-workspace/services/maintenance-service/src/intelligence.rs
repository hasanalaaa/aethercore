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
    EvidenceItem, EvidenceSurface, LlamaCppReasoner, ReasonerSelector, TypedEvidencePack,
};
use aethercore_persistence::Database;
use aethercore_timeline_intelligence::RecurrenceConfidence;

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

    // Surface 2: recurrence patterns the timeline engine DETECTED, most recent
    // first. P75: this used to push the first 12 raw ingested events here, in
    // per-table order, as `TimelinePattern` — so journal transitions that never
    // recurred were cited as patterns, the same identity appeared once per row,
    // and the rule engine announced "recurring pattern(s) … with full evidence
    // matrices" at Strong over them. A pattern is what `TimelineBuilder` emits
    // after its evidence-matrix checks, and nothing else.
    let mut patterns = recurrence_patterns(db, owner_principal_key);
    patterns.sort_by_key(|pattern| std::cmp::Reverse(pattern.last_observed_unix_ms));
    for pattern in patterns.into_iter().take(12) {
        pack.push(EvidenceItem {
            detail: format!(
                "recurring failure: class {} domain {} code {}, {} occurrences, recurrence confidence {}",
                pattern.class.as_str(),
                pattern.domain,
                pattern.code,
                pattern.occurrence_count,
                match pattern.confidence {
                    RecurrenceConfidence::Weak => "weak",
                    RecurrenceConfidence::Moderate => "moderate",
                    RecurrenceConfidence::Strong => "strong",
                }
            ),
            evidence_id: pattern.semantic_identity_sha256,
            surface: EvidenceSurface::TimelinePattern,
        });
    }

    pack
}

/// The owner's detected recurrence patterns, built the way the timeline page
/// builds them (`timeline.rs`): the service's own clock is the freshness
/// watermark, so a future-stamped row can neither order nor recur. An
/// unreadable history yields no patterns, as it yielded no events before.
fn recurrence_patterns(
    db: &Database,
    owner_principal_key: &str,
) -> Vec<aethercore_timeline_intelligence::RecurrencePattern> {
    let watermark = chrono::Utc::now().timestamp_millis();
    let Ok(candidates) =
        aethercore_timeline_intelligence::ingest::ingest_owner_history_with_watermark(
            db,
            owner_principal_key,
            watermark,
        )
    else {
        return Vec::new();
    };
    let mut builder = aethercore_timeline_intelligence::TimelineBuilder::new().watermark(watermark);
    if builder.ingest_all(candidates).is_err() {
        return Vec::new();
    }
    builder.build().patterns
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
    pub fn activate_embedded_default(
        &mut self,
        product_root: &std::path::Path,
    ) -> Option<LlamaCppReasoner> {
        use std::sync::atomic::Ordering;
        match aethercore_intelligence_core::activate_embedded_reasoner(product_root) {
            Ok((reasoner, label)) => {
                // Both clones hold the same model and generation gate.
                self.selector = Arc::new(ReasonerSelector::new(Some(Box::new(reasoner.clone()))));
                EMBEDDED_ENGINE_ACTIVE.store(true, Ordering::SeqCst);
                eprintln!("{label}");
                Some(reasoner)
            }
            Err(e) => {
                eprintln!(
                    "intelligence-core: embedded reasoner unavailable ({e}); degraded to rule fallback — defect"
                );
                EMBEDDED_ENGINE_ACTIVE.store(false, Ordering::SeqCst);
                None
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

    fn journal_db(transitions: &[(&str, i64)]) -> (Database, std::path::PathBuf) {
        let path =
            std::env::temp_dir().join(format!("aethercore-p75-pack-{}.db", uuid::Uuid::new_v4()));
        let db = Database::open(&path).expect("database");
        db.insert_plan(
            &aethercore_persistence::PlanRecord {
                id: "plan-1".into(),
                title: "seed".into(),
                state: "Completed".into(),
                digest: "digest-plan-1".into(),
                risk: "Low".into(),
                immutable_json: "{}".into(),
                created_unix_ms: 1_000,
                updated_unix_ms: 1_000,
                owner_principal_key: "owner-a".into(),
            },
            "created",
        )
        .expect("plan");
        for (state, at) in transitions {
            db.append_plan_event("plan-1", state, "state_transition", "", *at)
                .expect("journal row");
        }
        (db, path)
    }

    const DAY_MS: i64 = 24 * 60 * 60 * 1000;

    /// P75. Raw journal rows are events, not patterns. They used to enter the
    /// pack as `TimelinePattern`, so the rule engine announced "recurring
    /// pattern(s) detected by timeline intelligence with full evidence
    /// matrices" at Strong confidence over a machine where nothing recurred.
    #[test]
    fn raw_timeline_events_are_not_presented_as_recurring_patterns() {
        let (db, path) = journal_db(&[
            ("Executing", 2 * DAY_MS),
            ("Completed", 3 * DAY_MS),
            ("RecoveryRequired", 4 * DAY_MS),
        ]);
        let pack = compose_evidence_pack(&db, "owner-a");
        assert!(
            !pack
                .items
                .iter()
                .any(|item| item.surface == EvidenceSurface::TimelinePattern),
            "nothing recurred, yet the pack holds pattern evidence: {:?}",
            pack.items
        );
        let insights = ReasonerSelector::new(None)
            .request_insights(&pack, "", false)
            .unwrap_or_default();
        assert!(
            !insights
                .iter()
                .any(|insight| insight.summary_key == "insight.summary.recurrence"),
            "{insights:?}"
        );
        let _ = std::fs::remove_file(path);
    }

    /// And a failure that really recurs is cited under the pattern's own
    /// identity, with what the timeline engine measured about it.
    #[test]
    fn a_detected_recurrence_enters_the_pack_as_one_pattern() {
        let (db, path) = journal_db(&[
            ("RecoveryRequired", 2 * DAY_MS),
            ("RecoveryRequired", 3 * DAY_MS),
            ("RecoveryRequired", 4 * DAY_MS),
        ]);
        let pack = compose_evidence_pack(&db, "owner-a");
        let patterns: Vec<_> = pack
            .items
            .iter()
            .filter(|item| item.surface == EvidenceSurface::TimelinePattern)
            .collect();
        assert_eq!(patterns.len(), 1, "{:?}", pack.items);
        assert_eq!(
            patterns[0].evidence_id,
            aethercore_timeline_intelligence::semantic_identity(
                aethercore_timeline_intelligence::EventClass::Operation,
                "operationJournal",
                "journal.transition:RecoveryRequired",
            )
        );
        assert!(
            patterns[0].detail.contains("3 occurrences"),
            "{}",
            patterns[0].detail
        );
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn dismissing_an_unknown_handle_is_reported_rather_than_swallowed() {
        let registry = EphemeralInsights::default();
        registry.replace_all("owner-a", vec![insight("a")]);
        assert!(!registry.dismiss("owner-a", "insight-999"));
        assert_eq!(registry.list("owner-a").len(), 1);
    }
}
