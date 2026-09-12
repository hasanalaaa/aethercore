use std::{
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

use aethercore_cleaner::{CleanupEngine, CleanupScanState, CleanupSnapshot};
use aethercore_collector_runtime::CancellationToken;
use aethercore_diagnostic_engine::{
    DiagnosticEngine, DiagnosticsSnapshot, ScanState as DiagnosticScanState,
};
use aethercore_driver_hub::{DriverHub, DriverHubSnapshot, ScanState as DriverScanState};
use aethercore_operation_kernel::{ReadBudgetManager, ReadWorkload};
use aethercore_startup_manager::{StartupManager, StartupScanState, StartupSnapshot};
use aethercore_system_repair::{RepairAssessment, RepairAssessmentState, RepairCoordinator};
use aethercore_update_engine::{UpdateCoordinator, UpdateSnapshot};
use thiserror::Error;

const WAIT_SLICE: Duration = Duration::from_millis(100);
const DISCOVERY_TIMEOUT: Duration = Duration::from_secs(10 * 60);

#[derive(Debug, Error, Clone)]
pub enum SourceError {
    #[error("collector cancelled")]
    Cancelled,
    #[error("collector resource budget unavailable")]
    Budget,
    #[error("collector timed out")]
    TimedOut,
    #[error("collector unavailable: {0}")]
    Unavailable(String),
}

pub trait DeepScanBackend: Send + Sync + 'static {
    fn driver(
        &self,
        owner: &str,
        token: &CancellationToken,
    ) -> Result<DriverHubSnapshot, SourceError>;
    fn repair(
        &self,
        owner: &str,
        token: &CancellationToken,
    ) -> Result<RepairAssessment, SourceError>;
    fn diagnostics(
        &self,
        owner: &str,
        token: &CancellationToken,
    ) -> Result<DiagnosticsSnapshot, SourceError>;
    fn startup(
        &self,
        owner: &str,
        token: &CancellationToken,
    ) -> Result<StartupSnapshot, SourceError>;
    fn cleanup(
        &self,
        owner: &str,
        token: &CancellationToken,
    ) -> Result<CleanupSnapshot, SourceError>;
    fn update(&self, owner: &str) -> UpdateSnapshot;
}

pub struct ExistingSubsystemBackend {
    pub reads: ReadBudgetManager,
    pub driver_hub: Arc<DriverHub>,
    pub repair: Arc<RepairCoordinator>,
    pub diagnostics: Arc<DiagnosticEngine>,
    pub startup: Arc<StartupManager>,
    pub cleanup: Arc<CleanupEngine>,
    pub updates: Arc<UpdateCoordinator>,
}

impl ExistingSubsystemBackend {
    pub fn new(
        reads: ReadBudgetManager,
        driver_hub: Arc<DriverHub>,
        repair: Arc<RepairCoordinator>,
        diagnostics: Arc<DiagnosticEngine>,
        startup: Arc<StartupManager>,
        cleanup: Arc<CleanupEngine>,
        updates: Arc<UpdateCoordinator>,
    ) -> Self {
        Self {
            reads,
            driver_hub,
            repair,
            diagnostics,
            startup,
            cleanup,
            updates,
        }
    }
}

impl DeepScanBackend for ExistingSubsystemBackend {
    fn driver(
        &self,
        owner: &str,
        token: &CancellationToken,
    ) -> Result<DriverHubSnapshot, SourceError> {
        let lease = self
            .reads
            .try_acquire(ReadWorkload::DriverDiscovery)
            .map_err(|_| SourceError::Budget)?;
        self.driver_hub
            .start_scan_with_lease(owner, lease)
            .map_err(|e| SourceError::Unavailable(e.to_string()))?;
        wait(
            token,
            || {
                self.driver_hub
                    .snapshot_for_owner(owner)
                    .map_err(|e| SourceError::Unavailable(e.to_string()))
            },
            |v| {
                !matches!(
                    v.state,
                    DriverScanState::InventoryScanning
                        | DriverScanState::UpdateSearching
                        | DriverScanState::Matching
                )
            },
        )
    }
    fn repair(
        &self,
        owner: &str,
        token: &CancellationToken,
    ) -> Result<RepairAssessment, SourceError> {
        let lease = self
            .reads
            .try_acquire(ReadWorkload::RepairAssessment)
            .map_err(|_| SourceError::Budget)?;
        self.repair
            .start_assessment_with_lease(owner, lease)
            .map_err(|e| SourceError::Unavailable(e.to_string()))?;
        wait(
            token,
            || {
                self.repair
                    .assessment_for_owner(owner)
                    .map_err(|e| SourceError::Unavailable(e.to_string()))
            },
            |v| v.state != RepairAssessmentState::Scanning,
        )
    }
    fn diagnostics(
        &self,
        owner: &str,
        token: &CancellationToken,
    ) -> Result<DiagnosticsSnapshot, SourceError> {
        let lease = self
            .reads
            .try_acquire(ReadWorkload::Diagnostics)
            .map_err(|_| SourceError::Budget)?;
        self.diagnostics
            .start_scan_with_lease(owner, lease)
            .map_err(|e| SourceError::Unavailable(e.to_string()))?;
        wait(
            token,
            || {
                self.diagnostics
                    .snapshot_for_owner(owner)
                    .map_err(|e| SourceError::Unavailable(e.to_string()))
            },
            |v| v.state != DiagnosticScanState::Collecting,
        )
    }
    fn startup(
        &self,
        owner: &str,
        token: &CancellationToken,
    ) -> Result<StartupSnapshot, SourceError> {
        let lease = self
            .reads
            .try_acquire(ReadWorkload::StartupDiscovery)
            .map_err(|_| SourceError::Budget)?;
        self.startup
            .start_scan_with_lease(owner, lease)
            .map_err(|e| SourceError::Unavailable(e.to_string()))?;
        wait(
            token,
            || {
                self.startup
                    .snapshot_for_owner(owner)
                    .map_err(|e| SourceError::Unavailable(e.to_string()))
            },
            |v| v.state != StartupScanState::Scanning,
        )
    }
    fn cleanup(
        &self,
        owner: &str,
        token: &CancellationToken,
    ) -> Result<CleanupSnapshot, SourceError> {
        let lease = self
            .reads
            .try_acquire(ReadWorkload::CleanupDiscovery)
            .map_err(|_| SourceError::Budget)?;
        self.cleanup
            .start_scan_with_lease(owner, lease)
            .map_err(|e| SourceError::Unavailable(e.to_string()))?;
        wait(
            token,
            || {
                self.cleanup
                    .snapshot_for_owner(owner)
                    .map_err(|e| SourceError::Unavailable(e.to_string()))
            },
            |v| v.state != CleanupScanState::Scanning,
        )
    }
    fn update(&self, owner: &str) -> UpdateSnapshot {
        self.updates.snapshot(owner)
    }
}

fn wait<T>(
    token: &CancellationToken,
    mut read: impl FnMut() -> Result<T, SourceError>,
    done: impl Fn(&T) -> bool,
) -> Result<T, SourceError> {
    let deadline = Instant::now() + DISCOVERY_TIMEOUT;
    loop {
        if token.is_cancelled() {
            return Err(SourceError::Cancelled);
        }
        if Instant::now() >= deadline {
            return Err(SourceError::TimedOut);
        }
        let value = read()?;
        if done(&value) {
            return Ok(value);
        }
        thread::sleep(WAIT_SLICE)
    }
}
