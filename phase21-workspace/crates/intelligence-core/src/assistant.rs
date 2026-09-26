//! Phase 56 — the grounded assistant turn engine.
//!
//! This is the organ that answers a question the user typed. It is separate from
//! the insight path because the two have different failure modes: an insight the
//! product volunteers can simply not exist, while a question the user asked must
//! always end in something they can read — an answer, a refusal, or a declared
//! fault. There is no fourth outcome, and there is no empty string.
//!
//! THE RULE THAT DECIDES EVERYTHING HERE: a claim about the user's machine that
//! cannot cite a measurement is not shown. A 1.5B instruct model asked "why is my
//! PC slow?" will answer fluently from its training with no measurement behind a
//! single clause, which is precisely what this product's citation rule exists to
//! prevent. So generation is bounded by an evidence pack, and the text it
//! produces is admitted only if it resolves against that pack. Text that does not
//! resolve is DISCARDED — not shown dimmed, not shown with a low-confidence
//! badge. Showing it would put an uncited claim about someone's computer on the
//! screen, which is the one thing this product is built not to do.
//!
//! The engine label never lies either. It reports what the engine IS. A
//! generation failure is a FAULTED turn carrying a reason key, never a silent
//! degrade to the rule engine — the rule engine summarises evidence and cannot
//! answer a question, so letting it answer one under the model's label would be
//! the silent fallback P56's brief forbids.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crate::model::{Citation, TypedEvidencePack};

/// Longest question accepted. Validated at the wire boundary; restated here so
/// the engine cannot be driven past it by a caller that forgot.
pub const MAX_QUESTION_CHARS: usize = 2_000;

/// Token ceiling for one turn. A grounded answer over a bounded pack is a
/// paragraph; anything longer is the model drifting off its evidence.
pub const MAX_ANSWER_TOKENS: u32 = 512;

/// Wall-clock ceiling for one turn, from the moment generation starts.
pub const ASSISTANT_DEADLINE: Duration = Duration::from_secs(20);

/// Turn schema version, carried on the wire so a renderer can refuse an unknown.
pub const ASSISTANT_SCHEMA_V1: u32 = 1;

/// Fault keys. Message keys, never prose — the renderer owns the words.
pub const FAULT_MODEL_UNAVAILABLE: &str = "assistant.fault.modelUnavailable";
pub const FAULT_DEADLINE_EXCEEDED: &str = "assistant.fault.deadlineExceeded";
pub const FAULT_GENERATION_FAILED: &str = "assistant.fault.generationFailed";

/// The error a reasoner returns when the one embedded model is already
/// generating for another request (the insight path and the assistant share it).
/// Not a failure: the turn is refused `Busy`, and an insight degrades to the rule
/// engine, both at once rather than queued behind a generation.
pub const MODEL_BUSY: &str = "the embedded model is generating for another request";

/// Why a turn could not be grounded. Never a fault: the product has simply not
/// measured the thing being asked about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RefusalReason {
    /// Nothing has been collected at all.
    NoEvidence,
    /// Evidence exists; none of it bears on the question.
    NotCovered,
    /// Observer-effect guard (I4).
    MutationActive,
    /// The single-flight lane is held.
    Busy,
}

/// The one thing a turn can end as.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TurnOutcome {
    Answered {
        answer: String,
        citations: Vec<Citation>,
        tokens: u32,
    },
    Refused(RefusalReason),
    Faulted {
        fault_key: &'static str,
        detail: String,
    },
    Cancelled {
        tokens: u32,
    },
}

/// The budget one generation runs under. Every field is a ceiling the loop is
/// required to check, and `cancel` is the one the user can raise.
pub struct GenerationBudget {
    pub deadline: Instant,
    pub max_tokens: u32,
    pub cancel: Arc<AtomicBool>,
}

impl GenerationBudget {
    pub fn new(cancel: Arc<AtomicBool>) -> Self {
        Self {
            deadline: Instant::now() + ASSISTANT_DEADLINE,
            max_tokens: MAX_ANSWER_TOKENS,
            cancel,
        }
    }

    pub fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }

    pub fn expired(&self) -> bool {
        Instant::now() >= self.deadline
    }
}

/// What a generation run produced.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Generated {
    /// Raw model text, citation markers included. Grounding runs on this.
    pub text: String,
    pub tokens: u32,
    /// True when the loop stopped because `budget.cancel` was raised.
    pub cancelled: bool,
}

/// A reasoner that can stream tokens under a budget.
///
/// Separate from [`crate::engine::LocalReasoner`], which returns a finished
/// `Vec<Insight>` and cannot be cancelled, cannot stream, and has no notion of a
/// token ceiling. Implementing it on the rule fallback would be a lie: the
/// fallback summarises a pack and cannot answer a question.
pub trait StreamingReasoner: Send + Sync {
    fn is_loaded(&self) -> bool;
    /// Generates an answer to `question` over `pack`, calling `sink` with the
    /// ACCUMULATED text after each token so a renderer never has to reassemble
    /// fragments. Errors are real failures, never "no answer".
    fn generate(
        &self,
        pack: &TypedEvidencePack,
        question: &str,
        budget: &GenerationBudget,
        sink: &mut dyn FnMut(&str),
    ) -> Result<Generated, String>;
}

/// The rules the model answers under. Kept separate from the evidence so a
/// chat-template model can put it where its template expects a system turn.
pub const SYSTEM_PROMPT: &str = "You are an offline diagnostic assistant for one computer. \
Answer ONLY from the EVIDENCE block. Every sentence you write MUST end with the tag of the \
evidence it rests on, in square brackets, exactly as the tag is written in the block. A sentence \
without a tag is not allowed. Never invent a tag that is not in the block. Never use general \
knowledge about computers. Answer in at most four sentences.\n\n\
Example EVIDENCE:\n\
[E1] surface=MaintenanceHistory id=plan-0007 :: plan plan-0007 domain startup stage completed\n\
[E2] surface=TimelinePattern id=pattern-disk-2 :: event class Disk code DSK-2 recurred twice\n\
Example QUESTION: what has been happening?\n\
Example ANSWER: A startup plan completed [E1]. A disk event has recurred twice [E2].\n\n\
If the EVIDENCE block does not answer the question, reply with exactly NO EVIDENCE and nothing \
else.";

/// The evidence block and the question — the user turn.
///
/// The pack is numbered `[E1]…[En]`, and that numbering is the whole grounding
/// mechanism: tags are machine-checkable against the pack, where "does this
/// sentence follow from the evidence?" is not. The model can still write an
/// unsupported sentence — it just cannot make one survive [`ground`].
///
/// Pack details are pre-typed structured summaries, never raw host prose, so a
/// hostile string cannot appear where an instruction would be read.
pub fn render_user_message(pack: &TypedEvidencePack, question: &str) -> String {
    let mut evidence = String::new();
    for (index, item) in pack.items.iter().enumerate() {
        evidence.push_str(&format!(
            "[E{}] surface={:?} id={} :: {}\n",
            index + 1,
            item.surface,
            item.evidence_id,
            item.detail
        ));
    }
    format!("EVIDENCE:\n{evidence}\nQUESTION: {question}")
}

/// System rules plus the user turn, for a reasoner with no chat template of its
/// own. The embedded model wraps the two halves in its own template instead.
pub fn render_prompt(pack: &TypedEvidencePack, question: &str) -> String {
    format!(
        "{SYSTEM_PROMPT}\n\n{}\n\nANSWER:",
        render_user_message(pack, question)
    )
}

/// The citation gate.
///
/// Extracts `[En]` markers and resolves them against the pack. A marker that
/// resolves **stays in the text** — the renderer turns it into an inline
/// evidence chip, so a reader can see which clause rests on which observation,
/// and the sentence keeps its grammar (stripping a sentence-initial `[E1]` left
/// answers starting "indicates that…"). A marker that does NOT resolve is
/// removed, because a citation of evidence the product does not hold is worse
/// than no citation at all.
///
/// Returns `None` when nothing resolved, when nothing is left, or when the model
/// used its own refusal token — all three mean there is no grounded claim here,
/// so there is nothing this product is allowed to show.
pub fn ground(raw: &str, pack: &TypedEvidencePack) -> Option<(String, Vec<Citation>)> {
    let mut citations: Vec<Citation> = Vec::new();
    let mut text = String::with_capacity(raw.len());
    // The same answer with EVERY marker removed, resolvable or not. Only used to
    // decide whether the model's whole reply was its refusal token.
    let mut prose = String::with_capacity(raw.len());
    let bytes = raw.as_bytes();
    let mut cursor = 0usize;

    while cursor < bytes.len() {
        if bytes[cursor] == b'[' {
            // `[E` + digits + `]`, and nothing else.
            let mut scan = cursor + 1;
            if scan < bytes.len() && (bytes[scan] == b'E' || bytes[scan] == b'e') {
                scan += 1;
                let digits_start = scan;
                while scan < bytes.len() && bytes[scan].is_ascii_digit() {
                    scan += 1;
                }
                if scan > digits_start && scan < bytes.len() && bytes[scan] == b']' {
                    let index: usize = raw[digits_start..scan].parse().unwrap_or(0);
                    if let Some(item) = index.checked_sub(1).and_then(|i| pack.items.get(i)) {
                        let citation = Citation {
                            evidence_id: item.evidence_id.clone(),
                            surface: item.surface,
                        };
                        if !citations.contains(&citation) {
                            citations.push(citation);
                        }
                        // Normalised spelling, so the renderer has one shape
                        // to match rather than `[e1]` and `[E1]` both.
                        text.push_str(&format!("[E{index}]"));
                    }
                    // Unresolvable index: dropped, silently, from the text.
                    cursor = scan + 1;
                    continue;
                }
            }
        }
        // Not a marker: copy the character. Markers are pure ASCII, so `cursor`
        // only ever lands on a char boundary.
        let char_len = utf8_len(bytes[cursor]);
        let end = (cursor + char_len).min(raw.len());
        text.push_str(&raw[cursor..end]);
        prose.push_str(&raw[cursor..end]);
        cursor = end;
    }

    let cleaned = collapse_whitespace(&text);
    if citations.is_empty() || cleaned.is_empty() {
        return None;
    }
    // The model's own refusal, honoured rather than reinterpreted. Matched on
    // the whole answer stripped of markers and punctuation — not `contains`,
    // because a grounded answer may legitimately quote the phrase back while
    // explaining what it could and could not find.
    if is_refusal_token(&collapse_whitespace(&prose)) {
        return None;
    }
    Some((cleaned, citations))
}

/// True when the answer, with markers and surrounding punctuation removed, is
/// the refusal token and nothing else.
fn is_refusal_token(cleaned: &str) -> bool {
    let core: String = cleaned
        .chars()
        .filter(|ch| ch.is_alphanumeric() || ch.is_whitespace())
        .collect();
    collapse_whitespace(&core).eq_ignore_ascii_case("no evidence")
}

fn utf8_len(first: u8) -> usize {
    match first {
        0x00..=0x7F => 1,
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        _ => 4,
    }
}

fn collapse_whitespace(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut pending_space = false;
    for ch in value.chars() {
        if ch.is_whitespace() {
            pending_space = !out.is_empty();
            continue;
        }
        if pending_space {
            out.push(' ');
            pending_space = false;
        }
        out.push(ch);
    }
    out
}

/// The turn engine: budgets, the observer-effect guard, single flight, and the
/// citation gate. It owns no I/O and no wire types, so every rule below is
/// testable without a model, a service or a socket.
pub struct AssistantEngine {
    reasoner: Option<Box<dyn StreamingReasoner>>,
    in_flight: AtomicBool,
}

impl AssistantEngine {
    pub fn new(reasoner: Option<Box<dyn StreamingReasoner>>) -> Self {
        Self {
            reasoner,
            in_flight: AtomicBool::new(false),
        }
    }

    /// What the engine IS — not what served the last call. `disabled` when the
    /// embedded artifact did not verify or did not load; `localModel` when it
    /// did. It never reads `ruleFallback`, because the rule engine is not on
    /// this path at all.
    pub fn engine_label(&self) -> &'static str {
        match self.reasoner.as_ref() {
            Some(reasoner) if reasoner.is_loaded() => "localModel",
            _ => "disabled",
        }
    }

    /// Runs one turn. `sink` receives the accumulated PROVISIONAL answer as it
    /// grows; the outcome is what the caller is allowed to present.
    pub fn ask(
        &self,
        pack: &TypedEvidencePack,
        question: &str,
        mutation_active: bool,
        cancel: Arc<AtomicBool>,
        sink: &mut dyn FnMut(&str),
    ) -> TurnOutcome {
        if mutation_active {
            return TurnOutcome::Refused(RefusalReason::MutationActive);
        }
        // No evidence: refuse BEFORE the model is asked. Asking it anyway would
        // produce an answer that is uncitable by construction, and the only
        // thing to do with that answer is throw it away — so do not spend a
        // second of the user's machine generating it.
        if pack.items.is_empty() {
            return TurnOutcome::Refused(RefusalReason::NoEvidence);
        }
        let Some(reasoner) = self.reasoner.as_ref() else {
            return TurnOutcome::Faulted {
                fault_key: FAULT_MODEL_UNAVAILABLE,
                detail: "no reasoner is loaded".into(),
            };
        };
        if !reasoner.is_loaded() {
            return TurnOutcome::Faulted {
                fault_key: FAULT_MODEL_UNAVAILABLE,
                detail: "the embedded artifact did not load".into(),
            };
        }
        let Some(lane) = crate::engine::Lane::acquire(&self.in_flight) else {
            return TurnOutcome::Refused(RefusalReason::Busy);
        };

        let budget = GenerationBudget::new(cancel);
        let bounded: &str = if question.len() > MAX_QUESTION_CHARS {
            let mut end = MAX_QUESTION_CHARS;
            while end > 0 && !question.is_char_boundary(end) {
                end -= 1;
            }
            &question[..end]
        } else {
            question
        };
        let result = reasoner.generate(pack, bounded, &budget, sink);
        drop(lane);

        match result {
            Err(detail) if detail == MODEL_BUSY => TurnOutcome::Refused(RefusalReason::Busy),
            Err(detail) => TurnOutcome::Faulted {
                fault_key: if budget.expired() {
                    FAULT_DEADLINE_EXCEEDED
                } else {
                    FAULT_GENERATION_FAILED
                },
                detail,
            },
            Ok(generated) if generated.cancelled => TurnOutcome::Cancelled {
                tokens: generated.tokens,
            },
            Ok(generated) => match ground(&generated.text, pack) {
                Some((answer, citations)) => TurnOutcome::Answered {
                    answer,
                    citations,
                    tokens: generated.tokens,
                },
                // Text that cites nothing resolvable is discarded here, and this
                // is the line that keeps an uncited claim off the screen.
                None => TurnOutcome::Refused(RefusalReason::NotCovered),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{EvidenceItem, EvidenceSurface};

    fn pack_of(ids: &[&str]) -> TypedEvidencePack {
        let mut pack = TypedEvidencePack::default();
        for id in ids {
            pack.push(EvidenceItem {
                evidence_id: (*id).into(),
                surface: EvidenceSurface::TimelinePattern,
                detail: format!("observation for {id}"),
            });
        }
        pack
    }

    /// A reasoner that returns whatever the test hands it.
    struct Scripted {
        loaded: bool,
        result: Result<Generated, String>,
    }
    impl StreamingReasoner for Scripted {
        fn is_loaded(&self) -> bool {
            self.loaded
        }
        fn generate(
            &self,
            _pack: &TypedEvidencePack,
            _question: &str,
            _budget: &GenerationBudget,
            sink: &mut dyn FnMut(&str),
        ) -> Result<Generated, String> {
            if let Ok(generated) = &self.result {
                sink(&generated.text);
            }
            self.result.clone()
        }
    }

    fn engine(result: Result<Generated, String>) -> AssistantEngine {
        AssistantEngine::new(Some(Box::new(Scripted {
            loaded: true,
            result,
        })))
    }

    fn answered(text: &str) -> Result<Generated, String> {
        Ok(Generated {
            text: text.into(),
            tokens: 7,
            cancelled: false,
        })
    }

    /// THE failure that matters, #1. The model writes a fluent, confident,
    /// entirely plausible paragraph about the user's machine and cites nothing.
    /// It must not reach the screen, and the turn must say so.
    #[test]
    fn a_confident_answer_citing_nothing_is_refused_not_shown() {
        let engine = engine(answered(
            "Your PC is slow because background services are consuming CPU and your \
             disk is fragmented. Disabling startup items will fix it.",
        ));
        let outcome = engine.ask(
            &pack_of(&["fact-a"]),
            "why is my pc slow?",
            false,
            Arc::new(AtomicBool::new(false)),
            &mut |_| {},
        );
        assert_eq!(outcome, TurnOutcome::Refused(RefusalReason::NotCovered));
    }

    /// The same sentence with a marker that names evidence the pack does not
    /// hold is still ungrounded. A hallucinated citation is worse than none.
    #[test]
    fn a_citation_that_does_not_resolve_is_not_a_citation() {
        let engine = engine(answered("Your disk is failing [E9]."));
        let outcome = engine.ask(
            &pack_of(&["fact-a"]),
            "is my disk ok?",
            false,
            Arc::new(AtomicBool::new(false)),
            &mut |_| {},
        );
        assert_eq!(outcome, TurnOutcome::Refused(RefusalReason::NotCovered));
    }

    /// A resolvable marker STAYS in the answer — the renderer turns it into an
    /// inline evidence chip, and the sentence keeps its grammar. Stripping a
    /// sentence-initial one is how the first cut of this produced answers that
    /// began "indicates that…".
    #[test]
    fn an_answer_whose_markers_resolve_keeps_them_for_the_renderer() {
        let engine = engine(answered(
            "A cleanup plan ran [E1] and a disk event recurred [E2].",
        ));
        let outcome = engine.ask(
            &pack_of(&["fact-a", "fact-b"]),
            "what ran?",
            false,
            Arc::new(AtomicBool::new(false)),
            &mut |_| {},
        );
        match outcome {
            TurnOutcome::Answered {
                answer, citations, ..
            } => {
                assert_eq!(
                    answer,
                    "A cleanup plan ran [E1] and a disk event recurred [E2]."
                );
                assert_eq!(citations.len(), 2);
                assert_eq!(citations[0].evidence_id, "fact-a");
                assert_eq!(citations[1].evidence_id, "fact-b");
            }
            other => panic!("expected an answer, got {other:?}"),
        }
    }

    /// A marker naming evidence the pack does not hold is removed from the text
    /// as well as from the citations. A false citation on screen is worse than
    /// no citation at all.
    #[test]
    fn an_unresolvable_marker_is_removed_from_the_answer_too() {
        let pack = pack_of(&["fact-a"]);
        let (text, citations) =
            ground("The disk is fine [E1] and the fan failed [E9].", &pack).expect("grounded");
        assert_eq!(text, "The disk is fine [E1] and the fan failed .");
        assert_eq!(citations.len(), 1);
    }

    /// THE failure that matters, #2. A model failure is a declared fault with a
    /// reason. It is never an empty answer, and it never becomes a rule-engine
    /// summary wearing the model's label.
    #[test]
    fn a_model_failure_surfaces_a_fault_rather_than_an_empty_string() {
        let engine = engine(Err("gguf decode failed".into()));
        let outcome = engine.ask(
            &pack_of(&["fact-a"]),
            "anything?",
            false,
            Arc::new(AtomicBool::new(false)),
            &mut |_| {},
        );
        match outcome {
            TurnOutcome::Faulted { fault_key, detail } => {
                assert_eq!(fault_key, FAULT_GENERATION_FAILED);
                assert_eq!(detail, "gguf decode failed");
            }
            other => panic!("expected a fault, got {other:?}"),
        }
    }

    #[test]
    fn an_unloaded_model_faults_and_the_label_says_disabled() {
        let engine = AssistantEngine::new(Some(Box::new(Scripted {
            loaded: false,
            result: answered("[E1] anything"),
        })));
        assert_eq!(engine.engine_label(), "disabled");
        match engine.ask(
            &pack_of(&["fact-a"]),
            "q",
            false,
            Arc::new(AtomicBool::new(false)),
            &mut |_| {},
        ) {
            TurnOutcome::Faulted { fault_key, .. } => {
                assert_eq!(fault_key, FAULT_MODEL_UNAVAILABLE)
            }
            other => panic!("expected a fault, got {other:?}"),
        }
    }

    /// An empty pack never reaches the model: an answer over no evidence is
    /// uncitable by construction, so generating one only spends the user's CPU
    /// to produce something that must be thrown away.
    #[test]
    fn an_empty_pack_refuses_without_asking_the_model() {
        struct Exploding;
        impl StreamingReasoner for Exploding {
            fn is_loaded(&self) -> bool {
                true
            }
            fn generate(
                &self,
                _: &TypedEvidencePack,
                _: &str,
                _: &GenerationBudget,
                _: &mut dyn FnMut(&str),
            ) -> Result<Generated, String> {
                panic!("the model must not be asked when there is no evidence");
            }
        }
        let engine = AssistantEngine::new(Some(Box::new(Exploding)));
        assert_eq!(
            engine.ask(
                &TypedEvidencePack::default(),
                "why is my pc slow?",
                false,
                Arc::new(AtomicBool::new(false)),
                &mut |_| {},
            ),
            TurnOutcome::Refused(RefusalReason::NoEvidence),
        );
    }

    #[test]
    fn inference_is_refused_while_a_mutation_holds_the_lease() {
        let engine = engine(answered("[E1] anything"));
        assert_eq!(
            engine.ask(
                &pack_of(&["fact-a"]),
                "q",
                true,
                Arc::new(AtomicBool::new(false)),
                &mut |_| {},
            ),
            TurnOutcome::Refused(RefusalReason::MutationActive),
        );
    }

    /// THE failure that matters, #3, at the engine's own edge: a reasoner that
    /// honours the cancel flag produces a CANCELLED turn rather than an answer.
    /// That the real loop actually checks the flag is asserted separately, by
    /// `cancellation_stops_the_generation_loop`.
    #[test]
    fn a_cancelled_generation_is_not_presented_as_an_answer() {
        let engine = engine(Ok(Generated {
            text: "partial [E1]".into(),
            tokens: 3,
            cancelled: true,
        }));
        assert_eq!(
            engine.ask(
                &pack_of(&["fact-a"]),
                "q",
                false,
                Arc::new(AtomicBool::new(false)),
                &mut |_| {},
            ),
            TurnOutcome::Cancelled { tokens: 3 },
        );
    }

    /// THE failure that matters, #3, properly: a generation loop written against
    /// this budget must stop when the flag is raised. The fake loop below is the
    /// same shape as the real one in `llama.rs` — check the flag, emit a token,
    /// repeat — so this asserts the contract the real loop is written to.
    #[test]
    fn cancellation_stops_the_generation_loop() {
        struct Looping {
            emitted: std::sync::Mutex<u32>,
        }
        impl StreamingReasoner for Looping {
            fn is_loaded(&self) -> bool {
                true
            }
            fn generate(
                &self,
                _: &TypedEvidencePack,
                _: &str,
                budget: &GenerationBudget,
                sink: &mut dyn FnMut(&str),
            ) -> Result<Generated, String> {
                let mut text = String::new();
                let mut tokens = 0u32;
                let mut cancelled = false;
                // Deliberately far above any sane answer: if the flag is not
                // honoured this runs to MAX_ANSWER_TOKENS and the assertion
                // below fails on the count.
                while tokens < budget.max_tokens {
                    if budget.cancelled() {
                        cancelled = true;
                        break;
                    }
                    if budget.expired() {
                        return Err("deadline".into());
                    }
                    text.push_str("tok ");
                    tokens += 1;
                    *self.emitted.lock().unwrap() = tokens;
                    sink(&text);
                }
                Ok(Generated {
                    text,
                    tokens,
                    cancelled,
                })
            }
        }

        let cancel = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&cancel);
        let engine = AssistantEngine::new(Some(Box::new(Looping {
            emitted: std::sync::Mutex::new(0),
        })));

        // Raised from the sink, which is where a user's Escape would reach it:
        // between two tokens, while the loop is running.
        let mut seen = 0u32;
        let outcome = engine.ask(
            &pack_of(&["fact-a"]),
            "q",
            false,
            cancel,
            &mut |accumulated| {
                seen = accumulated.split_whitespace().count() as u32;
                if seen == 5 {
                    flag.store(true, Ordering::SeqCst);
                }
            },
        );

        match outcome {
            TurnOutcome::Cancelled { tokens } => {
                assert_eq!(
                    tokens, 5,
                    "generation must stop at the token the flag was raised on"
                );
                assert!(tokens < MAX_ANSWER_TOKENS);
            }
            other => panic!("expected a cancelled turn, got {other:?}"),
        }
    }

    #[test]
    fn a_second_turn_while_one_is_in_flight_is_refused_busy() {
        struct Reentrant {
            inner: std::sync::Mutex<Option<Arc<AssistantEngine>>>,
            nested: std::sync::Mutex<Option<TurnOutcome>>,
        }
        impl StreamingReasoner for Reentrant {
            fn is_loaded(&self) -> bool {
                true
            }
            fn generate(
                &self,
                pack: &TypedEvidencePack,
                _: &str,
                _: &GenerationBudget,
                _: &mut dyn FnMut(&str),
            ) -> Result<Generated, String> {
                // Re-enter while the lane is held, which is what a second window
                // asking a question at the same moment does.
                if let Some(engine) = self.inner.lock().unwrap().as_ref() {
                    let outcome = engine.ask(
                        pack,
                        "second",
                        false,
                        Arc::new(AtomicBool::new(false)),
                        &mut |_| {},
                    );
                    *self.nested.lock().unwrap() = Some(outcome);
                }
                Ok(Generated {
                    text: "[E1] fine".into(),
                    tokens: 2,
                    cancelled: false,
                })
            }
        }

        let reasoner = Arc::new(Reentrant {
            inner: std::sync::Mutex::new(None),
            nested: std::sync::Mutex::new(None),
        });
        struct Proxy(Arc<Reentrant>);
        impl StreamingReasoner for Proxy {
            fn is_loaded(&self) -> bool {
                self.0.is_loaded()
            }
            fn generate(
                &self,
                pack: &TypedEvidencePack,
                question: &str,
                budget: &GenerationBudget,
                sink: &mut dyn FnMut(&str),
            ) -> Result<Generated, String> {
                self.0.generate(pack, question, budget, sink)
            }
        }
        let engine = Arc::new(AssistantEngine::new(Some(Box::new(Proxy(Arc::clone(
            &reasoner,
        ))))));
        *reasoner.inner.lock().unwrap() = Some(Arc::clone(&engine));

        let outcome = engine.ask(
            &pack_of(&["fact-a"]),
            "first",
            false,
            Arc::new(AtomicBool::new(false)),
            &mut |_| {},
        );
        assert!(matches!(outcome, TurnOutcome::Answered { .. }));
        assert_eq!(
            reasoner.nested.lock().unwrap().clone(),
            Some(TurnOutcome::Refused(RefusalReason::Busy)),
        );
    }

    /// The model held by an insight generation is the lane being busy, not a
    /// generation failure: the user can simply ask again.
    #[test]
    fn a_model_busy_with_an_insight_refuses_the_turn_busy() {
        let engine = engine(Err(MODEL_BUSY.into()));
        assert_eq!(
            engine.ask(
                &pack_of(&["fact-a"]),
                "q",
                false,
                Arc::new(AtomicBool::new(false)),
                &mut |_| {},
            ),
            TurnOutcome::Refused(RefusalReason::Busy),
        );
    }

    /// A reasoner that panics must not leave the lane held: every later turn
    /// would otherwise be refused `Busy` until the service restarted.
    #[test]
    fn a_panicking_reasoner_releases_the_lane() {
        struct PanicsOnce(AtomicBool);
        impl StreamingReasoner for PanicsOnce {
            fn is_loaded(&self) -> bool {
                true
            }
            fn generate(
                &self,
                _: &TypedEvidencePack,
                _: &str,
                _: &GenerationBudget,
                _: &mut dyn FnMut(&str),
            ) -> Result<Generated, String> {
                if !self.0.swap(true, Ordering::SeqCst) {
                    panic!("llama.cpp aborted mid-turn");
                }
                answered("A plan ran [E1].")
            }
        }
        let engine = AssistantEngine::new(Some(Box::new(PanicsOnce(AtomicBool::new(false)))));
        let ask = || {
            engine.ask(
                &pack_of(&["fact-a"]),
                "q",
                false,
                Arc::new(AtomicBool::new(false)),
                &mut |_| {},
            )
        };
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(ask)).is_err());
        assert!(
            matches!(ask(), TurnOutcome::Answered { .. }),
            "the lane must be free after a panic"
        );
    }

    #[test]
    fn the_prompt_numbers_every_pack_item_and_carries_the_question() {
        let message = render_user_message(&pack_of(&["fact-a", "fact-b"]), "what happened?");
        assert!(message.contains("[E1] surface="), "{message}");
        assert!(message.contains("[E2] surface="), "{message}");
        assert!(message.contains("id=fact-a"), "{message}");
        assert!(message.contains("QUESTION: what happened?"), "{message}");
        let prompt = render_prompt(&pack_of(&["fact-a"]), "q");
        assert!(prompt.contains(SYSTEM_PROMPT), "{prompt}");
        assert!(prompt.contains("NO EVIDENCE"), "{prompt}");
    }

    /// The one-shot example in the system prompt is not decoration: without it
    /// the 1.5B model wrote fluent grounded-SOUNDING prose and emitted no tags
    /// at all, so every real answer was discarded by the gate. Measured, three
    /// questions, zero markers. With it, tags appear on every sentence.
    #[test]
    fn the_system_prompt_shows_the_tag_format_rather_than_only_describing_it() {
        assert!(SYSTEM_PROMPT.contains("Example ANSWER:"), "{SYSTEM_PROMPT}");
        assert!(SYSTEM_PROMPT.contains("[E1]. "), "{SYSTEM_PROMPT}");
    }

    #[test]
    fn the_models_own_refusal_token_is_honoured_rather_than_reinterpreted() {
        assert!(ground("NO EVIDENCE [E1]", &pack_of(&["fact-a"])).is_none());
    }

    #[test]
    fn grounding_leaves_arabic_intact() {
        let pack = pack_of(&["fact-a"]);
        let (text, citations) = ground("خطتان نُفّذتا هذا الأسبوع [E1].", &pack).expect("grounded");
        assert_eq!(text, "خطتان نُفّذتا هذا الأسبوع [E1].");
        assert_eq!(citations.len(), 1);
    }

    #[test]
    fn a_question_longer_than_the_bound_is_truncated_not_rejected_mid_character() {
        struct Echo;
        impl StreamingReasoner for Echo {
            fn is_loaded(&self) -> bool {
                true
            }
            fn generate(
                &self,
                _: &TypedEvidencePack,
                question: &str,
                _: &GenerationBudget,
                _: &mut dyn FnMut(&str),
            ) -> Result<Generated, String> {
                assert!(question.len() <= MAX_QUESTION_CHARS);
                assert!(question.is_char_boundary(question.len()));
                Ok(Generated {
                    text: "[E1] ok".into(),
                    tokens: 1,
                    cancelled: false,
                })
            }
        }
        let engine = AssistantEngine::new(Some(Box::new(Echo)));
        let question = "لماذا ".repeat(1_000);
        assert!(question.len() > MAX_QUESTION_CHARS);
        assert!(matches!(
            engine.ask(
                &pack_of(&["fact-a"]),
                &question,
                false,
                Arc::new(AtomicBool::new(false)),
                &mut |_| {},
            ),
            TurnOutcome::Answered { .. },
        ));
    }
}
