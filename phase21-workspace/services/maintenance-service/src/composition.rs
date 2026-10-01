use std::{path::Path, sync::Arc};

use aethercore_cleaner::CleanupEngine;
use aethercore_diagnostic_engine::DiagnosticEngine;
use aethercore_driver_hub::DriverHub;
use aethercore_driver_install::DriverInstallCoordinator;
use aethercore_operation_engine::OperationEngine;
use aethercore_operation_kernel::OperationKernel;
use aethercore_pc_intelligence::{DeepScanCoordinator, ExistingSubsystemBackend};
use aethercore_persistence::Database;
use aethercore_startup_manager::StartupManager;
use aethercore_support_bundle::SupportBundleEngine;
use aethercore_system_repair::RepairCoordinator;
use aethercore_update_engine::UpdateCoordinator;
use anyhow::{Context, Result};

use crate::router::ServiceContext;

/// Phase 27 (unix composition): resolves the OS build number where the platform API
/// exists (Windows) and reports 0 elsewhere. 0 disables update eligibility checks, which
/// is already the honest state on non-Windows platforms.
fn current_windows_build_or_zero() -> u32 {
    aethercore_update_engine::current_windows_build().unwrap_or(0)
}

pub fn build(data_path: &Path, product_data_root: &Path) -> Result<ServiceContext> {
    let db = Arc::new(Database::open(data_path).context("open journal database")?);
    let engine = Arc::new(OperationEngine::new(db.clone()));
    let kernel = Arc::new(OperationKernel::new(engine.clone(), db.clone()));
    let driver_hub = Arc::new(DriverHub::with_database(db.clone()));
    let installer = Arc::new(DriverInstallCoordinator::with_platform_and_telemetry(
        engine.clone(),
        driver_hub.clone(),
        db.clone(),
        product_data_root.to_path_buf(),
        Arc::new(aethercore_driver_install::WindowsInstallPlatform),
        kernel.telemetry().clone(),
    ));
    let repair = Arc::new(RepairCoordinator::with_telemetry(
        engine.clone(),
        db.clone(),
        kernel.telemetry().clone(),
    ));
    let cleaner = Arc::new(CleanupEngine::with_telemetry(
        engine.clone(),
        db.clone(),
        kernel.telemetry().clone(),
    ));
    let startup = Arc::new(StartupManager::with_telemetry(
        engine.clone(),
        db.clone(),
        product_data_root.to_path_buf(),
        kernel.telemetry().clone(),
    ));
    let diagnostics = Arc::new(DiagnosticEngine::new(db.clone()));
    let support = Arc::new(
        SupportBundleEngine::new(product_data_root).context("initialize support bundle engine")?,
    );
    let product_dir = std::env::current_exe()
        .context("resolve installed service path")?
        .parent()
        .ok_or_else(|| anyhow::anyhow!("service executable has no parent directory"))?
        .to_path_buf();
    let event_bus = kernel.events().clone();
    let updates = UpdateCoordinator::load_with_build(
        env!("CARGO_PKG_VERSION"),
        &product_dir,
        product_data_root,
        db.clone(),
        kernel.mutations().clone(),
        Some(Arc::new(move |owner, snapshot| {
            event_bus.publish(
                &owner,
                aethercore_contracts::v1::EventKind::Update,
                "",
                Some(
                    aethercore_contracts::v1::event_envelope::Payload::UpdateSnapshot(
                        crate::protocol::update_snapshot_proto(snapshot),
                    ),
                ),
            );
        })),
        // Phase 27 (unix composition): Windows build detection is unavailable off-Windows.
        // Update trust stays disabled (no channels configured) and the coordinator degrades
        // to an inert state — the honest matrix already reports windowsUpdate NotAvailable
        // on macOS/Linux. Zero behavior change on Windows (build detected normally).
        current_windows_build_or_zero(),
    )
    .context("initialize secure update coordinator")?;
    let intelligence_backend = Arc::new(ExistingSubsystemBackend::new(
        kernel.reads().clone(),
        driver_hub.clone(),
        repair.clone(),
        diagnostics.clone(),
        startup.clone(),
        cleaner.clone(),
        updates.clone(),
    ));
    let intelligence = Arc::new(DeepScanCoordinator::new(
        db.clone(),
        intelligence_backend,
        env!("CARGO_PKG_VERSION"),
    ));

    // Phase 20: passive performance telemetry + optimization governance. Sampling starts only on
    // explicit owner request; construction here never touches counters.
    // Phase 28 (anti-snake-oil, P28-I-007): the unix composition previously wired the
    // synthetic platform while engine_source() reported "native" — a simulation labeled
    // as truth. The cfg-selected REAL provider (macOS/Linux/Windows native) is now the
    // default; the synthetic platform remains available ONLY under the explicit
    // force-synthetic-perf audit feature.
    #[cfg(feature = "force-synthetic-perf")]
    let performance = Arc::new(crate::performance::PerformanceEngine::with_synthetic(
        kernel.mutations().clone(),
    ));
    #[cfg(not(feature = "force-synthetic-perf"))]
    let performance = Arc::new(crate::performance::PerformanceEngine::new(
        aethercore_performance_telemetry::default_platform(),
        kernel.mutations().clone(),
    ));

    // Phase 21: Timeline Intelligence. Read-only over persisted history; construction
    // never mutates the journal and every query is principal-scoped + bounded later.
    let timeline = Arc::new(crate::timeline::TimelineCoordinator::new(db.clone()));

    // Phase 22/23 Part A: typed dispatch into the real domain coordinators. Each step runs
    // under a lease delegated from the care run's machine-wide lease and polls to a
    // terminal state.
    let dispatch: Arc<dyn aethercore_care_orchestrator::DomainDispatch> =
        Arc::new(RealDomainDispatch {
            engine: engine.clone(),
            cleaner: cleaner.clone(),
            startup: startup.clone(),
        });

    // Phase 22: One-Click Care orchestration. Composes existing domain plans only;
    // single-flight behind MutationWorkload::OneClickCare on the machine-wide supervisor,
    // journaled resume.
    let care = Arc::new(crate::care::CareCoordinator::new(
        db.clone(),
        dispatch,
        kernel.mutations().clone(),
    ));

    // Phase 23.1: the embedded model is the PERMANENT default engine — verified and
    // loaded at every service start; fallback engages ONLY on runtime faults (I3).
    // P76 (DBT-P75-078): loaded in the background. The two paths hold clones of one
    // unresolved model, so the service accepts IPC at once; until the load resolves,
    // a request that needs the model answers loading (engine label `loading`).
    let model = aethercore_intelligence_core::LlamaCppReasoner::new();
    crate::intelligence::load_in_background(model.clone(), product_dir.clone());
    let intelligence_core = Arc::new(crate::intelligence::IntelligenceCoordinator::new(
        db.clone(),
        Some(Box::new(model.clone())),
        diagnostics.clone(),
        repair.clone(),
    ));
    let assistant_reasoner: Option<Box<dyn aethercore_intelligence_core::StreamingReasoner>> =
        Some(Box::new(model));

    // Phase 56/P75: the grounded assistant answers from the SAME evidence pack
    // and a clone of the SAME loaded model the insight path uses. The clones
    // share one generation gate; neither queues behind the other.
    //
    // A failure to load is NOT fatal and is NOT silent: the coordinator's engine
    // label then reads `disabled`, and every turn terminates FAULTED with
    // `assistant.fault.modelUnavailable` rather than with an empty answer.
    let assistant = Arc::new(crate::assistant::AssistantCoordinator::new(
        db.clone(),
        assistant_reasoner,
        diagnostics.clone(),
        repair.clone(),
    ));

    // Recovery is centralized and deliberately observation-only. A failure aborts service startup;
    // no client can enter the operation kernel before every domain has reconciled its journal.
    kernel
        .recovery()
        .run_task("driver-install", || installer.recover_incomplete())
        .context("driver recovery")?;
    kernel
        .recovery()
        .run_task("system-repair", || repair.recover_incomplete())
        .context("repair recovery")?;
    kernel
        .recovery()
        .run_task("cleanup", || cleaner.recover_incomplete())
        .context("cleanup recovery")?;
    kernel
        .recovery()
        .run_task("startup", || startup.recover_incomplete())
        .context("startup recovery")?;

    Ok(ServiceContext {
        kernel,
        engine,
        driver_hub,
        installer,
        repair,
        cleaner,
        startup,
        diagnostics,
        intelligence,
        db,
        updates,
        support,
        performance,
        timeline,
        care,
        intelligence_core,
        assistant,
    })
}

// ---------------------------------------------------------------------------
// Phase 23 / Part A — real domain dispatch for the care coordinator
// ---------------------------------------------------------------------------

struct RealDomainDispatch {
    engine: Arc<OperationEngine>,
    cleaner: Arc<CleanupEngine>,
    startup: Arc<StartupManager>,
}

/// What a domain reports once its plan stops moving.
fn step_result(plan_state: String, verification_state: String) -> (String, String, String) {
    let failure = if plan_state == "Failed" {
        "care.error.domainFailure"
    } else {
        ""
    };
    (plan_state, verification_state, failure.into())
}

fn verified_if(all: bool) -> String {
    if all {
        "Verified".into()
    } else {
        String::new()
    }
}

impl aethercore_care_orchestrator::DomainDispatch for RealDomainDispatch {
    /// Routed by the plan's kind, which care read from the plan itself. A domain's
    /// `status()` cannot route: it answers only for plans that have already started.
    fn start_and_await(
        &self,
        owner: &str,
        plan_id: &str,
        domain_kind: &str,
        approved_digest: &str,
        lease: &aethercore_care_orchestrator::MutationLeaseGuard,
    ) -> Result<(String, String, String), aethercore_care_orchestrator::CareError> {
        let rejected = |detail: String| aethercore_care_orchestrator::CareError::DomainRejected {
            domain_kind: domain_kind.to_string(),
            detail,
        };
        // Review-only kinds (driver install, system repair) never reach dispatch, and are
        // never authorized by care.
        if !matches!(domain_kind, "Cleanup" | "Startup") {
            return Err(rejected(format!(
                "plan {plan_id}: care cannot run {domain_kind:?} plans"
            )));
        }
        // DBT-P75-045: the owner's care approval becomes this plan's one-shot authorization
        // only while the plan is still the approved content; the domain consumes it through
        // its own barrier, and whatever it did not consume is withdrawn on the way out.
        let intent_id = self
            .engine
            .authorize_care_step(plan_id, owner, approved_digest)
            .map_err(|error| match error {
                aethercore_operation_engine::EngineError::DigestMismatch => {
                    aethercore_care_orchestrator::CareError::DigestChanged
                }
                other => rejected(other.to_string()),
            })?;
        let started = self.start_authorized(owner, plan_id, domain_kind, lease);
        self.engine
            .revoke_care_step(&intent_id)
            .map_err(|error| rejected(format!("withdrawing the step authorization: {error}")))?;
        started.map_err(rejected)
    }
}

impl RealDomainDispatch {
    fn start_authorized(
        &self,
        owner: &str,
        plan_id: &str,
        domain_kind: &str,
        lease: &aethercore_care_orchestrator::MutationLeaseGuard,
    ) -> Result<(String, String, String), String> {
        use crate::care::{DISPATCH_TIMEOUT_MS, await_terminal};
        use aethercore_operation_kernel::MutationWorkload;
        let deadline =
            std::time::Instant::now() + std::time::Duration::from_millis(DISPATCH_TIMEOUT_MS);
        let no_status = || format!("plan {plan_id}: no status before the deadline");
        match domain_kind {
            "Cleanup" => {
                let step = lease.delegate(MutationWorkload::Cleanup, plan_id);
                self.cleaner
                    .start_with_lease(owner, plan_id, step)
                    .map_err(|e| e.to_string())?;
                let s = await_terminal(
                    deadline,
                    || self.cleaner.status(owner, Some(plan_id)).ok().flatten(),
                    |s| &s.plan_state,
                )
                .ok_or_else(no_status)?;
                let verified =
                    !s.items.is_empty() && s.items.iter().all(|i| i.result_code == "Deleted");
                Ok(step_result(s.plan_state, verified_if(verified)))
            }
            "Startup" => {
                let step = lease.delegate(MutationWorkload::Startup, plan_id);
                self.startup
                    .start_with_lease(owner, plan_id, step)
                    .map_err(|e| e.to_string())?;
                let s = await_terminal(
                    deadline,
                    || self.startup.status(owner, Some(plan_id)).ok().flatten(),
                    |s| &s.plan_state,
                )
                .ok_or_else(no_status)?;
                let verified =
                    !s.items.is_empty() && s.items.iter().all(|i| i.result_code == "Verified");
                Ok(step_result(s.plan_state, verified_if(verified)))
            }
            other => Err(format!("plan {plan_id}: care cannot run {other:?} plans")),
        }
    }
}

#[cfg(test)]
mod p75_care_dispatch_tests {
    use aethercore_care_orchestrator::{
        CarePlan, CareSafety, CareStep, DomainDispatch, DomainStepExecutor, MutationLeaseGuard,
        StepOutcome,
    };
    use aethercore_cleaner::{
        CleanerError, CleanupCandidate, CleanupMutationLease, CleanupPlatform,
    };
    use aethercore_operation_engine::CleanupDeleteAction;

    use super::*;

    const OWNER: &str = "a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90";

    /// A cleaner platform that touches nothing on the machine: its cleanup always ends
    /// `Failed`, and it holds no machine-wide boundary. These tests used the real Windows
    /// platform, whose cleanup ended `Failed` only because the installer-provisioned lock file
    /// under ProgramData is absent on a machine without the product — and which took the LIVE
    /// lock file, shared with the installed service, on a machine that has it (P76,
    /// DBT-P76-011; the same reasoning as DBT-P61-002 in `cleaner/tests/coordinator.rs`).
    struct FailingCleanupPlatform;

    impl CleanupPlatform for FailingCleanupPlatform {
        fn scan(&self) -> aethercore_cleaner::Result<Vec<CleanupCandidate>> {
            Ok(Vec::new())
        }

        fn delete_action(
            &self,
            _action: &CleanupDeleteAction,
        ) -> aethercore_cleaner::Result<(u64, u64, String)> {
            Err(CleanerError::Safety(
                "injected: this cleanup always fails".into(),
            ))
        }

        fn acquire_mutation_lease(&self) -> aethercore_cleaner::Result<CleanupMutationLease> {
            Ok(Box::new(()))
        }
    }

    struct Via<'a>(&'a RealDomainDispatch, std::sync::Mutex<Option<String>>);

    struct NullJournal;

    impl aethercore_care_orchestrator::CareJournal for NullJournal {
        fn record_run_started(
            &self,
            _: &str,
            _: &str,
            _: &str,
            _: usize,
        ) -> Result<(), aethercore_care_orchestrator::CareError> {
            Ok(())
        }
        fn record_run_consent(
            &self,
            _: &str,
        ) -> Result<(), aethercore_care_orchestrator::CareError> {
            Ok(())
        }
        fn record_step_state(
            &self,
            _: &str,
            _: usize,
            _: &str,
            _: &str,
            _: i64,
            _: &str,
        ) -> Result<(), aethercore_care_orchestrator::CareError> {
            Ok(())
        }
        fn record_step_result(
            &self,
            _: &str,
            _: usize,
            _: &str,
            _: StepOutcome,
            _: &str,
            _: &str,
        ) -> Result<(), aethercore_care_orchestrator::CareError> {
            Ok(())
        }
        fn record_run_finished(
            &self,
            _: &str,
            _: &str,
            _: &str,
        ) -> Result<(), aethercore_care_orchestrator::CareError> {
            Ok(())
        }
    }

    impl DomainStepExecutor for Via<'_> {
        fn execute_step(
            &self,
            owner: &str,
            step: &CareStep,
            lease: &MutationLeaseGuard,
        ) -> Result<(StepOutcome, String, String), aethercore_care_orchestrator::CareError>
        {
            let answer = self.0.start_and_await(
                owner,
                &step.domain_plan_id,
                &step.domain_kind,
                &step.domain_plan_digest,
                lease,
            );
            *self.1.lock().unwrap() = Some(format!("{answer:?}"));
            Ok((StepOutcome::Failed, String::new(), String::new()))
        }
    }

    /// An unstarted plan has no execution record, so no domain's `status()` answers for
    /// it; routing by that made every non-empty care run fail with "no domain
    /// coordinator owns this plan". Routed by kind, the cleaner itself answers.
    #[test]
    fn an_unstarted_cleanup_plan_reaches_the_cleaner() {
        let path = std::env::temp_dir().join(format!(
            "aethercore-p75-dispatch-{}.db",
            uuid::Uuid::new_v4()
        ));
        let db = Arc::new(Database::open(&path).expect("database"));
        let engine = Arc::new(OperationEngine::new(db.clone()));
        let plan_id = engine
            .create_cleanup_plan(
                OWNER,
                1,
                "scan-1",
                vec![CleanupDeleteAction {
                    candidate_id: "c1".into(),
                    scan_id: "scan-1".into(),
                    inventory_epoch: 1,
                    provider: "test".into(),
                    title: "test".into(),
                    special_kind: String::new(),
                    files: Vec::new(),
                    expected_bytes: 0,
                }],
            )
            .expect("plan")
            .id;
        let dispatch = RealDomainDispatch {
            engine: engine.clone(),
            cleaner: Arc::new(CleanupEngine::with_platform(
                engine.clone(),
                db.clone(),
                Arc::new(FailingCleanupPlatform),
            )),
            startup: Arc::new(StartupManager::new(
                engine.clone(),
                db.clone(),
                std::env::temp_dir(),
            )),
        };
        let domain_plan_digest = engine.get_plan_for_owner(&plan_id, OWNER).unwrap().digest;
        let plan = CarePlan::build(vec![CareStep {
            domain_plan_id: plan_id,
            domain_plan_digest,
            domain_kind: "Cleanup".into(),
            safety: CareSafety::Auto,
            title_key: "care.step.cleanup".into(),
        }])
        .unwrap();
        let via = Via(&dispatch, std::sync::Mutex::new(None));
        aethercore_care_orchestrator::run_care_plan(
            &aethercore_operation_kernel::MutationSupervisor::new(),
            &via,
            &NullJournal,
            &aethercore_collector_runtime::CommitFence::new(),
            OWNER,
            "run-1",
            &plan,
            Some(&plan.plan_digest_sha256),
        )
        .expect("run");
        let answer = via.1.lock().unwrap().clone().expect("dispatched");
        // Routed by kind, the cleaner answers for the unstarted plan: it ran it under the care
        // step's authorization (DBT-P75-045) and reports its own terminal state (the injected
        // platform's cleanup fails).
        assert!(answer.starts_with("Ok((\"Failed\""), "{answer}");
        drop(db);
        let _ = std::fs::remove_file(&path);
    }

    fn cleanup_plan(engine: &OperationEngine, candidate: &str) -> String {
        engine
            .create_cleanup_plan(
                OWNER,
                1,
                "scan-1",
                vec![CleanupDeleteAction {
                    candidate_id: candidate.into(),
                    scan_id: "scan-1".into(),
                    inventory_epoch: 1,
                    provider: "test".into(),
                    title: "test".into(),
                    special_kind: String::new(),
                    files: Vec::new(),
                    expected_bytes: 0,
                }],
            )
            .expect("plan")
            .id
    }

    fn real_care(db: &Arc<Database>) -> (crate::care::CareCoordinator, Arc<OperationEngine>) {
        let engine = Arc::new(OperationEngine::new(db.clone()));
        let dispatch = Arc::new(RealDomainDispatch {
            engine: engine.clone(),
            cleaner: Arc::new(CleanupEngine::with_platform(
                engine.clone(),
                db.clone(),
                Arc::new(FailingCleanupPlatform),
            )),
            startup: Arc::new(StartupManager::new(
                engine.clone(),
                db.clone(),
                std::env::temp_dir(),
            )),
        });
        (
            crate::care::CareCoordinator::new(
                db.clone(),
                dispatch,
                aethercore_operation_kernel::MutationSupervisor::new(),
            ),
            engine,
        )
    }

    fn temp_db() -> (Arc<Database>, std::path::PathBuf) {
        let path = std::env::temp_dir().join(format!(
            "aethercore-p75-session-consent-{}.db",
            uuid::Uuid::new_v4()
        ));
        (Arc::new(Database::open(&path).expect("database")), path)
    }

    /// DBT-P75-045: one owner approval of the care plan runs every Auto step through the
    /// domains' own start path. Before, each domain demanded its own broker approval and
    /// answered `authorization required`.
    #[test]
    fn one_approval_runs_every_auto_step() {
        let (db, path) = temp_db();
        let (care, engine) = real_care(&db);
        let first = cleanup_plan(&engine, "c1");
        let second = cleanup_plan(&engine, "c2");
        let shown = care.plan_preview(OWNER).expect("preview");
        let granted = care
            .grant_session_consent(OWNER, &shown.plan_digest_sha256)
            .expect("grant");
        assert_eq!(granted.plan_digest_sha256, shown.plan_digest_sha256);
        let status = care.start_run(OWNER, "run-all").expect("run");
        assert_eq!(status.state, "Completed", "{status:?}");
        for plan in [&first, &second] {
            let state = engine.get_plan_for_owner(plan, OWNER).unwrap().state;
            assert_ne!(
                state,
                aethercore_operation_engine::PlanState::AwaitingAuthorization,
                "{plan} never started: {status:?}"
            );
        }
        drop(db);
        let _ = std::fs::remove_file(&path);
    }

    /// P76 DBT-P76-006 (the owner's Windows install): the Activity timeline stayed empty
    /// after a One-Click Care run that deleted files, even after Refresh. Two causes, both
    /// here: `before_sequence = 0` — the proto's "newest page", and what every client
    /// sends — was sliced as the END index, so the newest page was always empty; and the
    /// care run itself was never ingested, only its domain plan's journal.
    #[test]
    fn a_care_run_and_its_cleanup_are_on_the_newest_timeline_page() {
        let (db, path) = temp_db();
        let (care, engine) = real_care(&db);
        let plan = cleanup_plan(&engine, "c1");
        let shown = care.plan_preview(OWNER).expect("preview");
        care.grant_session_consent(OWNER, &shown.plan_digest_sha256)
            .expect("grant");
        let status = care.start_run(OWNER, "run-timeline").expect("run");
        // The report names the run it is about: a client finds it on the timeline by this id
        // (care_smoke.rs does). It was empty.
        assert_eq!(status.run_id, "run-timeline");
        let (page, _) = crate::timeline::TimelineCoordinator::new(db.clone())
            .page_for_owner(OWNER, 100, 0)
            .expect("page");
        let sources: Vec<&str> = page.entries.iter().map(|e| e.source_id.as_str()).collect();
        assert!(
            sources.contains(&format!("execution:{plan}").as_str()),
            "the cleanup the care run executed is not on the page: {sources:?}"
        );
        let run = page
            .entries
            .iter()
            .find(|e| e.source_id == "care-run:run-timeline")
            .unwrap_or_else(|| panic!("the care run is not on the page: {sources:?}"));
        assert_eq!(run.domain, "oneClickCare");
        assert!(run.code.starts_with("care.run:"), "{}", run.code);
        // The injected platform's cleanup fails, so the run finished with a failed step: both
        // entries must say so. The cleaner writes its result to `stage`
        // and leaves `outcome` empty, which read as a neutral "execution.outcome:".
        let execution = page
            .entries
            .iter()
            .find(|e| e.source_id == format!("execution:{plan}"))
            .unwrap();
        assert_eq!(execution.code, "execution.outcome:Failed");
        assert_eq!(
            execution.outcome,
            aethercore_contracts::v1::TimelineOutcome::Failed as i32
        );
        assert_eq!(
            run.outcome,
            aethercore_contracts::v1::TimelineOutcome::Failed as i32,
            "{run:?}"
        );
        drop(db);
        let _ = std::fs::remove_file(&path);
    }

    /// Review-only work never runs under any consent, and care never authorizes it at the
    /// domain barrier: the driver plan stays where the owner left it.
    #[test]
    fn review_only_work_never_executes_under_care_consent() {
        let (db, path) = temp_db();
        let (care, engine) = real_care(&db);
        let cleanup = cleanup_plan(&engine, "c1");
        db.insert_plan(
            &aethercore_persistence::PlanRecord {
                id: "driver-plan".into(),
                title: "driver".into(),
                state: "AwaitingAuthorization".into(),
                digest: "d".repeat(64),
                risk: "High".into(),
                immutable_json: r#"{"actions":[{"kind":"installWindowsDriver"}]}"#.into(),
                created_unix_ms: 1,
                updated_unix_ms: 1,
                owner_principal_key: OWNER.into(),
            },
            "test",
        )
        .unwrap();
        let shown = care
            .plan_preview(OWNER)
            .expect("preview")
            .plan_digest_sha256;
        let _ = care.grant_session_consent(OWNER, &shown).expect("grant");
        let status = care.start_run(OWNER, "run-review").expect("run");
        let driver = status
            .steps
            .iter()
            .find(|step| step.domain_plan_id == "driver-plan")
            .expect("the review step is listed");
        assert_eq!(driver.state, "Skipped");
        assert_eq!(
            db.get_plan("driver-plan").unwrap().unwrap().state,
            "AwaitingAuthorization"
        );
        assert_ne!(
            engine.get_plan_for_owner(&cleanup, OWNER).unwrap().state,
            aethercore_operation_engine::PlanState::AwaitingAuthorization,
            "the auto step ran: {status:?}"
        );
        drop(db);
        let _ = std::fs::remove_file(&path);
    }
}
