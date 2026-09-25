//! Phase 56 — does the embedded model actually generate a token?
//!
//! `DBT-P56-002`: it never has. `LlamaCppReasoner::infer_embedded` returned
//! `Err` unconditionally — "token-level generation requires the context pool
//! wired in intelligence.rs" — so the artifact verified its sha256, loaded, built
//! a context, and then every single request fell through to the deterministic
//! rule engine. The product has been reporting `localModel` for an engine that
//! has produced nothing.
//!
//! Nothing already in this repository would have caught that. `adversarial.rs`
//! and `offline_boundary.rs` both exercise the SELECTOR, whose whole job is to
//! degrade silently when the model path fails — so the model path failing on
//! every call is, to those tests, the expected behaviour.
//!
//! This is the test that fails when the model cannot generate. It runs against
//! the real shipped artifact, because a mock cannot prove that llama.cpp is
//! linked, that the gguf loads, or that a token comes out.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use aethercore_intelligence_core::{
    AssistantEngine, EMBEDDED_MODEL_RELATIVE_PATH, EvidenceItem, EvidenceSurface, GenerationBudget,
    INFERENCE_TIMEOUT, InsightConfidence, InsightEngineKind, LlamaCppReasoner, LocalReasoner,
    ReasonerSelector, RefusalReason, StreamingReasoner, TurnOutcome, TypedEvidencePack,
    embedded_model_entry, verify_model_hash,
};

/// The workspace root, two levels above this crate.
fn product_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/<name> sits two levels under the product root")
        .to_path_buf()
}

fn pack() -> TypedEvidencePack {
    let mut pack = TypedEvidencePack::default();
    pack.push(EvidenceItem {
        evidence_id: "plan-cleanup-0042".into(),
        surface: EvidenceSurface::MaintenanceHistory,
        detail: "plan plan-cleanup-0042 domain cleanup stage completed, reclaimed 18.4 GB".into(),
    });
    pack.push(EvidenceItem {
        evidence_id: "pattern-thermal-7".into(),
        surface: EvidenceSurface::TimelinePattern,
        detail: "event class Thermal code THRM-7 recurred on 3 of the last 5 sessions".into(),
    });
    pack
}

/// The artifact must be present for any of this to mean anything. A skipped run
/// of the test below would be indistinguishable from a passing one, so its
/// absence is asserted separately and loudly rather than silently tolerated.
#[test]
fn the_shipped_artifact_is_present_and_matches_its_pinned_hash() {
    let path = product_root().join(EMBEDDED_MODEL_RELATIVE_PATH);
    assert!(
        path.exists(),
        "the embedded model is not in the tree at {}; every generation claim below is untestable \
         until it is",
        path.display()
    );
    verify_model_hash(&path, &embedded_model_entry()).expect("pinned sha256 must match");
}

/// DBT-P62-004: the four tests below each load a 1.5B model and generate against it.
/// `cargo test` runs them on parallel threads, so on the 2-vCPU Windows runner four
/// llama contexts compete for two cores and `ASSISTANT_DEADLINE` — 20 seconds, a
/// PRODUCT constant covering generation only — expires before the first token.
/// Measured: run 34760963140 passed this binary, run 34765013157 reported
/// "deadline exceeded after 0 token(s)" for four of them with two logged as "running
/// for over 60 seconds". Same code, same deadline, different scheduling.
///
/// Serialising is not masking a product defect, and is the opposite of the P36 lock
/// this phase deleted from the cleaner tests: nothing about the product runs four
/// concurrent generations on one machine, and a throughput measurement taken under 4x
/// CPU oversubscription is not a measurement of anything the product does. Each test
/// gets the machine.
static ONE_MODEL_AT_A_TIME: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// DBT-P62-004 is retired on a MEASURED wall time on the runner, not an exit
/// code. `eprintln!` is captured by the test harness and shown only on failure,
/// so a passing run would log nothing; a direct write to the stderr handle is
/// not captured, and lands in the CI log on every run.
fn measured(line: &str) {
    use std::io::Write;
    let _ = writeln!(std::io::stderr(), "DBT-P62-004 measured: {line}");
}

/// The one that fails while `DBT-P56-002` is open.
#[test]
fn the_embedded_model_generates_tokens_over_an_evidence_pack() {
    let _serialised = ONE_MODEL_AT_A_TIME
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let path = product_root().join(EMBEDDED_MODEL_RELATIVE_PATH);
    verify_model_hash(&path, &embedded_model_entry()).expect("pinned sha256 must match");

    let mut reasoner = LlamaCppReasoner::new();
    reasoner.load(&path).expect("the artifact must load");
    assert!(StreamingReasoner::is_loaded(&reasoner));

    let mut streamed: Vec<String> = Vec::new();
    let budget = GenerationBudget::new(Arc::new(AtomicBool::new(false)));
    let started = Instant::now();
    let generated = reasoner
        .generate(
            &pack(),
            "what maintenance has run on this machine?",
            &budget,
            &mut |accumulated| streamed.push(accumulated.to_owned()),
        )
        .expect("generation must produce text, not a fault");
    measured(&format!(
        "assistant generation: {} token(s) in {} ms (deadline {} ms)",
        generated.tokens,
        started.elapsed().as_millis(),
        aethercore_intelligence_core::ASSISTANT_DEADLINE.as_millis()
    ));

    assert!(
        generated.tokens > 0,
        "the model emitted no tokens: {generated:?}"
    );
    assert!(
        !generated.text.trim().is_empty(),
        "the model emitted only whitespace: {generated:?}"
    );
    assert!(!generated.cancelled);
    // Streaming is accumulated, never fragments: each callback must extend the
    // previous one, so a client that missed an event is never left with a torn
    // sentence.
    assert!(!streamed.is_empty(), "nothing was streamed");
    for pair in streamed.windows(2) {
        assert!(
            pair[1].starts_with(&pair[0]),
            "stream went backwards:\n{:?}\n{:?}",
            pair[0],
            pair[1]
        );
    }
    assert_eq!(
        streamed.last().map(String::as_str),
        Some(generated.text.as_str())
    );
}

/// Cancellation must stop the REAL loop, not merely be a flag the loop could
/// have read. Raised from the stream callback, which is where a user's Escape
/// reaches it: between two tokens, while generation is running.
#[test]
fn cancelling_the_real_loop_stops_generation_early() {
    let _serialised = ONE_MODEL_AT_A_TIME
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let path = product_root().join(EMBEDDED_MODEL_RELATIVE_PATH);
    verify_model_hash(&path, &embedded_model_entry()).expect("pinned sha256 must match");

    let mut reasoner = LlamaCppReasoner::new();
    reasoner.load(&path).expect("the artifact must load");

    let cancel = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&cancel);
    let budget = GenerationBudget::new(cancel);
    let mut callbacks = 0u32;
    let generated = reasoner
        .generate(
            &pack(),
            "what maintenance has run on this machine?",
            &budget,
            &mut |_| {
                callbacks += 1;
                if callbacks == 3 {
                    flag.store(true, std::sync::atomic::Ordering::SeqCst);
                }
            },
        )
        .expect("a cancelled run reports what it had, it does not fault");

    assert!(generated.cancelled, "the turn must report that it stopped");
    assert_eq!(
        generated.tokens, 3,
        "generation must stop at the token the flag was raised on, not run to the ceiling"
    );
}

fn loaded_engine() -> AssistantEngine {
    let path = product_root().join(EMBEDDED_MODEL_RELATIVE_PATH);
    verify_model_hash(&path, &embedded_model_entry()).expect("pinned sha256 must match");
    let mut reasoner = LlamaCppReasoner::new();
    reasoner.load(&path).expect("the artifact must load");
    AssistantEngine::new(Some(Box::new(reasoner)))
}

/// The pack the service actually composes: maintenance rows keyed by plan UUIDs
/// and timeline patterns keyed by 64-hex semantic identities. Measured on this
/// shape rather than on `pack()` because prompt length is what a slow CPU pays
/// for, and a two-item pack of short ids would measure a prompt the product
/// never sends.
fn product_shaped_pack() -> TypedEvidencePack {
    let mut pack = TypedEvidencePack::default();
    for (index, (domain, stage)) in [
        ("Cleanup", "Completed"),
        ("Startup", "Completed"),
        ("WindowsRepair", "RecoveryRequired"),
        ("Drivers", "Completed"),
        ("Cleanup", "Completed"),
        ("Startup", "Failed"),
    ]
    .iter()
    .enumerate()
    {
        let id = format!("5f0c2a1e-8b7d-4c3a-9e21-00000000000{index}");
        pack.push(EvidenceItem {
            evidence_id: id.clone(),
            surface: EvidenceSurface::MaintenanceHistory,
            detail: format!("plan {id} domain {domain} stage {stage}"),
        });
    }
    for (id, code, count) in [
        (
            "3c9d0e5a7f1b2c4d6e8f0a1b3c5d7e9f1a2b4c6d8e0f1a3b5c7d9e1f2a4b6c8d",
            "verify-dism",
            5,
        ),
        (
            "e41b7a9c2d5f8e0a3b6c9d2e5f8a1b4c7d0e3f6a9b2c5d8e1f4a7b0c3d6e9f2a",
            "journal.transition:Failed",
            3,
        ),
    ] {
        pack.push(EvidenceItem {
            evidence_id: id.into(),
            surface: EvidenceSurface::TimelinePattern,
            detail: format!(
                "recurring failure: class operation code {code}, {count} occurrences, recurrence confidence weak"
            ),
        });
    }
    pack
}

/// P75 — the INSIGHT path over the real model. `DBT-P56-002`'s other half: until
/// this, `infer_embedded` returned `Err` unconditionally and every insight was
/// the rule engine's, whatever the badge said.
///
/// Served through the selector, because that is the product path: its 10 s
/// `INFERENCE_TIMEOUT` is the budget the RPC thread runs under, and the desktop
/// client gives up at 15 s. Prints its wall time so the Windows CI log carries
/// the `DBT-P62-004` measurement, not only an exit code.
///
/// Whether the model fits the budget is a property of the host, not of the code:
/// CI `36180691145` measured the 2-vCPU Windows runner at `deadline exceeded after
/// 640 of 927 prompt token(s)`. So every host must answer truthfully (cited, and
/// badged with the engine that served it); only macOS, where Metal measured
/// 3 insights in 2583 ms, must be served by the model.
#[test]
fn the_real_model_insight_path_is_cited_and_truthfully_badged() {
    let _serialised = ONE_MODEL_AT_A_TIME
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let path = product_root().join(EMBEDDED_MODEL_RELATIVE_PATH);
    verify_model_hash(&path, &embedded_model_entry()).expect("pinned sha256 must match");
    let mut reasoner = LlamaCppReasoner::new();
    reasoner.load(&path).expect("the artifact must load");

    let pack = product_shaped_pack();
    let question = "what stands out on this machine?";

    // The generation on its own first, for its token count and the text.
    let started = Instant::now();
    let generated = reasoner.generate_insight_text(&pack, question, started + INFERENCE_TIMEOUT);
    let elapsed = started.elapsed().as_millis();
    match &generated {
        Ok(g) => measured(&format!(
            "insight generation: {} token(s) in {elapsed} ms (budget {} ms): {:?}",
            g.tokens,
            INFERENCE_TIMEOUT.as_millis(),
            g.text
        )),
        Err(e) => measured(&format!(
            "insight generation did not fit: {e:?} after {elapsed} ms (budget {} ms)",
            INFERENCE_TIMEOUT.as_millis()
        )),
    }
    #[cfg(target_os = "macos")]
    generated.expect("insight generation must fit its budget on macOS");

    // Then the product path, which is what the badge is promised on.
    let selector = ReasonerSelector::new(Some(Box::new(reasoner)));
    let started = Instant::now();
    let insights = selector
        .request_insights(&pack, question, false)
        .expect("a loaded model with evidence answers");
    let wall = started.elapsed();
    let engine = insights.first().map(|i| i.engine);
    measured(&format!(
        "insight via selector: {} insight(s) from {engine:?} in {} ms (budget {} ms)",
        insights.len(),
        wall.as_millis(),
        INFERENCE_TIMEOUT.as_millis()
    ));

    assert!(
        !insights.is_empty(),
        "nothing cited was served: {insights:?}"
    );
    for insight in &insights {
        assert_eq!(
            Some(insight.engine),
            engine,
            "one engine per answer: {insight:?}"
        );
        assert!(!insight.citations.is_empty(), "{insight:?}");
        for citation in &insight.citations {
            assert!(pack.resolves(citation), "{citation:?} does not resolve");
        }
    }
    match engine {
        Some(InsightEngineKind::LocalModel) => {
            assert!(
                wall < INFERENCE_TIMEOUT,
                "the model path overran its budget"
            );
            for insight in &insights {
                assert_ne!(
                    insight.confidence,
                    InsightConfidence::Strong,
                    "a model finding is never Strong: {insight:?}"
                );
            }
            assert_eq!(selector.engine_label(), "localModel");
        }
        _ => {
            assert_eq!(selector.engine_label(), "ruleFallback");
            #[cfg(target_os = "macos")]
            panic!("served by the rule engine on macOS: {insights:?}");
        }
    }
}

/// P75 — one model, one generation at a time. The service used to load the
/// artifact twice with two independent busy lanes, so an insight and an
/// assistant turn could decode at once on a 2-vCPU machine. The two paths now
/// share one reasoner, and whichever finds it generating gets `MODEL_BUSY` at
/// once — asked here from INSIDE a running assistant generation, which is the
/// only moment the question means anything.
#[test]
fn an_insight_asked_while_the_assistant_generates_falls_back_at_once() {
    let _serialised = ONE_MODEL_AT_A_TIME
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let (reasoner, _label) =
        aethercore_intelligence_core::activate_embedded_reasoner(&product_root())
            .expect("the artifact must activate");
    let selector = ReasonerSelector::new(Some(Box::new(reasoner.clone())));
    let pack = pack();

    let cancel = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&cancel);
    let mut during: Option<(
        Result<Vec<aethercore_intelligence_core::Insight>, String>,
        _,
    )> = None;
    let generated = reasoner
        .generate(
            &pack,
            "what maintenance has run on this machine?",
            &GenerationBudget::new(cancel),
            &mut |_| {
                if during.is_none() {
                    let asked = Instant::now();
                    let direct =
                        LocalReasoner::infer(&reasoner, &pack, "q", asked + INFERENCE_TIMEOUT);
                    let served = selector.request_insights(&pack, "q", false);
                    during = Some((direct, (served, asked.elapsed())));
                    flag.store(true, std::sync::atomic::Ordering::SeqCst);
                }
            },
        )
        .expect("the assistant generation itself runs");
    assert!(generated.cancelled);

    let (direct, (served, waited)) = during.expect("the sink ran");
    assert_eq!(
        direct,
        Err(aethercore_intelligence_core::MODEL_BUSY.to_owned())
    );
    let served = served.expect("the selector serves");
    assert!(!served.is_empty());
    assert!(
        served
            .iter()
            .all(|insight| insight.engine == InsightEngineKind::RuleFallback),
        "{served:?}"
    );
    assert_eq!(selector.engine_label(), "ruleFallback");
    assert!(
        waited < std::time::Duration::from_secs(1),
        "the fallback must not queue behind the generation: {waited:?}"
    );
}

/// End to end, against the real model: the failure that matters, proven on the
/// thing that would actually commit it.
///
/// A general chatbot answers "what is the capital of France?" instantly and
/// correctly. This product must not, because nothing it has measured about this
/// computer bears on the question — and an assistant that will answer THAT from
/// training is one that will also answer "is my disk failing?" from training.
#[test]
fn the_real_model_refuses_a_question_its_evidence_cannot_answer() {
    let _serialised = ONE_MODEL_AT_A_TIME
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let engine = loaded_engine();
    assert_eq!(engine.engine_label(), "localModel");
    let outcome = engine.ask(
        &pack(),
        "what is the capital of France?",
        false,
        Arc::new(AtomicBool::new(false)),
        &mut |_| {},
    );
    assert_eq!(
        outcome,
        TurnOutcome::Refused(RefusalReason::NotCovered),
        "a question the evidence cannot answer must be refused, not answered"
    );
}

/// And the other side of it: a question the evidence DOES answer comes back
/// grounded, with every citation resolving against the pack it was given.
#[test]
fn the_real_model_answers_a_question_its_evidence_covers_and_cites_it() {
    let _serialised = ONE_MODEL_AT_A_TIME
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let engine = loaded_engine();
    let pack = pack();
    let outcome = engine.ask(
        &pack,
        "what maintenance has run on this machine?",
        false,
        Arc::new(AtomicBool::new(false)),
        &mut |_| {},
    );
    match outcome {
        TurnOutcome::Answered {
            answer,
            citations,
            tokens,
        } => {
            assert!(tokens > 0);
            assert!(!citations.is_empty(), "an answer must cite: {answer:?}");
            for citation in &citations {
                assert!(
                    pack.resolves(citation),
                    "citation {citation:?} does not resolve against the pack it came from"
                );
            }
            // The marker survives into the text so the renderer can turn it into
            // an inline chip beside the clause it belongs to.
            assert!(answer.contains("[E"), "no inline marker in {answer:?}");
        }
        other => panic!("expected a grounded answer, got {other:?}"),
    }
}
