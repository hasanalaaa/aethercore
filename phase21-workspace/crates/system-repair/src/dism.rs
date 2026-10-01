//! P85-02: what the DISM API's progress callback and HRESULTs mean, decided where they can be tested
//! without Windows. The FFI itself is in `dism_api.rs`.

use crate::RepairError;

// ponytail: one process-wide DISM lifecycle; split only if the API supports independent lifetimes.
static SESSION: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn acquire_session() -> crate::Result<std::sync::MutexGuard<'static, ()>> {
    SESSION.try_lock().map_err(|_| RepairError::ServicingBusy)
}

/// HRESULT_FROM_WIN32(ERROR_CANCELLED): the owner's cancel event reached the API.
const E_CANCELLED: u32 = 0x8007_04C7;
/// CBS_E_SOURCE_MISSING: the files a component-store repair needs are not available locally.
const CBS_E_SOURCE_MISSING: u32 = 0x800F_081F;

/// The tool's own percent, only when its callback gave a total and a current inside it. A total of 0,
/// or a current past the total, is "cannot say", never 0% and never 100%.
pub fn progress_percent(current: u32, total: u32) -> Option<u32> {
    if total == 0 || current > total {
        return None;
    }
    Some((u64::from(current) * 100 / u64::from(total)) as u32)
}

/// What a failed `DismRestoreImageHealth` means for the plan.
pub fn restore_error(hresult: i32) -> RepairError {
    match hresult as u32 {
        CBS_E_SOURCE_MISSING => RepairError::SourceRequired(
            "Windows could not find the source files a component-store repair needs (HRESULT 0x800F081F).".into(),
        ),
        E_CANCELLED => RepairError::RepairStopped,
        code => RepairError::Command(format!("DISM RestoreHealth failed: HRESULT=0x{code:08X}")),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn dism_sessions_cannot_overlap_and_the_slot_is_released_on_return() {
        let first = super::acquire_session().expect("first session");
        assert!(matches!(
            super::acquire_session(),
            Err(crate::RepairError::ServicingBusy)
        ));
        drop(first);
        assert!(super::acquire_session().is_ok());
    }
}
