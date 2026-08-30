use std::{collections::HashMap, time::Duration};
use crate::model::*;

#[derive(Clone, Copy, Debug)]
pub struct WorkloadPolicy {
    pub minimum_interval: Duration,
    pub timeout: Duration,
}

#[derive(Clone)]
pub struct EligibilityEngine {
    config: SchedulerConfig,
    policies: HashMap<AutonomousWorkload, WorkloadPolicy>,
}

impl EligibilityEngine {
    pub fn new(config: SchedulerConfig) -> Self {
        let policies = HashMap::from([
            (AutonomousWorkload::HardwareTelemetry, WorkloadPolicy { minimum_interval: Duration::from_secs(6 * 60 * 60), timeout: Duration::from_secs(20) }),
            (AutonomousWorkload::DriverDiscovery, WorkloadPolicy { minimum_interval: Duration::from_secs(12 * 60 * 60), timeout: Duration::from_secs(45) }),
            (AutonomousWorkload::CleanupInventory, WorkloadPolicy { minimum_interval: Duration::from_secs(6 * 60 * 60), timeout: Duration::from_secs(30) }),
            (AutonomousWorkload::StartupInventory, WorkloadPolicy { minimum_interval: Duration::from_secs(12 * 60 * 60), timeout: Duration::from_secs(20) }),
            (AutonomousWorkload::EventLogTriage, WorkloadPolicy { minimum_interval: Duration::from_secs(3 * 60 * 60), timeout: Duration::from_secs(20) }),
        ]);
        Self { config, policies }
    }

    pub fn policy(&self, workload: AutonomousWorkload) -> WorkloadPolicy {
        self.policies[&workload]
    }

    pub fn evaluate(&self, workload: AutonomousWorkload, state: &SystemState, mutation_active: bool) -> Vec<BlockReason> {
        let mut blocked = Vec::new();
        if !state.has_interactive_owner() { blocked.push(BlockReason::NoInteractiveOwner); }
        if !state.session_unlocked || matches!(state.presentation, PresentationState::LockedOrAbsent) { blocked.push(BlockReason::SessionLocked); }
        if state.idle_for < self.config.minimum_idle { blocked.push(BlockReason::UserActive); }
        if !state.on_ac_power { blocked.push(BlockReason::BatteryPower); }
        if state.battery_saver { blocked.push(BlockReason::BatterySaver); }
        if matches!(state.presentation, PresentationState::Busy | PresentationState::FullScreen | PresentationState::Presentation) { blocked.push(BlockReason::Presentation); }
        if matches!(state.presentation, PresentationState::Unknown) { blocked.push(BlockReason::PresentationUnknown); }
        if matches!(state.servicing, ServicingState::Busy) { blocked.push(BlockReason::Servicing); }
        if matches!(state.servicing, ServicingState::Unknown) { blocked.push(BlockReason::ServicingUnknown); }
        if mutation_active { blocked.push(BlockReason::MutationActive); }
        if workload.thermal_sensitive() && matches!(state.thermal_pressure, ThermalPressure::Elevated | ThermalPressure::Critical) { blocked.push(BlockReason::ThermalPressure); }
        if workload.thermal_sensitive() && matches!(state.thermal_pressure, ThermalPressure::Unknown) { blocked.push(BlockReason::ThermalPressureUnknown); }
        if workload.network_sensitive() {
            match state.network_cost {
                NetworkCost::Metered => blocked.push(BlockReason::MeteredNetwork),
                NetworkCost::Unknown => blocked.push(BlockReason::NetworkCostUnknown),
                NetworkCost::Unmetered => {}
            }
        }
        blocked
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn state() -> SystemState { SystemState {
        sampled_unix_ms: 1, owner_principal_key: "owner".into(), user_sid: "sid".into(), session_id: 1,
        idle_for: Duration::from_secs(600), session_unlocked: true, on_ac_power: true, battery_saver: false,
        thermal_pressure: ThermalPressure::Normal, network_cost: NetworkCost::Unmetered,
        presentation: PresentationState::Clear, servicing: ServicingState::Idle,
    }}

    #[test] fn mutations_block_every_autonomous_workload() {
        let e=EligibilityEngine::new(SchedulerConfig::default());
        for w in AutonomousWorkload::ALL { assert!(e.evaluate(w,&state(),true).contains(&BlockReason::MutationActive)); }
    }
    #[test] fn user_activity_preempts_all_workloads() {
        let e=EligibilityEngine::new(SchedulerConfig::default()); let mut s=state();s.idle_for=Duration::from_secs(1);
        for w in AutonomousWorkload::ALL { assert!(e.evaluate(w,&s,false).contains(&BlockReason::UserActive)); }
    }
    #[test] fn metered_and_unknown_network_only_block_network_sensitive_discovery() {
        let e=EligibilityEngine::new(SchedulerConfig::default()); let mut s=state();s.network_cost=NetworkCost::Metered;
        assert!(e.evaluate(AutonomousWorkload::DriverDiscovery,&s,false).contains(&BlockReason::MeteredNetwork));
        assert!(!e.evaluate(AutonomousWorkload::StartupInventory,&s,false).contains(&BlockReason::MeteredNetwork));
        s.network_cost=NetworkCost::Unknown;
        assert!(e.evaluate(AutonomousWorkload::DriverDiscovery,&s,false).contains(&BlockReason::NetworkCostUnknown));
    }
    #[test] fn unknown_presentation_and_servicing_fail_closed() {
        let e=EligibilityEngine::new(SchedulerConfig::default()); let mut s=state();
        s.presentation=PresentationState::Unknown; assert!(e.evaluate(AutonomousWorkload::StartupInventory,&s,false).contains(&BlockReason::PresentationUnknown));
        s.presentation=PresentationState::Clear; s.servicing=ServicingState::Unknown; assert!(e.evaluate(AutonomousWorkload::StartupInventory,&s,false).contains(&BlockReason::ServicingUnknown));
    }

    #[test] fn unknown_thermal_blocks_heavy_inventory_but_not_lightweight_observation() {
        let e=EligibilityEngine::new(SchedulerConfig::default()); let mut s=state(); s.thermal_pressure=ThermalPressure::Unknown;
        assert!(e.evaluate(AutonomousWorkload::DriverDiscovery,&s,false).contains(&BlockReason::ThermalPressureUnknown));
        assert!(e.evaluate(AutonomousWorkload::CleanupInventory,&s,false).contains(&BlockReason::ThermalPressureUnknown));
        assert!(!e.evaluate(AutonomousWorkload::HardwareTelemetry,&s,false).contains(&BlockReason::ThermalPressureUnknown));
        assert!(!e.evaluate(AutonomousWorkload::EventLogTriage,&s,false).contains(&BlockReason::ThermalPressureUnknown));
    }

}
