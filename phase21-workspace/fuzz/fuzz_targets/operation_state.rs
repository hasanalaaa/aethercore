#![no_main]
use libfuzzer_sys::fuzz_target;
use aethercore_operation_engine::PlanState;

fuzz_target!(|data: &[u8]| {
    let value = String::from_utf8_lossy(data);
    if let Some(state) = PlanState::parse(&value) {
        assert_eq!(PlanState::parse(state.as_str()), Some(state));
        assert_eq!(state.is_terminal(), matches!(state, PlanState::Completed | PlanState::Failed));
    }
    for state in [
        PlanState::Draft, PlanState::Scanning, PlanState::ReadyForReview,
        PlanState::AwaitingAuthorization, PlanState::Preflight, PlanState::Protected,
        PlanState::Executing, PlanState::Verifying, PlanState::Completed, PlanState::Failed,
        PlanState::RebootPending, PlanState::Resuming,
    ] {
        assert_eq!(PlanState::parse(state.as_str()), Some(state));
    }
});
