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

use super::{RepairCheck, RepairControl, RepairError, Result, dism};

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
    fn DismRestoreImageHealth(
        session: u32,
        source_paths: *const *const u16,
        source_path_count: u32,
        limit_access: i32,
        cancel_event: isize,
        progress: Option<unsafe extern "system" fn(u32, u32, *mut c_void)>,
        user_data: *mut c_void,
    ) -> i32;
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
    timed_out: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl CancelWatcher {
    fn start(cancel: &Arc<AtomicBool>, deadline: Option<std::time::Duration>) -> Result<Self> {
        let event = OwnedHandle::new(
            unsafe { CreateEventW(None, true, false, PCWSTR::null()) }
                .map_err(|error| RepairError::Command(format!("CreateEventW failed: {error}")))?,
        );
        let raw = event.get().0 as isize;
        let done = Arc::new(AtomicBool::new(false));
        let timed_out = Arc::new(AtomicBool::new(false));
        let (thread_done, cancel, expired) = (done.clone(), cancel.clone(), timed_out.clone());
        let began = std::time::Instant::now();
        let thread = std::thread::Builder::new()
            .name("aether-dism-cancel".into())
            .spawn(move || {
                while !thread_done.load(Ordering::SeqCst) {
                    // The owner's cancel, or the deadline: either raises DISM's own cancel event, once.
                    let passed_deadline = deadline.is_some_and(|limit| began.elapsed() >= limit);
                    if cancel.load(Ordering::SeqCst) || passed_deadline {
                        expired.store(passed_deadline, Ordering::SeqCst);
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
            timed_out,
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

struct DismLifecycle<'a> {
    initialized: bool,
    session: u32,
    drain: super::servicing::ServicingDrain<'a>,
}
impl Drop for DismLifecycle<'_> {
    fn drop(&mut self) {
        super::servicing::finish_dism_lifecycle(
            self.initialized,
            self.session,
            |session| unsafe { DismCloseSession(session) },
            || unsafe { DismShutdown() },
            || self.drain.note_pending(),
            || std::thread::sleep(std::time::Duration::from_millis(250)),
        );
        // Field drop performs the SCM fence only after confirmed lifecycle cleanup. If
        // cleanup is uncertain, this owned worker and all admission remain retained.
    }
}

pub fn check_online_image_health(
    scan_image: bool,
    id: &str,
    title: &str,
    pending: Option<&mut (dyn FnMut() + Send)>,
) -> Result<RepairCheck> {
    check_image_health(scan_image, id, title, None, pending)
}

/// As [`check_online_image_health`], stopped through DISM's own cancel event once `cancel`
/// is raised (P76, DBT-P76-007): the assessment's ScanHealth is read-only and takes minutes.
pub fn check_online_image_health_cancellable(
    scan_image: bool,
    id: &str,
    title: &str,
    cancel: &Arc<AtomicBool>,
    pending: Option<&mut (dyn FnMut() + Send)>,
) -> Result<RepairCheck> {
    check_image_health(scan_image, id, title, Some(cancel), pending)
}

fn check_image_health(
    scan_image: bool,
    id: &str,
    title: &str,
    cancel: Option<&Arc<AtomicBool>>,
    pending: Option<&mut (dyn FnMut() + Send)>,
) -> Result<RepairCheck> {
    let _session_guard = dism::acquire_session()?;
    let drain = super::servicing::ServicingDrain::before_owned_call(pending)?;
    let mut lifecycle = DismLifecycle {
        initialized: false,
        session: 0,
        drain,
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
    let watcher = cancel
        .map(|flag| CancelWatcher::start(flag, None))
        .transpose()?;
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

/// How long `DismRestoreImageHealth` may run before DISM's own cancel event is raised (once). A first
/// bound, not a measurement; past it the image is unknown until verified again.
const RESTORE_DEADLINE: std::time::Duration = std::time::Duration::from_secs(2 * 60 * 60);

/// P85-02: repairs the online component store through the DISM API, from local files only
/// (`LimitAccess = TRUE`: Windows Update is never asked, and no source path is given), reporting the
/// API's own progress and stopping through its cancel event when the owner cancels or the deadline
/// passes. The API returning success is mutation evidence only; the plan still verifies afterwards.
pub fn restore_online_image_health(control: &mut RepairControl<'_>) -> Result<RepairCheck> {
    struct Sink<'a>(std::sync::Mutex<&'a mut (dyn FnMut(Option<u32>) + Send)>);

    // A callback must not panic across the FFI boundary or touch anything shared: it forwards the
    // percent (or `None` when the API gives no total) to the coordinator's live telemetry.
    unsafe extern "system" fn on_progress(current: u32, total: u32, user_data: *mut c_void) {
        // SAFETY: `user_data` is the `Sink` below, alive for the whole `DismRestoreImageHealth` call.
        let sink = unsafe { &*(user_data as *const Sink<'_>) };
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            if let Ok(mut progress) = sink.0.try_lock() {
                progress(dism::progress_percent(current, total));
            }
        }));
    }

    let _session_guard = dism::acquire_session()?;
    let drain =
        super::servicing::ServicingDrain::before_owned_call(Some(&mut *control.servicing_pending))?;
    let mut lifecycle = DismLifecycle {
        initialized: false,
        session: 0,
        drain,
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

    (control.progress)(None);
    let watcher = CancelWatcher::start(control.cancel, Some(RESTORE_DEADLINE))?;
    let mut sink = Sink(std::sync::Mutex::new(&mut *control.progress));
    let hr = unsafe {
        DismRestoreImageHealth(
            lifecycle.session,
            ptr::null(),
            0,
            1, // TRUE: LimitAccess disables Windows Update source lookup.
            watcher.event(),
            Some(on_progress),
            &mut sink as *mut Sink<'_> as *mut c_void,
        )
    };
    let timed_out = watcher.timed_out.load(Ordering::SeqCst);
    drop(watcher);
    if timed_out {
        return Err(RepairError::RepairTimedOut);
    }
    if control.cancel.load(Ordering::SeqCst) {
        return Err(RepairError::RepairStopped);
    }
    if hr != S_OK {
        return Err(dism::restore_error(hr));
    }
    Ok(RepairCheck {
        id: "dism-restore".into(),
        title: "DISM RestoreHealth".into(),
        stage: "Completed".into(),
        result_code: "MutationSucceeded".into(),
        exit_code: 0,
        detail: "DISM RestoreHealth completed. This is mutation evidence only; component-store health must still be verified.".into(),
        log_hint: "%WINDIR%\\Logs\\DISM\\dism.log".into(),
    })
}
