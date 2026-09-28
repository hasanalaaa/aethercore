#![cfg(windows)]

use std::{
    ffi::c_void,
    ptr,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use aethercore_windows_foundation::OwnedHandle;
use windows::{
    Win32::{
        Foundation::HANDLE,
        System::Threading::{CreateEventW, SetEvent},
    },
    core::PCWSTR,
};

use super::{RepairCheck, RepairError, Result};

const DISM_LOG_ERRORS_WARNINGS_INFO: i32 = 2;
const DISM_ONLINE_IMAGE: &str = "DISM_{53BFAE52-B167-4E2F-A258-0A37B57FF845}";
const S_OK: i32 = 0;

#[link(name = "DismApi")]
unsafe extern "system" {
    fn DismInitialize(
        log_level: i32,
        log_file_path: *const u16,
        scratch_directory: *const u16,
    ) -> i32;
    fn DismShutdown() -> i32;
    fn DismOpenSession(
        image_path: *const u16,
        windows_directory: *const u16,
        system_drive: *const u16,
        session: *mut u32,
    ) -> i32;
    fn DismCloseSession(session: u32) -> i32;
    fn DismCheckImageHealth(
        session: u32,
        scan_image: i32,
        cancel_event: isize,
        progress: Option<unsafe extern "system" fn(u32, u32, *mut c_void)>,
        user_data: *mut c_void,
        image_health: *mut i32,
    ) -> i32;
}

/// A manual-reset event DISM polls, raised by a thread that watches the owner's cancel
/// flag. Dropping it stops the thread, then (field drop) closes the event.
struct CancelWatcher {
    event: OwnedHandle,
    done: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl CancelWatcher {
    fn start(cancel: &Arc<AtomicBool>) -> Result<Self> {
        let event = OwnedHandle::new(
            unsafe { CreateEventW(None, true, false, PCWSTR::null()) }
                .map_err(|error| RepairError::Command(format!("CreateEventW failed: {error}")))?,
        );
        let raw = event.get().0 as isize;
        let done = Arc::new(AtomicBool::new(false));
        let (thread_done, cancel) = (done.clone(), cancel.clone());
        let thread = std::thread::Builder::new()
            .name("aether-dism-cancel".into())
            .spawn(move || {
                while !thread_done.load(Ordering::SeqCst) {
                    if cancel.load(Ordering::SeqCst) {
                        // SAFETY: the event is closed only after this thread joins (`Drop`).
                        let _ = unsafe { SetEvent(HANDLE(raw as *mut c_void)) };
                        return;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(250));
                }
            })
            .map_err(|error| RepairError::Command(format!("DISM cancel watcher: {error}")))?;
        Ok(Self {
            event,
            done,
            thread: Some(thread),
        })
    }

    fn event(&self) -> isize {
        self.event.get().0 as isize
    }
}

impl Drop for CancelWatcher {
    fn drop(&mut self) {
        self.done.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

struct DismLifecycle {
    initialized: bool,
    session: u32,
}
impl Drop for DismLifecycle {
    fn drop(&mut self) {
        unsafe {
            if self.session != 0 {
                let _ = DismCloseSession(self.session);
            }
            if self.initialized {
                let _ = DismShutdown();
            }
        }
    }
}

pub fn check_online_image_health(scan_image: bool, id: &str, title: &str) -> Result<RepairCheck> {
    check_image_health(scan_image, id, title, None)
}

/// As [`check_online_image_health`], stopped through DISM's own cancel event once `cancel`
/// is raised (P76, DBT-P76-007): the assessment's ScanHealth is read-only and takes minutes.
pub fn check_online_image_health_cancellable(
    scan_image: bool,
    id: &str,
    title: &str,
    cancel: &Arc<AtomicBool>,
) -> Result<RepairCheck> {
    check_image_health(scan_image, id, title, Some(cancel))
}

fn check_image_health(
    scan_image: bool,
    id: &str,
    title: &str,
    cancel: Option<&Arc<AtomicBool>>,
) -> Result<RepairCheck> {
    let mut lifecycle = DismLifecycle {
        initialized: false,
        session: 0,
    };
    let init = unsafe { DismInitialize(DISM_LOG_ERRORS_WARNINGS_INFO, ptr::null(), ptr::null()) };
    if init != S_OK {
        return Err(RepairError::Command(format!(
            "DismInitialize failed: HRESULT=0x{:08X}",
            init as u32
        )));
    }
    lifecycle.initialized = true;

    let mut online = DISM_ONLINE_IMAGE.encode_utf16().collect::<Vec<_>>();
    online.push(0);
    let open = unsafe {
        DismOpenSession(
            online.as_ptr(),
            ptr::null(),
            ptr::null(),
            &mut lifecycle.session,
        )
    };
    if open != S_OK {
        return Err(RepairError::Command(format!(
            "DismOpenSession failed: HRESULT=0x{:08X}",
            open as u32
        )));
    }

    let mut health = -1i32;
    let watcher = cancel.map(CancelWatcher::start).transpose()?;
    let hr = unsafe {
        DismCheckImageHealth(
            lifecycle.session,
            if scan_image { 1 } else { 0 },
            watcher.as_ref().map_or(0, CancelWatcher::event),
            None,
            ptr::null_mut(),
            &mut health,
        )
    };
    drop(watcher);
    if cancel.is_some_and(|flag| flag.load(Ordering::SeqCst)) {
        return Err(RepairError::Cancelled);
    }
    if hr != S_OK {
        return Err(RepairError::Command(format!(
            "DismCheckImageHealth failed: HRESULT=0x{:08X}",
            hr as u32
        )));
    }

    let (result_code, stage, detail) = match health {
        0 => (
            "ComponentStoreHealthy",
            "Completed",
            "DISM API reports the online image as healthy.",
        ),
        1 => (
            "ComponentStoreRepairable",
            "Attention",
            "DISM API reports repairable online-image corruption.",
        ),
        2 => (
            "ComponentStoreNonRepairable",
            "Critical",
            "DISM API reports the online image as non-repairable by this servicing path.",
        ),
        value => (
            "ComponentStoreUnknown",
            "Unknown",
            if value < 0 {
                "DISM API did not return an image-health state."
            } else {
                "DISM API returned an unknown image-health state."
            },
        ),
    };
    Ok(RepairCheck {
        id: id.into(),
        title: title.into(),
        stage: stage.into(),
        result_code: result_code.into(),
        exit_code: hr,
        detail: detail.into(),
        log_hint: "%WINDIR%\\Logs\\DISM\\dism.log".into(),
    })
}
