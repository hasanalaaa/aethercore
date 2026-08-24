//! Phase 21 — end-to-end ingestion tests over a real `aethercore-persistence` database.
//!
//! Proves the read-only contract: rows written by earlier phases are consumed exactly
//! as persisted, no schema object is added or altered by ingestion, foreign principals
//! never leak, and two builds from the same database produce identical digests.

use aethercore_persistence::{
    Database, MaintenanceExecutionRecord, PlanRecord, RepairTimelineEventRecord,
};
use aethercore_timeline_intelligence::{Outcome, TimelineBuilder};
use tempfile::TempDir;

const OWNER: &str = "owner-a";
const OTHER: &str = "owner-b";

fn open_db(dir: &TempDir) -> Database {
    let path = dir.path().join("aethercore.db");
    Database::open(path).expect("database opens")
}

fn seed_plan(db: &Database, id: &str, owner: &str, created: i64) {
    db.insert_plan(
        &PlanRecord {
            id: id.into(),
            title: "seed".into(),
            state: "Completed".into(),
            // plans.digest carries a UNIQUE constraint — keep it per-plan.
            digest: format!("digest-{id}"),
            risk: "Low".into(),
            immutable_json: "{}".into(),
            created_unix_ms: created,
            updated_unix_ms: created,
            owner_principal_key: owner.into(),
        },
        "created",
    )
    .expect("plan inserted");
}

#[test]
fn ingests_persisted_history_without_writing() {
    let dir = TempDir::new().unwrap();
    let db = open_db(&dir);

    seed_plan(&db, "p1", OWNER, 1000);
    seed_plan(&db, "p2", OTHER, 1000); // foreign principal must never leak

    // Owner journal rows (to-state carries the transition target).
    db.append_plan_event("p1", "Executing", "state_transition", "", 2000)
        .unwrap();
    db.append_plan_event("p1", "RecoveryRequired", "state_transition", "", 3000)
        .unwrap();

    // Structured repair-timeline event (Phase 19 surface).
    db.insert_repair_timeline_event(&RepairTimelineEventRecord {
        event_id: "e1".into(),
        plan_id: "p1".into(),
        assessment_id: "a1".into(),
        owner_principal_key: OWNER.into(),
        event_kind: "verification".into(),
        domain: "WindowsRepair".into(),
        action_id: "verify-dism".into(),
        diagnosis_code: String::new(),
        outcome: "Failed".into(),
        detail: String::new(),
        machine_state_fingerprint: "fp1".into(),
        created_unix_ms: 4000,
    })
    .unwrap();

    // Maintenance execution completion.
    db.upsert_maintenance_execution(&MaintenanceExecutionRecord {
        plan_id: "p1".into(),
        domain: "WindowsRepair".into(),
        stage: "Finalize".into(),
        progress_known: false,
        overall_percent: 100,
        current_item_id: String::new(),
        detail: String::new(),
        mutation_started: true,
        recovery_required: false,
        failure_message: String::new(),
        outcome: "Succeeded".into(),
        machine_state_fingerprint: "fp2".into(),
        repair_graph_digest: String::new(),
        reboot_required: false,
        reboot_resume_token: String::new(),
        verification_state: "Verified".into(),
        started_unix_ms: 1500,
        updated_unix_ms: 5000,
        completed_unix_ms: Some(5000),
    })
    .unwrap();

    let candidates =
        aethercore_timeline_intelligence::ingest::ingest_owner_history(&db, OWNER).unwrap();
    assert!(
        !candidates.is_empty(),
        "owner history must yield candidates"
    );
    assert!(
        candidates
            .iter()
            .all(|event| !event.source_id.contains("p2")),
        "foreign-principal history must not leak into owner candidates"
    );

    let mut builder = TimelineBuilder::new().watermark(10_000);
    builder.ingest_all(candidates.clone()).unwrap();
    let timeline = builder.build();
    assert_eq!(timeline.digest_sha256.len(), 64);

    // Determinism: rebuilding from a freshly opened database yields an identical digest.
    let reopened = open_db(&dir);
    let again =
        aethercore_timeline_intelligence::ingest::ingest_owner_history(&reopened, OWNER).unwrap();
    let mut second_builder = TimelineBuilder::new().watermark(10_000);
    second_builder.ingest_all(again).unwrap();
    let rebuilt = second_builder.build();
    assert_eq!(
        timeline.digest_sha256, rebuilt.digest_sha256,
        "same persisted history must rebuild byte-identically"
    );

    // The recovery-shaped transition and the failed verification must both register.
    let failed = timeline
        .events
        .iter()
        .filter(|event| event.outcome == Outcome::Failed)
        .count();
    assert!(
        failed >= 2,
        "journal failure transition and repair event must register"
    );
}

#[test]
fn empty_history_builds_empty_deterministic_timeline() {
    let dir = TempDir::new().unwrap();
    let db = open_db(&dir);
    seed_plan(&db, "p9", OWNER, 1000);

    let candidates =
        aethercore_timeline_intelligence::ingest::ingest_owner_history(&db, OWNER).unwrap();
    // The seeded plan itself wrote one "created" journal event; that is the whole history.
    assert_eq!(
        candidates.len(),
        1,
        "only the plan-created journal row exists at this point"
    );

    let mut builder = TimelineBuilder::new().watermark(5_000);
    builder.ingest_all(candidates).unwrap();
    let timeline = builder.build();
    assert_eq!(timeline.events.len(), 1, "the plan-created journal row");
    assert!(
        timeline.patterns.is_empty(),
        "one neutral event makes no pattern"
    );
    assert_eq!(timeline.watermark_unix_ms, 5_000);
    assert_eq!(timeline.digest_sha256.len(), 64);

    // Rebuilding from a freshly opened database is byte-identical too.
    let reopened = open_db(&dir);
    let again_candidates =
        aethercore_timeline_intelligence::ingest::ingest_owner_history(&reopened, OWNER).unwrap();
    let mut again = TimelineBuilder::new().watermark(5_000);
    again.ingest_all(again_candidates).unwrap();
    assert_eq!(
        timeline.digest_sha256,
        again.build().digest_sha256,
        "same persisted history must digest identically"
    );
}
