use aethercore_windows_foundation::{ComApartment, MachineMutationGuard};
use std::{
    collections::HashMap,
    thread,
    time::{Duration, Instant},
};

use windows::{
    Win32::{
        Foundation::{DECIMAL, VARIANT_BOOL},
        System::{
            Com::{CLSCTX_INPROC_SERVER, CoCreateInstance},
            UpdateAgent::{
                IDownloadCompletedCallback, IDownloadCompletedCallback_Impl,
                IDownloadCompletedCallbackArgs, IDownloadJob, IDownloadProgressChangedCallback,
                IDownloadProgressChangedCallback_Impl, IDownloadProgressChangedCallbackArgs,
                IInstallationCompletedCallback, IInstallationCompletedCallback_Impl,
                IInstallationCompletedCallbackArgs, IInstallationJob,
                IInstallationProgressChangedCallback, IInstallationProgressChangedCallback_Impl,
                IInstallationProgressChangedCallbackArgs, IUpdate, IUpdateInstaller2,
                IUpdateSession, IWindowsDriverUpdate, UpdateSession, orcAborted, orcFailed,
                orcInProgress, orcNotStarted, orcSucceeded, orcSucceededWithErrors,
            },
            Variant::VARIANT,
        },
    },
    core::{BSTR, Interface, Ref},
};

use crate::{
    ExecutionStage, Result, UpdateError, UpdateIdentity, WuaExecutionResult, WuaProgress,
    WuaUpdateResult,
};

const SEARCH_CRITERIA: &str = "IsInstalled=0 and Type='Driver' and IsHidden=0";
const POLL_INTERVAL: Duration = Duration::from_millis(500);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(2 * 60 * 60);
const INSTALL_TIMEOUT: Duration = Duration::from_secs(2 * 60 * 60);

struct MachineMutationLease(MachineMutationGuard);

#[windows::core::implement(IDownloadProgressChangedCallback)]
struct DownloadProgressCallback;
impl IDownloadProgressChangedCallback_Impl for DownloadProgressCallback_Impl {
    fn Invoke(
        &self,
        _downloadjob: Ref<'_, IDownloadJob>,
        _callbackargs: Ref<'_, IDownloadProgressChangedCallbackArgs>,
    ) -> windows::core::Result<()> {
        Ok(())
    }
}

#[windows::core::implement(IDownloadCompletedCallback)]
struct DownloadCompletedCallback;
impl IDownloadCompletedCallback_Impl for DownloadCompletedCallback_Impl {
    fn Invoke(
        &self,
        _downloadjob: Ref<'_, IDownloadJob>,
        _callbackargs: Ref<'_, IDownloadCompletedCallbackArgs>,
    ) -> windows::core::Result<()> {
        Ok(())
    }
}

#[windows::core::implement(IInstallationProgressChangedCallback)]
struct InstallationProgressCallback;
impl IInstallationProgressChangedCallback_Impl for InstallationProgressCallback_Impl {
    fn Invoke(
        &self,
        _installationjob: Ref<'_, IInstallationJob>,
        _callbackargs: Ref<'_, IInstallationProgressChangedCallbackArgs>,
    ) -> windows::core::Result<()> {
        Ok(())
    }
}

#[windows::core::implement(IInstallationCompletedCallback)]
struct InstallationCompletedCallback;
impl IInstallationCompletedCallback_Impl for InstallationCompletedCallback_Impl {
    fn Invoke(
        &self,
        _installationjob: Ref<'_, IInstallationJob>,
        _callbackargs: Ref<'_, IInstallationCompletedCallbackArgs>,
    ) -> windows::core::Result<()> {
        Ok(())
    }
}

/// Conservative servicing preflight used by non-WUA maintenance flows (for example DISM/SFC).
/// The caller must hold AetherCore's machine-wide mutation lock for the entire operation. This
/// function only asks the Windows Update Agent whether another installer is active or Windows
/// requires a reboot; it does not reserve the WUA pipeline and performs no update mutation.
pub fn ensure_servicing_available() -> Result<()> {
    let _com = ComApartment::mta()
        .map_err(|hr| UpdateError::Wua(format!("CoInitializeEx failed: 0x{:08X}", hr.0 as u32)))?;
    unsafe {
        let session: IUpdateSession =
            CoCreateInstance(&UpdateSession, None, CLSCTX_INPROC_SERVER).map_err(wua_err)?;
        session
            .SetClientApplicationID(&BSTR::from("AetherCore System Repair Preflight"))
            .map_err(wua_err)?;
        let installer = session.CreateUpdateInstaller().map_err(wua_err)?;
        installer
            .SetClientApplicationID(&BSTR::from("AetherCore System Repair Preflight"))
            .map_err(wua_err)?;
        if installer.IsBusy().map_err(wua_err)?.as_bool() {
            return Err(UpdateError::Busy);
        }
        if installer
            .RebootRequiredBeforeInstallation()
            .map_err(wua_err)?
            .as_bool()
        {
            return Err(UpdateError::RebootPending);
        }
    }
    Ok(())
}

pub fn execute_driver_updates<P, B>(
    identities: &[UpdateIdentity],
    mut progress: P,
    before_install: B,
) -> Result<WuaExecutionResult>
where
    P: FnMut(WuaProgress),
    B: FnOnce() -> std::result::Result<(), String>,
{
    if identities.is_empty() {
        return Err(UpdateError::OfferNoLongerApplicable(
            "empty selection".into(),
        ));
    }
    let _mutex = acquire_mutation_mutex()?;

    let _com = ComApartment::mta()
        .map_err(|hr| UpdateError::Wua(format!("CoInitializeEx failed: 0x{:08X}", hr.0 as u32)))?;

    unsafe {
        let session: IUpdateSession =
            CoCreateInstance(&UpdateSession, None, CLSCTX_INPROC_SERVER).map_err(wua_err)?;
        session
            .SetClientApplicationID(&BSTR::from("AetherCore Safe Driver Installation"))
            .map_err(wua_err)?;
        let selected = revalidate_selection(&session, identities)?;
        // DBT-P46-B16: None, not Some(0) — this stage reports no byte progress at
        // all, which is not the same claim as "zero bytes transferred".
        progress(WuaProgress {
            stage: ExecutionStage::Revalidating,
            percent: 100,
            current_update_index: 0,
            current_update_percent: 100,
            bytes_downloaded: None,
            bytes_total: None,
        });

        // Ask WUA itself whether another installer owns the installation pipeline. The cross-process mutation lock
        // above only serializes AetherCore instances; IsBusy protects against the system orchestrator
        // and other WUA clients.
        let installer = session.CreateUpdateInstaller().map_err(wua_err)?;
        installer
            .SetClientApplicationID(&BSTR::from("AetherCore Safe Driver Installation"))
            .map_err(wua_err)?;
        installer.SetUpdates(&selected).map_err(wua_err)?;
        installer.SetIsForced(VARIANT_BOOL(0)).map_err(wua_err)?;
        installer
            .SetAllowSourcePrompts(VARIANT_BOOL(0))
            .map_err(wua_err)?;
        // Quiet means no Windows Update UI or source prompts. It does not force applicability:
        // SetIsForced remains false, and WUA still owns ranking/applicability decisions.
        let installer2: IUpdateInstaller2 = installer.cast().map_err(wua_err)?;
        installer2
            .SetForceQuiet(VARIANT_BOOL(-1))
            .map_err(wua_err)?;
        if installer.IsBusy().map_err(wua_err)?.as_bool() {
            return Err(UpdateError::Busy);
        }
        if installer
            .RebootRequiredBeforeInstallation()
            .map_err(wua_err)?
            .as_bool()
        {
            return Err(UpdateError::RebootPending);
        }

        let downloader = session.CreateUpdateDownloader().map_err(wua_err)?;
        downloader
            .SetClientApplicationID(&BSTR::from("AetherCore Safe Driver Installation"))
            .map_err(wua_err)?;
        downloader.SetUpdates(&selected).map_err(wua_err)?;
        downloader.SetIsForced(VARIANT_BOOL(0)).map_err(wua_err)?;

        let dp: IDownloadProgressChangedCallback = DownloadProgressCallback.into();
        let dc: IDownloadCompletedCallback = DownloadCompletedCallback.into();
        let state = VARIANT::default();
        let download_job = downloader
            .BeginDownload(&dp, &dc, &state)
            .map_err(wua_err)?;
        wait_download(&download_job, &mut progress)?;
        let download_result = downloader.EndDownload(&download_job).map_err(wua_err)?;
        let _ = download_job.CleanUp();
        let download_code = download_result.ResultCode().map_err(wua_err)?;
        if !is_success(download_code) {
            return Err(UpdateError::Wua(format!(
                "driver download failed: {download_code:?}; HRESULT={:?}",
                download_result.HResult().map_err(wua_err)?
            )));
        }
        let count = selected.Count().map_err(wua_err)?;
        for index in 0..count {
            let item = download_result.GetUpdateResult(index).map_err(wua_err)?;
            let code = item.ResultCode().map_err(wua_err)?;
            if !is_success(code) {
                return Err(UpdateError::Wua(format!(
                    "download failed for selected update {index}: {code:?}"
                )));
            }
        }

        // Re-check immediately before the transactional mutation barrier: another Windows Update
        // operation may have started while packages were downloading.
        if installer.IsBusy().map_err(wua_err)?.as_bool() {
            return Err(UpdateError::Busy);
        }
        if installer
            .RebootRequiredBeforeInstallation()
            .map_err(wua_err)?
            .as_bool()
        {
            return Err(UpdateError::RebootPending);
        }

        // This callback is the final transactional barrier. The caller must durably create/verify
        // the restore point, export all applicable OEM driver packages, journal Protected, and
        // commit the Executing transition immediately before BeginInstall can be invoked.
        before_install().map_err(UpdateError::Protection)?;

        let ip: IInstallationProgressChangedCallback = InstallationProgressCallback.into();
        let ic: IInstallationCompletedCallback = InstallationCompletedCallback.into();
        let install_job = installer.BeginInstall(&ip, &ic, &state).map_err(wua_err)?;
        wait_install(&install_job, &mut progress)?;
        let result = installer.EndInstall(&install_job).map_err(wua_err)?;
        let _ = install_job.CleanUp();

        let overall_code = result.ResultCode().map_err(wua_err)?;
        let overall_hresult = result.HResult().map_err(wua_err)?;
        let overall_reboot = result.RebootRequired().map_err(wua_err)?.as_bool();
        let mut updates = Vec::with_capacity(identities.len());
        for (index, identity) in identities.iter().enumerate() {
            let item = result.GetUpdateResult(index as i32).map_err(wua_err)?;
            updates.push(WuaUpdateResult {
                identity: identity.clone(),
                result_code: result_code_name(item.ResultCode().map_err(wua_err)?).into(),
                hresult: item.HResult().map_err(wua_err)?,
                reboot_required: item.RebootRequired().map_err(wua_err)?.as_bool(),
            });
        }
        Ok(WuaExecutionResult {
            result_code: result_code_name(overall_code).into(),
            hresult: overall_hresult,
            reboot_required: overall_reboot || updates.iter().any(|u| u.reboot_required),
            updates,
        })
    }
}

unsafe fn revalidate_selection(
    session: &IUpdateSession,
    identities: &[UpdateIdentity],
) -> Result<windows::Win32::System::UpdateAgent::IUpdateCollection> {
    let searcher = unsafe { session.CreateUpdateSearcher().map_err(wua_err)? };
    unsafe {
        searcher.SetOnline(VARIANT_BOOL(-1)).map_err(wua_err)?;
    }
    let search = unsafe {
        searcher
            .Search(&BSTR::from(SEARCH_CRITERIA))
            .map_err(wua_err)?
    };
    let code = unsafe { search.ResultCode().map_err(wua_err)? };
    // Installation preflight is stricter than discovery: partial search results must not authorize
    // a mutation because absence from an incomplete result is not trustworthy.
    if code != orcSucceeded {
        return Err(UpdateError::Wua(format!(
            "WUA pre-install revalidation was not complete: {code:?}"
        )));
    }
    let all = unsafe { search.Updates().map_err(wua_err)? };
    let selected = unsafe { all.Copy().map_err(wua_err)? };
    unsafe {
        selected.Clear().map_err(wua_err)?;
    }

    let mut found: HashMap<(String, i32), IUpdate> = HashMap::new();
    let count = unsafe { all.Count().map_err(wua_err)? };
    for i in 0..count {
        let update = unsafe { all.get_Item(i).map_err(wua_err)? };
        if update.cast::<IWindowsDriverUpdate>().is_err() {
            continue;
        }
        let identity = unsafe { update.Identity().map_err(wua_err)? };
        let key = (
            unsafe { identity.UpdateID().map_err(wua_err)?.to_string() },
            unsafe { identity.RevisionNumber().map_err(wua_err)? },
        );
        if identities
            .iter()
            .any(|want| want.update_id == key.0 && want.revision == key.1)
        {
            found.insert(key, update);
        }
    }

    for identity in identities {
        let key = (identity.update_id.clone(), identity.revision);
        let update = found.remove(&key).ok_or_else(|| {
            UpdateError::OfferNoLongerApplicable(format!(
                "{} rev {}",
                identity.update_id, identity.revision
            ))
        })?;
        if unsafe { update.IsInstalled().map_err(wua_err)?.as_bool() } {
            return Err(UpdateError::OfferNoLongerApplicable(format!(
                "{} is already installed",
                identity.update_id
            )));
        }
        if !unsafe { update.EulaAccepted().map_err(wua_err)?.as_bool() } {
            return Err(UpdateError::EulaRequired(identity.update_id.clone()));
        }
        let driver: IWindowsDriverUpdate = update.cast().map_err(wua_err)?;
        let class = unsafe { driver.DriverClass().map_err(wua_err)?.to_string() };
        if class.eq_ignore_ascii_case("firmware") {
            return Err(UpdateError::OfferNoLongerApplicable(
                "firmware is protected from generic driver execution".into(),
            ));
        }
        unsafe {
            selected.Add(&update).map_err(wua_err)?;
        }
    }
    Ok(selected)
}

unsafe fn wait_download<P: FnMut(WuaProgress)>(job: &IDownloadJob, progress: &mut P) -> Result<()> {
    let started = Instant::now();
    loop {
        if started.elapsed() > DOWNLOAD_TIMEOUT {
            unsafe {
                let _ = job.RequestAbort();
                let _ = job.CleanUp();
            }
            return Err(UpdateError::Timeout("download".into()));
        }
        let p = unsafe { job.GetProgress().map_err(wua_err)? };
        progress(WuaProgress {
            stage: ExecutionStage::Downloading,
            percent: clamp_percent(unsafe { p.PercentComplete().map_err(wua_err)? }),
            current_update_index: unsafe { p.CurrentUpdateIndex().map_err(wua_err)? }.max(0) as u32,
            current_update_percent: clamp_percent(unsafe {
                p.CurrentUpdatePercentComplete().map_err(wua_err)?
            }),
            // DBT-P46-B16: no .unwrap_or(0) — a failed conversion stays None so
            // the consumer keeps the last known figure instead of reporting a
            // download that appears to have rewound to zero bytes.
            bytes_downloaded: decimal_to_u64(&unsafe {
                p.TotalBytesDownloaded().map_err(wua_err)?
            })
            .ok(),
            bytes_total: decimal_to_u64(&unsafe { p.TotalBytesToDownload().map_err(wua_err)? })
                .ok(),
        });
        if unsafe { job.IsCompleted().map_err(wua_err)?.as_bool() } {
            break;
        }
        thread::sleep(POLL_INTERVAL);
    }
    Ok(())
}

unsafe fn wait_install<P: FnMut(WuaProgress)>(
    job: &IInstallationJob,
    progress: &mut P,
) -> Result<()> {
    let started = Instant::now();
    loop {
        if started.elapsed() > INSTALL_TIMEOUT {
            unsafe {
                let _ = job.RequestAbort();
                let _ = job.CleanUp();
            }
            return Err(UpdateError::Timeout("installation".into()));
        }
        let p = unsafe { job.GetProgress().map_err(wua_err)? };
        progress(WuaProgress {
            stage: ExecutionStage::Installing,
            percent: clamp_percent(unsafe { p.PercentComplete().map_err(wua_err)? }),
            current_update_index: unsafe { p.CurrentUpdateIndex().map_err(wua_err)? }.max(0) as u32,
            current_update_percent: clamp_percent(unsafe {
                p.CurrentUpdatePercentComplete().map_err(wua_err)?
            }),
            // DBT-P46-B16: the install stage reports no byte progress; None
            // says that, where 0 would claim a measured zero.
            bytes_downloaded: None,
            bytes_total: None,
        });
        if unsafe { job.IsCompleted().map_err(wua_err)?.as_bool() } {
            break;
        }
        thread::sleep(POLL_INTERVAL);
    }
    Ok(())
}

fn acquire_mutation_mutex() -> Result<MachineMutationLease> {
    let guard = MachineMutationGuard::try_acquire()
        .map_err(wua_err)?
        .ok_or(UpdateError::Busy)?;
    Ok(MachineMutationLease(guard))
}

fn clamp_percent(value: i32) -> u32 {
    value.clamp(0, 100) as u32
}
fn is_success(code: windows::Win32::System::UpdateAgent::OperationResultCode) -> bool {
    code == orcSucceeded
}
fn result_code_name(
    code: windows::Win32::System::UpdateAgent::OperationResultCode,
) -> &'static str {
    if code == orcNotStarted {
        "orcNotStarted"
    } else if code == orcInProgress {
        "orcInProgress"
    } else if code == orcSucceeded {
        "orcSucceeded"
    } else if code == orcSucceededWithErrors {
        "orcSucceededWithErrors"
    } else if code == orcFailed {
        "orcFailed"
    } else if code == orcAborted {
        "orcAborted"
    } else {
        "orcUnknown"
    }
}

fn decimal_to_u64(value: &DECIMAL) -> std::result::Result<u64, ()> {
    #[link(name = "oleaut32")]
    unsafe extern "system" {
        fn VarUI8FromDec(pdecin: *const DECIMAL, pout: *mut u64) -> windows::core::HRESULT;
    }
    let mut out = 0u64;
    let hr = unsafe { VarUI8FromDec(value, &mut out) };
    if hr.is_err() { Err(()) } else { Ok(out) }
}

fn wua_err(error: windows::core::Error) -> UpdateError {
    UpdateError::Wua(error.to_string())
}
