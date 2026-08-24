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
    })
}
