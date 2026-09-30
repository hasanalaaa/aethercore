//! P87-04A: page and pattern replies explicitly describe their bounded history.
#[allow(dead_code)]
#[path = "../src/timeline.rs"]
mod timeline;

#[test]
fn page_and_patterns_declare_the_history_window_without_reinterpreting_the_cursor() {
    use aethercore_persistence::{CareRunRecord, Database};
    use aethercore_timeline_intelligence::MAX_TIMELINE_EVENTS;
    let dir = std::env::temp_dir().join(format!("p87-timeline-{}", uuid::Uuid::new_v4()));
    let db = std::sync::Arc::new(Database::open(dir.join("timeline.db")).unwrap());
    for i in 1..=3 {
        db.upsert_care_run(&CareRunRecord {
            run_id: format!("run-{i}"),
            owner_principal_key: "owner-a".into(),
            state: "Completed".into(),
            created_unix_ms: i,
            updated_unix_ms: i,
            ..Default::default()
        })
        .unwrap();
    }
    let coordinator = timeline::TimelineCoordinator::new(db.clone());
    let (page, _) = coordinator.page_for_owner("owner-a", 1, 0).unwrap();
    assert_eq!(page.entries.len(), 1);
    assert_eq!(page.entries[0].source_id, "care-run:run-3");
    assert!(page.has_more);
    assert_eq!(page.next_before_sequence, 2);
    assert_eq!(page.history_window_limit, MAX_TIMELINE_EVENTS as u32);
    let (patterns, _) = coordinator.patterns_for_owner("owner-a").unwrap();
    assert_eq!(patterns.history_window_limit, page.history_window_limit);
    let (foreign, _) = coordinator.page_for_owner("owner-b", 1, 0).unwrap();
    assert!(foreign.entries.is_empty());
    drop(coordinator);
    drop(db);
    std::fs::remove_dir_all(dir).unwrap();
}
