use std::{
    fs,
    io::Read,
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use super::{
    RepairCheck, RepairError, RepairPlatform, Result, dism_api::check_online_image_health,
};
use aethercore_operation_engine::SystemRepairAction;
use aethercore_windows_foundation::{MachineMutationGuard, OwnedServiceHandle};
use windows::{
    Win32::System::Services::{
        OpenSCManagerW, OpenServiceW, QueryServiceStatusEx, SC_MANAGER_CONNECT,
        SC_STATUS_PROCESS_INFO, SERVICE_QUERY_STATUS, SERVICE_RUNNING, SERVICE_START,
        SERVICE_STATUS_PROCESS, StartServiceW,
    },
    core::PCWSTR,
};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const COMMAND_TIMEOUT: Duration = Duration::from_secs(2 * 60 * 60);
const CBS_TAIL_LIMIT: u64 = 4 * 1024 * 1024;

pub struct WindowsRepairPlatform;

struct ServicingGuard(MachineMutationGuard);

impl RepairPlatform for WindowsRepairPlatform {
    fn assess(&self) -> Result<(String, Vec<RepairCheck>)> {
        let root = system_root()?;
        let volume = system_volume()?;
        let system32 = root.join("System32");

        // A failing collector becomes Unknown evidence for its own domain. It does not make
        // Windows globally broken and it does not fabricate a repair candidate.
        let mut checks = Vec::new();
        checks.push(probe_or_unknown(
            "dism-scan",
            "Component store",
            "%WINDIR%\\Logs\\DISM\\dism.log",
            || check_online_image_health(true, "dism-scan", "Component store"),
        ));
        checks.push(probe_or_unknown(
            "sfc-verify",
            "Protected system files",
            "%WINDIR%\\Logs\\CBS\\CBS.log",
            || {
                run_sfc(
                    &system32.join("sfc.exe"),
                    &["/verifyonly"],
                    "sfc-verify",
                    "Protected system files",
                    &root,
                )
            },
        ));
        checks.push(servicing_state_check());
        let update = update_health_check();
        let update_failed = update.result_code == "UpdateFailure";
        checks.push(update);
        if update_failed {
            checks.push(required_update_service_check());
        }
        checks.push(probe_or_unknown(
            "disk-scan",
            "System volume online scan",
            "Event Viewer → Application → Chkdsk",
            || {
                run_chkdsk_scan(
                    &system32.join("chkdsk.exe"),
                    &volume,
                    "disk-scan",
                    "System volume online scan",
                )
            },
        ));
        checks.push(winre_presence_check(&system32));
        checks.push(restore_readiness_check(&root));

        Ok((volume, checks))
    }

    fn repair(
        &self,
        action: &SystemRepairAction,
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

        if start_required_service {
            emit(start_update_service()?);
        }
        if action.run_component_store {
            emit(run_dism_restore(&system32.join("dism.exe"))?);
        }
        if action.run_system_files {
            emit(run_sfc(
                &system32.join("sfc.exe"),
                &["/scannow"],
                "sfc-repair",
                "System File Checker repair",
                &root,
            )?);
        }
        if action.run_disk_scan {
            let volume = system_volume()?;
            emit(run_chkdsk_scan(
                &system32.join("chkdsk.exe"),
                &volume,
                "disk-scan",
                "System volume online scan",
            )?);
        }
        Ok(())
    }

    fn verify(&self, action: &SystemRepairAction, emit: &mut dyn FnMut(RepairCheck)) -> Result<()> {
        let root = system_root()?;
        let system32 = root.join("System32");
        if action.run_component_store {
            emit(check_online_image_health(
                true,
                "verify-dism",
                "Verify component store",
            )?);
        }
        if action.run_system_files {
            emit(run_sfc(
                &system32.join("sfc.exe"),
                &["/verifyonly"],
                "verify-sfc",
                "Verify protected system files",
                &root,
            )?);
        }
        if action.run_disk_scan {
            let volume = system_volume()?;
            emit(run_chkdsk_scan(
                &system32.join("chkdsk.exe"),
                &volume,
                "verify-disk",
                "Verify system volume",
            )?);
        }
        if action
            .repair_action_ids
            .iter()
            .any(|id| id == "start-required-service")
        {
            let mut check = required_update_service_check();
            check.id = "verify-required-service".into();
            check.title = "Verify required Windows Update service".into();
            emit(check);
            let mut update = update_health_check();
            update.id = "verify-windows-update".into();
            update.title = "Verify Windows Update discovery".into();
            emit(update);
        }
        Ok(())
    }
}

fn acquire_servicing_guard() -> Result<ServicingGuard> {
    let guard = MachineMutationGuard::try_acquire()
        .map_err(|error| RepairError::Command(error.to_string()))?
        .ok_or(RepairError::ServicingBusy)?;
    Ok(ServicingGuard(guard))
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
        Ok(status.dwCurrentState == SERVICE_RUNNING)
    }
}

fn required_update_service_check() -> RepairCheck {
    match service_running("wuauserv") {
        Ok(true) => RepairCheck { id:"required-service".into(), title:"Windows Update service".into(), stage:"Completed".into(), result_code:"ServiceRunning".into(), exit_code:0, detail:"The diagnosis-scoped Windows Update service is running.".into(), log_hint:"wuauserv".into() },
        Ok(false) => RepairCheck { id:"required-service".into(), title:"Windows Update service".into(), stage:"Attention".into(), result_code:"ServiceStopped".into(), exit_code:0, detail:"Windows Update discovery failed and its required wuauserv service is not running. AetherCore may offer only this targeted service start; it will not reset unrelated services.".into(), log_hint:"wuauserv".into() },
        Err(error) => RepairCheck { id:"required-service".into(), title:"Windows Update service".into(), stage:"Unknown".into(), result_code:"ServiceUnknown".into(), exit_code:-1, detail:sanitize(&error.to_string()), log_hint:"wuauserv".into() },
    }
}

fn start_update_service() -> Result<RepairCheck> {
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

fn run_dism_restore(exe: &Path) -> Result<RepairCheck> {
    match run_with_accepted_codes(
        exe,
        &["/Online", "/Cleanup-Image", "/RestoreHealth"],
        "dism-restore",
        "DISM RestoreHealth",
        "%WINDIR%\\Logs\\DISM\\dism.log",
        &[0],
    ) {
        Ok(mut check) => {
            check.result_code = "MutationSucceeded".into();
            check.detail = "DISM RestoreHealth completed. This is mutation evidence only; component-store health must still be verified.".into();
            Ok(check)
        }
        Err(RepairError::Command(detail)) if detail.contains("-2146498529") || detail.contains("0x800F081F") => {
            Err(RepairError::SourceRequired("Windows could not locate the source files required for component-store repair (HRESULT 0x800F081F).".into()))
        }
        Err(error) => Err(error),
    }
}

fn run_sfc(exe: &Path, args: &[&str], id: &str, title: &str, root: &Path) -> Result<RepairCheck> {
    let mut check = run_with_accepted_codes(
        exe,
        args,
        id,
        title,
        "%WINDIR%\\Logs\\CBS\\CBS.log",
        &[0, 1, 2],
    )?;

    // Do not infer integrity state from localized console prose. SFC's console output is retained
    // only as bounded diagnostic context. The normalized result comes from the bounded CBS [SR]
    // evidence stream when present; otherwise the state remains Unknown and cannot resolve a Finding.
    let cbs = parse_recent_cbs_sr(&root.join("Logs").join("CBS").join("CBS.log"));
    let repair_mode = args.iter().any(|arg| arg.eq_ignore_ascii_case(&"/scannow"));
    match cbs {
        CbsIntegrityEvidence::NoViolation => {
            check.stage = "Completed".into();
            check.result_code = "SystemFilesHealthy".into();
            check.detail = "Recent CBS [SR] evidence contains no unresolved protected-file integrity violation for this check window.".into();
        }
        CbsIntegrityEvidence::ViolationRepaired if repair_mode => {
            check.stage = "Completed".into();
            check.result_code = "SystemFilesRepairMutationSucceeded".into();
            check.detail = "CBS [SR] evidence records protected-file repair activity. A separate verify-only pass is still required.".into();
        }
        CbsIntegrityEvidence::ViolationRepaired => {
            check.stage = "Attention".into();
            check.result_code = "SystemFilesCorrupt".into();
            check.detail = "CBS [SR] evidence records protected-file repair activity; verify-only did not provide enough evidence to mark the state healthy.".into();
        }
        CbsIntegrityEvidence::ViolationUnresolved => {
            check.stage = "Attention".into();
            check.result_code = "SystemFilesCorrupt".into();
            check.detail = "Recent CBS [SR] evidence indicates unresolved protected-file integrity work is required.".into();
        }
        CbsIntegrityEvidence::Unknown => {
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

#[derive(Clone, Copy)]
enum CbsIntegrityEvidence {
    NoViolation,
    ViolationRepaired,
    ViolationUnresolved,
    Unknown,
}

fn parse_recent_cbs_sr(path: &Path) -> CbsIntegrityEvidence {
    let Ok(metadata) = fs::metadata(path) else {
        return CbsIntegrityEvidence::Unknown;
    };
    let Ok(mut file) = fs::File::open(path) else {
        return CbsIntegrityEvidence::Unknown;
    };
    let start = metadata.len().saturating_sub(CBS_TAIL_LIMIT);
    if start > 0 {
        use std::io::{Seek, SeekFrom};
        if file.seek(SeekFrom::Start(start)).is_err() {
            return CbsIntegrityEvidence::Unknown;
        }
    }
    let mut bytes = Vec::new();
    if file.take(CBS_TAIL_LIMIT).read_to_end(&mut bytes).is_err() {
        return CbsIntegrityEvidence::Unknown;
    }
    let text = String::from_utf8_lossy(&bytes);
    let sr = text
        .lines()
        .filter(|line| line.contains("[SR]"))
        .collect::<Vec<_>>();
    if sr.is_empty() {
        return CbsIntegrityEvidence::Unknown;
    }

    // These tokens are component identifiers emitted by CBS rather than localized SFC console prose.
    // The parser is deliberately conservative: anything ambiguous remains Unknown.
    let unresolved = sr.iter().any(|line| {
        let lower = line.to_ascii_lowercase();
        lower.contains("cannot repair") || lower.contains("repair failed")
    });
    if unresolved {
        return CbsIntegrityEvidence::ViolationUnresolved;
    }
    let repaired = sr.iter().any(|line| {
        let lower = line.to_ascii_lowercase();
        lower.contains("repairing")
            || lower.contains("repaired file")
            || lower.contains("repair complete")
    });
    if repaired {
        return CbsIntegrityEvidence::ViolationRepaired;
    }
    CbsIntegrityEvidence::NoViolation
}

fn run_chkdsk_scan(exe: &Path, volume: &str, id: &str, title: &str) -> Result<RepairCheck> {
    let mut check = run_with_accepted_codes(
        exe,
        &[volume, "/scan"],
        id,
        title,
        "Event Viewer → Application → Chkdsk",
        &[0, 1, 2, 3],
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

fn winre_presence_check(system32: &Path) -> RepairCheck {
    let exe = system32.join("reagentc.exe");
    if exe.is_file() {
        RepairCheck {
            id: "winre-state".into(), title: "Windows Recovery Environment".into(), stage: "Unknown".into(),
            result_code: "WinReStateUnverified".into(), exit_code: 0,
            detail: "REAgentC is available, but Phase 19 does not infer enabled/disabled WinRE state from localized console prose. Live state qualification remains pending on Windows.".into(),
            log_hint: "reagentc.exe /info (guided technical detail)".into(),
        }
    } else {
        RepairCheck {
            id: "winre-state".into(), title: "Windows Recovery Environment".into(), stage: "Unknown".into(),
            result_code: "WinReStateUnknown".into(), exit_code: -1,
            detail: "The supported REAgentC executable was not found at the trusted System32 path; recovery state is unknown, not assumed unavailable.".into(),
            log_hint: String::new(),
        }
    }
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

fn run_with_accepted_codes(
    exe: &Path,
    args: &[&str],
    id: &str,
    title: &str,
    log_hint: &str,
    accepted_codes: &[i32],
) -> Result<RepairCheck> {
    if !exe.is_absolute() || !exe.is_file() {
        return Err(RepairError::Command(format!(
            "required Windows executable missing: {}",
            exe.display()
        )));
    }

    let mut child = Command::new(exe)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|error| RepairError::Command(error.to_string()))?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let stdout_thread = match thread::Builder::new()
        .name("aether-repair-stdout".into())
        .spawn(move || read_tail(stdout, 32_768))
    {
        Ok(worker) => worker,
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(RepairError::Command(format!(
                "failed to create repair stdout reader: {error}"
            )));
        }
    };
    let stderr_thread = match thread::Builder::new()
        .name("aether-repair-stderr".into())
        .spawn(move || read_tail(stderr, 16_384))
    {
        Ok(worker) => worker,
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            let _ = stdout_thread.join();
            return Err(RepairError::Command(format!(
                "failed to create repair stderr reader: {error}"
            )));
        }
    };
    let deadline = Instant::now() + COMMAND_TIMEOUT;

    let status = loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|error| RepairError::Command(error.to_string()))?
        {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            let _ = stdout_thread.join();
            let _ = stderr_thread.join();
            return Err(RepairError::Command(format!("{title} timed out")));
        }
        thread::sleep(Duration::from_millis(500));
    };

    let mut notes = Vec::new();
    let mut detail = super::joined_stream(stdout_thread.join(), "stdout", &mut notes);
    let error_output = super::joined_stream(stderr_thread.join(), "stderr", &mut notes);
    if !error_output.trim().is_empty() {
        if !detail.is_empty() {
            detail.push('\n');
        }
        detail.push_str(&error_output);
    }
    super::trim_to_tail(&mut detail, 48_000);
    // After truncation, so a lost stream is never itself truncated away.
    for note in notes {
        if !detail.is_empty() {
            detail.push('\n');
        }
        detail.push_str(&note);
    }

    let code = status.code().unwrap_or(-1);
    if !accepted_codes.contains(&code) {
        return Err(RepairError::Command(format!(
            "{title} exited with code {code}; see {log_hint}"
        )));
    }

    Ok(RepairCheck {
        id: id.into(),
        title: title.into(),
        stage: "Completed".into(),
        result_code: format!("ExitCode{code}"),
        exit_code: code,
        detail: sanitize(&detail),
        log_hint: log_hint.into(),
    })
}

fn read_tail<R: Read>(reader: Option<R>, limit: usize) -> String {
    let Some(mut reader) = reader else {
        return String::new();
    };
    let mut buffer = Vec::new();
    let _ = reader.read_to_end(&mut buffer);
    if buffer.len() > limit {
        buffer.drain(..buffer.len() - limit);
    }
    String::from_utf8_lossy(&buffer).into_owned()
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
