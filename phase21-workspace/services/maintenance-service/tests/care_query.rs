//! P87-03: bounded Care sources must never turn an oversized plan into "nothing due".
#[allow(dead_code)]
#[path = "../src/care.rs"]
mod care;

use aethercore_persistence::{Database, PlanRecord};

#[test]
fn too_many_owned_candidates_are_not_an_empty_healthy_plan() {
    let dir = std::env::temp_dir().join(format!("p87-care-{}", uuid::Uuid::new_v4()));
    let db = Database::open(dir.join("care.db")).unwrap();
    for i in 0..17 {
        db.insert_plan(
            &PlanRecord {
                id: format!("cleanup-{i:02}"),
                title: "cleanup".into(),
                state: "AwaitingAuthorization".into(),
                digest: format!("{i:064x}"),
                risk: "Low".into(),
                immutable_json: r#"{"actions":[{"kind":"deleteCleanupCandidate"}]}"#.into(),
                created_unix_ms: 1,
                updated_unix_ms: 1,
                owner_principal_key: "owner-a".into(),
            },
            "test",
        )
        .unwrap();
    }
    assert!(
        care::compose_plan(&db, "owner-a").is_err(),
        "17 due plans silently became an empty healthy Care preview"
    );
    drop(db);
    std::fs::remove_dir_all(dir).unwrap();
}
