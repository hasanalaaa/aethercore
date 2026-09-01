//! Phase 39 — diagnostic: WHY did the owner-root lookup return nothing?
//!
//! `sec.ownerScopeUnresolved` on the installed service says `profile_dir` came back
//! `None`, but not which step failed. This measures each step separately instead of
//! guessing, because every guess costs a ~12 minute MSI rebuild.
//!
//! Throwaway diagnostic, not part of the product. Delete once the cause is closed.

#[cfg(not(windows))]
fn main() {
    eprintln!("windows only");
    std::process::exit(2);
}

#[cfg(windows)]
fn main() {
    use std::ffi::c_void;
    use windows::{
        Win32::{
            Foundation::{HANDLE, HLOCAL, LocalFree},
            Security::{
                Authorization::ConvertSidToStringSidW, GetTokenInformation, TOKEN_QUERY,
                TOKEN_USER, TokenUser,
            },
            System::{
                Registry::{
                    HKEY_LOCAL_MACHINE, RRF_NOEXPAND, RRF_RT_REG_EXPAND_SZ, RRF_RT_REG_SZ,
                    RegGetValueW,
                },
                Threading::{GetCurrentProcess, OpenProcessToken},
            },
        },
        core::{PCWSTR, PWSTR},
    };

    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }

    unsafe {
        let mut raw_token = HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut raw_token).is_err() {
            println!("OPEN_TOKEN=err");
            return;
        }
        let mut needed = 0u32;
        let _ = GetTokenInformation(raw_token, TokenUser, None, 0, &mut needed);
        let words = (needed as usize).div_ceil(std::mem::size_of::<usize>());
        let mut buffer = vec![0usize; words];
        if GetTokenInformation(
            raw_token,
            TokenUser,
            Some(buffer.as_mut_ptr().cast()),
            needed,
            &mut needed,
        )
        .is_err()
        {
            println!("TOKEN_USER=err");
            return;
        }
        let user = &*(buffer.as_ptr().cast::<TOKEN_USER>());

        let mut sid_raw = PWSTR::null();
        match ConvertSidToStringSidW(user.User.Sid, &mut sid_raw) {
            Ok(()) => {}
            Err(error) => {
                println!("CONVERT_SID=err {error}");
                return;
            }
        }
        let own = PCWSTR(sid_raw.0).to_string().unwrap_or_default();
        let _ = LocalFree(Some(HLOCAL(sid_raw.0.cast())));
        // Optional argument: look up a DIFFERENT principal's SID, which is what the
        // service does — it runs as LocalSystem and resolves the CALLER's root.
        let sid_text = std::env::args().nth(1).unwrap_or_else(|| own.clone());
        println!("OWN_SID={own}");
        println!("SID={sid_text}");

        let key = wide(&format!(
            r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\ProfileList\{sid_text}"
        ));
        let name = wide("ProfileImagePath");

        for (label, flags) in [
            ("SZ_ONLY", RRF_RT_REG_SZ),
            ("SZ_OR_EXPAND", RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ),
            ("EXPAND_NOEXPAND", RRF_RT_REG_EXPAND_SZ | RRF_NOEXPAND),
        ] {
            let mut bytes = 0u32;
            let status = RegGetValueW(
                HKEY_LOCAL_MACHINE,
                PCWSTR(key.as_ptr()),
                PCWSTR(name.as_ptr()),
                flags,
                None,
                None,
                Some(&mut bytes),
            );
            if status.is_err() {
                println!("{label}=SIZE_ERR {:?} bytes={bytes}", status.0);
                continue;
            }
            let mut out = vec![0u16; (bytes as usize).div_ceil(2)];
            let status = RegGetValueW(
                HKEY_LOCAL_MACHINE,
                PCWSTR(key.as_ptr()),
                PCWSTR(name.as_ptr()),
                flags,
                None,
                Some(out.as_mut_ptr().cast::<c_void>()),
                Some(&mut bytes),
            );
            if status.is_err() {
                println!("{label}=READ_ERR {:?}", status.0);
                continue;
            }
            let end = out.iter().position(|v| *v == 0).unwrap_or(out.len());
            let value = String::from_utf16_lossy(&out[..end]);
            println!("{label}=OK {value}");
            // The other half of the question: OwnerScope::new drops any root that does
            // not canonicalise, so measure that too rather than assuming it succeeds.
            match std::fs::canonicalize(&value) {
                Ok(path) => println!("{label}_CANONICAL=OK {}", path.display()),
                Err(error) => println!("{label}_CANONICAL=ERR {error}"),
            }
        }
    }
}
