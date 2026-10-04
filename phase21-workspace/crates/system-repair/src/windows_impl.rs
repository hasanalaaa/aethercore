use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

use super::{
    AssessStep, RepairCheck, RepairControl, RepairError, RepairPlatform, Result,
    bounded::{ProviderSlot, run_check},
    cbs,
    dism_api::{
        check_online_image_health, check_online_image_health_cancellable,
        restore_online_image_health,
    },
};
use aethercore_operation_engine::SystemRepairAction;
use aethercore_windows_foundation::{MachineMutationGuard, OwnedServiceHandle};
use windows::{
    Win32::System::Services::{
        OpenSCManagerW, OpenServiceW, QUERY_SERVICE_CONFIGW, QueryServiceConfigW,
        QueryServiceStatusEx, SC_MANAGER_CONNECT, SC_STATUS_PROCESS_INFO, SERVICE_QUERY_CONFIG,
        SERVICE_QUERY_STATUS, SERVICE_RUNNING, SERVICE_START, SERVICE_STATUS_PROCESS,
        StartServiceW,
    },
    core::PCWSTR,
};

const COMMAND_TIMEOUT: Duration = Duration::from_secs(2 * 60 * 60);

// P78-02: each assessment check has its own deadline, so one that never returns reads as unknown
// instead of leaving the screen on "Assessing". These are first bounds, not measurements from a
// real machine: a slower real scan reads as unknown and can be run again.
const DISM_SCAN_DEADLINE: Duration = Duration::from_secs(20 * 60);
const SFC_VERIFY_DEADLINE: Duration = Duration::from_secs(20 * 60);
const DISK_SCAN_DEADLINE: Duration = Duration::from_secs(20 * 60);
const UPDATE_PROBE_DEADLINE: Duration = Duration::from_secs(2 * 60);
/// The local history read: a bounded query of the agent's own store, first bound not a measurement.
const UPDATE_HISTORY_DEADLINE: Duration = Duration::from_secs(2 * 60);
/// The newest history entries read (two pages of 100 at most).
const UPDATE_HISTORY_ENTRIES: usize = 200;
static DISM_SLOT: ProviderSlot = ProviderSlot::new();
static SFC_SLOT: ProviderSlot = ProviderSlot::new();
static DISK_SLOT: ProviderSlot = ProviderSlot::new();
static UPDATE_SLOT: ProviderSlot = ProviderSlot::new();
static UPDATE_HISTORY_SLOT: ProviderSlot = ProviderSlot::new();
static UPDATE_CLIENT_SLOT: ProviderSlot = ProviderSlot::new();
static UPDATE_SERVICES_SLOT: ProviderSlot = ProviderSlot::new();
static WINRE_SLOT: ProviderSlot = ProviderSlot::new();

pub struct WindowsRepairPlatform;

/// RAII holder for the machine-mutation lock: the field is never read by design,
/// the lock is released when it drops. A tuple field cannot be underscore-named,
/// so this mirrors `windows-update`'s `MachineMutationLease` and names it `_guard`.
struct ServicingGuard {
    _guard: MachineMutationGuard,
}

impl WindowsRepairPlatform {
    fn assess_impl(
        &self,
        cancel: &std::sync::Arc<AtomicBool>,
        progress: &mut dyn FnMut(AssessStep<'_>),
        lease: Option<std::sync::Arc<aethercore_operation_kernel::ReadBudgetLease>>,
    ) -> Result<(String, Vec<RepairCheck>)> {
        let root = system_root()?;
        let volume = system_volume()?;
        let system32 = root.join("System32");

        // A failing collector becomes Unknown evidence for its own domain. It does not make
        // Windows globally broken and it does not fabricate a repair candidate.
        // DISM ScanHealth and SFC /verifyonly each take minutes on a real machine, run one
        // after the other, and are read-only: each is reported as it starts and finishes,
        // and a cancel stops the running one (P76, DBT-P76-007).
        let mut checks = Vec::new();
        let mut step = |checks: &mut Vec<RepairCheck>,
                        id: &str,
                        run: &mut dyn FnMut() -> Result<RepairCheck>| {
            if cancel.load(Ordering::SeqCst) {
                return Err(RepairError::Cancelled);
            }
            progress(AssessStep::Started(id));
            let check = run()?;
            if cancel.load(Ordering::SeqCst) {
                return Err(RepairError::Cancelled);
            }
            progress(AssessStep::Finished(&check));
            checks.push(check);
            Ok(())
        };
        step(&mut checks, "dism-scan", &mut || {
            const LOG: &str = "%WINDIR%\\Logs\\DISM\\dism.log";
            let lease_for_worker = lease.clone();
            run_check(
                &DISM_SLOT,
                ("dism-scan", "Component store", LOG),
                DISM_SCAN_DEADLINE,
                cancel,
                move |stop| {
                    let _read_budget_lease = lease_for_worker;
                    probe_or_unknown("dism-scan", "Component store", LOG, || {
                        check_online_image_health_cancellable(
                            true,
                            "dism-scan",
                            "Component store",
                            &stop,
                        )
                    })
                },
            )
        })?;
        step(&mut checks, "sfc-verify", &mut || {
            const LOG: &str = "%WINDIR%\\Logs\\CBS\\CBS.log";
            let (sfc, root) = (system32.join("sfc.exe"), root.clone());
            let lease_for_worker = lease.clone();
            run_check(
                &SFC_SLOT,
                ("sfc-verify", "Protected system files", LOG),
                SFC_VERIFY_DEADLINE,
                cancel,
                move |stop| {
                    let _read_budget_lease = lease_for_worker;
                    probe_or_unknown("sfc-verify", "Protected system files", LOG, || {
                        run_sfc(
                            &sfc,
                            &["/verifyonly"],
                            "sfc-verify",
                            "Protected system files",
                            &root,
                            Some(&*stop),
                        )
                    })
                },
            )
        })?;
        step(&mut checks, "servicing-state", &mut || {
            Ok(servicing_state_check())
        })?;
        step(&mut checks, "update-health", &mut || {
            let lease_for_worker = lease.clone();
            run_check(
                &UPDATE_SLOT,
                ("windows-update", "Windows Update", "Windows Update Agent"),
                UPDATE_PROBE_DEADLINE,
                cancel,
                move |_| {
                    let _read_budget_lease = lease_for_worker;
                    update_health_check()
                },
            )
        })?;
        let update_failed = checks
            .last()
            .is_some_and(|check| check.result_code == "UpdateFailure");
        for (step_id, id, name, title) in [
            (
                if update_failed {
                    "required-update-service"
                } else {
                    "update-agent-service"
                },
                if update_failed {
                    "required-service"
                } else {
                    "update-agent-service"
                },
                "wuauserv",
                "Windows Update service",
            ),
            (
                "update-bits-service",
                "update-bits-service",
                "BITS",
                "Background Intelligent Transfer Service",
            ),
            (
                "update-servicing-service",
                "update-servicing-service",
                "TrustedInstaller",
                "Windows Modules Installer service",
            ),
        ] {
            let lease_for_worker = lease.clone();
            step(&mut checks, step_id, &mut || {
                let lease_for_worker = lease_for_worker.clone();
                run_check(
                    &UPDATE_SERVICES_SLOT,
                    (id, title, name),
                    Duration::from_secs(5),
                    cancel,
                    move |_| {
                        let _read_budget_lease = lease_for_worker;
                        if id == "required-service" {
                            required_update_service_check()
                        } else {
                            update_dependency_service_check(id, name, title)
                        }
                    },
                )
            })?;
        }
        // Evidence only: the id has no diagnosis fact, so this proposes no repair (P83-03A).
        step(&mut checks, "update-history", &mut || {
            let lease_for_worker = lease.clone();
            run_check(
                &UPDATE_HISTORY_SLOT,
                (
                    "windows-update-history",
                    "Windows Update history",
                    "Windows Update Agent",
                ),
                UPDATE_HISTORY_DEADLINE,
                cancel,
                move |_| {
                    let _read_budget_lease = lease_for_worker;
                    crate::update_history_check(
                        aethercore_windows_update::query_update_history(UPDATE_HISTORY_ENTRIES)
                            .ok()
                            .as_ref(),
                    )
                },
            )
        })?;
        step(&mut checks, "update-client-events", &mut || {
            let lease_for_worker = lease.clone();
            run_check(
                &UPDATE_CLIENT_SLOT,
                (
                    "windows-update-client-events",
                    "Windows Update client events",
                    "Microsoft-Windows-WindowsUpdateClient/Operational",
                ),
                UPDATE_HISTORY_DEADLINE,
                cancel,
                move |_| {
                    let _read_budget_lease = lease_for_worker;
                    crate::update_client_check(
                        aethercore_windows_update::query_client_errors(200)
                            .ok()
                            .as_ref(),
                    )
                },
            )
        })?;
        step(&mut checks, "disk-scan", &mut || {
            const LOG: &str = "Event Viewer → Application → Chkdsk";
            let (chkdsk, volume) = (system32.join("chkdsk.exe"), volume.clone());
            let lease_for_worker = lease.clone();
            run_check(
                &DISK_SLOT,
                ("disk-scan", "System volume online scan", LOG),
                DISK_SCAN_DEADLINE,
                cancel,
                move |stop| {
                    let _read_budget_lease = lease_for_worker;
                    probe_or_unknown("disk-scan", "System volume online scan", LOG, || {
                        run_chkdsk_scan(
                            &chkdsk,
                            &volume,
                            "disk-scan",
                            "System volume online scan",
                            Some(&*stop),
                        )
                    })
                },
            )
        })?;
        step(&mut checks, "winre-presence", &mut || {
            let system32 = system32.clone();
            let lease_for_worker = lease.clone();
            run_check(
                &WINRE_SLOT,
                (
                    "winre-state",
                    "Windows Recovery Environment",
                    "reagentc.exe /info",
                ),
                UPDATE_PROBE_DEADLINE,
                cancel,
                move |stop| {
                    let _read_budget_lease = lease_for_worker;
                    probe_or_unknown(
                        "winre-state",
                        "Windows Recovery Environment",
                        "reagentc.exe /info",
                        || winre_configuration_check(&system32, &stop),
                    )
                },
            )
        })?;
        step(&mut checks, "restore-readiness", &mut || {
            Ok(restore_readiness_check(&root))
        })?;

        Ok((volume, checks))
    }
}

impl RepairPlatform for WindowsRepairPlatform {
    fn assess(
        &self,
        cancel: &std::sync::Arc<AtomicBool>,
        progress: &mut dyn FnMut(AssessStep<'_>),
    ) -> Result<(String, Vec<RepairCheck>)> {
        self.assess_impl(cancel, progress, None)
    }

    fn assess_with_lease(
        &self,
        cancel: &std::sync::Arc<AtomicBool>,
        progress: &mut dyn FnMut(AssessStep<'_>),
        lease: std::sync::Arc<aethercore_operation_kernel::ReadBudgetLease>,
    ) -> Result<(String, Vec<RepairCheck>)> {
        self.assess_impl(cancel, progress, Some(lease))
    }

    fn repair(
        &self,
        action: &SystemRepairAction,
        control: &mut RepairControl<'_>,
        begin_mutation: &mut dyn FnMut() -> Result<()>,
        emit: &mut dyn FnMut(RepairCheck),
    ) -> Result<()> {
        let _guard = acquire_servicing_guard()?;
        if action.run_component_store || action.run_system_files {
            match aethercore_windows_update::ensure_servicing_available() {
                Ok(()) => emit(servicing_available_check()),
                Err(aethercore_windows_update::UpdateError::Busy) => {
                    return Err(RepairError::ServicingBusy);
                }
                Err(aethercore_windows_update::UpdateError::RebootPending) => {
                    return Err(RepairError::RebootPending);
                }
                Err(error) => {
                    return Err(RepairError::Command(format!(
                        "Windows Update servicing preflight failed: {error}"
                    )));
                }
            }
        }

        let root = system_root()?;
        let system32 = root.join("System32");

        // Revalidate the component-store state using the DISM API immediately before mutation.
        // A now-healthy image invalidates the old repair assumption rather than replaying RestoreHealth.
        if action.run_component_store {
            let check =
                check_online_image_health(false, "dism-preflight", "Component store preflight")?;
            match check.result_code.as_str() {
                "ComponentStoreRepairable" => emit(check),
                "ComponentStoreHealthy" => return Err(RepairError::StaleAssessment),
                "ComponentStoreNonRepairable" => return Err(RepairError::RecoveryUnavailable("DISM API reports the online image as non-repairable by the selected servicing path".into())),
                _ => return Err(RepairError::Command("component-store health could not be established before mutation".into())),
            }
        }

        let start_required_service = action
            .repair_action_ids
            .iter()
            .any(|id| id == "start-required-service");
        if action.run_component_store || action.run_system_files || start_required_service {
            begin_mutation()?;
        }

        if control.cancel.load(Ordering::SeqCst) {
            return Err(RepairError::RepairStopped);
        }
        if start_required_service {
            emit(start_update_service()?);
        }
        if control.cancel.load(Ordering::SeqCst) {
            return Err(RepairError::RepairStopped);
        }
        if action.run_component_store {
            emit(restore_online_image_health(control)?);
        }
        if control.cancel.load(Ordering::SeqCst) {
            return Err(RepairError::RepairStopped);
        }
        if action.run_system_files {
            (control.progress)(None);
            emit(run_sfc(
                &system32.join("sfc.exe"),
                &["/scannow"],
                "sfc-repair",
                "System File Checker repair",
                &root,
                Some(control.cancel),
            )?);
        }
        if control.cancel.load(Ordering::SeqCst) {
            return Err(RepairError::RepairStopped);
        }
        if action.run_disk_scan {
            let volume = system_volume()?;
            emit(run_chkdsk_scan(
                &system32.join("chkdsk.exe"),
                &volume,
                "disk-scan",
                "System volume online scan",
                None,
            )?);
        }
        Ok(())
    }

    fn verify(
        &self,
        action: &SystemRepairAction,
        control: &mut RepairControl<'_>,
        emit: &mut dyn FnMut(RepairCheck),
    ) -> Result<()> {
        let checkpoint = || {
            if control.cancel.load(Ordering::SeqCst) {
                Err(RepairError::RepairStopped)
            } else {
                Ok(())
            }
        };
        let stopped = |error| match error {
            RepairError::Cancelled => RepairError::RepairStopped,
            other => other,
        };
        checkpoint()?;
        let root = system_root()?;
        let system32 = root.join("System32");
        if action.run_component_store {
            emit(
                check_online_image_health_cancellable(
                    true,
                    "verify-dism",
                    "Verify component store",
                    control.cancel,
                )
                .map_err(stopped)?,
            );
        }
        checkpoint()?;
        if action.run_system_files {
            emit(
                run_sfc(
                    &system32.join("sfc.exe"),
                    &["/verifyonly"],
                    "verify-sfc",
                    "Verify protected system files",
                    &root,
                    Some(control.cancel),
                )
                .map_err(stopped)?,
            );
        }
        checkpoint()?;
        if action.run_disk_scan {
            let volume = system_volume()?;
            emit(
                run_chkdsk_scan(
                    &system32.join("chkdsk.exe"),
                    &volume,
                    "verify-disk",
                    "Verify system volume",
                    Some(control.cancel),
                )
                .map_err(stopped)?,
            );
        }
        checkpoint()?;
        if action
            .repair_action_ids
            .iter()
            .any(|id| id == "start-required-service")
        {
            let mut check = required_update_service_check();
            check.id = "verify-required-service".into();
            check.title = "Verify required Windows Update service".into();
            emit(check);
            checkpoint()?;
            let mut update = update_health_check();
            update.id = "verify-windows-update".into();
            update.title = "Verify Windows Update discovery".into();
            emit(update);
        }
        checkpoint()
    }
}

fn acquire_servicing_guard() -> Result<ServicingGuard> {
    let guard = MachineMutationGuard::try_acquire()
        .map_err(|error| RepairError::Command(error.to_string()))?
        .ok_or(RepairError::ServicingBusy)?;
    Ok(ServicingGuard { _guard: guard })
}

fn update_health_check() -> RepairCheck {
    let probe = aethercore_windows_update::probe_update_health();
    RepairCheck {
        id: "windows-update".into(),
        title: "Windows Update".into(),
        stage: if probe.result_code == "UpdateHealthy" {
            "Completed"
        } else if probe.result_code == "UpdateOffline" {
            "Unknown"
        } else {
            "Attention"
        }
        .into(),
        result_code: probe.result_code,
        exit_code: probe.hresult,
        detail: probe.detail,
        log_hint: "Windows Update Agent".into(),
    }
}

fn service_running(name: &str) -> Result<bool> {
    Ok(service_state(name)? == SERVICE_RUNNING)
}

fn service_state(
    name: &str,
) -> Result<windows::Win32::System::Services::SERVICE_STATUS_CURRENT_STATE> {
    unsafe {
        let scm = OwnedServiceHandle::new(
            OpenSCManagerW(PCWSTR::null(), PCWSTR::null(), SC_MANAGER_CONNECT)
                .map_err(|e| RepairError::Command(format!("OpenSCManagerW: {e}")))?,
        );
        let wide = name
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        let service = OwnedServiceHandle::new(
            OpenServiceW(scm.get(), PCWSTR(wide.as_ptr()), SERVICE_QUERY_STATUS)
                .map_err(|e| RepairError::Command(format!("OpenServiceW({name}): {e}")))?,
        );
        let mut status = SERVICE_STATUS_PROCESS::default();
        let mut needed = 0u32;
        let bytes = std::slice::from_raw_parts_mut(
            (&mut status as *mut SERVICE_STATUS_PROCESS).cast::<u8>(),
            std::mem::size_of::<SERVICE_STATUS_PROCESS>(),
        );
        QueryServiceStatusEx(
            service.get(),
            SC_STATUS_PROCESS_INFO,
            Some(bytes),
            &mut needed,
        )
        .map_err(|e| RepairError::Command(format!("QueryServiceStatusEx({name}): {e}")))?;
        Ok(status.dwCurrentState)
    }
}

fn service_start_type(name: &str) -> Result<u32> {
    unsafe {
        let scm = OwnedServiceHandle::new(
            OpenSCManagerW(PCWSTR::null(), PCWSTR::null(), SC_MANAGER_CONNECT)
                .map_err(|e| RepairError::Command(format!("OpenSCManagerW: {e}")))?,
        );
        let wide = name.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
        let service = OwnedServiceHandle::new(
            OpenServiceW(scm.get(), PCWSTR(wide.as_ptr()), SERVICE_QUERY_CONFIG)
                .map_err(|e| RepairError::Command(format!("OpenServiceW({name}): {e}")))?,
        );
        // QueryServiceConfigW's documented maximum is 8 KiB. usize storage preserves ABI alignment.
        let mut buffer = [0usize; 8192 / std::mem::size_of::<usize>()];
        let config = buffer.as_mut_ptr().cast::<QUERY_SERVICE_CONFIGW>();
        let mut needed = 0;
        QueryServiceConfigW(service.get(), Some(config), 8192, &mut needed)
            .map_err(|e| RepairError::Command(format!("QueryServiceConfigW({name}): {e}")))?;
        Ok((*config).dwStartType.0)
    }
}

/// Read-only dependency state: this id never maps to a repair fact or starts a service.
fn update_dependency_service_check(id: &str, name: &str, title: &str) -> RepairCheck {
    let observed = service_state(name).and_then(|state| {
        service_start_type(name).map(|start| super::service_start_verdict(state.0, start))
    });
    let (code, stage, detail) = match observed {
        Ok("ServiceRunning") => (
            "ServiceRunning",
            "Completed",
            "The update dependency service is running; this alone does not establish update health.",
        ),
        Ok("ServiceDemandStopped") => (
            "ServiceDemandStopped",
            "Completed",
            "The update dependency service is stopped in demand-start mode. This alone does not require repair.",
        ),
        Ok("ServiceDisabled") => (
            "ServiceDisabled",
            "Attention",
            "The update dependency service is disabled. Its configuration may be managed by policy; AetherCore will not change it.",
        ),
        Ok("ServiceStopped") => (
            "ServiceStopped",
            "Attention",
            "The update dependency service is stopped in automatic-start mode. This evidence alone does not authorize a service change.",
        ),
        _ => (
            "ServiceUnknown",
            "Unknown",
            "The update dependency service state or configuration could not be established. No service change is recommended.",
        ),
    };
    RepairCheck {
        id: id.into(),
        title: title.into(),
        stage: stage.into(),
        result_code: code.into(),
        exit_code: 0,
        detail: detail.into(),
        log_hint: name.into(),
    }
}

fn required_update_service_check() -> RepairCheck {
    let result = service_state("wuauserv").and_then(|state| {
        service_start_type("wuauserv").map(|start| super::service_start_verdict(state.0, start))
    });
    let (code, stage, detail) = match result {
        Ok("ServiceRunning") => (
            "ServiceRunning",
            "Completed",
            "The diagnosis-scoped Windows Update service is running.",
        ),
        Ok("ServiceDemandStopped") => (
            "ServiceDemandStopped",
            "Completed",
            "The Windows Update service is stopped in demand-start mode. This alone does not require repair.",
        ),
        Ok("ServiceDisabled") => (
            "ServiceDisabled",
            "Attention",
            "The Windows Update service is disabled. Its configuration may be managed by policy; AetherCore will not change it.",
        ),
        Ok("ServiceStopped") => (
            "ServiceStopped",
            "Attention",
            "Windows Update discovery failed and its automatic-start service is stopped. Only this diagnosis-scoped start may be reviewed.",
        ),
        _ => (
            "ServiceUnknown",
            "Unknown",
            "The Windows Update service configuration could not be established. No service change is recommended.",
        ),
    };
    RepairCheck {
        id: "required-service".into(),
        title: "Windows Update service".into(),
        stage: stage.into(),
        result_code: code.into(),
        exit_code: 0,
        detail: detail.into(),
        log_hint: "wuauserv".into(),
    }
}

fn start_update_service() -> Result<RepairCheck> {
    if required_update_service_check().result_code != "ServiceStopped" {
        return Err(RepairError::StaleAssessment);
    }
    unsafe {
        let scm = OwnedServiceHandle::new(
            OpenSCManagerW(PCWSTR::null(), PCWSTR::null(), SC_MANAGER_CONNECT)
                .map_err(|e| RepairError::Command(format!("OpenSCManagerW: {e}")))?,
        );
        let wide = "wuauserv"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        let service = OwnedServiceHandle::new(
            OpenServiceW(
                scm.get(),
                PCWSTR(wide.as_ptr()),
                SERVICE_START | SERVICE_QUERY_STATUS,
            )
            .map_err(|e| RepairError::Command(format!("OpenServiceW(wuauserv): {e}")))?,
        );
        if !service_running("wuauserv")? {
            StartServiceW(service.get(), None)
                .map_err(|e| RepairError::Command(format!("StartServiceW(wuauserv): {e}")))?;
        }
    }
    Ok(RepairCheck { id:"start-required-service".into(), title:"Start required Windows Update service".into(), stage:"Completed".into(), result_code:"MutationSucceeded".into(), exit_code:0, detail:"AetherCore requested the fixed diagnosis-scoped wuauserv service to start. Verification will query the service state separately.".into(), log_hint:"wuauserv".into() })
}

fn servicing_state_check() -> RepairCheck {
    match aethercore_windows_update::ensure_servicing_available() {
        Ok(()) => servicing_available_check(),
        Err(aethercore_windows_update::UpdateError::Busy) => RepairCheck {
            id: "servicing-state".into(), title: "Windows servicing state".into(), stage: "Attention".into(),
            result_code: "ServicingBusy".into(), exit_code: 0,
            detail: "Windows reports that another servicing installation is active. AetherCore will not compete with it.".into(),
            log_hint: String::new(),
        },
        Err(aethercore_windows_update::UpdateError::RebootPending) => RepairCheck {
            id: "servicing-state".into(), title: "Windows servicing state".into(), stage: "Attention".into(),
            result_code: "RebootPending".into(), exit_code: 0,
            detail: "Windows Update Agent reports that a restart is required before another installation can safely begin.".into(),
            log_hint: String::new(),
        },
        Err(error) => RepairCheck {
            id: "servicing-state".into(), title: "Windows servicing state".into(), stage: "Unknown".into(),
            result_code: "ServicingUnknown".into(), exit_code: -1,
            detail: sanitize(&format!("Servicing state could not be queried: {error}")), log_hint: String::new(),
        },
    }
}

fn servicing_available_check() -> RepairCheck {
    RepairCheck {
        id: "servicing-state".into(),
        title: "Windows servicing state".into(),
        stage: "Completed".into(),
        result_code: "ServicingAvailable".into(),
        exit_code: 0,
        detail:
            "Windows servicing is not reporting an active installer or required pre-install reboot."
                .into(),
        log_hint: String::new(),
    }
}

fn probe_or_unknown<F>(id: &str, title: &str, log_hint: &str, probe: F) -> RepairCheck
where
    F: FnOnce() -> Result<RepairCheck>,
{
    match probe() {
        Ok(value) => value,
        Err(error) => RepairCheck {
            id: id.into(),
            title: title.into(),
            stage: "Unknown".into(),
            result_code: "ProbeUnavailable".into(),
            exit_code: -1,
            detail: sanitize(&format!("This check could not be completed: {error}")),
            log_hint: log_hint.into(),
        },
    }
}

fn system_root() -> Result<PathBuf> {
    let path = std::env::var_os("SystemRoot")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
    if !path.is_absolute() {
        return Err(RepairError::Command("SystemRoot is not absolute".into()));
    }
    Ok(path)
}

fn system_volume() -> Result<String> {
    let raw = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
    let bytes = raw.as_bytes();
    if bytes.len() != 2 || !bytes[0].is_ascii_alphabetic() || bytes[1] != b':' {
        return Err(RepairError::Command("invalid SystemDrive".into()));
    }
    Ok(raw.to_ascii_uppercase())
}

fn run_sfc(
    exe: &Path,
    args: &[&str],
    id: &str,
    title: &str,
    root: &Path,
    cancel: Option<&AtomicBool>,
) -> Result<RepairCheck> {
    // P85-01: the verdict comes from what THIS run wrote to the CBS log, found from a baseline taken
    // before it started, not from a tail that holds older runs (see `cbs`).
    let cbs_log = root.join("Logs").join("CBS").join("CBS.log");
    let baseline = cbs::CbsBaseline::capture(&cbs_log);
    let mut check = super::process::run_tool(
        exe,
        args,
        (id, title, "%WINDIR%\\Logs\\CBS\\CBS.log"),
        &[0, 1, 2],
        cancel,
        args.iter().any(|arg| arg.eq_ignore_ascii_case("/scannow")),
        COMMAND_TIMEOUT,
    )?;

    // Do not infer integrity state from localized console prose. SFC's console output is retained
    // only as bounded diagnostic context; a run the log cannot be attributed to stays Unknown and
    // cannot resolve a Finding.
    let evidence = match cbs::window(&cbs_log, baseline.as_ref()) {
        cbs::Window::Text(text) => cbs::classify(&text, check.exit_code),
        cbs::Window::Unknown => cbs::CbsEvidence::Unknown,
    };
    let repair_mode = args.iter().any(|arg| arg.eq_ignore_ascii_case("/scannow"));
    match evidence {
        cbs::CbsEvidence::NoViolation => {
            check.stage = "Completed".into();
            check.result_code = "SystemFilesHealthy".into();
            check.detail = "Recent CBS [SR] evidence contains no unresolved protected-file integrity violation for this check window.".into();
        }
        cbs::CbsEvidence::ViolationRepaired if repair_mode => {
            check.stage = "Completed".into();
            check.result_code = "SystemFilesRepairMutationSucceeded".into();
            check.detail = "CBS [SR] evidence records protected-file repair activity. A separate verify-only pass is still required.".into();
        }
        cbs::CbsEvidence::ViolationRepaired => {
            check.stage = "Attention".into();
            check.result_code = "SystemFilesCorrupt".into();
            check.detail = "CBS [SR] evidence records protected-file repair activity; verify-only did not provide enough evidence to mark the state healthy.".into();
        }
        cbs::CbsEvidence::ViolationUnresolved => {
            check.stage = "Attention".into();
            check.result_code = "SystemFilesCorrupt".into();
            check.detail = "Recent CBS [SR] evidence indicates unresolved protected-file integrity work is required.".into();
        }
        cbs::CbsEvidence::Unknown => {
            check.stage = "Unknown".into();
            check.result_code = if repair_mode {
                "SystemFilesRepairUnverified"
            } else {
                "SystemFilesUnknown"
            }
            .into();
            check.detail = "SFC completed, but stable machine-readable evidence was insufficient to prove protected-file health. Console-language parsing is intentionally not used.".into();
        }
    }
    Ok(check)
}

fn run_chkdsk_scan(
    exe: &Path,
    volume: &str,
    id: &str,
    title: &str,
    cancel: Option<&AtomicBool>,
) -> Result<RepairCheck> {
    let mut check = super::process::run_tool(
        exe,
        &[volume, "/scan"],
        (id, title, "Event Viewer → Application → Chkdsk"),
        &[0, 1, 2, 3],
        cancel,
        false,
        COMMAND_TIMEOUT,
    )?;
    if check.exit_code == 0 {
        check.stage = "Completed".into();
        check.result_code = "NoErrors".into();
        check.detail = "CHKDSK online scan completed without an exit-status indication that offline repair is required.".into();
    } else {
        check.stage = "Attention".into();
        check.result_code = format!("ChkdskExit{}", check.exit_code);
        check.detail = format!(
            "CHKDSK /scan reported exit code {}. AetherCore does not infer physical-disk failure from this filesystem result and does not run /f, /r, /spotfix, format, partition or raw-disk repair automatically.",
            check.exit_code
        );
    }
    Ok(check)
}

fn winre_configuration_check(system32: &Path, cancel: &AtomicBool) -> Result<RepairCheck> {
    if cancel.load(Ordering::SeqCst) {
        return Err(RepairError::Cancelled);
    }
    let check = super::process::run_tool(
        &system32.join("reagentc.exe"),
        &["/info"],
        (
            "winre-state",
            "Windows Recovery Environment",
            "reagentc.exe /info",
        ),
        &[0],
        Some(cancel),
        false,
        UPDATE_PROBE_DEADLINE,
    )?;
    Ok(super::winre::from_info(&check.detail, check.exit_code))
}

fn restore_readiness_check(root: &Path) -> RepairCheck {
    let sr_dir = root.join("System32").join("Restore");
    RepairCheck {
        id: "restore-state".into(), title: "System Restore readiness".into(), stage: "Unknown".into(),
        result_code: if sr_dir.is_dir() { "RestoreStateUnverified" } else { "RestoreStateUnknown" }.into(), exit_code: 0,
        detail: "System Restore availability and restore-point creation success are treated as runtime recovery evidence; directory presence alone is not claimed as protection.".into(),
        log_hint: String::new(),
    }
}

fn sanitize(value: &str) -> String {
    value
        .chars()
        .filter(|character| {
            *character == '\n'
                || *character == '\r'
                || *character == '\t'
                || !character.is_control()
        })
        .collect()
}
