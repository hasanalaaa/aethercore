//! Phase 56 — the service side of the grounded assistant.
//!
//! The turn engine itself lives in `aethercore-intelligence-core` and owns no
//! I/O. This file is the part that touches the machine: it composes the evidence
//! pack from the SAME public read APIs the insight path uses, runs generation on
//! a worker thread so the request can return immediately, publishes the streamed
//! turn into the ordered event stream, and holds the cancel flags a user's
//! Escape reaches.
//!
//! Three rules this file is written to keep, all from `docs/phase56/DIRECTION.md`:
//!
//! 1. **`engine_label` never lies.** It reports what the engine IS —
//!    `localModel` when the artifact verified and loaded, `disabled` when it did
//!    not. A generation failure is a FAULTED turn carrying a reason key. It never
//!    labels a model failure as successful generation. P86 typed fact fallback
//!    is admitted without generation while disabled/loading, and answered turns
//!    explicitly report `ruleFallback`.
//! 2. **No turn ends in an empty answer.** Every path here terminates in one of
//!    ANSWERED / REFUSED / FAULTED / CANCELLED, and each carries its reason.
//! 3. **Cancellation stops generation**, rather than merely marking the turn
//!    cancelled after it has run to the ceiling.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use aethercore_contracts::v1::{self, EventKind};
use aethercore_intelligence_core::{
    ASSISTANT_SCHEMA_V1, AssistantEngine, MAX_QUESTION_CHARS, RefusalReason, StreamingReasoner,
    TurnOutcome, TypedEvidencePack,
};
use aethercore_operation_kernel::EventBus;
use aethercore_persistence::Database;

/// How often a streaming turn is published while generation runs.
///
/// Not per token: a 512-token answer would put 512 envelopes into the ordered
/// event stream, every one of them carrying the whole accumulated answer, and
/// the replay window is shared with every other domain in the product. At 120 ms
/// a fast answer publishes a handful of frames and a slow one still updates
/// faster than a reader can notice.
const STREAM_PUBLISH_INTERVAL: Duration = Duration::from_millis(120);

/// Turns are session state. They are never persisted, exactly as insights are
/// not: nothing the assistant says is republished as a fact.
#[derive(Default)]
struct LiveTurns {
    /// owner → turn id → the flag its generation loop reads.
    flags: Mutex<HashMap<String, HashMap<String, Arc<AtomicBool>>>>,
}

impl LiveTurns {
    fn register(&self, owner: &str, turn_id: &str) -> Arc<AtomicBool> {
        let flag = Arc::new(AtomicBool::new(false));
        self.flags
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .entry(owner.to_owned())
            .or_default()
            .insert(turn_id.to_owned(), Arc::clone(&flag));
        flag
    }

    /// Closes admission and captures cancellation under the same lock as `cancel`.
    /// A cancel accepted before removal must win; a later cancel returns false.
    fn finish(&self, owner: &str, turn_id: &str) -> bool {
        self.flags
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get_mut(owner)
            .and_then(|turns| turns.remove(turn_id))
            .is_some_and(|flag| flag.load(Ordering::SeqCst))
    }

    /// Raises the flag. Returns false when the turn is not in flight — which is
    /// not an error: a turn that already finished cannot be cancelled, and the
    /// caller says so with a state rather than a fault.
    fn cancel(&self, owner: &str, turn_id: &str) -> bool {
        match self
            .flags
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(owner)
            .and_then(|turns| turns.get(turn_id))
        {
            Some(flag) => {
                flag.store(true, Ordering::SeqCst);
                true
            }
            None => false,
        }
    }
}

pub struct AssistantCoordinator {
    db: Arc<Database>,
    engine: Arc<AssistantEngine>,
    live: Arc<LiveTurns>,
    diagnostics: Arc<aethercore_diagnostic_engine::DiagnosticEngine>,
    repair: Arc<aethercore_system_repair::RepairCoordinator>,
}

impl AssistantCoordinator {
    /// `reasoner` is `Some` only when the embedded artifact passed hash pinning
    /// at startup AND the build has the local-model feature (I5 fail-closed).
    pub fn new(
        db: Arc<Database>,
        reasoner: Option<Box<dyn StreamingReasoner>>,
        diagnostics: Arc<aethercore_diagnostic_engine::DiagnosticEngine>,
        repair: Arc<aethercore_system_repair::RepairCoordinator>,
    ) -> Self {
        Self {
            db,
            diagnostics,
            repair,
            engine: Arc::new(AssistantEngine::new(reasoner)),
            live: Arc::new(LiveTurns::default()),
        }
    }

    pub fn engine_label(&self) -> &'static str {
        self.engine.engine_label()
    }

    /// What the assistant can answer from, right now. The drawer's empty state
    /// is a count of these, not a list of capabilities.
    pub fn evidence_pack(&self, owner: &str) -> TypedEvidencePack {
        crate::intelligence::compose_current_evidence_pack(
            self.db.as_ref(),
            owner,
            self.diagnostics.as_ref(),
            self.repair.as_ref(),
        )
    }

    /// Starts a turn.
    ///
    /// Returns the turn the caller should hand back on the RPC: a terminal state
    /// when the answer was decided without generation (no evidence, a mutation
    /// holding the lease, no model), or `STREAMING` when a worker took it. In the
    /// streaming case exactly one terminal envelope follows on the event stream.
    pub fn ask(
        &self,
        owner: &str,
        turn_id: &str,
        question: &str,
        locale: aethercore_intelligence_core::Locale,
        mutation_active: bool,
        events: EventBus,
    ) -> v1::AssistantTurn {
        let pack = self.evidence_pack(owner);
        let label = self.engine_label();

        // Decided without the model. `ask` on the engine would reach the same
        // conclusions, but doing it here keeps the worker thread for work that
        // actually needs one.
        if mutation_active {
            return refused(turn_id, label, RefusalReason::MutationActive, &pack);
        }
        if pack.items.is_empty() {
            return refused(turn_id, label, RefusalReason::NoEvidence, &pack);
        }
        if label != "localModel" {
            // Use the existing bounded fact fallback without a reasoner. The loading
            // artifact could become ready here; never start inference on the RPC thread.
            let outcome = AssistantEngine::new(None).ask(
                &pack,
                question,
                locale,
                false,
                Arc::new(AtomicBool::new(false)),
                &mut |_| {},
            );
            let outcome = match outcome {
                TurnOutcome::Faulted { detail, .. } if label == "loading" => TurnOutcome::Faulted {
                    fault_key: aethercore_intelligence_core::assistant::FAULT_MODEL_LOADING,
                    detail,
                },
                other => other,
            };
            return terminal(turn_id, label, outcome, &pack);
        }

        let flag = self.live.register(owner, turn_id);
        let opening = streaming(turn_id, label, String::new(), 0, &pack);

        let engine = Arc::clone(&self.engine);
        let live = Arc::clone(&self.live);
        let owner_key = owner.to_owned();
        let turn = turn_id.to_owned();
        let question = question.to_owned();
        let pack_for_worker = pack.clone();
        let db = Arc::clone(&self.db);
        let diagnostics = Arc::clone(&self.diagnostics);
        let repair = Arc::clone(&self.repair);

        // A worker, not the request thread: generation is seconds of CPU, and an
        // RPC that blocks for it would hold an inflight slot and be indistinguishable
        // from a hung service.
        std::thread::Builder::new()
            .name("aether-assistant".into())
            .spawn(move || {
                let mut last_publish = Instant::now();
                let mut sink = |accumulated: &str| {
                    if last_publish.elapsed() < STREAM_PUBLISH_INTERVAL {
                        return;
                    }
                    last_publish = Instant::now();
                    publish(
                        &events,
                        &owner_key,
                        streaming(&turn, label, accumulated.to_owned(), 0, &pack_for_worker),
                    );
                };

                let outcome = engine.ask(
                    &pack_for_worker,
                    &question,
                    locale,
                    false,
                    Arc::clone(&flag),
                    &mut sink,
                );
                let current = crate::intelligence::compose_current_evidence_pack(
                    db.as_ref(),
                    &owner_key,
                    diagnostics.as_ref(),
                    repair.as_ref(),
                );
                let cancelled = live.finish(&owner_key, &turn);
                let outcome = if cancelled {
                    let tokens = match &outcome {
                        TurnOutcome::Answered { tokens, .. }
                        | TurnOutcome::Cancelled { tokens } => *tokens,
                        _ => 0,
                    };
                    TurnOutcome::Cancelled { tokens }
                } else if matches!(outcome, TurnOutcome::Answered { .. })
                    && pack_for_worker.digest_sha256() != current.digest_sha256()
                {
                    // Same owner, fresh facts: the old selection cannot authorize an answer.
                    TurnOutcome::Refused(RefusalReason::NotCovered)
                } else {
                    outcome
                };
                publish(
                    &events,
                    &owner_key,
                    terminal(&turn, label, outcome, &current),
                );
            })
            .map(|_| opening.clone())
            .unwrap_or_else(|error| {
                self.live.finish(owner, turn_id);
                faulted(
                    turn_id,
                    label,
                    aethercore_intelligence_core::assistant::FAULT_GENERATION_FAILED,
                    &pack,
                )
                .tap_detail(error.to_string())
            })
    }

    /// Raises the cancel flag for one turn. The generation loop checks it between
    /// tokens and terminates the turn `CANCELLED` with whatever it had.
    ///
    /// Returns whether a turn was in flight. A cancel for a turn that already
    /// finished is not an error — two windows can cancel the same turn.
    pub fn cancel(&self, owner: &str, turn_id: &str) -> bool {
        self.live.cancel(owner, turn_id)
    }
}

/// Small helper so the spawn-failure path can attach its reason without another
/// constructor. Not public: this is one call site.
trait TapDetail {
    fn tap_detail(self, detail: String) -> Self;
}
impl TapDetail for v1::AssistantTurn {
    fn tap_detail(mut self, detail: String) -> Self {
        self.answer = detail;
        self
    }
}

/// The pack as wire refs. `pub(crate)` because the router answers
/// `GetAssistantPack` with the same shape a turn carries — the drawer's empty
/// state and a turn's `pack` field must never disagree about what is held.
pub(crate) fn evidence_refs(pack: &TypedEvidencePack) -> Vec<v1::AssistantEvidenceRef> {
    pack.items
        .iter()
        .map(|item| v1::AssistantEvidenceRef {
            evidence_id: item.evidence_id.clone(),
            surface: surface_name(item.surface).into(),
            detail: item.detail.clone(),
        })
        .collect()
}

fn surface_name(surface: aethercore_intelligence_core::EvidenceSurface) -> &'static str {
    use aethercore_intelligence_core::EvidenceSurface as S;
    match surface {
        S::BottleneckReport => "bottleneckReport",
        S::RepairDiagnosis => "repairDiagnosis",
        S::TimelinePattern => "timelinePattern",
        S::MaintenanceHistory => "maintenanceHistory",
        S::SecurityFinding => "securityFinding",
        S::Diagnostics => "diagnostics",
        S::CareHistory => "careHistory",
    }
}

fn base(turn_id: &str, label: &str, pack: &TypedEvidencePack) -> v1::AssistantTurn {
    v1::AssistantTurn {
        turn_id: turn_id.to_owned(),
        schema_version: ASSISTANT_SCHEMA_V1,
        state: v1::AssistantTurnState::Unspecified as i32,
        answer: String::new(),
        citations: Vec::new(),
        engine_label: label.to_owned(),
        refusal: v1::AssistantRefusalReason::Unspecified as i32,
        fault_key: String::new(),
        tokens_emitted: 0,
        pack: evidence_refs(pack),
    }
}

fn streaming(
    turn_id: &str,
    label: &str,
    answer: String,
    tokens: u32,
    pack: &TypedEvidencePack,
) -> v1::AssistantTurn {
    v1::AssistantTurn {
        state: v1::AssistantTurnState::Streaming as i32,
        answer,
        tokens_emitted: tokens,
        ..base(turn_id, label, pack)
    }
}

fn refused(
    turn_id: &str,
    label: &str,
    reason: RefusalReason,
    pack: &TypedEvidencePack,
) -> v1::AssistantTurn {
    v1::AssistantTurn {
        state: v1::AssistantTurnState::Refused as i32,
        refusal: match reason {
            RefusalReason::NoEvidence => v1::AssistantRefusalReason::NoEvidence,
            RefusalReason::NotCovered => v1::AssistantRefusalReason::NotCovered,
            RefusalReason::MutationActive => v1::AssistantRefusalReason::MutationActive,
            RefusalReason::Busy => v1::AssistantRefusalReason::Busy,
        } as i32,
        ..base(turn_id, label, pack)
    }
}

fn faulted(
    turn_id: &str,
    label: &str,
    fault_key: &str,
    pack: &TypedEvidencePack,
) -> v1::AssistantTurn {
    v1::AssistantTurn {
        state: v1::AssistantTurnState::Faulted as i32,
        fault_key: fault_key.to_owned(),
        ..base(turn_id, label, pack)
    }
}

fn terminal(
    turn_id: &str,
    label: &str,
    outcome: TurnOutcome,
    pack: &TypedEvidencePack,
) -> v1::AssistantTurn {
    match outcome {
        TurnOutcome::Answered {
            answer,
            citations,
            tokens,
            engine,
        } => v1::AssistantTurn {
            state: v1::AssistantTurnState::Answered as i32,
            answer,
            citations: citations
                .iter()
                .map(|citation| v1::AssistantEvidenceRef {
                    evidence_id: citation.evidence_id.clone(),
                    surface: surface_name(citation.surface).into(),
                    detail: pack
                        .items
                        .iter()
                        .find(|item| {
                            item.evidence_id == citation.evidence_id
                                && item.surface == citation.surface
                        })
                        .map(|item| item.detail.clone())
                        .unwrap_or_default(),
                })
                .collect(),
            tokens_emitted: tokens,
            ..base(
                turn_id,
                if engine == aethercore_intelligence_core::InsightEngineKind::RuleFallback {
                    "ruleFallback"
                } else {
                    label
                },
                pack,
            )
        },
        TurnOutcome::Refused(reason) => refused(turn_id, label, reason, pack),
        TurnOutcome::Faulted { fault_key, detail } => {
            // The detail rides in `answer` so the renderer has something concrete
            // to show beside the fault key. It is a service diagnostic, never a
            // claim about the machine, so it cannot be mistaken for an answer:
            // the state is FAULTED and the renderer keys off state, not text.
            let mut turn = faulted(turn_id, label, fault_key, pack);
            turn.answer = detail;
            turn
        }
        TurnOutcome::Cancelled { tokens } => v1::AssistantTurn {
            state: v1::AssistantTurnState::Cancelled as i32,
            tokens_emitted: tokens,
            ..base(turn_id, label, pack)
        },
    }
}

fn publish(events: &EventBus, owner: &str, turn: v1::AssistantTurn) {
    events.publish(
        owner,
        EventKind::AssistantTurn,
        "",
        Some(v1::event_envelope::Payload::AssistantTurn(turn)),
    );
}

/// Boundary validation for the wire verb. Returns the reason it is invalid.
pub fn validate(turn_id: &str, question: &str) -> Result<(), &'static str> {
    if turn_id.is_empty() || turn_id.len() > 128 {
        return Err("assistant.invalid.turnId");
    }
    if !turn_id
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err("assistant.invalid.turnId");
    }
    if question.trim().is_empty() {
        return Err("assistant.invalid.emptyQuestion");
    }
    if question.chars().count() > MAX_QUESTION_CHARS {
        return Err("assistant.invalid.questionTooLong");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(
        reasoner: Option<Box<dyn StreamingReasoner>>,
    ) -> (AssistantCoordinator, std::path::PathBuf) {
        let path =
            std::env::temp_dir().join(format!("assistant-admission-{}.db", uuid::Uuid::new_v4()));
        let db = Arc::new(Database::open(&path).unwrap());
        for owner in ["owner-a", "owner-b"] {
            let id = format!("plan-{owner}");
            db.insert_plan(
                &aethercore_persistence::PlanRecord {
                    id: id.clone(),
                    title: "fixture".into(),
                    state: "Completed".into(),
                    digest: id.clone(),
                    risk: "Low".into(),
                    immutable_json: "{}".into(),
                    created_unix_ms: 1_000,
                    updated_unix_ms: 1_000,
                    owner_principal_key: owner.into(),
                },
                "created",
            )
            .unwrap();
            db.upsert_maintenance_execution(&aethercore_persistence::MaintenanceExecutionRecord {
                plan_id: id,
                domain: "Cleanup".into(),
                stage: "Completed".into(),
                updated_unix_ms: 2_000,
                ..Default::default()
            })
            .unwrap();
        }
        let diagnostics = Arc::new(aethercore_diagnostic_engine::DiagnosticEngine::new(
            db.clone(),
        ));
        let repair = Arc::new(aethercore_system_repair::RepairCoordinator::new(
            Arc::new(aethercore_operation_engine::OperationEngine::new(
                db.clone(),
            )),
            db.clone(),
        ));
        (
            AssistantCoordinator::new(db, reasoner, diagnostics, repair),
            path,
        )
    }
    struct Loading;
    impl StreamingReasoner for Loading {
        fn is_loaded(&self) -> bool {
            false
        }
        fn is_loading(&self) -> bool {
            true
        }
        fn generate(
            &self,
            _: &TypedEvidencePack,
            _: &str,
            _: aethercore_intelligence_core::Locale,
            _: &aethercore_intelligence_core::GenerationBudget,
            _: &mut dyn FnMut(&str),
        ) -> Result<aethercore_intelligence_core::Generated, String> {
            panic!("loading must not generate")
        }
    }
    fn immediate_fallback(reasoner: Option<Box<dyn StreamingReasoner>>, fault: &str) {
        let (coordinator, path) = fixture(reasoner);
        let events = EventBus::new(16);
        let turn = coordinator.ask(
            "owner-a",
            "known",
            "maintenance",
            aethercore_intelligence_core::Locale::En,
            false,
            events.clone(),
        );
        assert_eq!(
            turn.state,
            v1::AssistantTurnState::Answered as i32,
            "{turn:?}"
        );
        assert_eq!(turn.engine_label, "ruleFallback");
        assert_eq!(turn.tokens_emitted, 0);
        assert_eq!(turn.citations.len(), 1);
        assert_eq!(turn.citations[0].evidence_id, "plan-owner-a");
        assert!(!turn.answer.is_empty());
        let unknown = coordinator.ask(
            "owner-a",
            "unknown",
            "weather forecast",
            aethercore_intelligence_core::Locale::En,
            false,
            events.clone(),
        );
        assert_eq!(unknown.state, v1::AssistantTurnState::Faulted as i32);
        assert_eq!(unknown.fault_key, fault);
        assert_eq!(events.current_sequence("owner-a"), 0);
        drop(coordinator);
        let _ = std::fs::remove_file(path);
    }
    #[test]
    fn coordinator_absent_model_answers_known_owner_facts_without_generation() {
        immediate_fallback(
            None,
            aethercore_intelligence_core::assistant::FAULT_MODEL_UNAVAILABLE,
        );
    }
    #[test]
    fn coordinator_loading_model_answers_known_owner_facts_without_generation() {
        immediate_fallback(
            Some(Box::new(Loading)),
            aethercore_intelligence_core::assistant::FAULT_MODEL_LOADING,
        );
    }
    struct Deferred {
        entered: std::sync::mpsc::Sender<()>,
        release: Mutex<std::sync::mpsc::Receiver<()>>,
    }
    impl StreamingReasoner for Deferred {
        fn is_loaded(&self) -> bool {
            true
        }
        fn generate(
            &self,
            _: &TypedEvidencePack,
            _: &str,
            _: aethercore_intelligence_core::Locale,
            _: &aethercore_intelligence_core::GenerationBudget,
            _: &mut dyn FnMut(&str),
        ) -> Result<aethercore_intelligence_core::Generated, String> {
            self.entered.send(()).unwrap();
            self.release
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(5))
                .unwrap();
            Ok(aethercore_intelligence_core::Generated {
                text: r#"{"facts":[1]}"#.into(),
                tokens: 4,
                cancelled: false,
            })
        }
    }
    fn deferred_change(changed_owner: Option<&str>, expect_answer: bool, cancel: bool) {
        let (entered_tx, entered_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let (coordinator, path) = fixture(Some(Box::new(Deferred {
            entered: entered_tx,
            release: Mutex::new(release_rx),
        })));
        let events = EventBus::new(16);
        let (subscription, _) = events.subscribe("owner-a", 0);
        let opening = coordinator.ask(
            "owner-a",
            "deferred",
            "maintenance",
            aethercore_intelligence_core::Locale::En,
            false,
            events.clone(),
        );
        assert_eq!(opening.state, v1::AssistantTurnState::Streaming as i32);
        entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        if let Some(owner) = changed_owner {
            coordinator
                .db
                .upsert_maintenance_execution(&aethercore_persistence::MaintenanceExecutionRecord {
                    plan_id: format!("plan-{owner}"),
                    domain: "Cleanup".into(),
                    stage: "Cancelled".into(),
                    updated_unix_ms: 3_000,
                    ..Default::default()
                })
                .unwrap();
        }
        if cancel {
            assert!(coordinator.cancel("owner-a", "deferred"));
        }
        release_tx.send(()).unwrap();
        let aethercore_operation_kernel::SubscriptionItem::Event(event) =
            subscription.recv_timeout(Duration::from_secs(5))
        else {
            panic!("missing terminal");
        };
        let Some(v1::event_envelope::Payload::AssistantTurn(turn)) = event.payload else {
            panic!("wrong payload");
        };
        if cancel {
            assert_eq!(
                turn.state,
                v1::AssistantTurnState::Cancelled as i32,
                "{turn:?}"
            );
            assert!(turn.answer.is_empty());
            assert!(turn.citations.is_empty());
        } else if expect_answer {
            assert_eq!(
                turn.state,
                v1::AssistantTurnState::Answered as i32,
                "{turn:?}"
            );
            assert_eq!(turn.citations[0].evidence_id, "plan-owner-a");
            assert!(!turn.answer.is_empty());
        } else {
            assert_eq!(
                turn.state,
                v1::AssistantTurnState::Refused as i32,
                "{turn:?}"
            );
            assert_eq!(turn.refusal, v1::AssistantRefusalReason::NotCovered as i32);
            assert!(turn.answer.is_empty());
            assert!(turn.citations.is_empty());
            assert!(
                turn.pack
                    .iter()
                    .all(|item| !item.detail.contains("Completed")),
                "old evidence escaped"
            );
        }
        assert!(!coordinator.cancel("owner-a", "deferred"));
        let (_, replay) = events.subscribe("owner-a", 0);
        assert_eq!(replay.events.len(), 1);
        drop(coordinator);
        let _ = std::fs::remove_file(path);
    }
    #[test]
    fn coordinator_changed_owner_evidence_discards_old_terminal_answer() {
        deferred_change(Some("owner-a"), false, false);
    }
    #[test]
    fn coordinator_unchanged_evidence_keeps_terminal_answer() {
        deferred_change(None, true, false);
    }
    #[test]
    fn coordinator_other_owner_evidence_change_keeps_terminal_answer() {
        deferred_change(Some("owner-b"), true, false);
    }

    #[test]
    fn coordinator_accepted_cancel_blocks_answer_even_when_backend_returns_selection() {
        deferred_change(None, false, true);
    }

    #[test]
    fn coordinator_accepted_cancel_keeps_cancelled_when_evidence_changes() {
        deferred_change(Some("owner-a"), false, true);
    }

    #[test]
    fn a_turn_id_that_is_not_a_handle_is_rejected_at_the_boundary() {
        assert!(validate("turn-1", "why?").is_ok());
        assert_eq!(validate("", "why?"), Err("assistant.invalid.turnId"));
        assert_eq!(
            validate("../etc/passwd", "why?"),
            Err("assistant.invalid.turnId")
        );
        assert_eq!(
            validate(&"a".repeat(129), "why?"),
            Err("assistant.invalid.turnId")
        );
    }

    #[test]
    fn an_empty_or_oversized_question_is_rejected_at_the_boundary() {
        assert_eq!(
            validate("turn-1", "   "),
            Err("assistant.invalid.emptyQuestion")
        );
        assert_eq!(
            validate("turn-1", &"ل".repeat(MAX_QUESTION_CHARS + 1)),
            Err("assistant.invalid.questionTooLong")
        );
        // Counted in CHARACTERS, not bytes: an Arabic question of the allowed
        // length is three times that many bytes and must still be accepted.
        assert!(validate("turn-1", &"ل".repeat(MAX_QUESTION_CHARS)).is_ok());
    }

    /// Cancelling a turn that already finished is a state, not a fault: two
    /// windows can press Escape on the same turn.
    #[test]
    fn cancelling_an_unknown_turn_reports_that_it_was_not_in_flight() {
        let live = LiveTurns::default();
        assert!(!live.cancel("owner-a", "turn-1"));
        let flag = live.register("owner-a", "turn-1");
        assert!(!flag.load(Ordering::SeqCst));
        assert!(live.cancel("owner-a", "turn-1"));
        assert!(flag.load(Ordering::SeqCst));
        assert!(live.finish("owner-a", "turn-1"));
        assert!(!live.cancel("owner-a", "turn-1"));
    }

    /// One principal's cancel cannot reach another principal's turn.
    #[test]
    fn a_cancel_is_scoped_to_the_principal_that_started_the_turn() {
        let live = LiveTurns::default();
        let flag = live.register("owner-a", "turn-1");
        assert!(!live.cancel("owner-b", "turn-1"));
        assert!(!flag.load(Ordering::SeqCst));
    }
}
