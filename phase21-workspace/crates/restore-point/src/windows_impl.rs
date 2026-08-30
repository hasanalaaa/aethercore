use std::sync::OnceLock;

use aethercore_windows_foundation::ComApartment;
use windows::{
    Win32::{
        Foundation::FreeLibrary,
        System::{
            Com::{
                CLSCTX_INPROC_SERVER, CoCreateInstance,
                CoInitializeSecurity, CoSetProxyBlanket, EOAC_NONE, RPC_C_AUTHN_LEVEL_CALL,
                RPC_C_AUTHN_LEVEL_DEFAULT,
                RPC_C_IMP_LEVEL_IMPERSONATE,
            },
            LibraryLoader::{GetProcAddress, LoadLibraryW},
            Rpc::{RPC_C_AUTHN_WINNT, RPC_C_AUTHZ_NONE},
            Wmi::{
                IWbemLocator, WBEM_FLAG_FORWARD_ONLY, WBEM_FLAG_RETURN_IMMEDIATELY, WBEM_S_FALSE, WBEM_S_TIMEDOUT, WbemLocator,
            },
        },
    },
    core::{BSTR, BOOL, PCWSTR},
};

use crate::{RestorePointError, RestorePointEvidence, Result, description_for_plan};

const BEGIN_SYSTEM_CHANGE: u32 = 100;
const END_SYSTEM_CHANGE: u32 = 101;
const DEVICE_DRIVER_INSTALL: u32 = 10;
const CANCELLED_OPERATION: u32 = 13;
const MAX_DESC_W: usize = 256;

#[repr(C)]
struct RestorePointInfoW {
    event_type: u32,
    restore_point_type: u32,
    sequence_number: i64,
    description: [u16; MAX_DESC_W],
}

#[repr(C)]
#[derive(Default)]
struct StateManagerStatus {
    status: u32,
    sequence_number: i64,
}

type SrSetRestorePointW = unsafe extern "system" fn(*mut RestorePointInfoW, *mut StateManagerStatus) -> BOOL;

static COM_SECURITY: OnceLock<std::result::Result<(), String>> = OnceLock::new();

fn initialize_current_thread_com() -> Result<ComApartment> {
    ComApartment::mta().map_err(|hr| RestorePointError::ComSecurity(format!(
        "CoInitializeEx failed on restore-point worker thread: 0x{:08X}", hr.0 as u32
    )))
}

/// Must be called by the maintenance service before any worker creates WUA/WMI COM objects.
/// CoInitializeSecurity is process-wide, while CoInitializeEx is apartment/thread scoped; this
/// function deliberately initializes COM only long enough to establish process security, then
/// releases the startup thread's COM apartment. Worker threads initialize their own apartment.
pub fn initialize_process_com_security() -> Result<()> {
    let value = COM_SECURITY.get_or_init(|| {
        let _com = ComApartment::mta().map_err(|hr| format!(
            "CoInitializeEx failed during process security initialization: 0x{:08X}", hr.0 as u32
        ))?;
        unsafe {
            CoInitializeSecurity(
                None,
                -1,
                None,
                None,
                RPC_C_AUTHN_LEVEL_DEFAULT,
                RPC_C_IMP_LEVEL_IMPERSONATE,
                None,
                EOAC_NONE,
                None,
            )
            .map_err(|e| e.to_string())
        }
    });
    value.clone().map_err(RestorePointError::ComSecurity)
}

pub fn begin_driver_install(plan_id: &str) -> Result<RestorePointEvidence> {
    initialize_process_com_security()?;
    let _com = initialize_current_thread_com()?;
    let description = description_for_plan(plan_id);
    let sequence = call_restore(BEGIN_SYSTEM_CHANGE, DEVICE_DRIVER_INSTALL, 0, &description)?;
    if sequence <= 0 {
        return Err(RestorePointError::Unavailable("System Restore returned an invalid sequence number".into()));
    }

    // Windows 8+ may legally return TRUE and the sequence of a restore point created earlier
    // inside the configured frequency window. Verify this exact sequence carries our unique
    // service-generated description. If verification cannot prove freshness, pair the BEGIN with
    // CANCELLED_OPERATION before refusing the installation.
    match verify_restore_point(sequence, &description) {
        Ok(true) => Ok(RestorePointEvidence { sequence_number: sequence, description, verified_fresh: true }),
        Ok(false) => {
            let _ = call_restore(END_SYSTEM_CHANGE, CANCELLED_OPERATION, sequence, &description);
            Err(RestorePointError::NotFresh)
        }
        Err(error) => {
            let _ = call_restore(END_SYSTEM_CHANGE, CANCELLED_OPERATION, sequence, &description);
            Err(error)
        }
    }
}

pub fn end_driver_install(sequence: i64, description: &str) -> Result<()> {
    initialize_process_com_security()?;
    let _com = initialize_current_thread_com()?;
    let _ = call_restore(END_SYSTEM_CHANGE, DEVICE_DRIVER_INSTALL, sequence, description)?;
    Ok(())
}

pub fn cancel_driver_install(sequence: i64, description: &str) -> Result<()> {
    initialize_process_com_security()?;
    let _com = initialize_current_thread_com()?;
    let _ = call_restore(END_SYSTEM_CHANGE, CANCELLED_OPERATION, sequence, description)?;
    Ok(())
}

fn call_restore(event_type: u32, restore_type: u32, sequence: i64, description: &str) -> Result<i64> {
    let mut info = RestorePointInfoW {
        event_type,
        restore_point_type: restore_type,
        sequence_number: sequence,
        description: [0; MAX_DESC_W],
    };
    for (dst, src) in info.description.iter_mut().zip(description.encode_utf16().take(MAX_DESC_W - 1)) {
        *dst = src;
    }
    let mut status = StateManagerStatus::default();

    let library = unsafe { LoadLibraryW(windows::core::w!("srclient.dll")) }
        .map_err(|e| RestorePointError::Unavailable(e.to_string()))?;
    let result = unsafe {
        let proc = GetProcAddress(library, windows::core::s!("SRSetRestorePointW"));
        let Some(proc) = proc else {
            let _ = FreeLibrary(library);
            return Err(RestorePointError::Unavailable("SRSetRestorePointW export not found".into()));
        };
        let function: SrSetRestorePointW = std::mem::transmute(proc);
        let ok = function(&mut info, &mut status);
        let _ = FreeLibrary(library);
        ok
    };
    if !result.as_bool() {
        return Err(RestorePointError::RestoreStatus(status.status));
    }
    Ok(status.sequence_number)
}

fn verify_restore_point(sequence: i64, description: &str) -> Result<bool> {
    // The description is service-generated and contains only alphanumeric/hyphen characters.
    let query = format!(
        "SELECT * FROM SystemRestore WHERE SequenceNumber = {sequence} AND Description = '{}'",
        description.replace('\'', "''")
    );
    unsafe {
        let locator: IWbemLocator = CoCreateInstance(&WbemLocator, None, CLSCTX_INPROC_SERVER)
            .map_err(|e| RestorePointError::Verification(e.to_string()))?;
        let services = locator
            .ConnectServer(
                &BSTR::from("ROOT\\DEFAULT"),
                &BSTR::new(), &BSTR::new(), &BSTR::new(), 0, &BSTR::new(), None,
            )
            .map_err(|e| RestorePointError::Verification(e.to_string()))?;
        CoSetProxyBlanket(
            &services,
            RPC_C_AUTHN_WINNT,
            RPC_C_AUTHZ_NONE,
            PCWSTR::null(),
            RPC_C_AUTHN_LEVEL_CALL,
            RPC_C_IMP_LEVEL_IMPERSONATE,
            None,
            EOAC_NONE,
        )
        .map_err(|e| RestorePointError::Verification(e.to_string()))?;
        let flags = WBEM_FLAG_FORWARD_ONLY | WBEM_FLAG_RETURN_IMMEDIATELY;
        let enumerator = services
            .ExecQuery(&BSTR::from("WQL"), &BSTR::from(query), flags, None)
            .map_err(|e| RestorePointError::Verification(e.to_string()))?;
        let mut values = [None];
        let mut returned = 0u32;
        // Freshness verification is fail-closed and may never wait indefinitely on WMI.
        let hr = enumerator.Next(5_000, &mut values, &mut returned);
        if hr.0 == WBEM_S_TIMEDOUT.0 {
            return Err(RestorePointError::Verification(
                "restore-point freshness verification timed out after the bounded 5-second WMI slice".into(),
            ));
        }
        if hr.is_err() {
            return Err(RestorePointError::Verification(format!("IEnumWbemClassObject::Next failed: {hr:?}")));
        }
        if returned > 1 {
            return Err(RestorePointError::Verification(
                "restore-point WMI enumerator reported more objects than the supplied output slice".into(),
            ));
        }
        if hr.0 == WBEM_S_FALSE.0 {
            return match (returned, values[0].take()) {
                (0, None) => Ok(false),
                _ => Err(RestorePointError::Verification(
                    "restore-point WMI enumerator returned terminal WBEM_S_FALSE with a non-empty result".into(),
                )),
            };
        }
        match (returned, values[0].take()) {
            (1, Some(_)) => Ok(true),
            (1, None) => Err(RestorePointError::Verification(
                "restore-point WMI enumerator reported an object without supplying one".into(),
            )),
            (0, Some(_)) => Err(RestorePointError::Verification(
                "restore-point WMI enumerator populated an object while reporting zero returned objects".into(),
            )),
            (0, None) => Err(RestorePointError::Verification(
                "restore-point WMI enumerator returned zero objects without terminal WBEM_S_FALSE".into(),
            )),
            _ => Err(RestorePointError::Verification(
                "restore-point WMI enumerator returned an inconsistent result count".into(),
            )),
        }
    }
}
