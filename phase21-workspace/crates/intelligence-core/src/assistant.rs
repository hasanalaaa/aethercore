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

use crate::model::{Citation, Locale, TypedEvidencePack};

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
/// The model is still loading after a service start (P76, DBT-P75-078).
pub const FAULT_MODEL_LOADING: &str = "assistant.fault.modelLoading";
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
        /// Whether selection came from the model or the bounded deterministic fallback.
        engine: crate::model::InsightEngineKind,
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
    /// Whether the model is still loading; a turn then faults loading, not unavailable.
    fn is_loading(&self) -> bool {
        false
    }
    /// Generates an answer to `question` over `pack`, in `locale`, calling `sink`
    /// with the ACCUMULATED text after each token so a renderer never has to
    /// reassemble fragments. Errors are real failures, never "no answer".
    fn generate(
        &self,
        pack: &TypedEvidencePack,
        question: &str,
        locale: Locale,
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

/// The rule appended to a system prompt when the reader asked for Arabic
/// (DBT-P75-052). Everything a gate reads stays as it is: the tags, the refusal
/// token, and numbers in the digits the number gate reads.
pub const ARABIC_RULE: &str = "\n\nThe reader reads Arabic: write every sentence in Arabic. Keep \
each evidence tag exactly as written, for example [E1]. Write every number with the digits 0-9. \
NO EVIDENCE stays in English.\n\
Example ANSWER in Arabic:\n\
اكتملت خطة صيانة لبدء التشغيل [E1].\n\
تكرر حدث في القرص [E2].";

/// `system` with the language rule for `locale`; English is the prompt as written.
pub fn system_prompt_in(system: &str, locale: Locale) -> String {
    match locale {
        Locale::En => system.to_owned(),
        Locale::Ar => format!("{system}{ARABIC_RULE}"),
    }
}

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

pub const FACT_SYSTEM_PROMPT: &str = "Select up to four fact IDs that answer the question. \
Use ONLY FACTS, never general knowledge. Return ONLY JSON of the form {\"facts\":[1]}. \
Do not supply prose, values, commands or other fields. For an unrelated question return {\"facts\":[]}.";

pub fn render_fact_user_message(pack: &TypedEvidencePack, question: &str) -> String {
    let facts = pack
        .propositions
        .iter()
        .enumerate()
        .map(|(index, proposition)| {
            format!("{}: {}", index + 1, proposition.fact.sentence(Locale::En))
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!("FACTS:\n{facts}\nQUESTION: {question}")
}

fn fallback_facts(
    pack: &TypedEvidencePack,
    question: &str,
    locale: Locale,
) -> Option<(String, Vec<Citation>)> {
    let question = question.to_lowercase();
    // ponytail: fallback uses a narrow topic match over the same typed facts; unknown questions refuse.
    let maintenance = ["maintenance", "صيانة"]
        .iter()
        .any(|word| question.contains(word));
    let recurrence = ["recur", "repeated failure", "تكرر", "تكرار"]
        .iter()
        .any(|word| question.contains(word));
    let diagnostics = ["diagnostic", "finding", "تشخيص", "نتيجة"]
        .iter()
        .any(|word| question.contains(word));
    let repair = ["repair", "إصلاح"]
        .iter()
        .any(|word| question.contains(word));
    let care = ["care", "عناية"].iter().any(|word| question.contains(word));
    let ids =
        pack.propositions
            .iter()
            .enumerate()
            .filter(|(_, proposition)| match proposition.fact {
                crate::model::Fact::PlanCompleted { .. }
                | crate::model::Fact::PlanCancelled { .. } => maintenance,
                crate::model::Fact::RepeatedFailure { .. } => recurrence,
                crate::model::Fact::DiagnosticAttention { .. }
                | crate::model::Fact::DiagnosticsIncomplete => diagnostics,
                crate::model::Fact::RepairObservation { .. } => repair,
                crate::model::Fact::CareResult { .. } => care,
            })
            .take(4)
            .map(|(index, _)| (index + 1).to_string())
            .collect::<Vec<_>>()
            .join(",");
    render_fact_selection(&format!("{{\"facts\":[{ids}]}}"), pack, locale)
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FactSelection {
    facts: Vec<usize>,
}

/// P86-02A: model output selects a bounded list of existing facts; it supplies no prose or values.
/// Unknown/duplicate IDs and every extra field refuse the whole selection.
pub fn render_fact_selection(
    raw: &str,
    pack: &TypedEvidencePack,
    locale: Locale,
) -> Option<(String, Vec<Citation>)> {
    if raw.len() > 1024 {
        return None;
    }
    let selected: FactSelection = serde_json::from_str(raw).ok()?;
    if selected.facts.is_empty() || selected.facts.len() > 4 {
        return None;
    }
    let mut seen = Vec::new();
    let mut sentences = Vec::new();
    let mut citations = Vec::new();
    for id in selected.facts {
        if seen.contains(&id) {
            return None;
        }
        seen.push(id);
        let proposition = pack.propositions.get(id.checked_sub(1)?)?;
        let index = pack.items.iter().position(|item| {
            item.evidence_id == proposition.citation.evidence_id
                && item.surface == proposition.citation.surface
        })? + 1;
        sentences.push(format!("{} [E{index}].", proposition.fact.sentence(locale)));
        if !citations.contains(&proposition.citation) {
            citations.push(proposition.citation.clone());
        }
    }
    Some((sentences.join(" "), citations))
}

/// The citation gate.
///
/// Extracts `[En]` markers and resolves them against the pack. A marker that
/// resolves **stays in the text** — the renderer turns it into an inline
/// evidence chip, so a reader can see which clause rests on which observation,
/// and the sentence keeps its grammar (stripping a sentence-initial `[E1]` left
/// answers starting "indicates that…").
///
/// The answer is admitted whole or not at all (P86-01). Every sentence must carry
/// its own resolvable marker: one valid `[E1]` used to carry every uncited
/// sentence around it, and a marker naming evidence the pack does not hold was
/// removed while the claim it was attached to stayed on screen. A sentence ends
/// at a line break, or at `. ! ? ; … ؟ ؛ ۔` followed by whitespace, a closing
/// quote or bracket, or the end. A stop glued to the next word could be one
/// sentence or two, so it refuses the answer rather than guessing where an
/// Arabic or English sentence ends; a decimal point between digits is not a stop.
/// This is a parse, not a proof of meaning: a cited sentence can still be wrong.
///
/// Returns `None` when any sentence is uncited, any marker does not resolve, a
/// boundary cannot be read, no word is left, or the model used its own refusal
/// token — each means there is no grounded answer here to show.
pub fn ground(raw: &str, pack: &TypedEvidencePack) -> Option<(String, Vec<Citation>)> {
    let mut citations: Vec<Citation> = Vec::new();
    let mut text = String::with_capacity(raw.len());
    // The same answer with every marker removed. Decides whether any word is
    // left, and whether the model's whole reply was its refusal token.
    let mut prose = String::with_capacity(raw.len());
    // The sentence being read: whether it says anything, and whether it cites.
    let mut says = false;
    let mut cites = false;
    let mut cursor = 0usize;

    while let Some(ch) = raw[cursor..].chars().next() {
        if let Some((index, len)) = marker_at(&raw[cursor..]) {
            let item = index.checked_sub(1).and_then(|i| pack.items.get(i))?;
            let citation = Citation {
                evidence_id: item.evidence_id.clone(),
                surface: item.surface,
            };
            if !citations.contains(&citation) {
                citations.push(citation);
            }
            // Normalised spelling, so the renderer has one shape to match
            // rather than `[e1]` and `[E1]` both.
            text.push_str(&format!("[E{index}]"));
            cites = true;
            cursor += len;
            continue;
        }
        cursor += ch.len_utf8();
        let after_number = text.chars().next_back().is_some_and(char::is_numeric);
        text.push(ch);
        prose.push(ch);
        says |= ch.is_alphanumeric();
        let next = raw[cursor..].chars().next();
        let ends = match ch {
            '\n' | '\r' => true,
            _ if is_stop(ch) => match next {
                None => true,
                Some(next) if next.is_whitespace() || is_closing(next) => true,
                // `...` or `?!`: the last stop decides.
                Some(next) if is_stop(next) => false,
                Some(next) if ch == '.' && after_number && next.is_numeric() => false,
                Some(_) => return None,
            },
            _ => false,
        };
        if ends {
            if says && !cites {
                return None;
            }
            says = false;
            cites = false;
        }
    }
    if says && !cites {
        return None;
    }

    let cleaned = collapse_whitespace(&text);
    if citations.is_empty() || !prose.chars().any(char::is_alphanumeric) {
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

/// `[E` + digits + `]` at the start of `rest`, either case of `E`: the index it
/// names and the marker's length in bytes. An index too long to parse is 0,
/// which resolves to nothing.
fn marker_at(rest: &str) -> Option<(usize, usize)> {
    let body = rest
        .strip_prefix("[E")
        .or_else(|| rest.strip_prefix("[e"))?;
    let digits = body.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 || !body[digits..].starts_with(']') {
        return None;
    }
    Some((body[..digits].parse().unwrap_or(0), digits + 3))
}

fn is_stop(ch: char) -> bool {
    matches!(ch, '.' | '!' | '?' | ';' | '…' | '؟' | '؛' | '۔')
}

fn is_closing(ch: char) -> bool {
    matches!(ch, ')' | '"' | '\'' | '”' | '’' | '»')
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
            Some(reasoner) if reasoner.is_loading() => "loading",
            _ => "disabled",
        }
    }

    /// Runs one turn. `sink` receives the accumulated PROVISIONAL answer as it
    /// grows; the outcome is what the caller is allowed to present.
    pub fn ask(
        &self,
        pack: &TypedEvidencePack,
        question: &str,
        locale: Locale,
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
        if reasoner.is_loading() {
            return TurnOutcome::Faulted {
                fault_key: FAULT_MODEL_LOADING,
                detail: "the embedded model is still loading".into(),
            };
        }
        if !reasoner.is_loaded() {
            return TurnOutcome::Faulted {
                fault_key: FAULT_MODEL_UNAVAILABLE,
                detail: "the embedded artifact did not load".into(),
            };
        }
        if pack.propositions.is_empty() {
            return TurnOutcome::Refused(RefusalReason::NotCovered);
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
        let result = reasoner.generate(pack, bounded, locale, &budget, sink);
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
            Ok(generated) => {
                let model_answer = render_fact_selection(&generated.text, pack, locale);
                let engine = if model_answer.is_some() {
                    crate::model::InsightEngineKind::LocalModel
                } else {
                    crate::model::InsightEngineKind::RuleFallback
                };
                // An explicit empty selection is a refusal, never rewritten as a summary.
                let answer = if serde_json::from_str::<FactSelection>(&generated.text)
                    .is_ok_and(|selection| selection.facts.is_empty())
                    || generated.text.trim() == "NO EVIDENCE"
                {
                    None
                } else {
                    model_answer.or_else(|| fallback_facts(pack, bounded, locale))
                };
                match answer {
                    Some((answer, citations)) => TurnOutcome::Answered {
                        answer,
                        citations,
                        tokens: generated.tokens,
                        engine,
                    },
                    // Text that cites nothing resolvable is discarded here, and this
                    // is the line that keeps an uncited claim off the screen.
                    None => TurnOutcome::Refused(RefusalReason::NotCovered),
                }
            }
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
            pack.push_fact(
                Citation {
                    evidence_id: (*id).into(),
                    surface: EvidenceSurface::TimelinePattern,
                },
                crate::model::Fact::RepeatedFailure { occurrences: 2 },
            );
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
            _locale: Locale,
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
            Locale::En,
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
            Locale::En,
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
        let engine = engine(answered(r#"{"facts":[1,2]}"#));
        let outcome = engine.ask(
            &pack_of(&["fact-a", "fact-b"]),
            "what ran?",
            Locale::En,
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
                    "A failure recurred 2 times [E1]. A failure recurred 2 times [E2]."
                );
                assert_eq!(citations.len(), 2);
                assert_eq!(citations[0].evidence_id, "fact-a");
                assert_eq!(citations[1].evidence_id, "fact-b");
            }
            other => panic!("expected an answer, got {other:?}"),
        }
    }

    /// A marker naming evidence the pack does not hold refuses the answer. It
    /// used to be removed while the claim it was attached to stayed (P86-01).
    #[test]
    fn an_unresolvable_marker_refuses_the_whole_answer() {
        let pack = pack_of(&["fact-a"]);
        assert!(ground("The disk is fine [E1] and the fan failed [E9].", &pack).is_none());
        assert!(ground("The disk is fine [E1] [E99999999999999999999999].", &pack).is_none());
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
            Locale::En,
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
            Locale::En,
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
                _locale: Locale,
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
                Locale::En,
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
                Locale::En,
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
                Locale::En,
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
                _locale: Locale,
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
            Locale::En,
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
                _locale: Locale,
                _: &GenerationBudget,
                _: &mut dyn FnMut(&str),
            ) -> Result<Generated, String> {
                // Re-enter while the lane is held, which is what a second window
                // asking a question at the same moment does.
                if let Some(engine) = self.inner.lock().unwrap().as_ref() {
                    let outcome = engine.ask(
                        pack,
                        "second",
                        Locale::En,
                        false,
                        Arc::new(AtomicBool::new(false)),
                        &mut |_| {},
                    );
                    *self.nested.lock().unwrap() = Some(outcome);
                }
                Ok(Generated {
                    text: r#"{"facts":[1]}"#.into(),
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
                locale: Locale,
                budget: &GenerationBudget,
                sink: &mut dyn FnMut(&str),
            ) -> Result<Generated, String> {
                self.0.generate(pack, question, locale, budget, sink)
            }
        }
        let engine = Arc::new(AssistantEngine::new(Some(Box::new(Proxy(Arc::clone(
            &reasoner,
        ))))));
        *reasoner.inner.lock().unwrap() = Some(Arc::clone(&engine));

        let outcome = engine.ask(
            &pack_of(&["fact-a"]),
            "first",
            Locale::En,
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
                Locale::En,
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
                _locale: Locale,
                _: &GenerationBudget,
                _: &mut dyn FnMut(&str),
            ) -> Result<Generated, String> {
                if !self.0.swap(true, Ordering::SeqCst) {
                    panic!("llama.cpp aborted mid-turn");
                }
                answered(r#"{"facts":[1]}"#)
            }
        }
        let engine = AssistantEngine::new(Some(Box::new(PanicsOnce(AtomicBool::new(false)))));
        let ask = || {
            engine.ask(
                &pack_of(&["fact-a"]),
                "q",
                Locale::En,
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
                _locale: Locale,
                _: &GenerationBudget,
                _: &mut dyn FnMut(&str),
            ) -> Result<Generated, String> {
                assert!(question.len() <= MAX_QUESTION_CHARS);
                assert!(question.is_char_boundary(question.len()));
                Ok(Generated {
                    text: r#"{"facts":[1]}"#.into(),
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
                Locale::En,
                false,
                Arc::new(AtomicBool::new(false)),
                &mut |_| {},
            ),
            TurnOutcome::Answered { .. },
        ));
    }
}
