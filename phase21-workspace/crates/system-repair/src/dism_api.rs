#![cfg(windows)]

use std::{ffi::c_void, ptr};

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
    let hr = unsafe {
        DismCheckImageHealth(
            lifecycle.session,
            if scan_image { 1 } else { 0 },
            0,
            None,
            ptr::null_mut(),
            &mut health,
        )
    };
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
