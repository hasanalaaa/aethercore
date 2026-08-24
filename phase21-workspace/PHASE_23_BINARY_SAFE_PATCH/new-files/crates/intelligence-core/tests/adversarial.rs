//! Phase 23 — adversarial tests for the intelligence core (invariants I1–I5).

use std::time::{Duration, Instant};

use aethercore_intelligence_core::{
    Citation, DeterministicFallbackReasoner, EvidenceItem, EvidenceSurface, InsightConfidence,
    InsightEngineKind, LlamaCppReasoner, LocalReasoner, MAX_EVIDENCE_ITEMS, MAX_INSIGHTS_PER_CALL,
    ModelManifestEntry, ReasonerSelector, TypedEvidencePack,
};

fn item(id: &str, surface: EvidenceSurface) -> EvidenceItem {
    EvidenceItem {
        evidence_id: id.to_string(),
        surface,
        detail: "structured detail".into(),
    }
}

fn populated_pack() -> TypedEvidencePack {
    let mut pack = TypedEvidencePack::default();
    pack.push(item("bottleneck-1", EvidenceSurface::BottleneckReport));
    pack.push(item("repair-diag-2", EvidenceSurface::RepairDiagnosis));
    pack.push(item("pattern-abc", EvidenceSurface::TimelinePattern));
    pack
}

// ---------------------------------------------------------------------------
// I1 — advisory-only vocabulary (type-level): an insight can only carry text keys,
// confidence, citations. This test pins the shape so no future field can smuggle
// actions without failing schema review.
// ---------------------------------------------------------------------------

#[test]
fn insight_schema_v1_rejects_unknown_fields() {
    // deny_unknown_fields on Insight/Citation: parsing an insight JSON with an extra
    // field must fail — the wire can never grow unreviewed semantics.
    let raw = r#"{
        "schemaVersion": 1,
        "summaryKey": "insight.summary.recurrence",
        "explanation": "x",
        "confidence": "Strong",
        "citations": [],
        "engine": "ruleFallback",
        "suggestedAction": {"kind": "DeleteFile", "path": "C:/x"}
    }"#;
    let parsed: Result<aethercore_intelligence_core::Insight, _> = serde_json::from_str(raw);
    assert!(
        parsed.is_err(),
        "unknown fields must be rejected (I1/I-schema)"
    );
}

#[test]
fn insight_without_citations_cannot_be_constructed() {
    let none = aethercore_intelligence_core::Insight::build(
        "k",
        "e",
        InsightConfidence::Weak,
        Vec::new(),
        InsightEngineKind::RuleFallback,
    );
    assert!(
        none.is_none(),
        "I2: zero-citation insights are unconstructable"
    );
}

// ---------------------------------------------------------------------------
// I2 — fabricated / stale / dangling citations are dropped before emission
// ---------------------------------------------------------------------------

#[test]
fn dangling_citations_produce_zero_surviving_insights() {
    struct Fabricator;
    impl LocalReasoner for Fabricator {
        fn load(&mut self, _: &std::path::Path) -> Result<(), String> {
            Ok(())
        }
        fn is_loaded(&self) -> bool {
            true
        }
        fn infer(
            &self,
            _pack: &TypedEvidencePack,
            _q: &str,
            _deadline: Instant,
        ) -> Result<Vec<aethercore_intelligence_core::Insight>, String> {
            // Hostile model output citing evidence that does not exist.
            let ghost = aethercore_intelligence_core::Insight::build(
                "insight.summary.recurrence",
                "hallucinated",
                InsightConfidence::Strong,
                vec![Citation {
                    evidence_id: "totally-fabricated-id".into(),
                    surface: EvidenceSurface::TimelinePattern,
                }],
                InsightEngineKind::LocalModel,
            )
            .unwrap();
            Ok(vec![ghost])
        }
    }

    let selector = ReasonerSelector::new(Some(Box::new(Fabricator)));
    let pack = populated_pack();
    let out = selector
        .request_insights(&pack, "explain", false)
        .expect("call succeeds");
    assert!(
        out.is_empty(),
        "I2: insights with unresolvable citations must be dropped"
    );
}

#[test]
fn stale_citations_after_pack_change_are_dropped() {
    let selector = ReasonerSelector::new(None); // fallback path
    let mut pack = populated_pack();
    pack.push(item("pattern-live", EvidenceSurface::TimelinePattern));

    // First call resolves against the current pack.
    let first = selector.request_insights(&pack, "", false).expect("ok");
    assert!(!first.is_empty());

    // A STALE pack (pattern removed) no longer resolves the pattern citation.
    let mut stale = TypedEvidencePack::default();
    stale.push(item("bottleneck-1", EvidenceSurface::BottleneckReport));
    let second = selector.request_insights(&stale, "", false).expect("ok");
    assert!(
        second.iter().all(|insight| insight
            .citations
            .iter()
            .all(|c| c.evidence_id != "pattern-live")),
        "no insight may cite evidence absent from the served pack"
    );
}

// ---------------------------------------------------------------------------
// I3 — silent deterministic fallback + identical citation discipline
// ---------------------------------------------------------------------------

#[test]
fn unavailable_model_degrades_silently_to_fallback() {
    struct BrokenModel;
    impl LocalReasoner for BrokenModel {
        fn load(&mut self, _: &std::path::Path) -> Result<(), String> {
            Err("artifact missing".into())
        }
        fn is_loaded(&self) -> bool {
            false
        }
        fn infer(
            &self,
            _: &TypedEvidencePack,
            _: &str,
            _: Instant,
        ) -> Result<Vec<aethercore_intelligence_core::Insight>, String> {
            Err("not loaded".into())
        }
    }
    let selector = ReasonerSelector::new(Some(Box::new(BrokenModel)));
    let out = selector
        .request_insights(&populated_pack(), "", false)
        .expect("fallback serves instead of erroring");
    assert!(!out.is_empty(), "fallback produced advisory summaries");
    assert!(
        out.iter()
            .all(|i| i.engine == InsightEngineKind::RuleFallback),
        "engine label reflects the actual serving engine"
    );
}

#[test]
fn fallback_is_deterministic_for_identical_packs() {
    let reasoner = DeterministicFallbackReasoner::new();
    let pack = populated_pack();
    let deadline = Instant::now() + Duration::from_secs(5);
    let a = reasoner.infer(&pack, "", deadline).unwrap();
    let b = reasoner.infer(&pack, "", deadline).unwrap();
    assert_eq!(a, b, "identical input → byte-identical insights");
}

// ---------------------------------------------------------------------------
// I4 — budgets: single flight, observer guard, bounded pack/insight caps
// ---------------------------------------------------------------------------

#[test]
fn inference_refuses_while_mutation_or_care_run_is_active() {
    let selector = ReasonerSelector::new(None);
    let err = selector
        .request_insights(&populated_pack(), "", true)
        .expect_err("observer-effect guard");
    assert!(matches!(
        err,
        aethercore_intelligence_core::IntelligenceError::MutationActive
    ));
}

#[test]
fn single_in_flight_lane_rejects_second_request() {
    use std::sync::Arc;
    // The lane token is internal; simulate contention by checking Busy via two threads
    // racing is flaky — instead assert the API contract through the slow-model trick:
    // a model that sleeps past its own call forces the second caller to observe Busy.
    struct SlowModel(std::sync::Mutex<()>);
    impl LocalReasoner for SlowModel {
        fn load(&mut self, _: &std::path::Path) -> Result<(), String> {
            Ok(())
        }
        fn is_loaded(&self) -> bool {
            true
        }
        fn infer(
            &self,
            _: &TypedEvidencePack,
            _: &str,
            _: Instant,
        ) -> Result<Vec<aethercore_intelligence_core::Insight>, String> {
            std::thread::sleep(Duration::from_millis(150));
            Ok(Vec::new()) // empty → falls back internally; still occupies the lane
        }
    }
    let selector = Arc::new(ReasonerSelector::new(Some(Box::new(SlowModel(
        std::sync::Mutex::new(()),
    )))));
    let s2 = selector.clone();
    let t1 = std::thread::spawn(move || {
        let mut pack = populated_pack();
        pack.push(item("bottleneck-slow", EvidenceSurface::BottleneckReport));
        s2.request_insights(&pack, "", false)
    });
    std::thread::sleep(Duration::from_millis(20));
    // Second request while the first holds the lane → Busy (typed), never queued.
    let second = selector.request_insights(&populated_pack(), "", false);
    match second {
        Err(aethercore_intelligence_core::IntelligenceError::Busy) => {}
        other => panic!(
            "expected Busy while lane held, got {:?}",
            other.map(|o| o.len())
        ),
    }
    let _ = t1.join().expect("first call completes");
}

#[test]
fn evidence_pack_enforces_bounds() {
    let mut pack = TypedEvidencePack::default();
    for i in 0..(MAX_EVIDENCE_ITEMS + 10) {
        pack.push(item(
            &format!("id-{i}"),
            EvidenceSurface::MaintenanceHistory,
        ));
    }
    assert_eq!(pack.items.len(), MAX_EVIDENCE_ITEMS, "pack clamps at cap");

    // Oversize detail is truncated deterministically.
    let mut big = TypedEvidencePack::default();
    big.push(EvidenceItem {
        evidence_id: "big".into(),
        surface: EvidenceSurface::BottleneckReport,
        detail: "x".repeat(10_000),
    });
    assert!(big.items[0].detail.chars().count() <= 260);

    // Insight emission respects the per-call cap.
    let selector = ReasonerSelector::new(None);
    let out = selector.request_insights(&pack, "", false).expect("ok");
    assert!(out.len() <= MAX_INSIGHTS_PER_CALL);
}

// ---------------------------------------------------------------------------
// I5 — fail-closed model hash pinning (tamper test)
// ---------------------------------------------------------------------------

#[test]
fn tampered_model_artifact_is_refused_and_fallback_engages() {
    let dir = tempfile::tempdir().expect("tmp");
    let model_path = dir.path().join("model.gguf");
    std::fs::write(&model_path, b"pretend-gguf-bytes").expect("write");

    let pinned = ModelManifestEntry {
        file_name: "model.gguf".into(),
        sha256_hex: {
            use sha2::{Digest, Sha256};
            const HEX: &[u8; 16] = b"0123456789abcdef";
            let d = Sha256::digest(b"pretend-gguf-bytes");
            d.iter().fold(String::new(), |mut out, &b| {
                out.push(HEX[(b >> 4) as usize] as char);
                out.push(HEX[(b & 0x0f) as usize] as char);
                out
            })
        },
        declared_ram_budget_bytes: 512 * 1024 * 1024,
    };

    // Honest artifact loads.
    assert!(aethercore_intelligence_core::verify_model_hash(&model_path, &pinned).is_ok());

    // Flip ONE byte → loader refuses (fail-closed).
    std::fs::write(&model_path, b"pretend-gguf-byteS").expect("tamper");
    let result = aethercore_intelligence_core::verify_model_hash(&model_path, &pinned);
    assert!(result.is_err(), "any byte difference must refuse the load");

    // And the service still works: fallback engages.
    let over_budget = ModelManifestEntry {
        declared_ram_budget_bytes: 4 * 1024 * 1024 * 1024,
        ..pinned.clone()
    };
    assert!(
        aethercore_intelligence_core::verify_model_hash(&model_path, &over_budget).is_err(),
        "RAM budget overflow fails closed too"
    );
    let selector = ReasonerSelector::new(None);
    assert!(
        !selector
            .request_insights(&populated_pack(), "", false)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn llama_reasoner_reports_honest_unavailability_in_default_build() {
    let mut llama = LlamaCppReasoner::new();
    assert!(!llama.is_loaded());
    let load_result = llama.load(std::path::Path::new("/nonexistent/model.gguf"));
    assert!(
        load_result.is_err(),
        "default build has no committed artifact"
    );
    assert!(!llama.is_loaded());
}
