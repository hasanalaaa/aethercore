use std::{thread, time::Duration};

use aethercore_contracts::v1::{self, EventKind, event_envelope};
use aethercore_operation_engine::PlanState;
use tracing::warn;

use crate::{protocol::*, router::ServiceContext};

fn spawn_watcher(name: &'static str, worker: impl FnOnce() + Send + 'static) {
    if let Err(error) = thread::Builder::new().name(name.into()).spawn(worker) {
        warn!(watcher = name, error = %error, "stream watcher creation failed; reconnect hydration will recover current state");
    }
}

pub(crate) fn publish(
    ctx: &ServiceContext,
    owner: &str,
    kind: EventKind,
    plan_id: &str,
    payload: Option<event_envelope::Payload>,
) {
    ctx.kernel.events().publish(owner, kind, plan_id, payload);
}
pub(crate) fn publish_latest_plan(ctx: &ServiceContext, owner: &str) {
    if let Ok(Some(plan)) = ctx.engine.latest_plan_for_owner(owner) {
        let p = plan_proto(plan);
        let plan_id = p.id.clone();
        publish(
            ctx,
            owner,
            EventKind::PlanChanged,
            plan_id.as_str(),
            Some(event_envelope::Payload::Plan(p)),
        );
    }
    if let Ok(value) = snapshot(&ctx.engine, owner) {
        publish(
            ctx,
            owner,
            EventKind::ServiceSnapshot,
            "",
            Some(event_envelope::Payload::ServiceSnapshot(value)),
        );
    }
}

/// Emit a complete, principal-scoped current-state image into the ordered event stream.
///
/// This is the synchronization primitive for first connect, replay-window exhaustion, subscriber
/// lag, and service sequence reset. It deliberately uses the same EventBus sequence space as live
/// changes, so the renderer never has to merge an out-of-band polling snapshot with stream state.
pub fn publish_hydration(ctx: &ServiceContext, owner: &str) {
    if let Ok(value) = snapshot(&ctx.engine, owner) {
        publish(
            ctx,
            owner,
            EventKind::ServiceSnapshot,
            "",
            Some(event_envelope::Payload::ServiceSnapshot(value)),
        );
    }
    if let Ok(Some(plan)) = ctx.engine.latest_plan_for_owner(owner) {
        let value = plan_proto(plan);
        let plan_id = value.id.clone();
        publish(
            ctx,
            owner,
            EventKind::PlanChanged,
            plan_id.as_str(),
            Some(event_envelope::Payload::Plan(value)),
        );
    }
    if let Ok(value) = ctx.driver_hub.snapshot_for_owner(owner) {
        publish(
            ctx,
            owner,
            EventKind::DriverDiscovery,
            "",
            Some(event_envelope::Payload::DriverHubSnapshot(
                driver_hub_proto(value),
            )),
        );
    }
    if let Ok(Some(value)) = ctx.installer.status(owner, None) {
        let plan_id = value.plan_id.clone();
        publish(
            ctx,
            owner,
            EventKind::DriverInstall,
            &plan_id,
            Some(event_envelope::Payload::DriverInstallStatus(
                install_status_proto(value),
            )),
        );
    }
    if let Ok(entries) = ctx.installer.recovery_history(owner, 24) {
        publish(
            ctx,
            owner,
            EventKind::RecoveryHistory,
            "",
            Some(event_envelope::Payload::RecoveryHistory(
                v1::RecoveryHistoryResponse {
                    entries: entries.into_iter().map(recovery_proto).collect(),
                },
            )),
        );
    }
    if let Ok(value) = ctx.repair.assessment_for_owner(owner) {
        publish(
            ctx,
            owner,
            EventKind::RepairAssessment,
            "",
            Some(event_envelope::Payload::RepairAssessment(
                repair_assessment_proto(value),
            )),
        );
    }
    if let Ok(Some(value)) = ctx.repair.status(owner, None) {
        let plan_id = value.plan_id.clone();
        publish(
            ctx,
            owner,
            EventKind::SystemRepair,
            &plan_id,
            Some(event_envelope::Payload::SystemRepairStatus(
                repair_status_proto(value),
            )),
        );
    }
    if let Ok(value) = ctx.cleaner.snapshot_for_owner(owner) {
        publish(
            ctx,
            owner,
            EventKind::CleanupDiscovery,
            "",
            Some(event_envelope::Payload::CleanupSnapshot(
                cleanup_snapshot_proto(value),
            )),
        );
    }
    if let Ok(Some(value)) = ctx.cleaner.status(owner, None) {
        let plan_id = value.plan_id.clone();
        publish(
            ctx,
            owner,
            EventKind::CleanupExecution,
            &plan_id,
            Some(event_envelope::Payload::CleanupStatus(
                cleanup_status_proto(value),
            )),
        );
    }
    if let Ok(value) = ctx.startup.snapshot_for_owner(owner) {
        publish(
            ctx,
            owner,
            EventKind::StartupDiscovery,
            "",
            Some(event_envelope::Payload::StartupSnapshot(
                startup_snapshot_proto(value),
            )),
        );
    }
    if let Ok(Some(value)) = ctx.startup.status(owner, None) {
        let plan_id = value.plan_id.clone();
        publish(
            ctx,
            owner,
            EventKind::StartupExecution,
            &plan_id,
            Some(event_envelope::Payload::StartupStatus(
                startup_status_proto(value),
            )),
        );
    }
    if let Ok(entries) = ctx.startup.history(owner, 24) {
        publish(
            ctx,
            owner,
            EventKind::StartupHistory,
            "",
            Some(event_envelope::Payload::StartupHistory(
                v1::StartupHistoryResponse {
                    entries: entries.into_iter().map(startup_history_proto).collect(),
                },
            )),
        );
    }
    if let Ok(value) = ctx.diagnostics.snapshot_for_owner(owner) {
        publish(
            ctx,
            owner,
            EventKind::Diagnostics,
            "",
            Some(event_envelope::Payload::DiagnosticsSnapshot(
                diagnostics_snapshot_proto(value),
            )),
        );
    }
    if let Ok(entries) = ctx.diagnostics.history(owner, 24) {
        publish(
            ctx,
            owner,
            EventKind::DiagnosticsHistory,
            "",
            Some(event_envelope::Payload::DiagnosticsHistory(
                v1::DiagnosticsHistoryResponse {
                    entries: entries.into_iter().map(diagnostic_history_proto).collect(),
                },
            )),
        );
    }
    publish(
        ctx,
        owner,
        EventKind::Update,
        "",
        Some(event_envelope::Payload::UpdateSnapshot(
            update_snapshot_proto(ctx.updates.snapshot(owner)),
        )),
    );
    if let Ok(value) = ctx.intelligence.snapshot_for_owner(owner) {
        if !value.scan_id.is_empty() {
            let _ = ctx.intelligence.record_stream_event(owner, &value.scan_id);
            let value = ctx.intelligence.snapshot_for_owner(owner).unwrap_or(value);
            publish(
                ctx,
                owner,
                EventKind::DeepScan,
                "",
                Some(event_envelope::Payload::DeepScanSnapshot(
                    deep_scan_snapshot_proto(value),
                )),
            );
        }
    }
}

pub(crate) fn watch_deep_scan(ctx: ServiceContext, owner: String) {
    spawn_watcher("aether-watch-deep-scan", move || {
        let mut last = String::new();
        loop {
            thread::sleep(Duration::from_millis(120));
            let Ok(value) = ctx.intelligence.snapshot_for_owner(&owner) else {
                break;
            };
            let terminal = value.state != aethercore_pc_intelligence::ScanState::Scanning;
            let sig = format!(
                "{:?}:{}:{}:{}:{}:{}",
                value.state,
                value.progress.completed_weight,
                value.progress.active_tasks,
                value.facts_count,
                value.findings.len(),
                value.collectors.len()
            );
            if sig != last {
                last = sig;
                let _ = ctx.intelligence.record_stream_event(&owner, &value.scan_id);
                let value = ctx.intelligence.snapshot_for_owner(&owner).unwrap_or(value);
                publish(
                    &ctx,
                    &owner,
                    EventKind::DeepScan,
                    "",
                    Some(event_envelope::Payload::DeepScanSnapshot(
                        deep_scan_snapshot_proto(value),
                    )),
                );
            }
            if terminal {
                break;
            }
        }
    });
}

pub(crate) fn watch_driver_scan(ctx: ServiceContext, owner: String) {
    spawn_watcher("aether-watch-driver-scan", move || {
        let mut last = String::new();
        loop {
            thread::sleep(Duration::from_millis(160));
            let Ok(v) = ctx.driver_hub.snapshot_for_owner(&owner) else {
                break;
            };
            let terminal = !matches!(
                v.state,
                aethercore_driver_hub::ScanState::InventoryScanning
                    | aethercore_driver_hub::ScanState::UpdateSearching
                    | aethercore_driver_hub::ScanState::Matching
            );
            let sig = format!(
                "{}:{}:{}:{}",
                v.state.as_str(),
                v.summary.device_count,
                v.summary.matched_candidate_count,
                v.completed_unix_ms
            );
            if sig != last {
                last = sig;
                publish(
                    &ctx,
                    &owner,
                    EventKind::DriverDiscovery,
                    "",
                    Some(event_envelope::Payload::DriverHubSnapshot(
                        driver_hub_proto(v),
                    )),
                );
            }
            if terminal {
                break;
            }
        }
    });
}

pub(crate) fn watch_repair_assessment(ctx: ServiceContext, owner: String) {
    spawn_watcher("aether-watch-repair-assessment", move || {
        let mut last = String::new();
        loop {
            thread::sleep(Duration::from_millis(160));
            let Ok(v) = ctx.repair.assessment_for_owner(&owner) else {
                break;
            };
            let terminal = !matches!(
                v.state,
                aethercore_system_repair::RepairAssessmentState::Scanning
            );
            let sig = format!("{:?}:{}:{}", v.state, v.checks.len(), v.completed_unix_ms);
            if sig != last {
                last = sig;
                publish(
                    &ctx,
                    &owner,
                    EventKind::RepairAssessment,
                    "",
                    Some(event_envelope::Payload::RepairAssessment(
                        repair_assessment_proto(v),
                    )),
                );
            }
            if terminal {
                break;
            }
        }
    });
}

pub(crate) fn watch_cleanup_scan(ctx: ServiceContext, owner: String) {
    spawn_watcher("aether-watch-cleanup-scan", move || {
        let mut last = String::new();
        loop {
            thread::sleep(Duration::from_millis(160));
            let Ok(v) = ctx.cleaner.snapshot_for_owner(&owner) else {
                break;
            };
            let terminal = !matches!(v.state, aethercore_cleaner::CleanupScanState::Scanning);
            let sig = format!(
                "{:?}:{}:{}",
                v.state,
                v.candidates.len(),
                v.completed_unix_ms
            );
            if sig != last {
                last = sig;
                publish(
                    &ctx,
                    &owner,
                    EventKind::CleanupDiscovery,
                    "",
                    Some(event_envelope::Payload::CleanupSnapshot(
                        cleanup_snapshot_proto(v),
                    )),
                );
            }
            if terminal {
                break;
            }
        }
    });
}

pub(crate) fn watch_startup_scan(ctx: ServiceContext, owner: String) {
    spawn_watcher("aether-watch-startup-scan", move || {
        let mut last = String::new();
        loop {
            thread::sleep(Duration::from_millis(160));
            let Ok(v) = ctx.startup.snapshot_for_owner(&owner) else {
                break;
            };
            let terminal = !matches!(
                v.state,
                aethercore_startup_manager::StartupScanState::Scanning
            );
            let sig = format!("{:?}:{}:{}", v.state, v.items.len(), v.completed_unix_ms);
            if sig != last {
                last = sig;
                publish(
                    &ctx,
                    &owner,
                    EventKind::StartupDiscovery,
                    "",
                    Some(event_envelope::Payload::StartupSnapshot(
                        startup_snapshot_proto(v),
                    )),
                );
            }
            if terminal {
                break;
            }
        }
    });
}

pub(crate) fn watch_diagnostics(ctx: ServiceContext, owner: String) {
    spawn_watcher("aether-watch-diagnostics", move || {
        let mut last = String::new();
        loop {
            thread::sleep(Duration::from_millis(180));
            let Ok(v) = ctx.diagnostics.snapshot_for_owner(&owner) else {
                break;
            };
            let terminal = v.state != aethercore_diagnostic_engine::ScanState::Collecting;
            let sig = format!(
                "{}:{}:{}",
                v.state.as_str(),
                v.cards.len(),
                v.completed_unix_ms
            );
            if sig != last {
                last = sig;
                publish(
                    &ctx,
                    &owner,
                    EventKind::Diagnostics,
                    "",
                    Some(event_envelope::Payload::DiagnosticsSnapshot(
                        diagnostics_snapshot_proto(v),
                    )),
                );
            }
            if terminal {
                if let Ok(entries) = ctx.diagnostics.history(&owner, 12) {
                    publish(
                        &ctx,
                        &owner,
                        EventKind::DiagnosticsHistory,
                        "",
                        Some(event_envelope::Payload::DiagnosticsHistory(
                            v1::DiagnosticsHistoryResponse {
                                entries: entries
                                    .into_iter()
                                    .map(diagnostic_history_proto)
                                    .collect(),
                            },
                        )),
                    );
                }
                break;
            }
        }
    });
}

pub(crate) fn durable_mutation_released(
    ctx: &ServiceContext,
    owner: &str,
    plan_id: &str,
    reboot_releases: bool,
) -> bool {
    match ctx.engine.get_plan_for_owner(plan_id, owner) {
        Ok(plan) => {
            plan.state.is_terminal() || (reboot_releases && plan.state == PlanState::RebootPending)
        }
        Err(_) => false,
    }
}

pub(crate) fn watch_driver_install(ctx: ServiceContext, owner: String, plan_id: String) {
    spawn_watcher("aether-watch-driver-install", move || {
        let mut last = String::new();
        loop {
            thread::sleep(Duration::from_millis(120));
            match ctx.installer.status(&owner, Some(&plan_id)) {
                Ok(Some(v)) => {
                    let terminal = durable_mutation_released(&ctx, &owner, &plan_id, true);
                    // DBT-P46-B16: {:?} rather than {} so the dedup signature
                    // distinguishes None (never determined) from Some(0) — a
                    // transition between the two is a real change to publish.
                    let sig = format!(
                        "{}:{}:{}:{:?}:{}",
                        v.plan_state, v.stage, v.overall_percent, v.bytes_downloaded, v.detail
                    );
                    if sig != last {
                        last = sig;
                        publish(
                            &ctx,
                            &owner,
                            EventKind::DriverInstall,
                            &plan_id,
                            Some(event_envelope::Payload::DriverInstallStatus(
                                install_status_proto(v),
                            )),
                        );
                        publish_latest_plan(&ctx, &owner);
                    }
                    if terminal {
                        if let Ok(entries) = ctx.installer.recovery_history(&owner, 8) {
                            publish(
                                &ctx,
                                &owner,
                                EventKind::RecoveryHistory,
                                &plan_id,
                                Some(event_envelope::Payload::RecoveryHistory(
                                    v1::RecoveryHistoryResponse {
                                        entries: entries.into_iter().map(recovery_proto).collect(),
                                    },
                                )),
                            );
                        }
                        break;
                    }
                }
                Ok(None) | Err(_) if durable_mutation_released(&ctx, &owner, &plan_id, true) => {
                    break;
                }
                Ok(None) | Err(_) => continue,
            }
        }
    });
}

pub(crate) fn watch_repair(ctx: ServiceContext, owner: String, plan_id: String) {
    spawn_watcher("aether-watch-system-repair", move || {
        let mut last = String::new();
        loop {
            thread::sleep(Duration::from_millis(120));
            match ctx.repair.status(&owner, Some(&plan_id)) {
                Ok(Some(v)) => {
                    let terminal = durable_mutation_released(&ctx, &owner, &plan_id, true);
                    let sig = format!(
                        "{}:{}:{}:{}",
                        v.plan_state, v.stage, v.overall_percent, v.detail
                    );
                    if sig != last {
                        last = sig;
                        publish(
                            &ctx,
                            &owner,
                            EventKind::SystemRepair,
                            &plan_id,
                            Some(event_envelope::Payload::SystemRepairStatus(
                                repair_status_proto(v),
                            )),
                        );
                        publish_latest_plan(&ctx, &owner);
                    }
                    if terminal {
                        break;
                    }
                }
                Ok(None) | Err(_) if durable_mutation_released(&ctx, &owner, &plan_id, true) => {
                    break;
                }
                Ok(None) | Err(_) => continue,
            }
        }
    });
}

pub(crate) fn watch_cleanup(ctx: ServiceContext, owner: String, plan_id: String) {
    spawn_watcher("aether-watch-cleanup", move || {
        let mut last = String::new();
        loop {
            thread::sleep(Duration::from_millis(120));
            match ctx.cleaner.status(&owner, Some(&plan_id)) {
                Ok(Some(v)) => {
                    let terminal = durable_mutation_released(&ctx, &owner, &plan_id, false);
                    let sig = format!(
                        "{}:{}:{}:{}",
                        v.plan_state, v.stage, v.overall_percent, v.detail
                    );
                    if sig != last {
                        last = sig;
                        publish(
                            &ctx,
                            &owner,
                            EventKind::CleanupExecution,
                            &plan_id,
                            Some(event_envelope::Payload::CleanupStatus(
                                cleanup_status_proto(v),
                            )),
                        );
                        publish_latest_plan(&ctx, &owner);
                    }
                    if terminal {
                        break;
                    }
                }
                Ok(None) | Err(_) if durable_mutation_released(&ctx, &owner, &plan_id, false) => {
                    break;
                }
                Ok(None) | Err(_) => continue,
            }
        }
    });
}

pub(crate) fn watch_startup(ctx: ServiceContext, owner: String, plan_id: String) {
    spawn_watcher("aether-watch-startup", move || {
        let mut last = String::new();
        loop {
            thread::sleep(Duration::from_millis(120));
            match ctx.startup.status(&owner, Some(&plan_id)) {
                Ok(Some(v)) => {
                    let terminal = durable_mutation_released(&ctx, &owner, &plan_id, false);
                    let sig = format!(
                        "{}:{}:{}:{}",
                        v.plan_state, v.stage, v.overall_percent, v.detail
                    );
                    if sig != last {
                        last = sig;
                        publish(
                            &ctx,
                            &owner,
                            EventKind::StartupExecution,
                            &plan_id,
                            Some(event_envelope::Payload::StartupStatus(
                                startup_status_proto(v),
                            )),
                        );
                        publish_latest_plan(&ctx, &owner);
                    }
                    if terminal {
                        if let Ok(entries) = ctx.startup.history(&owner, 20) {
                            publish(
                                &ctx,
                                &owner,
                                EventKind::StartupHistory,
                                &plan_id,
                                Some(event_envelope::Payload::StartupHistory(
                                    v1::StartupHistoryResponse {
                                        entries: entries
                                            .into_iter()
                                            .map(startup_history_proto)
                                            .collect(),
                                    },
                                )),
                            );
                        }
                        break;
                    }
                }
                Ok(None) | Err(_) if durable_mutation_released(&ctx, &owner, &plan_id, false) => {
                    break;
                }
                Ok(None) | Err(_) => continue,
            }
        }
    });
}
