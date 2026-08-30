#![forbid(unsafe_op_in_unsafe_fn)]

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MachineIdentity {
    pub manufacturer: String,
    pub product_model: String,
    pub product_family: String,
    pub board_product: String,
    pub windows_edition: String,
    pub windows_build: String,
    pub os_display_version: String,
}

#[cfg(windows)]
mod windows_impl {
    use std::path::PathBuf;
    use windows::{
        core::{HRESULT, PCWSTR},
        Win32::{
            Foundation::{CloseHandle, ERROR_LOCK_VIOLATION, ERROR_SUCCESS, HANDLE, INVALID_HANDLE_VALUE},
            Security::{ImpersonateLoggedOnUser, RevertToSelf},
            Storage::FileSystem::{
                CreateFileW, LockFileEx, UnlockFileEx, FILE_ATTRIBUTE_NORMAL,
                FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
                LOCKFILE_EXCLUSIVE_LOCK, LOCKFILE_FAIL_IMMEDIATELY, OPEN_EXISTING,
            },
            System::{
                Com::{CoInitializeEx, CoTaskMemFree, CoUninitialize, COINIT_MULTITHREADED},
                IO::OVERLAPPED,
                Pipes::ImpersonateNamedPipeClient,
                Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ},
                Services::{CloseServiceHandle, SC_HANDLE},
                Threading::{
                    GetCurrentThread, SetThreadPriority, THREAD_MODE_BACKGROUND_BEGIN,
                    THREAD_MODE_BACKGROUND_END,
                },
            },
            UI::Shell::{FOLDERID_ProgramData, KF_FLAG_DEFAULT, SHGetKnownFolderPath},
        },
    };

    fn wide_text(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn read_hklm_string(subkey: &str, value_name: &str) -> String {
        let subkey = wide_text(subkey);
        let value_name = wide_text(value_name);
        let mut bytes = 0u32;
        let status = unsafe {
            RegGetValueW(
                HKEY_LOCAL_MACHINE,
                PCWSTR(subkey.as_ptr()),
                PCWSTR(value_name.as_ptr()),
                RRF_RT_REG_SZ,
                None,
                None,
                Some(&mut bytes),
            )
        };
        if status != ERROR_SUCCESS || bytes < 2 || bytes > 64 * 1024 { return String::new(); }
        let mut buffer = vec![0u16; ((bytes as usize) + 1) / 2];
        let status = unsafe {
            RegGetValueW(
                HKEY_LOCAL_MACHINE,
                PCWSTR(subkey.as_ptr()),
                PCWSTR(value_name.as_ptr()),
                RRF_RT_REG_SZ,
                None,
                Some(buffer.as_mut_ptr().cast()),
                Some(&mut bytes),
            )
        };
        if status != ERROR_SUCCESS { return String::new(); }
        let len = buffer.iter().position(|ch| *ch == 0).unwrap_or(buffer.len());
        String::from_utf16_lossy(&buffer[..len]).trim().to_owned()
    }

    /// Reads privacy-minimized machine/OEM context from documented HKLM system metadata.
    /// Serial numbers, UUIDs and asset tags are intentionally never queried.
    pub fn machine_identity() -> Result<super::MachineIdentity, String> {
        const BIOS: &str = r"HARDWARE\DESCRIPTION\System\BIOS";
        const NT: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";
        let read = |key: &str, name: &str| read_hklm_string(key, name);
        Ok(super::MachineIdentity {
            manufacturer: read(BIOS, "SystemManufacturer"),
            product_model: read(BIOS, "SystemProductName"),
            product_family: read(BIOS, "SystemFamily"),
            board_product: read(BIOS, "BaseBoardProduct"),
            windows_edition: read(NT, "ProductName"),
            windows_build: read(NT, "CurrentBuildNumber"),
            os_display_version: read(NT, "DisplayVersion"),
        })
    }

    /// Owns a Win32 kernel HANDLE and closes it exactly once.
    ///
    /// This wrapper is intentionally small: callers still decide which access rights to request,
    /// while early returns can no longer bypass CloseHandle.
    #[derive(Debug)]
    pub struct OwnedHandle(HANDLE);

    impl OwnedHandle {
        pub fn new(handle: HANDLE) -> Self {
            Self(handle)
        }

        pub fn get(&self) -> HANDLE {
            self.0
        }

        /// Transfers ownership to a different RAII owner (for example `std::fs::File::from_raw_handle`).
        pub fn into_raw(self) -> HANDLE {
            let this = std::mem::ManuallyDrop::new(self);
            this.0
        }
    }

    impl Drop for OwnedHandle {
        fn drop(&mut self) {
            if self.0 != HANDLE::default() && self.0 != INVALID_HANDLE_VALUE {
                unsafe {
                    let _ = CloseHandle(self.0);
                }
            }
        }
    }


    const MACHINE_MUTATION_LOCK_RELATIVE_PATH: &str = r"AetherCore\state\machine-mutation.lock";
    const GENERIC_READ_ACCESS: u32 = 0x8000_0000;
    const GENERIC_WRITE_ACCESS: u32 = 0x4000_0000;

    fn machine_mutation_lock_path() -> windows::core::Result<PathBuf> {
        // Resolve the machine-wide data root from Windows rather than trusting an inherited
        // environment variable. The installer owns and ACL-hardens this location.
        let raw = unsafe { SHGetKnownFolderPath(&FOLDERID_ProgramData, KF_FLAG_DEFAULT, None)? };
        let text = unsafe { PCWSTR(raw.0).to_string() };
        unsafe { CoTaskMemFree(Some(raw.0.cast())) };
        Ok(PathBuf::from(text?).join(MACHINE_MUTATION_LOCK_RELATIVE_PATH))
    }

    fn wide_path(path: &std::path::Path) -> Vec<u16> {
        use std::os::windows::ffi::OsStrExt;
        path.as_os_str().encode_wide().chain(std::iter::once(0)).collect()
    }

    /// Cross-process machine-mutation lease backed by an installer-provisioned file in the
    /// protected ProgramData tree. Unlike a predictable Global named mutex, an unprivileged
    /// process cannot pre-create this authority object to deny service, because the parent tree
    /// grants no access to ordinary users.
    #[derive(Debug)]
    pub struct MachineMutationGuard {
        file: OwnedHandle,
    }

    impl MachineMutationGuard {
        /// `Ok(None)` means another trusted process currently owns the byte-range lock.
        /// Missing/untrusted lock infrastructure is an error rather than silently weakening the
        /// cross-process mutation boundary.
        pub fn try_acquire() -> windows::core::Result<Option<Self>> {
            let path = machine_mutation_lock_path()?;
            let wide = wide_path(&path);
            let handle = unsafe {
                CreateFileW(
                    PCWSTR(wide.as_ptr()),
                    GENERIC_READ_ACCESS | GENERIC_WRITE_ACCESS,
                    FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                    None,
                    OPEN_EXISTING,
                    FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OPEN_REPARSE_POINT,
                    None,
                )?
            };
            let file = OwnedHandle::new(handle);
            let mut overlapped = OVERLAPPED::default();
            match unsafe {
                LockFileEx(
                    file.get(),
                    LOCKFILE_EXCLUSIVE_LOCK | LOCKFILE_FAIL_IMMEDIATELY,
                    None,
                    1,
                    0,
                    &mut overlapped,
                )
            } {
                Ok(()) => Ok(Some(Self { file })),
                Err(error) if error.code() == ERROR_LOCK_VIOLATION.to_hresult() => Ok(None),
                Err(error) => Err(error),
            }
        }
    }

    impl Drop for MachineMutationGuard {
        fn drop(&mut self) {
            let mut overlapped = OVERLAPPED::default();
            unsafe {
                let _ = UnlockFileEx(self.file.get(), None, 1, 0, &mut overlapped);
            }
        }
    }

    /// Owns a Service Control Manager/service handle returned by OpenSCManager/OpenService.
    #[derive(Debug)]
    pub struct OwnedServiceHandle(SC_HANDLE);

    impl OwnedServiceHandle {
        pub fn new(handle: SC_HANDLE) -> Self {
            Self(handle)
        }

        pub fn get(&self) -> SC_HANDLE {
            self.0
        }
    }

    impl Drop for OwnedServiceHandle {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseServiceHandle(self.0);
            }
        }
    }

    /// Balances a successful CoInitializeEx(COINIT_MULTITHREADED) on the current thread.
    pub struct ComApartment;

    impl ComApartment {
        pub fn mta() -> Result<Self, HRESULT> {
            let status = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
            if status.is_err() {
                Err(status)
            } else {
                Ok(Self)
            }
        }
    }

    impl Drop for ComApartment {
        fn drop(&mut self) {
            unsafe { CoUninitialize() };
        }
    }

    enum ImpersonationKind {
        NamedPipe,
        LoggedOnUser,
    }

    /// Scope guard for thread impersonation. `revert` is explicit so security-sensitive callers
    /// can surface RevertToSelf failure; Drop is a final defensive fallback for early returns.
    pub struct ThreadImpersonation {
        active: bool,
        _kind: ImpersonationKind,
    }

    impl ThreadImpersonation {
        pub fn named_pipe_client(pipe: HANDLE) -> windows::core::Result<Self> {
            unsafe { ImpersonateNamedPipeClient(pipe)? };
            Ok(Self {
                active: true,
                _kind: ImpersonationKind::NamedPipe,
            })
        }

        pub fn logged_on_user(token: HANDLE) -> windows::core::Result<Self> {
            unsafe { ImpersonateLoggedOnUser(token)? };
            Ok(Self {
                active: true,
                _kind: ImpersonationKind::LoggedOnUser,
            })
        }

        pub fn revert(mut self) -> windows::core::Result<()> {
            unsafe { RevertToSelf()? };
            self.active = false;
            Ok(())
        }
    }

    impl Drop for ThreadImpersonation {
        fn drop(&mut self) {
            if self.active {
                unsafe {
                    let _ = RevertToSelf();
                }
                self.active = false;
            }
        }
    }

    /// Applies Windows background processing mode for the lifetime of the guard.
    ///
    /// Autonomous work treats inability to enter background mode as an admission failure rather
    /// than silently consuming foreground CPU/I/O priority.
    pub struct BackgroundThreadMode;

    impl BackgroundThreadMode {
        pub fn enter() -> windows::core::Result<Self> {
            unsafe {
                SetThreadPriority(GetCurrentThread(), THREAD_MODE_BACKGROUND_BEGIN)?;
            }
            Ok(Self)
        }
    }

    impl Drop for BackgroundThreadMode {
        fn drop(&mut self) {
            unsafe {
                let _ = SetThreadPriority(GetCurrentThread(), THREAD_MODE_BACKGROUND_END);
            }
        }
    }
}

#[cfg(windows)]
pub use windows_impl::{machine_identity, BackgroundThreadMode, ComApartment, MachineMutationGuard, OwnedHandle, OwnedServiceHandle, ThreadImpersonation};

#[cfg(not(windows))]
pub fn machine_identity() -> Result<MachineIdentity, String> {
    Ok(MachineIdentity::default())
}
