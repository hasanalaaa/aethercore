use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{mpsc, Arc, Mutex},
    thread,
};

use aethercore_collector_runtime::CancellationToken;
use aethercore_persistence::{Database, IntelligenceFindingRecord, IntelligenceOverrideRecord, IntelligenceScanRecord};
use chrono::Utc;
use thiserror::Error;
use uuid::Uuid;

use crate::{
    fingerprint::machine_state_fingerprint,
    lifecycle,
    model::*,
    normalize,
    rules,
    run_ownership::{RunIdentity, RunOwnership},
    sources::{DeepScanBackend, SourceError},
};

const TOTAL_COLLECTOR_TASKS: u32 = 7;

#[derive(Debug, Error)]
pub enum IntelligenceError {
    #[error("deep scan already running")]
    Busy,
    #[error("deep scan state belongs to another Windows principal")]
    Ownership,
    #[error("unknown deep scan")]
    UnknownScan,
    #[error("deep scan persistence failed: {0}")]
    Persistence(String),
    #[error("deep scan worker failed: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, IntelligenceError>;

#[derive(Clone)]
pub struct DeepScanCoordinator {
    inner: Arc<Inner>,
}

struct Inner {
    backend: Arc<dyn DeepScanBackend>,
    db: Arc<Database>,
    snapshot: Mutex<DeepScanSnapshot>,
    owner: Mutex<String>,
    run_ownership: Mutex<RunOwnership>,
    app_version: String,
}

enum Collected {
    Driver(aethercore_driver_hub::DriverHubSnapshot),
    Repair(aethercore_system_repair::RepairAssessment),
    Diagnostics(aethercore_diagnostic_engine::DiagnosticsSnapshot),
    Startup(aethercore_startup_manager::StartupSnapshot),
    Cleanup(aethercore_cleaner::CleanupSnapshot),
}

struct TaskResult {
    id: &'static str,
    stages: Vec<ScanStage>,
    result: std::result::Result<Collected, SourceError>,
    started_unix_ms: i64,
    completed_unix_ms: i64,
}

impl DeepScanCoordinator {
    pub fn new(
        db: Arc<Database>,
        backend: Arc<dyn DeepScanBackend>,
        app_version: impl Into<String>,
    ) -> Self {
        let app_version = app_version.into();
        let snapshot = DeepScanSnapshot {
            app_version: app_version.clone(),
            ..Default::default()
        };
        Self {
            inner: Arc::new(Inner {
                backend,
                db,
                snapshot: Mutex::new(snapshot),
                owner: Mutex::new(String::new()),
                run_ownership: Mutex::new(RunOwnership::default()),
                app_version,
            }),
        }
    }

    pub fn snapshot_for_owner(&self, owner: &str) -> Result<DeepScanSnapshot> {
        let current = self.inner.owner.lock().unwrap_or_else(|p| p.into_inner());
        if !current.is_empty() && current.as_str() != owner {
            return Err(IntelligenceError::Ownership);
        }
        Ok(self
            .inner
            .snapshot
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone())
    }

    pub fn start(&self, owner: &str) -> Result<DeepScanSnapshot> {
        let now = Utc::now().timestamp_millis();
        let scan_id = Uuid::new_v4().to_string();
        let token = CancellationToken::new();
        let identity;
        {
            let mut current_owner = self.inner.owner.lock().unwrap_or_else(|p| p.into_inner());
            let mut ownership = self.inner.run_ownership.lock().unwrap_or_else(|p| p.into_inner());
            let mut snapshot = self.inner.snapshot.lock().unwrap_or_else(|p| p.into_inner());
            if snapshot.state == ScanState::Scanning {
                return Err(IntelligenceError::Busy);
            }
            identity = ownership.install(scan_id.clone(), now, token.clone());
            *current_owner = owner.to_owned();
            *snapshot = DeepScanSnapshot {
                scan_id: scan_id.clone(),
                state: ScanState::Scanning,
                started_unix_ms: now,
                progress: ScanProgress {
                    total_weight: total_weight(),
                    completed_weight: 0,
                    completed_tasks: 0,
                    total_tasks: TOTAL_COLLECTOR_TASKS,
                    active_tasks: 0,
                    skipped_tasks: 0,
                    failed_tasks: 0,
                    unavailable_tasks: 0,
                    current_stage_key: ScanStage::SystemIdentity.message_key().into(),
                },
                app_version: self.inner.app_version.clone(),
                ..Default::default()
            };
        }

        let inner = self.inner.clone();
        let owner = owner.to_owned();
        let worker_identity = identity.clone();
        if let Err(error) = thread::Builder::new()
            .name("aether-deep-scan".into())
            .spawn(move || run(inner, owner, token, worker_identity))
        {
            if owns_run(&self.inner, &identity) {
                let mut snapshot = self.inner.snapshot.lock().unwrap_or_else(|p| p.into_inner());
                if snapshot.scan_id == identity.scan_id {
                    snapshot.state = ScanState::Failed;
                    snapshot.completed_unix_ms = Utc::now().timestamp_millis();
                    snapshot.warnings.push(format!("deep scan worker unavailable: {error}"));
                }
            }
            clear_run_if_owner(&self.inner, &identity);
            return Err(IntelligenceError::Internal(error.to_string()));
        }

        self.snapshot_for_owner(owner.as_str())
    }

    pub fn cancel(&self, owner: &str, scan_id: &str) -> Result<DeepScanSnapshot> {
        let current = self.snapshot_for_owner(owner)?;
        if current.scan_id != scan_id {
            return Err(IntelligenceError::UnknownScan);
        }
        if current.state != ScanState::Scanning {
            return Ok(current);
        }
        let cancelled = self
            .inner
            .run_ownership
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .cancel_if_current(scan_id);
        if !cancelled {
            return Err(IntelligenceError::UnknownScan);
        }
        self.snapshot_for_owner(owner)
    }

    pub fn set_finding_ignored(&self, owner: &str, finding_id: &str, ignored: bool) -> Result<()> {
        let now = Utc::now().timestamp_millis();
        let override_id = stable_id("override", &format!("finding|{finding_id}"));
        if ignored {
            self.inner.db.upsert_intelligence_override(&IntelligenceOverrideRecord {
                owner_principal_key: owner.into(),
                override_id,
                scope_kind: "finding".into(),
                scope_value_hash: private_id(finding_id),
                behavior: "Ignore".into(),
                expires_unix_ms: None,
                created_unix_ms: now,
                updated_unix_ms: now,
            }).map_err(|e| IntelligenceError::Persistence(e.to_string()))
        } else {
            self.inner.db.delete_intelligence_override(owner, &override_id)
                .map_err(|e| IntelligenceError::Persistence(e.to_string()))
        }
    }

    pub fn record_stream_event(&self, owner: &str, scan_id: &str) -> Result<()> {
        let current_owner = self.inner.owner.lock().unwrap_or_else(|p| p.into_inner());
        if current_owner.as_str() != owner {
            return Err(IntelligenceError::Ownership);
        }
        let mut snapshot = self.inner.snapshot.lock().unwrap_or_else(|p| p.into_inner());
        if snapshot.scan_id != scan_id {
            return Err(IntelligenceError::UnknownScan);
        }
        snapshot.metrics.streamed_event_count = snapshot.metrics.streamed_event_count.saturating_add(1);
        Ok(())
    }

    pub fn history(&self, owner: &str, limit: usize) -> Result<Vec<DeepScanHistoryEntry>> {
        self.inner
            .db
            .intelligence_scans_for_owner(owner, limit)
            .map_err(|e| IntelligenceError::Persistence(e.to_string()))
            .map(|rows| {
                rows.into_iter()
                    .filter_map(|record| {
                        serde_json::from_str::<DeepScanSnapshot>(&record.snapshot_json)
                            .ok()
                            .map(|snapshot| DeepScanHistoryEntry {
                                scan_id: snapshot.scan_id,
                                state: snapshot.state,
                                status: snapshot.status,
                                completed_unix_ms: snapshot.completed_unix_ms,
                                duration_ms: snapshot
                                    .completed_unix_ms
                                    .saturating_sub(snapshot.started_unix_ms),
                                finding_count: snapshot.findings.len() as u32,
                                unavailable_collector_count: snapshot
                                    .collectors
                                    .iter()
                                    .filter(|collector| is_unavailable(collector.state))
                                    .count() as u32,
                                machine_state_fingerprint: snapshot.machine_state_fingerprint,
                            })
                    })
                    .collect()
            })
    }

    pub fn seal_remediation_plan(
        &self,
        owner: &str,
        scan_id: &str,
        selected_action_ids: &[String],
    ) -> Result<RemediationPlan> {
        let snapshot = self.snapshot_for_owner(owner)?;
        if snapshot.scan_id != scan_id {
            return Err(IntelligenceError::UnknownScan);
        }
        if !matches!(snapshot.state, ScanState::Completed | ScanState::Partial) {
            return Err(IntelligenceError::Internal(
                "remediation plans can only be sealed from a completed scan".into(),
            ));
        }

        let selected: BTreeSet<&String> = selected_action_ids.iter().collect();
        if selected.len() != selected_action_ids.len() {
            return Err(IntelligenceError::Internal("remediation selection contains duplicate action IDs".into()));
        }
        let actions: Vec<_> = snapshot
            .remediation_candidates
            .into_iter()
            .filter(|action| selected.contains(&action.action_id))
            .collect();
        if actions.len() != selected.len() {
            return Err(IntelligenceError::Internal("remediation selection contains an unknown or stale action ID".into()));
        }
        let plan = RemediationPlan::seal(scan_id, Utc::now().timestamp_millis(), actions);
        let plan_json = serde_json::to_string(&plan)
            .map_err(|e| IntelligenceError::Persistence(e.to_string()))?;
        self.inner
            .db
            .save_intelligence_remediation_plan(
                owner,
                &plan.plan_id,
                scan_id,
                &plan.digest,
                &plan_json,
                plan.created_unix_ms,
            )
            .map_err(|e| IntelligenceError::Persistence(e.to_string()))?;
        Ok(plan)
    }
}

fn run(inner: Arc<Inner>, owner: String, token: CancellationToken, identity: RunIdentity) {
    let mut facts = Vec::new();
    let mut statuses = Vec::new();
    let mut warnings = Vec::new();
    let mut supplemental_scopes = BTreeMap::new();

    if !owns_run(&inner, &identity) {
        return;
    }

    // Batch one is deliberately capped at three expensive read families. The global
    // read budget is four, preserving one slot for interactive foreground work.
    let batch_one = [
        (
            "drivers",
            vec![
                ScanStage::SystemIdentity,
                ScanStage::HardwareInventory,
                ScanStage::DriverInventory,
            ],
        ),
        ("windows", vec![ScanStage::WindowsIntegrity]),
        (
            "diagnostics",
            vec![
                ScanStage::StorageHealth,
                ScanStage::MemoryPressure,
                ScanStage::HardwareErrors,
                ScanStage::CrashDiagnostics,
            ],
        ),
    ];
    execute_batch(
        &inner,
        &owner,
        &token,
        &identity,
        &batch_one,
        &mut facts,
        &mut statuses,
        &mut warnings,
    );

    if !token.is_cancelled() && owns_run(&inner, &identity) {
        let now = Utc::now().timestamp_millis();
        let since = now.saturating_sub(30_i64 * 24 * 60 * 60 * 1000);
        match inner.db.recent_verified_driver_install_items_for_owner(&owner, since, 100) {
            Ok(items) => {
                facts.extend(items.iter().map(normalize::driver_change));
                supplemental_scopes.insert("driver-history".into(), CollectorState::Completed);
            }
            Err(error) => {
                supplemental_scopes.insert("driver-history".into(), CollectorState::Unavailable);
                warnings.push(format!("persistence driver-change history unavailable: {error}"));
            }
        }
    }

    if !token.is_cancelled() && owns_run(&inner, &identity) {
        let batch_two = [
            ("startup", vec![ScanStage::StartupFootprint]),
            ("cleanup", vec![ScanStage::CleanupOpportunities]),
        ];
        execute_batch(
            &inner,
            &owner,
            &token,
            &identity,
            &batch_two,
            &mut facts,
            &mut statuses,
            &mut warnings,
        );
    }

    if !token.is_cancelled() && owns_run(&inner, &identity) {
        publish_active(
            &inner,
            &identity,
            ScanStage::UpdateState,
            1,
            &facts,
            &statuses,
            &warnings,
        );
        let started = Utc::now().timestamp_millis();
        let update = inner.backend.update(&owner);
        facts.push(normalize::update(&update, started));
        statuses.push(CollectorStatus {
            id: "updates".into(),
            state: CollectorState::Completed,
            stage_keys: vec![ScanStage::UpdateState.message_key().into()],
            started_unix_ms: started,
            completed_unix_ms: Utc::now().timestamp_millis(),
            detail: String::new(),
        });
        publish_progress(
            &inner,
            &identity,
            &facts,
            &statuses,
            &warnings,
            ScanStage::RecoveryReadiness,
        );
    }

    if !token.is_cancelled() && owns_run(&inner, &identity) {
        publish_active(
            &inner,
            &identity,
            ScanStage::RecoveryReadiness,
            1,
            &facts,
            &statuses,
            &warnings,
        );
        let started = Utc::now().timestamp_millis();
        match inner.db.recent_recovery_event_count_for_owner(&owner) {
            Ok(recovery_count) => {
                facts.push(SystemFact::new(
                    Domain::Recovery,
                    "operation-journal",
                    ResourceRef::global("recovery", "recovery-readiness", "Recovery readiness"),
                    started,
                    Freshness::Current,
                    Confidence::Confirmed,
                    FactPayload::RecoveryReadiness {
                        active_recovery_records: recovery_count,
                    },
                    EvidenceKind::JournalEvent,
                    format!("recoveryRecords={recovery_count}"),
                ));
                statuses.push(CollectorStatus {
                    id: "recovery".into(),
                    state: CollectorState::Completed,
                    stage_keys: vec![ScanStage::RecoveryReadiness.message_key().into()],
                    started_unix_ms: started,
                    completed_unix_ms: Utc::now().timestamp_millis(),
                    detail: String::new(),
                });
            }
            Err(error) => {
                let detail = bounded_text(error.to_string(), 1024);
                facts.push(normalize::limitation(
                    "recovery",
                    CollectorState::Unavailable,
                    &detail,
                    Utc::now().timestamp_millis(),
                ));
                statuses.push(CollectorStatus {
                    id: "recovery".into(),
                    state: CollectorState::Unavailable,
                    stage_keys: vec![ScanStage::RecoveryReadiness.message_key().into()],
                    started_unix_ms: started,
                    completed_unix_ms: Utc::now().timestamp_millis(),
                    detail: detail.clone(),
                });
                warnings.push(format!("recovery: {detail}"));
            }
        }
        publish_progress(
            &inner,
            &identity,
            &facts,
            &statuses,
            &warnings,
            ScanStage::Correlation,
        );
    }

    if token.is_cancelled() {
        complete(
            &inner,
            &owner,
            &identity,
            &supplemental_scopes,
            facts,
            statuses,
            warnings,
            ScanState::Cancelled,
        );
        return;
    }
    if !owns_run(&inner, &identity) {
        return;
    }

    // Correlation and recommendation synthesis are deterministic CPU-only work. They
    // become visible as real stages and complete only after their computation finishes.
    publish_active(
        &inner,
        &identity,
        ScanStage::Correlation,
        1,
        &facts,
        &statuses,
        &warnings,
    );
    let evaluated_at = Utc::now().timestamp_millis();
    let current_findings = rules::evaluate(&facts, evaluated_at);
    let lifecycle = lifecycle::reconcile(
        &inner.db,
        &owner,
        &identity.scan_id,
        evaluated_at,
        &facts,
        &statuses,
        &supplemental_scopes,
        current_findings,
    );
    warnings.extend(lifecycle.warnings);
    let findings = lifecycle.visible_findings;
    let persisted_findings = lifecycle.persisted_findings;
    publish_findings(
        &inner,
        &identity,
        &facts,
        &statuses,
        &warnings,
        &findings,
        ScanStage::RecommendationSynthesis,
        ScanStage::Correlation.weight(),
    );

    let remediation = rules::remediation_candidates(&findings);
    let mut state = if statuses.iter().all(|status| {
        matches!(
            status.state,
            CollectorState::Completed | CollectorState::CompletedWithWarnings
        )
    }) {
        if statuses
            .iter()
            .any(|status| status.state == CollectorState::CompletedWithWarnings)
        {
            ScanState::Partial
        } else {
            ScanState::Completed
        }
    } else if statuses.iter().any(|status| {
        matches!(
            status.state,
            CollectorState::Completed | CollectorState::CompletedWithWarnings
        )
    }) {
        ScanState::Partial
    } else {
        ScanState::Failed
    };
    if state == ScanState::Completed
        && warnings
            .iter()
            .any(|warning| warning.starts_with("persistence "))
    {
        state = ScanState::Partial;
    }

    let completed = Utc::now().timestamp_millis();
    let metrics = final_metrics(
        &inner,
        &identity,
        completed,
        &statuses,
        &facts,
        &findings,
    );
    let snapshot = DeepScanSnapshot {
        scan_id: identity.scan_id.clone(),
        state,
        status: status_from(&findings),
        started_unix_ms: identity.started_unix_ms,
        completed_unix_ms: completed,
        progress: ScanProgress {
            total_weight: total_weight(),
            completed_weight: total_weight(),
            completed_tasks: completed_task_count(&statuses),
            total_tasks: TOTAL_COLLECTOR_TASKS,
            active_tasks: 0,
            skipped_tasks: TOTAL_COLLECTOR_TASKS.saturating_sub(statuses.len() as u32),
            failed_tasks: statuses.iter().filter(|status| is_failed(status.state)).count() as u32,
            unavailable_tasks: statuses
                .iter()
                .filter(|status| is_unavailable(status.state))
                .count() as u32,
            current_stage_key: ScanStage::RecommendationSynthesis.message_key().into(),
        },
        facts_count: facts.len() as u32,
        findings: findings.clone(),
        remediation_candidates: remediation,
        collectors: statuses,
        warnings,
        summary: summarize(&findings, &facts),
        metrics,
        machine_state_fingerprint: machine_state_fingerprint(&facts),
        rule_engine_version: RULE_ENGINE_VERSION.into(),
        app_version: inner.app_version.clone(),
    };

    let snapshot = persist(&inner.db, &owner, snapshot, &persisted_findings);
    set_snapshot_if_owner(&inner, &identity, snapshot);
    clear_run_if_owner(&inner, &identity);
}

fn execute_batch(
    inner: &Arc<Inner>,
    owner: &str,
    token: &CancellationToken,
    identity: &RunIdentity,
    tasks: &[(&'static str, Vec<ScanStage>)],
    facts: &mut Vec<SystemFact>,
    statuses: &mut Vec<CollectorStatus>,
    warnings: &mut Vec<String>,
) {
    if token.is_cancelled() || !owns_run(inner, identity) {
        return;
    }

    let first_stage = tasks
        .first()
        .and_then(|(_, stages)| stages.first())
        .copied()
        .unwrap_or(ScanStage::SystemIdentity);
    publish_active(inner, identity, first_stage, tasks.len() as u32, facts, statuses, warnings);

    let (tx, rx) = mpsc::channel();
    for (id, stages) in tasks {
        if token.is_cancelled() {
            break;
        }
        let worker_tx = tx.clone();
        let backend = inner.backend.clone();
        let owner = owner.to_owned();
        let child = token.child();
        let id = *id;
        let stages = stages.clone();
        let failure_stages = stages.clone();
        let spawn_started_unix_ms = Utc::now().timestamp_millis();
        if let Err(error) = thread::Builder::new()
            .name(format!("aether-deep-scan-{id}"))
            .spawn(move || {
                let started_unix_ms = Utc::now().timestamp_millis();
                let result = match id {
                    "drivers" => backend.driver(&owner, &child).map(Collected::Driver),
                    "windows" => backend.repair(&owner, &child).map(Collected::Repair),
                    "diagnostics" => backend.diagnostics(&owner, &child).map(Collected::Diagnostics),
                    "startup" => backend.startup(&owner, &child).map(Collected::Startup),
                    "cleanup" => backend.cleanup(&owner, &child).map(Collected::Cleanup),
                    _ => Err(SourceError::Unavailable("unknown collector".into())),
                };
                let _ = worker_tx.send(TaskResult {
                    id,
                    stages,
                    result,
                    started_unix_ms,
                    completed_unix_ms: Utc::now().timestamp_millis(),
                });
            })
        {
            let _ = tx.send(TaskResult {
                id,
                stages: failure_stages,
                result: Err(SourceError::Unavailable(format!("collector worker unavailable: {error}"))),
                started_unix_ms: spawn_started_unix_ms,
                completed_unix_ms: Utc::now().timestamp_millis(),
            });
        }
    }
    drop(tx);

    for task in rx {
        if !owns_run(inner, identity) {
            break;
        }
        consume_task(task, facts, statuses, warnings);
        let next_stage = pending_stage(tasks, statuses).unwrap_or(ScanStage::Correlation);
        publish_progress(inner, identity, facts, statuses, warnings, next_stage);
        if token.is_cancelled() {
            break;
        }
    }
}

fn consume_task(
    task: TaskResult,
    facts: &mut Vec<SystemFact>,
    statuses: &mut Vec<CollectorStatus>,
    warnings: &mut Vec<String>,
) {
    let (state, detail) = match &task.result {
        Ok(value) => collected_state(value),
        Err(SourceError::Cancelled) => (CollectorState::Cancelled, "collector cancelled".into()),
        Err(SourceError::Budget) => (
            CollectorState::Unavailable,
            "read-only resource budget unavailable".into(),
        ),
        Err(SourceError::TimedOut) => (
            CollectorState::TimedOut,
            "collector deadline exceeded".into(),
        ),
        Err(SourceError::Unavailable(value)) => {
            (CollectorState::Unavailable, bounded_text(value.clone(), 1024))
        }
    };

    statuses.push(CollectorStatus {
        id: task.id.into(),
        state,
        stage_keys: task
            .stages
            .iter()
            .map(|stage| stage.message_key().into())
            .collect(),
        started_unix_ms: task.started_unix_ms,
        completed_unix_ms: task.completed_unix_ms,
        detail: detail.clone(),
    });

    match task.result {
        Ok(Collected::Driver(value)) => facts.extend(normalize::driver(&value, task.completed_unix_ms)),
        Ok(Collected::Repair(value)) => facts.extend(normalize::repair(&value)),
        Ok(Collected::Diagnostics(value)) => facts.extend(normalize::diagnostics(&value)),
        Ok(Collected::Startup(value)) => facts.extend(normalize::startup(&value)),
        Ok(Collected::Cleanup(value)) => facts.extend(normalize::cleanup(&value)),
        Err(_) if state != CollectorState::Cancelled => {
            facts.push(normalize::limitation(
                task.id,
                state,
                &detail,
                task.completed_unix_ms,
            ));
            warnings.push(format!("{}: {detail}", task.id));
        }
        Err(_) => {}
    }

    if state == CollectorState::CompletedWithWarnings && !detail.is_empty() {
        warnings.push(format!("{}: {detail}", task.id));
    }
}

fn collected_state(value: &Collected) -> (CollectorState, String) {
    match value {
        Collected::Driver(snapshot) => match snapshot.state {
            aethercore_driver_hub::ScanState::Ready => (CollectorState::Completed, String::new()),
            aethercore_driver_hub::ScanState::Failed => (
                CollectorState::Failed,
                "driver discovery reported failure".into(),
            ),
            _ => (
                CollectorState::CompletedWithWarnings,
                "driver discovery ended in a non-final state".into(),
            ),
        },
        Collected::Repair(snapshot) => match snapshot.state {
            aethercore_system_repair::RepairAssessmentState::Ready => {
                (CollectorState::Completed, String::new())
            }
            aethercore_system_repair::RepairAssessmentState::Failed => (
                CollectorState::Failed,
                "Windows integrity assessment reported failure".into(),
            ),
            _ => (
                CollectorState::CompletedWithWarnings,
                "Windows integrity assessment ended in a non-final state".into(),
            ),
        },
        Collected::Diagnostics(snapshot) => match snapshot.state {
            aethercore_diagnostic_engine::ScanState::Ready => {
                (CollectorState::Completed, String::new())
            }
            aethercore_diagnostic_engine::ScanState::Partial => (
                CollectorState::CompletedWithWarnings,
                "some hardware diagnostics were unavailable".into(),
            ),
            aethercore_diagnostic_engine::ScanState::Failed => (
                CollectorState::Failed,
                "hardware diagnostics reported failure".into(),
            ),
            _ => (
                CollectorState::CompletedWithWarnings,
                "hardware diagnostics ended in a non-final state".into(),
            ),
        },
        Collected::Startup(snapshot) => match snapshot.state {
            aethercore_startup_manager::StartupScanState::Ready => {
                (CollectorState::Completed, String::new())
            }
            aethercore_startup_manager::StartupScanState::Failed => (
                CollectorState::Failed,
                "startup discovery reported failure".into(),
            ),
            _ => (
                CollectorState::CompletedWithWarnings,
                "startup discovery ended in a non-final state".into(),
            ),
        },
        Collected::Cleanup(snapshot) => match snapshot.state {
            aethercore_cleaner::CleanupScanState::Ready => {
                (CollectorState::Completed, String::new())
            }
            aethercore_cleaner::CleanupScanState::Failed => (
                CollectorState::Failed,
                "cleanup discovery reported failure".into(),
            ),
            _ => (
                CollectorState::CompletedWithWarnings,
                "cleanup discovery ended in a non-final state".into(),
            ),
        },
    }
}

fn pending_stage(
    tasks: &[(&'static str, Vec<ScanStage>)],
    statuses: &[CollectorStatus],
) -> Option<ScanStage> {
    tasks.iter().find_map(|(id, stages)| {
        if statuses.iter().any(|status| status.id == *id) {
            None
        } else {
            stages.first().copied()
        }
    })
}

fn publish_active(
    inner: &Inner,
    identity: &RunIdentity,
    stage: ScanStage,
    active_tasks: u32,
    facts: &[SystemFact],
    statuses: &[CollectorStatus],
    warnings: &[String],
) {
    mutate_snapshot_if_owner(inner, identity, |snapshot| {
        snapshot.progress = progress(statuses, stage, active_tasks, false);
        snapshot.metrics.peak_active_tasks = snapshot.metrics.peak_active_tasks.max(active_tasks);
        snapshot.facts_count = facts.len() as u32;
        snapshot.collectors = statuses.to_vec();
        snapshot.warnings = warnings.to_vec();
        publish_interim_intelligence(snapshot, facts);
    });
}

fn publish_progress(
    inner: &Inner,
    identity: &RunIdentity,
    facts: &[SystemFact],
    statuses: &[CollectorStatus],
    warnings: &[String],
    next_stage: ScanStage,
) {
    mutate_snapshot_if_owner(inner, identity, |snapshot| {
        snapshot.progress = progress(statuses, next_stage, 0, false);
        snapshot.facts_count = facts.len() as u32;
        snapshot.collectors = statuses.to_vec();
        snapshot.warnings = warnings.to_vec();
        publish_interim_intelligence(snapshot, facts);
    });
}

fn publish_interim_intelligence(snapshot: &mut DeepScanSnapshot, facts: &[SystemFact]) {
    let findings = rules::evaluate(facts, Utc::now().timestamp_millis());
    snapshot.remediation_candidates = rules::remediation_candidates(&findings);
    snapshot.summary = summarize(&findings, facts);
    snapshot.status = status_from(&findings);
    snapshot.findings = findings;
}

#[allow(clippy::too_many_arguments)]
fn publish_findings(
    inner: &Inner,
    identity: &RunIdentity,
    facts: &[SystemFact],
    statuses: &[CollectorStatus],
    warnings: &[String],
    findings: &[Finding],
    stage: ScanStage,
    extra_completed_weight: u32,
) {
    mutate_snapshot_if_owner(inner, identity, |snapshot| {
        let mut scan_progress = progress(statuses, stage, 1, false);
        scan_progress.completed_weight = scan_progress
            .completed_weight
            .saturating_add(extra_completed_weight)
            .min(total_weight());
        snapshot.progress = scan_progress;
        snapshot.facts_count = facts.len() as u32;
        snapshot.collectors = statuses.to_vec();
        snapshot.warnings = warnings.to_vec();
        snapshot.findings = findings.to_vec();
        snapshot.summary = summarize(findings, facts);
        snapshot.status = status_from(findings);
    });
}

fn complete(
    inner: &Inner,
    owner: &str,
    identity: &RunIdentity,
    supplemental_scopes: &BTreeMap<String, CollectorState>,
    facts: Vec<SystemFact>,
    statuses: Vec<CollectorStatus>,
    warnings: Vec<String>,
    state: ScanState,
) {
    if !owns_run(inner, identity) {
        return;
    }
    let completed = Utc::now().timestamp_millis();
    let current_findings = rules::evaluate(&facts, completed);
    let lifecycle = lifecycle::reconcile(
        &inner.db,
        owner,
        &identity.scan_id,
        completed,
        &facts,
        &statuses,
        supplemental_scopes,
        current_findings,
    );
    let findings = lifecycle.visible_findings;
    let persisted_findings = lifecycle.persisted_findings;
    let mut warnings = warnings;
    warnings.extend(lifecycle.warnings);
    let state = if state == ScanState::Completed
        && warnings.iter().any(|warning| warning.starts_with("persistence "))
    {
        ScanState::Partial
    } else {
        state
    };
    let metrics = final_metrics(inner, identity, completed, &statuses, &facts, &findings);
    let snapshot = DeepScanSnapshot {
        scan_id: identity.scan_id.clone(),
        state,
        status: status_from(&findings),
        started_unix_ms: identity.started_unix_ms,
        completed_unix_ms: completed,
        progress: ScanProgress {
            total_weight: total_weight(),
            completed_weight: completed_stage_weight(&statuses),
            completed_tasks: completed_task_count(&statuses),
            total_tasks: TOTAL_COLLECTOR_TASKS,
            active_tasks: 0,
            skipped_tasks: TOTAL_COLLECTOR_TASKS.saturating_sub(statuses.len() as u32),
            failed_tasks: statuses.iter().filter(|status| is_failed(status.state)).count() as u32,
            unavailable_tasks: statuses
                .iter()
                .filter(|status| is_unavailable(status.state))
                .count() as u32,
            current_stage_key: ScanStage::RecommendationSynthesis.message_key().into(),
        },
        facts_count: facts.len() as u32,
        findings: findings.clone(),
        remediation_candidates: rules::remediation_candidates(&findings),
        collectors: statuses,
        warnings,
        summary: summarize(&findings, &facts),
        metrics,
        machine_state_fingerprint: machine_state_fingerprint(&facts),
        rule_engine_version: RULE_ENGINE_VERSION.into(),
        app_version: inner.app_version.clone(),
    };
    let snapshot = persist(&inner.db, owner, snapshot, &persisted_findings);
    set_snapshot_if_owner(inner, identity, snapshot);
    clear_run_if_owner(inner, identity);
}

fn progress(
    statuses: &[CollectorStatus],
    stage: ScanStage,
    active_tasks: u32,
    include_intelligence_stages: bool,
) -> ScanProgress {
    let mut completed_weight = completed_stage_weight(statuses);
    if include_intelligence_stages {
        completed_weight = completed_weight
            .saturating_add(ScanStage::Correlation.weight())
            .saturating_add(ScanStage::RecommendationSynthesis.weight());
    }
    ScanProgress {
        total_weight: total_weight(),
        completed_weight: completed_weight.min(total_weight()),
        completed_tasks: completed_task_count(statuses),
        total_tasks: TOTAL_COLLECTOR_TASKS,
        active_tasks,
        skipped_tasks: 0,
        failed_tasks: statuses.iter().filter(|s| is_failed(s.state)).count() as u32,
        unavailable_tasks: statuses
            .iter()
            .filter(|s| is_unavailable(s.state))
            .count() as u32,
        current_stage_key: stage.message_key().into(),
    }
}

fn completed_stage_weight(statuses: &[CollectorStatus]) -> u32 {
    statuses
        .iter()
        .filter(|status| status.state != CollectorState::Cancelled)
        .flat_map(|status| status.stage_keys.iter())
        .filter_map(|key| {
            ALL_STAGES
                .iter()
                .find(|stage| stage.message_key() == key)
                .copied()
        })
        .map(ScanStage::weight)
        .sum()
}

fn completed_task_count(statuses: &[CollectorStatus]) -> u32 {
    statuses
        .iter()
        .filter(|status| {
            !matches!(
                status.state,
                CollectorState::Pending | CollectorState::Running | CollectorState::Cancelled
            )
        })
        .count() as u32
}

fn total_weight() -> u32 {
    ALL_STAGES.iter().copied().map(ScanStage::weight).sum()
}

fn owns_run(inner: &Inner, identity: &RunIdentity) -> bool {
    inner
        .run_ownership
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .owns(identity)
}

fn clear_run_if_owner(inner: &Inner, identity: &RunIdentity) -> bool {
    inner
        .run_ownership
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clear_if_owner(identity)
}

fn mutate_snapshot_if_owner(
    inner: &Inner,
    identity: &RunIdentity,
    mutator: impl FnOnce(&mut DeepScanSnapshot),
) -> bool {
    let ownership = inner
        .run_ownership
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    if !ownership.owns(identity) {
        return false;
    }
    let mut snapshot = inner.snapshot.lock().unwrap_or_else(|p| p.into_inner());
    if snapshot.scan_id != identity.scan_id || snapshot.state != ScanState::Scanning {
        return false;
    }
    mutator(&mut snapshot);
    true
}

fn set_snapshot_if_owner(
    inner: &Inner,
    identity: &RunIdentity,
    snapshot: DeepScanSnapshot,
) -> bool {
    let ownership = inner
        .run_ownership
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    if !ownership.owns(identity) {
        return false;
    }
    let mut current = inner.snapshot.lock().unwrap_or_else(|p| p.into_inner());
    if current.scan_id != identity.scan_id {
        return false;
    }
    *current = snapshot;
    true
}

fn final_metrics(
    inner: &Inner,
    identity: &RunIdentity,
    completed_unix_ms: i64,
    statuses: &[CollectorStatus],
    facts: &[SystemFact],
    findings: &[Finding],
) -> ScanMetrics {
    let previous = {
        let snapshot = inner.snapshot.lock().unwrap_or_else(|p| p.into_inner());
        if snapshot.scan_id == identity.scan_id {
            snapshot.metrics.clone()
        } else {
            ScanMetrics::default()
        }
    };
    let collector_duration_ms = statuses
        .iter()
        .map(|status| status.completed_unix_ms.saturating_sub(status.started_unix_ms).max(0))
        .sum();
    let normalized_payload_bytes_estimate = serde_json::to_vec(&(facts, findings))
        .map(|bytes| bytes.len() as u64)
        .unwrap_or(0);
    ScanMetrics {
        duration_ms: completed_unix_ms
            .saturating_sub(identity.started_unix_ms)
            .max(0),
        collector_duration_ms,
        peak_active_tasks: previous.peak_active_tasks,
        streamed_event_count: previous.streamed_event_count,
        persistence_write_count: previous.persistence_write_count,
        normalized_payload_bytes_estimate,
    }
}

fn status_from(findings: &[Finding]) -> SystemStatus {
    if findings
        .iter()
        .any(|finding| finding.severity == Severity::Critical)
    {
        SystemStatus::Critical
    } else if findings
        .iter()
        .any(|finding| finding.severity == Severity::High)
    {
        SystemStatus::ActionRequired
    } else if findings
        .iter()
        .any(|finding| matches!(finding.severity, Severity::Moderate | Severity::Low))
    {
        SystemStatus::AttentionRecommended
    } else {
        SystemStatus::Healthy
    }
}

fn summarize(findings: &[Finding], facts: &[SystemFact]) -> FindingSummary {
    let mut summary = FindingSummary::default();
    for finding in findings {
        match finding.severity {
            Severity::Critical => summary.critical += 1,
            Severity::High => summary.high += 1,
            Severity::Moderate => summary.moderate += 1,
            Severity::Low => summary.low += 1,
            Severity::Informational => summary.informational += 1,
        }
        if finding.remediation_available {
            let low_risk = matches!(
                finding.remediation_safety,
                Some(RemediationSafety::SafeAuto | RemediationSafety::SafeReview)
            );
            let optional = low_risk
                && matches!(finding.severity, Severity::Low | Severity::Informational);
            if optional {
                summary.optional_optimizations += 1;
            } else if !matches!(finding.severity, Severity::Critical | Severity::High) {
                summary.recommended_actions += 1;
            }
        }
    }
    summary.healthy_checks = facts
        .iter()
        .filter(|fact| rules::explicitly_healthy(fact))
        .count() as u32;
    summary
}

fn persist(db: &Database, owner: &str, mut snapshot: DeepScanSnapshot, findings: &[Finding]) -> DeepScanSnapshot {
    let mut writes = 0_u32;
    let mut persistence_errors = Vec::new();
    for finding in findings {
        match serde_json::to_string(finding) {
            Ok(finding_json) => {
                let record = IntelligenceFindingRecord {
                    owner_principal_key: owner.into(),
                    finding_id: finding.id.clone(),
                    finding_code: finding.code.clone(),
                    first_observed_unix_ms: finding.first_observed_unix_ms,
                    last_observed_unix_ms: finding.last_observed_unix_ms,
                    lifecycle: format!("{:?}", finding.lifecycle),
                    severity: format!("{:?}", finding.severity),
                    confidence: format!("{:?}", finding.confidence),
                    verification_status: format!("{:?}", finding.verification_status),
                    resolved_at_unix_ms: finding.resolved_at_unix_ms,
                    resolution_scan_id: finding.resolution_scan_id.clone(),
                    resolution_reason_key: finding.resolution_reason_key.clone(),
                    finding_json,
                };
                match db.upsert_intelligence_finding(&record) {
                    Ok(()) => writes = writes.saturating_add(1),
                    Err(error) => persistence_errors.push(format!("finding persistence unavailable: {error}")),
                }
            }
            Err(error) => persistence_errors.push(format!("finding serialization unavailable: {error}")),
        }
    }

    if !persistence_errors.is_empty() {
        if snapshot.state == ScanState::Completed {
            snapshot.state = ScanState::Partial;
        }
        snapshot.warnings.push("persistence continuity is partially unavailable".into());
    }

    snapshot.metrics.persistence_write_count = writes.saturating_add(1);
    match serde_json::to_string(&snapshot) {
        Ok(snapshot_json) => {
            let record = IntelligenceScanRecord {
                scan_id: snapshot.scan_id.clone(),
                owner_principal_key: owner.into(),
                state: format!("{:?}", snapshot.state),
                status: format!("{:?}", snapshot.status),
                started_unix_ms: snapshot.started_unix_ms,
                completed_unix_ms: snapshot.completed_unix_ms,
                machine_state_fingerprint: snapshot.machine_state_fingerprint.clone(),
                rule_engine_version: snapshot.rule_engine_version.clone(),
                app_version: snapshot.app_version.clone(),
                finding_count: snapshot.findings.len() as u32,
                unavailable_collector_count: snapshot
                    .collectors
                    .iter()
                    .filter(|collector| is_unavailable(collector.state))
                    .count() as u32,
                snapshot_json,
            };
            match db.save_intelligence_scan(&record) {
                Ok(()) => writes = writes.saturating_add(1),
                Err(error) => {
                    if snapshot.state == ScanState::Completed {
                        snapshot.state = ScanState::Partial;
                    }
                    snapshot.warnings.push(format!("scan history persistence unavailable: {error}"));
                }
            }
        }
        Err(error) => {
            if snapshot.state == ScanState::Completed {
                snapshot.state = ScanState::Partial;
            }
            snapshot.warnings.push(format!("scan history serialization unavailable: {error}"));
        }
    }
    snapshot.metrics.persistence_write_count = writes;
    snapshot
}

fn is_failed(state: CollectorState) -> bool {
    matches!(state, CollectorState::Failed | CollectorState::TimedOut)
}

fn is_unavailable(state: CollectorState) -> bool {
    matches!(
        state,
        CollectorState::Unavailable
            | CollectorState::PermissionDenied
            | CollectorState::TimedOut
            | CollectorState::Failed
    )
}
