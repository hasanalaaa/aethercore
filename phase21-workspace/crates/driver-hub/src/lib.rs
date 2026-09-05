#![forbid(unsafe_code)]

use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
    thread,
};

pub use aethercore_collector_runtime::{CancellationToken, CommitFence};
use aethercore_operation_kernel::{ReadBudgetLease, ReadWorkload};
use aethercore_gpu_policy::{installed_app_path, policy as gpu_vendor_policy};
use aethercore_driver_authority::{
    AcquisitionCapability, ApplicabilityState, AuthorityType, CoverageState, DeviceIdentity,
    DriverAuthorityEngine, DriverCandidateV2, InstallationCapability, MachineProfile, MatchKind,
    default_defer_until, privacy_device_key, DriverOverride, DriverOverrideKind, OfficialUtilityProvider,
    PackageTrustState, RecommendationState, WindowsUpdateProvider, builtin_provider_registry,
    evaluate_required_authorities, management_authority_for_registry_provider, required_authorities,
    DeviceAuthorityCoverage, DriverManagementAuthority, ManagementAuthorityAvailability, UpdateAvailabilityEvidence,
};
use aethercore_windows_pnp::{DeviceRecord, InstalledDriver, normalize_pnp_id};
use aethercore_persistence::{Database, DriverAuthorityOverrideRecord, DriverAuthorityScanRecord};
use aethercore_windows_update::DiscoveryResult;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum HubError {
    #[error("a driver scan is already running")]
    Busy,
    #[error("driver inventory failed: {0}")]
    Inventory(String),
    #[error("driver discovery was cancelled before publication")]
    Cancelled,
    #[error("driver snapshot is not ready")]
    SnapshotNotReady,
    #[error("driver snapshot changed; rescan before installing")]
    StaleSnapshot,
    #[error("driver snapshot belongs to a different Windows principal")]
    OwnershipMismatch,
    #[error("no driver candidates were selected")]
    EmptySelection,
    #[error("too many driver candidates selected in one plan")]
    TooManySelections,
    #[error("driver candidate not found: {0}")]
    CandidateNotFound(String),
    #[error("driver candidate is protected and cannot be installed by AetherCore: {0}")]
    CandidateNotSelectable(String),
    #[error("duplicate driver candidate: {0}")]
    DuplicateCandidate(String),
    #[error("invalid driver preference policy: {0}")]
    InvalidPolicy(String),
    #[error("driver preference persistence failed: {0}")]
    Persistence(String),
}

pub type Result<T> = std::result::Result<T, HubError>;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ScanState {
    Idle,
    InventoryScanning,
    UpdateSearching,
    Matching,
    Ready,
    Failed,
}

impl ScanState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "Idle",
            Self::InventoryScanning => "InventoryScanning",
            Self::UpdateSearching => "UpdateSearching",
            Self::Matching => "Matching",
            Self::Ready => "Ready",
            Self::Failed => "Failed",
        }
    }

    fn is_running(self) -> bool {
        matches!(
            self,
            Self::InventoryScanning | Self::UpdateSearching | Self::Matching
        )
    }
}

use aethercore_gpu_policy::GpuVendor;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GpuManagement {
    pub vendor: GpuVendor,
    pub provider_id: String,
    pub app_installed: bool,
    pub app_name: String,
    pub official_url: String,
    pub management_status: String,
    pub update_availability: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DriverManagementAuthorityInfo {
    pub provider_id: String,
    pub authority_type: String,
    pub display_name: String,
    pub official_authority: String,
    pub official_url: String,
    pub availability: String,
    pub update_availability: String,
    pub installation_mode: String,
    pub update_evidence_candidate_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DriverCandidate {
    pub candidate_id: String,
    pub update_id: String,
    pub revision: i32,
    pub title: String,
    pub provider: String,
    pub manufacturer: String,
    pub model: String,
    pub driver_class: String,
    pub matched_hardware_id: String,
    pub match_quality: String,
    pub driver_date_iso: String,
    pub min_download_bytes: u64,
    pub max_download_bytes: u64,
    pub target_version: String,
    pub target_version_source: String,
    pub selectable: bool,
    pub selected_by_default: bool,
    pub vendor_managed: bool,
    pub firmware_managed: bool,
    pub selection_policy: String,
    pub authority_type: String,
    pub authority_provider_id: String,
    pub authority_name: String,
    pub official_source: String,
    pub applicability: String,
    pub trust_state: String,
    pub recommendation_state: String,
    pub recommendation_reasons: Vec<String>,
    pub acquisition_mode: String,
    pub installation_mode: String,
    pub official_support_url: String,
    pub recommended: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DriverDevice {
    pub instance_id: String,
    pub display_name: String,
    pub description: String,
    pub class_name: String,
    pub class_guid: String,
    pub manufacturer: String,
    pub enumerator: String,
    pub location: String,
    pub hardware_ids: Vec<String>,
    pub compatible_ids: Vec<String>,
    pub raw_status: u32,
    pub problem_code: u32,
    pub has_problem: bool,
    pub missing_driver: bool,
    pub driver: Option<InstalledDriver>,
    pub device_state: String,
    pub gpu: Option<GpuManagement>,
    /// All display-class devices are protected from automatic driver selection, even when
    /// their PCI vendor is not one of NVIDIA/AMD/Intel.
    pub display_managed: bool,
    pub recommended_candidate_id: String,
    pub update_status: String,
    pub authority_coverage: String,
    pub required_authorities: Vec<String>,
    pub evaluated_authorities: Vec<String>,
    pub unavailable_authorities: Vec<String>,
    pub unsupported_authorities: Vec<String>,
    pub manual_authorities: Vec<String>,
    pub management_authorities: Vec<DriverManagementAuthorityInfo>,
    pub candidates: Vec<DriverCandidate>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UnmatchedOffer {
    pub update_id: String,
    pub revision: i32,
    pub title: String,
    pub hardware_id: String,
    pub provider: String,
    pub driver_class: String,
    pub min_download_bytes: u64,
    pub max_download_bytes: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DriverHubSummary {
    pub device_count: u32,
    pub missing_driver_count: u32,
    pub problem_device_count: u32,
    pub update_offer_count: u32,
    pub matched_device_count: u32,
    pub matched_candidate_count: u32,
    pub selectable_update_count: u32,
    pub vendor_managed_gpu_count: u32,
    pub vendor_managed_display_count: u32,
    pub recommended_update_count: u32,
    pub optional_update_count: u32,
    pub vendor_managed_update_count: u32,
    pub status_unknown_count: u32,
    pub management_authority_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DriverHubSnapshot {
    pub scan_id: String,
    pub state: ScanState,
    pub inventory_epoch: u64,
    pub started_unix_ms: i64,
    pub completed_unix_ms: i64,
    pub error_message: String,
    pub summary: DriverHubSummary,
    pub devices: Vec<DriverDevice>,
    pub unmatched_offers: Vec<UnmatchedOffer>,
    pub warnings: Vec<String>,
    pub authority_coverage: String,
    pub provider_status: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InstallSelection {
    pub candidate: DriverCandidate,
    pub instance_id: String,
    pub display_name: String,
    pub class_name: String,
    pub current_driver: Option<InstalledDriver>,
    pub problem_code: u32,
}

impl Default for DriverHubSnapshot {
    fn default() -> Self {
        Self {
            scan_id: String::new(),
            state: ScanState::Idle,
            inventory_epoch: 0,
            started_unix_ms: 0,
            completed_unix_ms: 0,
            error_message: String::new(),
            summary: DriverHubSummary::default(),
            devices: Vec::new(),
            unmatched_offers: Vec::new(),
            warnings: Vec::new(),
            authority_coverage: "Unknown".into(),
            provider_status: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiscoveryFailure {
    Offline(String),
    Unavailable(String),
}

impl std::fmt::Display for DiscoveryFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Offline(detail) => write!(f, "discovery offline: {detail}"),
            Self::Unavailable(detail) => write!(f, "discovery unavailable: {detail}"),
        }
    }
}

pub trait DiscoveryBackend: Send + Sync + 'static {
    fn inventory(&self) -> std::result::Result<Vec<DeviceRecord>, String>;
    fn updates(&self) -> std::result::Result<DiscoveryResult, DiscoveryFailure>;
}

#[derive(Default)]
pub struct WindowsDiscoveryBackend;

impl DiscoveryBackend for WindowsDiscoveryBackend {
    fn inventory(&self) -> std::result::Result<Vec<DeviceRecord>, String> {
        aethercore_windows_pnp::scan_present_devices().map_err(|e| e.to_string())
    }

    fn updates(&self) -> std::result::Result<DiscoveryResult, DiscoveryFailure> {
        aethercore_windows_update::discover_driver_offers().map_err(|error| match error {
            aethercore_windows_update::UpdateError::Offline(detail) => DiscoveryFailure::Offline(detail),
            other => DiscoveryFailure::Unavailable(other.to_string()),
        })
    }
}

#[derive(Clone)]
pub struct DriverHub {
    inner: Arc<DriverHubInner>,
}

struct DriverHubInner {
    backend: Arc<dyn DiscoveryBackend>,
    database: Option<Arc<Database>>,
    snapshot: Mutex<DriverHubSnapshot>,
    owner_principal_key: Mutex<String>,
}

impl Default for DriverHub {
    fn default() -> Self {
        Self::new()
    }
}

impl DriverHub {
    pub fn new() -> Self {
        Self::with_backend(Arc::new(WindowsDiscoveryBackend))
    }

    pub fn with_database(database: Arc<Database>) -> Self {
        Self::with_backend_and_database(Arc::new(WindowsDiscoveryBackend), Some(database))
    }

    pub fn with_backend(backend: Arc<dyn DiscoveryBackend>) -> Self {
        Self::with_backend_and_database(backend, None)
    }

    fn with_backend_and_database(backend: Arc<dyn DiscoveryBackend>, database: Option<Arc<Database>>) -> Self {
        Self {
            inner: Arc::new(DriverHubInner {
                backend, database,
                snapshot: Mutex::new(DriverHubSnapshot::default()),
                owner_principal_key: Mutex::new(String::new()),
            }),
        }
    }

    fn snapshot(&self) -> DriverHubSnapshot {
        self.inner
            .snapshot
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    pub fn snapshot_for_owner(&self, owner_principal_key: &str) -> Result<DriverHubSnapshot> {
        let owner = self
            .inner
            .owner_principal_key
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if owner.as_str() != owner_principal_key {
            return Err(HubError::OwnershipMismatch);
        }
        Ok(self.snapshot())
    }

    /// Resolves UI candidate IDs against one exact, completed service snapshot. This is the
    /// Phase 3 trust boundary: the UI cannot construct an update identity, instance ID, class,
    /// or INF path. Every install action is copied from IDs the service minted during Phase 2.
    pub fn resolve_install_selection(
        &self,
        owner_principal_key: &str,
        scan_id: &str,
        inventory_epoch: u64,
        candidate_ids: &[String],
    ) -> Result<Vec<InstallSelection>> {
        if candidate_ids.is_empty() {
            return Err(HubError::EmptySelection);
        }
        if candidate_ids.len() > 32 {
            return Err(HubError::TooManySelections);
        }

        let snapshot = self.snapshot_for_owner(owner_principal_key)?;
        if snapshot.state != ScanState::Ready {
            return Err(HubError::SnapshotNotReady);
        }
        if snapshot.scan_id != scan_id || snapshot.inventory_epoch != inventory_epoch {
            return Err(HubError::StaleSnapshot);
        }

        let mut requested = HashSet::new();
        for id in candidate_ids {
            if !requested.insert(id.as_str()) {
                return Err(HubError::DuplicateCandidate(id.clone()));
            }
        }

        let mut found: HashMap<&str, InstallSelection> = HashMap::new();
        for device in &snapshot.devices {
            for candidate in &device.candidates {
                if !requested.contains(candidate.candidate_id.as_str()) {
                    continue;
                }
                if !candidate.selectable
                    || candidate.vendor_managed
                    || candidate.firmware_managed
                    || candidate.installation_mode != "WindowsManaged"
                    || !matches!(candidate.recommendation_state.as_str(), "Recommended" | "Optional")
                {
                    return Err(HubError::CandidateNotSelectable(candidate.candidate_id.clone()));
                }
                found.insert(candidate.candidate_id.as_str(), InstallSelection {
                    candidate: candidate.clone(),
                    instance_id: device.instance_id.clone(),
                    display_name: device.display_name.clone(),
                    class_name: device.class_name.clone(),
                    current_driver: device.driver.clone(),
                    problem_code: device.problem_code,
                });
            }
        }

        let mut result = Vec::with_capacity(candidate_ids.len());
        for id in candidate_ids {
            result.push(found.remove(id.as_str()).ok_or_else(|| HubError::CandidateNotFound(id.clone()))?);
        }
        Ok(result)
    }

    pub fn set_candidate_policy(
        &self,
        owner_principal_key: &str,
        scan_id: &str,
        inventory_epoch: u64,
        candidate_id: &str,
        policy: &str,
    ) -> Result<DriverHubSnapshot> {
        let database = self.inner.database.as_ref().ok_or_else(|| HubError::Persistence("driver authority preference store unavailable".into()))?;
        let snapshot = self.snapshot_for_owner(owner_principal_key)?;
        if snapshot.state != ScanState::Ready || snapshot.scan_id != scan_id || snapshot.inventory_epoch != inventory_epoch { return Err(HubError::StaleSnapshot); }
        let mut matched: Option<(&DriverDevice, &DriverCandidate)> = None;
        for device in &snapshot.devices {
            if let Some(candidate) = device.candidates.iter().find(|candidate| candidate.candidate_id == candidate_id) { matched = Some((device, candidate)); break; }
        }
        let (device, candidate) = matched.ok_or_else(|| HubError::CandidateNotFound(candidate_id.into()))?;
        let privacy_key = privacy_device_key(&device.instance_id);
        let now = Utc::now().timestamp_millis();
        let (kind, behavior, version, expires) = match policy {
            "IgnoreExactVersion" => (DriverOverrideKind::IgnoreExactVersion, "IgnoreExactVersion", candidate.target_version.clone(), None),
            "RemindLater" => (DriverOverrideKind::RemindLater, "RemindLater", String::new(), Some(default_defer_until(now))),
            "IgnoreOptional" if candidate.recommendation_state == "Optional" => (DriverOverrideKind::IgnoreOptional, "IgnoreOptional", String::new(), None),
            other => return Err(HubError::InvalidPolicy(other.into())),
        };
        let override_id = format!("{}:{}:{}:{}", privacy_key, behavior, version, candidate.authority_provider_id);
        database.upsert_driver_authority_override(&DriverAuthorityOverrideRecord {
            owner_principal_key: owner_principal_key.into(), override_id, device_privacy_key: privacy_key,
            behavior: behavior.into(), candidate_version: version, provider_id: candidate.authority_provider_id.clone(),
            expires_unix_ms: expires, created_unix_ms: now, updated_unix_ms: now,
        }).map_err(|e| HubError::Persistence(e.to_string()))?;
        let _ = kind;
        let mut current = self.inner.snapshot.lock().unwrap_or_else(|p| p.into_inner());
        if current.scan_id == scan_id && current.inventory_epoch == inventory_epoch {
            for device in &mut current.devices {
                for candidate in &mut device.candidates {
                    if candidate.candidate_id == candidate_id {
                        candidate.selectable = false; candidate.selected_by_default = false; candidate.recommended = false;
                        candidate.recommendation_state = "NotRecommended".into();
                        candidate.recommendation_reasons.push(match policy { "RemindLater" => "USER_DEFERRED", "IgnoreOptional" => "USER_IGNORED_OPTIONAL", _ => "USER_IGNORED_EXACT_VERSION" }.into());
                    }
                }
                if device.recommended_candidate_id == candidate_id {
                    device.recommended_candidate_id.clear();
                    if device.missing_driver {
                        device.update_status = "NoTrustedCandidate".into();
                        device.device_state = "MissingDriver".into();
                    } else if device.has_problem {
                        device.update_status = "DeviceProblemNoTrustedCandidate".into();
                        device.device_state = "Problem".into();
                    } else if policy == "RemindLater" {
                        device.update_status = "UpdateDeferred".into();
                        device.device_state = "Healthy".into();
                    } else {
                        device.update_status = "VersionIgnored".into();
                        device.device_state = "Healthy".into();
                    }
                }
            }
            current.summary = summarize(&current.devices, current.summary.update_offer_count as usize);
        }
        Ok(current.clone())
    }

    fn current_owner(&self) -> String {
        self.inner.owner_principal_key.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }

    // DBT-P46-B19: a DB read failure here used to be indistinguishable from
    // "this owner has no saved overrides" — .unwrap_or_default() silently
    // dropped the user's own saved driver-update preferences (e.g. "ignore
    // this update") with no signal anywhere. Now returns the load failure as
    // a warning string, threaded into DriverHubSnapshot.warnings by both
    // callers, the same way a Windows Update discovery failure already is
    // two lines above each call site.
    fn load_overrides(&self, owner: &str) -> (Vec<DriverOverride>, Option<String>) {
        let Some(database) = self.inner.database.as_ref() else { return (Vec::new(), None); };
        match database.driver_authority_overrides_for_owner(owner, Utc::now().timestamp_millis()) {
            Ok(rows) => (rows.into_iter().filter_map(|r| {
                let kind = match r.behavior.as_str() { "IgnoreExactVersion" => DriverOverrideKind::IgnoreExactVersion, "RemindLater" => DriverOverrideKind::RemindLater, "IgnoreOptional" => DriverOverrideKind::IgnoreOptional, _ => return None };
                Some(DriverOverride { device_privacy_key:r.device_privacy_key, kind, candidate_version:r.candidate_version, authority_provider_id:r.provider_id, expires_unix_ms:r.expires_unix_ms })
            }).collect(), None),
            Err(error) => (Vec::new(), Some(format!("Saved driver overrides unavailable: {error}"))),
        }
    }

    fn persist_authority_snapshot(&self, owner: &str, snapshot: &DriverHubSnapshot) {
        let Some(database) = self.inner.database.as_ref() else { return; };
        if let Ok(snapshot_json) = serde_json::to_string(snapshot) {
            let _ = database.upsert_driver_authority_scan(&DriverAuthorityScanRecord { owner_principal_key:owner.into(), scan_id:snapshot.scan_id.clone(), inventory_epoch:snapshot.inventory_epoch, authority_coverage:snapshot.authority_coverage.clone(), snapshot_json, created_unix_ms:snapshot.completed_unix_ms });
        }
    }

    /// Passive, synchronous discovery for the idle scheduler. No intermediate `Scanning` state is
    /// published. A cancellation observed before the final owner+snapshot commit makes the result
    /// ineligible for publication, so a late platform return cannot overwrite interactive state.
    pub fn passive_scan(&self, owner_principal_key: &str, token: CancellationToken) -> Result<DriverHubSnapshot> {
        self.passive_scan_with_fence(owner_principal_key, token, CommitFence::new())
    }

    pub fn passive_scan_with_fence(&self, owner_principal_key: &str, token: CancellationToken, commit_fence: CommitFence) -> Result<DriverHubSnapshot> {
        if owner_principal_key.trim().is_empty() || token.is_cancelled() { return Err(HubError::Cancelled); }
        let (scan_id, epoch, started) = {
            let current = self.inner.snapshot.lock().unwrap_or_else(|p| p.into_inner());
            if current.state.is_running() { return Err(HubError::Busy); }
            (Uuid::new_v4().to_string(), current.inventory_epoch.saturating_add(1), Utc::now().timestamp_millis())
        };
        let devices = self.inner.backend.inventory().map_err(HubError::Inventory)?;
        if token.is_cancelled() { return Err(HubError::Cancelled); }
        let discovery = self.inner.backend.updates().unwrap_or_else(|error| DiscoveryResult {
            offers: Vec::new(), warnings: vec![format!("Windows Update discovery unavailable: {error}")],
        });
        if token.is_cancelled() { return Err(HubError::Cancelled); }
        let (overrides, overrides_warning) = self.load_overrides(owner_principal_key);
        let mut ready = match_inventory_with_overrides(scan_id, epoch, started, devices, discovery, &overrides);
        ready.state = ScanState::Ready; ready.completed_unix_ms = Utc::now().timestamp_millis();
        ready.warnings.extend(overrides_warning);
        let committed = commit_fence.try_commit_checked(|| {
            let mut owner = self.inner.owner_principal_key.lock().unwrap_or_else(|p| p.into_inner());
            let mut current = self.inner.snapshot.lock().unwrap_or_else(|p| p.into_inner());
            if current.state.is_running() || token.is_cancelled() { return None; }
            *owner = owner_principal_key.to_owned();
            *current = ready.clone();
            Some(())
        });
        if committed != Some(()) { return Err(HubError::Cancelled); }
        self.persist_authority_snapshot(owner_principal_key, &ready);
        Ok(ready)
    }

    pub fn start_scan_with_lease(
        &self,
        owner_principal_key: &str,
        lease: ReadBudgetLease,
    ) -> Result<DriverHubSnapshot> {
        if !lease.matches(ReadWorkload::DriverDiscovery) {
            return Err(HubError::Inventory("read budget lease identity mismatch".into()));
        }
        self.start_scan_inner(owner_principal_key, lease)
    }

    fn start_scan_inner(
        &self,
        owner_principal_key: &str,
        read_budget_lease: ReadBudgetLease,
    ) -> Result<DriverHubSnapshot> {
        {
            let mut owner = self
                .inner
                .owner_principal_key
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let mut current = self
                .inner
                .snapshot
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if current.state.is_running() {
                return Err(HubError::Busy);
            }
            *owner = owner_principal_key.to_owned();
            let next_epoch = current.inventory_epoch.saturating_add(1);
            *current = DriverHubSnapshot {
                scan_id: Uuid::new_v4().to_string(),
                state: ScanState::InventoryScanning,
                inventory_epoch: next_epoch,
                started_unix_ms: Utc::now().timestamp_millis(),
                ..DriverHubSnapshot::default()
            };
        }

        let initial = self.snapshot_for_owner(owner_principal_key)?;
        let hub = self.clone();
        if let Err(error) = thread::Builder::new()
            .name("aether-driver-discovery".into())
            .spawn(move || {
                let _read_budget_lease = read_budget_lease;
                hub.run_scan();
            })
        {
            let detail = format!("driver discovery worker unavailable: {error}");
            self.fail_scan(detail.clone());
            return Err(HubError::Inventory(detail));
        }
        Ok(initial)
    }

    fn run_scan(&self) {
        let base = self.snapshot();
        let devices = match self.inner.backend.inventory() {
            Ok(devices) => devices,
            Err(error) => {
                self.fail_scan(format!("PnP inventory failed: {error}"));
                return;
            }
        };

        self.update_state(ScanState::UpdateSearching);
        let discovery = match self.inner.backend.updates() {
            Ok(result) => result,
            Err(DiscoveryFailure::Offline(error)) => DiscoveryResult {
                offers: Vec::new(),
                warnings: vec![format!("Windows Update discovery offline: {error}")],
            },
            Err(DiscoveryFailure::Unavailable(error)) => DiscoveryResult {
                offers: Vec::new(),
                warnings: vec![format!("Windows Update discovery unavailable: {error}")],
            },
        };

        self.update_state(ScanState::Matching);
        let owner = self.current_owner();
        let (overrides, overrides_warning) = self.load_overrides(&owner);
        let mut ready = match_inventory_with_overrides(
            base.scan_id, base.inventory_epoch, base.started_unix_ms, devices, discovery, &overrides,
        );
        ready.state = ScanState::Ready;
        ready.completed_unix_ms = Utc::now().timestamp_millis();
        ready.warnings.extend(overrides_warning);
        self.persist_authority_snapshot(&owner, &ready);
        let mut current = self.inner.snapshot.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        *current = ready;
    }

    fn update_state(&self, state: ScanState) {
        let mut current = self
            .inner
            .snapshot
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        current.state = state;
    }

    fn fail_scan(&self, message: String) {
        let mut current = self
            .inner
            .snapshot
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        current.state = ScanState::Failed;
        current.error_message = message;
        current.completed_unix_ms = Utc::now().timestamp_millis();
    }
}

pub fn match_inventory(scan_id: String, inventory_epoch: u64, started_unix_ms: i64, devices: Vec<DeviceRecord>, discovery: DiscoveryResult) -> DriverHubSnapshot {
    match_inventory_with_overrides(scan_id, inventory_epoch, started_unix_ms, devices, discovery, &[])
}

fn match_inventory_with_overrides(
    scan_id: String,
    inventory_epoch: u64,
    started_unix_ms: i64,
    devices: Vec<DeviceRecord>,
    discovery: DiscoveryResult,
    overrides: &[DriverOverride],
) -> DriverHubSnapshot {
    let mut hardware_map: HashMap<String, Vec<usize>> = HashMap::new();
    let mut compatible_map: HashMap<String, Vec<usize>> = HashMap::new();
    for (index, device) in devices.iter().enumerate() {
        for id in &device.hardware_ids {
            let key = normalize_pnp_id(id);
            if !key.is_empty() { hardware_map.entry(key).or_default().push(index); }
        }
        for id in &device.compatible_ids {
            let key = normalize_pnp_id(id);
            if !key.is_empty() { compatible_map.entry(key).or_default().push(index); }
        }
    }

    let windows_update_state = windows_update_scan_state(&discovery);
    let registry_result = builtin_provider_registry();
    let registry = registry_result.as_ref().ok();
    let mut warnings = discovery.warnings.clone();
    if let Err(error) = &registry_result {
        warnings.push(format!("Driver provider coverage registry invalid; authority completeness is unknown: {error}"));
    }
    let gpu_policies: Vec<Option<GpuManagement>> = devices.iter().map(gpu_policy_for_device).collect();
    let mut normalized: Vec<Vec<DriverCandidateV2>> = vec![Vec::new(); devices.len()];
    let mut unmatched = Vec::new();

    // Derive status flags from the raw problem code when a backend did not populate them.
    // Problem code 28 (CM_PROB_FAILED_INSTALL) is the canonical missing-driver classification.
    let devices: Vec<DeviceRecord> = devices
        .into_iter()
        .map(|mut device| {
            if device.status.problem_code != 0 && !device.status.has_problem {
                device.status.has_problem = true;
            }
            if aethercore_windows_pnp::is_missing_driver_problem(device.status.problem_code) {
                device.status.missing_driver = true;
            }
            device
        })
        .collect();

    for offer in &discovery.offers {
        let key = normalize_pnp_id(&offer.hardware_id);
        let (indexes, match_kind) = if key.is_empty() {
            (None, MatchKind::Unknown)
        } else if let Some(indexes) = hardware_map.get(&key) {
            (Some(indexes), MatchKind::HardwareId)
        } else if let Some(indexes) = compatible_map.get(&key) {
            (Some(indexes), MatchKind::CompatibleId)
        } else {
            (None, MatchKind::Unknown)
        };
        if let Some(indexes) = indexes {
            for index in indexes {
                let identity = DeviceIdentity::from_pnp(&devices[*index]);
                normalized[*index].push(WindowsUpdateProvider::from_offer(&identity, offer, match_kind));
            }
        } else {
            unmatched.push(UnmatchedOffer {
                update_id: offer.update_id.clone(), revision: offer.revision, title: offer.title.clone(),
                hardware_id: offer.hardware_id.clone(), provider: offer.provider.clone(), driver_class: offer.driver_class.clone(),
                min_download_bytes: offer.min_download_bytes, max_download_bytes: offer.max_download_bytes,
            });
        }
    }

    let machine_profile = MachineProfile::collect_local();
    let mut result_devices = Vec::with_capacity(devices.len());
    for (index, device) in devices.into_iter().enumerate() {
        let identity = DeviceIdentity::from_pnp(&device);
        let (required, evaluations, coverage_detail) = if let Some(registry) = registry {
            let required = required_authorities(&identity, &machine_profile, registry);
            let evaluations = evaluate_required_authorities(&required, registry, windows_update_state);
            let coverage = DeviceAuthorityCoverage::from_evaluations(&required, &evaluations);
            (required, evaluations, coverage)
        } else {
            (Vec::new(), Vec::new(), DeviceAuthorityCoverage { completeness: CoverageState::Unknown, ..Default::default() })
        };

        let mut management: Vec<DriverManagementAuthorityInfo> = Vec::new();
        let gpu = gpu_policies[index].clone();
        if let Some(gpu_info) = &gpu {
            let guidance = OfficialUtilityProvider { vendor: gpu_info.vendor }
                .management_authority(&identity, gpu_info.app_installed);
            management.push(management_info(guidance));
        }
        if let Some(registry) = registry {
            for provider_id in coverage_detail.manual_authorities.iter().chain(coverage_detail.unsupported_authorities.iter()) {
                if management.iter().any(|m| &m.provider_id == provider_id) { continue; }
                if let Some(guidance) = management_authority_for_registry_provider(provider_id, registry) {
                    management.push(management_info(guidance));
                }
            }
        }
        management.sort_by(|a,b| a.provider_id.cmp(&b.provider_id));
        management.dedup_by(|a,b| a.provider_id == b.provider_id);

        let decision = DriverAuthorityEngine::evaluate(
            &identity,
            &machine_profile,
            std::mem::take(&mut normalized[index]),
            overrides,
            coverage_detail.completeness,
            Utc::now().timestamp_millis(),
        );
        let recommended_candidate_id = decision.recommended_candidate_id.clone();
        let mut device_candidates: Vec<DriverCandidate> = decision.candidates.into_iter().map(driver_candidate_from_v2).collect();
        device_candidates.sort_by(|a,b| {
            recommendation_priority(a).cmp(&recommendation_priority(b))
                .then_with(|| match_priority(a).cmp(&match_priority(b)))
                .then_with(|| a.authority_name.cmp(&b.authority_name))
                .then_with(|| a.title.cmp(&b.title))
                .then_with(|| a.candidate_id.cmp(&b.candidate_id))
        });
        device_candidates.dedup_by(|a,b| a.candidate_id == b.candidate_id);
        let display_managed = is_display_adapter(&device) && gpu.is_some();
        let update_status = if device.status.missing_driver && recommended_candidate_id.is_empty() {
            "NoTrustedCandidate"
        } else if device.status.missing_driver {
            "MissingDriverCandidateAvailable"
        } else if device.status.has_problem && recommended_candidate_id.is_empty() {
            "DeviceProblemNoTrustedCandidate"
        } else if !recommended_candidate_id.is_empty() {
            "RecommendedUpdateAvailable"
        } else {
            match coverage_detail.completeness {
                CoverageState::CompleteForRequiredAuthorities if coverage_detail.permits_strong_up_to_date() => "UpToDate",
                CoverageState::ManualAuthorityRequired | CoverageState::Partial => "NoUpdateFoundFromCheckedSources",
                CoverageState::Offline => "UpdateStatusUnknownOffline",
                CoverageState::ProviderUnavailable => "ProviderUnavailable",
                CoverageState::Unknown | CoverageState::CompleteForRequiredAuthorities => "UpdateStatusUnknown",
            }
        }.to_string();
        let device_state = if device.status.missing_driver { "MissingDriver" }
            else if device.status.has_problem { "Problem" }
            else if device_candidates.iter().any(|c| c.firmware_managed) { "FirmwareManualReview" }
            else if !recommended_candidate_id.is_empty() { "UpdateAvailable" }
            else { "Healthy" }.to_string();

        result_devices.push(DriverDevice {
            instance_id: device.instance_id, display_name: device.display_name, description: device.description,
            class_name: device.class_name, class_guid: device.class_guid, manufacturer: device.manufacturer,
            enumerator: device.enumerator, location: device.location, hardware_ids: device.hardware_ids,
            compatible_ids: device.compatible_ids, raw_status: device.status.raw_status, problem_code: device.status.problem_code,
            has_problem: device.status.has_problem, missing_driver: device.status.missing_driver, driver: device.driver,
            device_state, gpu, display_managed, recommended_candidate_id, update_status,
            authority_coverage: coverage_label(coverage_detail.completeness).into(),
            required_authorities: coverage_detail.required_authorities,
            evaluated_authorities: coverage_detail.evaluated_authorities,
            unavailable_authorities: coverage_detail.unavailable_authorities,
            unsupported_authorities: coverage_detail.unsupported_authorities,
            manual_authorities: coverage_detail.manual_authorities,
            management_authorities: management,
            candidates: device_candidates,
        });
        let _ = (required, evaluations); // retained as typed source evidence for future provider adapters.
    }
    let summary = summarize(&result_devices, discovery.offers.len());
    let global_coverage = aggregate_device_coverage(&result_devices, windows_update_state);
    let mut provider_status = provider_status_from_devices(&result_devices, windows_update_state);
    provider_status.sort();
    provider_status.dedup();
    DriverHubSnapshot {
        scan_id, state: ScanState::Matching, inventory_epoch, started_unix_ms, completed_unix_ms: 0,
        error_message: String::new(), summary, devices: result_devices, unmatched_offers: unmatched,
        warnings, authority_coverage: coverage_label(global_coverage).into(), provider_status,
    }
}

fn windows_update_scan_state(discovery: &DiscoveryResult) -> CoverageState {
    let offline = discovery.warnings.iter().any(|warning| warning.starts_with("Windows Update discovery offline:"));
    let unavailable = discovery.warnings.iter().any(|warning| warning.starts_with("Windows Update discovery unavailable:"));
    if offline { CoverageState::Offline }
    else if unavailable { CoverageState::ProviderUnavailable }
    else if discovery.warnings.is_empty() { CoverageState::CompleteForRequiredAuthorities }
    else { CoverageState::Partial }
}

fn aggregate_device_coverage(devices: &[DriverDevice], fallback: CoverageState) -> CoverageState {
    if devices.is_empty() { return fallback; }
    let states: Vec<&str> = devices.iter().map(|d| d.authority_coverage.as_str()).collect();
    if states.iter().any(|s| *s == "Offline") { CoverageState::Offline }
    else if states.iter().any(|s| *s == "ProviderUnavailable") { CoverageState::ProviderUnavailable }
    else if states.iter().any(|s| *s == "Unknown") { CoverageState::Unknown }
    else if states.iter().any(|s| *s == "Partial") { CoverageState::Partial }
    else if states.iter().any(|s| *s == "ManualAuthorityRequired") { CoverageState::ManualAuthorityRequired }
    else { CoverageState::CompleteForRequiredAuthorities }
}

fn provider_status_from_devices(devices: &[DriverDevice], wua: CoverageState) -> Vec<String> {
    let mut out = vec![format!("microsoft.windows-update:{}", match wua {
        CoverageState::CompleteForRequiredAuthorities => "Evaluated",
        CoverageState::Partial => "Partial",
        CoverageState::Offline => "Offline",
        CoverageState::ProviderUnavailable => "Unavailable",
        CoverageState::ManualAuthorityRequired => "ManualAuthorityRequired",
        CoverageState::Unknown => "Unknown",
    })];
    for device in devices {
        out.extend(device.manual_authorities.iter().map(|id| format!("{id}:ManualAuthorityRequired")));
        out.extend(device.unavailable_authorities.iter().filter(|id| id.as_str() != "microsoft.windows-update").map(|id| format!("{id}:Unavailable")));
        out.extend(device.unsupported_authorities.iter().map(|id| format!("{id}:UnsupportedAutomation")));
    }
    out
}

fn management_info(authority: DriverManagementAuthority) -> DriverManagementAuthorityInfo {
    DriverManagementAuthorityInfo {
        provider_id: authority.provider_id,
        authority_type: authority_type_label(authority.authority_type).into(),
        display_name: authority.display_name,
        official_authority: authority.official_authority,
        official_url: authority.official_url,
        availability: management_availability_label(authority.availability).into(),
        update_availability: update_availability_label(authority.update_availability).into(),
        installation_mode: installation_capability_label(authority.installation_capability).into(),
        update_evidence_candidate_id: authority.update_evidence_candidate_id,
    }
}

fn management_availability_label(value: ManagementAuthorityAvailability) -> &'static str {
    match value { ManagementAuthorityAvailability::Available=>"Available",ManagementAuthorityAvailability::Installed=>"Installed",ManagementAuthorityAvailability::NotInstalled=>"NotInstalled",ManagementAuthorityAvailability::ProviderUnavailable=>"ProviderUnavailable" }
}
fn update_availability_label(value: UpdateAvailabilityEvidence) -> &'static str {
    match value { UpdateAvailabilityEvidence::UnknownUntilVendorCheck=>"UnknownUntilVendorCheck",UpdateAvailabilityEvidence::UpdateAvailable=>"UpdateAvailable",UpdateAvailabilityEvidence::NoUpdateReported=>"NoUpdateReported",UpdateAvailabilityEvidence::ProviderUnavailable=>"ProviderUnavailable" }
}

fn recommendation_priority(candidate: &DriverCandidate) -> u8 {
    match candidate.recommendation_state.as_str() {
        "Recommended" => 0, "Alternative" => 1, "Optional" => 2, "ManualOfficial" => 3,
        "FirmwareProtected" => 4, "CurrentDriverPreferred" => 5, _ => 6,
    }
}

fn match_priority(candidate: &DriverCandidate) -> u8 {
    match candidate.match_quality.as_str() { "Hardware ID" => 0, "Compatible ID" => 1, _ => 2 }
}

fn summarize(devices: &[DriverDevice], offer_count: usize) -> DriverHubSummary {
    let mut unique_selectable_updates = HashSet::new();
    for candidate in devices.iter().flat_map(|d| &d.candidates).filter(|c| c.selectable) {
        unique_selectable_updates.insert((candidate.candidate_id.clone(), candidate.update_id.clone(), candidate.revision));
    }
    DriverHubSummary {
        device_count: devices.len().try_into().unwrap_or(u32::MAX),
        missing_driver_count: devices.iter().filter(|d| d.missing_driver).count().try_into().unwrap_or(u32::MAX),
        problem_device_count: devices.iter().filter(|d| d.has_problem).count().try_into().unwrap_or(u32::MAX),
        update_offer_count: offer_count.try_into().unwrap_or(u32::MAX),
        matched_device_count: devices.iter().filter(|d| !d.candidates.is_empty()).count().try_into().unwrap_or(u32::MAX),
        matched_candidate_count: devices.iter().map(|d| d.candidates.len()).sum::<usize>().try_into().unwrap_or(u32::MAX),
        selectable_update_count: unique_selectable_updates.len().try_into().unwrap_or(u32::MAX),
        vendor_managed_gpu_count: devices.iter().filter(|d| d.gpu.is_some()).count().try_into().unwrap_or(u32::MAX),
        vendor_managed_display_count: devices.iter().filter(|d| d.display_managed).count().try_into().unwrap_or(u32::MAX),
        recommended_update_count: devices.iter().filter(|d| !d.recommended_candidate_id.is_empty() && d.candidates.iter().any(|c| c.recommended && c.selectable)).count().try_into().unwrap_or(u32::MAX),
        optional_update_count: devices.iter().flat_map(|d| &d.candidates).filter(|c| c.recommendation_state == "Optional").count().try_into().unwrap_or(u32::MAX),
        vendor_managed_update_count: devices.iter().flat_map(|d| &d.management_authorities)
            .filter(|m| m.update_availability == "UpdateAvailable" && !m.update_evidence_candidate_id.is_empty())
            .count().try_into().unwrap_or(u32::MAX),
        status_unknown_count: devices.iter().filter(|d| matches!(d.update_status.as_str(),
            "NoUpdateFoundFromCheckedSources" | "UpdateStatusUnknownOffline" | "ProviderUnavailable" | "UpdateStatusUnknown"))
            .count().try_into().unwrap_or(u32::MAX),
        management_authority_count: devices.iter().map(|d| d.management_authorities.len()).sum::<usize>().try_into().unwrap_or(u32::MAX),
    }
}

fn driver_candidate_from_v2(candidate: DriverCandidateV2) -> DriverCandidate {
    let target_version_source = if candidate.update_id.is_empty() { "ProviderMetadata" } else { "WuaMetadata" };
    let recommendation_state = recommendation_state_label(candidate.recommendation_state).to_string();
    let installation_mode = installation_capability_label(candidate.installation_mode).to_string();
    let acquisition_mode = acquisition_capability_label(candidate.acquisition_mode).to_string();
    // DirectTrusted acquisition is modeled and verified in Phase 18, but privileged direct-package
    // execution is deliberately not exposed until a provider-specific installer contract is qualified.
    let selectable = candidate.executable() && candidate.installation_mode == InstallationCapability::WindowsManaged;
    DriverCandidate {
        candidate_id: candidate.candidate_id, update_id: candidate.update_id, revision: candidate.revision, title: candidate.title,
        provider: candidate.provenance.package_publisher.clone(), manufacturer: candidate.provenance.package_publisher.clone(), model: String::new(),
        driver_class: candidate.driver_class, matched_hardware_id: candidate.matched_id,
        match_quality: match candidate.match_kind { MatchKind::HardwareId => "Hardware ID", MatchKind::CompatibleId => "Compatible ID", MatchKind::VendorFamily => "Vendor family", MatchKind::DeviceClass => "Device class", MatchKind::Unknown => "Unknown" }.into(),
        driver_date_iso: candidate.driver_date_iso, min_download_bytes: candidate.min_download_bytes, max_download_bytes: candidate.max_download_bytes,
        target_version: candidate.target_version, target_version_source: target_version_source.into(), selectable,
        selected_by_default: selectable && candidate.recommendation_state == RecommendationState::Recommended,
        vendor_managed: candidate.vendor_managed, firmware_managed: candidate.firmware,
        selection_policy: if candidate.firmware { "FirmwareManualReview" } else if candidate.vendor_managed { "OfficialVendorUtility" } else if selectable { "SelectableRecommended" } else { "ReviewOnly" }.into(),
        authority_type: authority_type_label(candidate.authority.authority_type).into(), authority_provider_id: candidate.authority.provider_id,
        authority_name: candidate.authority.display_name, official_source: candidate.provenance.official_source,
        applicability: applicability_label(candidate.applicability).into(), trust_state: trust_state_label(candidate.trust_state).into(),
        recommendation_state, recommendation_reasons: candidate.recommendation_reasons.into_iter().map(|r| r.as_str().to_string()).collect(),
        acquisition_mode, installation_mode, official_support_url: candidate.official_support_url,
        recommended: candidate.recommendation_state == RecommendationState::Recommended,
    }
}

fn authority_type_label(v: AuthorityType)->&'static str { match v { AuthorityType::WindowsUpdate=>"WindowsUpdate",AuthorityType::Oem=>"Oem",AuthorityType::ComponentVendor=>"ComponentVendor",AuthorityType::VendorUtility=>"VendorUtility",AuthorityType::ManualOfficial=>"ManualOfficial" } }
fn applicability_label(v: ApplicabilityState)->&'static str { match v { ApplicabilityState::Exact=>"Exact",ApplicabilityState::Compatible=>"Compatible",ApplicabilityState::Contextual=>"Contextual",ApplicabilityState::Incompatible=>"Incompatible",ApplicabilityState::Unknown=>"Unknown" } }
fn trust_state_label(v: PackageTrustState)->&'static str { match v { PackageTrustState::WindowsManaged=>"WindowsManaged",PackageTrustState::TrustedSignature=>"TrustedSignature",PackageTrustState::TrustedSignatureAndDigest=>"TrustedSignatureAndDigest",PackageTrustState::UnexpectedPublisher=>"UnexpectedPublisher",PackageTrustState::InvalidSignature=>"InvalidSignature",PackageTrustState::Unsigned=>"Unsigned",PackageTrustState::Unverifiable=>"Unverifiable",PackageTrustState::TestSigned=>"TestSigned",PackageTrustState::NotApplicable=>"NotApplicable" } }
fn recommendation_state_label(v: RecommendationState)->&'static str { match v { RecommendationState::Recommended=>"Recommended",RecommendationState::Alternative=>"Alternative",RecommendationState::Optional=>"Optional",RecommendationState::NotRecommended=>"NotRecommended",RecommendationState::ManualOfficial=>"ManualOfficial",RecommendationState::FirmwareProtected=>"FirmwareProtected",RecommendationState::CurrentDriverPreferred=>"CurrentDriverPreferred" } }
fn installation_capability_label(v: InstallationCapability)->&'static str { match v { InstallationCapability::WindowsManaged=>"WindowsManaged",InstallationCapability::DirectTrusted=>"DirectTrusted",InstallationCapability::OfficialUtility=>"OfficialUtility",InstallationCapability::ManualOfficial=>"ManualOfficial",InstallationCapability::Unsupported=>"Unsupported" } }
fn acquisition_capability_label(v: AcquisitionCapability)->&'static str { match v { AcquisitionCapability::WindowsManaged=>"WindowsManaged",AcquisitionCapability::DirectTrusted=>"DirectTrusted",AcquisitionCapability::OfficialUtility=>"OfficialUtility",AcquisitionCapability::ManualOfficial=>"ManualOfficial",AcquisitionCapability::Unsupported=>"Unsupported" } }
fn coverage_label(v: CoverageState)->&'static str { match v { CoverageState::CompleteForRequiredAuthorities=>"CompleteForRequiredAuthorities",CoverageState::Partial=>"Partial",CoverageState::Offline=>"Offline",CoverageState::ProviderUnavailable=>"ProviderUnavailable",CoverageState::ManualAuthorityRequired=>"ManualAuthorityRequired",CoverageState::Unknown=>"Unknown" } }

fn is_firmware_device(device: &DeviceRecord) -> bool {
    device.class_name.eq_ignore_ascii_case("Firmware")
        || device
            .class_guid
            .eq_ignore_ascii_case("{F2E7DD72-6468-4E36-B6F1-6488F42C1B52}")
}

fn gpu_policy_for_device(device: &DeviceRecord) -> Option<GpuManagement> {
    if !is_display_adapter(device) {
        return None;
    }
    let vendor = detect_gpu_vendor(device)?;
    let definition = gpu_vendor_policy(vendor);

    let app_installed = installed_app_path(vendor).is_some();
    Some(GpuManagement {
        vendor,
        provider_id: match vendor { GpuVendor::Nvidia=>"component.nvidia",GpuVendor::Amd=>"component.amd",GpuVendor::Intel=>"component.intel" }.into(),
        app_installed,
        app_name: definition.app_name.to_string(),
        official_url: definition.official_url.to_string(),
        management_status: if app_installed { "Installed" } else { "NotInstalled" }.into(),
        update_availability: "UnknownUntilVendorCheck".into(),
    })
}

fn is_display_adapter(device: &DeviceRecord) -> bool {
    device.class_name.eq_ignore_ascii_case("Display")
        || device
            .class_guid
            .eq_ignore_ascii_case("{4D36E968-E325-11CE-BFC1-08002BE10318}")
}

fn detect_gpu_vendor(device: &DeviceRecord) -> Option<GpuVendor> {
    let ids = device
        .hardware_ids
        .iter()
        .chain(device.compatible_ids.iter())
        .map(|id| normalize_pnp_id(id))
        .collect::<Vec<_>>();
    if ids.iter().any(|id| id.contains("VEN_10DE")) {
        Some(GpuVendor::Nvidia)
    } else if ids.iter().any(|id| id.contains("VEN_1002")) {
        Some(GpuVendor::Amd)
    } else if ids.iter().any(|id| id.contains("VEN_8086")) {
        Some(GpuVendor::Intel)
    } else {
        let manufacturer = device.manufacturer.to_ascii_lowercase();
        if manufacturer.contains("nvidia") {
            Some(GpuVendor::Nvidia)
        } else if manufacturer.contains("advanced micro devices") || manufacturer == "amd" {
            Some(GpuVendor::Amd)
        } else if manufacturer.contains("intel") {
            Some(GpuVendor::Intel)
        } else {
            None
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use aethercore_operation_kernel::ReadBudgetManager;

    const OWNER: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn start_scan_leased(hub: &DriverHub, owner: &str) -> Result<DriverHubSnapshot> {
        let budget = ReadBudgetManager::new(4);
        let lease = budget.try_acquire(ReadWorkload::DriverDiscovery).expect("read budget lease");
        hub.start_scan_with_lease(owner, lease)
    }
    use aethercore_windows_pnp::{DeviceStatus, InstalledDriver};
    use aethercore_windows_update::{DriverOffer, VersionSource};

    fn device(id: &str, class: &str) -> DeviceRecord {
        DeviceRecord {
            instance_id: format!("PCI\\INSTANCE\\{id}"),
            display_name: "Test device".into(),
            class_name: class.into(),
            class_guid: if class == "Display" {
                "{4D36E968-E325-11CE-BFC1-08002BE10318}".into()
            } else if class == "Firmware" {
                "{F2E7DD72-6468-4E36-B6F1-6488F42C1B52}".into()
            } else {
                String::new()
            },
            manufacturer: "Vendor".into(),
            hardware_ids: vec![id.into()],
            driver: Some(InstalledDriver { provider: "Vendor".into(), version: "1.0.0.0".into(), inf_path: "oem1.inf".into(), date: "01-01-2025".into() }),
            status: DeviceStatus::default(),
            ..DeviceRecord::default()
        }
    }

    fn offer(id: &str) -> DriverOffer {
        DriverOffer {
            update_id: "update-guid".into(),
            revision: 3,
            title: "Vendor - Device - 2.0.0.0".into(),
            hardware_id: id.into(),
            provider: "Vendor".into(),
            target_version: "2.0.0.0".into(),
            target_version_source: VersionSource::TitleHeuristic,
            min_download_bytes: 10,
            max_download_bytes: 20,
            ..DriverOffer::default()
        }
    }

    #[test]
    fn d18_01_healthy_current_device_is_up_to_date_only_when_required_authorities_complete() {
        let snapshot=match_inventory("scan".into(),1,1,vec![device("PCI\\VEN_ABCD&DEV_0001","Net")],DiscoveryResult{offers:vec![],warnings:vec![]});
        let d=&snapshot.devices[0];
        assert_eq!(d.authority_coverage,"CompleteForRequiredAuthorities");
        assert_eq!(d.update_status,"UpToDate");
        assert!(d.recommended_candidate_id.is_empty());
        assert_eq!(snapshot.summary.recommended_update_count,0);
    }

    #[test]
    fn d18_02_missing_driver_with_exact_wua_offer_has_concrete_recommendation() {
        let mut d=device("PCI\\VEN_ABCD&DEV_0002","Net");
        d.driver=None; d.status.problem_code=28;
        let snapshot=match_inventory("scan".into(),1,1,vec![d],DiscoveryResult{offers:vec![offer("PCI\\VEN_ABCD&DEV_0002")],warnings:vec![]});
        let d=&snapshot.devices[0];
        assert!(d.missing_driver);
        assert_eq!(d.update_status,"MissingDriverCandidateAvailable");
        assert_eq!(d.candidates.len(),1);
        assert!(d.candidates[0].recommended);
        assert_eq!(d.candidates[0].installation_mode,"WindowsManaged");
    }

    #[test]
    fn exact_hardware_id_match_is_case_insensitive() {
        let snapshot = match_inventory(
            "scan".into(),
            1,
            1,
            vec![device("PCI\\VEN_1234&DEV_5678", "Net")],
            DiscoveryResult { offers: vec![offer("pci\\ven_1234&dev_5678")], warnings: vec![] },
        );
        assert_eq!(snapshot.devices[0].candidates.len(), 1);
        assert_eq!(snapshot.devices[0].candidates[0].match_quality, "Hardware ID");
        assert!(snapshot.devices[0].candidates[0].selectable);
    }

    #[test]
    fn compatible_id_is_used_only_when_hardware_id_does_not_match() {
        let mut d = device("PCI\\VEN_1234&DEV_9999", "Net");
        d.compatible_ids = vec!["PCI\\VEN_1234&CC_0200".into()];
        let snapshot = match_inventory(
            "scan".into(), 1, 1, vec![d],
            DiscoveryResult { offers: vec![offer("PCI\\VEN_1234&CC_0200")], warnings: vec![] },
        );
        assert_eq!(snapshot.devices[0].candidates[0].match_quality, "Compatible ID");
    }

    #[test]
    fn exact_hardware_entry_wins_when_same_update_also_matches_compatible_id() {
        let mut d = device("PCI\\VEN_1234&DEV_9999", "Net");
        d.compatible_ids = vec!["PCI\\VEN_1234&CC_0200".into()];

        let exact = offer("PCI\\VEN_1234&DEV_9999");
        let compatible = offer("PCI\\VEN_1234&CC_0200");
        let snapshot = match_inventory(
            "scan".into(),
            1,
            1,
            vec![d],
            DiscoveryResult {
                // Put the compatible entry first to prove output does not depend on WUA order.
                offers: vec![compatible, exact],
                warnings: vec![],
            },
        );
        assert_eq!(snapshot.devices[0].candidates.len(), 1);
        assert_eq!(snapshot.devices[0].candidates[0].match_quality, "Hardware ID");
        assert_eq!(
            snapshot.devices[0].candidates[0].matched_hardware_id,
            "PCI\\VEN_1234&DEV_9999"
        );
    }

    #[test]
    fn unmatched_offer_is_not_guessed_onto_a_device() {
        let snapshot = match_inventory(
            "scan".into(), 1, 1,
            vec![device("PCI\\VEN_1234&DEV_5678", "Net")],
            DiscoveryResult { offers: vec![offer("PCI\\VEN_AAAA&DEV_BBBB")], warnings: vec![] },
        );
        assert!(snapshot.devices[0].candidates.is_empty());
        assert_eq!(snapshot.unmatched_offers.len(), 1);
    }

    #[derive(Default)]
    struct FakeBackend;
    impl DiscoveryBackend for FakeBackend {
        fn inventory(&self) -> std::result::Result<Vec<DeviceRecord>, String> {
            Ok(vec![device("PCI\\VEN_1234&DEV_5678", "Net")])
        }
        fn updates(&self) -> std::result::Result<DiscoveryResult, DiscoveryFailure> {
            Ok(DiscoveryResult { offers: vec![offer("PCI\\VEN_1234&DEV_5678")], warnings: vec![] })
        }
    }

    #[test]
    fn firmware_offers_are_visible_but_never_selectable() {
        let mut firmware_offer = offer("UEFI\\RES_{12345678-1234-1234-1234-123456789ABC}");
        firmware_offer.driver_class = "Firmware".into();
        let d = device("UEFI\\RES_{12345678-1234-1234-1234-123456789ABC}", "Firmware");
        let snapshot = match_inventory(
            "scan".into(),
            1,
            1,
            vec![d],
            DiscoveryResult { offers: vec![firmware_offer], warnings: vec![] },
        );
        let candidate = &snapshot.devices[0].candidates[0];
        assert!(candidate.firmware_managed);
        assert!(!candidate.selectable);
        assert!(!candidate.selected_by_default);
        assert_eq!(candidate.selection_policy, "FirmwareManualReview");
        assert_eq!(snapshot.devices[0].device_state, "FirmwareManualReview");
    }

    #[test]
    fn d18_04_known_nvidia_display_keeps_management_separate_from_wua_package() {
        let d = device("PCI\\VEN_10DE&DEV_2684", "Display");
        let snapshot = match_inventory(
            "scan".into(), 1, 1, vec![d],
            DiscoveryResult { offers: vec![offer("PCI\\VEN_10DE&DEV_2684")], warnings: vec![] },
        );
        let device = &snapshot.devices[0];
        assert_eq!(device.gpu.as_ref().unwrap().vendor, GpuVendor::Nvidia);
        assert!(device.display_managed);
        assert_eq!(device.candidates.len(), 1);
        assert_eq!(device.candidates[0].authority_type, "WindowsUpdate");
        assert_eq!(device.candidates[0].recommendation_state, "Recommended");
        assert!(device.candidates[0].selectable);
        assert_eq!(device.management_authorities.len(), 1);
        assert_eq!(device.management_authorities[0].provider_id, "component.nvidia");
        assert_eq!(device.management_authorities[0].update_availability, "UnknownUntilVendorCheck");
        assert_eq!(device.update_status, "RecommendedUpdateAvailable");
    }

    #[test]
    fn d18_1_gpu_01_nvidia_utility_without_update_evidence_never_fabricates_update() {
        let d = device("PCI\\VEN_10DE&DEV_2684", "Display");
        let snapshot = match_inventory("scan".into(),1,1,vec![d],DiscoveryResult{offers:vec![],warnings:vec![]});
        let device=&snapshot.devices[0];
        assert!(device.candidates.is_empty());
        assert!(device.recommended_candidate_id.is_empty());
        assert_ne!(device.update_status,"RecommendedUpdateAvailable");
        assert_eq!(device.update_status,"NoUpdateFoundFromCheckedSources");
        assert_eq!(device.management_authorities[0].update_availability,"UnknownUntilVendorCheck");
        assert_eq!(snapshot.summary.recommended_update_count,0);
        assert_eq!(snapshot.summary.vendor_managed_update_count,0);
        assert_eq!(snapshot.summary.management_authority_count,1);
    }

    #[test]
    fn d18_1_gpu_01_amd_and_intel_follow_same_no_update_truth_rule() {
        for hardware_id in ["PCI\\VEN_1002&DEV_73BF","PCI\\VEN_8086&DEV_46A6"] {
            let snapshot=match_inventory("scan".into(),1,1,vec![device(hardware_id,"Display")],DiscoveryResult{offers:vec![],warnings:vec![]});
            let d=&snapshot.devices[0];
            assert!(d.candidates.is_empty());
            assert!(d.recommended_candidate_id.is_empty());
            assert_ne!(d.update_status,"RecommendedUpdateAvailable");
            assert_eq!(d.management_authorities.len(),1);
            assert_eq!(d.management_authorities[0].update_availability,"UnknownUntilVendorCheck");
        }
    }

    #[test]
    fn unknown_display_vendor_can_use_exact_trusted_windows_update_authority() {
        let d = device("PCI\\VEN_ABCD&DEV_0001", "Display");
        let snapshot = match_inventory(
            "scan".into(),
            1,
            1,
            vec![d],
            DiscoveryResult {
                offers: vec![offer("PCI\\VEN_ABCD&DEV_0001")],
                warnings: vec![],
            },
        );
        let device = &snapshot.devices[0];
        assert!(device.gpu.is_none());
        assert!(!device.display_managed);
        assert_eq!(device.device_state, "UpdateAvailable");
        assert_eq!(device.candidates.len(), 1);
        assert_eq!(device.candidates[0].authority_type, "WindowsUpdate");
        assert_eq!(device.candidates[0].recommendation_state, "Recommended");
        assert!(device.candidates[0].selectable);
        assert!(device.candidates[0].selected_by_default);
        assert_eq!(snapshot.summary.vendor_managed_display_count, 0);
    }

    #[test]
    fn wua_partial_warning_marks_partial_coverage() {
        let snapshot = match_inventory(
            "scan".into(), 1, 1,
            vec![device("PCI\\VEN_1234&DEV_5678", "Net")],
            DiscoveryResult { offers: vec![offer("PCI\\VEN_1234&DEV_5678")], warnings: vec!["WUA item 0 metadata was rejected".into()] },
        );
        assert_eq!(snapshot.authority_coverage, "Partial");
        assert_eq!(snapshot.provider_status, vec!["microsoft.windows-update:Partial"]);
        assert_eq!(snapshot.devices[0].authority_coverage, "Partial");
    }

    #[test]
    fn coordinator_reaches_ready_without_mutation_actions() {
        let hub = DriverHub::with_backend(Arc::new(FakeBackend));
        let started = start_scan_leased(&hub, OWNER).unwrap();
        assert_eq!(started.state, ScanState::InventoryScanning);
        for _ in 0..100 {
            let snapshot = hub.snapshot();
            if snapshot.state == ScanState::Ready {
                assert_eq!(snapshot.summary.device_count, 1);
                assert_eq!(snapshot.summary.selectable_update_count, 1);
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        panic!("scan did not reach Ready");
    }

    struct SlowBackend;
    impl DiscoveryBackend for SlowBackend {
        fn inventory(&self) -> std::result::Result<Vec<DeviceRecord>, String> {
            std::thread::sleep(std::time::Duration::from_millis(75));
            Ok(vec![device("PCI\\VEN_1234&DEV_5678", "Net")])
        }

        fn updates(&self) -> std::result::Result<DiscoveryResult, DiscoveryFailure> {
            Ok(DiscoveryResult::default())
        }
    }

    #[test]
    fn driver_snapshot_is_principal_bound() {
        let hub = DriverHub::with_backend(Arc::new(FakeBackend));
        start_scan_leased(&hub, OWNER).unwrap();
        for _ in 0..100 {
            if hub.snapshot_for_owner(OWNER).unwrap().state == ScanState::Ready {
                let other = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
                assert!(matches!(hub.snapshot_for_owner(other), Err(HubError::OwnershipMismatch)));
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        panic!("scan did not reach Ready");
    }

    #[test]
    fn overlapping_scan_is_rejected_while_inventory_is_running() {
        let hub = DriverHub::with_backend(Arc::new(SlowBackend));
        start_scan_leased(&hub, OWNER).unwrap();
        assert!(matches!(start_scan_leased(&hub, OWNER), Err(HubError::Busy)));
    }

    struct InventoryFailureBackend;
    impl DiscoveryBackend for InventoryFailureBackend {
        fn inventory(&self) -> std::result::Result<Vec<DeviceRecord>, String> {
            Err("simulated SetupAPI failure".into())
        }

        fn updates(&self) -> std::result::Result<DiscoveryResult, DiscoveryFailure> {
            unreachable!("WUA must not run after inventory failure")
        }
    }

    #[test]
    fn inventory_failure_transitions_scan_to_failed() {
        let hub = DriverHub::with_backend(Arc::new(InventoryFailureBackend));
        start_scan_leased(&hub, OWNER).unwrap();
        for _ in 0..100 {
            let snapshot = hub.snapshot();
            if snapshot.state == ScanState::Failed {
                assert!(snapshot.error_message.contains("PnP inventory failed"));
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        panic!("scan did not reach Failed");
    }

    struct OfflineBackend;
    impl DiscoveryBackend for OfflineBackend {
        fn inventory(&self) -> std::result::Result<Vec<DeviceRecord>, String> {
            Ok(vec![device("PCI\\VEN_1234&DEV_5678", "Net")])
        }
        fn updates(&self) -> std::result::Result<DiscoveryResult, DiscoveryFailure> {
            Err(DiscoveryFailure::Offline("WU_E_NO_CONNECTION".into()))
        }
    }

    #[test]
    fn offline_scan_keeps_inventory_and_marks_update_truth_unknown() {
        let hub = DriverHub::with_backend(Arc::new(OfflineBackend));
        start_scan_leased(&hub, OWNER).unwrap();
        for _ in 0..100 {
            let snapshot = hub.snapshot();
            if snapshot.state == ScanState::Ready {
                assert_eq!(snapshot.authority_coverage, "Offline");
                assert_eq!(snapshot.devices[0].update_status, "UpdateStatusUnknownOffline");
                assert_eq!(snapshot.provider_status, vec!["microsoft.windows-update:Offline"]);
                assert!(snapshot.warnings.iter().any(|warning| warning.contains("discovery offline")));
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        panic!("scan did not reach Ready while offline");
    }

    struct UpdateFailureBackend;
    impl DiscoveryBackend for UpdateFailureBackend {
        fn inventory(&self) -> std::result::Result<Vec<DeviceRecord>, String> {
            Ok(vec![device("PCI\\VEN_1234&DEV_5678", "Net")])
        }

        fn updates(&self) -> std::result::Result<DiscoveryResult, DiscoveryFailure> {
            Err(DiscoveryFailure::Unavailable("simulated WUA policy failure".into()))
        }
    }

    #[test]
    fn wua_failure_keeps_inventory_and_completes_with_warning() {
        let hub = DriverHub::with_backend(Arc::new(UpdateFailureBackend));
        start_scan_leased(&hub, OWNER).unwrap();
        for _ in 0..100 {
            let snapshot = hub.snapshot();
            if snapshot.state == ScanState::Ready {
                assert_eq!(snapshot.summary.device_count, 1);
                assert_eq!(snapshot.summary.update_offer_count, 0);
                assert_eq!(snapshot.authority_coverage, "ProviderUnavailable");
                assert_eq!(snapshot.devices[0].update_status, "ProviderUnavailable");
                assert_eq!(snapshot.summary.status_unknown_count, 1);
                assert!(snapshot
                    .warnings
                    .iter()
                    .any(|warning| warning.contains("Windows Update discovery unavailable")));
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        panic!("scan did not reach Ready after WUA failure");
    }
}
