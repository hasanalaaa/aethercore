#![forbid(unsafe_code)]

use std::sync::Arc;

use aethercore_persistence::{Database, PlanJournal, PlanRecord};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanState {
    Draft,
    Scanning,
    ReadyForReview,
    AwaitingAuthorization,
    Preflight,
    Protected,
    Executing,
    Verifying,
    Completed,
    Failed,
    RebootPending,
    Resuming,
}

impl PlanState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "Draft",
            Self::Scanning => "Scanning",
            Self::ReadyForReview => "ReadyForReview",
            Self::AwaitingAuthorization => "AwaitingAuthorization",
            Self::Preflight => "Preflight",
            Self::Protected => "Protected",
            Self::Executing => "Executing",
            Self::Verifying => "Verifying",
            Self::Completed => "Completed",
            Self::Failed => "Failed",
            Self::RebootPending => "RebootPending",
            Self::Resuming => "Resuming",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "Draft" => Self::Draft,
            "Scanning" => Self::Scanning,
            "ReadyForReview" => Self::ReadyForReview,
            "AwaitingAuthorization" => Self::AwaitingAuthorization,
            "Preflight" => Self::Preflight,
            "Protected" => Self::Protected,
            "Executing" => Self::Executing,
            "Verifying" => Self::Verifying,
            "Completed" => Self::Completed,
            "Failed" => Self::Failed,
            "RebootPending" => Self::RebootPending,
            "Resuming" => Self::Resuming,
            _ => return None,
        })
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DriverEvidence {
    pub provider: String,
    pub version: String,
    pub inf_path: String,
    pub date: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DriverInstallAction {
    pub candidate_id: String,
    pub update_id: String,
    pub revision: i32,
    pub instance_id: String,
    pub display_name: String,
    pub matched_hardware_id: String,
    pub match_quality: String,
    pub current_driver: Option<DriverEvidence>,
    pub current_problem_code: u32,
    pub update_title: String,
    pub provider: String,
    pub driver_class: String,
    pub min_download_bytes: u64,
    pub max_download_bytes: u64,
    pub target_version: String,
    pub authority_type: String,
    pub authority_provider_id: String,
    pub official_source: String,
    pub recommendation_state: String,
    pub installation_mode: String,
    pub trust_state: String,
    pub provenance_digest: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SystemRepairAction {
    pub assessment_id: String,
    pub run_component_store: bool,
    pub run_system_files: bool,
    pub run_disk_scan: bool,
    /// Trusted server-selected typed action IDs. Renderer input can never supply executable paths/arguments.
    pub repair_action_ids: Vec<String>,
    /// Canonical Phase 19 machine-state fingerprint captured before consent.
    pub machine_state_fingerprint: String,
    /// SHA-256 of the deterministic RepairGraph reviewed by the user.
    pub repair_graph_digest: String,
    /// Canonical serialized graph material sealed into the immutable plan.
    pub repair_graph_json: String,
    /// Highest safety tier represented by the executable plan subset.
    pub safety_tier: String,
    /// Number of hard reboot barriers represented by the reviewed graph.
    pub reboot_boundary_count: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CleanupFileEvidence {
    pub path: String,
    pub root: String,
    /// Final normalized provider-root path captured from a stable directory handle at scan time.
    /// Execution must reopen the provider root and match this identity before any target deletion.
    pub root_final_path: String,
    pub size_bytes: u64,
    pub modified_unix_ms: i64,
    /// Volume serial captured from FILE_ID_INFO on the opened scan handle.
    pub volume_serial_number: u64,
    /// 128-bit file identifier captured from FILE_ID_INFO, encoded as lowercase hex.
    pub file_id_128: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CleanupDeleteAction {
    pub candidate_id: String,
    pub scan_id: String,
    pub inventory_epoch: u64,
    pub provider: String,
    pub title: String,
    pub special_kind: String,
    pub files: Vec<CleanupFileEvidence>,
    pub expected_bytes: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StartupChangeAction {
    pub change_id: String,
    pub item_id: String,
    pub direction: String,
    /// Registry/task/service classification. Serialized as `startupKind` because `kind` is the
    /// PlanAction enum tag; `alias` keeps historical documents readable.
    #[serde(alias = "kind")]
    pub startup_kind: String,
    pub display_name: String,
    pub source_locator: String,
    pub original_state_json: String,
    pub applied_state_json: String,
    pub service_change: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PlanAction {
    InstallWindowsDriver(Box<DriverInstallAction>),
    RepairWindowsIntegrity(SystemRepairAction),
    DeleteCleanupCandidate(CleanupDeleteAction),
    ChangeStartupTarget(StartupChangeAction),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImmutablePlanMaterial {
    id: String,
    title: String,
    created_unix_ms: i64,
    inventory_epoch: u64,
    scan_id: String,
    risk: String,
    #[serde(default)]
    owner_principal_key: String,
    actions: Vec<PlanAction>,
}

#[derive(Clone, Debug)]
pub struct PlanView {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub state: PlanState,
    pub digest: String,
    pub risk: String,
    pub created_unix_ms: i64,
    pub updated_unix_ms: i64,
    pub consent_ready_until_unix_ms: i64,
    pub action_count: u32,
    pub inventory_epoch: u64,
    pub scan_id: String,
    pub owner_principal_key: String,
}

#[derive(Clone, Debug)]
pub struct ConsentIntentView {
    pub intent_id: String,
    pub plan_id: String,
    pub plan_digest: String,
    pub title: String,
    pub risk: String,
    pub action_count: u32,
    pub expires_unix_ms: i64,
}

#[derive(Debug, Error)]
pub enum EngineError {
    #[error("persistence: {0}")]
    Persistence(#[from] aethercore_persistence::PersistenceError),
    #[error("plan not found")]
    NotFound,
    #[error("invalid persisted state")]
    InvalidState,
    #[error("invalid transition from {0} to {1}")]
    InvalidTransition(String, String),
    #[error("authorization required")]
    AuthorizationRequired,
    #[error("plan digest mismatch")]
    DigestMismatch,
    #[error("persisted immutable plan failed integrity verification")]
    IntegrityMismatch,
    #[error("plan is owned by a different Windows principal")]
    OwnershipMismatch,
    #[error("consent intent invalid, expired, already approved, or already consumed")]
    ConsentIntentInvalid,
    #[error("plan contains no installable driver actions")]
    EmptyDriverPlan,
    #[error("plan action type does not match this operation")]
    WrongActionType,
    #[error("plan contains no repair action")]
    EmptyRepairPlan,
    #[error("plan contains no cleanup actions")]
    EmptyCleanupPlan,
    #[error("plan contains no startup actions")]
    EmptyStartupPlan,
    #[error("serialization: {0}")]
    Serialization(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, EngineError>;

pub struct OperationEngine {
    db: Arc<Database>,
}

impl OperationEngine {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }
    pub fn database(&self) -> Arc<Database> {
        self.db.clone()
    }

    pub fn create_driver_install_plan(
        &self,
        owner_principal_key: &str,
        inventory_epoch: u64,
        scan_id: &str,
        actions: Vec<DriverInstallAction>,
    ) -> Result<PlanView> {
        if actions.is_empty() {
            return Err(EngineError::EmptyDriverPlan);
        }
        require_owner_key(owner_principal_key)?;
        let now = now_ms();
        let count = actions.len();
        let material = ImmutablePlanMaterial {
            id: Uuid::new_v4().to_string(),
            title: format!(
                "Install {count} Windows driver update{}",
                if count == 1 { "" } else { "s" }
            ),
            created_unix_ms: now,
            inventory_epoch,
            scan_id: scan_id.to_owned(),
            risk: "Amber".into(),
            owner_principal_key: owner_principal_key.into(),
            actions: actions
                .into_iter()
                .map(|action| PlanAction::InstallWindowsDriver(Box::new(action)))
                .collect(),
        };
        self.insert_material(
            material,
            PlanState::AwaitingAuthorization,
            "driver_install_plan_created",
        )
    }

    fn insert_material(
        &self,
        material: ImmutablePlanMaterial,
        state: PlanState,
        event_kind: &str,
    ) -> Result<PlanView> {
        let immutable_json = serde_json::to_string(&material)?;
        let digest = hex::encode(Sha256::digest(immutable_json.as_bytes()));
        let now = material.created_unix_ms;
        let record = PlanRecord {
            id: material.id.clone(),
            title: material.title.clone(),
            state: state.as_str().into(),
            digest,
            risk: material.risk.clone(),
            immutable_json,
            created_unix_ms: now,
            updated_unix_ms: now,
            owner_principal_key: material.owner_principal_key.clone(),
        };
        self.db.insert_plan(&record, event_kind)?;
        self.view_from(record)
    }

    pub fn latest_plan(&self) -> Result<Option<PlanView>> {
        self.db
            .latest_plan()?
            .map(|p| self.view_from(p))
            .transpose()
    }
    pub fn latest_plan_for_owner(&self, owner_principal_key: &str) -> Result<Option<PlanView>> {
        self.db
            .latest_plan_for_owner(owner_principal_key)?
            .map(|p| self.view_from(p))
            .transpose()
    }
    pub fn get_plan(&self, id: &str) -> Result<PlanView> {
        self.view_from(self.db.get_plan(id)?.ok_or(EngineError::NotFound)?)
    }
    pub fn get_plan_for_owner(&self, id: &str, owner_principal_key: &str) -> Result<PlanView> {
        require_owner_key(owner_principal_key)?;
        let record = self
            .db
            .get_plan_for_owner(id, owner_principal_key)?
            .ok_or(EngineError::NotFound)?;
        self.view_from(record)
    }

    /// Plans still in non-terminal states, oldest activity first. Recovery workers use this at
    /// service startup to classify interrupted operations; it never replays anything by itself.
    pub fn recoverable_plans(&self) -> Result<Vec<PlanView>> {
        let records = self.db.plans_in_states(&[
            "Preflight",
            "Protected",
            "Executing",
            "Verifying",
            "RebootPending",
            "Resuming",
        ])?;
        let mut views = Vec::with_capacity(records.len());
        for record in records {
            // A plan whose hashed material no longer validates is skipped rather than surfaced:
            // recovery must never act on a record that fails integrity verification.
            if let Ok(view) = self.view_from(record) {
                views.push(view);
            }
        }
        Ok(views)
    }

    pub fn driver_install_actions(&self, id: &str) -> Result<Vec<DriverInstallAction>> {
        let record = self.db.get_plan(id)?.ok_or(EngineError::NotFound)?;
        let material = self.material_from(&record)?;
        let mut actions = Vec::new();
        for action in material.actions {
            match action {
                PlanAction::InstallWindowsDriver(v) => actions.push(*v),
                _ => return Err(EngineError::WrongActionType),
            }
        }
        if actions.is_empty() {
            return Err(EngineError::EmptyDriverPlan);
        }
        Ok(actions)
    }

    pub fn create_system_repair_plan(
        &self,
        owner_principal_key: &str,
        action: SystemRepairAction,
    ) -> Result<PlanView> {
        if action.repair_action_ids.is_empty()
            && !action.run_component_store
            && !action.run_system_files
            && !action.run_disk_scan
        {
            return Err(EngineError::EmptyRepairPlan);
        }
        require_owner_key(owner_principal_key)?;
        let now = now_ms();
        self.insert_material(
            ImmutablePlanMaterial {
                id: Uuid::new_v4().to_string(),
                title: "Repair Windows integrity".into(),
                created_unix_ms: now,
                inventory_epoch: 0,
                scan_id: action.assessment_id.clone(),
                risk: "Amber".into(),
                owner_principal_key: owner_principal_key.into(),
                actions: vec![PlanAction::RepairWindowsIntegrity(action)],
            },
            PlanState::AwaitingAuthorization,
            "system_repair_plan_created",
        )
    }

    pub fn system_repair_action(&self, id: &str) -> Result<SystemRepairAction> {
        let record = self.db.get_plan(id)?.ok_or(EngineError::NotFound)?;
        let material = self.material_from(&record)?;
        if material.actions.len() != 1 {
            return Err(EngineError::WrongActionType);
        }
        match material.actions.into_iter().next() {
            Some(PlanAction::RepairWindowsIntegrity(v)) => Ok(v),
            _ => Err(EngineError::WrongActionType),
        }
    }

    pub fn create_cleanup_plan(
        &self,
        owner_principal_key: &str,
        inventory_epoch: u64,
        scan_id: &str,
        actions: Vec<CleanupDeleteAction>,
    ) -> Result<PlanView> {
        if actions.is_empty() {
            return Err(EngineError::EmptyCleanupPlan);
        }
        require_owner_key(owner_principal_key)?;
        let now = now_ms();
        let count = actions.len();
        self.insert_material(
            ImmutablePlanMaterial {
                id: Uuid::new_v4().to_string(),
                title: format!(
                    "Clean {count} reviewed categor{}",
                    if count == 1 { "y" } else { "ies" }
                ),
                created_unix_ms: now,
                inventory_epoch,
                scan_id: scan_id.to_owned(),
                risk: "Amber".into(),
                owner_principal_key: owner_principal_key.into(),
                actions: actions
                    .into_iter()
                    .map(PlanAction::DeleteCleanupCandidate)
                    .collect(),
            },
            PlanState::AwaitingAuthorization,
            "cleanup_plan_created",
        )
    }

    pub fn cleanup_actions(&self, id: &str) -> Result<Vec<CleanupDeleteAction>> {
        let record = self.db.get_plan(id)?.ok_or(EngineError::NotFound)?;
        let material = self.material_from(&record)?;
        let mut actions = Vec::new();
        for action in material.actions {
            match action {
                PlanAction::DeleteCleanupCandidate(v) => actions.push(v),
                _ => return Err(EngineError::WrongActionType),
            }
        }
        if actions.is_empty() {
            return Err(EngineError::EmptyCleanupPlan);
        }
        Ok(actions)
    }

    pub fn create_startup_plan(
        &self,
        owner_principal_key: &str,
        inventory_epoch: u64,
        scan_id: &str,
        actions: Vec<StartupChangeAction>,
    ) -> Result<PlanView> {
        if actions.is_empty() {
            return Err(EngineError::EmptyStartupPlan);
        }
        require_owner_key(owner_principal_key)?;
        let now = now_ms();
        let restoring = actions.iter().all(|a| a.direction == "Restore");
        let title = if restoring {
            format!(
                "Restore {} startup change{}",
                actions.len(),
                if actions.len() == 1 { "" } else { "s" }
            )
        } else {
            format!(
                "Optimize {} startup target{}",
                actions.len(),
                if actions.len() == 1 { "" } else { "s" }
            )
        };
        self.insert_material(
            ImmutablePlanMaterial {
                id: Uuid::new_v4().to_string(),
                title,
                created_unix_ms: now,
                inventory_epoch,
                scan_id: scan_id.to_owned(),
                risk: "Amber".into(),
                owner_principal_key: owner_principal_key.into(),
                actions: actions
                    .into_iter()
                    .map(PlanAction::ChangeStartupTarget)
                    .collect(),
            },
            PlanState::AwaitingAuthorization,
            "startup_plan_created",
        )
    }

    pub fn startup_actions(&self, id: &str) -> Result<Vec<StartupChangeAction>> {
        let record = self.db.get_plan(id)?.ok_or(EngineError::NotFound)?;
        let material = self.material_from(&record)?;
        let mut actions = Vec::new();
        for action in material.actions {
            match action {
                PlanAction::ChangeStartupTarget(v) => actions.push(v),
                _ => return Err(EngineError::WrongActionType),
            }
        }
        if actions.is_empty() {
            return Err(EngineError::EmptyStartupPlan);
        }
        Ok(actions)
    }

    pub fn transition(
        &self,
        id: &str,
        expected: PlanState,
        next: PlanState,
        detail: &str,
    ) -> Result<PlanView> {
        self.transition_via(id, expected, next, |now| {
            self.db
                .transition_plan(id, expected.as_str(), next.as_str(), detail, now)
        })
    }

    /// `transition`, with `journal` committed in the same transaction as the new state. Nothing
    /// is written unless the state actually moves.
    pub fn transition_with_journal(
        &self,
        id: &str,
        expected: PlanState,
        next: PlanState,
        detail: &str,
        journal: PlanJournal<'_>,
    ) -> Result<PlanView> {
        self.transition_via(id, expected, next, |now| {
            self.db.transition_plan_with_journal(
                id,
                expected.as_str(),
                next.as_str(),
                detail,
                now,
                journal,
            )
        })
    }

    fn transition_via(
        &self,
        id: &str,
        expected: PlanState,
        next: PlanState,
        write: impl FnOnce(i64) -> aethercore_persistence::Result<bool>,
    ) -> Result<PlanView> {
        if !valid_transition(expected, next) {
            return Err(EngineError::InvalidTransition(
                expected.as_str().into(),
                next.as_str().into(),
            ));
        }
        let current = self.get_plan(id)?;
        if current.state != expected {
            return Err(EngineError::InvalidTransition(
                current.state.as_str().into(),
                next.as_str().into(),
            ));
        }
        if expected == PlanState::AwaitingAuthorization && next == PlanState::Preflight {
            return Err(EngineError::AuthorizationRequired);
        }
        if !write(now_ms())? {
            return Err(EngineError::InvalidTransition(
                expected.as_str().into(),
                next.as_str().into(),
            ));
        }
        self.get_plan(id)
    }

    pub fn append_event(&self, id: &str, kind: &str, detail: &str) -> Result<()> {
        let plan = self.get_plan(id)?;
        self.db
            .append_plan_event(id, plan.state.as_str(), kind, detail, now_ms())?;
        Ok(())
    }

    pub fn begin_consent_intent(
        &self,
        id: &str,
        owner_principal_key: &str,
    ) -> Result<ConsentIntentView> {
        let plan = self.get_plan_for_owner(id, owner_principal_key)?;
        if plan.state != PlanState::AwaitingAuthorization {
            return Err(EngineError::InvalidTransition(
                plan.state.as_str().into(),
                "ConsentIntent".into(),
            ));
        }
        let intent_id = Uuid::new_v4().to_string();
        let now = now_ms();
        let expiry = now + 120_000;
        self.db.begin_consent_intent(
            &intent_id,
            id,
            &plan.digest,
            owner_principal_key,
            now,
            expiry,
        )?;
        Ok(ConsentIntentView {
            intent_id,
            plan_id: plan.id,
            plan_digest: plan.digest,
            title: plan.title,
            risk: plan.risk,
            action_count: plan.action_count,
            expires_unix_ms: expiry,
        })
    }

    pub fn consent_intent_for_broker(
        &self,
        intent_id: &str,
        owner_principal_key: &str,
    ) -> Result<ConsentIntentView> {
        let record = self
            .db
            .get_consent_intent(intent_id)?
            .ok_or(EngineError::ConsentIntentInvalid)?;
        let now = now_ms();
        if record.owner_principal_key != owner_principal_key
            || record.expires_unix_ms < now
            || record.approved_unix_ms.is_some()
            || record.consumed_unix_ms.is_some()
        {
            return Err(EngineError::ConsentIntentInvalid);
        }
        let plan = self.get_plan_for_owner(&record.plan_id, owner_principal_key)?;
        if plan.state != PlanState::AwaitingAuthorization || plan.digest != record.digest {
            return Err(EngineError::ConsentIntentInvalid);
        }
        Ok(ConsentIntentView {
            intent_id: record.intent_id,
            plan_id: plan.id,
            plan_digest: plan.digest,
            title: plan.title,
            risk: plan.risk,
            action_count: plan.action_count,
            expires_unix_ms: record.expires_unix_ms,
        })
    }

    pub fn approve_consent_intent(
        &self,
        intent_id: &str,
        owner_principal_key: &str,
        broker_pid: u32,
    ) -> Result<(ConsentIntentView, i64)> {
        let intent = self.consent_intent_for_broker(intent_id, owner_principal_key)?;
        let now = now_ms();
        if !self
            .db
            .approve_consent_intent(intent_id, owner_principal_key, broker_pid, now)?
        {
            return Err(EngineError::ConsentIntentInvalid);
        }
        Ok((intent, now))
    }

    pub fn consume_authorization_and_begin(
        &self,
        id: &str,
        owner_principal_key: &str,
        detail: &str,
    ) -> Result<PlanView> {
        let plan = self.get_plan_for_owner(id, owner_principal_key)?;
        if plan.state != PlanState::AwaitingAuthorization {
            return Err(EngineError::InvalidTransition(
                plan.state.as_str().into(),
                PlanState::Preflight.as_str().into(),
            ));
        }
        let changed = self.db.consume_consent_and_transition(
            id,
            owner_principal_key,
            PlanState::AwaitingAuthorization.as_str(),
            PlanState::Preflight.as_str(),
            detail,
            now_ms(),
        )?;
        if !changed {
            return Err(EngineError::AuthorizationRequired);
        }
        self.get_plan_for_owner(id, owner_principal_key)
    }

    pub fn event_count(&self) -> Result<u64> {
        Ok(self.db.event_count()?)
    }
    pub fn event_count_for_owner(&self, owner_principal_key: &str) -> Result<u64> {
        Ok(self.db.event_count_for_owner(owner_principal_key)?)
    }

    fn view_from(&self, p: PlanRecord) -> Result<PlanView> {
        let material = self.material_from(&p)?;
        let state = PlanState::parse(&p.state).ok_or(EngineError::InvalidState)?;
        let auth = self.db.approved_consent_until(
            &material.id,
            &p.digest,
            &material.owner_principal_key,
            now_ms(),
        )?;
        let kind = plan_kind(&material.actions)?.to_owned();
        Ok(PlanView {
            id: material.id,
            kind,
            title: material.title,
            state,
            digest: p.digest,
            risk: material.risk,
            created_unix_ms: material.created_unix_ms,
            updated_unix_ms: p.updated_unix_ms,
            consent_ready_until_unix_ms: auth,
            action_count: material.actions.len().try_into().unwrap_or(u32::MAX),
            inventory_epoch: material.inventory_epoch,
            scan_id: material.scan_id,
            owner_principal_key: material.owner_principal_key,
        })
    }

    fn material_from(&self, p: &PlanRecord) -> Result<ImmutablePlanMaterial> {
        let persisted_digest = hex::encode(Sha256::digest(p.immutable_json.as_bytes()));
        if persisted_digest != p.digest {
            return Err(EngineError::IntegrityMismatch);
        }
        let material: ImmutablePlanMaterial = serde_json::from_str(&p.immutable_json)?;
        // `immutable_json` is the authoritative consent/execution material. The duplicated columns
        // exist for indexed queries only and must never be allowed to disagree with what is hashed.
        // A mismatch is corruption (or tampering) and fails closed before a plan can be displayed,
        // consented, or executed.
        if material.id != p.id
            || material.title != p.title
            || material.risk != p.risk
            || material.created_unix_ms != p.created_unix_ms
            || material.owner_principal_key != p.owner_principal_key
            || p.updated_unix_ms < p.created_unix_ms
        {
            return Err(EngineError::IntegrityMismatch);
        }
        Ok(material)
    }
}

fn plan_kind(actions: &[PlanAction]) -> Result<&'static str> {
    let Some(first) = actions.first() else {
        return Err(EngineError::WrongActionType);
    };
    let kind = match first {
        PlanAction::InstallWindowsDriver(_) => "DriverInstall",
        PlanAction::RepairWindowsIntegrity(_) => "SystemRepair",
        PlanAction::DeleteCleanupCandidate(_) => "Cleanup",
        PlanAction::ChangeStartupTarget(_) => "Startup",
    };
    if actions.iter().any(|action| {
        !matches!(
            (kind, action),
            ("DriverInstall", PlanAction::InstallWindowsDriver(_))
                | ("SystemRepair", PlanAction::RepairWindowsIntegrity(_))
                | ("Cleanup", PlanAction::DeleteCleanupCandidate(_))
                | ("Startup", PlanAction::ChangeStartupTarget(_))
        )
    }) {
        return Err(EngineError::WrongActionType);
    }
    Ok(kind)
}

fn valid_transition(from: PlanState, to: PlanState) -> bool {
    matches!(
        (from, to),
        (PlanState::Draft, PlanState::Scanning)
            | (PlanState::Scanning, PlanState::ReadyForReview)
            | (PlanState::ReadyForReview, PlanState::AwaitingAuthorization)
            | (PlanState::AwaitingAuthorization, PlanState::Preflight)
            | (PlanState::Preflight, PlanState::Protected)
            | (PlanState::Preflight, PlanState::Failed)
            | (PlanState::Protected, PlanState::Executing)
            | (PlanState::Protected, PlanState::Failed)
            | (PlanState::Executing, PlanState::Verifying)
            | (PlanState::Executing, PlanState::RebootPending)
            | (PlanState::Executing, PlanState::Failed)
            | (PlanState::Verifying, PlanState::Completed)
            | (PlanState::Verifying, PlanState::RebootPending)
            | (PlanState::Verifying, PlanState::Failed)
            | (PlanState::RebootPending, PlanState::Resuming)
            | (PlanState::Resuming, PlanState::Verifying)
            | (PlanState::Resuming, PlanState::Completed)
            | (PlanState::Resuming, PlanState::Failed)
    )
}

fn require_owner_key(value: &str) -> Result<()> {
    if value.len() != 64 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(EngineError::OwnershipMismatch);
    }
    Ok(())
}

pub fn now_ms() -> i64 {
    Utc::now().timestamp_millis()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    const OWNER_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const OWNER_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    fn engine() -> (OperationEngine, PathBuf) {
        let path = std::env::temp_dir().join(format!("aethercore-engine-{}.db", Uuid::new_v4()));
        let db = Arc::new(Database::open(&path).unwrap());
        (OperationEngine::new(db), path)
    }
    fn cleanup(path: &Path) {
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(path.with_extension("db-wal"));
        let _ = std::fs::remove_file(path.with_extension("db-shm"));
    }
    fn action() -> DriverInstallAction {
        DriverInstallAction {
            candidate_id: "c1".into(),
            update_id: "u1".into(),
            revision: 2,
            instance_id: "PCI\\A".into(),
            display_name: "Adapter".into(),
            matched_hardware_id: "PCI\\VEN_1".into(),
            match_quality: "Hardware ID".into(),
            current_driver: None,
            current_problem_code: 0,
            update_title: "Vendor - Net - 2.0.0.0".into(),
            provider: "Vendor".into(),
            driver_class: "Net".into(),
            min_download_bytes: 10,
            max_download_bytes: 20,
            target_version: "2.0.0.0".into(),
            authority_type: "WindowsUpdate".into(),
            authority_provider_id: "microsoft.windows-update".into(),
            official_source: "Windows Update".into(),
            recommendation_state: "Recommended".into(),
            installation_mode: "WindowsManaged".into(),
            trust_state: "WindowsManaged".into(),
            provenance_digest: "deadbeef".into(),
        }
    }

    #[test]
    fn plan_ownership_blocks_cross_user_access() {
        let (e, p) = engine();
        let plan = e
            .create_driver_install_plan(OWNER_A, 7, "scan-7", vec![action()])
            .unwrap();
        assert_eq!(e.get_plan_for_owner(&plan.id, OWNER_A).unwrap().id, plan.id);
        assert!(matches!(
            e.get_plan_for_owner(&plan.id, OWNER_B),
            Err(EngineError::NotFound)
        ));
        drop(e);
        cleanup(&p);
    }

    #[test]
    fn consent_intent_is_principal_bound_and_one_shot() {
        let (e, p) = engine();
        let plan = e
            .create_driver_install_plan(OWNER_A, 7, "scan-7", vec![action()])
            .unwrap();
        let intent = e.begin_consent_intent(&plan.id, OWNER_A).unwrap();
        assert!(matches!(
            e.consent_intent_for_broker(&intent.intent_id, OWNER_B),
            Err(EngineError::ConsentIntentInvalid)
        ));
        e.approve_consent_intent(&intent.intent_id, OWNER_A, 99)
            .unwrap();
        assert!(matches!(
            e.consent_intent_for_broker(&intent.intent_id, OWNER_A),
            Err(EngineError::ConsentIntentInvalid)
        ));
        assert!(matches!(
            e.approve_consent_intent(&intent.intent_id, OWNER_A, 100),
            Err(EngineError::ConsentIntentInvalid)
        ));
        let started = e
            .consume_authorization_and_begin(&plan.id, OWNER_A, "test atomic consume")
            .unwrap();
        assert_eq!(started.state, PlanState::Preflight);
        assert!(matches!(
            e.consume_authorization_and_begin(&plan.id, OWNER_A, "replay"),
            Err(EngineError::InvalidTransition(_, _))
        ));
        drop(e);
        cleanup(&p);
    }

    #[test]
    fn ordinary_transition_cannot_bypass_one_shot_authorization() {
        let (e, p) = engine();
        let plan = e
            .create_driver_install_plan(OWNER_A, 1, "s", vec![action()])
            .unwrap();
        assert!(matches!(
            e.transition(
                &plan.id,
                PlanState::AwaitingAuthorization,
                PlanState::Preflight,
                "bypass"
            ),
            Err(EngineError::AuthorizationRequired)
        ));
        drop(e);
        cleanup(&p);
    }

    #[test]
    fn duplicated_plan_columns_cannot_diverge_from_hashed_material() {
        let (e, p) = engine();
        let plan = e
            .create_driver_install_plan(OWNER_A, 7, "scan-7", vec![action()])
            .unwrap();
        let conn = rusqlite::Connection::open(&p).unwrap();
        conn.execute(
            "UPDATE plans SET title = ?1, risk = ?2, created_unix_ms = created_unix_ms - 1 WHERE id = ?3",
            rusqlite::params!["Install 0 harmless updates", "Green", &plan.id],
        ).unwrap();
        drop(conn);
        assert!(matches!(
            e.get_plan(&plan.id),
            Err(EngineError::IntegrityMismatch)
        ));
        assert!(matches!(
            e.begin_consent_intent(&plan.id, OWNER_A),
            Err(EngineError::IntegrityMismatch)
        ));
        drop(e);
        cleanup(&p);
    }

    #[test]
    fn phase4_and_startup_plans_remain_typed() {
        let (e, p) = engine();
        let repair = e
            .create_system_repair_plan(
                OWNER_A,
                SystemRepairAction {
                    assessment_id: "assessment-1".into(),
                    run_component_store: true,
                    run_system_files: true,
                    run_disk_scan: false,
                    repair_action_ids: vec![
                        "repair-component-store".into(),
                        "repair-system-files".into(),
                    ],
                    machine_state_fingerprint: "fp".into(),
                    repair_graph_digest: "graph".into(),
                    repair_graph_json: "{}".into(),
                    safety_tier: "Level2SensitiveRepair".into(),
                    reboot_boundary_count: 0,
                },
            )
            .unwrap();
        assert_eq!(
            e.system_repair_action(&repair.id).unwrap().assessment_id,
            "assessment-1"
        );
        let cleanup_action = CleanupDeleteAction {
            candidate_id: "cleanup-1".into(),
            scan_id: "scan-clean".into(),
            inventory_epoch: 9,
            provider: "Temp".into(),
            title: "Temp files".into(),
            special_kind: "Files".into(),
            files: vec![CleanupFileEvidence {
                path: r"C:\\Temp\\a.tmp".into(),
                root: r"C:\\Temp".into(),
                root_final_path: r"C:\\Temp".into(),
                size_bytes: 10,
                modified_unix_ms: 1,
                volume_serial_number: 1,
                file_id_128: "00112233445566778899aabbccddeeff".into(),
            }],
            expected_bytes: 10,
        };
        let cleanup_plan = e
            .create_cleanup_plan(OWNER_A, 9, "scan-clean", vec![cleanup_action])
            .unwrap();
        assert_eq!(
            e.cleanup_actions(&cleanup_plan.id).unwrap()[0].candidate_id,
            "cleanup-1"
        );
        let startup = StartupChangeAction {
            change_id: "chg-1".into(),
            item_id: "item-1".into(),
            direction: "Disable".into(),
            startup_kind: "RegistryRun".into(),
            display_name: "Agent".into(),
            source_locator: "HKLM64|...|Agent".into(),
            original_state_json: r#"{"exists":true,"data":"AA=="}"#.into(),
            applied_state_json: r#"{"exists":false}"#.into(),
            service_change: false,
        };
        let startup_plan = e
            .create_startup_plan(OWNER_A, 11, "startup-scan", vec![startup.clone()])
            .unwrap();
        assert_eq!(e.startup_actions(&startup_plan.id).unwrap(), vec![startup]);
        drop(e);
        cleanup(&p);
    }
}
