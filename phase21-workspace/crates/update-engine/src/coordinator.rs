use std::{
    collections::{HashMap, HashSet},
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

use aethercore_operation_kernel::{MutationLease, MutationSupervisor, MutationWorkload};
use aethercore_persistence::{Database, UpdateExecutionGuardRecord, UpdateManifestFloorRecord};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    manifest::{
        self, MAX_MANIFEST_BYTES, MAX_SIGNATURE_BYTES, UpdateManifestRelease, UpdatePackageKind,
        UpdateTrustConfig, validate_trust_config, verify_manifest_bytes,
    },
    platform::{PlatformVerifier, current_windows_build, default_platform_verifier},
};

const INTENT_TTL_MS: i64 = 5 * 60 * 1000;
const EXECUTION_TTL_MS: i64 = 2 * 60 * 60 * 1000;
pub const MAX_STAGE_CHUNK_BYTES: u32 = 256 * 1024;
const MAX_OWNER_STATE_CACHE: usize = 128;
const UPLOAD_TTL_MS: i64 = 30 * 60 * 1000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum UpdateChannel {
    Stable,
    Beta,
}
impl UpdateChannel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stable => "stable",
            Self::Beta => "beta",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum UpdateState {
    Disabled,
    Idle,
    Checking,
    UpToDate,
    Available,
    Staging,
    Staged,
    AwaitingConsent,
    Installing,
    Completed,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateReleaseView {
    pub release_id: String,
    pub version: String,
    pub channel: UpdateChannel,
    pub published_unix_ms: i64,
    pub notes_message_key: String,
    pub minimum_windows_build: u32,
    pub size_bytes: u64,
    pub sha256: String,
    pub package_kind: UpdatePackageKind,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpdateSnapshot {
    pub state: UpdateState,
    pub channel: UpdateChannel,
    pub current_version: String,
    pub latest_release: Option<UpdateReleaseView>,
    pub staged_release: Option<UpdateReleaseView>,
    pub progress_known: bool,
    pub overall_percent: u32,
    pub bytes_completed: u64,
    pub bytes_total: u64,
    pub status_message_key: String,
    pub checked_unix_ms: i64,
    pub updated_unix_ms: i64,
}

#[derive(Clone, Debug)]
pub struct UpdateInstallIntent {
    pub intent_id: String,
    pub owner_principal_key: String,
    pub release: UpdateReleaseView,
    pub expires_unix_ms: i64,
}
#[derive(Clone, Debug)]
pub struct UpdateExecutionTicket {
    pub ticket_id: String,
    pub release: UpdateReleaseView,
    pub staged_path: PathBuf,
    pub expected_sha256: String,
    pub expected_size: u64,
    pub expires_unix_ms: i64,
}
#[derive(Clone, Debug)]
pub struct UpdateCompletion {
    pub ticket_id: String,
    pub release_id: String,
    pub exit_code: i32,
    pub succeeded: bool,
    pub reboot_recommended: bool,
}
#[derive(Clone, Debug)]
pub struct UpdateCheckDescriptor {
    pub channel: UpdateChannel,
    pub manifest_url: String,
    pub signature_url: String,
    pub max_manifest_bytes: u32,
    pub max_signature_bytes: u32,
}
#[derive(Clone, Debug)]
pub struct UpdateStageUploadDescriptor {
    pub upload_id: String,
    pub release: UpdateReleaseView,
    pub package_url: String,
    pub expected_size: u64,
    pub expected_sha256: String,
    pub max_chunk_bytes: u32,
    pub expires_unix_ms: i64,
}

#[derive(Debug, Error)]
pub enum UpdateEngineError {
    #[error("in-app update trust is not configured")]
    Disabled,
    #[error("update trust configuration is invalid: {0}")]
    Trust(String),
    #[error("update response is larger than the allowed limit")]
    ResponseTooLarge,
    #[error("update artifact size does not match signed metadata")]
    SizeMismatch,
    #[error("update artifact hash does not match signed metadata")]
    HashMismatch,
    #[error("update manifest was rejected: {0}")]
    Manifest(String),
    #[error("update manifest sequence rollback was rejected")]
    Rollback,
    #[error("unknown update release")]
    UnknownRelease,
    #[error("update state does not allow this action")]
    InvalidState,
    #[error("update intent was not found or expired")]
    IntentUnavailable,
    #[error("update execution ticket was not found")]
    TicketUnavailable,
    #[error("update principal does not own this operation")]
    Ownership,
    #[error("update package Authenticode verification failed with status 0x{0:08x}")]
    Authenticode(i32),
    #[error("update execution is already active")]
    Busy,
    #[error("filesystem error: {0}")]
    Io(#[source] std::io::Error),
    #[error("persistence error: {0}")]
    Persistence(String),
    #[error("unsupported platform")]
    UnsupportedPlatform,
}

impl From<manifest::ManifestError> for UpdateEngineError {
    fn from(e: manifest::ManifestError) -> Self {
        Self::Manifest(e.to_string())
    }
}

type Observer = Arc<dyn Fn(String, UpdateSnapshot) + Send + Sync + 'static>;

struct StagedRelease {
    release: UpdateManifestRelease,
    channel: UpdateChannel,
    path: PathBuf,
}
struct OwnerState {
    snapshot: UpdateSnapshot,
    releases: HashMap<String, UpdateManifestRelease>,
    staged: Option<StagedRelease>,
}

fn owner_state_is_evictable(state: &OwnerState) -> bool {
    state.staged.is_none()
        && !matches!(
            state.snapshot.state,
            UpdateState::Staging
                | UpdateState::Staged
                | UpdateState::AwaitingConsent
                | UpdateState::Installing
        )
}

fn admit_owner_state(states: &mut HashMap<String, OwnerState>, incoming_owner: &str) {
    if states.contains_key(incoming_owner) {
        return;
    }
    while states.len() >= MAX_OWNER_STATE_CACHE {
        let candidate = states
            .iter()
            .filter(|(_, state)| owner_state_is_evictable(state))
            .min_by_key(|(_, state)| state.snapshot.updated_unix_ms)
            .map(|(owner, _)| owner.clone());
        let Some(candidate) = candidate else { break };
        states.remove(&candidate);
    }
}
struct IntentRecord {
    intent: UpdateInstallIntent,
    staged_path: PathBuf,
    claimed: bool,
}
struct UploadRecord {
    owner: String,
    release: UpdateManifestRelease,
    channel: UpdateChannel,
    temp_path: PathBuf,
    offset: u64,
    expires_unix_ms: i64,
    closed: bool,
}
#[derive(Default)]
struct UploadState {
    records: HashMap<String, Arc<Mutex<UploadRecord>>>,
    owner_upload: HashMap<String, String>,
    starting_owners: HashSet<String>,
}
struct ActiveExecution {
    ticket: UpdateExecutionTicket,
    owner: String,
    _lease: MutationLease,
}

pub struct UpdateCoordinator {
    current_version: String,
    current_windows_build: u32,
    trust: UpdateTrustConfig,
    staging_root: PathBuf,
    db: Arc<Database>,
    mutations: MutationSupervisor,
    verifier: Arc<dyn PlatformVerifier>,
    states: Mutex<HashMap<String, OwnerState>>,
    intents: Mutex<HashMap<String, IntentRecord>>,
    uploads: Mutex<UploadState>,
    active: Mutex<Option<ActiveExecution>>,
    observer: Option<Observer>,
}

fn should_remove_stale_staging(
    path: &Path,
    name: &str,
    is_file: bool,
    old_enough: bool,
    protected: &HashSet<PathBuf>,
) -> bool {
    if !is_file || !old_enough || protected.contains(path) {
        return false;
    }
    (name.starts_with("upload-") && name.ends_with(".part"))
        || (name.starts_with("AetherCoreUpdate-") && name.ends_with(".exe"))
}

impl UpdateCoordinator {
    pub fn load(
        current_version: impl Into<String>,
        product_dir: &Path,
        data_root: &Path,
        db: Arc<Database>,
        mutations: MutationSupervisor,
        observer: Option<Observer>,
    ) -> Result<Arc<Self>, UpdateEngineError> {
        let trust_path = product_dir.join("update-trust.json");
        let trust = if trust_path.is_file() {
            let bytes = fs::read(&trust_path).map_err(UpdateEngineError::Io)?;
            if bytes.len() > 64 * 1024 {
                return Err(UpdateEngineError::Trust(
                    "update-trust.json is too large".into(),
                ));
            }
            let parsed: UpdateTrustConfig = serde_json::from_slice(&bytes)
                .map_err(|e| UpdateEngineError::Trust(e.to_string()))?;
            validate_trust_config(&parsed).map_err(|e| UpdateEngineError::Trust(e.to_string()))?;
            parsed
        } else {
            UpdateTrustConfig {
                schema: manifest::TRUST_SCHEMA.into(),
                enabled: false,
                channels: Vec::new(),
            }
        };
        let current_windows_build = current_windows_build()?;
        Self::assemble(
            current_version.into(),
            current_windows_build,
            trust,
            data_root.to_path_buf(),
            db,
            mutations,
            default_platform_verifier(),
            observer,
        )
    }

    /// Phase 27 (unix composition): load variant that accepts a pre-resolved Windows
    /// build number. On non-Windows hosts the caller passes 0 — update eligibility then
    /// stays disabled exactly as the honest capability matrix reports. On Windows the
    /// behavior is byte-identical to `load`.
    pub fn load_with_build(
        current_version: impl Into<String>,
        product_dir: &Path,
        data_root: &Path,
        db: Arc<Database>,
        mutations: MutationSupervisor,
        observer: Option<Observer>,
        resolved_windows_build: u32,
    ) -> Result<Arc<Self>, UpdateEngineError> {
        let trust_path = product_dir.join("update-trust.json");
        let trust = if trust_path.is_file() {
            let bytes = fs::read(&trust_path).map_err(UpdateEngineError::Io)?;
            if bytes.len() > 64 * 1024 {
                return Err(UpdateEngineError::Trust(
                    "update-trust.json is too large".into(),
                ));
            }
            let parsed: UpdateTrustConfig = serde_json::from_slice(&bytes)
                .map_err(|e| UpdateEngineError::Trust(e.to_string()))?;
            validate_trust_config(&parsed).map_err(|e| UpdateEngineError::Trust(e.to_string()))?;
            parsed
        } else {
            UpdateTrustConfig {
                schema: manifest::TRUST_SCHEMA.into(),
                enabled: false,
                channels: Vec::new(),
            }
        };
        let current_windows_build = if resolved_windows_build > 0 {
            Ok(resolved_windows_build)
        } else {
            #[cfg(windows)]
            {
                current_windows_build()
            }
            #[cfg(not(windows))]
            {
                Err(UpdateEngineError::UnsupportedPlatform)
            }
        }
        .unwrap_or(0);
        Self::assemble(
            current_version.into(),
            current_windows_build,
            trust,
            data_root.to_path_buf(),
            db,
            mutations,
            default_platform_verifier(),
            observer,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn assemble(
        current_version: String,
        current_windows_build: u32,
        trust: UpdateTrustConfig,
        staging_root_data: PathBuf,
        db: Arc<Database>,
        mutations: MutationSupervisor,
        verifier: Arc<dyn PlatformVerifier>,
        observer: Option<Observer>,
    ) -> Result<Arc<Self>, UpdateEngineError> {
        let staging_root = staging_root_data.join("update-staging");
        let coordinator = Arc::new(Self {
            current_version,
            current_windows_build,
            trust,
            staging_root,
            db,
            mutations,
            verifier,
            states: Mutex::new(HashMap::new()),
            intents: Mutex::new(HashMap::new()),
            uploads: Mutex::new(UploadState::default()),
            active: Mutex::new(None),
            observer,
        });
        coordinator.recover_execution_guard()?;
        coordinator.cleanup_stale_staging();
        Ok(coordinator)
    }

    #[cfg(test)]
    pub fn with_dependencies(
        current_version: &str,
        current_windows_build: u32,
        trust: UpdateTrustConfig,
        staging_root: PathBuf,
        db: Arc<Database>,
        mutations: MutationSupervisor,
        verifier: Arc<dyn PlatformVerifier>,
    ) -> Arc<Self> {
        Self::with_dependencies_and_observer(
            current_version,
            current_windows_build,
            trust,
            staging_root,
            db,
            mutations,
            verifier,
            None,
        )
    }

    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    fn with_dependencies_and_observer(
        current_version: &str,
        current_windows_build: u32,
        trust: UpdateTrustConfig,
        staging_root: PathBuf,
        db: Arc<Database>,
        mutations: MutationSupervisor,
        verifier: Arc<dyn PlatformVerifier>,
        observer: Option<Observer>,
    ) -> Arc<Self> {
        Arc::new(Self {
            current_version: current_version.into(),
            current_windows_build,
            trust,
            staging_root,
            db,
            mutations,
            verifier,
            states: Mutex::new(HashMap::new()),
            intents: Mutex::new(HashMap::new()),
            uploads: Mutex::new(UploadState::default()),
            active: Mutex::new(None),
            observer,
        })
    }

    pub fn snapshot(&self, owner: &str) -> UpdateSnapshot {
        self.states
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(owner)
            .map(|s| s.snapshot.clone())
            .unwrap_or_else(|| self.initial_snapshot(UpdateChannel::Stable))
    }

    pub fn check_descriptor(
        &self,
        channel: UpdateChannel,
    ) -> Result<UpdateCheckDescriptor, UpdateEngineError> {
        if !self.trust.enabled {
            return Err(UpdateEngineError::Disabled);
        }
        let trust = self
            .trust
            .channels
            .iter()
            .find(|v| v.channel == channel.as_str())
            .ok_or(UpdateEngineError::Disabled)?;
        Ok(UpdateCheckDescriptor {
            channel,
            manifest_url: trust.manifest_url.clone(),
            signature_url: trust.signature_url.clone(),
            max_manifest_bytes: MAX_MANIFEST_BYTES as u32,
            max_signature_bytes: MAX_SIGNATURE_BYTES as u32,
        })
    }

    pub fn submit_manifest(
        &self,
        owner: &str,
        channel: UpdateChannel,
        manifest_bytes: &[u8],
        sig_bytes: &[u8],
    ) -> Result<UpdateSnapshot, UpdateEngineError> {
        if owner.trim().is_empty() {
            return Err(UpdateEngineError::Ownership);
        }
        if matches!(
            self.snapshot(owner).state,
            UpdateState::Staging | UpdateState::AwaitingConsent | UpdateState::Installing
        ) {
            return Err(UpdateEngineError::Busy);
        }
        if !self.trust.enabled {
            return Ok(self.set_snapshot(owner, self.disabled_snapshot(channel)));
        }
        let trust = self
            .trust
            .channels
            .iter()
            .find(|v| v.channel == channel.as_str())
            .ok_or(UpdateEngineError::Disabled)?;
        let manifest =
            verify_manifest_bytes(manifest_bytes, sig_bytes, trust, channel.as_str(), now())?;
        let manifest_sha256 = manifest::sha256_hex(manifest_bytes);
        let floor = self
            .db
            .update_manifest_floor(channel.as_str())
            .map_err(|e| UpdateEngineError::Persistence(e.to_string()))?;
        if let Some(floor) = floor.as_ref() {
            if manifest.sequence < floor.highest_sequence {
                return Err(UpdateEngineError::Rollback);
            }
            if manifest.sequence == floor.highest_sequence
                && !manifest_sha256.eq_ignore_ascii_case(&floor.manifest_sha256)
            {
                return Err(UpdateEngineError::Rollback);
            }
        }
        if floor
            .as_ref()
            .map(|v| manifest.sequence > v.highest_sequence)
            .unwrap_or(true)
        {
            self.db
                .upsert_update_manifest_floor(&UpdateManifestFloorRecord {
                    channel: channel.as_str().into(),
                    highest_sequence: manifest.sequence,
                    generated_unix_ms: manifest.generated_unix_ms,
                    manifest_sha256: manifest_sha256.clone(),
                    updated_unix_ms: now(),
                })
                .map_err(|e| UpdateEngineError::Persistence(e.to_string()))?;
        }
        let mut releases = HashMap::new();
        let mut latest: Option<UpdateManifestRelease> = None;
        for release in manifest.releases {
            if release.minimum_windows_build <= self.current_windows_build
                && manifest::is_newer(&release.version, &self.current_version)
            {
                let choose = latest.as_ref().is_none_or(|current| {
                    manifest::parse_version(&release.version)
                        > manifest::parse_version(&current.version)
                });
                releases.insert(release.release_id.clone(), release.clone());
                if choose {
                    latest = Some(release)
                }
            }
        }
        let snapshot = {
            let mut states = self.states.lock().unwrap_or_else(|p| p.into_inner());
            admit_owner_state(&mut states, owner);
            let state = states.entry(owner.into()).or_insert_with(|| OwnerState {
                snapshot: self.initial_snapshot(channel),
                releases: HashMap::new(),
                staged: None,
            });
            state.releases = releases;
            let checked = now();
            state.snapshot = UpdateSnapshot {
                state: if latest.is_some() {
                    UpdateState::Available
                } else {
                    UpdateState::UpToDate
                },
                channel,
                current_version: self.current_version.clone(),
                latest_release: latest.as_ref().map(|r| release_view(channel, r)),
                staged_release: state
                    .staged
                    .as_ref()
                    .map(|v| release_view(v.channel, &v.release)),
                progress_known: false,
                overall_percent: 0,
                bytes_completed: 0,
                bytes_total: 0,
                status_message_key: if latest.is_some() {
                    "update.status.available"
                } else {
                    "update.status.upToDate"
                }
                .into(),
                checked_unix_ms: checked,
                updated_unix_ms: checked,
            };
            state.snapshot.clone()
        };
        self.emit(owner, &snapshot);
        Ok(snapshot)
    }

    fn reserve_upload_start(
        &self,
        owner: &str,
    ) -> Result<Vec<Arc<Mutex<UploadRecord>>>, UpdateEngineError> {
        let mut uploads = self.uploads.lock().unwrap_or_else(|p| p.into_inner());
        if !uploads.starting_owners.insert(owner.to_owned()) {
            return Err(UpdateEngineError::Busy);
        }
        let stale = uploads
            .owner_upload
            .remove(owner)
            .and_then(|id| uploads.records.remove(&id));
        Ok(stale.into_iter().collect())
    }

    fn release_upload_start(&self, owner: &str) {
        self.uploads
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .starting_owners
            .remove(owner);
    }

    fn publish_upload_start(
        &self,
        owner: &str,
        upload_id: &str,
        record: Arc<Mutex<UploadRecord>>,
    ) -> Result<(), UpdateEngineError> {
        let mut uploads = self.uploads.lock().unwrap_or_else(|p| p.into_inner());
        if !uploads.starting_owners.remove(owner) {
            return Err(UpdateEngineError::InvalidState);
        }
        if uploads.owner_upload.contains_key(owner) {
            return Err(UpdateEngineError::Busy);
        }
        uploads
            .owner_upload
            .insert(owner.to_owned(), upload_id.to_owned());
        uploads.records.insert(upload_id.to_owned(), record);
        Ok(())
    }

    fn upload_record(
        &self,
        upload_id: &str,
    ) -> Result<Arc<Mutex<UploadRecord>>, UpdateEngineError> {
        self.uploads
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .records
            .get(upload_id)
            .cloned()
            .ok_or(UpdateEngineError::InvalidState)
    }

    fn take_upload_record(
        &self,
        owner: &str,
        upload_id: &str,
    ) -> Result<Arc<Mutex<UploadRecord>>, UpdateEngineError> {
        let record = self.upload_record(upload_id)?;
        {
            let upload = record.lock().unwrap_or_else(|p| p.into_inner());
            if upload.owner != owner {
                return Err(UpdateEngineError::Ownership);
            }
        }
        let mut uploads = self.uploads.lock().unwrap_or_else(|p| p.into_inner());
        let same = uploads
            .records
            .get(upload_id)
            .is_some_and(|current| Arc::ptr_eq(current, &record));
        if !same {
            return Err(UpdateEngineError::InvalidState);
        }
        uploads.records.remove(upload_id);
        if uploads
            .owner_upload
            .get(owner)
            .is_some_and(|id| id == upload_id)
        {
            uploads.owner_upload.remove(owner);
        }
        Ok(record)
    }

    pub fn begin_stage_upload(
        &self,
        owner: &str,
        release_id: &str,
    ) -> Result<UpdateStageUploadDescriptor, UpdateEngineError> {
        self.cleanup_expired();
        if !manifest::safe_id(release_id) {
            return Err(UpdateEngineError::UnknownRelease);
        }
        let (release, channel, published) = {
            let mut states = self.states.lock().unwrap_or_else(|p| p.into_inner());
            let state = states
                .get_mut(owner)
                .ok_or(UpdateEngineError::UnknownRelease)?;
            if !matches!(
                state.snapshot.state,
                UpdateState::Available | UpdateState::Failed
            ) {
                return Err(UpdateEngineError::InvalidState);
            }
            let release = state
                .releases
                .get(release_id)
                .cloned()
                .ok_or(UpdateEngineError::UnknownRelease)?;
            state.snapshot.state = UpdateState::Staging;
            state.snapshot.progress_known = true;
            state.snapshot.overall_percent = 0;
            state.snapshot.bytes_completed = 0;
            state.snapshot.bytes_total = release.package.size_bytes;
            state.snapshot.status_message_key = "update.status.staging".into();
            state.snapshot.updated_unix_ms = now();
            (release, state.snapshot.channel, state.snapshot.clone())
        };
        self.emit(owner, &published);
        let stale = self.reserve_upload_start(owner)?;
        let result = (|| {
            for stale_record in stale {
                let path = {
                    let mut upload = stale_record.lock().unwrap_or_else(|p| p.into_inner());
                    upload.closed = true;
                    upload.temp_path.clone()
                };
                let _ = fs::remove_file(path);
            }
            fs::create_dir_all(&self.staging_root).map_err(UpdateEngineError::Io)?;
            let upload_id = Uuid::new_v4().to_string();
            let temp_path = self.staging_root.join(format!("upload-{upload_id}.part"));
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            let file = options.open(&temp_path).map_err(UpdateEngineError::Io)?;
            file.sync_all().map_err(UpdateEngineError::Io)?;
            drop(file);
            let expires_unix_ms = now() + UPLOAD_TTL_MS;
            let record = Arc::new(Mutex::new(UploadRecord {
                owner: owner.into(),
                release: release.clone(),
                channel,
                temp_path: temp_path.clone(),
                offset: 0,
                expires_unix_ms,
                closed: false,
            }));
            if let Err(error) = self.publish_upload_start(owner, &upload_id, record) {
                let _ = fs::remove_file(&temp_path);
                return Err(error);
            }
            Ok(UpdateStageUploadDescriptor {
                upload_id,
                release: release_view(channel, &release),
                package_url: release.package.url.clone(),
                expected_size: release.package.size_bytes,
                expected_sha256: release.package.sha256.clone(),
                max_chunk_bytes: MAX_STAGE_CHUNK_BYTES,
                expires_unix_ms,
            })
        })();
        if result.is_err() {
            self.release_upload_start(owner);
            self.mark_stage_failed(owner);
        }
        result
    }

    pub fn write_stage_chunk(
        &self,
        owner: &str,
        upload_id: &str,
        offset: u64,
        data: &[u8],
    ) -> Result<UpdateSnapshot, UpdateEngineError> {
        if data.is_empty() || data.len() > MAX_STAGE_CHUNK_BYTES as usize {
            return Err(UpdateEngineError::ResponseTooLarge);
        }
        let record = self.upload_record(upload_id)?;
        let (done, total) = {
            let mut upload = record.lock().unwrap_or_else(|p| p.into_inner());
            if upload.owner != owner {
                return Err(UpdateEngineError::Ownership);
            }
            if upload.closed || upload.expires_unix_ms < now() {
                return Err(UpdateEngineError::InvalidState);
            }
            if upload.offset != offset {
                return Err(UpdateEngineError::InvalidState);
            }
            let next = upload
                .offset
                .checked_add(data.len() as u64)
                .ok_or(UpdateEngineError::SizeMismatch)?;
            if next > upload.release.package.size_bytes {
                return Err(UpdateEngineError::SizeMismatch);
            }
            let meta = fs::metadata(&upload.temp_path).map_err(UpdateEngineError::Io)?;
            if meta.len() != upload.offset {
                return Err(UpdateEngineError::SizeMismatch);
            }
            let mut file = OpenOptions::new()
                .append(true)
                .open(&upload.temp_path)
                .map_err(UpdateEngineError::Io)?;
            file.write_all(data).map_err(UpdateEngineError::Io)?;
            upload.offset = next;
            (next, upload.release.package.size_bytes)
        };
        self.update_progress(owner, done, total);
        Ok(self.snapshot(owner))
    }

    pub fn finalize_stage_upload(
        &self,
        owner: &str,
        upload_id: &str,
    ) -> Result<UpdateSnapshot, UpdateEngineError> {
        let record = self.take_upload_record(owner, upload_id)?;
        let upload = {
            let mut upload = record.lock().unwrap_or_else(|p| p.into_inner());
            if upload.closed {
                return Err(UpdateEngineError::InvalidState);
            }
            upload.closed = true;
            UploadRecord {
                owner: upload.owner.clone(),
                release: upload.release.clone(),
                channel: upload.channel,
                temp_path: upload.temp_path.clone(),
                offset: upload.offset,
                expires_unix_ms: upload.expires_unix_ms,
                closed: true,
            }
        };
        let result = (|| {
            if upload.expires_unix_ms < now() || upload.offset != upload.release.package.size_bytes
            {
                return Err(UpdateEngineError::SizeMismatch);
            }
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .open(&upload.temp_path)
                .map_err(UpdateEngineError::Io)?;
            file.sync_all().map_err(UpdateEngineError::Io)?;
            drop(file);
            verify_file_hash_size(
                &upload.temp_path,
                &upload.release.package.sha256,
                upload.release.package.size_bytes,
            )?;
            self.verifier.verify_authenticode(&upload.temp_path)?;
            verify_file_hash_size(
                &upload.temp_path,
                &upload.release.package.sha256,
                upload.release.package.size_bytes,
            )?;
            let path = self.expected_staged_path(
                owner,
                &upload.release.release_id,
                &upload.release.package.sha256,
            )?;
            match fs::remove_file(&path) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(UpdateEngineError::Io(e)),
            }
            fs::rename(&upload.temp_path, &path).map_err(UpdateEngineError::Io)?;
            let snapshot = {
                let mut states = self.states.lock().unwrap_or_else(|p| p.into_inner());
                let state = states.get_mut(owner).ok_or(UpdateEngineError::Ownership)?;
                state.staged = Some(StagedRelease {
                    release: upload.release.clone(),
                    channel: upload.channel,
                    path,
                });
                state.snapshot.state = UpdateState::Staged;
                state.snapshot.staged_release = Some(release_view(upload.channel, &upload.release));
                state.snapshot.progress_known = true;
                state.snapshot.overall_percent = 100;
                state.snapshot.bytes_completed = upload.release.package.size_bytes;
                state.snapshot.bytes_total = upload.release.package.size_bytes;
                state.snapshot.status_message_key = "update.status.staged".into();
                state.snapshot.updated_unix_ms = now();
                state.snapshot.clone()
            };
            self.emit(owner, &snapshot);
            Ok(snapshot)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&upload.temp_path);
            self.mark_stage_failed(owner);
        }
        result
    }

    pub fn cancel_stage_upload(
        &self,
        owner: &str,
        upload_id: &str,
    ) -> Result<UpdateSnapshot, UpdateEngineError> {
        let record = self.take_upload_record(owner, upload_id)?;
        let path = {
            let mut upload = record.lock().unwrap_or_else(|p| p.into_inner());
            upload.closed = true;
            upload.temp_path.clone()
        };
        let _ = fs::remove_file(path);
        self.mutate_snapshot(owner, |s| {
            s.state = if s.latest_release.is_some() {
                UpdateState::Available
            } else {
                UpdateState::Idle
            };
            s.status_message_key = if s.latest_release.is_some() {
                "update.status.available"
            } else {
                "update.status.idle"
            }
            .into();
            s.progress_known = false;
            s.overall_percent = 0;
            s.bytes_completed = 0;
            s.bytes_total = 0;
        });
        Ok(self.snapshot(owner))
    }

    pub fn begin_install_intent(
        &self,
        owner: &str,
        release_id: &str,
    ) -> Result<UpdateInstallIntent, UpdateEngineError> {
        let (release, path) = {
            let mut states = self.states.lock().unwrap_or_else(|p| p.into_inner());
            let state = states
                .get_mut(owner)
                .ok_or(UpdateEngineError::InvalidState)?;
            let staged = state
                .staged
                .as_ref()
                .filter(|v| v.release.release_id == release_id)
                .ok_or(UpdateEngineError::InvalidState)?;
            (
                release_view(staged.channel, &staged.release),
                staged.path.clone(),
            )
        };
        verify_file_hash_size(&path, &release.sha256, release.size_bytes)?;
        self.verifier.verify_authenticode(&path)?;
        verify_file_hash_size(&path, &release.sha256, release.size_bytes)?;
        let intent = UpdateInstallIntent {
            intent_id: Uuid::new_v4().to_string(),
            owner_principal_key: owner.into(),
            release,
            expires_unix_ms: now() + INTENT_TTL_MS,
        };
        {
            let mut intents = self.intents.lock().unwrap_or_else(|p| p.into_inner());
            intents
                .retain(|_, record| record.intent.owner_principal_key != owner || record.claimed);
            intents.insert(
                intent.intent_id.clone(),
                IntentRecord {
                    intent: intent.clone(),
                    staged_path: path,
                    claimed: false,
                },
            );
        }
        self.mutate_snapshot(owner, |s| {
            s.state = UpdateState::AwaitingConsent;
            s.status_message_key = "update.status.awaitingConsent".into();
        });
        Ok(intent)
    }

    pub fn intent_for_broker(
        &self,
        owner: &str,
        intent_id: &str,
    ) -> Result<UpdateInstallIntent, UpdateEngineError> {
        self.cleanup_expired();
        let intents = self.intents.lock().unwrap_or_else(|p| p.into_inner());
        let record = intents
            .get(intent_id)
            .ok_or(UpdateEngineError::IntentUnavailable)?;
        if record.intent.owner_principal_key != owner {
            return Err(UpdateEngineError::Ownership);
        }
        if record.claimed || record.intent.expires_unix_ms < now() {
            return Err(UpdateEngineError::IntentUnavailable);
        }
        Ok(record.intent.clone())
    }

    pub fn cancel_install_intent(
        &self,
        owner: &str,
        intent_id: &str,
    ) -> Result<UpdateSnapshot, UpdateEngineError> {
        self.cleanup_expired();
        let removed = {
            let mut intents = self.intents.lock().unwrap_or_else(|p| p.into_inner());
            let record = intents
                .get(intent_id)
                .ok_or(UpdateEngineError::IntentUnavailable)?;
            if record.intent.owner_principal_key != owner {
                return Err(UpdateEngineError::Ownership);
            }
            if record.claimed {
                return Err(UpdateEngineError::InvalidState);
            }
            intents
                .remove(intent_id)
                .ok_or(UpdateEngineError::IntentUnavailable)?
        };
        let _ = removed;
        self.mutate_snapshot(owner, |s| {
            if s.state == UpdateState::AwaitingConsent {
                s.state = if s.staged_release.is_some() {
                    UpdateState::Staged
                } else if s.latest_release.is_some() {
                    UpdateState::Available
                } else {
                    UpdateState::Idle
                };
                s.status_message_key = match s.state {
                    UpdateState::Staged => "update.status.staged",
                    UpdateState::Available => "update.status.available",
                    _ => "update.status.idle",
                }
                .into();
                s.progress_known = s.state == UpdateState::Staged;
                s.overall_percent = if s.state == UpdateState::Staged {
                    100
                } else {
                    0
                };
            }
        });
        Ok(self.snapshot(owner))
    }

    pub fn claim_install(
        &self,
        owner: &str,
        intent_id: &str,
    ) -> Result<UpdateExecutionTicket, UpdateEngineError> {
        self.cleanup_expired();
        // Linearize the one-shot claim before any expensive file/authenticode work so a concurrent
        // cancel cannot revoke an intent after the elevated broker has begun claiming it.
        let (intent, path) = {
            let mut intents = self.intents.lock().unwrap_or_else(|p| p.into_inner());
            let record = intents
                .get_mut(intent_id)
                .ok_or(UpdateEngineError::IntentUnavailable)?;
            if record.intent.owner_principal_key != owner {
                return Err(UpdateEngineError::Ownership);
            }
            if record.claimed || record.intent.expires_unix_ms < now() {
                return Err(UpdateEngineError::IntentUnavailable);
            }
            record.claimed = true;
            (record.intent.clone(), record.staged_path.clone())
        };
        let result = (|| {
            verify_file_hash_size(&path, &intent.release.sha256, intent.release.size_bytes)?;
            self.verifier.verify_authenticode(&path)?;
            verify_file_hash_size(&path, &intent.release.sha256, intent.release.size_bytes)?;
            let lease = self
                .mutations
                .try_acquire(
                    MutationWorkload::Update,
                    &format!("update:{}", intent.release.release_id),
                    owner,
                )
                .map_err(|_| UpdateEngineError::Busy)?;
            let ticket = UpdateExecutionTicket {
                ticket_id: Uuid::new_v4().to_string(),
                release: intent.release.clone(),
                staged_path: path.clone(),
                expected_sha256: intent.release.sha256.clone(),
                expected_size: intent.release.size_bytes,
                expires_unix_ms: now() + EXECUTION_TTL_MS,
            };
            let guard = UpdateExecutionGuardRecord {
                ticket_id: ticket.ticket_id.clone(),
                owner_principal_key: owner.into(),
                release_id: intent.release.release_id.clone(),
                release_version: intent.release.version.clone(),
                channel: intent.release.channel.as_str().into(),
                notes_message_key: intent.release.notes_message_key.clone(),
                minimum_windows_build: intent.release.minimum_windows_build,
                staged_path: path.to_string_lossy().into_owned(),
                expected_sha256: ticket.expected_sha256.clone(),
                expected_size: ticket.expected_size,
                expires_unix_ms: ticket.expires_unix_ms,
                created_unix_ms: now(),
            };
            if let Err(e) = self.db.replace_update_execution_guard(&guard) {
                drop(lease);
                return Err(UpdateEngineError::Persistence(e.to_string()));
            }
            {
                let mut active = self.active.lock().unwrap_or_else(|p| p.into_inner());
                if active.is_some() {
                    let _ = self.db.clear_update_execution_guard(&ticket.ticket_id);
                    drop(lease);
                    return Err(UpdateEngineError::Busy);
                }
                *active = Some(ActiveExecution {
                    ticket: ticket.clone(),
                    owner: owner.into(),
                    _lease: lease,
                });
            }
            self.mutate_snapshot(owner, |s| {
                s.state = UpdateState::Installing;
                s.status_message_key = "update.status.installing".into();
                s.progress_known = false;
            });
            Ok(ticket)
        })();
        if result.is_err() {
            if let Some(record) = self
                .intents
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .get_mut(intent_id)
            {
                record.claimed = false;
            }
        } else {
            // The durable execution guard + machine mutation lease now carry the one-shot authority.
            // Remove the ephemeral consent intent only after that handoff succeeds.
            self.intents
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .remove(intent_id);
        }
        result
    }

    pub fn complete_install(
        &self,
        owner: &str,
        ticket_id: &str,
        exit_code: i32,
    ) -> Result<UpdateCompletion, UpdateEngineError> {
        let (release_id, staged_path) = {
            let active = self.active.lock().unwrap_or_else(|p| p.into_inner());
            let value = active
                .as_ref()
                .ok_or(UpdateEngineError::TicketUnavailable)?;
            if value.owner != owner {
                return Err(UpdateEngineError::Ownership);
            }
            if value.ticket.ticket_id != ticket_id {
                return Err(UpdateEngineError::TicketUnavailable);
            }
            (
                value.ticket.release.release_id.clone(),
                value.ticket.staged_path.clone(),
            )
        };
        self.db
            .clear_update_execution_guard(ticket_id)
            .map_err(|e| UpdateEngineError::Persistence(e.to_string()))?;
        self.active.lock().unwrap_or_else(|p| p.into_inner()).take();
        let succeeded = matches!(exit_code, 0 | 3010);
        let reboot_recommended = exit_code == 3010;
        self.mutate_snapshot(owner, |s| {
            s.state = if succeeded {
                UpdateState::Completed
            } else {
                UpdateState::Failed
            };
            s.status_message_key = if succeeded {
                "update.status.completed"
            } else {
                "update.status.installFailed"
            }
            .into();
            s.progress_known = false;
        });
        if succeeded {
            let _ = fs::remove_file(staged_path);
        }
        Ok(UpdateCompletion {
            ticket_id: ticket_id.into(),
            release_id,
            exit_code,
            succeeded,
            reboot_recommended,
        })
    }

    pub fn reap_expired_execution(&self) {
        let expired_ticket = {
            let active = self.active.lock().unwrap_or_else(|p| p.into_inner());
            active
                .as_ref()
                .filter(|v| v.ticket.expires_unix_ms < now())
                .map(|v| (v.ticket.ticket_id.clone(), v.owner.clone()))
        };
        if let Some((ticket_id, owner)) = expired_ticket {
            // Fail closed: never release the machine Update lease while the durable guard still says
            // an installer execution may be active. A later reap retries ledger cleanup.
            if self.db.clear_update_execution_guard(&ticket_id).is_ok() {
                self.active.lock().unwrap_or_else(|p| p.into_inner()).take();
                self.mutate_snapshot(&owner, |s| {
                    s.state = UpdateState::Failed;
                    s.status_message_key = "update.status.executionExpired".into();
                    s.progress_known = false;
                });
            }
        }
        self.cleanup_expired();
    }

    fn recover_execution_guard(&self) -> Result<(), UpdateEngineError> {
        let Some(record) = self
            .db
            .active_update_execution_guard()
            .map_err(|e| UpdateEngineError::Persistence(e.to_string()))?
        else {
            return Ok(());
        };
        if record.expires_unix_ms < now() {
            self.db
                .clear_update_execution_guard(&record.ticket_id)
                .map_err(|e| UpdateEngineError::Persistence(e.to_string()))?;
            return Ok(());
        }
        let path = PathBuf::from(&record.staged_path);
        let expected = self.expected_staged_path(
            &record.owner_principal_key,
            &record.release_id,
            &record.expected_sha256,
        )?;
        if path != expected {
            return Err(UpdateEngineError::Manifest(
                "durable update staging path is outside the service-derived location".into(),
            ));
        }
        verify_file_hash_size(&path, &record.expected_sha256, record.expected_size)?;
        self.verifier.verify_authenticode(&path)?;
        verify_file_hash_size(&path, &record.expected_sha256, record.expected_size)?;
        let lease = self
            .mutations
            .try_acquire(
                MutationWorkload::Update,
                &format!("update:{}", record.release_id),
                &record.owner_principal_key,
            )
            .map_err(|_| UpdateEngineError::Busy)?;
        let recovered_channel = match record.channel.as_str() {
            "stable" => UpdateChannel::Stable,
            "beta" => UpdateChannel::Beta,
            _ => {
                return Err(UpdateEngineError::Manifest(
                    "durable update channel is invalid".into(),
                ));
            }
        };
        let release = UpdateReleaseView {
            release_id: record.release_id.clone(),
            version: record.release_version.clone(),
            channel: recovered_channel,
            published_unix_ms: 0,
            notes_message_key: record.notes_message_key.clone(),
            minimum_windows_build: record.minimum_windows_build,
            size_bytes: record.expected_size,
            sha256: record.expected_sha256.clone(),
            package_kind: UpdatePackageKind::Burn,
        };
        let ticket = UpdateExecutionTicket {
            ticket_id: record.ticket_id.clone(),
            release: release.clone(),
            staged_path: path,
            expected_sha256: record.expected_sha256,
            expected_size: record.expected_size,
            expires_unix_ms: record.expires_unix_ms,
        };
        let owner = record.owner_principal_key.clone();
        *self.active.lock().unwrap_or_else(|p| p.into_inner()) = Some(ActiveExecution {
            ticket,
            owner: owner.clone(),
            _lease: lease,
        });
        // A service restart must not make an active installer look idle. The durable guard does not
        // carry download authority; the reconstructed view is intentionally minimal and non-actionable.
        let mut snapshot = self.initial_snapshot(recovered_channel);
        snapshot.state = UpdateState::Installing;
        snapshot.staged_release = Some(release);
        snapshot.status_message_key = "update.status.installing".into();
        snapshot.progress_known = false;
        snapshot.updated_unix_ms = now();
        self.set_snapshot(&owner, snapshot);
        Ok(())
    }

    fn mark_stage_failed(&self, owner: &str) {
        self.mutate_snapshot(owner, |s| {
            s.state = UpdateState::Failed;
            s.status_message_key = "update.status.stageFailed".into();
            s.progress_known = false;
            s.overall_percent = 0;
        });
    }
    fn expected_staged_path(
        &self,
        owner: &str,
        release_id: &str,
        sha256: &str,
    ) -> Result<PathBuf, UpdateEngineError> {
        if !manifest::safe_id(release_id) {
            return Err(UpdateEngineError::UnknownRelease);
        }
        if sha256.len() != 64 || !sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(UpdateEngineError::Manifest(
                "update package sha256 is invalid".into(),
            ));
        }
        let owner_digest = Sha256::digest(owner.as_bytes());
        let owner_scope = hex::encode(&owner_digest[..16]);
        Ok(self.staging_root.join(format!(
            "AetherCoreUpdate-{owner_scope}-{release_id}-{}.exe",
            sha256.to_ascii_lowercase()
        )))
    }
    fn cleanup_expired(&self) {
        let now = now();
        let expired_intent_owners = {
            let mut intents = self.intents.lock().unwrap_or_else(|p| p.into_inner());
            let owners: Vec<String> = intents
                .values()
                .filter(|v| v.intent.expires_unix_ms < now && !v.claimed)
                .map(|v| v.intent.owner_principal_key.clone())
                .collect();
            intents.retain(|_, v| v.claimed || v.intent.expires_unix_ms >= now);
            owners
        };
        for owner in expired_intent_owners {
            self.mutate_snapshot(&owner, |s| {
                if s.state == UpdateState::AwaitingConsent {
                    s.state = if s.staged_release.is_some() {
                        UpdateState::Staged
                    } else if s.latest_release.is_some() {
                        UpdateState::Available
                    } else {
                        UpdateState::Idle
                    };
                    s.status_message_key = match s.state {
                        UpdateState::Staged => "update.status.staged",
                        UpdateState::Available => "update.status.available",
                        _ => "update.status.idle",
                    }
                    .into();
                    s.progress_known = s.state == UpdateState::Staged;
                    s.overall_percent = if s.state == UpdateState::Staged {
                        100
                    } else {
                        0
                    };
                }
            })
        }
        let candidates: Vec<(String, Arc<Mutex<UploadRecord>>)> = {
            let uploads = self.uploads.lock().unwrap_or_else(|p| p.into_inner());
            uploads
                .records
                .iter()
                .map(|(id, record)| (id.clone(), record.clone()))
                .collect()
        };
        let mut expired = Vec::new();
        for (id, record) in candidates {
            let is_expired = {
                let upload = record.lock().unwrap_or_else(|p| p.into_inner());
                upload.expires_unix_ms < now
            };
            if is_expired {
                expired.push((id, record));
            }
        }
        let mut expired_records = Vec::new();
        for (id, record) in expired {
            let removed = {
                let mut uploads = self.uploads.lock().unwrap_or_else(|p| p.into_inner());
                let same = uploads
                    .records
                    .get(&id)
                    .is_some_and(|current| Arc::ptr_eq(current, &record));
                if same {
                    uploads.records.remove(&id);
                    uploads
                        .owner_upload
                        .retain(|_, current_id| current_id != &id);
                    true
                } else {
                    false
                }
            };
            if removed {
                let (owner, path) = {
                    let mut upload = record.lock().unwrap_or_else(|p| p.into_inner());
                    upload.closed = true;
                    (upload.owner.clone(), upload.temp_path.clone())
                };
                expired_records.push((owner, path));
            }
        }
        for (_, path) in &expired_records {
            let _ = fs::remove_file(path);
        }
        for (owner, _) in expired_records {
            self.mutate_snapshot(&owner, |s| {
                if s.state == UpdateState::Staging {
                    s.state = if s.latest_release.is_some() {
                        UpdateState::Available
                    } else {
                        UpdateState::Idle
                    };
                    s.status_message_key = if s.latest_release.is_some() {
                        "update.status.available"
                    } else {
                        "update.status.idle"
                    }
                    .into();
                    s.progress_known = false;
                }
            })
        }
    }
    fn cleanup_stale_staging(&self) {
        let mut protected = HashSet::new();
        if let Some(active) = self
            .active
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_ref()
        {
            protected.insert(active.ticket.staged_path.clone());
        }
        {
            let states = self.states.lock().unwrap_or_else(|p| p.into_inner());
            for state in states.values() {
                if let Some(staged) = state.staged.as_ref() {
                    protected.insert(staged.path.clone());
                }
            }
        }
        if let Ok(entries) = fs::read_dir(&self.staging_root) {
            for entry in entries.flatten() {
                let path = entry.path();
                let Ok(file_type) = entry.file_type() else {
                    continue;
                };
                let name = entry.file_name();
                let name = name.to_string_lossy();
                let old_enough = entry
                    .metadata()
                    .ok()
                    .and_then(|meta| meta.modified().ok())
                    .and_then(|modified| modified.elapsed().ok())
                    .is_some_and(|age| age > Duration::from_secs(48 * 60 * 60));
                if should_remove_stale_staging(
                    &path,
                    &name,
                    file_type.is_file(),
                    old_enough,
                    &protected,
                ) {
                    let _ = fs::remove_file(path);
                }
            }
        }
    }
    fn initial_snapshot(&self, channel: UpdateChannel) -> UpdateSnapshot {
        UpdateSnapshot {
            state: if self.trust.enabled {
                UpdateState::Idle
            } else {
                UpdateState::Disabled
            },
            channel,
            current_version: self.current_version.clone(),
            latest_release: None,
            staged_release: None,
            progress_known: false,
            overall_percent: 0,
            bytes_completed: 0,
            bytes_total: 0,
            status_message_key: if self.trust.enabled {
                "update.status.idle"
            } else {
                "update.status.disabled"
            }
            .into(),
            checked_unix_ms: 0,
            updated_unix_ms: now(),
        }
    }
    fn disabled_snapshot(&self, channel: UpdateChannel) -> UpdateSnapshot {
        self.initial_snapshot(channel)
    }
    fn set_snapshot(&self, owner: &str, snapshot: UpdateSnapshot) -> UpdateSnapshot {
        {
            let mut states = self.states.lock().unwrap_or_else(|p| p.into_inner());
            admit_owner_state(&mut states, owner);
            states
                .entry(owner.into())
                .or_insert_with(|| OwnerState {
                    snapshot: snapshot.clone(),
                    releases: HashMap::new(),
                    staged: None,
                })
                .snapshot = snapshot.clone();
        }
        self.emit(owner, &snapshot);
        snapshot
    }
    fn mutate_snapshot(&self, owner: &str, f: impl FnOnce(&mut UpdateSnapshot)) {
        let published = {
            let mut states = self.states.lock().unwrap_or_else(|p| p.into_inner());
            admit_owner_state(&mut states, owner);
            let state = states.entry(owner.into()).or_insert_with(|| OwnerState {
                snapshot: self.initial_snapshot(UpdateChannel::Stable),
                releases: HashMap::new(),
                staged: None,
            });
            f(&mut state.snapshot);
            state.snapshot.updated_unix_ms = now();
            state.snapshot.clone()
        };
        self.emit(owner, &published);
    }
    fn update_progress(&self, owner: &str, done: u64, total: u64) {
        self.mutate_snapshot(owner, |s| {
            s.progress_known = total > 0;
            s.bytes_completed = done;
            s.bytes_total = total;
            s.overall_percent = done
                .saturating_mul(100)
                .checked_div(total)
                .unwrap_or(0)
                .min(100) as u32;
        });
    }
    fn emit(&self, owner: &str, snapshot: &UpdateSnapshot) {
        if let Some(observer) = &self.observer {
            observer(owner.into(), snapshot.clone())
        }
    }
}

fn release_view(channel: UpdateChannel, r: &UpdateManifestRelease) -> UpdateReleaseView {
    UpdateReleaseView {
        release_id: r.release_id.clone(),
        version: r.version.clone(),
        channel,
        published_unix_ms: r.published_unix_ms,
        notes_message_key: r.notes_message_key.clone(),
        minimum_windows_build: r.minimum_windows_build,
        size_bytes: r.package.size_bytes,
        sha256: r.package.sha256.clone(),
        package_kind: r.package.kind,
    }
}
fn now() -> i64 {
    Utc::now().timestamp_millis()
}

pub fn verify_file_hash_size(
    path: &Path,
    expected_hash: &str,
    expected_size: u64,
) -> Result<(), UpdateEngineError> {
    let mut file = fs::File::open(path).map_err(UpdateEngineError::Io)?;
    let meta = file.metadata().map_err(UpdateEngineError::Io)?;
    if meta.len() != expected_size {
        return Err(UpdateEngineError::SizeMismatch);
    }
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 128 * 1024];
    loop {
        let n = std::io::Read::read(&mut file, &mut buffer).map_err(UpdateEngineError::Io)?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    let actual = hex::encode(hash.finalize());
    if !actual.eq_ignore_ascii_case(expected_hash) {
        return Err(UpdateEngineError::HashMismatch);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        PlatformVerifier, SignatureVerification,
        manifest::{
            MANIFEST_SCHEMA, SIGNATURE_SCHEMA, TRUST_SCHEMA, UpdateManifest, UpdatePackage,
            UpdateTrustChannel, manifest_signature_from_hex, sign_manifest_bytes,
        },
    };
    use ed25519_dalek::SigningKey;
    use std::{
        fs,
        sync::{Arc, Weak, mpsc},
        time::Duration,
    };

    #[derive(Default)]
    struct AcceptVerifier;
    impl PlatformVerifier for AcceptVerifier {
        fn verify_authenticode(
            &self,
            path: &Path,
        ) -> Result<SignatureVerification, UpdateEngineError> {
            if path.is_file() {
                Ok(SignatureVerification {
                    valid: true,
                    signer_subject: "AetherCore Test Publisher".into(),
                    signer_thumbprint_or_identity: "TEST-IDENTITY".into(),
                    chain_status: "FixtureTrusted".into(),
                    test_signed: false,
                })
            } else {
                Err(UpdateEngineError::Authenticode(-1))
            }
        }
    }

    struct Fixture {
        coordinator: Arc<UpdateCoordinator>,
        mutation: MutationSupervisor,
        db_path: PathBuf,
        root: PathBuf,
        signing: [u8; 32],
        owner: String,
        package: Vec<u8>,
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.db_path);
            let _ = fs::remove_file(self.db_path.with_extension("db-wal"));
            let _ = fs::remove_file(self.db_path.with_extension("db-shm"));
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn fixture() -> Fixture {
        fixture_with_observer(None)
    }

    fn fixture_with_observer(observer: Option<Observer>) -> Fixture {
        let root = std::env::temp_dir().join(format!("aethercore-update-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let db_path = root.join("state.db");
        let db = Arc::new(Database::open(&db_path).unwrap());
        let mutation = MutationSupervisor::new();
        let signing = [31u8; 32];
        let key = SigningKey::from_bytes(&signing);
        let trust = UpdateTrustConfig {
            schema: TRUST_SCHEMA.into(),
            enabled: true,
            channels: vec![UpdateTrustChannel {
                channel: "stable".into(),
                manifest_url: "https://updates.example.invalid/stable/manifest.json".into(),
                signature_url: "https://updates.example.invalid/stable/manifest.sig".into(),
                key_id: "test-key".into(),
                public_key_hex: hex::encode(key.verifying_key().to_bytes()),
            }],
        };
        let coordinator = UpdateCoordinator::with_dependencies_and_observer(
            "1.0.0",
            22621,
            trust,
            root.join("staging"),
            db,
            mutation.clone(),
            Arc::new(AcceptVerifier),
            observer,
        );
        Fixture {
            coordinator,
            mutation,
            db_path,
            root,
            signing,
            owner: "owner-a".into(),
            package: b"verified burn package bytes".to_vec(),
        }
    }

    fn signed_manifest(
        f: &Fixture,
        sequence: u64,
        package: &[u8],
        release_id: &str,
        minimum_windows_build: u32,
    ) -> (Vec<u8>, Vec<u8>) {
        let now = Utc::now().timestamp_millis();
        let sha = hex::encode(Sha256::digest(package));
        let manifest = UpdateManifest {
            schema: MANIFEST_SCHEMA.into(),
            channel: "stable".into(),
            sequence,
            generated_unix_ms: now - 1_000,
            expires_unix_ms: now + 60_000,
            releases: vec![UpdateManifestRelease {
                release_id: release_id.into(),
                version: "1.1.0".into(),
                published_unix_ms: now - 1_000,
                notes_message_key: "update.notes.release".into(),
                minimum_windows_build,
                package: UpdatePackage {
                    kind: UpdatePackageKind::Burn,
                    url: "https://updates.example.invalid/AetherCore-1.1.0.exe".into(),
                    size_bytes: package.len() as u64,
                    sha256: sha,
                },
            }],
        };
        let bytes = serde_json::to_vec(&manifest).unwrap();
        let signature = sign_manifest_bytes(&bytes, &f.signing).unwrap();
        let envelope = manifest_signature_from_hex("test-key", &signature);
        assert_eq!(envelope.schema, SIGNATURE_SCHEMA);
        (bytes, serde_json::to_vec(&envelope).unwrap())
    }

    fn submit(f: &Fixture, sequence: u64, package: &[u8], release_id: &str) -> UpdateSnapshot {
        let (manifest, sig) = signed_manifest(f, sequence, package, release_id, 22621);
        f.coordinator
            .submit_manifest(&f.owner, UpdateChannel::Stable, &manifest, &sig)
            .unwrap()
    }

    fn upload_all(f: &Fixture, release_id: &str, package: &[u8]) -> UpdateSnapshot {
        let descriptor = f
            .coordinator
            .begin_stage_upload(&f.owner, release_id)
            .unwrap();
        let mut offset = 0u64;
        for chunk in package.chunks(7) {
            f.coordinator
                .write_stage_chunk(&f.owner, &descriptor.upload_id, offset, chunk)
                .unwrap();
            offset += chunk.len() as u64;
        }
        f.coordinator
            .finalize_stage_upload(&f.owner, &descriptor.upload_id)
            .unwrap()
    }

    #[test]
    fn upload_start_reservations_are_owner_scoped_not_global() {
        let f = fixture();
        assert!(f.coordinator.reserve_upload_start("owner-a").is_ok());
        assert!(f.coordinator.reserve_upload_start("owner-b").is_ok());
        assert!(matches!(
            f.coordinator.reserve_upload_start("owner-a"),
            Err(UpdateEngineError::Busy)
        ));
        f.coordinator.release_upload_start("owner-a");
        f.coordinator.release_upload_start("owner-b");
    }

    #[test]
    fn upload_records_use_independent_per_upload_mutexes() {
        let f = fixture();
        let package = f.package.clone();
        let (manifest, sig) = signed_manifest(&f, 1, &package, "release-a", 22621);
        f.coordinator
            .submit_manifest("owner-a", UpdateChannel::Stable, &manifest, &sig)
            .unwrap();
        f.coordinator
            .submit_manifest("owner-b", UpdateChannel::Stable, &manifest, &sig)
            .unwrap();
        let a = f
            .coordinator
            .begin_stage_upload("owner-a", "release-a")
            .unwrap();
        let b = f
            .coordinator
            .begin_stage_upload("owner-b", "release-a")
            .unwrap();
        let (a_record, b_record) = {
            let uploads = f.coordinator.uploads.lock().unwrap();
            (
                uploads.records.get(&a.upload_id).unwrap().clone(),
                uploads.records.get(&b.upload_id).unwrap().clone(),
            )
        };
        let _a_guard = a_record.lock().unwrap();
        assert!(b_record.try_lock().is_ok());
        drop(_a_guard);
        f.coordinator
            .cancel_stage_upload("owner-a", &a.upload_id)
            .unwrap();
        f.coordinator
            .cancel_stage_upload("owner-b", &b.upload_id)
            .unwrap();
    }

    #[test]
    fn staged_paths_are_owner_scoped_and_content_addressed() {
        let f = fixture();
        let hash_a = hex::encode(Sha256::digest(b"package-a"));
        let hash_b = hex::encode(Sha256::digest(b"package-b"));
        let a = f
            .coordinator
            .expected_staged_path("owner-a", "release-a", &hash_a)
            .unwrap();
        let b = f
            .coordinator
            .expected_staged_path("owner-b", "release-a", &hash_a)
            .unwrap();
        let changed = f
            .coordinator
            .expected_staged_path("owner-a", "release-a", &hash_b)
            .unwrap();
        assert_ne!(a, b);
        assert_ne!(a, changed);
        let file = a.file_name().unwrap().to_string_lossy();
        assert!(file.starts_with("AetherCoreUpdate-"));
        assert!(file.ends_with(&format!("-release-a-{hash_a}.exe")));
        assert!(!file.contains("owner-a"));
        assert!(matches!(
            f.coordinator
                .expected_staged_path("owner-a", "release-a", "xyz"),
            Err(UpdateEngineError::Manifest(_))
        ));
    }

    #[test]
    fn stale_staging_cleanup_never_removes_protected_execution_artifact() {
        let protected_path = PathBuf::from(
            r"C:\ProgramData\AetherCore\update-staging\AetherCoreUpdate-owner-release-hash.exe",
        );
        let mut protected = HashSet::new();
        protected.insert(protected_path.clone());
        assert!(!should_remove_stale_staging(
            &protected_path,
            "AetherCoreUpdate-owner-release-hash.exe",
            true,
            true,
            &protected
        ));
        let stale = PathBuf::from(r"C:\ProgramData\AetherCore\update-staging\upload-test.part");
        assert!(should_remove_stale_staging(
            &stale,
            "upload-test.part",
            true,
            true,
            &protected
        ));
        assert!(!should_remove_stale_staging(
            &stale,
            "upload-test.part",
            false,
            true,
            &protected
        ));
        assert!(!should_remove_stale_staging(
            &stale,
            "unowned.tmp",
            true,
            true,
            &protected
        ));
    }

    #[test]
    fn observer_publication_never_runs_while_state_mutex_is_held() {
        let holder = Arc::new(Mutex::new(None::<Weak<UpdateCoordinator>>));
        let observer_holder = holder.clone();
        let (tx, rx) = mpsc::channel();
        let observer: Observer = Arc::new(move |_owner, _snapshot| {
            let lock_is_free = observer_holder
                .lock()
                .unwrap()
                .as_ref()
                .and_then(Weak::upgrade)
                .map(|coordinator| coordinator.states.try_lock().is_ok())
                .unwrap_or(true);
            tx.send(lock_is_free).unwrap();
        });
        let f = fixture_with_observer(Some(observer));
        *holder.lock().unwrap() = Some(Arc::downgrade(&f.coordinator));
        let package = f.package.clone();
        submit(&f, 1, &package, "release-a");
        assert!(rx.recv_timeout(Duration::from_millis(100)).unwrap());
        let descriptor = f
            .coordinator
            .begin_stage_upload(&f.owner, "release-a")
            .unwrap();
        assert!(rx.recv_timeout(Duration::from_millis(100)).unwrap());
        f.coordinator
            .write_stage_chunk(&f.owner, &descriptor.upload_id, 0, &package)
            .unwrap();
        assert!(rx.recv_timeout(Duration::from_millis(100)).unwrap());
        f.coordinator
            .finalize_stage_upload(&f.owner, &descriptor.upload_id)
            .unwrap();
        assert!(rx.recv_timeout(Duration::from_millis(100)).unwrap());
    }

    #[test]
    fn signed_manifest_floor_rejects_rollback_and_same_sequence_equivocation() {
        let f = fixture();
        let first = f.package.clone();
        submit(&f, 7, &first, "release-a");
        let lower = signed_manifest(&f, 6, &first, "release-a", 22621);
        assert!(matches!(
            f.coordinator
                .submit_manifest(&f.owner, UpdateChannel::Stable, &lower.0, &lower.1),
            Err(UpdateEngineError::Rollback)
        ));
        let changed = b"different signed package bytes";
        let equiv = signed_manifest(&f, 7, changed, "release-b", 22621);
        assert!(matches!(
            f.coordinator
                .submit_manifest(&f.owner, UpdateChannel::Stable, &equiv.0, &equiv.1),
            Err(UpdateEngineError::Rollback)
        ));
    }

    #[test]
    fn releases_requiring_newer_windows_build_are_not_offered() {
        let f = fixture();
        let package = f.package.clone();
        let (manifest, sig) = signed_manifest(&f, 1, &package, "future-release", 26100);
        let snapshot = f
            .coordinator
            .submit_manifest(&f.owner, UpdateChannel::Stable, &manifest, &sig)
            .unwrap();
        assert_eq!(snapshot.state, UpdateState::UpToDate);
        assert!(snapshot.latest_release.is_none());
    }

    #[test]
    fn stage_upload_is_offset_bounded_and_rejects_truncation() {
        let f = fixture();
        let package = f.package.clone();
        submit(&f, 1, &package, "release-a");
        let descriptor = f
            .coordinator
            .begin_stage_upload(&f.owner, "release-a")
            .unwrap();
        assert!(matches!(
            f.coordinator
                .write_stage_chunk(&f.owner, &descriptor.upload_id, 1, b"x"),
            Err(UpdateEngineError::InvalidState)
        ));
        f.coordinator
            .write_stage_chunk(&f.owner, &descriptor.upload_id, 0, &package[..4])
            .unwrap();
        assert!(matches!(
            f.coordinator
                .finalize_stage_upload(&f.owner, &descriptor.upload_id),
            Err(UpdateEngineError::SizeMismatch)
        ));
        assert_eq!(f.coordinator.snapshot(&f.owner).state, UpdateState::Failed);
    }

    #[test]
    fn claim_reserves_intent_before_machine_lease_and_releases_reservation_on_busy() {
        let f = fixture();
        let package = f.package.clone();
        submit(&f, 4, &package, "release-a");
        upload_all(&f, "release-a", &package);
        let intent = f
            .coordinator
            .begin_install_intent(&f.owner, "release-a")
            .unwrap();
        let foreign = f
            .mutation
            .try_acquire(MutationWorkload::Cleanup, "cleanup-plan", "other-owner")
            .unwrap();
        assert!(matches!(
            f.coordinator.claim_install(&f.owner, &intent.intent_id),
            Err(UpdateEngineError::Busy)
        ));
        drop(foreign);
        let snapshot = f
            .coordinator
            .cancel_install_intent(&f.owner, &intent.intent_id)
            .unwrap();
        assert_eq!(snapshot.state, UpdateState::Staged);
    }

    #[test]
    fn cleanup_does_not_erase_an_inflight_claim_reservation() {
        let f = fixture();
        let package = f.package.clone();
        submit(&f, 5, &package, "release-a");
        upload_all(&f, "release-a", &package);
        let intent = f
            .coordinator
            .begin_install_intent(&f.owner, "release-a")
            .unwrap();
        {
            let mut intents = f.coordinator.intents.lock().unwrap();
            intents.get_mut(&intent.intent_id).unwrap().claimed = true;
        }
        f.coordinator.cleanup_expired();
        assert!(
            f.coordinator
                .intents
                .lock()
                .unwrap()
                .contains_key(&intent.intent_id)
        );
        f.coordinator
            .intents
            .lock()
            .unwrap()
            .get_mut(&intent.intent_id)
            .unwrap()
            .claimed = false;
    }

    #[test]
    fn cancelled_install_intent_returns_snapshot_to_staged_and_cannot_be_claimed() {
        let f = fixture();
        let package = f.package.clone();
        submit(&f, 3, &package, "release-a");
        upload_all(&f, "release-a", &package);
        let intent = f
            .coordinator
            .begin_install_intent(&f.owner, "release-a")
            .unwrap();
        assert_eq!(
            f.coordinator.snapshot(&f.owner).state,
            UpdateState::AwaitingConsent
        );
        let snapshot = f
            .coordinator
            .cancel_install_intent(&f.owner, &intent.intent_id)
            .unwrap();
        assert_eq!(snapshot.state, UpdateState::Staged);
        assert!(matches!(
            f.coordinator.claim_install(&f.owner, &intent.intent_id),
            Err(UpdateEngineError::IntentUnavailable)
        ));
        assert!(!f.mutation.is_active());
    }
    #[test]
    fn durable_execution_recovery_restores_installing_snapshot_and_release_identity() {
        let f = fixture();
        let package = f.package.clone();
        submit(&f, 5, &package, "release-a");
        upload_all(&f, "release-a", &package);
        let intent = f
            .coordinator
            .begin_install_intent(&f.owner, "release-a")
            .unwrap();
        let ticket = f
            .coordinator
            .claim_install(&f.owner, &intent.intent_id)
            .unwrap();
        let recovered_mutation = MutationSupervisor::new();
        let recovered = UpdateCoordinator::with_dependencies(
            "1.0.0",
            22621,
            f.coordinator.trust.clone(),
            f.coordinator.staging_root.clone(),
            f.coordinator.db.clone(),
            recovered_mutation.clone(),
            Arc::new(AcceptVerifier),
        );
        recovered.recover_execution_guard().unwrap();
        let snapshot = recovered.snapshot(&f.owner);
        assert_eq!(snapshot.state, UpdateState::Installing);
        let staged = snapshot.staged_release.expect("recovered staged release");
        assert_eq!(staged.release_id, "release-a");
        assert_eq!(staged.version, "1.1.0");
        assert_eq!(staged.channel, UpdateChannel::Stable);
        assert!(recovered_mutation.is_active());
        f.coordinator
            .complete_install(&f.owner, &ticket.ticket_id, 1)
            .unwrap();
    }

    #[test]
    fn staged_path_is_service_derived_and_update_lease_is_machine_exclusive() {
        let f = fixture();
        let package = f.package.clone();
        submit(&f, 2, &package, "release-a");
        let staged = upload_all(&f, "release-a", &package);
        assert_eq!(staged.state, UpdateState::Staged);
        let intent = f
            .coordinator
            .begin_install_intent(&f.owner, "release-a")
            .unwrap();
        let foreign = f
            .mutation
            .try_acquire(MutationWorkload::Cleanup, "cleanup-plan", "other-owner")
            .unwrap();
        assert!(matches!(
            f.coordinator.claim_install(&f.owner, &intent.intent_id),
            Err(UpdateEngineError::Busy)
        ));
        drop(foreign);
        let ticket = f
            .coordinator
            .claim_install(&f.owner, &intent.intent_id)
            .unwrap();
        assert!(ticket.staged_path.starts_with(f.root.join("staging")));
        let expected = f
            .coordinator
            .expected_staged_path(&f.owner, "release-a", &ticket.expected_sha256)
            .unwrap();
        assert_eq!(ticket.staged_path, expected);
        assert!(matches!(
            f.coordinator.claim_install(&f.owner, &intent.intent_id),
            Err(UpdateEngineError::IntentUnavailable)
        ));
        let completed = f
            .coordinator
            .complete_install(&f.owner, &ticket.ticket_id, 1)
            .unwrap();
        assert!(!completed.succeeded);
        assert!(!f.mutation.is_active());
    }

    #[test]
    fn inactive_update_owner_state_cache_is_bounded() {
        let f = fixture();
        {
            let mut states = f.coordinator.states.lock().unwrap();
            for index in 0..MAX_OWNER_STATE_CACHE {
                let mut snapshot = f.coordinator.initial_snapshot(UpdateChannel::Stable);
                snapshot.state = UpdateState::UpToDate;
                snapshot.updated_unix_ms = index as i64;
                states.insert(
                    format!("historical-owner-{index}"),
                    OwnerState {
                        snapshot,
                        releases: HashMap::new(),
                        staged: None,
                    },
                );
            }
            admit_owner_state(&mut states, "new-owner");
            let snapshot = f.coordinator.initial_snapshot(UpdateChannel::Stable);
            states.insert(
                "new-owner".into(),
                OwnerState {
                    snapshot,
                    releases: HashMap::new(),
                    staged: None,
                },
            );
            assert_eq!(states.len(), MAX_OWNER_STATE_CACHE);
            assert!(!states.contains_key("historical-owner-0"));
            assert!(states.contains_key("new-owner"));
        }
    }

    #[test]
    fn mutation_critical_update_state_is_never_evicted() {
        let f = fixture();
        let protected_owner = "protected-owner";
        {
            let mut states = f.coordinator.states.lock().unwrap();
            let mut protected = f.coordinator.initial_snapshot(UpdateChannel::Stable);
            protected.state = UpdateState::AwaitingConsent;
            protected.updated_unix_ms = 0;
            states.insert(
                protected_owner.into(),
                OwnerState {
                    snapshot: protected,
                    releases: HashMap::new(),
                    staged: None,
                },
            );
            for index in 1..MAX_OWNER_STATE_CACHE {
                let mut snapshot = f.coordinator.initial_snapshot(UpdateChannel::Stable);
                snapshot.state = UpdateState::UpToDate;
                snapshot.updated_unix_ms = index as i64;
                states.insert(
                    format!("historical-owner-{index}"),
                    OwnerState {
                        snapshot,
                        releases: HashMap::new(),
                        staged: None,
                    },
                );
            }
            admit_owner_state(&mut states, "new-owner");
            assert!(states.contains_key(protected_owner));
            assert_eq!(states.len(), MAX_OWNER_STATE_CACHE - 1);
        }
    }
}
