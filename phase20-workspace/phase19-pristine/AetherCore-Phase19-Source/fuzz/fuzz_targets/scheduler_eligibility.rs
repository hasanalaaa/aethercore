#![no_main]
use std::{collections::HashSet, time::Duration};
use libfuzzer_sys::fuzz_target;
use aethercore_idle_scheduler::{
    AutonomousWorkload, EligibilityEngine, NetworkCost, PresentationState, SchedulerConfig,
    ServicingState, SystemState, ThermalPressure, BlockReason,
};

fn pick<T: Copy>(byte: u8, values: &[T]) -> T { values[(byte as usize) % values.len()] }

fuzz_target!(|data: &[u8]| {
    if data.len() < 8 { return; }
    let thermal = [ThermalPressure::Normal, ThermalPressure::Elevated, ThermalPressure::Critical, ThermalPressure::Unknown];
    let network = [NetworkCost::Unmetered, NetworkCost::Metered, NetworkCost::Unknown];
    let presentation = [PresentationState::Clear, PresentationState::Busy, PresentationState::FullScreen, PresentationState::Presentation, PresentationState::LockedOrAbsent, PresentationState::Unknown];
    let servicing = [ServicingState::Idle, ServicingState::Busy, ServicingState::Unknown];
    let state = SystemState {
        sampled_unix_ms: i64::from(data[0]),
        owner_principal_key: if data[1] & 1 == 0 { "owner".into() } else { String::new() },
        user_sid: if data[1] & 2 == 0 { "S-1-5-21-test".into() } else { String::new() },
        session_id: if data[1] & 4 == 0 { 1 } else { u32::MAX },
        idle_for: Duration::from_secs(u64::from(data[2]) * 5),
        session_unlocked: data[3] & 1 != 0,
        on_ac_power: data[3] & 2 != 0,
        battery_saver: data[3] & 4 != 0,
        thermal_pressure: pick(data[4], &thermal),
        network_cost: pick(data[5], &network),
        presentation: pick(data[6], &presentation),
        servicing: pick(data[7], &servicing),
    };
    let engine = EligibilityEngine::new(SchedulerConfig::default());
    for workload in AutonomousWorkload::ALL {
        let first = engine.evaluate(workload, &state, false);
        let second = engine.evaluate(workload, &state, false);
        assert_eq!(first, second, "eligibility must be deterministic for an identical snapshot");
        let unique: HashSet<_> = first.iter().map(BlockReason::as_str).collect();
        assert_eq!(unique.len(), first.len(), "duplicate block reasons make policy evidence ambiguous");
        let with_mutation = engine.evaluate(workload, &state, true);
        assert!(with_mutation.contains(&BlockReason::MutationActive));
    }
});
