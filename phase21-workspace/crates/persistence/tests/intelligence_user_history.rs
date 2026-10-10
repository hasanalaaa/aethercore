//! Quality pass B6: a deep scan's history belongs to the Windows user, not to the logon
//! session that ran it. The session key changes at every sign-in and reboot, so history keyed
//! only by it vanished from the owner's view after each restart (owner PC, 2026-10-10).
use aethercore_persistence::{Database, IntelligenceScanRecord};

fn scan(id: &str, session_key: &str, user_key: &str, completed: i64) -> IntelligenceScanRecord {
    IntelligenceScanRecord {
        scan_id: id.into(),
        owner_principal_key: session_key.into(),
        owner_user_key: user_key.into(),
        state: "Completed".into(),
        status: "AttentionRecommended".into(),
        started_unix_ms: completed - 5,
        completed_unix_ms: completed,
        machine_state_fingerprint: "fp".into(),
        rule_engine_version: "rules".into(),
        app_version: "0.1.13".into(),
        finding_count: 1,
        unavailable_collector_count: 0,
        snapshot_json: format!(r#"{{"scanId":"{id}"}}"#),
    }
}

#[test]
fn history_follows_the_user_across_logon_sessions_and_survives_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("history.sqlite");
    let db = Database::open(&path).unwrap();
    db.save_intelligence_scan(&scan("before-reboot", "session-1", "user-a", 10))
        .unwrap();
    db.save_intelligence_scan(&scan("after-reboot", "session-2", "user-a", 20))
        .unwrap();
    db.save_intelligence_scan(&scan("someone-else", "session-3", "user-b", 30))
        .unwrap();
    db.save_intelligence_scan(&scan("legacy", "session-1", "", 40))
        .unwrap();
    drop(db);
    let db = Database::open(&path).unwrap();
    let ids = |user: &str| {
        db.intelligence_scans_for_user(user, 10)
            .unwrap()
            .into_iter()
            .map(|r| r.scan_id)
            .collect::<Vec<_>>()
    };
    assert_eq!(ids("user-a"), ["after-reboot", "before-reboot"]);
    assert_eq!(ids("user-b"), ["someone-else"]);
    assert!(
        ids("").is_empty(),
        "an unknown user key must not match legacy rows"
    );
    // The session-keyed view is unchanged.
    assert_eq!(
        db.intelligence_scans_for_owner("session-1", 10)
            .unwrap()
            .len(),
        2,
        "session history still holds its own rows, legacy included"
    );
}
