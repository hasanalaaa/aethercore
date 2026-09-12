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

use aethercore_intelligence_core::{
    EMBEDDED_MODEL_RELATIVE_PATH, EvidenceItem, EvidenceSurface, GenerationBudget, LlamaCppReasoner,
    LocalReasoner, StreamingReasoner, TypedEvidencePack, embedded_model_entry, verify_model_hash,
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

/// The one that fails while `DBT-P56-002` is open.
#[test]
fn the_embedded_model_generates_tokens_over_an_evidence_pack() {
    let path = product_root().join(EMBEDDED_MODEL_RELATIVE_PATH);
    verify_model_hash(&path, &embedded_model_entry()).expect("pinned sha256 must match");

    let mut reasoner = LlamaCppReasoner::new();
    reasoner.load(&path).expect("the artifact must load");
    assert!(StreamingReasoner::is_loaded(&reasoner));

    let mut streamed: Vec<String> = Vec::new();
    let budget = GenerationBudget::new(Arc::new(AtomicBool::new(false)));
    let generated = reasoner
        .generate(
            &pack(),
            "what maintenance has run on this machine?",
            &budget,
            &mut |accumulated| streamed.push(accumulated.to_owned()),
        )
        .expect("generation must produce text, not a fault");

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
    assert_eq!(streamed.last().map(String::as_str), Some(generated.text.as_str()));
}

/// Cancellation must stop the REAL loop, not merely be a flag the loop could
/// have read. Raised from the stream callback, which is where a user's Escape
/// reaches it: between two tokens, while generation is running.
#[test]
fn cancelling_the_real_loop_stops_generation_early() {
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
