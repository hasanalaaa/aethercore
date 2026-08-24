//! Phase 21 — adversarial and determinism tests for Timeline Intelligence.
//!
//! Hostile inputs attacked here: future timestamps, clock skew, duplicate rows,
//! unbounded input, empty evidence groups. Determinism contract: identical input
//! MUST produce a byte-identical timeline digest regardless of insertion order.

use aethercore_timeline_intelligence::{
    EventClass, MAX_PAGE_SIZE, MAX_PATTERNS, MAX_TIMELINE_EVENTS, MIN_OCCURRENCES_FOR_PATTERN,
    Outcome, RecurrenceConfidence, TimelineBuilder, TimelineError, semantic_identity,
};

const WATERMARK: i64 = 1_800_000_000_000; // fixed "now" for every test

fn failure_event(
    source: &str,
    code: &str,
    at: i64,
) -> aethercore_timeline_intelligence::TimelineEvent {
    aethercore_timeline_intelligence::TimelineEvent::new(
        source,
        EventClass::Operation,
        "WindowsRepair",
        code,
        Outcome::Failed,
        at,
    )
}

fn neutral_event(
    source: &str,
    code: &str,
    at: i64,
) -> aethercore_timeline_intelligence::TimelineEvent {
    aethercore_timeline_intelligence::TimelineEvent::new(
        source,
        EventClass::Operation,
        "operationJournal",
        code,
        Outcome::Neutral,
        at,
    )
}

#[test]
fn rejects_future_timestamps_relative_to_watermark() {
    let mut builder = TimelineBuilder::new().watermark(WATERMARK);
    let error = builder
        .ingest(failure_event("e1", "code.a", WATERMARK + 1))
        .expect_err("future timestamp must be rejected");
    assert_eq!(
        error,
        TimelineError::FutureTimestamp {
            observed_unix_ms: WATERMARK + 1,
            watermark_unix_ms: WATERMARK
        }
    );
}

#[test]
fn accepts_timestamps_exactly_at_watermark() {
    let mut builder = TimelineBuilder::new().watermark(WATERMARK);
    builder
        .ingest(failure_event("e1", "code.a", WATERMARK))
        .expect("watermark-equal timestamp is not future");
    assert_eq!(builder.len(), 1);
}

#[test]
fn duplicates_collapse_by_semantic_identity_and_timestamp() {
    let mut builder = TimelineBuilder::new().watermark(WATERMARK);
    builder
        .ingest_all([
            failure_event("a", "code.x", 1000),
            failure_event("a", "code.x", 1000), // exact duplicate row (retry/replay)
            failure_event("b", "code.x", 1000), // same fact re-reported -> also collapses
            failure_event("a", "code.y", 1000), // different identity -> distinct
            failure_event("a", "code.x", 2000), // same identity, later time -> distinct
        ])
        .unwrap();
    let timeline = builder.build();
    assert_eq!(timeline.events.len(), 3);
    assert_eq!(timeline.duplicates_collapsed, 2);
}

#[test]
fn capacity_is_bounded_not_unbounded() {
    let mut builder = TimelineBuilder::new().watermark(WATERMARK);
    for index in 0..MAX_TIMELINE_EVENTS {
        let at = WATERMARK - ((index as i64) + 1) * 1000;
        builder
            .ingest(neutral_event(&format!("n{index}"), "fill", at))
            .expect("within capacity");
    }
    let error = builder
        .ingest(neutral_event("overflow", "fill", WATERMARK - 1_000_000))
        .expect_err("capacity must clamp ingestion");
    assert_eq!(
        error,
        TimelineError::CapacityExceeded {
            limit: MAX_TIMELINE_EVENTS
        }
    );
}

#[test]
fn ordering_is_total_and_ascending() {
    let mut builder = TimelineBuilder::new().watermark(WATERMARK);
    builder
        .ingest_all([
            failure_event("c", "code.z", 3000),
            failure_event("a", "code.m", 1000),
            failure_event("b", "code.z", 2000),
            failure_event("aa", "code.y", 3000), // same ts as c; identity tiebreak
        ])
        .unwrap();
    let timeline = builder.build();
    let stamps: Vec<i64> = timeline
        .events
        .iter()
        .map(|event| event.observed_unix_ms)
        .collect();
    let mut sorted = stamps.clone();
    sorted.sort();
    assert_eq!(
        stamps, sorted,
        "timeline must be ascending by observation time"
    );
    // Same-timestamp tiebreak is semantic identity ascending (source ids differ here).
    let same_ts: Vec<&str> = timeline
        .events
        .iter()
        .filter(|event| event.observed_unix_ms == 3000)
        .map(|event| event.source_id.as_str())
        .collect();
    let first_identity = semantic_identity(EventClass::Operation, "WindowsRepair", "code.y");
    let second_identity = semantic_identity(EventClass::Operation, "WindowsRepair", "code.z");
    let expected: Vec<&str> = if first_identity <= second_identity {
        vec!["aa", "c"]
    } else {
        vec!["c", "aa"]
    };
    assert_eq!(same_ts, expected, "tiebreak must follow the total order");
}

#[test]
fn determinism_identical_input_byte_identical_digest_any_insertion_order() {
    let base = vec![
        ("r1", 1000_i64),
        ("r2", 2000_i64),
        ("r3", 3000_i64),
        ("r4", 4000_i64),
        ("r5", 5000_i64),
        ("x1", 1500_i64),
        ("x2", 2500_i64),
    ];
    let build = |order: &[(&str, i64)]| -> String {
        let mut builder = TimelineBuilder::new().watermark(WATERMARK);
        for (source, at) in order {
            builder
                .ingest(failure_event(source, "recurring.code", *at))
                .unwrap();
        }
        builder.build().digest_sha256
    };
    let forward = build(&base);
    let mut reversed = base.clone();
    reversed.reverse();
    let backward = build(reversed.as_slice());
    let mut shuffled = base.clone();
    shuffled.swap(0, 5);
    shuffled.swap(3, 6);
    let shuffled_digest = build(shuffled.as_slice());
    assert_eq!(
        forward, backward,
        "insertion order must not change the digest"
    );
    assert_eq!(
        forward, shuffled_digest,
        "permutation must not change the digest"
    );
}

#[test]
fn recurrence_requires_full_evidence_matrix_minimum_occurrences() {
    // Two occurrences only — below the floor, so NOTHING may be emitted.
    let mut builder = TimelineBuilder::new().watermark(WATERMARK);
    builder
        .ingest_all([
            failure_event("f1", "flaky.rule", 1000),
            failure_event("f2", "flaky.rule", 2000),
        ])
        .unwrap();
    let timeline = builder.build();
    assert!(
        timeline.patterns.is_empty(),
        "a rule below the occurrence floor emits nothing"
    );

    // Exactly the floor: a Weak pattern becomes admissible.
    let mut builder = TimelineBuilder::new().watermark(WATERMARK);
    builder
        .ingest_all([
            failure_event("f1", "flaky.rule", 1000),
            failure_event("f2", "flaky.rule", 2000),
            failure_event("f3", "flaky.rule", 3500),
        ])
        .unwrap();
    let timeline = builder.build();
    assert_eq!(timeline.patterns.len(), 1);
    let pattern = &timeline.patterns[0];
    assert_eq!(pattern.confidence, RecurrenceConfidence::Weak);
    assert_eq!(pattern.occurrence_count, MIN_OCCURRENCES_FOR_PATTERN);
    assert_eq!(
        pattern.evidence.len(),
        3,
        "every occurrence cited as evidence"
    );
}

#[test]
fn success_outcomes_never_become_recurrence_evidence() {
    let mut builder = TimelineBuilder::new().watermark(WATERMARK);
    builder
        .ingest_all([
            neutral_event("s1", "happy.rule", 1000),
            neutral_event("s2", "happy.rule", 2000),
            neutral_event("s3", "happy.rule", 3000),
            neutral_event("s4", "happy.rule", 4000),
            neutral_event("s5", "happy.rule", 5000),
        ])
        .unwrap();
    let timeline = builder.build();
    assert!(
        timeline.patterns.is_empty(),
        "successes are timeline entries but never pattern input"
    );
}

#[test]
fn over_window_gaps_disqualify_the_run() {
    let month_ms = 31 * 24 * 60 * 60 * 1000_i64; // beyond MAX_RECURRENCE_GAP_MS
    let mut builder = TimelineBuilder::new().watermark(WATERMARK);
    builder
        .ingest_all([
            failure_event("g1", "sparse.rule", 1000),
            failure_event("g2", "sparse.rule", 1000 + month_ms),
            failure_event("g3", "sparse.rule", 1000 + 2 * month_ms),
        ])
        .unwrap();
    let timeline = builder.build();
    assert!(
        timeline.patterns.is_empty(),
        "distances beyond the admissible window are hostile input, not evidence"
    );
}

#[test]
fn confidence_classes_follow_regularity_and_count() {
    // Strong: >= 5 occurrences, gaps within +/-25% of mean.
    let strong_stamps = [1000_i64, 10_000, 19_000, 28_000, 37_000]; // gaps 9000 x4
    let mut builder = TimelineBuilder::new().watermark(WATERMARK);
    for (index, at) in strong_stamps.iter().enumerate() {
        builder
            .ingest(failure_event(&format!("s{index}"), "strong.rule", *at))
            .unwrap();
    }
    let timeline = builder.build();
    assert_eq!(timeline.patterns.len(), 1);
    assert_eq!(
        timeline.patterns[0].confidence,
        RecurrenceConfidence::Strong
    );

    // Moderate: >= 4 occurrences, gaps within +/-50% of mean.
    let moderate_stamps = [1000_i64, 11_000, 16_000, 26_000]; // gaps 10k,5k,10k (mean ~8.3k)
    let mut builder = TimelineBuilder::new().watermark(WATERMARK);
    for (index, at) in moderate_stamps.iter().enumerate() {
        builder
            .ingest(failure_event(&format!("m{index}"), "moderate.rule", *at))
            .unwrap();
    }
    let timeline = builder.build();
    assert_eq!(timeline.patterns.len(), 1);
    assert_eq!(
        timeline.patterns[0].confidence,
        RecurrenceConfidence::Moderate
    );

    // Irregular spacing stays Weak even with many occurrences.
    let irregular_stamps = [1000_i64, 2000, 9000, 9500, 40_000];
    let mut builder = TimelineBuilder::new().watermark(WATERMARK);
    for (index, at) in irregular_stamps.iter().enumerate() {
        builder
            .ingest(failure_event(&format!("w{index}"), "weak.rule", *at))
            .unwrap();
    }
    let timeline = builder.build();
    assert_eq!(timeline.patterns.len(), 1);
    assert_eq!(timeline.patterns[0].confidence, RecurrenceConfidence::Weak);
}

#[test]
fn pattern_cap_is_enforced_deterministically() {
    let mut builder = TimelineBuilder::new().watermark(WATERMARK);
    // Distinct identities each reaching the occurrence floor, far more than MAX_PATTERNS.
    for rule in 0..(MAX_PATTERNS + 20) {
        for occurrence in 0..MIN_OCCURRENCES_FOR_PATTERN {
            let at = 1000 + (rule as i64 * 10_000) + (occurrence as i64 * 1000);
            builder
                .ingest(failure_event(
                    &format!("p{rule}-{occurrence}"),
                    &format!("rule.{rule}"),
                    at,
                ))
                .unwrap();
        }
    }
    let timeline = builder.build();
    assert_eq!(timeline.patterns.len(), MAX_PATTERNS);
    // Truncation kept the lexicographically smallest identities (sorted before truncate).
    let identities: Vec<&str> = timeline
        .patterns
        .iter()
        .map(|pattern| pattern.semantic_identity_sha256.as_str())
        .collect();
    let mut expected = identities.clone();
    expected.sort();
    assert_eq!(
        identities, expected,
        "kept patterns must be in stable identity order"
    );
}

#[test]
fn semantic_identity_is_length_prefixed_against_collisions() {
    // ("ab","c") and ("a","bc") must never share an identity.
    let left = semantic_identity(EventClass::Operation, "ab", "c");
    let right = semantic_identity(EventClass::Operation, "a", "bc");
    assert_ne!(left, right);
    assert_eq!(left, semantic_identity(EventClass::Operation, "ab", "c"));
}

#[test]
fn digest_changes_when_history_changes_but_pattern_content_does_not_leak_noise() {
    let mut builder_a = TimelineBuilder::new().watermark(WATERMARK);
    builder_a
        .ingest(failure_event("d1", "digest.rule", 1000))
        .unwrap();
    let timeline_a = builder_a.build();

    let mut builder_b = TimelineBuilder::new().watermark(WATERMARK);
    builder_b
        .ingest(failure_event("d1", "digest.rule", 1000))
        .unwrap();
    builder_b
        .ingest(failure_event("d2", "other.rule", 2000))
        .unwrap();
    let timeline_b = builder_b.build();

    assert_ne!(timeline_a.digest_sha256, timeline_b.digest_sha256);
}

#[test]
fn page_size_contract_constant_is_render_safe() {
    // Server-side clamping uses this bound; it must be sane against abuse.
    assert!(MAX_PAGE_SIZE >= 1 && MAX_PAGE_SIZE <= 1000);
}
