use sha2::{Digest, Sha256};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrincipalContext {
    pub pid: u32,
    pub image_path: String,
    pub elevated: bool,
    /// Canonical binary SID encoded as lowercase hex. This avoids locale/account-name lookups.
    pub user_sid: String,
    /// TOKEN_STATISTICS.AuthenticationId packed as high/low 32-bit parts.
    pub authentication_id: u64,
    /// Windows Terminal Services session that owns the named-pipe client.
    pub session_id: u32,
    /// The caller's own profile/home directory, as the OS reports it for THIS token.
    ///
    /// Phase 39: the maintenance service runs as LocalSystem, so any request that names
    /// a filesystem target has to be confined to something the CALLER owns. That root
    /// cannot come from the request and cannot be reconstructed later — on Windows it is
    /// read from the impersonated client token while the service still holds it. `None`
    /// means the OS did not answer, which callers must treat as "no scope", never as
    /// "no restriction".
    pub profile_dir: Option<String>,
}

impl PrincipalContext {
    /// Opaque ownership key persisted in the safety ledger. The service derives it from the
    /// kernel-observed client token; callers never supply it over IPC.
    pub fn binding_key(&self) -> String {
        let material = format!(
            "sid={}|auth={:016x}|session={}",
            self.user_sid, self.authentication_id, self.session_id
        );
        hex::encode(Sha256::digest(material.as_bytes()))
    }

    /// Fills in `profile_dir` from `user_sid`. Must be called with the SERVICE's own
    /// authority, never while impersonating the caller.
    pub fn resolve_profile_dir(&mut self) {
        #[cfg(windows)]
        {
            self.profile_dir = hex::decode(&self.user_sid)
                .ok()
                .as_deref()
                .and_then(sid_text_from_bytes)
                .as_deref()
                .and_then(profile_dir_for_sid_text);
        }
    }

    /// Roots this principal may name in a request. Empty when the OS gave no answer —
    /// an empty allowlist refuses everything, which is the correct fail-closed reading.
    pub fn owner_roots(&self) -> Vec<std::path::PathBuf> {
        self.profile_dir
            .iter()
            .filter(|dir| !dir.trim().is_empty())
            .map(std::path::PathBuf::from)
            .collect()
    }

    pub fn same_logon_principal(&self, other: &Self) -> bool {
        self.user_sid == other.user_sid
            && self.authentication_id == other.authentication_id
            && self.session_id == other.session_id
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SecurityError {
    #[error("unsupported platform")]
    Unsupported,
    #[error("windows api error: {0}")]
    Windows(String),
    #[error("invalid access token data: {0}")]
    InvalidToken(&'static str),
}

#[cfg(windows)]
pub fn inspect_named_pipe_client(
    raw_handle: std::os::windows::io::RawHandle,
) -> Result<PrincipalContext, SecurityError> {
    use std::ffi::c_void;

    use aethercore_windows_foundation::{OwnedHandle, ThreadImpersonation};
    use windows::{
        Win32::{
            Foundation::HANDLE,
            Security::TOKEN_QUERY,
            System::{
                Pipes::{GetNamedPipeClientProcessId, GetNamedPipeClientSessionId},
                Threading::{
                    GetCurrentThread, OpenProcess, OpenThreadToken, PROCESS_NAME_WIN32,
                    PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
                },
            },
        },
        core::PWSTR,
    };

    unsafe {
        let pipe = HANDLE(raw_handle as *mut c_void);

        // PID and Terminal Services session are kernel-reported properties of this server-side pipe
        // connection. PID is used only to resolve the executable image; security ownership is read
        // from the impersonated client token below so PID reuse cannot substitute a different SID or
        // logon session into the persisted principal binding.
        let mut pid = 0u32;
        GetNamedPipeClientProcessId(pipe, &mut pid).map_err(winerr)?;
        let mut session_id = 0u32;
        GetNamedPipeClientSessionId(pipe, &mut session_id).map_err(winerr)?;

        let process = OwnedHandle::new(
            OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).map_err(winerr)?,
        );
        let mut buffer = vec![0u16; 32768];
        let mut size = buffer.len() as u32;
        let image = QueryFullProcessImageNameW(
            process.get(),
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut size,
        )
        .map(|()| String::from_utf16_lossy(&buffer[..size as usize]))
        .map_err(winerr)?;

        // Bind ownership to the effective security token of the exact named-pipe client, not to a
        // token reopened through a process ID. RAII provides a fallback revert on every early return,
        // while the explicit revert below still makes a failed RevertToSelf security-fatal.
        let impersonation = ThreadImpersonation::named_pipe_client(pipe).map_err(winerr)?;
        let result = (|| {
            let mut raw_token = HANDLE::default();
            OpenThreadToken(GetCurrentThread(), TOKEN_QUERY, true, &mut raw_token)
                .map_err(winerr)?;
            let token = OwnedHandle::new(raw_token);
            principal_from_token(token.get(), pid, image, session_id)
        })();

        // Reverting the thread security context is mandatory even when token inspection failed.
        // A RevertToSelf failure is more security-significant than the original inspection error
        // because returning while still impersonating would contaminate later service work.
        impersonation.revert().map_err(winerr)?;
        // Only now, with the service's own authority restored, resolve the caller's own
        // audit root. Doing it here rather than inside `principal_from_token` keeps the
        // lookup out of the impersonated window entirely.
        result.map(|mut principal| {
            principal.resolve_profile_dir();
            principal
        })
    }
}

#[cfg(windows)]
pub fn inspect_session_token(
    token: windows::Win32::Foundation::HANDLE,
    session_id: u32,
) -> Result<PrincipalContext, SecurityError> {
    principal_from_token(token, 0, "active-console-session".into(), session_id).map(
        |mut principal| {
            principal.resolve_profile_dir();
            principal
        },
    )
}

#[cfg(windows)]
fn principal_from_token(
    token: windows::Win32::Foundation::HANDLE,
    pid: u32,
    image_path: String,
    session_id: u32,
) -> Result<PrincipalContext, SecurityError> {
    use windows::Win32::Security::{
        GetLengthSid, GetTokenInformation, TOKEN_ELEVATION, TOKEN_STATISTICS, TOKEN_USER,
        TokenElevation, TokenStatistics, TokenUser,
    };

    unsafe {
        let mut elevation = TOKEN_ELEVATION::default();
        let mut returned = 0u32;
        GetTokenInformation(
            token,
            TokenElevation,
            Some((&mut elevation as *mut TOKEN_ELEVATION).cast()),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut returned,
        )
        .map_err(winerr)?;

        let mut statistics = TOKEN_STATISTICS::default();
        GetTokenInformation(
            token,
            TokenStatistics,
            Some((&mut statistics as *mut TOKEN_STATISTICS).cast()),
            std::mem::size_of::<TOKEN_STATISTICS>() as u32,
            &mut returned,
        )
        .map_err(winerr)?;

        // TOKEN_USER is variable-sized because the SID storage follows the structure.
        let mut needed = 0u32;
        let _ = GetTokenInformation(token, TokenUser, None, 0, &mut needed);
        if needed < std::mem::size_of::<TOKEN_USER>() as u32 || needed > 64 * 1024 {
            return Err(SecurityError::InvalidToken("TokenUser size"));
        }
        // Use pointer-sized storage rather than Vec<u8> so TOKEN_USER is correctly aligned on both
        // x86 and x64. The SID pointer returned by Windows must point back inside this owned buffer.
        let word = std::mem::size_of::<usize>();
        let words = (needed as usize).div_ceil(word);
        let mut user_buffer = vec![0usize; words];
        GetTokenInformation(
            token,
            TokenUser,
            Some(user_buffer.as_mut_ptr().cast()),
            needed,
            &mut returned,
        )
        .map_err(winerr)?;
        let user = &*(user_buffer.as_ptr().cast::<TOKEN_USER>());
        if user.User.Sid.is_invalid() {
            return Err(SecurityError::InvalidToken("missing user SID"));
        }
        let buffer_start = user_buffer.as_ptr() as usize;
        let buffer_end = buffer_start
            .checked_add(user_buffer.len().saturating_mul(word))
            .ok_or(SecurityError::InvalidToken("TokenUser buffer range"))?;
        let sid_start = user.User.Sid.0 as usize;
        if sid_start < buffer_start
            || sid_start
                .checked_add(8)
                .is_none_or(|minimum_end| minimum_end > buffer_end)
        {
            return Err(SecurityError::InvalidToken(
                "user SID pointer outside TokenUser buffer",
            ));
        }
        let sid_len = GetLengthSid(user.User.Sid) as usize;
        let sid_end = sid_start
            .checked_add(sid_len)
            .ok_or(SecurityError::InvalidToken("user SID range"))?;
        if sid_len == 0 || sid_end > buffer_end {
            return Err(SecurityError::InvalidToken("invalid user SID length"));
        }
        let sid_bytes = std::slice::from_raw_parts(user.User.Sid.0.cast::<u8>(), sid_len);

        let high = statistics.AuthenticationId.HighPart as i64 as u64;
        let authentication_id = (high << 32) | statistics.AuthenticationId.LowPart as u64;

        Ok(PrincipalContext {
            pid,
            image_path,
            elevated: elevation.TokenIsElevated != 0,
            user_sid: hex::encode(sid_bytes),
            authentication_id,
            session_id,
            // Deliberately NOT resolved here: this runs while the thread is still
            // impersonating the client. `inspect_named_pipe_client` fills it in after
            // RevertToSelf, so the lookup happens with the service's own authority.
            profile_dir: None,
        })
    }
}

/// Canonical `S-R-A-S1-S2-...` text for a binary SID.
///
/// Pure arithmetic on the documented layout — revision, sub-authority count, a 6-byte
/// big-endian identifier authority, then little-endian 32-bit sub-authorities. Written by
/// hand rather than through `ConvertSidToStringSidW` so it is testable on any host and so
/// the caller needs no Win32 call at all.
fn sid_text_from_bytes(sid: &[u8]) -> Option<String> {
    if sid.len() < 8 || sid[0] != 1 {
        return None;
    }
    let sub_count = sid[1] as usize;
    if sub_count > 15 || sid.len() < 8 + sub_count * 4 {
        return None;
    }
    let authority = sid[2..8]
        .iter()
        .fold(0u64, |acc, byte| (acc << 8) | u64::from(*byte));
    let mut text = if authority < (1u64 << 32) {
        format!("S-1-{authority}")
    } else {
        format!("S-1-0x{authority:012x}")
    };
    for index in 0..sub_count {
        let offset = 8 + index * 4;
        let value = u32::from_le_bytes([
            sid[offset],
            sid[offset + 1],
            sid[offset + 2],
            sid[offset + 3],
        ]);
        use std::fmt::Write as _;
        let _ = write!(text, "-{value}");
    }
    Some(text)
}

/// The profile directory Windows records for the user named by `sid_text`.
///
/// `ProfileList` is where Windows itself keeps the mapping, so this is the OS's answer
/// rather than a reconstruction. It deliberately does not go through `%USERPROFILE%`
/// (inherited and caller-influenced), and it does not go through
/// `SHGetKnownFolderPath(FOLDERID_Profile, token)`, which was tried first and MEASURED on
/// the qualification VM to return nothing for every principal: the service opens the
/// impersonated client token with `TOKEN_QUERY` only, and that API also wants
/// `TOKEN_IMPERSONATE`. Rather than widen the rights on the token handle the principal
/// binding already depends on, this reads a mapping that needs no token rights at all.
/// `HKLM\SOFTWARE` is writable only by administrators, so an unprivileged caller cannot
/// redirect its own scope by writing here.
///
/// Any failure yields `None`, which the audit allowlist reads as an EMPTY scope — every
/// path-bearing target is then refused. Fail-closed, never fail-open.
#[cfg(windows)]
fn profile_dir_for_sid_text(sid_text: &str) -> Option<String> {
    use std::ffi::c_void;
    use windows::{
        Win32::System::Registry::{HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RegGetValueW},
        core::PCWSTR,
    };

    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }

    if sid_text.is_empty() || sid_text.len() > 256 {
        return None;
    }
    let key = wide(&format!(
        r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\ProfileList\{sid_text}"
    ));
    let name = wide("ProfileImagePath");
    // RegGetValueW expands REG_EXPAND_SZ into REG_SZ unless asked not to, so the single
    // RRF_RT_REG_SZ restriction is the right one. Measured on the VM: it returns the
    // expanded path.
    let mut bytes = 0u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            PCWSTR(key.as_ptr()),
            PCWSTR(name.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            None,
            Some(&mut bytes),
        )
    };
    if status.is_err() || bytes < 2 || bytes > 4096 {
        return None;
    }
    let mut buffer = vec![0u16; (bytes as usize).div_ceil(2)];
    let status = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            PCWSTR(key.as_ptr()),
            PCWSTR(name.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            Some(buffer.as_mut_ptr().cast::<c_void>()),
            Some(&mut bytes),
        )
    };
    if status.is_err() {
        return None;
    }
    let end = buffer
        .iter()
        .position(|value| *value == 0)
        .unwrap_or(buffer.len());
    let text = String::from_utf16_lossy(&buffer[..end]);
    (!text.is_empty()).then_some(text)
}

#[cfg(windows)]
pub fn verify_maintenance_service_token(service_name: &str) -> Result<(), SecurityError> {
    use std::ffi::c_void;

    use aethercore_windows_foundation::OwnedHandle;
    use windows::{
        Win32::{
            Foundation::{ERROR_INSUFFICIENT_BUFFER, HANDLE},
            Security::{
                CheckTokenMembership, GetTokenInformation, IsValidSid, LookupAccountNameW,
                PSID, SID_NAME_USE, TOKEN_QUERY, TokenRestrictedSids,
            },
            System::Threading::{GetCurrentProcess, OpenProcessToken},
        },
        core::{BOOL, PCWSTR, PWSTR},
    };

    const MAX_ACCOUNT_SID_BYTES: u32 = 4 * 1024;
    const MAX_ACCOUNT_DOMAIN_CHARS: u32 = 32 * 1024;
    const MAX_TOKEN_GROUP_BUFFER_BYTES: u32 = 1024 * 1024;

    unsafe {
        let account_name = format!(r"NT SERVICE\{service_name}");
        let account: Vec<u16> = account_name.encode_utf16().chain(Some(0)).collect();
        let mut sid_bytes = 0u32;
        let mut domain_chars = 0u32;
        let mut sid_use = SID_NAME_USE::default();
        match LookupAccountNameW(
            PCWSTR::null(),
            PCWSTR(account.as_ptr()),
            None,
            &mut sid_bytes,
            None,
            &mut domain_chars,
            &mut sid_use,
        ) {
            Err(error) if error.code() == ERROR_INSUFFICIENT_BUFFER.to_hresult() => {}
            Err(error) => return Err(winerr(error)),
            Ok(()) => {
                return Err(SecurityError::InvalidToken(
                    "service SID lookup unexpectedly needed no buffer",
                ));
            }
        }
        if sid_bytes == 0 || sid_bytes > MAX_ACCOUNT_SID_BYTES {
            return Err(SecurityError::InvalidToken(
                "service SID size outside safety bound",
            ));
        }
        if domain_chars > MAX_ACCOUNT_DOMAIN_CHARS {
            return Err(SecurityError::InvalidToken(
                "service SID domain size outside safety bound",
            ));
        }

        let sid_words = usize::try_from(sid_bytes)
            .ok()
            .and_then(|bytes| bytes.checked_add(std::mem::size_of::<u64>() - 1))
            .map(|bytes| bytes / std::mem::size_of::<u64>())
            .ok_or(SecurityError::InvalidToken(
                "service SID allocation overflow",
            ))?;
        let mut sid = vec![0u64; sid_words];
        let sid_capacity = sid
            .len()
            .checked_mul(std::mem::size_of::<u64>())
            .ok_or(SecurityError::InvalidToken("service SID capacity overflow"))?;
        let mut domain = vec![0u16; domain_chars as usize];
        let domain_buffer = if domain.is_empty() {
            None
        } else {
            Some(PWSTR(domain.as_mut_ptr()))
        };
        let sid_ptr = PSID(sid.as_mut_ptr().cast::<c_void>());
        LookupAccountNameW(
            PCWSTR::null(),
            PCWSTR(account.as_ptr()),
            Some(sid_ptr),
            &mut sid_bytes,
            domain_buffer,
            &mut domain_chars,
            &mut sid_use,
        )
        .map_err(winerr)?;
        if sid_bytes == 0 || usize::try_from(sid_bytes).map_or(true, |bytes| bytes > sid_capacity) {
            return Err(SecurityError::InvalidToken(
                "service SID length exceeded allocated buffer",
            ));
        }
        if !IsValidSid(sid_ptr).as_bool() {
            return Err(SecurityError::InvalidToken(
                "service SID lookup returned invalid SID",
            ));
        }

        // CheckTokenMembership proves the service SID is not merely configured/present: it must be
        // enabled in the effective service token and therefore usable by normal access checks.
        let mut is_member = BOOL::default();
        CheckTokenMembership(None, sid_ptr, &mut is_member).map_err(winerr)?;
        if !is_member.as_bool() {
            return Err(SecurityError::InvalidToken(
                "service SID is not enabled in effective token",
            ));
        }

        let mut raw_token = HANDLE::default();
        OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut raw_token).map_err(winerr)?;
        let token = OwnedHandle::new(raw_token);
        let mut needed = 0u32;
        let _ = GetTokenInformation(token.get(), TokenRestrictedSids, None, 0, &mut needed);
        if needed < std::mem::size_of::<u32>() as u32 || needed > MAX_TOKEN_GROUP_BUFFER_BYTES {
            return Err(SecurityError::InvalidToken(
                "restricted SID list size outside safety bound",
            ));
        }
        let word = std::mem::size_of::<usize>();
        let words = (needed as usize).div_ceil(word);
        let mut restricted = vec![0usize; words];
        let mut returned = 0u32;
        GetTokenInformation(
            token.get(),
            TokenRestrictedSids,
            Some(restricted.as_mut_ptr().cast()),
            needed,
            &mut returned,
        )
        .map_err(winerr)?;
        if returned < std::mem::size_of::<u32>() as u32 || returned > needed {
            return Err(SecurityError::InvalidToken(
                "restricted SID list length changed outside buffer",
            ));
        }
        let restricted_count = *(restricted.as_ptr().cast::<u32>());
        if restricted_count != 0 {
            return Err(SecurityError::InvalidToken(
                "maintenance service token contains restricting SIDs",
            ));
        }
        Ok(())
    }
}

#[cfg(windows)]
fn winerr(e: windows::core::Error) -> SecurityError {
    SecurityError::Windows(e.to_string())
}

#[cfg(not(windows))]
pub fn inspect_named_pipe_client(
    _: *mut std::ffi::c_void,
) -> Result<PrincipalContext, SecurityError> {
    Err(SecurityError::Unsupported)
}

/// Phase 27 (CX-4/QD-026-001) — unix socket principal binding.
///
/// Binds the connecting peer to the socket-owning user via the documented permission
/// boundary: the maintenance socket lives in a 0700 directory with a 0600 socket file,
/// so only the owning OS user can connect. The peer principal is therefore the service's
/// own uid, expressed through the same `binding_key()` derivation the Windows host uses
/// (a stable SHA-256 of the identity material). This deliberately does NOT claim deeper
/// credential verification (SO_PEERCRED is absent on macOS); deepening stays recorded in
/// QD-026-001.
#[cfg(unix)]
pub fn socket_owner_principal() -> PrincipalContext {
    let uid = unsafe { libc_getuid() };
    let pid = std::process::id();
    // Identity material mirrors the Windows shape: a stable per-user string plus stable
    // session context. On unix the logon/session dimension collapses to the uid itself.
    let identity = format!("unix:uid={uid}");
    let image_path = std::env::current_exe()
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "unknown".to_string());
    let mut hasher = Sha256::new();
    hasher.update(identity.as_bytes());
    let digest = hasher.finalize();
    let mut hex_sid = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(hex_sid, "{byte:02x}");
    }
    // Truncate to a 32-hex canonical form so the encoded SID stays compact and stable.
    hex_sid.truncate(32);
    PrincipalContext {
        pid,
        image_path,
        elevated: false,
        user_sid: hex_sid,
        authentication_id: u64::from(uid),
        session_id: 0,
        // The 0700/0600 rendezvous means the peer IS the socket-owning user, i.e. this
        // process's own uid, so this process's HOME is that user's home. The unix
        // composition claims nothing deeper than the permission boundary it already
        // documents (SO_PEERCRED deepening stays in QD-026-001).
        profile_dir: std::env::var_os("HOME")
            .map(|home| home.to_string_lossy().into_owned())
            .filter(|home| !home.is_empty()),
    }
}

#[cfg(unix)]
fn libc_getuid() -> u32 {
    unsafe extern "C" {
        fn getuid() -> u32;
    }
    // SAFETY: getuid(2) is unconditionally memory-safe.
    unsafe { getuid() }
}

pub fn is_expected_broker(peer: &PrincipalContext, expected_path: &std::path::Path) -> bool {
    // Absolute-ness is judged on Windows path semantics (drive letter or UNC root), not on the
    // host parser, so the guard behaves identically when audited on a non-Windows host.
    let text = expected_path.to_string_lossy();
    let bytes = text.as_bytes();
    let looks_absolute_windows =
        (bytes.len() >= 3 && bytes[1] == b':' && (bytes[2] == b'\\' || bytes[2] == b'/'))
            || text.starts_with(r"\\");
    // Accept either Windows-absolute (drive/UNC) or host-absolute expected paths so the identical
    // validation logic is exercisable when audited on a non-Windows host.
    if !peer.elevated || !(looks_absolute_windows || expected_path.is_absolute()) {
        return false;
    }

    let expected = normalize_windows_path(&expected_path.to_string_lossy());
    let actual = normalize_windows_path(&peer.image_path);
    actual == expected
}

fn normalize_windows_path(value: &str) -> String {
    // Strip both the `\\?\` extended-length prefix and the `\\?\UNC\` server form before
    // canonicalizing separators and case, so an extended-prefix peer image still compares
    // equal to its plain installed path.
    let normalized = value.replace('/', "\\");
    let without_extended_prefix = normalized
        .strip_prefix(r"\\?\UNC\")
        .map(|rest| format!(r"\\{rest}"))
        .unwrap_or_else(|| {
            normalized
                .strip_prefix(r"\\?\")
                .unwrap_or(&normalized)
                .to_string()
        });
    without_extended_prefix.to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn principal(path: &str, elevated: bool, auth: u64, session: u32) -> PrincipalContext {
        PrincipalContext {
            pid: 42,
            image_path: path.into(),
            elevated,
            user_sid: "01050000000000051500000001020304".into(),
            authentication_id: auth,
            session_id: session,
            profile_dir: None,
        }
    }

    #[test]
    fn principal_binding_is_logon_and_session_scoped() {
        let a = principal(r"C:\A.exe", false, 10, 1);
        let same = principal(r"C:\B.exe", true, 10, 1);
        let other_session = principal(r"C:\B.exe", true, 10, 2);
        let other_logon = principal(r"C:\B.exe", true, 11, 1);
        let mut other_user = principal(r"C:\B.exe", true, 10, 1);
        other_user.user_sid = "010500000000000515000000ffffffff".into();
        assert_eq!(a.binding_key(), same.binding_key());
        assert_ne!(a.binding_key(), other_session.binding_key());
        assert_ne!(a.binding_key(), other_logon.binding_key());
        assert_ne!(a.binding_key(), other_user.binding_key());
        assert!(a.same_logon_principal(&same));
        assert!(!a.same_logon_principal(&other_session));
        assert!(!a.same_logon_principal(&other_user));
    }

    #[test]
    fn an_unresolved_profile_dir_yields_no_owner_roots_rather_than_no_restriction() {
        let mut peer = principal(r"C:\A.exe", false, 10, 1);
        assert!(peer.owner_roots().is_empty(), "None means no scope");
        peer.profile_dir = Some("   ".into());
        assert!(peer.owner_roots().is_empty(), "blank means no scope");
        peer.profile_dir = Some(r"C:\Users\alice".into());
        assert_eq!(
            peer.owner_roots(),
            vec![std::path::PathBuf::from(r"C:\Users\alice")]
        );
    }

    #[test]
    fn sid_text_matches_the_canonical_string_form() {
        // S-1-5-18 (LocalSystem): revision 1, one sub-authority, authority 5.
        let local_system = [1u8, 1, 0, 0, 0, 0, 0, 5, 18, 0, 0, 0];
        assert_eq!(sid_text_from_bytes(&local_system).as_deref(), Some("S-1-5-18"));

        // A real machine account SID, the shape the audit allowlist actually keys on:
        // S-1-5-21-3596463104-2050256853-579393690-1003.
        let mut user = vec![1u8, 5, 0, 0, 0, 0, 0, 5];
        for value in [21u32, 3596463104, 2050256853, 579393690, 1003] {
            user.extend_from_slice(&value.to_le_bytes());
        }
        assert_eq!(
            sid_text_from_bytes(&user).as_deref(),
            Some("S-1-5-21-3596463104-2050256853-579393690-1003")
        );

        // Malformed input is refused rather than guessed at: wrong revision, truncated
        // sub-authority array, and an impossible sub-authority count.
        assert_eq!(sid_text_from_bytes(&[2u8, 1, 0, 0, 0, 0, 0, 5, 18, 0, 0, 0]), None);
        assert_eq!(sid_text_from_bytes(&[1u8, 1, 0, 0, 0, 0, 0, 5, 18]), None);
        assert_eq!(sid_text_from_bytes(&[1u8, 99, 0, 0, 0, 0, 0, 5]), None);
        assert_eq!(sid_text_from_bytes(&[]), None);
    }

    #[test]
    fn broker_validation_requires_elevation_and_exact_path() {
        let expected_buf = std::env::temp_dir()
            .join("AetherCore")
            .join("aethercore-consent-broker.exe");
        let expected = expected_buf.as_path();
        let good = principal(&expected.display().to_string(), true, 1, 1);
        assert!(is_expected_broker(&good, expected));

        let not_elevated = PrincipalContext {
            elevated: false,
            ..good.clone()
        };
        assert!(!is_expected_broker(&not_elevated, expected));

        let renamed = principal(
            &std::env::temp_dir()
                .join("Other")
                .join("aethercore-consent-broker.exe")
                .display()
                .to_string(),
            true,
            1,
            1,
        );
        assert!(!is_expected_broker(&renamed, expected));
    }

    #[test]
    fn broker_validation_accepts_extended_path_prefix() {
        let expected =
            std::path::Path::new(r"C:\Program Files\AetherCore\aethercore-consent-broker.exe");
        let peer = principal(
            r"\\?\C:\Program Files\AetherCore\aethercore-consent-broker.exe",
            true,
            1,
            1,
        );
        assert!(is_expected_broker(&peer, expected));
    }

    #[test]
    fn broker_validation_rejects_common_prefix_and_relative_expected_path() {
        let expected =
            std::path::Path::new(r"C:\Program Files\AetherCore\aethercore-consent-broker.exe");
        let prefix_attack = principal(
            r"C:\Program Files\AetherCoreEvil\aethercore-consent-broker.exe",
            true,
            1,
            1,
        );
        assert!(!is_expected_broker(&prefix_attack, expected));
        let relative = std::path::Path::new(r"AetherCore\aethercore-consent-broker.exe");
        assert!(!is_expected_broker(&prefix_attack, relative));
    }
}
