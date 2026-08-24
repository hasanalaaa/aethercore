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
    let updates = UpdateCoordinator::load(
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
    let performance = Arc::new(crate::performance::PerformanceEngine::with_synthetic(
        kernel.mutations().clone(),
    ));

    // Phase 21: Timeline Intelligence. Read-only over persisted history; construction
    // never mutates the journal and every query is principal-scoped + bounded later.
    let timeline = Arc::new(crate::timeline::TimelineCoordinator::new(db.clone()));

    // Phase 22/23 Part A: typed dispatch into the real domain coordinators, shared by
    // the router handlers and the CareCoordinator. Each arm acquires that domain's own
    // mutation lease (so single-flight stays machine-wide) and polls to a terminal state.
    let dispatch: Arc<dyn aethercore_care_orchestrator::DomainDispatch> =
        Arc::new(RealDomainDispatch {
            kernel: kernel.mutations().clone(),
            cleaner: cleaner.clone(),
            startup: startup.clone(),
            repair: repair.clone(),
        });

    // Phase 22: One-Click Care orchestration. Composes existing domain plans only;
    // single-flight behind MutationWorkload::OneClickCare, journaled resume.
    let care = Arc::new(crate::care::CareCoordinator::new(db.clone(), dispatch));

    // Phase 23: Embedded Local Intelligence — advisory-only, air-gapped, ephemeral.
    // Default build ships the deterministic fallback engine; the on-device model path
    // activates only with --features local-model + hash-pinned artifact (I5).
    let intelligence_core = Arc::new(crate::intelligence::IntelligenceCoordinator::new(
        db.clone(),
        None,
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
    })
}

// ---------------------------------------------------------------------------
// Phase 23 / Part A — real domain dispatch shared by router + care coordinator
// ---------------------------------------------------------------------------

struct RealDomainDispatch {
    kernel: aethercore_operation_kernel::MutationSupervisor,
    cleaner: Arc<CleanupEngine>,
    startup: Arc<StartupManager>,
    repair: Arc<RepairCoordinator>,
}

impl aethercore_care_orchestrator::DomainDispatch for RealDomainDispatch {
    fn start_and_await(
        &self,
        owner_principal_key: &str,
        domain_plan_id: &str,
    ) -> Result<(String, String, String), String> {
        use crate::care::{DISPATCH_POLL_MS, DISPATCH_TIMEOUT_MS, is_terminal_plan_state};
        let deadline =
            std::time::Instant::now() + std::time::Duration::from_millis(DISPATCH_TIMEOUT_MS);
        let plan = self
            .cleaner
            .status(owner_principal_key, Some(domain_plan_id))
            .map_err(|e| e.to_string())?
            .or_else(|| {
                self.startup
                    .status(owner_principal_key, Some(domain_plan_id))
                    .ok()
                    .flatten()
            })
            .map(|_| ())
            .is_some();
        let _ = plan; // presence check only; dispatch decided by kind probe below

        // Kind probe order mirrors plan_kind(): cleanup → startup → repair. The first
        // coordinator that OWNS this plan id performs the dispatch.
        if let Ok(Some(status)) = self
            .cleaner
            .status(owner_principal_key, Some(domain_plan_id))
        {
            let lease = self
                .kernel
                .try_acquire(
                    aethercore_operation_kernel::MutationWorkload::Cleanup,
                    domain_plan_id,
                    owner_principal_key,
                )
                .map_err(|e| format!("cleanup lease: {e}"))?;
            self.cleaner
                .start_with_lease(owner_principal_key, domain_plan_id, lease)
                .map_err(|e| e.to_string())?;
            loop {
                if let Ok(Some(s)) = self
                    .cleaner
                    .status(owner_principal_key, Some(domain_plan_id))
                {
                    if is_terminal_plan_state(&s.plan_state)
                        || std::time::Instant::now() >= deadline
                    {
                        let verified = !s.items.is_empty()
                            && s.items.iter().all(|item| item.result_code == "Deleted");
                        let failure = if s.plan_state == "Failed" {
                            "care.error.domainFailure"
                        } else {
                            ""
                        };
                        return Ok((
                            s.plan_state,
                            if verified {
                                "Verified".into()
                            } else {
                                String::new()
                            },
                            failure.into(),
                        ));
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(DISPATCH_POLL_MS));
            }
        }
        if let Ok(Some(status)) = self
            .startup
            .status(owner_principal_key, Some(domain_plan_id))
        {
            let _ = status;
            let lease = self
                .kernel
                .try_acquire(
                    aethercore_operation_kernel::MutationWorkload::Startup,
                    domain_plan_id,
                    owner_principal_key,
                )
                .map_err(|e| format!("startup lease: {e}"))?;
            self.startup
                .start_with_lease(owner_principal_key, domain_plan_id, lease)
                .map_err(|e| e.to_string())?;
            loop {
                if let Ok(Some(s)) = self
                    .startup
                    .status(owner_principal_key, Some(domain_plan_id))
                {
                    if is_terminal_plan_state(&s.plan_state)
                        || std::time::Instant::now() >= deadline
                    {
                        let verified = !s.items.is_empty()
                            && s.items.iter().all(|i| i.result_code == "Verified");
                        let failure = if s.plan_state == "Failed" {
                            "care.error.domainFailure"
                        } else {
                            ""
                        };
                        return Ok((
                            s.plan_state,
                            if verified {
                                "Verified".into()
                            } else {
                                String::new()
                            },
                            failure.into(),
                        ));
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(DISPATCH_POLL_MS));
            }
        }
        if let Ok(Some(status)) = self
            .repair
            .status(owner_principal_key, Some(domain_plan_id))
        {
            let _ = status;
            let lease = self
                .kernel
                .try_acquire(
                    aethercore_operation_kernel::MutationWorkload::SystemRepair,
                    domain_plan_id,
                    owner_principal_key,
                )
                .map_err(|e| format!("system repair lease: {e}"))?;
            self.repair
                .start_with_lease(owner_principal_key, domain_plan_id, lease)
                .map_err(|e| e.to_string())?;
            loop {
                if let Ok(Some(s)) = self
                    .repair
                    .status(owner_principal_key, Some(domain_plan_id))
                {
                    if is_terminal_plan_state(&s.plan_state)
                        || std::time::Instant::now() >= deadline
                    {
                        let failure = if s.plan_state == "Failed" {
                            "care.error.domainFailure"
                        } else {
                            ""
                        };
                        return Ok((s.plan_state, s.verification_state, failure.into()));
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(DISPATCH_POLL_MS));
            }
        }
        Err(format!(
            "plan {domain_plan_id}: no domain coordinator owns this plan"
        ))
    }
}
