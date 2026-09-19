use std::{path::Path, sync::Arc};

use serde::{Deserialize, Serialize};

use crate::coordinator::UpdateEngineError;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SignatureVerification {
    pub valid: bool,
    pub signer_subject: String,
    pub signer_thumbprint_or_identity: String,
    pub chain_status: String,
    pub test_signed: bool,
}

impl SignatureVerification {
    pub fn validity_only() -> Self {
        Self {
            valid: true,
            signer_subject: String::new(),
            signer_thumbprint_or_identity: String::new(),
            chain_status: "ValidIdentityNotExtracted".into(),
            test_signed: false,
        }
    }
}

pub trait PlatformVerifier: Send + Sync + 'static {
    fn verify_authenticode(&self, path: &Path) -> Result<SignatureVerification, UpdateEngineError>;
}

pub fn default_platform_verifier() -> Arc<dyn PlatformVerifier> {
    #[cfg(windows)]
    {
        Arc::new(WindowsAuthenticodeVerifier)
    }
    #[cfg(not(windows))]
    {
        Arc::new(UnsupportedVerifier)
    }
}

pub fn current_windows_build() -> Result<u32, UpdateEngineError> {
    #[cfg(windows)]
    {
        current_windows_build_impl()
    }
    #[cfg(not(windows))]
    {
        Err(UpdateEngineError::UnsupportedPlatform)
    }
}

#[cfg(windows)]
#[repr(C)]
struct RtlOsVersionInfoW {
    size: u32,
    major: u32,
    minor: u32,
    build: u32,
    platform: u32,
    service_pack: [u16; 128],
}
#[cfg(windows)]
#[link(name = "ntdll")]
unsafe extern "system" {
    fn RtlGetVersion(info: *mut RtlOsVersionInfoW) -> i32;
}
#[cfg(windows)]
fn current_windows_build_impl() -> Result<u32, UpdateEngineError> {
    let mut info = RtlOsVersionInfoW {
        size: std::mem::size_of::<RtlOsVersionInfoW>() as u32,
        major: 0,
        minor: 0,
        build: 0,
        platform: 0,
        service_pack: [0; 128],
    };
    let status = unsafe { RtlGetVersion(&mut info) };
    if status < 0 || info.build == 0 {
        return Err(UpdateEngineError::UnsupportedPlatform);
    }
    Ok(info.build)
}

#[cfg(not(windows))]
struct UnsupportedVerifier;
#[cfg(not(windows))]
impl PlatformVerifier for UnsupportedVerifier {
    fn verify_authenticode(&self, _: &Path) -> Result<SignatureVerification, UpdateEngineError> {
        Err(UpdateEngineError::UnsupportedPlatform)
    }
}

#[cfg(windows)]
struct WindowsAuthenticodeVerifier;

#[cfg(windows)]
impl PlatformVerifier for WindowsAuthenticodeVerifier {
    fn verify_authenticode(&self, path: &Path) -> Result<SignatureVerification, UpdateEngineError> {
        verify_authenticode_wintrust(path)
    }
}

#[cfg(windows)]
#[repr(C)]
struct WinTrustFileInfo {
    cb_struct: u32,
    pcwsz_file_path: *const u16,
    h_file: *mut core::ffi::c_void,
    pg_known_subject: *const Guid,
}

#[cfg(windows)]
#[repr(C)]
struct WinTrustData {
    cb_struct: u32,
    p_policy_callback_data: *mut core::ffi::c_void,
    p_sip_client_data: *mut core::ffi::c_void,
    dw_ui_choice: u32,
    fdw_revocation_checks: u32,
    dw_union_choice: u32,
    p_file: *mut WinTrustFileInfo,
    dw_state_action: u32,
    h_wvt_state_data: *mut core::ffi::c_void,
    pwsz_url_reference: *mut u16,
    dw_prov_flags: u32,
    dw_ui_context: u32,
    p_signature_settings: *mut core::ffi::c_void,
}

#[cfg(windows)]
#[repr(C)]
#[derive(Clone, Copy)]
struct Guid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

#[cfg(windows)]
const WINTRUST_ACTION_GENERIC_VERIFY_V2: Guid = Guid {
    data1: 0x00AAC56B,
    data2: 0xCD44,
    data3: 0x11D0,
    data4: [0x8C, 0xC2, 0x00, 0xC0, 0x4F, 0xC2, 0x95, 0xEE],
};
#[cfg(windows)]
const WTD_UI_NONE: u32 = 2;
#[cfg(windows)]
const WTD_REVOKE_WHOLECHAIN: u32 = 1;
#[cfg(windows)]
const WTD_CHOICE_FILE: u32 = 1;
#[cfg(windows)]
const WTD_STATEACTION_VERIFY: u32 = 1;
#[cfg(windows)]
const WTD_STATEACTION_CLOSE: u32 = 2;
#[cfg(windows)]
const WTD_REVOCATION_CHECK_CHAIN_EXCLUDE_ROOT: u32 = 0x80;
#[cfg(windows)]
const WTD_CACHE_ONLY_URL_RETRIEVAL: u32 = 0x1000;

#[cfg(windows)]
#[link(name = "wintrust")]
unsafe extern "system" {
    fn WinVerifyTrust(
        hwnd: *mut core::ffi::c_void,
        action: *const Guid,
        data: *mut core::ffi::c_void,
    ) -> i32;
}

#[cfg(windows)]
fn verify_authenticode_wintrust(path: &Path) -> Result<SignatureVerification, UpdateEngineError> {
    use std::os::windows::ffi::OsStrExt;
    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let mut file = WinTrustFileInfo {
        cb_struct: std::mem::size_of::<WinTrustFileInfo>() as u32,
        pcwsz_file_path: wide.as_ptr(),
        h_file: std::ptr::null_mut(),
        pg_known_subject: std::ptr::null(),
    };
    let mut data = WinTrustData {
        cb_struct: std::mem::size_of::<WinTrustData>() as u32,
        p_policy_callback_data: std::ptr::null_mut(),
        p_sip_client_data: std::ptr::null_mut(),
        dw_ui_choice: WTD_UI_NONE,
        fdw_revocation_checks: WTD_REVOKE_WHOLECHAIN,
        dw_union_choice: WTD_CHOICE_FILE,
        p_file: &mut file,
        dw_state_action: WTD_STATEACTION_VERIFY,
        h_wvt_state_data: std::ptr::null_mut(),
        pwsz_url_reference: std::ptr::null_mut(),
        // Chain revocation is required; cache-only URL retrieval avoids an unbounded secondary network path here.
        // HTTPS + signed update-manifest verification is the primary online trust path.
        dw_prov_flags: WTD_REVOCATION_CHECK_CHAIN_EXCLUDE_ROOT | WTD_CACHE_ONLY_URL_RETRIEVAL,
        dw_ui_context: 0,
        p_signature_settings: std::ptr::null_mut(),
    };
    let status = unsafe {
        WinVerifyTrust(
            std::ptr::null_mut(),
            &WINTRUST_ACTION_GENERIC_VERIFY_V2,
            (&mut data as *mut WinTrustData).cast(),
        )
    };
    data.dw_state_action = WTD_STATEACTION_CLOSE;
    let _ = unsafe {
        WinVerifyTrust(
            std::ptr::null_mut(),
            &WINTRUST_ACTION_GENERIC_VERIFY_V2,
            (&mut data as *mut WinTrustData).cast(),
        )
    };
    if status == 0 {
        Ok(SignatureVerification::validity_only())
    } else {
        Err(UpdateEngineError::Authenticode(status))
    }
}
