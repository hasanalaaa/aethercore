//! P87-03: SQL materializes only bounded, eligible sources for this owner.
use aethercore_persistence::{Database, PlanRecord};
use rusqlite::Connection;

#[test]
fn foreign_rows_cannot_be_materialized_and_limit_has_stable_ties() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("care.db");
    let db = Database::open(&path).unwrap();
    let conn = Connection::open(&path).unwrap();
    for (i, id) in ["a-2", "a-1", "a-3", "a-terminal"].iter().enumerate() {
        db.insert_plan(
            &PlanRecord {
                id: (*id).into(),
                title: "cleanup".into(),
                state: if i == 3 {
                    "Completed"
                } else {
                    "ReadyForReview"
                }
                .into(),
                digest: format!("{i:064x}"),
                risk: "Low".into(),
                immutable_json: "{}".into(),
                created_unix_ms: 1,
                updated_unix_ms: 1,
                owner_principal_key: "owner-a".into(),
            },
            "test",
        )
        .unwrap();
    }
    // B's malformed payload cannot be decoded as a String. An owner predicate
    // applied after query_map would fail A's read before it could filter B out.
    conn.execute_batch(
        "WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<2000)
         INSERT INTO plans(id,title,state,digest,risk,immutable_json,created_unix_ms,updated_unix_ms,owner_principal_key)
         SELECT 'b-'||x,'foreign','ReadyForReview','foreign-'||x,'Low',x'ff',1,1,'owner-b' FROM n;",
    )
    .unwrap();
    let rows = db.care_plan_candidates("owner-a", 2).unwrap();
    assert_eq!(
        rows.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(),
        ["a-1", "a-2"]
    );
    assert_eq!(
        db.care_plan_candidates("owner-a", usize::MAX)
            .unwrap()
            .len(),
        3
    );
    assert!(db.care_plan_candidates("owner-a", 0).unwrap().is_empty());
    assert!(db.care_plan_candidates("missing", 100).unwrap().is_empty());
    assert!(db.care_plan_candidates("owner-b", 2).is_err());
    // Recovery retains its intentional global scope, including malformed rows.
    assert!(db.plans_in_states(&["ReadyForReview"]).is_err());
    let sql = "EXPLAIN QUERY PLAN SELECT id FROM plans WHERE owner_principal_key=?
               AND state IN ('ReadyForReview','AwaitingAuthorization')
               ORDER BY updated_unix_ms ASC,id ASC LIMIT 17";
    let details = conn
        .prepare(sql)
        .unwrap()
        .query_map(["owner-a"], |r| r.get::<_, String>(3))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    assert!(
        details
            .iter()
            .any(|d| d.contains("idx_plans_owner_updated") && d.contains("SEARCH")),
        "{details:?}"
    );
}
