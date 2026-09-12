//! Phase 34 — fleet persistence proofs: forward migration, non-secret
//! storage, deterministic read-back, append-only run history.
//!
//! Phase 34 corrective (CF-2): typed AuthReference DB round-trips through the
//! authoritative strict `FleetHost` type — Agent/KeyFile/Certificate restore
//! EXACTLY, malformed/unknown/secret-shaped auth JSON is REJECTED, and path
//! references are never replaced by file contents.

use aethercore_fleet::{AuthReference, FleetHost, FleetInventory};
use aethercore_persistence::Database;
use std::collections::BTreeSet;

fn temp_db(tag: &str) -> (tempfile::TempDir, Database) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Database::open(dir.path().join(format!("{tag}.sqlite"))).expect("open db");
    (dir, db)
}

fn base_host(id: &str, auth: AuthReference) -> FleetHost {
    FleetHost::new(
        id,
        "Build host",
        &format!("{id}.example.internal"),
        22022,
        "ops",
        auth,
        BTreeSet::new(),
    )
    .expect("valid host")
}

/// write → read → parse strictly → auth variant restored exactly.
fn roundtrip_auth(db: &Database, host: &FleetHost) -> AuthReference {
    let json = serde_json::to_string(host).expect("serialize host");
    db.upsert_fleet_host(&json, 1_700_000_000_000)
        .expect("upsert");
    let rows = db.fleet_hosts().expect("read back");
    assert_eq!(rows.len(), 1);
    let restored = FleetInventory::from_bytes_raw(rows[0].as_bytes()).expect("strict parse");
    assert_eq!(restored.hosts[0].host_id, host.host_id);
    restored.hosts[0].auth.clone()
}

#[test]
fn migration_0015_forward_applies_cleanly() {
    let (_dir, db) = temp_db("fleet-migrate");
    // Migration 0015 applied on open; fleet tables readable and empty.
    let hosts = db.fleet_hosts().expect("fleet_hosts readable");
    assert!(hosts.is_empty());
}

#[test]
fn fleet_host_roundtrip_is_deterministic_and_secret_free() {
    let (_dir, db) = temp_db("fleet-roundtrip");
    let mut host = base_host(
        "host-build-01",
        AuthReference::KeyFile {
            path: "/home/ops/.ssh/id_ed25519".into(),
        },
    );
    host.pin_trust(&"ab".repeat(32), "ssh-ed25519", 1_700_000_000_000)
        .expect("pin");
    let json = serde_json::to_string(&host).expect("serialize host");
    db.upsert_fleet_host(&json, 1_700_000_000_001)
        .expect("upsert");
    db.upsert_fleet_host(&json, 1_700_000_000_002)
        .expect("upsert idempotent");

    let rows = db.fleet_hosts().expect("read back");
    assert_eq!(rows.len(), 1);
    let restored = FleetInventory::from_bytes_raw(rows[0].as_bytes()).expect("strict parse");
    assert_eq!(restored.hosts[0].host_id, "host-build-01");
    assert_eq!(
        restored.hosts[0].trust.as_ref().unwrap().host_key_sha256,
        "ab".repeat(32)
    );

    // The persisted row stores the identity path REFERENCE, never key
    // material; no password/secret body can appear because the strict
    // schema has no such field.
    assert!(rows[0].contains("/home/ops/.ssh/id_ed25519"));
    assert!(!rows[0].to_ascii_lowercase().contains("private key"));
    assert!(!rows[0].contains("BEGIN OPENSSH"));
    assert!(!rows[0].contains("password"));

    db.delete_fleet_host("host-build-01").expect("delete");
    assert!(db.fleet_hosts().unwrap().is_empty());
}

// ---------------------------------------------------------- CF-2 proofs

#[test]
fn cf2_agent_roundtrip_is_exactly_agent() {
    let (_dir, db) = temp_db("cf2-agent");
    let host = base_host("host-agent", AuthReference::Agent);
    assert_eq!(roundtrip_auth(&db, &host), AuthReference::Agent);
}

#[test]
fn cf2_key_file_roundtrip_is_exactly_key_file() {
    let (_dir, db) = temp_db("cf2-keyfile");
    let host = base_host(
        "host-keyfile",
        AuthReference::KeyFile {
            path: "/home/ops/.ssh/id_ed25519".into(),
        },
    );
    assert_eq!(
        roundtrip_auth(&db, &host),
        AuthReference::KeyFile {
            path: "/home/ops/.ssh/id_ed25519".into()
        }
    );
}

#[test]
fn cf2_certificate_roundtrip_is_exactly_certificate() {
    let (_dir, db) = temp_db("cf2-cert");
    let host = base_host(
        "host-cert",
        AuthReference::Certificate {
            path: "/etc/ssh/ops-user.crt".into(),
        },
    );
    assert_eq!(
        roundtrip_auth(&db, &host),
        AuthReference::Certificate {
            path: "/etc/ssh/ops-user.crt".into()
        }
    );
}

#[test]
fn cf2_path_reference_is_never_replaced_by_content() {
    // A path-shaped value must survive verbatim: never inlined, never read.
    let tricky = "/a path/with spaces/id file?query";
    let (_dir, db) = temp_db("cf2-path");
    let host = base_host(
        "host-path",
        AuthReference::KeyFile {
            path: tricky.into(),
        },
    );
    assert_eq!(
        roundtrip_auth(&db, &host),
        AuthReference::KeyFile {
            path: tricky.into()
        }
    );
}

#[test]
fn cf2_malformed_auth_json_is_rejected() {
    let (_dir, db) = temp_db("cf2-malformed");
    // auth: null — the old code silently defaulted this to agent.
    let raw = r#"{"schema":"aethercore.fleet.host.v1","host_id":"host-x","display_name":"X","hostname":"x.example.internal","port":22,"username":"ops","auth":null,"enabled":true}"#;
    assert!(db.upsert_fleet_host(raw, 0).is_err());
    // auth as an object with an unknown shape (newtype-typed unit variant
    // payload is rejected by serde).
    let raw2 = r#"{"schema":"aethercore.fleet.host.v1","host_id":"host-x","display_name":"X","hostname":"x.example.internal","port":22,"username":"ops","auth":{"agent":{}},"enabled":true}"#;
    assert!(db.upsert_fleet_host(raw2, 0).is_err());
    // Unknown auth variant.
    let raw3 = r#"{"schema":"aethercore.fleet.host.v1","host_id":"host-x","display_name":"X","hostname":"x.example.internal","port":22,"username":"ops","auth":{"password":"hunter2"},"enabled":true}"#;
    assert!(db.upsert_fleet_host(raw3, 0).is_err());
    // Nothing was persisted by any rejected write.
    assert!(db.fleet_hosts().unwrap().is_empty());
}

#[test]
fn cf2_unknown_field_is_rejected() {
    let (_dir, db) = temp_db("cf2-unknown");
    let mut host = base_host("host-unknown", AuthReference::Agent);
    let mut json: serde_json::Value = serde_json::to_value(&host).unwrap();
    json["sneaky_extra"] = serde_json::json!("boom");
    let raw = serde_json::to_string(&json).unwrap();
    assert!(db.upsert_fleet_host(&raw, 0).is_err());
    host.display_name = "Build host".into(); // silence unused-mut lint path
    let _ = host;
    assert!(db.fleet_hosts().unwrap().is_empty());
}

#[test]
fn cf2_secret_shaped_input_is_rejected() {
    let (_dir, db) = temp_db("cf2-secret");
    // A "secret" field is an unknown field of the strict schema (structurally
    // impossible to store) — the write must fail, not silently drop it.
    let mut json: serde_json::Value =
        serde_json::to_value(base_host("host-secret", AuthReference::Agent)).unwrap();
    json["password"] = serde_json::json!("hunter2");
    let raw = serde_json::to_string(&json).unwrap();
    assert!(db.upsert_fleet_host(&raw, 0).is_err());
    // Private-key-body-shaped identity path is invalid per domain validation.
    let raw2 = r#"{"schema":"aethercore.fleet.host.v1","host_id":"host-secret","display_name":"X","hostname":"s.example.internal","port":22,"username":"ops","auth":{"key_file":{"path":"-----BEGIN OPENSSH PRIVATE KEY-----"}},"enabled":true}"#;
    assert!(
        db.upsert_fleet_host(raw2, 0).is_err(),
        "private-key body is an invalid identity path"
    );
    // A trust block with a malformed fingerprint is rejected on re-validation.
    let raw3 = r#"{"schema":"aethercore.fleet.host.v1","host_id":"host-secret","display_name":"X","hostname":"s.example.internal","port":22,"username":"ops","auth":"agent","enabled":true,"trust":{"host_key_sha256":"not-a-fingerprint","key_type":"ssh-ed25519","trusted_unix_ms":1}}"#;
    assert!(db.upsert_fleet_host(raw3, 0).is_err());
    assert!(db.fleet_hosts().unwrap().is_empty());
}

#[test]
fn cf2_corrupt_db_auth_kind_is_read_rejected() {
    // A row whose auth_ref_kind was corrupted out-of-band must be a hard
    // read-time rejection, never a silent "agent" fallback.
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("cf2-corrupt.sqlite");
    {
        let db = Database::open(&db_path).expect("open");
        let host = base_host("host-a", AuthReference::Agent);
        db.upsert_fleet_host(&serde_json::to_string(&host).unwrap(), 0)
            .unwrap();
    }
    // Corrupt the row directly through SQLite.
    let conn = rusqlite_connection(&db_path);
    conn.execute(
        "UPDATE fleet_hosts SET auth_ref_kind='?evil' WHERE host_id='host-a'",
        [],
    )
    .unwrap();
    let db = Database::open(&db_path).expect("reopen");
    assert!(db.fleet_hosts().is_err(), "corrupt auth kind must reject");
}

fn rusqlite_connection(path: &std::path::Path) -> rusqlite::Connection {
    rusqlite::Connection::open(path).expect("raw connection")
}

#[test]
fn fleet_run_history_is_append_only_across_failures() {
    let (_dir, db) = temp_db("fleet-history");
    let run1 = db
        .append_fleet_run(None, "manual", 1000, 3, 1, 2, "partial failure")
        .expect("append 1");
    let run2 = db
        .append_fleet_run(Some("sched-daily"), "schedule", 2000, 3, 3, 0, "all ok")
        .expect("append 2");
    let run3 = db
        .append_fleet_run(None, "manual", 3000, 1, 0, 1, "total failure")
        .expect("append 3");
    assert!(run1 < run2 && run2 < run3);
    // There is no API to delete or rewrite a history row: failures append,
    // they never erase prior records.
}
