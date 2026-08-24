//! Phase 20 Domain C tests — safe-optimization governance under adversarial pressure.
//!
//! These tests exist to prove the anti-snake-oil invariants hold even when a caller tries to
//! abuse the planner: no destructive operations, mandatory consent for persistent changes,
//! single-flight machine mutation, deterministic plans, and working restore paths.

use std::collections::BTreeMap;
use std::sync::Arc;

use aethercore_operation_kernel::MutationSupervisor;
use aethercore_performance_bottleneck::{Confidence, Report, Role};
use aethercore_performance_optimization::{
    ActionKind, ApplyOutcome, ChangeRecord, NoopPlatform, OptimizationError, OptimizationGovernor,
    OptimizationPlatform, Plan, ProcessKey, Reversibility,
};

const OWNER_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const OWNER_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

fn finding(id: &str, code: &str, actions: &[&str]) -> aethercore_performance_bottleneck::Finding {
    aethercore_performance_bottleneck::Finding {
        id: id.to_string(),
        code: code.to_string(),
        role: Role::RootCause,
        confidence: Confidence::Confirmed,
        caused_by_finding_ids: Vec::new(),
        title_key: format!("t.{code}"),
        summary_key: format!("s.{code}"),
        message_args: Vec::new(),
        evidence: Vec::new(),
        applicable_action_kinds: actions.iter().map(|action| action.to_string()).collect(),
        first_observed_unix_ms: 1_700_000_000_000,
        last_observed_unix_ms: 1_700_000_010_000,
    }
}

fn report_with(findings: Vec<aethercore_performance_bottleneck::Finding>) -> Report {
    Report {
        report_id: "bottleneck-test".into(),
        generated_unix_ms: 1_700_000_020_000,
        analyzed_sample_count: 42,
        analysis_window_ms: 60_000,
        findings,
        digest_sha256: "digest".into(),
        rule_engine_version: "test".into(),
    }
}

fn governor() -> (OptimizationGovernor, Arc<NoopPlatform>) {
    let platform = Arc::new(NoopPlatform::new());
    let governor = OptimizationGovernor::new(platform.clone(), MutationSupervisor::new());
    (governor, platform)
}

fn offender(pid: u32) -> BTreeMap<String, Vec<ProcessKey>> {
    let mut map = BTreeMap::new();
    map.insert(
        "CPU_SATURATION".to_string(),
        vec![ProcessKey { pid, image_key: "abc123def456".into() }],
    );
    map
}

#[test]
fn plan_is_deterministic_and_immutable() {
    let (governor, _) = governor();
    let report = report_with(vec![finding(
        "finding:cpu_saturation",
        "CPU_SATURATION",
        &["ecoQos", "backgroundPriority"],
    )]);
    let selected = vec!["finding:cpu_saturation".to_string()];
    let plan_a = governor.create_plan(&report, &selected, &offender(4242)).unwrap();
    let plan_b = governor.create_plan(&report, &selected, &offender(4242)).unwrap();
    // Digest covers candidates; plan ids differ so digests differ per plan instance — but the
    // candidate sets and their order must be identical.
    assert_eq!(plan_a.candidates, plan_b.candidates);
    assert_eq!(plan_a.candidates.len(), 2);
    assert!(plan_a.immutable);
}

#[test]
fn empty_selection_is_rejected() {
    let (governor, _) = governor();
    let report = report_with(vec![finding("f1", "CPU_SATURATION", &["ecoQos"])]);
    let error = governor.create_plan(&report, &[], &BTreeMap::new()).unwrap_err();
    assert!(matches!(error, OptimizationError::EmptyPlan));
}

#[test]
fn findings_without_actions_never_become_candidates() {
    let (governor, _) = governor();
    // STANDBY_STARVATION carries no applicable actions by design.
    let report = report_with(vec![finding("finding:standby", "STANDBY_STARVATION", &[])]);
    let error = governor
        .create_plan(&report, &["finding:standby".to_string()], &BTreeMap::new())
        .unwrap_err();
    assert!(matches!(error, OptimizationError::NoEligibleFindings));
}

#[test]
fn session_only_hints_do_not_require_consent_but_persistent_changes_do() {
    let (governor, _) = governor();
    let report = report_with(vec![finding(
        "finding:cpu",
        "CPU_SATURATION",
        &["ecoQos", "cooperativeTrimRequest"],
    )]);
    let plan = governor
        .create_plan(&report, &["finding:cpu".to_string()], &offender(99))
        .unwrap();
    let eco = plan.candidates.iter().find(|candidate| candidate.kind == ActionKind::EcoQos).unwrap();
    let trim = plan
        .candidates
        .iter()
        .find(|candidate| candidate.kind == ActionKind::CooperativeTrimRequest)
        .unwrap();
    assert_eq!(eco.reversibility, Reversibility::SessionOnly);
    assert!(!eco.requires_explicit_consent);
    assert_eq!(trim.reversibility, Reversibility::AutomaticRestore);
    assert!(trim.requires_explicit_consent, "memory changes must always demand explicit consent");
}

#[test]
fn execution_records_operations_and_journals_restorable_changes() {
    let (governor, platform) = governor();
    let report = report_with(vec![finding(
        "finding:cpu",
        "CPU_SATURATION",
        &["ecoQos"],
    )]);
    let plan = governor
        .create_plan(&report, &["finding:cpu".to_string()], &offender(4242))
        .unwrap();
    let status = governor.start(OWNER_A, &plan, Default::default()).unwrap();
    assert_eq!(status.plan_state, "Completed");
    assert!(status.mutation_started);
    assert!(status.items.iter().all(|item| item.result_code == "Applied"));
    let ops = platform.operations();
    assert_eq!(ops, vec!["ecoQos:4242/abc123def456".to_string()]);
    assert_eq!(governor.history_len(), 1);
}

#[test]
fn restore_path_marks_history_and_invokes_platform_restore() {
    let (governor, platform) = governor();
    let report = report_with(vec![finding("finding:cpu", "CPU_SATURATION", &["ecoQos"])]);
    let plan = governor
        .create_plan(&report, &["finding:cpu".to_string()], &offender(7))
        .unwrap();
    governor.start(OWNER_A, &plan, Default::default()).unwrap();
    let restored = governor.restore_plan_changes(&plan.plan_id).unwrap();
    assert_eq!(restored, 1);
    assert!(platform.operations().iter().any(|op| op.starts_with("restore:")));
    // Second restore is idempotent — nothing left to undo.
    assert_eq!(governor.restore_plan_changes(&plan.plan_id).unwrap(), 0);
}

#[test]
fn concurrent_execution_on_one_governor_is_single_flight() {
    use std::sync::{Arc, Barrier};
    let (inner, _) = governor();
    let governor = Arc::new(inner);
    let report = report_with(vec![finding("finding:cpu", "CPU_SATURATION", &["ecoQos"])]);
    let plan = Arc::new(
        governor
            .create_plan(&report, &["finding:cpu".to_string()], &offender(1))
            .unwrap(),
    );
    let barrier = Arc::new(Barrier::new(8));
    let mut handles = Vec::new();
    for _ in 0..8 {
        let governor = governor.clone();
        let plan = plan.clone();
        let barrier = barrier.clone();
        handles.push(std::thread::spawn(move || {
            barrier.wait();
            governor.start(OWNER_A, &plan, Default::default())
        }));
    }
    let outcomes: Vec<_> = handles.into_iter().map(|handle| handle.join().unwrap()).collect();
    let ok = outcomes.iter().filter(|result| result.is_ok()).count();
    assert_eq!(ok, 1, "exactly one execution may win the machine lease");
    assert!(outcomes.iter().all(|result| match result {
        Ok(_) => true,
        Err(OptimizationError::Busy) => true,
        Err(_) => false,
    }));
}

#[test]
fn noop_platform_can_never_emit_out_of_contract_operations() {
    let platform = NoopPlatform::new();
    let key = ProcessKey { pid: 5, image_key: "deadbeef".into() };
    let _ = platform.apply_eco_qos(&key);
    let _ = platform.apply_background_priority(&key);
    let _ = platform.apply_cooperative_trim(&key);
    let _ = platform.set_game_mode(true);
    let ops = platform.operations();
    assert!(ops.iter().all(|op| {
        op.starts_with("ecoQos:")
            || op.starts_with("backgroundPriority:")
            || op.starts_with("cooperativeTrim:")
            || op.starts_with("gameMode:")
            || op.starts_with("restore:")
    }));
    assert!(
        !ops.iter().any(|op| op.contains("delete") || op.contains("clean") || op.contains("purge")),
        "no destructive verb may ever appear in the operation log"
    );
}

/// A hostile platform that fails every apply; the executor must degrade into per-item failures
/// instead of poisoning global state or leaving the lease stuck.
struct FailingPlatform;

impl OptimizationPlatform for FailingPlatform {
    fn apply_eco_qos(&self, _: &ProcessKey) -> std::io::Result<ApplyOutcome> {
        Err(std::io::Error::other("injected failure"))
    }

    fn apply_background_priority(&self, _: &ProcessKey) -> std::io::Result<ApplyOutcome> {
        Err(std::io::Error::other("injected failure"))
    }

    fn apply_cooperative_trim(&self, _: &ProcessKey) -> std::io::Result<ApplyOutcome> {
        Err(std::io::Error::other("injected failure"))
    }

    fn set_game_mode(&self, _: bool) -> std::io::Result<ApplyOutcome> {
        Err(std::io::Error::other("injected failure"))
    }

    fn restore(&self, _: &ChangeRecord) -> std::io::Result<ApplyOutcome> {
        Ok(ApplyOutcome::Applied)
    }
}

#[test]
fn platform_failures_degrade_to_item_errors_not_poisoned_state() {
    let platform = Arc::new(FailingPlatform);
    let governor = OptimizationGovernor::new(platform.clone(), MutationSupervisor::new());
    let report = report_with(vec![finding("finding:cpu", "CPU_SATURATION", &["ecoQos"])]);
    let plan = governor
        .create_plan(&report, &["finding:cpu".to_string()], &offender(3))
        .unwrap();
    let status = governor.start(OWNER_A, &plan, Default::default()).unwrap();
    assert_eq!(status.plan_state, "Completed");
    assert!(status.items.iter().all(|item| item.result_code == "ApplyError" && !item.verified));
    assert!(!status.mutation_started, "no successful mutation may be claimed");
    // The supervisor must be free again after the failed run.
    assert!(governor.status(&plan.plan_id).is_some());
}
