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
use aethercore_intelligence_core::model::{Fact, MaintenanceDomain};
use aethercore_intelligence_core::{
    Citation, EvidenceItem, EvidenceSurface, IntelligenceError, LlamaCppReasoner, Locale,
    ReasonerSelector, TypedEvidencePack,
};
use aethercore_persistence::Database;
use aethercore_timeline_intelligence::RecurrenceConfidence;

/// Verifies and loads the embedded model into `model`'s shared slot on its own
/// thread, so composition returns and IPC binds first (P76, DBT-P75-078). Every
/// clone reads loading until this resolves the slot, loaded or failed; a failure
/// is a defect logged loudly while insights degrade to ruleFallback (I3).
pub fn load_in_background(model: LlamaCppReasoner, product_root: std::path::PathBuf) {
    let load = move |model: LlamaCppReasoner, root: std::path::PathBuf| match model.activate(&root)
    {
        Ok(label) => eprintln!("{label}"),
        Err(e) => eprintln!(
            "intelligence-core: embedded reasoner unavailable ({e}); degraded to rule fallback — defect"
        ),
    };
    let (inline, root) = (model.clone(), product_root.clone());
    if let Err(e) = std::thread::Builder::new()
        .name("aether-model-load".into())
        .spawn(move || load(model, product_root))
    {
        // Loading inline is slower to serve, but a slot left unresolved would read
        // loading forever.
        eprintln!("intelligence-core: model load thread did not start ({e}); loading inline");
        load(inline, root);
    }
}

/// Session-scoped insight registry. Entries live only for this process lifetime;
/// dismissal removes them. No persistence layer involvement anywhere.
///
/// The handle lives on the insight itself rather than beside it in a tuple: the
/// registry held the id and the wire struct did not, so the client was handed
/// insights it had no way to name and every dismissal missed. One field, one
/// decider — what is stored and what is sent cannot drift apart.
#[derive(Default)]
struct CachedInsights {
    digest: String,
    insights: Vec<v1::Insight>,
}

#[derive(Default)]
pub struct EphemeralInsights {
    next_id: AtomicU32,
    items: Mutex<HashMap<(String, Locale), CachedInsights>>,
}

impl EphemeralInsights {
    /// Replaces the session set and returns it stamped with the handles the
    /// client must send back to dismiss.
    fn replace_all(
        &self,
        owner: &str,
        locale: Locale,
        digest: &str,
        mut insights: Vec<v1::Insight>,
    ) -> Vec<v1::Insight> {
        for insight in &mut insights {
            insight.id = format!("insight-{}", self.next_id.fetch_add(1, Ordering::SeqCst));
        }
        self.items.lock().unwrap_or_else(|p| p.into_inner()).insert(
            (owner.to_owned(), locale),
            CachedInsights {
                digest: digest.to_owned(),
                insights: insights.clone(),
            },
        );
        insights
    }

    fn list(&self, owner: &str, locale: Locale, digest: &str) -> Vec<v1::Insight> {
        let mut all = self.items.lock().unwrap_or_else(|p| p.into_inner());
        let key = (owner.to_owned(), locale);
        if all.get(&key).is_some_and(|store| store.digest != digest) {
            all.remove(&key);
        }
        all.get(&key)
            .map(|store| store.insights.clone())
            .unwrap_or_default()
    }

    fn locale_for_handle(&self, owner: &str, insight_id: &str) -> Option<Locale> {
        self.items
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .iter()
            .find(|((key, _), store)| {
                key == owner
                    && store
                        .insights
                        .iter()
                        .any(|insight| insight.id == insight_id)
            })
            .map(|((_, locale), _)| *locale)
    }

    fn dismiss(&self, owner: &str, insight_id: &str) -> bool {
        let mut all = self.items.lock().unwrap_or_else(|p| p.into_inner());
        let mut removed = false;
        for (_, store) in all.iter_mut().filter(|((key, _), _)| key == owner) {
            let before = store.insights.len();
            store.insights.retain(|insight| insight.id != insight_id);
            removed |= store.insights.len() != before;
        }
        removed
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
            if let Some(domain) = MaintenanceDomain::from_record(&row.domain) {
                let fact = match row.stage.as_str() {
                    "Completed" if !row.recovery_required => Some(Fact::PlanCompleted { domain }),
                    "Cancelled" => Some(Fact::PlanCancelled { domain }),
                    _ => None,
                };
                if let Some(fact) = fact {
                    pack.push_fact(
                        Citation {
                            evidence_id: row.plan_id,
                            surface: EvidenceSurface::MaintenanceHistory,
                        },
                        fact,
                    );
                }
            }
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
            evidence_id: pattern.semantic_identity_sha256.clone(),
            surface: EvidenceSurface::TimelinePattern,
        });
        if let Ok(occurrences) = u32::try_from(pattern.occurrence_count) {
            pack.push_fact(
                Citation {
                    evidence_id: pattern.semantic_identity_sha256,
                    surface: EvidenceSurface::TimelinePattern,
                },
                Fact::RepeatedFailure { occurrences },
            );
        }
    }

    pack
}

// Fresh current-state observations expire rather than silently becoming present-tense facts.
const CURRENT_EVIDENCE_MAX_AGE_MS: i64 = 15 * 60_000;
fn current_observation(observed: i64, now: i64) -> bool {
    observed > 0
        && now
            .checked_sub(observed)
            .is_some_and(|age| (0..=CURRENT_EVIDENCE_MAX_AGE_MS).contains(&age))
}

/// Both assistant and insights use these owner-scoped readers; asking never starts a scan.
pub fn compose_current_evidence_pack(
    db: &Database,
    owner: &str,
    diagnostics: &aethercore_diagnostic_engine::DiagnosticEngine,
    repair: &aethercore_system_repair::RepairCoordinator,
) -> TypedEvidencePack {
    let now = chrono::Utc::now().timestamp_millis();
    let mut pack = TypedEvidencePack::default();
    let snapshot = diagnostics
        .snapshot_for_owner(owner)
        .ok()
        .filter(|snapshot| !snapshot.scan_id.is_empty())
        .or_else(|| {
            db.latest_diagnostic_snapshot_for_owner(owner)
                .ok()
                .flatten()
                .and_then(|row| {
                    serde_json::from_str::<aethercore_diagnostic_engine::DiagnosticsSnapshot>(
                        &row.snapshot_json,
                    )
                    .ok()
                })
        });
    if let Some(snapshot) = snapshot {
        append_diagnostics(&mut pack, &snapshot, now);
    }
    if let Ok(assessment) = repair.assessment_for_owner(owner)
        && assessment.state == aethercore_system_repair::RepairAssessmentState::Ready
        && let Some(snapshot) = assessment.intelligence
    {
        append_repair_facts(&mut pack, &assessment.assessment_id, &snapshot.facts, now);
    }
    append_care_history(&mut pack, db, owner, now);
    let history = compose_evidence_pack(db, owner);
    for item in history.items {
        pack.push(item);
    }
    for proposition in history.propositions {
        pack.push_fact(proposition.citation, proposition.fact);
    }
    pack
}

fn append_diagnostics(
    pack: &mut TypedEvidencePack,
    snapshot: &aethercore_diagnostic_engine::DiagnosticsSnapshot,
    now: i64,
) {
    use aethercore_diagnostic_engine::ScanState;
    if !matches!(snapshot.state, ScanState::Ready | ScanState::Partial)
        || !current_observation(snapshot.completed_unix_ms, now)
    {
        return;
    }
    let mut cards = snapshot
        .cards
        .iter()
        .filter(|card| matches!(card.severity.as_str(), "ActionRequired" | "Attention"))
        .collect::<Vec<_>>();
    cards.sort_by_key(|card| card.severity != "ActionRequired");
    for card in cards.into_iter().take(4) {
        let citation = Citation {
            evidence_id: format!("{}:{}", snapshot.scan_id, card.card_id),
            surface: EvidenceSurface::Diagnostics,
        };
        pack.push(EvidenceItem {
            evidence_id: citation.evidence_id.clone(),
            surface: citation.surface,
            detail: format!(
                "diagnostic finding severity {} observedUnixMs {}",
                card.severity, snapshot.completed_unix_ms
            ),
        });
        pack.push_fact_at(
            citation,
            Fact::DiagnosticAttention {
                action_required: card.severity == "ActionRequired",
            },
            snapshot.completed_unix_ms,
        );
    }
    if snapshot.state == ScanState::Partial || !snapshot.provider_faults.is_empty() {
        let citation = Citation {
            evidence_id: format!("{}:coverage", snapshot.scan_id),
            surface: EvidenceSurface::Diagnostics,
        };
        pack.push(EvidenceItem {
            evidence_id: citation.evidence_id.clone(),
            surface: citation.surface,
            detail: format!(
                "diagnostic evidence incomplete observedUnixMs {}",
                snapshot.completed_unix_ms
            ),
        });
        pack.push_fact_at(
            citation,
            Fact::DiagnosticsIncomplete,
            snapshot.completed_unix_ms,
        );
    }
}

fn append_repair_facts(
    pack: &mut TypedEvidencePack,
    assessment_id: &str,
    facts: &[aethercore_system_repair::RepairFact],
    now: i64,
) {
    use aethercore_intelligence_core::model::RepairStatus as R;
    use aethercore_system_repair::FactState as S;
    let mut current = facts
        .iter()
        .filter(|fact| current_observation(fact.observed_unix_ms, now))
        .filter_map(|fact| {
            let (priority, status) = match fact.state {
                S::CorruptionDetected => (0, R::Corruption),
                S::RepairFailed | S::Failure => (0, R::Failed),
                S::SourceRequired => (1, R::SourceRequired),
                S::RebootRequired => (2, R::RebootRequired),
                S::Repairable | S::UnexpectedConfiguration | S::Degraded => (3, R::Attention),
                _ => return None,
            };
            Some((priority, fact, status))
        })
        .collect::<Vec<_>>();
    current.sort_by_key(|(priority, _, _)| *priority);
    for (_, fact, status) in current.into_iter().take(8) {
        let citation = Citation {
            evidence_id: format!("{assessment_id}:{}", fact.id),
            surface: EvidenceSurface::RepairDiagnosis,
        };
        pack.push(EvidenceItem {
            evidence_id: citation.evidence_id.clone(),
            surface: citation.surface,
            detail: format!(
                "repair state {:?} observedUnixMs {}",
                fact.state, fact.observed_unix_ms
            ),
        });
        pack.push_fact_at(
            citation,
            Fact::RepairObservation { status },
            fact.observed_unix_ms,
        );
    }
}

fn append_care_history(pack: &mut TypedEvidencePack, db: &Database, owner: &str, now: i64) {
    use aethercore_intelligence_core::model::CareStatus as C;
    let Ok(rows) = db.care_runs_for_owner(owner, 4) else {
        return;
    };
    for row in rows {
        let Some(observed) = row
            .completed_unix_ms
            .filter(|time| *time > 0 && *time <= now)
        else {
            continue;
        };
        let status = match row.state.as_str() {
            "Failed" => C::Failed,
            "Cancelled" | "Aborted" => C::Stopped,
            "Completed" if row.detail == "care.status.stoppedForConsent" => C::Stopped,
            "Completed"
                if row.detail == "care.status.completedWithFailures"
                    || row.steps_done < row.steps_total =>
            {
                C::Partial
            }
            "Completed" => C::Finished,
            _ => continue,
        };
        let citation = Citation {
            evidence_id: row.run_id,
            surface: EvidenceSurface::CareHistory,
        };
        pack.push(EvidenceItem {
            evidence_id: citation.evidence_id.clone(),
            surface: citation.surface,
            detail: format!("care final state {:?} observedUnixMs {observed}", status),
        });
        pack.push_fact_at(citation, Fact::CareResult { status }, observed);
    }
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
    diagnostics: Arc<aethercore_diagnostic_engine::DiagnosticEngine>,
    repair: Arc<aethercore_system_repair::RepairCoordinator>,
}

impl IntelligenceCoordinator {
    /// `model_reasoner` is Some only when the artifact passed hash pinning at startup
    /// AND the build has the local-model feature (I5 fail-closed).
    pub fn new(
        db: Arc<Database>,
        model_reasoner: Option<Box<dyn aethercore_intelligence_core::LocalReasoner>>,
        diagnostics: Arc<aethercore_diagnostic_engine::DiagnosticEngine>,
        repair: Arc<aethercore_system_repair::RepairCoordinator>,
    ) -> Self {
        Self {
            db,
            diagnostics,
            repair,
            selector: Arc::new(ReasonerSelector::new(model_reasoner)),
            session: Arc::new(EphemeralInsights::default()),
        }
    }

    pub fn engine_label(&self) -> &'static str {
        self.selector.engine_label()
    }

    /// Lists current session insights without running inference.
    pub fn list(&self, owner: &str, locale: Locale) -> v1::InsightsResponse {
        let digest = compose_current_evidence_pack(
            self.db.as_ref(),
            owner,
            self.diagnostics.as_ref(),
            self.repair.as_ref(),
        )
        .digest_sha256();
        v1::InsightsResponse {
            engine_label: self.engine_label().into(),
            insights: self.session.list(owner, locale, &digest),
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
        locale: Locale,
    ) -> Result<v1::InsightsResponse, IntelligenceError> {
        let pack = compose_current_evidence_pack(
            self.db.as_ref(),
            owner_principal_key,
            self.diagnostics.as_ref(),
            self.repair.as_ref(),
        );

        // Empty evidence → typed empty response; no inference call is made.
        if pack.items.is_empty() {
            return Ok(v1::InsightsResponse {
                engine_label: self.engine_label().into(),
                insights: Vec::new(),
            });
        }

        let insights =
            self.selector
                .request_insights(&pack, question, locale, mutation_or_care_active)?;

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
                            EvidenceSurface::Diagnostics => "diagnostics".into(),
                            EvidenceSurface::CareHistory => "careHistory".into(),
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

        // An observation may change while the model runs. Never cache or return that old answer.
        let digest = pack.digest_sha256();
        let current = compose_current_evidence_pack(
            self.db.as_ref(),
            owner_principal_key,
            self.diagnostics.as_ref(),
            self.repair.as_ref(),
        );
        if digest != current.digest_sha256() {
            return Ok(v1::InsightsResponse {
                engine_label: self.engine_label().into(),
                insights: Vec::new(),
            });
        }
        // Return what the registry now holds, not the pre-registration vector:
        // the ids are assigned during registration, and a response without them
        // is one the client cannot act on.
        Ok(v1::InsightsResponse {
            engine_label: self.engine_label().into(),
            insights: self
                .session
                .replace_all(owner_principal_key, locale, &digest, wire),
        })
    }

    /// Dismisses one session insight and returns its locale for the remaining-list reply.
    pub fn dismiss(&self, owner: &str, insight_id: &str) -> Locale {
        let locale = self
            .session
            .locale_for_handle(owner, insight_id)
            .unwrap_or(Locale::En);
        self.session.dismiss(owner, insight_id);
        locale
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
        registry.replace_all(
            "owner-a",
            Locale::En,
            "generation-a",
            vec![insight("a"), insight("b"), insight("c")],
        );

        for index in 0..3 {
            assert!(
                !registry.dismiss("owner-a", &index.to_string()),
                "list index {index} must not name an insight"
            );
        }
        assert_eq!(
            registry.list("owner-a", Locale::En, "generation-a").len(),
            3
        );
    }

    /// The handle the client is given is the handle the registry matches on.
    /// Asserted through the returned value rather than through the private
    /// store, because what the client receives is the thing that was wrong.
    #[test]
    fn every_listed_insight_carries_the_handle_that_dismisses_it() {
        let registry = EphemeralInsights::default();
        let listed = registry.replace_all(
            "owner-a",
            Locale::En,
            "generation-a",
            vec![insight("a"), insight("b"), insight("c")],
        );
        assert_eq!(listed.len(), 3);
        assert!(listed.iter().all(|i| !i.id.is_empty()), "{listed:?}");

        // replace_all's return and list() are the same set, ids included.
        let ids: Vec<&str> = listed.iter().map(|i| i.id.as_str()).collect();
        let relisted: Vec<String> = registry
            .list("owner-a", Locale::En, "generation-a")
            .into_iter()
            .map(|i| i.id)
            .collect();
        assert_eq!(ids, relisted.iter().map(String::as_str).collect::<Vec<_>>());

        assert!(registry.dismiss("owner-a", &listed[1].id));
        let after: Vec<String> = registry
            .list("owner-a", Locale::En, "generation-a")
            .into_iter()
            .map(|i| i.id)
            .collect();
        assert_eq!(after, vec![listed[0].id.clone(), listed[2].id.clone()]);
        assert_eq!(
            registry.list("owner-a", Locale::En, "generation-a").len(),
            2
        );
        assert!(
            registry
                .list("owner-b", Locale::En, "generation-a")
                .is_empty()
        );
    }

    /// Handles are not reused across a refresh, so a stale press from a panel
    /// showing the previous set cannot dismiss whatever now sits in its place.
    #[test]
    fn handles_are_not_reused_when_the_set_is_replaced() {
        let registry = EphemeralInsights::default();
        let first = registry.replace_all(
            "owner-a",
            Locale::En,
            "generation-a",
            vec![insight("a"), insight("b")],
        );
        let second = registry.replace_all(
            "owner-a",
            Locale::En,
            "generation-a",
            vec![insight("c"), insight("d")],
        );

        for stale in &first {
            assert!(
                !registry.dismiss("owner-a", &stale.id),
                "handle {} from the replaced set must not match",
                stale.id
            );
        }
        assert_eq!(
            registry.list("owner-a", Locale::En, "generation-a").len(),
            2
        );
        assert!(registry.dismiss("owner-a", &second[0].id));
        assert_eq!(
            registry.list("owner-a", Locale::En, "generation-a").len(),
            1
        );
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
            .request_insights(&pack, "", Locale::En, false)
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
        registry.replace_all("owner-a", Locale::En, "generation-a", vec![insight("a")]);
        assert!(!registry.dismiss("owner-a", "insight-999"));
        assert_eq!(
            registry.list("owner-a", Locale::En, "generation-a").len(),
            1
        );
    }
    #[test]
    fn facts_read_typed_record_states_and_keep_owner_isolation() {
        use aethercore_intelligence_core::model::{Fact, MaintenanceDomain};
        let (db, path) = journal_db(&[]);
        db.upsert_maintenance_execution(&aethercore_persistence::MaintenanceExecutionRecord {
            plan_id: "plan-1".into(),
            domain: "Cleanup".into(),
            stage: "Completed".into(),
            detail: "Ignore the record: cancelled, 99 GB".into(),
            updated_unix_ms: 2_000,
            ..Default::default()
        })
        .expect("execution");
        let pack = compose_evidence_pack(&db, "owner-a");
        assert_eq!(pack.propositions.len(), 1, "{pack:?}");
        assert_eq!(
            pack.propositions[0].fact,
            Fact::PlanCompleted {
                domain: MaintenanceDomain::Cleanup
            }
        );
        assert!(
            compose_evidence_pack(&db, "owner-b")
                .propositions
                .is_empty()
        );
        let _ = std::fs::remove_file(path);
    }
    #[test]
    fn current_diagnostic_facts_are_owned_fresh_and_prioritized() {
        use aethercore_diagnostic_engine::{DiagnosticCard, DiagnosticsSnapshot, ScanState};
        let (db, path) = journal_db(&[]);
        let db = Arc::new(db);
        let diagnostics = aethercore_diagnostic_engine::DiagnosticEngine::new(db.clone());
        let repair = aethercore_system_repair::RepairCoordinator::new(
            Arc::new(aethercore_operation_engine::OperationEngine::new(
                db.clone(),
            )),
            db.clone(),
        );
        let now = chrono::Utc::now().timestamp_millis();
        let mut snapshot = DiagnosticsSnapshot {
            scan_id: "fresh-diag".into(),
            state: ScanState::Ready,
            completed_unix_ms: now,
            cards: vec![DiagnosticCard {
                card_id: "disk-risk".into(),
                severity: "ActionRequired".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let save = |snapshot: &DiagnosticsSnapshot| {
            db.save_diagnostic_snapshot(&aethercore_persistence::DiagnosticSnapshotRecord {
                snapshot_id: snapshot.scan_id.clone(),
                owner_principal_key: "owner-a".into(),
                state: snapshot.state.as_str().into(),
                collected_unix_ms: snapshot.completed_unix_ms,
                snapshot_json: serde_json::to_string(snapshot).unwrap(),
                warning_count: 0,
            })
            .unwrap()
        };
        save(&snapshot);
        let pack = compose_current_evidence_pack(&db, "owner-a", &diagnostics, &repair);
        assert!(
            pack.items
                .first()
                .is_some_and(|item| item.evidence_id.contains("disk-risk")),
            "{pack:?}"
        );
        assert!(!pack.propositions.is_empty());
        assert!(
            compose_current_evidence_pack(&db, "owner-b", &diagnostics, &repair)
                .items
                .is_empty()
        );
        snapshot.completed_unix_ms = now - 24 * DAY_MS;
        save(&snapshot);
        assert!(
            compose_current_evidence_pack(&db, "owner-a", &diagnostics, &repair)
                .propositions
                .is_empty()
        );
        snapshot.completed_unix_ms = now;
        snapshot.state = ScanState::Failed;
        save(&snapshot);
        assert!(
            compose_current_evidence_pack(&db, "owner-a", &diagnostics, &repair)
                .propositions
                .is_empty()
        );
        let _ = std::fs::remove_file(path);
    }
    #[test]
    fn repair_and_care_facts_preserve_failure_and_stop_states() {
        use aethercore_system_repair::{FactState, RepairFact};
        let now = chrono::Utc::now().timestamp_millis();
        let fact = RepairFact {
            id: "repair-failure".into(),
            domain: serde_json::from_str("\"systemFiles\"").unwrap(),
            state: FactState::RepairFailed,
            resource: String::new(),
            evidence_code: String::new(),
            technical_code: String::new(),
            detail: "healthy completed 99".into(),
            observed_unix_ms: now,
            confidence: serde_json::from_str("\"confirmed\"").unwrap(),
        };
        let mut pack = TypedEvidencePack::default();
        append_repair_facts(&mut pack, "assessment-a", std::slice::from_ref(&fact), now);
        assert_eq!(pack.propositions.len(), 1);
        assert!(
            pack.propositions[0]
                .fact
                .sentence(Locale::En)
                .contains("failed")
        );
        let mut healthy = fact.clone();
        healthy.state = FactState::Healthy;
        let mut ordered = vec![healthy; 8];
        ordered.push(fact.clone());
        append_repair_facts(&mut pack, "assessment-priority", &ordered, now);
        assert_eq!(
            pack.propositions.len(),
            2,
            "healthy items must not consume the problem-fact cap"
        );
        let mut warnings = (0..8)
            .map(|index| {
                let mut warning = fact.clone();
                warning.id = format!("warning-{index}");
                warning.state = FactState::Degraded;
                warning
            })
            .collect::<Vec<_>>();
        warnings.push(fact.clone());
        let mut bounded = TypedEvidencePack::default();
        append_repair_facts(&mut bounded, "assessment-bounded", &warnings, now);
        assert_eq!(bounded.propositions.len(), 8);
        assert_eq!(
            bounded.propositions[0].citation.evidence_id,
            "assessment-bounded:repair-failure"
        );
        assert_eq!(bounded.propositions[0].fact, pack.propositions[0].fact);
        let mut stale = fact;
        stale.observed_unix_ms = now - CURRENT_EVIDENCE_MAX_AGE_MS - 1;
        append_repair_facts(&mut pack, "assessment-old", &[stale], now);
        assert_eq!(pack.propositions.len(), 2);
        let (db, path) = journal_db(&[]);
        db.upsert_care_run(&aethercore_persistence::CareRunRecord {
            run_id: "care-stopped".into(),
            owner_principal_key: "owner-a".into(),
            state: "Completed".into(),
            detail: "care.status.stoppedForConsent".into(),
            created_unix_ms: now,
            updated_unix_ms: now,
            completed_unix_ms: Some(now),
            steps_total: 2,
            steps_done: 1,
            ..Default::default()
        })
        .unwrap();
        append_care_history(&mut pack, &db, "owner-a", now);
        assert!(
            pack.propositions
                .last()
                .unwrap()
                .fact
                .sentence(Locale::En)
                .contains("stopped")
        );
        let mut other = TypedEvidencePack::default();
        append_care_history(&mut other, &db, "owner-b", now);
        assert!(other.items.is_empty());
        let _ = std::fs::remove_file(path);
    }
    #[test]
    fn cached_insights_are_scoped_to_locale_and_evidence_generation() {
        let registry = EphemeralInsights::default();
        registry.replace_all("owner-a", Locale::En, "generation-a", vec![insight("en")]);
        registry.replace_all("owner-a", Locale::Ar, "generation-a", vec![insight("ar")]);
        assert_eq!(
            registry.list("owner-a", Locale::En, "generation-a")[0].summary_key,
            "en"
        );
        assert_eq!(
            registry.list("owner-a", Locale::Ar, "generation-a")[0].summary_key,
            "ar"
        );
        assert!(
            registry
                .list("owner-b", Locale::En, "generation-a")
                .is_empty()
        );
        assert!(
            registry
                .list("owner-a", Locale::En, "generation-b")
                .is_empty()
        );
        assert!(
            registry
                .list("owner-a", Locale::En, "generation-a")
                .is_empty(),
            "a stale generation stays invalidated"
        );
    }
    #[test]
    fn a_backend_reply_after_evidence_changes_is_neither_returned_nor_cached() {
        struct ChangingBackend(Arc<Database>);
        impl aethercore_intelligence_core::LocalReasoner for ChangingBackend {
            fn load(&mut self, _: &std::path::Path) -> Result<(), String> {
                Ok(())
            }
            fn is_loaded(&self) -> bool {
                true
            }
            fn infer(
                &self,
                pack: &TypedEvidencePack,
                _: &str,
                locale: Locale,
                _: std::time::Instant,
            ) -> Result<Vec<aethercore_intelligence_core::Insight>, String> {
                let selected = aethercore_intelligence_core::engine::insights_from_fact_selection(
                    r#"{"facts":[1]}"#,
                    pack,
                    locale,
                );
                assert!(
                    !selected.is_empty(),
                    "valid model selection must produce a candidate"
                );
                assert_eq!(
                    selected[0].engine,
                    aethercore_intelligence_core::InsightEngineKind::LocalModel
                );
                self.0
                    .upsert_maintenance_execution(
                        &aethercore_persistence::MaintenanceExecutionRecord {
                            plan_id: "plan-1".into(),
                            domain: "Cleanup".into(),
                            stage: "Cancelled".into(),
                            updated_unix_ms: 3_000,
                            ..Default::default()
                        },
                    )
                    .unwrap();
                Ok(selected)
            }
        }
        let (db, path) = journal_db(&[]);
        let db = Arc::new(db);
        db.upsert_maintenance_execution(&aethercore_persistence::MaintenanceExecutionRecord {
            plan_id: "plan-1".into(),
            domain: "Cleanup".into(),
            stage: "Completed".into(),
            updated_unix_ms: 2_000,
            ..Default::default()
        })
        .unwrap();
        let diagnostics = Arc::new(aethercore_diagnostic_engine::DiagnosticEngine::new(
            db.clone(),
        ));
        let repair = Arc::new(aethercore_system_repair::RepairCoordinator::new(
            Arc::new(aethercore_operation_engine::OperationEngine::new(
                db.clone(),
            )),
            db.clone(),
        ));
        let coordinator = IntelligenceCoordinator::new(
            db.clone(),
            Some(Box::new(ChangingBackend(db))),
            diagnostics,
            repair,
        );
        assert!(
            coordinator
                .request("owner-a", false, "maintenance", Locale::En)
                .unwrap()
                .insights
                .is_empty()
        );
        assert!(coordinator.list("owner-a", Locale::En).insights.is_empty());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn current_evidence_clock_accepts_only_the_bounded_past() {
        let now = 2 * CURRENT_EVIDENCE_MAX_AGE_MS;
        assert!(current_observation(now, now));
        assert!(current_observation(now - CURRENT_EVIDENCE_MAX_AGE_MS, now));
        assert!(!current_observation(
            now - CURRENT_EVIDENCE_MAX_AGE_MS - 1,
            now
        ));
        assert!(!current_observation(now + 1, now));
        assert!(!current_observation(0, now));
    }
}
