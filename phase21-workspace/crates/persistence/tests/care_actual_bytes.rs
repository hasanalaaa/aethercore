use aethercore_persistence::{CareRunRecord, CareStepRecord, Database};

#[test]
fn care_actual_deleted_bytes_survive_reopen_without_rounding_or_unknown_zero() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("care.sqlite");
    let db = Database::open(&path).unwrap();
    db.upsert_care_run(&CareRunRecord {
        run_id: "r".into(),
        owner_principal_key: "owner".into(),
        ..Default::default()
    })
    .unwrap();
    let values = [None, Some(0), Some(9_007_199_254_740_993), Some(u64::MAX)];
    for (step_index, actual_deleted_bytes) in values.into_iter().enumerate() {
        db.insert_care_step(&CareStepRecord {
            run_id: "r".into(),
            step_index: step_index as u32,
            actual_deleted_bytes,
            ..Default::default()
        })
        .unwrap();
    }
    drop(db);
    let db = Database::open(&path).unwrap();
    assert_eq!(
        db.care_steps_for_run("r")
            .unwrap()
            .into_iter()
            .map(|s| s.actual_deleted_bytes)
            .collect::<Vec<_>>(),
        values
    );
    let mut step = db.care_steps_for_run("r").unwrap()[0].clone();
    step.actual_deleted_bytes = Some(1536);
    db.update_care_step(&step).unwrap();
    drop(db);
    let db = Database::open(&path).unwrap();
    assert_eq!(
        db.care_steps_for_run("r").unwrap()[0].actual_deleted_bytes,
        Some(1536)
    );
    // Corrupt decimal data is a read failure, never a fabricated measured zero.
    rusqlite::Connection::open(&path)
        .unwrap()
        .execute(
            "UPDATE care_steps SET actual_deleted_bytes='-1' WHERE step_index=0",
            [],
        )
        .unwrap();
    assert!(db.care_steps_for_run("r").is_err());
}
