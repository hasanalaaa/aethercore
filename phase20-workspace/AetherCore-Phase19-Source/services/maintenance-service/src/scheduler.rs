use std::{sync::Arc, time::Duration};

use aethercore_collector_runtime::{CancellationToken, CommitFence};
use aethercore_contracts::v1::{event_envelope, EventKind};
use aethercore_idle_scheduler::{
    AutonomousWorkload, IdleScheduler, PassiveWorkExecutor, PassiveWorkReport, ResourceGovernor,
    SchedulerConfig, SchedulerHandle, SchedulerStartError, WindowsSystemStateProbe,
};

use crate::{
    protocol::{cleanup_snapshot_proto, diagnostics_snapshot_proto, driver_hub_proto, startup_snapshot_proto},
    router::ServiceContext,
    streaming::publish,
};

#[derive(Clone)]
struct ServicePassiveExecutor { context: ServiceContext }

fn io_reservation(workload: AutonomousWorkload) -> u64 {
    match workload {
        AutonomousWorkload::HardwareTelemetry => 512 * 1024,
        AutonomousWorkload::EventLogTriage => 2 * 1024 * 1024,
        AutonomousWorkload::DriverDiscovery | AutonomousWorkload::CleanupInventory => 4 * 1024 * 1024,
        AutonomousWorkload::StartupInventory => 1024 * 1024,
    }
}

impl PassiveWorkExecutor for ServicePassiveExecutor {
    fn execute(
        &self,
        workload: AutonomousWorkload,
        owner: &str,
        token: CancellationToken,
        commit_fence: CommitFence,
        governor: &ResourceGovernor,
    ) -> Result<PassiveWorkReport, String> {
        // Opaque Windows providers cannot be preempted at arbitrary kernel instruction boundaries.
        // Reserve a deliberately small process-local duty-cycle budget before entering them; the
        // worker itself also runs in Windows background mode. Every wait here is cancellable.
        governor
            .account_cpu_cancellable(&token, Duration::from_millis(10))
            .map_err(|_| "cancelled".to_owned())?;
        governor
            .account_io_cancellable(&token, io_reservation(workload))
            .map_err(|_| "cancelled".to_owned())?;

        match workload {
            AutonomousWorkload::HardwareTelemetry => {
                let snapshot = self.context.diagnostics
                    .passive_hardware_refresh(owner, token, commit_fence.clone())
                    .map_err(|e| e.to_string())?;
                if commit_fence.is_committed() {
                    publish(&self.context, owner, EventKind::Diagnostics, "", Some(
                        event_envelope::Payload::DiagnosticsSnapshot(diagnostics_snapshot_proto(snapshot.clone()))
                    ));
                }
                Ok(PassiveWorkReport {
                    evidence_count: snapshot.storage.len() as u32 + u32::from(snapshot.memory.is_some()),
                    warning_count: (snapshot.warnings.len() + snapshot.provider_faults.len()) as u32,
                })
            }
            AutonomousWorkload::EventLogTriage => {
                let snapshot = self.context.diagnostics
                    .passive_event_log_refresh(owner, token, commit_fence.clone())
                    .map_err(|e| e.to_string())?;
                if commit_fence.is_committed() {
                    publish(&self.context, owner, EventKind::Diagnostics, "", Some(
                        event_envelope::Payload::DiagnosticsSnapshot(diagnostics_snapshot_proto(snapshot.clone()))
                    ));
                }
                Ok(PassiveWorkReport {
                    evidence_count: (snapshot.events.len() + snapshot.crashes.len()) as u32,
                    warning_count: (snapshot.warnings.len() + snapshot.provider_faults.len()) as u32,
                })
            }
            AutonomousWorkload::DriverDiscovery => {
                let snapshot = self.context.driver_hub
                    .passive_scan_with_fence(owner, token, commit_fence.clone())
                    .map_err(|e| e.to_string())?;
                if commit_fence.is_committed() {
                    publish(&self.context, owner, EventKind::DriverDiscovery, "", Some(
                        event_envelope::Payload::DriverHubSnapshot(driver_hub_proto(snapshot.clone()))
                    ));
                }
                Ok(PassiveWorkReport {
                    evidence_count: snapshot.summary.device_count,
                    warning_count: snapshot.warnings.len() as u32,
                })
            }
            AutonomousWorkload::CleanupInventory => {
                let snapshot = self.context.cleaner
                    .passive_scan_with_fence(owner, token, commit_fence.clone())
                    .map_err(|e| e.to_string())?;
                if commit_fence.is_committed() {
                    publish(&self.context, owner, EventKind::CleanupDiscovery, "", Some(
                        event_envelope::Payload::CleanupSnapshot(cleanup_snapshot_proto(snapshot.clone()))
                    ));
                }
                Ok(PassiveWorkReport {
                    evidence_count: snapshot.candidates.len() as u32,
                    warning_count: snapshot.warnings.len() as u32,
                })
            }
            AutonomousWorkload::StartupInventory => {
                let snapshot = self.context.startup
                    .passive_scan_with_fence(owner, token, commit_fence.clone())
                    .map_err(|e| e.to_string())?;
                if commit_fence.is_committed() {
                    publish(&self.context, owner, EventKind::StartupDiscovery, "", Some(
                        event_envelope::Payload::StartupSnapshot(startup_snapshot_proto(snapshot.clone()))
                    ));
                }
                Ok(PassiveWorkReport {
                    evidence_count: snapshot.items.len() as u32,
                    warning_count: snapshot.warnings.len() as u32,
                })
            }
        }
    }
}

pub fn start(context: &ServiceContext) -> Result<SchedulerHandle, SchedulerStartError> {
    IdleScheduler::start(
        context.kernel.clone(),
        Arc::new(WindowsSystemStateProbe::new()),
        Arc::new(ServicePassiveExecutor { context: context.clone() }),
        SchedulerConfig::default(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn autonomous_reservations_never_exceed_one_governor_io_window() {
        for workload in AutonomousWorkload::ALL {
            assert!(io_reservation(workload) <= 4 * 1024 * 1024);
        }
    }
}
