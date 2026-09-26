#![forbid(unsafe_code)]

use std::{path::Path, sync::Mutex};

use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use sha2::{Digest, Sha256};
use thiserror::Error;

const MIGRATION_0001: &str = include_str!("../migrations/0001_init.sql");
const MIGRATION_0002: &str = include_str!("../migrations/0002_driver_install.sql");
const MIGRATION_0003: &str = include_str!("../migrations/0003_phase4.sql");
const MIGRATION_0004: &str = include_str!("../migrations/0004_startup_manager.sql");
const MIGRATION_0005: &str = include_str!("../migrations/0005_diagnostics.sql");
const MIGRATION_0006: &str = include_str!("../migrations/0006_phase9_security.sql");
const MIGRATION_0007: &str = include_str!("../migrations/0007_phase10_kernel.sql");
const MIGRATION_0008: &str = include_str!("../migrations/0008_phase14_scheduler.sql");
const MIGRATION_0009: &str = include_str!("../migrations/0009_phase15_update.sql");
const MIGRATION_0010: &str = include_str!("../migrations/0010_phase17_intelligence.sql");
const MIGRATION_0011: &str =
    include_str!("../migrations/0011_phase17_1_intelligence_integrity.sql");
const MIGRATION_0012: &str = include_str!("../migrations/0012_phase18_driver_authority.sql");
const MIGRATION_0013: &str = include_str!("../migrations/0013_phase19_windows_repair.sql");
const MIGRATION_0014: &str = include_str!("../migrations/0014_phase22_care_orchestration.sql");
/// Phase 34 — fleet & secure remote operations (additive tables only).
const MIGRATION_0015: &str = include_str!("../migrations/0015_phase34_fleet.sql");
const MIGRATION_0016: &str =
    include_str!("../migrations/0016_p46_byte_progress_determinedness.sql");

const MIGRATIONS: &[(i64, &str, &str)] = &[
    (1, "0001_init", MIGRATION_0001),
    (2, "0002_driver_install", MIGRATION_0002),
    (3, "0003_phase4", MIGRATION_0003),
    (4, "0004_startup_manager", MIGRATION_0004),
    (5, "0005_diagnostics", MIGRATION_0005),
    (6, "0006_phase9_security", MIGRATION_0006),
    (7, "0007_phase10_kernel", MIGRATION_0007),
    (8, "0008_phase14_scheduler", MIGRATION_0008),
    (9, "0009_phase15_update", MIGRATION_0009),
    (10, "0010_phase17_intelligence", MIGRATION_0010),
    (11, "0011_phase17_1_intelligence_integrity", MIGRATION_0011),
    (12, "0012_phase18_driver_authority", MIGRATION_0012),
    (13, "0013_phase19_windows_repair", MIGRATION_0013),
    (14, "0014_phase22_care_orchestration", MIGRATION_0014),
    (15, "0015_phase34_fleet", MIGRATION_0015),
    (16, "0016_p46_byte_progress_determinedness", MIGRATION_0016),
];

#[derive(Debug, Error)]
pub enum PersistenceError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("filesystem error: {0}")]
    Io(#[from] std::io::Error),
    #[error("database lock poisoned")]
    Poisoned,
    #[error("migration checksum drift for version {0}")]
    MigrationDrift(i64),
}

pub type Result<T> = std::result::Result<T, PersistenceError>;

/// Phase 22 — one orchestrated One-Click Care run (crash-safe resume anchor).
#[derive(Clone, Debug, Default)]
pub struct CareRunRecord {
    pub run_id: String,
    pub owner_principal_key: String,
    pub state: String,
    pub stage: String,
    pub plan_digest_sha256: String,
    /// True only after the owner granted the explicit per-session consent.
    pub session_consent_granted: bool,
    pub steps_total: u32,
    pub steps_done: u32,
    pub detail: String,
    pub created_unix_ms: i64,
    pub updated_unix_ms: i64,
    pub completed_unix_ms: Option<i64>,
}

/// Phase 22 — one step inside a care run. Each step wraps an EXISTING domain plan
/// and cites that domain's own verification outcome; the orchestrator never invents one.
#[derive(Clone, Debug, Default)]
pub struct CareStepRecord {
    pub run_id: String,
    pub step_index: u32,
    pub domain_plan_id: String,
    pub domain_kind: String,
    pub safety_level: i64,
    pub state: String,
    pub outcome: String,
    pub verification_state: String,
    pub failure_message: String,
    pub started_unix_ms: i64,
    pub updated_unix_ms: i64,
}

#[derive(Clone, Debug)]
pub struct PlanRecord {
    pub id: String,
    pub title: String,
    pub state: String,
    pub digest: String,
    pub risk: String,
    pub immutable_json: String,
    pub created_unix_ms: i64,
    pub updated_unix_ms: i64,
    pub owner_principal_key: String,
}

#[derive(Clone, Debug, Default)]
pub struct ConsentIntentRecord {
    pub intent_id: String,
    pub plan_id: String,
    pub digest: String,
    pub owner_principal_key: String,
    pub created_unix_ms: i64,
    pub expires_unix_ms: i64,
    pub approved_unix_ms: Option<i64>,
    pub consumed_unix_ms: Option<i64>,
    pub broker_pid: Option<u32>,
}

#[derive(Clone, Debug, Default)]
pub struct ExecutionRecord {
    pub plan_id: String,
    pub stage: String,
    pub progress_known: bool,
    pub overall_percent: u32,
    pub current_candidate_id: String,
    /// DBT-P46-B16: None means no WUA progress tick ever determined a figure.
    /// Some(0) means a tick determined that zero bytes had transferred.
    pub bytes_downloaded: Option<u64>,
    /// DBT-P46-B16: None means the total size is unknown, which is a different
    /// fact from a zero-length download.
    pub bytes_total: Option<u64>,
    pub detail: String,
    pub reboot_required: bool,
    pub reboot_boot_marker_ms: i64,
    pub restore_point_sequence: Option<i64>,
    pub backup_root: String,
    pub mutation_started: bool,
    pub recovery_required: bool,
    pub failure_message: String,
    pub started_unix_ms: i64,
    pub updated_unix_ms: i64,
    pub completed_unix_ms: Option<i64>,
}

#[derive(Clone, Debug, Default)]
pub struct InstallItemRecord {
    pub plan_id: String,
    pub candidate_id: String,
    pub update_id: String,
    pub revision: i32,
    pub instance_id: String,
    pub title: String,
    pub stage: String,
    pub progress_known: bool,
    pub progress_percent: u32,
    pub result_code: String,
    pub hresult: i32,
    pub reboot_required: bool,
    pub verified: bool,
    pub before_driver_json: String,
    pub after_driver_json: String,
    pub before_problem_code: u32,
    pub after_problem_code: u32,
    pub backup_path: String,
    pub detail: String,
    pub updated_unix_ms: i64,
}

#[derive(Clone, Debug, Default)]
pub struct DriverAuthorityOverrideRecord {
    pub owner_principal_key: String,
    pub override_id: String,
    pub device_privacy_key: String,
    pub behavior: String,
    pub candidate_version: String,
    pub provider_id: String,
    pub expires_unix_ms: Option<i64>,
    pub created_unix_ms: i64,
    pub updated_unix_ms: i64,
}

#[derive(Clone, Debug, Default)]
pub struct DriverAuthorityScanRecord {
    pub owner_principal_key: String,
    pub scan_id: String,
    pub inventory_epoch: u64,
    pub authority_coverage: String,
    pub snapshot_json: String,
    pub created_unix_ms: i64,
}

#[derive(Clone, Debug, Default)]
pub struct CheckpointRecord {
    pub seq: i64,
    pub plan_id: String,
    pub candidate_id: String,
    pub checkpoint: String,
    pub detail_json: String,
    pub created_unix_ms: i64,
}

#[derive(Clone, Debug, Default)]
pub struct RecoveryRecord {
    pub seq: i64,
    pub plan_id: String,
    pub severity: String,
    pub kind: String,
    pub summary: String,
    pub detail: String,
    pub restore_point_sequence: Option<i64>,
    pub backup_root: String,
    pub created_unix_ms: i64,
}

/// The execution record a plan transition commits together with the new plan state, so no reader
/// can see a terminal plan without the record that explains it (DBT-P63-012). The recovery record,
/// when present, lands in the same transaction.
pub enum PlanJournal<'a> {
    Maintenance(&'a MaintenanceExecutionRecord, Option<&'a RecoveryRecord>),
    Driver(&'a ExecutionRecord, Option<&'a RecoveryRecord>),
}

#[derive(Clone, Debug, Default)]
pub struct MaintenanceExecutionRecord {
    pub plan_id: String,
    pub domain: String,
    pub stage: String,
    pub progress_known: bool,
    pub overall_percent: u32,
    pub current_item_id: String,
    pub detail: String,
    pub mutation_started: bool,
    pub recovery_required: bool,
    pub failure_message: String,
    pub outcome: String,
    pub machine_state_fingerprint: String,
    pub repair_graph_digest: String,
    pub reboot_required: bool,
    pub reboot_resume_token: String,
    pub verification_state: String,
    pub started_unix_ms: i64,
    pub updated_unix_ms: i64,
    pub completed_unix_ms: Option<i64>,
}

#[derive(Clone, Debug, Default)]
pub struct RepairTimelineEventRecord {
    pub event_id: String,
    pub plan_id: String,
    pub assessment_id: String,
    pub owner_principal_key: String,
    pub event_kind: String,
    pub domain: String,
    pub action_id: String,
    pub diagnosis_code: String,
    pub outcome: String,
    pub detail: String,
    pub machine_state_fingerprint: String,
    pub created_unix_ms: i64,
}

#[derive(Clone, Debug, Default)]
pub struct RepairRebootResumeRecord {
    pub token_sha256: String,
    pub owner_principal_key: String,
    pub assessment_id: String,
    pub machine_state_fingerprint: String,
    pub repair_graph_digest: String,
    pub resume_policy: String,
    pub state: String,
    pub created_unix_ms: i64,
    pub consumed_unix_ms: Option<i64>,
}

#[derive(Clone, Debug, Default)]
pub struct MaintenanceItemRecord {
    pub plan_id: String,
    pub item_id: String,
    pub kind: String,
    pub stage: String,
    pub result_code: String,
    pub bytes_affected: u64,
    pub detail: String,
    pub updated_unix_ms: i64,
}

#[derive(Clone, Debug, Default)]
pub struct StartupChangeRecord {
    pub change_id: String,
    pub origin_change_id: String,
    pub plan_id: String,
    pub item_id: String,
    pub kind: String,
    pub display_name: String,
    pub direction: String,
    pub original_json: String,
    pub applied_json: String,
    pub state: String,
    pub detail: String,
    pub created_unix_ms: i64,
    pub updated_unix_ms: i64,
    pub restored_unix_ms: Option<i64>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SchedulerRunRecord {
    pub owner_principal_key: String,
    pub workload: String,
    pub failure_count: u32,
    pub next_eligible_unix_ms: i64,
    pub last_outcome: String,
    pub last_completed_unix_ms: Option<i64>,
    pub updated_unix_ms: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UpdateManifestFloorRecord {
    pub channel: String,
    pub highest_sequence: u64,
    pub generated_unix_ms: i64,
    pub manifest_sha256: String,
    pub updated_unix_ms: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UpdateExecutionGuardRecord {
    pub ticket_id: String,
    pub owner_principal_key: String,
    pub release_id: String,
    pub release_version: String,
    pub channel: String,
    pub notes_message_key: String,
    pub minimum_windows_build: u32,
    pub staged_path: String,
    pub expected_sha256: String,
    pub expected_size: u64,
    pub expires_unix_ms: i64,
    pub created_unix_ms: i64,
}

#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct SupportJournalEventRecord {
    pub seq: i64,
    pub plan_id: String,
    pub from_state: Option<String>,
    pub to_state: String,
    pub event_kind: String,
    pub detail: String,
    pub created_unix_ms: i64,
}

#[derive(Clone, Debug, Default)]
pub struct DiagnosticSnapshotRecord {
    pub snapshot_id: String,
    pub owner_principal_key: String,
    pub state: String,
    pub collected_unix_ms: i64,
    pub warning_count: u32,
    pub snapshot_json: String,
}

#[derive(Clone, Debug, Default)]
pub struct IntelligenceScanRecord {
    pub scan_id: String,
    pub owner_principal_key: String,
    pub state: String,
    pub status: String,
    pub started_unix_ms: i64,
    pub completed_unix_ms: i64,
    pub machine_state_fingerprint: String,
    pub rule_engine_version: String,
    pub app_version: String,
    pub finding_count: u32,
    pub unavailable_collector_count: u32,
    pub snapshot_json: String,
}

#[derive(Clone, Debug, Default)]
pub struct IntelligenceFindingRecord {
    pub owner_principal_key: String,
    pub finding_id: String,
    pub finding_code: String,
    pub first_observed_unix_ms: i64,
    pub last_observed_unix_ms: i64,
    pub lifecycle: String,
    pub severity: String,
    pub confidence: String,
    pub verification_status: String,
    pub resolved_at_unix_ms: Option<i64>,
    pub resolution_scan_id: String,
    pub resolution_reason_key: String,
    pub finding_json: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IntelligenceOverrideRecord {
    pub owner_principal_key: String,
    pub override_id: String,
    pub scope_kind: String,
    pub scope_value_hash: String,
    pub behavior: String,
    pub expires_unix_ms: Option<i64>,
    pub created_unix_ms: i64,
    pub updated_unix_ms: i64,
}

pub struct Database {
    connection: Mutex<Connection>,
}

impl Database {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "FULL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        apply_migrations(&mut conn)?;
        Ok(Self {
            connection: Mutex::new(conn),
        })
    }

    pub fn insert_plan(&self, p: &PlanRecord, event_kind: &str) -> Result<()> {
        let mut conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "INSERT INTO plans(id,title,state,digest,risk,immutable_json,created_unix_ms,updated_unix_ms,owner_principal_key) VALUES(?,?,?,?,?,?,?,?,?)",
            params![p.id,p.title,p.state,p.digest,p.risk,p.immutable_json,p.created_unix_ms,p.updated_unix_ms,p.owner_principal_key],
        )?;
        tx.execute(
            "INSERT INTO plan_events(plan_id,from_state,to_state,event_kind,detail,created_unix_ms) VALUES(?,NULL,?,?,?,?)",
            params![p.id,p.state,event_kind,"plan created",p.created_unix_ms],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn latest_plan(&self) -> Result<Option<PlanRecord>> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        conn.query_row(
            "SELECT id,title,state,digest,risk,immutable_json,created_unix_ms,updated_unix_ms,owner_principal_key FROM plans ORDER BY created_unix_ms DESC LIMIT 1",
            [], row_to_plan,
        ).optional().map_err(Into::into)
    }

    pub fn latest_plan_for_owner(&self, owner_principal_key: &str) -> Result<Option<PlanRecord>> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        conn.query_row(
            "SELECT id,title,state,digest,risk,immutable_json,created_unix_ms,updated_unix_ms,owner_principal_key FROM plans WHERE owner_principal_key=? ORDER BY created_unix_ms DESC LIMIT 1",
            [owner_principal_key], row_to_plan,
        ).optional().map_err(Into::into)
    }

    pub fn get_plan(&self, id: &str) -> Result<Option<PlanRecord>> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        conn.query_row(
            "SELECT id,title,state,digest,risk,immutable_json,created_unix_ms,updated_unix_ms,owner_principal_key FROM plans WHERE id=?",
            [id], row_to_plan,
        ).optional().map_err(Into::into)
    }

    pub fn get_plan_for_owner(
        &self,
        id: &str,
        owner_principal_key: &str,
    ) -> Result<Option<PlanRecord>> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        conn.query_row(
            "SELECT id,title,state,digest,risk,immutable_json,created_unix_ms,updated_unix_ms,owner_principal_key FROM plans WHERE id=? AND owner_principal_key=?",
            params![id, owner_principal_key], row_to_plan,
        ).optional().map_err(Into::into)
    }

    pub fn plans_in_states(&self, states: &[&str]) -> Result<Vec<PlanRecord>> {
        if states.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders = std::iter::repeat_n("?", states.len())
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT id,title,state,digest,risk,immutable_json,created_unix_ms,updated_unix_ms,owner_principal_key FROM plans WHERE state IN ({placeholders}) ORDER BY updated_unix_ms"
        );
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        let mut stmt = conn.prepare(&sql)?;
        let params = rusqlite::params_from_iter(states.iter().copied());
        let rows = stmt.query_map(params, row_to_plan)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn transition_plan(
        &self,
        id: &str,
        expected: &str,
        next: &str,
        detail: &str,
        now_ms: i64,
    ) -> Result<bool> {
        let mut conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let changed = transition_plan_in(&tx, id, expected, next, detail, now_ms)?;
        tx.commit()?;
        Ok(changed)
    }

    /// `transition_plan` and the plan's execution journal in ONE transaction. The journal is
    /// written first, so it is already in place at the instant the state flips; if the state CAS
    /// misses, or any write fails, the whole transaction rolls back and nothing is recorded.
    pub fn transition_plan_with_journal(
        &self,
        id: &str,
        expected: &str,
        next: &str,
        detail: &str,
        now_ms: i64,
        journal: PlanJournal<'_>,
    ) -> Result<bool> {
        let mut conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let recovery = match journal {
            PlanJournal::Maintenance(r, recovery) => {
                upsert_maintenance_execution_in(&tx, r)?;
                recovery
            }
            PlanJournal::Driver(r, recovery) => {
                upsert_execution_in(&tx, r)?;
                recovery
            }
        };
        if let Some(r) = recovery {
            add_recovery_record_in(&tx, r)?;
        }
        if !transition_plan_in(&tx, id, expected, next, detail, now_ms)? {
            return Ok(false);
        }
        tx.commit()?;
        Ok(true)
    }

    pub fn append_plan_event(
        &self,
        plan_id: &str,
        state: &str,
        event_kind: &str,
        detail: &str,
        now_ms: i64,
    ) -> Result<()> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        conn.execute(
            "INSERT INTO plan_events(plan_id,from_state,to_state,event_kind,detail,created_unix_ms) VALUES(?,?,?,?,?,?)",
            params![plan_id,state,state,event_kind,detail,now_ms],
        )?;
        Ok(())
    }

    pub fn event_count(&self) -> Result<u64> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        let n: i64 = conn.query_row("SELECT COUNT(*) FROM plan_events", [], |r| r.get(0))?;
        Ok(n as u64)
    }

    pub fn event_count_for_owner(&self, owner_principal_key: &str) -> Result<u64> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        let n: i64 = conn.query_row(
            "SELECT COUNT(*) FROM plan_events e INNER JOIN plans p ON p.id=e.plan_id WHERE p.owner_principal_key=?",
            [owner_principal_key],
            |r| r.get(0),
        )?;
        Ok(n as u64)
    }

    pub fn begin_consent_intent(
        &self,
        intent_id: &str,
        plan_id: &str,
        digest: &str,
        owner_principal_key: &str,
        created_ms: i64,
        expires_ms: i64,
    ) -> Result<()> {
        let mut conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "DELETE FROM consent_intents WHERE plan_id=? AND owner_principal_key=? AND consumed_unix_ms IS NULL",
            params![plan_id, owner_principal_key],
        )?;
        tx.execute(
            "INSERT INTO consent_intents(intent_id,plan_id,digest,owner_principal_key,created_unix_ms,expires_unix_ms,approved_unix_ms,consumed_unix_ms,broker_pid) VALUES(?,?,?,?,?,?,NULL,NULL,NULL)",
            params![intent_id,plan_id,digest,owner_principal_key,created_ms,expires_ms],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn get_consent_intent(&self, intent_id: &str) -> Result<Option<ConsentIntentRecord>> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        conn.query_row(
            "SELECT intent_id,plan_id,digest,owner_principal_key,created_unix_ms,expires_unix_ms,approved_unix_ms,consumed_unix_ms,broker_pid FROM consent_intents WHERE intent_id=?",
            [intent_id],
            row_to_consent_intent,
        ).optional().map_err(Into::into)
    }

    pub fn approve_consent_intent(
        &self,
        intent_id: &str,
        owner_principal_key: &str,
        broker_pid: u32,
        now_ms: i64,
    ) -> Result<bool> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        let changed = conn.execute(
            "UPDATE consent_intents SET approved_unix_ms=?,broker_pid=? WHERE intent_id=? AND owner_principal_key=? AND approved_unix_ms IS NULL AND consumed_unix_ms IS NULL AND expires_unix_ms>=?",
            params![now_ms,broker_pid,intent_id,owner_principal_key,now_ms],
        )?;
        Ok(changed == 1)
    }

    pub fn approved_consent_until(
        &self,
        plan_id: &str,
        digest: &str,
        owner_principal_key: &str,
        now_ms: i64,
    ) -> Result<i64> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        let value: Option<i64> = conn.query_row(
            "SELECT MAX(expires_unix_ms) FROM consent_intents WHERE plan_id=? AND digest=? AND owner_principal_key=? AND approved_unix_ms IS NOT NULL AND consumed_unix_ms IS NULL AND expires_unix_ms>=?",
            params![plan_id,digest,owner_principal_key,now_ms],
            |r| r.get(0),
        )?;
        Ok(value.unwrap_or(0))
    }

    /// Atomically consumes exactly one approved consent intent and advances the owned plan into
    /// Preflight. If either condition fails, the transaction rolls back and the intent remains
    /// unconsumed. This is the Phase 9 one-shot authorization barrier.
    pub fn consume_consent_and_transition(
        &self,
        plan_id: &str,
        owner_principal_key: &str,
        expected: &str,
        next: &str,
        detail: &str,
        now_ms: i64,
    ) -> Result<bool> {
        let mut conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let intent_id: Option<String> = tx.query_row(
            "SELECT c.intent_id FROM consent_intents c INNER JOIN plans p ON p.id=c.plan_id WHERE c.plan_id=? AND c.owner_principal_key=? AND c.digest=p.digest AND p.owner_principal_key=c.owner_principal_key AND c.approved_unix_ms IS NOT NULL AND c.consumed_unix_ms IS NULL AND c.expires_unix_ms>=? ORDER BY c.approved_unix_ms DESC LIMIT 1",
            params![plan_id,owner_principal_key,now_ms],
            |r| r.get(0),
        ).optional()?;
        let Some(intent_id) = intent_id else {
            return Ok(false);
        };
        let consumed = tx.execute(
            "UPDATE consent_intents SET consumed_unix_ms=? WHERE intent_id=? AND consumed_unix_ms IS NULL",
            params![now_ms,intent_id],
        )?;
        if consumed != 1 {
            return Ok(false);
        }
        let changed = tx.execute(
            "UPDATE plans SET state=?,updated_unix_ms=? WHERE id=? AND owner_principal_key=? AND state=?",
            params![next,now_ms,plan_id,owner_principal_key,expected],
        )?;
        if changed != 1 {
            return Ok(false);
        }
        tx.execute(
            "INSERT INTO plan_events(plan_id,from_state,to_state,event_kind,detail,created_unix_ms) VALUES(?,?,?,?,?,?)",
            params![plan_id,expected,next,"authorization_consumed_transition",detail,now_ms],
        )?;
        tx.commit()?;
        Ok(true)
    }

    pub fn upsert_execution(&self, r: &ExecutionRecord) -> Result<()> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        upsert_execution_in(&conn, r)
    }

    pub fn get_execution(&self, plan_id: &str) -> Result<Option<ExecutionRecord>> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        conn.query_row(
            "SELECT plan_id,stage,progress_known,overall_percent,current_candidate_id,bytes_downloaded,bytes_total,detail,reboot_required,reboot_boot_marker_ms,restore_point_sequence,backup_root,mutation_started,recovery_required,failure_message,started_unix_ms,updated_unix_ms,completed_unix_ms,bytes_downloaded_known,bytes_total_known FROM plan_executions WHERE plan_id=?",
            [plan_id], row_to_execution,
        ).optional().map_err(Into::into)
    }

    pub fn latest_execution(&self) -> Result<Option<ExecutionRecord>> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        conn.query_row(
            "SELECT plan_id,stage,progress_known,overall_percent,current_candidate_id,bytes_downloaded,bytes_total,detail,reboot_required,reboot_boot_marker_ms,restore_point_sequence,backup_root,mutation_started,recovery_required,failure_message,started_unix_ms,updated_unix_ms,completed_unix_ms,bytes_downloaded_known,bytes_total_known FROM plan_executions ORDER BY updated_unix_ms DESC LIMIT 1",
            [], row_to_execution,
        ).optional().map_err(Into::into)
    }

    pub fn latest_execution_for_owner(
        &self,
        owner_principal_key: &str,
    ) -> Result<Option<ExecutionRecord>> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        conn.query_row(
            "SELECT e.plan_id,e.stage,e.progress_known,e.overall_percent,e.current_candidate_id,e.bytes_downloaded,e.bytes_total,e.detail,e.reboot_required,e.reboot_boot_marker_ms,e.restore_point_sequence,e.backup_root,e.mutation_started,e.recovery_required,e.failure_message,e.started_unix_ms,e.updated_unix_ms,e.completed_unix_ms,e.bytes_downloaded_known,e.bytes_total_known FROM plan_executions e INNER JOIN plans p ON p.id=e.plan_id WHERE p.owner_principal_key=? ORDER BY e.updated_unix_ms DESC LIMIT 1",
            [owner_principal_key], row_to_execution,
        ).optional().map_err(Into::into)
    }

    pub fn upsert_install_item(&self, r: &InstallItemRecord) -> Result<()> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        conn.execute(
            "INSERT INTO driver_install_items(plan_id,candidate_id,update_id,revision,instance_id,title,stage,progress_known,progress_percent,result_code,hresult,reboot_required,verified,before_driver_json,after_driver_json,before_problem_code,after_problem_code,backup_path,detail,updated_unix_ms) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)
             ON CONFLICT(plan_id,candidate_id) DO UPDATE SET stage=excluded.stage,progress_known=excluded.progress_known,progress_percent=excluded.progress_percent,result_code=excluded.result_code,hresult=excluded.hresult,reboot_required=excluded.reboot_required,verified=excluded.verified,before_driver_json=excluded.before_driver_json,after_driver_json=excluded.after_driver_json,before_problem_code=excluded.before_problem_code,after_problem_code=excluded.after_problem_code,backup_path=excluded.backup_path,detail=excluded.detail,updated_unix_ms=excluded.updated_unix_ms",
            params![r.plan_id,r.candidate_id,r.update_id,r.revision,r.instance_id,r.title,r.stage,bool_i(r.progress_known),r.progress_percent,r.result_code,r.hresult,bool_i(r.reboot_required),bool_i(r.verified),r.before_driver_json,r.after_driver_json,r.before_problem_code,r.after_problem_code,r.backup_path,r.detail,r.updated_unix_ms],
        )?;
        Ok(())
    }

    pub fn install_items(&self, plan_id: &str) -> Result<Vec<InstallItemRecord>> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        let mut stmt = conn.prepare(
            "SELECT plan_id,candidate_id,update_id,revision,instance_id,title,stage,progress_known,progress_percent,result_code,hresult,reboot_required,verified,before_driver_json,after_driver_json,before_problem_code,after_problem_code,backup_path,detail,updated_unix_ms FROM driver_install_items WHERE plan_id=? ORDER BY rowid"
        )?;
        let rows = stmt.query_map([plan_id], row_to_install_item)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn recent_verified_driver_install_items_for_owner(
        &self,
        owner_principal_key: &str,
        since_unix_ms: i64,
        limit: usize,
    ) -> Result<Vec<InstallItemRecord>> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        let mut stmt = conn.prepare(
            "SELECT i.plan_id,i.candidate_id,i.update_id,i.revision,i.instance_id,i.title,i.stage,i.progress_known,i.progress_percent,i.result_code,i.hresult,i.reboot_required,i.verified,i.before_driver_json,i.after_driver_json,i.before_problem_code,i.after_problem_code,i.backup_path,i.detail,i.updated_unix_ms \
             FROM driver_install_items i INNER JOIN plans p ON p.id=i.plan_id \
             WHERE p.owner_principal_key=? AND i.verified=1 AND i.updated_unix_ms>=? \
             ORDER BY i.updated_unix_ms DESC LIMIT ?"
        )?;
        let rows = stmt.query_map(
            rusqlite::params![owner_principal_key, since_unix_ms, limit.min(500) as i64],
            row_to_install_item,
        )?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn upsert_driver_authority_override(
        &self,
        r: &DriverAuthorityOverrideRecord,
    ) -> Result<()> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        conn.execute(
            "INSERT INTO driver_authority_overrides(owner_principal_key,override_id,device_privacy_key,behavior,candidate_version,provider_id,expires_unix_ms,created_unix_ms,updated_unix_ms) VALUES(?,?,?,?,?,?,?,?,?) \
             ON CONFLICT(owner_principal_key,override_id) DO UPDATE SET device_privacy_key=excluded.device_privacy_key,behavior=excluded.behavior,candidate_version=excluded.candidate_version,provider_id=excluded.provider_id,expires_unix_ms=excluded.expires_unix_ms,updated_unix_ms=excluded.updated_unix_ms",
            params![r.owner_principal_key,r.override_id,r.device_privacy_key,r.behavior,r.candidate_version,r.provider_id,r.expires_unix_ms,r.created_unix_ms,r.updated_unix_ms],
        )?;
        Ok(())
    }

    pub fn driver_authority_overrides_for_owner(
        &self,
        owner_principal_key: &str,
        now_ms: i64,
    ) -> Result<Vec<DriverAuthorityOverrideRecord>> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        let mut stmt = conn.prepare(
            "SELECT owner_principal_key,override_id,device_privacy_key,behavior,candidate_version,provider_id,expires_unix_ms,created_unix_ms,updated_unix_ms FROM driver_authority_overrides WHERE owner_principal_key=? AND (expires_unix_ms IS NULL OR expires_unix_ms>?) ORDER BY updated_unix_ms DESC"
        )?;
        let rows = stmt.query_map(params![owner_principal_key, now_ms], |r| {
            Ok(DriverAuthorityOverrideRecord {
                owner_principal_key: r.get(0)?,
                override_id: r.get(1)?,
                device_privacy_key: r.get(2)?,
                behavior: r.get(3)?,
                candidate_version: r.get(4)?,
                provider_id: r.get(5)?,
                expires_unix_ms: r.get(6)?,
                created_unix_ms: r.get(7)?,
                updated_unix_ms: r.get(8)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn delete_driver_authority_override(
        &self,
        owner_principal_key: &str,
        override_id: &str,
    ) -> Result<()> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        conn.execute(
            "DELETE FROM driver_authority_overrides WHERE owner_principal_key=? AND override_id=?",
            params![owner_principal_key, override_id],
        )?;
        Ok(())
    }

    pub fn upsert_driver_authority_scan(&self, r: &DriverAuthorityScanRecord) -> Result<()> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        conn.execute(
            "INSERT INTO driver_authority_scans(owner_principal_key,scan_id,inventory_epoch,authority_coverage,snapshot_json,created_unix_ms) VALUES(?,?,?,?,?,?) \
             ON CONFLICT(owner_principal_key,scan_id) DO UPDATE SET inventory_epoch=excluded.inventory_epoch,authority_coverage=excluded.authority_coverage,snapshot_json=excluded.snapshot_json,created_unix_ms=excluded.created_unix_ms",
            params![r.owner_principal_key,r.scan_id,r.inventory_epoch as i64,r.authority_coverage,r.snapshot_json,r.created_unix_ms],
        )?;
        Ok(())
    }

    pub fn driver_authority_scans_for_owner(
        &self,
        owner_principal_key: &str,
        limit: usize,
    ) -> Result<Vec<DriverAuthorityScanRecord>> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        let mut stmt = conn.prepare("SELECT owner_principal_key,scan_id,inventory_epoch,authority_coverage,snapshot_json,created_unix_ms FROM driver_authority_scans WHERE owner_principal_key=? ORDER BY created_unix_ms DESC LIMIT ?")?;
        let rows = stmt.query_map(params![owner_principal_key, limit.min(100) as i64], |r| {
            let epoch: i64 = r.get(2)?;
            Ok(DriverAuthorityScanRecord {
                owner_principal_key: r.get(0)?,
                scan_id: r.get(1)?,
                inventory_epoch: epoch.max(0) as u64,
                authority_coverage: r.get(3)?,
                snapshot_json: r.get(4)?,
                created_unix_ms: r.get(5)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn add_checkpoint(
        &self,
        plan_id: &str,
        candidate_id: &str,
        checkpoint: &str,
        detail_json: &str,
        now_ms: i64,
    ) -> Result<i64> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        conn.execute(
            "INSERT INTO execution_checkpoints(plan_id,candidate_id,checkpoint,detail_json,created_unix_ms) VALUES(?,?,?,?,?)",
            params![plan_id,candidate_id,checkpoint,detail_json,now_ms],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn checkpoints(&self, plan_id: &str, limit: usize) -> Result<Vec<CheckpointRecord>> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        let mut stmt = conn.prepare(
            "SELECT seq,plan_id,candidate_id,checkpoint,detail_json,created_unix_ms FROM execution_checkpoints WHERE plan_id=? ORDER BY seq DESC LIMIT ?"
        )?;
        let rows = stmt.query_map(params![plan_id, limit.min(500) as i64], |r| {
            Ok(CheckpointRecord {
                seq: r.get(0)?,
                plan_id: r.get(1)?,
                candidate_id: r.get(2)?,
                checkpoint: r.get(3)?,
                detail_json: r.get(4)?,
                created_unix_ms: r.get(5)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn add_recovery_record(&self, r: &RecoveryRecord) -> Result<i64> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        add_recovery_record_in(&conn, r)
    }

    pub fn recovery_records(&self, limit: usize) -> Result<Vec<RecoveryRecord>> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        let mut stmt = conn.prepare(
            "SELECT seq,plan_id,severity,kind,summary,detail,restore_point_sequence,backup_root,created_unix_ms FROM recovery_records ORDER BY seq DESC LIMIT ?"
        )?;
        let rows = stmt.query_map([limit.min(500) as i64], |r| {
            Ok(RecoveryRecord {
                seq: r.get(0)?,
                plan_id: r.get(1)?,
                severity: r.get(2)?,
                kind: r.get(3)?,
                summary: r.get(4)?,
                detail: r.get(5)?,
                restore_point_sequence: r.get(6)?,
                backup_root: r.get(7)?,
                created_unix_ms: r.get(8)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn recovery_records_for_owner(
        &self,
        owner_principal_key: &str,
        limit: usize,
    ) -> Result<Vec<RecoveryRecord>> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        let mut stmt = conn.prepare(
            "SELECT r.seq,r.plan_id,r.severity,r.kind,r.summary,r.detail,r.restore_point_sequence,r.backup_root,r.created_unix_ms FROM recovery_records r INNER JOIN plans p ON p.id=r.plan_id WHERE p.owner_principal_key=? ORDER BY r.seq DESC LIMIT ?"
        )?;
        let rows = stmt.query_map(params![owner_principal_key, limit.min(500) as i64], |r| {
            Ok(RecoveryRecord {
                seq: r.get(0)?,
                plan_id: r.get(1)?,
                severity: r.get(2)?,
                kind: r.get(3)?,
                summary: r.get(4)?,
                detail: r.get(5)?,
                restore_point_sequence: r.get(6)?,
                backup_root: r.get(7)?,
                created_unix_ms: r.get(8)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn upsert_maintenance_execution(&self, r: &MaintenanceExecutionRecord) -> Result<()> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        upsert_maintenance_execution_in(&conn, r)
    }

    pub fn get_maintenance_execution(
        &self,
        plan_id: &str,
    ) -> Result<Option<MaintenanceExecutionRecord>> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        conn.query_row(
            "SELECT plan_id,domain,stage,progress_known,overall_percent,current_item_id,detail,mutation_started,recovery_required,failure_message,outcome,machine_state_fingerprint,repair_graph_digest,reboot_required,reboot_resume_token,verification_state,started_unix_ms,updated_unix_ms,completed_unix_ms FROM maintenance_executions WHERE plan_id=?",
            [plan_id], row_to_maintenance_execution,
        ).optional().map_err(Into::into)
    }

    pub fn latest_maintenance_execution(
        &self,
        domain: &str,
    ) -> Result<Option<MaintenanceExecutionRecord>> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        conn.query_row(
            "SELECT plan_id,domain,stage,progress_known,overall_percent,current_item_id,detail,mutation_started,recovery_required,failure_message,outcome,machine_state_fingerprint,repair_graph_digest,reboot_required,reboot_resume_token,verification_state,started_unix_ms,updated_unix_ms,completed_unix_ms FROM maintenance_executions WHERE domain=? ORDER BY updated_unix_ms DESC LIMIT 1",
            [domain], row_to_maintenance_execution,
        ).optional().map_err(Into::into)
    }

    pub fn latest_maintenance_execution_for_owner(
        &self,
        domain: &str,
        owner_principal_key: &str,
    ) -> Result<Option<MaintenanceExecutionRecord>> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        conn.query_row(
            "SELECT e.plan_id,e.domain,e.stage,e.progress_known,e.overall_percent,e.current_item_id,e.detail,e.mutation_started,e.recovery_required,e.failure_message,e.outcome,e.machine_state_fingerprint,e.repair_graph_digest,e.reboot_required,e.reboot_resume_token,e.verification_state,e.started_unix_ms,e.updated_unix_ms,e.completed_unix_ms FROM maintenance_executions e INNER JOIN plans p ON p.id=e.plan_id WHERE e.domain=? AND p.owner_principal_key=? ORDER BY e.updated_unix_ms DESC LIMIT 1",
            params![domain, owner_principal_key], row_to_maintenance_execution,
        ).optional().map_err(Into::into)
    }

    pub fn insert_repair_timeline_event(&self, r: &RepairTimelineEventRecord) -> Result<()> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        conn.execute(
            "INSERT INTO repair_timeline_events(event_id,plan_id,assessment_id,owner_principal_key,event_kind,domain,action_id,diagnosis_code,outcome,detail,machine_state_fingerprint,created_unix_ms) VALUES(?,?,?,?,?,?,?,?,?,?,?,?)",
            params![r.event_id,r.plan_id,r.assessment_id,r.owner_principal_key,r.event_kind,r.domain,r.action_id,r.diagnosis_code,r.outcome,r.detail,r.machine_state_fingerprint,r.created_unix_ms],
        )?;
        Ok(())
    }

    /// Phase 21 (Timeline Intelligence): read-only, additive owner-scoped reader over
    /// `repair_timeline_events`. No schema change — consumes rows exactly as written.
    pub fn repair_timeline_events_for_owner(
        &self,
        owner_principal_key: &str,
        limit: usize,
    ) -> Result<Vec<RepairTimelineEventRecord>> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        let mut stmt = conn.prepare(
            "SELECT event_id,plan_id,assessment_id,owner_principal_key,event_kind,domain,action_id,diagnosis_code,outcome,detail,machine_state_fingerprint,created_unix_ms FROM repair_timeline_events WHERE owner_principal_key=? ORDER BY created_unix_ms DESC, event_id DESC LIMIT ?"
        )?;
        let rows = stmt.query_map(
            params![owner_principal_key, limit.min(2000) as i64],
            |row| {
                Ok(RepairTimelineEventRecord {
                    event_id: row.get(0)?,
                    plan_id: row.get(1)?,
                    assessment_id: row.get(2)?,
                    owner_principal_key: row.get(3)?,
                    event_kind: row.get(4)?,
                    domain: row.get(5)?,
                    action_id: row.get(6)?,
                    diagnosis_code: row.get(7)?,
                    outcome: row.get(8)?,
                    detail: row.get(9)?,
                    machine_state_fingerprint: row.get(10)?,
                    created_unix_ms: row.get(11)?,
                })
            },
        )?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    /// Phase 21 (Timeline Intelligence): read-only, additive owner-scoped reader over
    /// maintenance executions of owner-visible plans. No schema change.
    pub fn maintenance_executions_for_owner(
        &self,
        owner_principal_key: &str,
        limit: usize,
    ) -> Result<Vec<MaintenanceExecutionRecord>> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        let mut stmt = conn.prepare(
            "SELECT e.plan_id,e.domain,e.stage,e.progress_known,e.overall_percent,e.current_item_id,e.detail,e.mutation_started,e.recovery_required,e.failure_message,e.outcome,e.machine_state_fingerprint,e.repair_graph_digest,e.reboot_required,e.reboot_resume_token,e.verification_state,e.started_unix_ms,e.updated_unix_ms,e.completed_unix_ms FROM maintenance_executions e INNER JOIN plans p ON p.id=e.plan_id WHERE p.owner_principal_key=? ORDER BY e.updated_unix_ms DESC LIMIT ?"
        )?;
        let rows = stmt.query_map(
            params![owner_principal_key, limit.min(2000) as i64],
            row_to_maintenance_execution,
        )?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    // ---------------- Phase 22: One-Click Care orchestration journal ----------------

    /// Inserts or updates a care run row. Crash-safe resume anchor: the row exists
    /// before any step starts and is updated in place as the run progresses.
    pub fn upsert_care_run(&self, r: &CareRunRecord) -> Result<()> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        conn.execute(
            "INSERT INTO care_runs(run_id,owner_principal_key,state,stage,plan_digest_sha256,session_consent_granted,steps_total,steps_done,detail,created_unix_ms,updated_unix_ms,completed_unix_ms) VALUES(?,?,?,?,?,?,?,?,?,?,?,?)\n             ON CONFLICT(run_id) DO UPDATE SET state=excluded.state,stage=excluded.stage,session_consent_granted=excluded.session_consent_granted,steps_done=excluded.steps_done,detail=excluded.detail,updated_unix_ms=excluded.updated_unix_ms,completed_unix_ms=excluded.completed_unix_ms",
            params![r.run_id,r.owner_principal_key,r.state,r.stage,r.plan_digest_sha256,bool_i(r.session_consent_granted),r.steps_total,r.steps_done,r.detail,r.created_unix_ms,r.updated_unix_ms,r.completed_unix_ms],
        )?;
        Ok(())
    }

    pub fn get_care_run(&self, run_id: &str) -> Result<Option<CareRunRecord>> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        conn.query_row(
            "SELECT run_id,owner_principal_key,state,stage,plan_digest_sha256,session_consent_granted,steps_total,steps_done,detail,created_unix_ms,updated_unix_ms,completed_unix_ms FROM care_runs WHERE run_id=?",
            [run_id], row_to_care_run,
        ).optional().map_err(Into::into)
    }

    /// The owner's most recent non-terminal run, if any (resume target).
    pub fn active_care_run_for_owner(
        &self,
        owner_principal_key: &str,
    ) -> Result<Option<CareRunRecord>> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        conn.query_row(
            "SELECT run_id,owner_principal_key,state,stage,plan_digest_sha256,session_consent_granted,steps_total,steps_done,detail,created_unix_ms,updated_unix_ms,completed_unix_ms FROM care_runs WHERE owner_principal_key=? AND state NOT IN ('Completed','Failed','Cancelled') ORDER BY created_unix_ms DESC LIMIT 1",
            [owner_principal_key], row_to_care_run,
        ).optional().map_err(Into::into)
    }

    pub fn care_runs_for_owner(
        &self,
        owner_principal_key: &str,
        limit: usize,
    ) -> Result<Vec<CareRunRecord>> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        let mut stmt = conn.prepare(
            "SELECT run_id,owner_principal_key,state,stage,plan_digest_sha256,session_consent_granted,steps_total,steps_done,detail,created_unix_ms,updated_unix_ms,completed_unix_ms FROM care_runs WHERE owner_principal_key=? ORDER BY created_unix_ms DESC LIMIT ?"
        )?;
        let rows = stmt.query_map(
            params![owner_principal_key, limit.min(100) as i64],
            row_to_care_run,
        )?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    /// Journals a step the first time it runs. Keyed by (run_id, step_index), so no
    /// other run's history is touched.
    pub fn insert_care_step(&self, s: &CareStepRecord) -> Result<()> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        conn.execute(
            "INSERT INTO care_steps(run_id,step_index,domain_plan_id,domain_kind,safety_level,state,outcome,verification_state,failure_message,started_unix_ms,updated_unix_ms) VALUES(?,?,?,?,?,?,?,?,?,?,?)",
            params![s.run_id,s.step_index,s.domain_plan_id,s.domain_kind,s.safety_level,s.state,s.outcome,s.verification_state,s.failure_message,s.started_unix_ms,s.updated_unix_ms],
        )?;
        Ok(())
    }

    /// Updates one step row's mutable columns (state/outcome/verification/failure).
    pub fn update_care_step(&self, r: &CareStepRecord) -> Result<()> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        conn.execute(
            "UPDATE care_steps SET state=?3,outcome=?4,verification_state=?5,failure_message=?6,updated_unix_ms=?7 WHERE run_id=?1 AND step_index=?2",
            params![r.run_id,r.step_index,r.state,r.outcome,r.verification_state,r.failure_message,r.updated_unix_ms],
        )?;
        Ok(())
    }

    pub fn care_steps_for_run(&self, run_id: &str) -> Result<Vec<CareStepRecord>> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        let mut stmt = conn.prepare(
            "SELECT run_id,step_index,domain_plan_id,domain_kind,safety_level,state,outcome,verification_state,failure_message,started_unix_ms,updated_unix_ms FROM care_steps WHERE run_id=? ORDER BY step_index"
        )?;
        let rows = stmt.query_map([run_id], |row| {
            Ok(CareStepRecord {
                run_id: row.get(0)?,
                step_index: row.get::<_, i64>(1)?.max(0).min(u32::MAX as i64) as u32,
                domain_plan_id: row.get(2)?,
                domain_kind: row.get(3)?,
                safety_level: row.get(4)?,
                state: row.get(5)?,
                outcome: row.get(6)?,
                verification_state: row.get(7)?,
                failure_message: row.get(8)?,
                started_unix_ms: row.get(9)?,
                updated_unix_ms: row.get(10)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn upsert_repair_reboot_resume(&self, r: &RepairRebootResumeRecord) -> Result<()> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        conn.execute(
            "INSERT INTO repair_reboot_resume_tickets(token_sha256,owner_principal_key,assessment_id,machine_state_fingerprint,repair_graph_digest,resume_policy,state,created_unix_ms,consumed_unix_ms) VALUES(?,?,?,?,?,?,?,?,?) ON CONFLICT(token_sha256) DO UPDATE SET state=excluded.state,consumed_unix_ms=excluded.consumed_unix_ms",
            params![r.token_sha256,r.owner_principal_key,r.assessment_id,r.machine_state_fingerprint,r.repair_graph_digest,r.resume_policy,r.state,r.created_unix_ms,r.consumed_unix_ms],
        )?;
        Ok(())
    }

    pub fn latest_pending_repair_reboot_resume(
        &self,
        owner: &str,
    ) -> Result<Option<RepairRebootResumeRecord>> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        conn.query_row(
            "SELECT token_sha256,owner_principal_key,assessment_id,machine_state_fingerprint,repair_graph_digest,resume_policy,state,created_unix_ms,consumed_unix_ms FROM repair_reboot_resume_tickets WHERE owner_principal_key=? AND state='AwaitingRebootReassessment' ORDER BY created_unix_ms DESC LIMIT 1",
            [owner],
            |row| Ok(RepairRebootResumeRecord { token_sha256:row.get(0)?,owner_principal_key:row.get(1)?,assessment_id:row.get(2)?,machine_state_fingerprint:row.get(3)?,repair_graph_digest:row.get(4)?,resume_policy:row.get(5)?,state:row.get(6)?,created_unix_ms:row.get(7)?,consumed_unix_ms:row.get(8)? }),
        ).optional().map_err(Into::into)
    }

    pub fn consume_repair_reboot_resume(
        &self,
        token: &str,
        state: &str,
        consumed_unix_ms: i64,
    ) -> Result<()> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        conn.execute("UPDATE repair_reboot_resume_tickets SET state=?2,consumed_unix_ms=?3 WHERE token_sha256=?1 AND state='AwaitingRebootReassessment'", params![token,state,consumed_unix_ms])?;
        Ok(())
    }

    pub fn upsert_maintenance_item(&self, r: &MaintenanceItemRecord) -> Result<()> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        conn.execute(
            "INSERT INTO maintenance_execution_items(plan_id,item_id,kind,stage,result_code,bytes_affected,detail,updated_unix_ms) VALUES(?,?,?,?,?,?,?,?)
             ON CONFLICT(plan_id,item_id) DO UPDATE SET kind=excluded.kind,stage=excluded.stage,result_code=excluded.result_code,bytes_affected=excluded.bytes_affected,detail=excluded.detail,updated_unix_ms=excluded.updated_unix_ms",
            params![r.plan_id,r.item_id,r.kind,r.stage,r.result_code,u64_to_i64(r.bytes_affected),r.detail,r.updated_unix_ms],
        )?;
        Ok(())
    }

    pub fn maintenance_items(&self, plan_id: &str) -> Result<Vec<MaintenanceItemRecord>> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        let mut stmt = conn.prepare(
            "SELECT plan_id,item_id,kind,stage,result_code,bytes_affected,detail,updated_unix_ms FROM maintenance_execution_items WHERE plan_id=? ORDER BY rowid"
        )?;
        let rows = stmt.query_map([plan_id], row_to_maintenance_item)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn scheduler_run(
        &self,
        owner_principal_key: &str,
        workload: &str,
    ) -> Result<Option<SchedulerRunRecord>> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        conn.query_row(
            "SELECT owner_principal_key,workload,failure_count,next_eligible_unix_ms,last_outcome,last_completed_unix_ms,updated_unix_ms FROM autonomous_scheduler_runs WHERE owner_principal_key=? AND workload=?",
            params![owner_principal_key, workload],
            |row| Ok(SchedulerRunRecord {
                owner_principal_key: row.get(0)?, workload: row.get(1)?, failure_count: row.get::<_, i64>(2)?.max(0).min(u32::MAX as i64) as u32,
                next_eligible_unix_ms: row.get(3)?, last_outcome: row.get(4)?, last_completed_unix_ms: row.get(5)?, updated_unix_ms: row.get(6)?,
            }),
        ).optional().map_err(Into::into)
    }

    pub fn upsert_scheduler_run(&self, record: &SchedulerRunRecord) -> Result<()> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        conn.execute(
            "INSERT INTO autonomous_scheduler_runs(owner_principal_key,workload,failure_count,next_eligible_unix_ms,last_outcome,last_completed_unix_ms,updated_unix_ms) VALUES(?,?,?,?,?,?,?) ON CONFLICT(owner_principal_key,workload) DO UPDATE SET failure_count=excluded.failure_count,next_eligible_unix_ms=excluded.next_eligible_unix_ms,last_outcome=excluded.last_outcome,last_completed_unix_ms=excluded.last_completed_unix_ms,updated_unix_ms=excluded.updated_unix_ms",
            params![record.owner_principal_key,record.workload,record.failure_count,record.next_eligible_unix_ms,record.last_outcome,record.last_completed_unix_ms,record.updated_unix_ms],
        )?;
        Ok(())
    }

    pub fn update_manifest_floor(
        &self,
        channel: &str,
    ) -> Result<Option<UpdateManifestFloorRecord>> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        conn.query_row(
            "SELECT channel,highest_sequence,generated_unix_ms,manifest_sha256,updated_unix_ms FROM update_manifest_floor WHERE channel=?",
            [channel],
            |row| Ok(UpdateManifestFloorRecord { channel:row.get(0)?, highest_sequence:row.get::<_,i64>(1)?.max(0) as u64, generated_unix_ms:row.get(2)?, manifest_sha256:row.get(3)?, updated_unix_ms:row.get(4)? }),
        ).optional().map_err(Into::into)
    }

    pub fn upsert_update_manifest_floor(&self, record: &UpdateManifestFloorRecord) -> Result<()> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        conn.execute(
            "INSERT INTO update_manifest_floor(channel,highest_sequence,generated_unix_ms,manifest_sha256,updated_unix_ms) VALUES(?,?,?,?,?) ON CONFLICT(channel) DO UPDATE SET highest_sequence=excluded.highest_sequence,generated_unix_ms=excluded.generated_unix_ms,manifest_sha256=excluded.manifest_sha256,updated_unix_ms=excluded.updated_unix_ms WHERE excluded.highest_sequence>update_manifest_floor.highest_sequence OR (excluded.highest_sequence=update_manifest_floor.highest_sequence AND excluded.manifest_sha256=update_manifest_floor.manifest_sha256)",
            params![record.channel,record.highest_sequence as i64,record.generated_unix_ms,record.manifest_sha256,record.updated_unix_ms],
        )?;
        Ok(())
    }

    pub fn replace_update_execution_guard(
        &self,
        record: &UpdateExecutionGuardRecord,
    ) -> Result<()> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        conn.execute(
            "INSERT INTO active_update_execution(slot,ticket_id,owner_principal_key,release_id,release_version,channel,notes_message_key,minimum_windows_build,staged_path,expected_sha256,expected_size,expires_unix_ms,created_unix_ms) VALUES(1,?,?,?,?,?,?,?,?,?,?,?,?) ON CONFLICT(slot) DO UPDATE SET ticket_id=excluded.ticket_id,owner_principal_key=excluded.owner_principal_key,release_id=excluded.release_id,release_version=excluded.release_version,channel=excluded.channel,notes_message_key=excluded.notes_message_key,minimum_windows_build=excluded.minimum_windows_build,staged_path=excluded.staged_path,expected_sha256=excluded.expected_sha256,expected_size=excluded.expected_size,expires_unix_ms=excluded.expires_unix_ms,created_unix_ms=excluded.created_unix_ms",
            params![record.ticket_id,record.owner_principal_key,record.release_id,record.release_version,record.channel,record.notes_message_key,record.minimum_windows_build as i64,record.staged_path,record.expected_sha256,record.expected_size as i64,record.expires_unix_ms,record.created_unix_ms],
        )?;
        Ok(())
    }

    pub fn active_update_execution_guard(&self) -> Result<Option<UpdateExecutionGuardRecord>> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        conn.query_row(
            "SELECT ticket_id,owner_principal_key,release_id,release_version,channel,notes_message_key,minimum_windows_build,staged_path,expected_sha256,expected_size,expires_unix_ms,created_unix_ms FROM active_update_execution WHERE slot=1",
            [],
            |row| Ok(UpdateExecutionGuardRecord { ticket_id:row.get(0)?, owner_principal_key:row.get(1)?, release_id:row.get(2)?, release_version:row.get(3)?, channel:row.get(4)?, notes_message_key:row.get(5)?, minimum_windows_build:row.get::<_,i64>(6)?.max(0).min(u32::MAX as i64) as u32, staged_path:row.get(7)?, expected_sha256:row.get(8)?, expected_size:row.get::<_,i64>(9)?.max(0) as u64, expires_unix_ms:row.get(10)?, created_unix_ms:row.get(11)? }),
        ).optional().map_err(Into::into)
    }

    pub fn clear_update_execution_guard(&self, ticket_id: &str) -> Result<()> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        conn.execute(
            "DELETE FROM active_update_execution WHERE slot=1 AND ticket_id=?",
            [ticket_id],
        )?;
        Ok(())
    }

    pub fn support_journal_events_for_owner(
        &self,
        owner_principal_key: &str,
        limit: usize,
    ) -> Result<Vec<SupportJournalEventRecord>> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        let mut stmt = conn.prepare(
            "SELECT e.seq,e.plan_id,e.from_state,e.to_state,e.event_kind,e.detail,e.created_unix_ms FROM plan_events e INNER JOIN plans p ON p.id=e.plan_id WHERE p.owner_principal_key=? ORDER BY e.seq DESC LIMIT ?"
        )?;
        let rows = stmt.query_map(params![owner_principal_key, limit.min(500) as i64], |row| {
            Ok(SupportJournalEventRecord {
                seq: row.get(0)?,
                plan_id: row.get(1)?,
                from_state: row.get(2)?,
                to_state: row.get(3)?,
                event_kind: row.get(4)?,
                detail: row.get(5)?,
                created_unix_ms: row.get(6)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn scheduler_runs_for_owner(
        &self,
        owner_principal_key: &str,
    ) -> Result<Vec<SchedulerRunRecord>> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        let mut stmt = conn.prepare("SELECT owner_principal_key,workload,failure_count,next_eligible_unix_ms,last_outcome,last_completed_unix_ms,updated_unix_ms FROM autonomous_scheduler_runs WHERE owner_principal_key=? ORDER BY workload")?;
        let rows = stmt.query_map([owner_principal_key], |row| {
            Ok(SchedulerRunRecord {
                owner_principal_key: row.get(0)?,
                workload: row.get(1)?,
                failure_count: row.get::<_, i64>(2)?.max(0).min(u32::MAX as i64) as u32,
                next_eligible_unix_ms: row.get(3)?,
                last_outcome: row.get(4)?,
                last_completed_unix_ms: row.get(5)?,
                updated_unix_ms: row.get(6)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn save_diagnostic_snapshot(&self, record: &DiagnosticSnapshotRecord) -> Result<()> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        conn.execute(
            "INSERT OR REPLACE INTO diagnostic_snapshots(snapshot_id,state,collected_unix_ms,warning_count,snapshot_json,owner_principal_key) VALUES(?,?,?,?,?,?)",
            params![record.snapshot_id, record.state, record.collected_unix_ms, record.warning_count, record.snapshot_json, record.owner_principal_key],
        )?;
        conn.execute(
            "DELETE FROM diagnostic_snapshots WHERE owner_principal_key=? AND snapshot_id NOT IN (SELECT snapshot_id FROM diagnostic_snapshots WHERE owner_principal_key=? ORDER BY collected_unix_ms DESC LIMIT 50)",
            params![record.owner_principal_key, record.owner_principal_key],
        )?;
        Ok(())
    }

    pub fn latest_diagnostic_snapshot_for_owner(
        &self,
        owner_principal_key: &str,
    ) -> Result<Option<DiagnosticSnapshotRecord>> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        conn.query_row(
            "SELECT snapshot_id,owner_principal_key,state,collected_unix_ms,warning_count,snapshot_json FROM diagnostic_snapshots WHERE owner_principal_key=? ORDER BY collected_unix_ms DESC LIMIT 1",
            [owner_principal_key], |r| Ok(DiagnosticSnapshotRecord{snapshot_id:r.get(0)?,owner_principal_key:r.get(1)?,state:r.get(2)?,collected_unix_ms:r.get(3)?,warning_count:r.get(4)?,snapshot_json:r.get(5)?}),
        ).optional().map_err(Into::into)
    }

    pub fn diagnostic_snapshots_for_owner(
        &self,
        owner_principal_key: &str,
        limit: usize,
    ) -> Result<Vec<DiagnosticSnapshotRecord>> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        let mut stmt=conn.prepare("SELECT snapshot_id,owner_principal_key,state,collected_unix_ms,warning_count,snapshot_json FROM diagnostic_snapshots WHERE owner_principal_key=? ORDER BY collected_unix_ms DESC LIMIT ?")?;
        let rows = stmt.query_map(params![owner_principal_key, limit.min(200) as i64], |r| {
            Ok(DiagnosticSnapshotRecord {
                snapshot_id: r.get(0)?,
                owner_principal_key: r.get(1)?,
                state: r.get(2)?,
                collected_unix_ms: r.get(3)?,
                warning_count: r.get(4)?,
                snapshot_json: r.get(5)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn upsert_startup_change(&self, record: &StartupChangeRecord) -> Result<()> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        conn.execute(
            "INSERT INTO startup_change_records(change_id,origin_change_id,plan_id,item_id,kind,display_name,direction,original_json,applied_json,state,detail,created_unix_ms,updated_unix_ms,restored_unix_ms) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?) ON CONFLICT(change_id) DO UPDATE SET origin_change_id=excluded.origin_change_id,plan_id=excluded.plan_id,item_id=excluded.item_id,kind=excluded.kind,display_name=excluded.display_name,direction=excluded.direction,original_json=excluded.original_json,applied_json=excluded.applied_json,state=excluded.state,detail=excluded.detail,updated_unix_ms=excluded.updated_unix_ms,restored_unix_ms=excluded.restored_unix_ms",
            params![record.change_id,record.origin_change_id,record.plan_id,record.item_id,record.kind,record.display_name,record.direction,record.original_json,record.applied_json,record.state,record.detail,record.created_unix_ms,record.updated_unix_ms,record.restored_unix_ms],
        )?;
        Ok(())
    }

    pub fn get_startup_change(&self, change_id: &str) -> Result<Option<StartupChangeRecord>> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        conn.query_row(
            "SELECT change_id,origin_change_id,plan_id,item_id,kind,display_name,direction,original_json,applied_json,state,detail,created_unix_ms,updated_unix_ms,restored_unix_ms FROM startup_change_records WHERE change_id=?",
            [change_id], row_to_startup_change,
        ).optional().map_err(Into::into)
    }

    pub fn get_startup_change_for_owner(
        &self,
        change_id: &str,
        owner_principal_key: &str,
    ) -> Result<Option<StartupChangeRecord>> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        conn.query_row(
            "SELECT s.change_id,s.origin_change_id,s.plan_id,s.item_id,s.kind,s.display_name,s.direction,s.original_json,s.applied_json,s.state,s.detail,s.created_unix_ms,s.updated_unix_ms,s.restored_unix_ms FROM startup_change_records s INNER JOIN plans p ON p.id=s.plan_id WHERE s.change_id=? AND p.owner_principal_key=?",
            params![change_id, owner_principal_key], row_to_startup_change,
        ).optional().map_err(Into::into)
    }

    pub fn startup_changes(&self, limit: usize) -> Result<Vec<StartupChangeRecord>> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        let mut stmt = conn.prepare("SELECT change_id,origin_change_id,plan_id,item_id,kind,display_name,direction,original_json,applied_json,state,detail,created_unix_ms,updated_unix_ms,restored_unix_ms FROM startup_change_records ORDER BY updated_unix_ms DESC LIMIT ?")?;
        let rows = stmt.query_map([limit.min(500) as i64], row_to_startup_change)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn startup_changes_for_owner(
        &self,
        owner_principal_key: &str,
        limit: usize,
    ) -> Result<Vec<StartupChangeRecord>> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        let mut stmt = conn.prepare("SELECT s.change_id,s.origin_change_id,s.plan_id,s.item_id,s.kind,s.display_name,s.direction,s.original_json,s.applied_json,s.state,s.detail,s.created_unix_ms,s.updated_unix_ms,s.restored_unix_ms FROM startup_change_records s INNER JOIN plans p ON p.id=s.plan_id WHERE p.owner_principal_key=? ORDER BY s.updated_unix_ms DESC LIMIT ?")?;
        let rows = stmt.query_map(
            params![owner_principal_key, limit.min(500) as i64],
            row_to_startup_change,
        )?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn startup_changes_in_states(&self, states: &[&str]) -> Result<Vec<StartupChangeRecord>> {
        if states.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders = std::iter::repeat_n("?", states.len())
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT change_id,origin_change_id,plan_id,item_id,kind,display_name,direction,original_json,applied_json,state,detail,created_unix_ms,updated_unix_ms,restored_unix_ms FROM startup_change_records WHERE state IN ({placeholders}) ORDER BY updated_unix_ms"
        );
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(
            rusqlite::params_from_iter(states.iter().copied()),
            row_to_startup_change,
        )?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }
}

impl Database {
    pub fn save_intelligence_scan(&self, record: &IntelligenceScanRecord) -> Result<()> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        conn.execute(
            "INSERT OR REPLACE INTO intelligence_scans(scan_id,owner_principal_key,state,status,started_unix_ms,completed_unix_ms,machine_state_fingerprint,rule_engine_version,app_version,finding_count,unavailable_collector_count,snapshot_json) VALUES(?,?,?,?,?,?,?,?,?,?,?,?)",
            params![record.scan_id,record.owner_principal_key,record.state,record.status,record.started_unix_ms,record.completed_unix_ms,record.machine_state_fingerprint,record.rule_engine_version,record.app_version,record.finding_count,record.unavailable_collector_count,record.snapshot_json],
        )?;
        conn.execute(
            "DELETE FROM intelligence_scans WHERE owner_principal_key=? AND scan_id NOT IN (SELECT scan_id FROM intelligence_scans WHERE owner_principal_key=? ORDER BY completed_unix_ms DESC LIMIT 50) AND scan_id NOT IN (SELECT scan_id FROM intelligence_remediation_plans)",
            params![record.owner_principal_key,record.owner_principal_key],
        )?;
        Ok(())
    }

    pub fn intelligence_scans_for_owner(
        &self,
        owner_principal_key: &str,
        limit: usize,
    ) -> Result<Vec<IntelligenceScanRecord>> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        let mut stmt=conn.prepare("SELECT scan_id,owner_principal_key,state,status,started_unix_ms,completed_unix_ms,machine_state_fingerprint,rule_engine_version,app_version,finding_count,unavailable_collector_count,snapshot_json FROM intelligence_scans WHERE owner_principal_key=? ORDER BY completed_unix_ms DESC LIMIT ?")?;
        let rows = stmt.query_map(
            params![owner_principal_key, limit.clamp(1, 200) as i64],
            |r| {
                Ok(IntelligenceScanRecord {
                    scan_id: r.get(0)?,
                    owner_principal_key: r.get(1)?,
                    state: r.get(2)?,
                    status: r.get(3)?,
                    started_unix_ms: r.get(4)?,
                    completed_unix_ms: r.get(5)?,
                    machine_state_fingerprint: r.get(6)?,
                    rule_engine_version: r.get(7)?,
                    app_version: r.get(8)?,
                    finding_count: r.get::<_, i64>(9)?.max(0) as u32,
                    unavailable_collector_count: r.get::<_, i64>(10)?.max(0) as u32,
                    snapshot_json: r.get(11)?,
                })
            },
        )?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn upsert_intelligence_finding(&self, record: &IntelligenceFindingRecord) -> Result<()> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        conn.execute(
            "INSERT INTO intelligence_findings(owner_principal_key,finding_id,finding_code,first_observed_unix_ms,last_observed_unix_ms,lifecycle,severity,confidence,verification_status,resolved_at_unix_ms,resolution_scan_id,resolution_reason_key,finding_json) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?) ON CONFLICT(owner_principal_key,finding_id) DO UPDATE SET finding_code=excluded.finding_code,first_observed_unix_ms=MIN(intelligence_findings.first_observed_unix_ms,excluded.first_observed_unix_ms),last_observed_unix_ms=excluded.last_observed_unix_ms,lifecycle=excluded.lifecycle,severity=excluded.severity,confidence=excluded.confidence,verification_status=excluded.verification_status,resolved_at_unix_ms=excluded.resolved_at_unix_ms,resolution_scan_id=excluded.resolution_scan_id,resolution_reason_key=excluded.resolution_reason_key,finding_json=excluded.finding_json",
            params![
                record.owner_principal_key, record.finding_id, record.finding_code,
                record.first_observed_unix_ms, record.last_observed_unix_ms,
                record.lifecycle, record.severity, record.confidence,
                record.verification_status, record.resolved_at_unix_ms,
                record.resolution_scan_id, record.resolution_reason_key, record.finding_json
            ],
        )?;
        Ok(())
    }

    pub fn intelligence_findings_for_owner(
        &self,
        owner_principal_key: &str,
    ) -> Result<Vec<IntelligenceFindingRecord>> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        let mut stmt = conn.prepare(
            "SELECT owner_principal_key,finding_id,finding_code,first_observed_unix_ms,last_observed_unix_ms,lifecycle,severity,confidence,verification_status,resolved_at_unix_ms,resolution_scan_id,resolution_reason_key,finding_json FROM intelligence_findings WHERE owner_principal_key=? ORDER BY last_observed_unix_ms DESC",
        )?;
        let rows = stmt.query_map([owner_principal_key], |r| {
            Ok(IntelligenceFindingRecord {
                owner_principal_key: r.get(0)?,
                finding_id: r.get(1)?,
                finding_code: r.get(2)?,
                first_observed_unix_ms: r.get(3)?,
                last_observed_unix_ms: r.get(4)?,
                lifecycle: r.get(5)?,
                severity: r.get(6)?,
                confidence: r.get(7)?,
                verification_status: r.get(8)?,
                resolved_at_unix_ms: r.get(9)?,
                resolution_scan_id: r.get(10)?,
                resolution_reason_key: r.get(11)?,
                finding_json: r.get(12)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn save_intelligence_remediation_plan(
        &self,
        owner_principal_key: &str,
        plan_id: &str,
        scan_id: &str,
        digest: &str,
        immutable_json: &str,
        created_unix_ms: i64,
    ) -> Result<()> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        conn.execute("INSERT INTO intelligence_remediation_plans(plan_id,owner_principal_key,scan_id,digest,immutable_json,created_unix_ms) VALUES(?,?,?,?,?,?)",params![plan_id,owner_principal_key,scan_id,digest,immutable_json,created_unix_ms])?;
        Ok(())
    }

    pub fn upsert_intelligence_override(&self, record: &IntelligenceOverrideRecord) -> Result<()> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        conn.execute("INSERT INTO intelligence_overrides(owner_principal_key,override_id,scope_kind,scope_value_hash,behavior,expires_unix_ms,created_unix_ms,updated_unix_ms) VALUES(?,?,?,?,?,?,?,?) ON CONFLICT(owner_principal_key,override_id) DO UPDATE SET scope_kind=excluded.scope_kind,scope_value_hash=excluded.scope_value_hash,behavior=excluded.behavior,expires_unix_ms=excluded.expires_unix_ms,updated_unix_ms=excluded.updated_unix_ms",params![record.owner_principal_key,record.override_id,record.scope_kind,record.scope_value_hash,record.behavior,record.expires_unix_ms,record.created_unix_ms,record.updated_unix_ms])?;
        Ok(())
    }

    pub fn intelligence_overrides_for_owner(
        &self,
        owner_principal_key: &str,
        now_ms: i64,
    ) -> Result<Vec<IntelligenceOverrideRecord>> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        let mut stmt=conn.prepare("SELECT owner_principal_key,override_id,scope_kind,scope_value_hash,behavior,expires_unix_ms,created_unix_ms,updated_unix_ms FROM intelligence_overrides WHERE owner_principal_key=? AND (expires_unix_ms IS NULL OR expires_unix_ms>?) ORDER BY updated_unix_ms DESC")?;
        let rows = stmt.query_map(params![owner_principal_key, now_ms], |r| {
            Ok(IntelligenceOverrideRecord {
                owner_principal_key: r.get(0)?,
                override_id: r.get(1)?,
                scope_kind: r.get(2)?,
                scope_value_hash: r.get(3)?,
                behavior: r.get(4)?,
                expires_unix_ms: r.get(5)?,
                created_unix_ms: r.get(6)?,
                updated_unix_ms: r.get(7)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn delete_intelligence_override(
        &self,
        owner_principal_key: &str,
        override_id: &str,
    ) -> Result<()> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        conn.execute(
            "DELETE FROM intelligence_overrides WHERE owner_principal_key=? AND override_id=?",
            params![owner_principal_key, override_id],
        )?;
        Ok(())
    }

    pub fn recent_recovery_event_count_for_owner(&self, owner_principal_key: &str) -> Result<u32> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        let count:i64=conn.query_row("SELECT COUNT(*) FROM plan_events e INNER JOIN plans p ON p.id=e.plan_id WHERE p.owner_principal_key=? AND (e.event_kind LIKE '%recovery%' OR e.to_state='RecoveryRequired')",[owner_principal_key],|r|r.get(0))?;
        Ok(count.max(0).min(u32::MAX as i64) as u32)
    }

    // =====================================================================
    // Phase 34 — Fleet & Secure Remote Operations (additive methods only).
    // Persistence stores NON-SECRET fleet configuration and metadata only.
    // There is no column or API that can store password, private-key body,
    // passphrase or API key material — the audit gates check this.
    // =====================================================================

    /// Upserts one fleet host row from its strict-domain JSON serialization.
    /// Phase 34 corrective: the JSON is parsed through the AUTHORITATIVE
    /// strict `FleetHost` type (`deny_unknown_fields` + full re-validation),
    /// and the DB columns are derived through an EXHAUSTIVE match on
    /// `AuthReference`. Malformed or secret-shaped auth representations are
    /// typed rejections — never silently defaulted to `"agent"`. Secrets are
    /// structurally absent from that schema.
    pub fn upsert_fleet_host(&self, host_json: &str, updated_unix_ms: i64) -> Result<()> {
        let invalid = |message: String| {
            PersistenceError::Sqlite(rusqlite::Error::ToSqlConversionFailure(Box::new(
                std::io::Error::new(std::io::ErrorKind::InvalidData, message),
            )))
        };
        let host = aethercore_fleet::FleetHost::from_bytes(host_json.as_bytes())
            .map_err(|error| invalid(format!("strict fleet host parse: {error}")))?;
        let (auth_ref_kind, auth_ref_path) = match &host.auth {
            aethercore_fleet::AuthReference::Agent => ("agent".to_string(), None),
            aethercore_fleet::AuthReference::KeyFile { path } => {
                ("key_file".to_string(), Some(path.clone()))
            }
            aethercore_fleet::AuthReference::Certificate { path } => {
                ("certificate".to_string(), Some(path.clone()))
            }
        };
        let (trusted_fingerprint, trusted_key_type, trusted_unix_ms): (
            Option<String>,
            Option<String>,
            Option<i64>,
        ) = match &host.trust {
            Some(trust) => (
                Some(trust.host_key_sha256.clone()),
                Some(trust.key_type.clone()),
                Some(trust.trusted_unix_ms),
            ),
            None => (None, None, None),
        };
        let tags_json =
            serde_json::to_string(&host.tags.iter().collect::<std::collections::BTreeSet<_>>())
                .map_err(|error| invalid(format!("tags serialize: {error}")))?;
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        conn.execute(
            "INSERT INTO fleet_hosts (host_id, display_name, hostname, port, username, auth_ref_kind, auth_ref_path, trusted_fingerprint, trusted_key_type, trusted_unix_ms, enabled, tags_json, updated_unix_ms)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)
             ON CONFLICT(host_id) DO UPDATE SET
               display_name=excluded.display_name, hostname=excluded.hostname,
               port=excluded.port, username=excluded.username,
               auth_ref_kind=excluded.auth_ref_kind, auth_ref_path=excluded.auth_ref_path,
               trusted_fingerprint=excluded.trusted_fingerprint,
               trusted_key_type=excluded.trusted_key_type,
               trusted_unix_ms=excluded.trusted_unix_ms,
               enabled=excluded.enabled, tags_json=excluded.tags_json,
               updated_unix_ms=excluded.updated_unix_ms",
            rusqlite::params![
                host.host_id,
                host.display_name,
                host.hostname,
                i64::from(host.port),
                host.username,
                auth_ref_kind,
                auth_ref_path,
                trusted_fingerprint,
                trusted_key_type,
                trusted_unix_ms,
                host.enabled,
                tags_json,
                updated_unix_ms
            ],
        )?;
        Ok(())
    }

    /// Deletes a fleet host row (explicit administrative action).
    pub fn delete_fleet_host(&self, host_id: &str) -> Result<()> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        conn.execute("DELETE FROM fleet_hosts WHERE host_id=?1", [host_id])?;
        Ok(())
    }

    /// All fleet host rows as strict-domain JSON strings (deterministic order).
    /// Phase 34 corrective: the auth columns are restored into the same typed
    /// domain through an EXHAUSTIVE match — an unknown `auth_ref_kind` value
    /// in the DB is a hard rejection, never a silent `"agent"` fallback.
    pub fn fleet_hosts(&self) -> Result<Vec<String>> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        let mut statement = conn.prepare(
            "SELECT host_id, display_name, hostname, port, username, auth_ref_kind, auth_ref_path,
                    trusted_fingerprint, trusted_key_type, trusted_unix_ms, enabled, tags_json
             FROM fleet_hosts ORDER BY host_id ASC",
        )?;
        let rows = statement.query_map([], |row| {
            let auth_ref_kind: String = row.get(5)?;
            let auth_ref_path: Option<String> = row.get(6)?;
            // Exhaustive typed restoration of AuthReference. Any auth_ref_kind
            // outside the domain's three variants is a read-time rejection.
            let auth_value = match (auth_ref_kind.as_str(), auth_ref_path) {
                ("agent", None) => serde_json::json!("agent"),
                ("key_file", Some(path)) => serde_json::json!({"key_file": {"path": path}}),
                ("certificate", Some(path)) => {
                    serde_json::json!({"certificate": {"path": path}})
                }
                // key_file/certificate without a path reference, an unexpected
                // path with agent, or any unknown kind: rejected.
                _ => {
                    return Err(rusqlite::Error::FromSqlConversionFailure(
                        5,
                        rusqlite::types::Type::Text,
                        format!("corrupt fleet row: unknown auth_ref_kind/auth_ref_path combination {auth_ref_kind:?}").into(),
                    ))
                }
            };
            let mut host = serde_json::json!({
                "schema": "aethercore.fleet.host.v1",
                "host_id": row.get::<_, String>(0)?,
                "display_name": row.get::<_, String>(1)?,
                "hostname": row.get::<_, String>(2)?,
                "port": row.get::<_, i64>(3)?,
                "username": row.get::<_, String>(4)?,
                "auth": auth_value,
                "enabled": row.get::<_, i64>(10)? != 0,
                "tags": serde_json::from_str::<serde_json::Value>(&row.get::<_, String>(11)?)
                    .unwrap_or(serde_json::json!([])),
            });
            if let (Some(fp), Some(kt), Some(ts)) = (
                row.get::<_, Option<String>>(7)?,
                row.get::<_, Option<String>>(8)?,
                row.get::<_, Option<i64>>(9)?,
            ) {
                host["trust"] = serde_json::json!({
                    "host_key_sha256": fp, "key_type": kt, "trusted_unix_ms": ts,
                });
            }
            Ok(host.to_string())
        })?;
        let mut hosts = Vec::new();
        for row in rows {
            hosts.push(row?);
        }
        Ok(hosts)
    }

    /// Upserts a fleet schedule row.
    #[allow(clippy::too_many_arguments)]
    pub fn upsert_fleet_schedule(
        &self,
        schedule_id: &str,
        scope_json: &str,
        profile_id: &str,
        enabled: bool,
        cadence_kind: &str,
        cadence_value: i64,
        next_run_unix_ms: i64,
        last_result_json: Option<&str>,
        updated_unix_ms: i64,
    ) -> Result<()> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        conn.execute(
            "INSERT INTO fleet_schedules (schedule_id, scope_json, profile_id, enabled, cadence_kind, cadence_value, next_run_unix_ms, last_result_json, updated_unix_ms)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)
             ON CONFLICT(schedule_id) DO UPDATE SET
               scope_json=excluded.scope_json, profile_id=excluded.profile_id,
               enabled=excluded.enabled, cadence_kind=excluded.cadence_kind,
               cadence_value=excluded.cadence_value,
               next_run_unix_ms=excluded.next_run_unix_ms,
               last_result_json=excluded.last_result_json,
               updated_unix_ms=excluded.updated_unix_ms",
            rusqlite::params![
                schedule_id,
                scope_json,
                profile_id,
                enabled,
                cadence_kind,
                cadence_value,
                next_run_unix_ms,
                last_result_json,
                updated_unix_ms
            ],
        )?;
        Ok(())
    }

    pub fn delete_fleet_schedule(&self, schedule_id: &str) -> Result<()> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        conn.execute(
            "DELETE FROM fleet_schedules WHERE schedule_id=?1",
            [schedule_id],
        )?;
        Ok(())
    }

    /// Appends one fleet run-history record; returns its sequence number.
    /// History is append-only: failures update nothing here, they append.
    #[allow(clippy::too_many_arguments)]
    pub fn append_fleet_run(
        &self,
        schedule_id: Option<&str>,
        trigger_kind: &str,
        started_unix_ms: i64,
        hosts_attempted: i64,
        hosts_ok: i64,
        hosts_failed: i64,
        outcome_summary: &str,
    ) -> Result<i64> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| PersistenceError::Poisoned)?;
        conn.execute(
            "INSERT INTO fleet_run_history (schedule_id, trigger_kind, started_unix_ms, finished_unix_ms, hosts_attempted, hosts_ok, hosts_failed, outcome_summary)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            rusqlite::params![
                schedule_id,
                trigger_kind,
                started_unix_ms,
                started_unix_ms,
                hosts_attempted,
                hosts_ok,
                hosts_failed,
                outcome_summary
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }
}

// Statement bodies shared by the standalone writers and `transition_plan_with_journal`, which
// must run them on one transaction.
fn transition_plan_in(
    conn: &Connection,
    id: &str,
    expected: &str,
    next: &str,
    detail: &str,
    now_ms: i64,
) -> Result<bool> {
    let changed = conn.execute(
        "UPDATE plans SET state=?,updated_unix_ms=? WHERE id=? AND state=?",
        params![next, now_ms, id, expected],
    )?;
    if changed == 1 {
        conn.execute(
            "INSERT INTO plan_events(plan_id,from_state,to_state,event_kind,detail,created_unix_ms) VALUES(?,?,?,?,?,?)",
            params![id,expected,next,"state_transition",detail,now_ms],
        )?;
    }
    Ok(changed == 1)
}

fn upsert_execution_in(conn: &Connection, r: &ExecutionRecord) -> Result<()> {
    conn.execute(
        "INSERT INTO plan_executions(plan_id,stage,progress_known,overall_percent,current_candidate_id,bytes_downloaded,bytes_total,detail,reboot_required,reboot_boot_marker_ms,restore_point_sequence,backup_root,mutation_started,recovery_required,failure_message,started_unix_ms,updated_unix_ms,completed_unix_ms,bytes_downloaded_known,bytes_total_known) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)
         ON CONFLICT(plan_id) DO UPDATE SET stage=excluded.stage,progress_known=excluded.progress_known,overall_percent=excluded.overall_percent,current_candidate_id=excluded.current_candidate_id,bytes_downloaded=excluded.bytes_downloaded,bytes_total=excluded.bytes_total,bytes_downloaded_known=excluded.bytes_downloaded_known,bytes_total_known=excluded.bytes_total_known,detail=excluded.detail,reboot_required=excluded.reboot_required,reboot_boot_marker_ms=excluded.reboot_boot_marker_ms,restore_point_sequence=excluded.restore_point_sequence,backup_root=excluded.backup_root,mutation_started=excluded.mutation_started,recovery_required=excluded.recovery_required,failure_message=excluded.failure_message,updated_unix_ms=excluded.updated_unix_ms,completed_unix_ms=excluded.completed_unix_ms",
        params![r.plan_id,r.stage,bool_i(r.progress_known),r.overall_percent,r.current_candidate_id,u64_to_i64(r.bytes_downloaded.unwrap_or(0)),u64_to_i64(r.bytes_total.unwrap_or(0)),r.detail,bool_i(r.reboot_required),r.reboot_boot_marker_ms,r.restore_point_sequence,r.backup_root,bool_i(r.mutation_started),bool_i(r.recovery_required),r.failure_message,r.started_unix_ms,r.updated_unix_ms,r.completed_unix_ms,bool_i(r.bytes_downloaded.is_some()),bool_i(r.bytes_total.is_some())],
    )?;
    Ok(())
}

fn upsert_maintenance_execution_in(
    conn: &Connection,
    r: &MaintenanceExecutionRecord,
) -> Result<()> {
    conn.execute(
        "INSERT INTO maintenance_executions(plan_id,domain,stage,progress_known,overall_percent,current_item_id,detail,mutation_started,recovery_required,failure_message,outcome,machine_state_fingerprint,repair_graph_digest,reboot_required,reboot_resume_token,verification_state,started_unix_ms,updated_unix_ms,completed_unix_ms) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)
         ON CONFLICT(plan_id) DO UPDATE SET domain=excluded.domain,stage=excluded.stage,progress_known=excluded.progress_known,overall_percent=excluded.overall_percent,current_item_id=excluded.current_item_id,detail=excluded.detail,mutation_started=excluded.mutation_started,recovery_required=excluded.recovery_required,failure_message=excluded.failure_message,outcome=excluded.outcome,machine_state_fingerprint=excluded.machine_state_fingerprint,repair_graph_digest=excluded.repair_graph_digest,reboot_required=excluded.reboot_required,reboot_resume_token=excluded.reboot_resume_token,verification_state=excluded.verification_state,updated_unix_ms=excluded.updated_unix_ms,completed_unix_ms=excluded.completed_unix_ms",
        params![r.plan_id,r.domain,r.stage,bool_i(r.progress_known),r.overall_percent,r.current_item_id,r.detail,bool_i(r.mutation_started),bool_i(r.recovery_required),r.failure_message,r.outcome,r.machine_state_fingerprint,r.repair_graph_digest,bool_i(r.reboot_required),r.reboot_resume_token,r.verification_state,r.started_unix_ms,r.updated_unix_ms,r.completed_unix_ms],
    )?;
    Ok(())
}

fn add_recovery_record_in(conn: &Connection, r: &RecoveryRecord) -> Result<i64> {
    conn.execute(
        "INSERT INTO recovery_records(plan_id,severity,kind,summary,detail,restore_point_sequence,backup_root,created_unix_ms) VALUES(?,?,?,?,?,?,?,?)",
        params![r.plan_id,r.severity,r.kind,r.summary,r.detail,r.restore_point_sequence,r.backup_root,r.created_unix_ms],
    )?;
    Ok(conn.last_insert_rowid())
}

fn apply_migrations(conn: &mut Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (version INTEGER PRIMARY KEY, name TEXT NOT NULL, checksum_sha256 TEXT NOT NULL, applied_unix_ms INTEGER NOT NULL);",
    )?;
    for (version, name, sql) in MIGRATIONS {
        let checksum = hex_lower(&Sha256::digest(sql.as_bytes()));
        let existing: Option<String> = conn
            .query_row(
                "SELECT checksum_sha256 FROM schema_migrations WHERE version=?",
                [version],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(existing) = existing {
            if existing != checksum {
                return Err(PersistenceError::MigrationDrift(*version));
            }
            continue;
        }
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute_batch(sql)?;
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        tx.execute(
            "INSERT INTO schema_migrations(version,name,checksum_sha256,applied_unix_ms) VALUES(?,?,?,?)",
            params![version,name,checksum,now_ms],
        )?;
        tx.commit()?;
    }
    Ok(())
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

fn bool_i(v: bool) -> i32 {
    if v { 1 } else { 0 }
}

/// SQLite has no unsigned 64-bit integers. Byte-count columns are stored as i64; values above
/// `i64::MAX` saturate instead of wrapping, and reads clamp negatives (legacy/corrupt rows) to 0.
fn u64_to_i64(v: u64) -> i64 {
    i64::try_from(v).unwrap_or(i64::MAX)
}
fn i64_to_u64(v: i64) -> u64 {
    v.max(0) as u64
}

fn row_to_care_run(row: &rusqlite::Row<'_>) -> rusqlite::Result<CareRunRecord> {
    Ok(CareRunRecord {
        run_id: row.get(0)?,
        owner_principal_key: row.get(1)?,
        state: row.get(2)?,
        stage: row.get(3)?,
        plan_digest_sha256: row.get(4)?,
        session_consent_granted: row.get::<_, i32>(5)? != 0,
        steps_total: row.get::<_, i64>(6)?.max(0).min(u32::MAX as i64) as u32,
        steps_done: row.get::<_, i64>(7)?.max(0).min(u32::MAX as i64) as u32,
        detail: row.get(8)?,
        created_unix_ms: row.get(9)?,
        updated_unix_ms: row.get(10)?,
        completed_unix_ms: row.get(11)?,
    })
}

fn row_to_plan(row: &rusqlite::Row<'_>) -> rusqlite::Result<PlanRecord> {
    Ok(PlanRecord {
        id: row.get(0)?,
        title: row.get(1)?,
        state: row.get(2)?,
        digest: row.get(3)?,
        risk: row.get(4)?,
        immutable_json: row.get(5)?,
        created_unix_ms: row.get(6)?,
        updated_unix_ms: row.get(7)?,
        owner_principal_key: row.get(8)?,
    })
}

fn row_to_consent_intent(row: &rusqlite::Row<'_>) -> rusqlite::Result<ConsentIntentRecord> {
    Ok(ConsentIntentRecord {
        intent_id: row.get(0)?,
        plan_id: row.get(1)?,
        digest: row.get(2)?,
        owner_principal_key: row.get(3)?,
        created_unix_ms: row.get(4)?,
        expires_unix_ms: row.get(5)?,
        approved_unix_ms: row.get(6)?,
        consumed_unix_ms: row.get(7)?,
        broker_pid: row.get(8)?,
    })
}

fn row_to_execution(row: &rusqlite::Row<'_>) -> rusqlite::Result<ExecutionRecord> {
    Ok(ExecutionRecord {
        plan_id: row.get(0)?,
        stage: row.get(1)?,
        progress_known: row.get::<_, i32>(2)? != 0,
        overall_percent: row.get(3)?,
        current_candidate_id: row.get(4)?,
        // DBT-P46-B16: the _known columns (migration 0016) carry whether the
        // stored figure was ever determined; a pre-0016 row reads as not
        // determined, which is the honest answer for data written before the
        // distinction existed.
        bytes_downloaded: row
            .get::<_, i64>(18)?
            .ne(&0)
            .then(|| i64_to_u64(row.get::<_, i64>(5).unwrap_or(0))),
        bytes_total: row
            .get::<_, i64>(19)?
            .ne(&0)
            .then(|| i64_to_u64(row.get::<_, i64>(6).unwrap_or(0))),
        detail: row.get(7)?,
        reboot_required: row.get::<_, i32>(8)? != 0,
        reboot_boot_marker_ms: row.get(9)?,
        restore_point_sequence: row.get(10)?,
        backup_root: row.get(11)?,
        mutation_started: row.get::<_, i32>(12)? != 0,
        recovery_required: row.get::<_, i32>(13)? != 0,
        failure_message: row.get(14)?,
        started_unix_ms: row.get(15)?,
        updated_unix_ms: row.get(16)?,
        completed_unix_ms: row.get(17)?,
    })
}

fn row_to_install_item(row: &rusqlite::Row<'_>) -> rusqlite::Result<InstallItemRecord> {
    Ok(InstallItemRecord {
        plan_id: row.get(0)?,
        candidate_id: row.get(1)?,
        update_id: row.get(2)?,
        revision: row.get(3)?,
        instance_id: row.get(4)?,
        title: row.get(5)?,
        stage: row.get(6)?,
        progress_known: row.get::<_, i32>(7)? != 0,
        progress_percent: row.get(8)?,
        result_code: row.get(9)?,
        hresult: row.get(10)?,
        reboot_required: row.get::<_, i32>(11)? != 0,
        verified: row.get::<_, i32>(12)? != 0,
        before_driver_json: row.get(13)?,
        after_driver_json: row.get(14)?,
        before_problem_code: row.get(15)?,
        after_problem_code: row.get(16)?,
        backup_path: row.get(17)?,
        detail: row.get(18)?,
        updated_unix_ms: row.get(19)?,
    })
}

fn row_to_maintenance_execution(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<MaintenanceExecutionRecord> {
    Ok(MaintenanceExecutionRecord {
        plan_id: row.get(0)?,
        domain: row.get(1)?,
        stage: row.get(2)?,
        progress_known: row.get::<_, i32>(3)? != 0,
        overall_percent: row.get(4)?,
        current_item_id: row.get(5)?,
        detail: row.get(6)?,
        mutation_started: row.get::<_, i32>(7)? != 0,
        recovery_required: row.get::<_, i32>(8)? != 0,
        failure_message: row.get(9)?,
        outcome: row.get(10)?,
        machine_state_fingerprint: row.get(11)?,
        repair_graph_digest: row.get(12)?,
        reboot_required: row.get::<_, i32>(13)? != 0,
        reboot_resume_token: row.get(14)?,
        verification_state: row.get(15)?,
        started_unix_ms: row.get(16)?,
        updated_unix_ms: row.get(17)?,
        completed_unix_ms: row.get(18)?,
    })
}

fn row_to_maintenance_item(row: &rusqlite::Row<'_>) -> rusqlite::Result<MaintenanceItemRecord> {
    Ok(MaintenanceItemRecord {
        plan_id: row.get(0)?,
        item_id: row.get(1)?,
        kind: row.get(2)?,
        stage: row.get(3)?,
        result_code: row.get(4)?,
        bytes_affected: i64_to_u64(row.get::<_, i64>(5)?),
        detail: row.get(6)?,
        updated_unix_ms: row.get(7)?,
    })
}

fn row_to_startup_change(row: &rusqlite::Row<'_>) -> rusqlite::Result<StartupChangeRecord> {
    Ok(StartupChangeRecord {
        change_id: row.get(0)?,
        origin_change_id: row.get(1)?,
        plan_id: row.get(2)?,
        item_id: row.get(3)?,
        kind: row.get(4)?,
        display_name: row.get(5)?,
        direction: row.get(6)?,
        original_json: row.get(7)?,
        applied_json: row.get(8)?,
        state: row.get(9)?,
        detail: row.get(10)?,
        created_unix_ms: row.get(11)?,
        updated_unix_ms: row.get(12)?,
        restored_unix_ms: row.get(13)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn temp_db() -> (Database, std::path::PathBuf) {
        let path =
            std::env::temp_dir().join(format!("aethercore-persistence-{}.db", Uuid::new_v4()));
        (Database::open(&path).unwrap(), path)
    }

    #[test]
    fn uncommitted_transition_is_rolled_back_like_power_loss() {
        let (db, path) = temp_db();
        let plan = PlanRecord {
            id: "plan-atomicity".into(),
            title: "Atomicity".into(),
            state: "Draft".into(),
            digest: "digest-atomicity".into(),
            risk: "Amber".into(),
            immutable_json: "{}".into(),
            created_unix_ms: 1,
            updated_unix_ms: 1,
            owner_principal_key: "test-owner".into(),
        };
        db.insert_plan(&plan, "plan_created").unwrap();
        {
            let mut conn = db.connection.lock().unwrap();
            let tx = conn
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .unwrap();
            tx.execute(
                "UPDATE plans SET state='Scanning', updated_unix_ms=2 WHERE id=?",
                [&plan.id],
            )
            .unwrap();
            tx.execute("INSERT INTO plan_events(plan_id,from_state,to_state,event_kind,detail,created_unix_ms) VALUES(?,?,?,?,?,?)", params![plan.id,"Draft","Scanning","state_transition","fault injection",2]).unwrap();
        }
        assert_eq!(db.get_plan(&plan.id).unwrap().unwrap().state, "Draft");
        assert_eq!(db.event_count().unwrap(), 1);
        drop(db);
        cleanup_database(&path);
    }

    #[test]
    fn consent_consumption_and_plan_transition_are_atomic_and_one_shot() {
        let (db, path) = temp_db();
        let owner = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let plan = PlanRecord {
            id: "consent-plan".into(),
            title: "Consent".into(),
            state: "AwaitingAuthorization".into(),
            digest: "digest-consent".into(),
            risk: "Amber".into(),
            immutable_json: "{}".into(),
            created_unix_ms: 1,
            updated_unix_ms: 1,
            owner_principal_key: owner.into(),
        };
        db.insert_plan(&plan, "plan_created").unwrap();
        db.begin_consent_intent("intent-1", &plan.id, &plan.digest, owner, 10, 10_000)
            .unwrap();
        assert!(
            db.approve_consent_intent("intent-1", owner, 4242, 20)
                .unwrap()
        );

        // Wrong expected state must roll back the attempted consume as well as the plan update.
        assert!(
            !db.consume_consent_and_transition(
                &plan.id,
                owner,
                "Draft",
                "Preflight",
                "wrong expected",
                30
            )
            .unwrap()
        );
        assert!(
            db.get_consent_intent("intent-1")
                .unwrap()
                .unwrap()
                .consumed_unix_ms
                .is_none()
        );
        assert_eq!(
            db.get_plan(&plan.id).unwrap().unwrap().state,
            "AwaitingAuthorization"
        );

        assert!(
            db.consume_consent_and_transition(
                &plan.id,
                owner,
                "AwaitingAuthorization",
                "Preflight",
                "approved",
                40
            )
            .unwrap()
        );
        assert_eq!(
            db.get_consent_intent("intent-1")
                .unwrap()
                .unwrap()
                .consumed_unix_ms,
            Some(40)
        );
        assert_eq!(db.get_plan(&plan.id).unwrap().unwrap().state, "Preflight");
        assert!(
            !db.consume_consent_and_transition(
                &plan.id,
                owner,
                "Preflight",
                "Protected",
                "replay",
                50
            )
            .unwrap()
        );
        drop(db);
        cleanup_database(&path);
    }

    fn journal_plan(db: &Database, id: &str, state: &str) {
        db.insert_plan(
            &PlanRecord {
                id: id.into(),
                title: "Journal".into(),
                state: state.into(),
                digest: format!("digest-{id}"),
                risk: "Amber".into(),
                immutable_json: "{}".into(),
                created_unix_ms: 1,
                updated_unix_ms: 1,
                owner_principal_key: "test-owner".into(),
            },
            "plan_created",
        )
        .unwrap();
    }

    fn failed_journal(plan_id: &str) -> (MaintenanceExecutionRecord, RecoveryRecord) {
        (
            MaintenanceExecutionRecord {
                plan_id: plan_id.into(),
                domain: "Cleanup".into(),
                stage: "Failed".into(),
                mutation_started: true,
                recovery_required: true,
                updated_unix_ms: 5,
                completed_unix_ms: Some(5),
                ..Default::default()
            },
            RecoveryRecord {
                plan_id: plan_id.into(),
                severity: "Amber".into(),
                kind: "CleanupFailed".into(),
                created_unix_ms: 5,
                ..Default::default()
            },
        )
    }

    /// DBT-P63-012. The probe trigger fires inside the transition's own UPDATE, so it captures
    /// the journal exactly as any reader could see it at the instant the plan turned terminal.
    #[test]
    fn terminal_state_is_committed_with_the_journal_that_explains_it() {
        let (db, path) = temp_db();
        journal_plan(&db, "p-probe", "Executing");
        db.connection
            .lock()
            .unwrap()
            .execute_batch(
                "CREATE TABLE probe(stage TEXT, recovery_required INTEGER, completed_unix_ms INTEGER);
                 CREATE TRIGGER probe_flip AFTER UPDATE OF state ON plans
                 WHEN NEW.state IN ('Failed','Completed') BEGIN
                   INSERT INTO probe SELECT stage,recovery_required,completed_unix_ms
                   FROM maintenance_executions WHERE plan_id=NEW.id;
                 END;",
            )
            .unwrap();
        let (record, recovery) = failed_journal("p-probe");
        assert!(
            db.transition_plan_with_journal(
                "p-probe",
                "Executing",
                "Failed",
                "cleanup failed",
                5,
                PlanJournal::Maintenance(&record, Some(&recovery)),
            )
            .unwrap()
        );
        let probe: (String, i64, Option<i64>) = db
            .connection
            .lock()
            .unwrap()
            .query_row("SELECT * FROM probe", [], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .unwrap();
        assert_eq!(probe, ("Failed".into(), 1, Some(5)));
        assert_eq!(db.recovery_records(10).unwrap()[0].plan_id, "p-probe");
        drop(db);
        cleanup_database(&path);
    }

    /// A journal write that fails - the in-process stand-in for dying mid-write - must leave the
    /// plan non-terminal, where `recoverable_plans` still finds it after restart.
    #[test]
    fn failed_journal_write_leaves_the_plan_recoverable() {
        let (db, path) = temp_db();
        journal_plan(&db, "p-abort", "Executing");
        db.upsert_maintenance_execution(&MaintenanceExecutionRecord {
            plan_id: "p-abort".into(),
            domain: "Cleanup".into(),
            stage: "Executing".into(),
            ..Default::default()
        })
        .unwrap();
        db.connection
            .lock()
            .unwrap()
            .execute_batch(
                "CREATE TRIGGER journal_fault BEFORE UPDATE ON maintenance_executions
                 WHEN NEW.stage='Failed' BEGIN SELECT RAISE(ABORT,'injected journal fault'); END;",
            )
            .unwrap();
        let (record, recovery) = failed_journal("p-abort");
        assert!(
            db.transition_plan_with_journal(
                "p-abort",
                "Executing",
                "Failed",
                "cleanup failed",
                5,
                PlanJournal::Maintenance(&record, Some(&recovery)),
            )
            .is_err()
        );
        assert_eq!(db.get_plan("p-abort").unwrap().unwrap().state, "Executing");
        assert_eq!(db.plans_in_states(&["Executing"]).unwrap().len(), 1);
        assert_eq!(db.event_count().unwrap(), 1);
        assert!(db.recovery_records(10).unwrap().is_empty());
        drop(db);
        cleanup_database(&path);
    }

    #[test]
    fn missed_state_cas_writes_no_journal() {
        let (db, path) = temp_db();
        journal_plan(&db, "p-cas", "Verifying");
        let (_, recovery) = failed_journal("p-cas");
        let execution = ExecutionRecord {
            plan_id: "p-cas".into(),
            stage: "FailedAfterMutation".into(),
            ..Default::default()
        };
        assert!(
            !db.transition_plan_with_journal(
                "p-cas",
                "Executing",
                "Failed",
                "stale expectation",
                5,
                PlanJournal::Driver(&execution, Some(&recovery)),
            )
            .unwrap()
        );
        assert_eq!(db.get_plan("p-cas").unwrap().unwrap().state, "Verifying");
        assert!(db.get_execution("p-cas").unwrap().is_none());
        assert!(db.recovery_records(10).unwrap().is_empty());
        drop(db);
        cleanup_database(&path);
    }

    #[test]
    fn execution_and_checkpoint_are_durable() {
        let (db, path) = temp_db();
        let plan = PlanRecord {
            id: "p".into(),
            title: "Driver plan".into(),
            state: "AwaitingAuthorization".into(),
            digest: "d".into(),
            risk: "Amber".into(),
            immutable_json: "{}".into(),
            created_unix_ms: 1,
            updated_unix_ms: 1,
            owner_principal_key: "test-owner".into(),
        };
        db.insert_plan(&plan, "driver_plan_created").unwrap();
        let execution = ExecutionRecord {
            plan_id: "p".into(),
            stage: "Preflight".into(),
            started_unix_ms: 2,
            updated_unix_ms: 2,
            ..ExecutionRecord::default()
        };
        db.upsert_execution(&execution).unwrap();
        db.add_checkpoint("p", "", "preflight_started", "{}", 3)
            .unwrap();
        drop(db);
        let reopened = Database::open(&path).unwrap();
        assert_eq!(
            reopened.get_execution("p").unwrap().unwrap().stage,
            "Preflight"
        );
        assert_eq!(
            reopened.checkpoints("p", 10).unwrap()[0].checkpoint,
            "preflight_started"
        );
        drop(reopened);
        cleanup_database(&path);
    }

    #[test]
    fn generic_maintenance_execution_and_items_are_durable() {
        let (db, path) = temp_db();
        let plan = PlanRecord {
            id: "phase4".into(),
            title: "Repair".into(),
            state: "Executing".into(),
            digest: "d4".into(),
            risk: "Amber".into(),
            immutable_json: "{}".into(),
            created_unix_ms: 1,
            updated_unix_ms: 1,
            owner_principal_key: "test-owner".into(),
        };
        db.insert_plan(&plan, "phase4_plan_created").unwrap();
        db.upsert_maintenance_execution(&MaintenanceExecutionRecord {
            plan_id: "phase4".into(),
            domain: "SystemRepair".into(),
            stage: "Executing".into(),
            progress_known: true,
            overall_percent: 50,
            mutation_started: true,
            started_unix_ms: 2,
            updated_unix_ms: 3,
            ..Default::default()
        })
        .unwrap();
        db.upsert_maintenance_item(&MaintenanceItemRecord {
            plan_id: "phase4".into(),
            item_id: "dism".into(),
            kind: "DISM RestoreHealth".into(),
            stage: "Completed".into(),
            result_code: "ExitCode0".into(),
            bytes_affected: 0,
            detail: "ok".into(),
            updated_unix_ms: 4,
        })
        .unwrap();
        drop(db);
        let reopened = Database::open(&path).unwrap();
        let execution = reopened
            .get_maintenance_execution("phase4")
            .unwrap()
            .unwrap();
        assert_eq!(execution.domain, "SystemRepair");
        assert!(execution.mutation_started);
        let items = reopened.maintenance_items("phase4").unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].item_id, "dism");
        drop(reopened);
        cleanup_database(&path);
    }

    #[test]
    fn startup_change_record_is_durable_and_queryable() {
        let (db, path) = temp_db();
        let plan = PlanRecord {
            id: "startup-plan".into(),
            title: "Optimize startup".into(),
            state: "Executing".into(),
            digest: "startup-digest".into(),
            risk: "Amber".into(),
            immutable_json: "{}".into(),
            created_unix_ms: 1,
            updated_unix_ms: 1,
            owner_principal_key: "test-owner".into(),
        };
        db.insert_plan(&plan, "startup_plan_created").unwrap();
        let record = StartupChangeRecord {
            change_id: "change-1".into(),
            origin_change_id: "change-1".into(),
            plan_id: plan.id.clone(),
            item_id: "item-1".into(),
            kind: "RegistryRun".into(),
            display_name: "Vendor Agent".into(),
            direction: "Disable".into(),
            original_json: r#"{"exists":true}"#.into(),
            applied_json: r#"{"exists":false}"#.into(),
            state: "Prepared".into(),
            detail: "frozen before mutation".into(),
            created_unix_ms: 2,
            updated_unix_ms: 2,
            restored_unix_ms: None,
        };
        db.upsert_startup_change(&record).unwrap();
        drop(db);

        let reopened = Database::open(&path).unwrap();
        let loaded = reopened.get_startup_change("change-1").unwrap().unwrap();
        assert_eq!(loaded.original_json, record.original_json);
        assert_eq!(loaded.applied_json, record.applied_json);
        assert_eq!(loaded.state, "Prepared");
        let pending = reopened.startup_changes_in_states(&["Prepared"]).unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].change_id, "change-1");
        drop(reopened);
        cleanup_database(&path);
    }

    #[test]
    fn diagnostic_snapshot_is_durable() {
        let (db, path) = temp_db();
        db.save_diagnostic_snapshot(&DiagnosticSnapshotRecord {
            snapshot_id: "diag-1".into(),
            owner_principal_key: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                .into(),
            state: "Ready".into(),
            collected_unix_ms: 9,
            warning_count: 1,
            snapshot_json: r#"{"cards":[]}"#.into(),
        })
        .unwrap();
        drop(db);
        let reopened = Database::open(&path).unwrap();
        let row = reopened
            .latest_diagnostic_snapshot_for_owner(
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            )
            .unwrap()
            .unwrap();
        assert_eq!(row.snapshot_id, "diag-1");
        assert_eq!(row.warning_count, 1);
        drop(reopened);
        cleanup_database(&path);
    }

    #[test]
    fn phase17_intelligence_history_findings_and_sealed_plan_are_durable() {
        let (db, path) = temp_db();
        let owner = "phase17-owner";
        db.save_intelligence_scan(&IntelligenceScanRecord {
            scan_id: "scan-1".into(),
            owner_principal_key: owner.into(),
            state: "Completed".into(),
            status: "AttentionRecommended".into(),
            started_unix_ms: 10,
            completed_unix_ms: 20,
            machine_state_fingerprint: "fingerprint".into(),
            rule_engine_version: "phase17-rules-v1".into(),
            app_version: "0.1".into(),
            finding_count: 1,
            unavailable_collector_count: 0,
            snapshot_json: r#"{"scanId":"scan-1"}"#.into(),
        })
        .unwrap();
        db.upsert_intelligence_finding(&IntelligenceFindingRecord {
            owner_principal_key: owner.into(),
            finding_id: "finding-1".into(),
            finding_code: "DRIVER_MISSING".into(),
            first_observed_unix_ms: 10,
            last_observed_unix_ms: 20,
            lifecycle: "New".into(),
            severity: "High".into(),
            confidence: "Confirmed".into(),
            verification_status: "ConfirmedCurrent".into(),
            resolved_at_unix_ms: None,
            resolution_scan_id: String::new(),
            resolution_reason_key: String::new(),
            finding_json: r#"{"id":"finding-1"}"#.into(),
        })
        .unwrap();
        db.save_intelligence_remediation_plan(
            owner,
            "plan-1",
            "scan-1",
            "digest",
            r#"{"immutable":true}"#,
            21,
        )
        .unwrap();
        db.upsert_intelligence_override(&IntelligenceOverrideRecord {
            owner_principal_key: owner.into(),
            override_id: "override-1".into(),
            scope_kind: "finding".into(),
            scope_value_hash: "hash".into(),
            behavior: "Ignore".into(),
            expires_unix_ms: None,
            created_unix_ms: 22,
            updated_unix_ms: 22,
        })
        .unwrap();
        let stored = db.intelligence_findings_for_owner(owner).unwrap();
        assert_eq!(stored[0].lifecycle, "New");
        assert_eq!(stored[0].verification_status, "ConfirmedCurrent");
        drop(db);

        let reopened = Database::open(&path).unwrap();
        let scans = reopened.intelligence_scans_for_owner(owner, 10).unwrap();
        assert_eq!(scans.len(), 1);
        assert_eq!(scans[0].scan_id, "scan-1");
        assert_eq!(
            reopened.intelligence_findings_for_owner(owner).unwrap()[0].first_observed_unix_ms,
            10
        );
        assert_eq!(
            reopened
                .intelligence_overrides_for_owner(owner, 30)
                .unwrap()[0]
                .behavior,
            "Ignore"
        );
        reopened
            .delete_intelligence_override(owner, "override-1")
            .unwrap();
        assert!(
            reopened
                .intelligence_overrides_for_owner(owner, 30)
                .unwrap()
                .is_empty()
        );
        drop(reopened);
        cleanup_database(&path);
    }

    #[test]
    fn recent_verified_driver_changes_are_principal_scoped() {
        let (db, path) = temp_db();
        let plan = |id: &str, owner: &str, digest: &str| PlanRecord {
            id: id.into(),
            title: "driver plan".into(),
            state: "Completed".into(),
            digest: digest.into(),
            risk: "Sensitive".into(),
            immutable_json: "{}".into(),
            created_unix_ms: 100,
            updated_unix_ms: 200,
            owner_principal_key: owner.into(),
        };
        db.insert_plan(&plan("plan-a", "owner-a", "digest-a"), "test")
            .unwrap();
        db.insert_plan(&plan("plan-b", "owner-b", "digest-b"), "test")
            .unwrap();
        for (plan_id, candidate_id, verified, updated) in [
            ("plan-a", "candidate-a", true, 250_i64),
            ("plan-a", "candidate-old", true, 50_i64),
            ("plan-b", "candidate-b", true, 260_i64),
            ("plan-a", "candidate-unverified", false, 270_i64),
        ] {
            db.upsert_install_item(&InstallItemRecord {
                plan_id: plan_id.into(),
                candidate_id: candidate_id.into(),
                update_id: format!("update-{candidate_id}"),
                revision: 1,
                instance_id: format!("PCI\\{candidate_id}"),
                title: candidate_id.into(),
                stage: "Verified".into(),
                progress_known: true,
                progress_percent: 100,
                result_code: "orcSucceeded".into(),
                hresult: 0,
                reboot_required: false,
                verified,
                before_driver_json: "{}".into(),
                after_driver_json: r#"{"version":"2.0"}"#.into(),
                before_problem_code: 0,
                after_problem_code: 0,
                backup_path: String::new(),
                detail: String::new(),
                updated_unix_ms: updated,
            })
            .unwrap();
        }
        let rows = db
            .recent_verified_driver_install_items_for_owner("owner-a", 100, 10)
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].candidate_id, "candidate-a");
        assert!(
            db.recent_verified_driver_install_items_for_owner("owner-c", 0, 10)
                .unwrap()
                .is_empty()
        );
        drop(db);
        cleanup_database(&path);
    }

    #[test]
    fn scheduler_cadence_is_durable_and_principal_scoped() {
        let (db, path) = temp_db();
        let record = SchedulerRunRecord {
            owner_principal_key: "owner-a".into(),
            workload: "HardwareTelemetry".into(),
            failure_count: 2,
            next_eligible_unix_ms: 12345,
            last_outcome: "failed".into(),
            last_completed_unix_ms: Some(9000),
            updated_unix_ms: 10000,
        };
        db.upsert_scheduler_run(&record).unwrap();
        assert!(
            db.scheduler_run("owner-b", "HardwareTelemetry")
                .unwrap()
                .is_none()
        );
        drop(db);
        let reopened = Database::open(&path).unwrap();
        let loaded = reopened
            .scheduler_run("owner-a", "HardwareTelemetry")
            .unwrap()
            .unwrap();
        assert_eq!(loaded, record);
        drop(reopened);
        cleanup_database(&path);
    }

    fn cleanup_database(path: &Path) {
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(path.with_extension("db-wal"));
        let _ = std::fs::remove_file(path.with_extension("db-shm"));
    }
}

// Phase 29 (T1): EXPORT_V1 — canonical hash-chained, optionally signed journal export.
// Owner-review waiver: ed25519-dalek is the ONLY new dependency this phase; it performs
// local Ed25519 signing/verification for export digests and does no network I/O.
pub mod export;
