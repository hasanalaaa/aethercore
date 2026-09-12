use std::{
    collections::HashMap,
    ffi::c_void,
    fs::File,
    os::windows::{
        fs::OpenOptionsExt,
        io::{AsRawHandle, IntoRawHandle},
    },
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

use aethercore_contracts::{
    DEFAULT_REQUEST_DEADLINE_MS, MAX_REQUEST_DEADLINE_MS, PROTOCOL_VERSION,
    v1::{
        self, ClientFrame, ClientHello, Request, Response, ServerFrame, SessionRequest,
        client_frame, server_frame,
    },
};
use aethercore_windows_foundation::{OwnedHandle, OwnedServiceHandle};
use prost::Message;
use windows::{
    Win32::{
        Foundation::{
            CloseHandle, ERROR_FILE_NOT_FOUND, ERROR_INSUFFICIENT_BUFFER, ERROR_IO_PENDING,
            ERROR_PIPE_BUSY, ERROR_PIPE_CONNECTED, HANDLE, HLOCAL, INVALID_HANDLE_VALUE, LocalFree,
        },
        Security::{
            Authorization::{
                ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
                GetSecurityInfo, SDDL_REVISION_1, SE_FILE_OBJECT,
            },
            IsValidSid, LookupAccountNameW, OWNER_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID,
            SECURITY_ATTRIBUTES, SID_NAME_USE,
        },
        Storage::FileSystem::{
            FILE_FLAG_FIRST_PIPE_INSTANCE, FILE_FLAG_OVERLAPPED, PIPE_ACCESS_DUPLEX, ReadFile,
            WriteFile,
        },
        System::{
            IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED},
            Pipes::{
                ConnectNamedPipe, CreateNamedPipeW, PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS,
                PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
            },
            Services::{
                OpenSCManagerW, OpenServiceW, QueryServiceStatusEx, SC_MANAGER_CONNECT,
                SC_STATUS_PROCESS_INFO, SERVICE_QUERY_STATUS, SERVICE_RUNNING,
                SERVICE_STATUS_PROCESS, SERVICE_WIN32_OWN_PROCESS,
            },
            Threading::CreateEventW,
        },
    },
    core::{PCWSTR, PWSTR},
};

use crate::{
    IpcError, PIPE_NAME, Result, configured_pipe_name, read_client_frame, read_server_frame,
    write_client_frame, write_server_frame,
};

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}
fn unix_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis().min(i64::MAX as u128) as i64)
        .unwrap_or(0)
}

fn retryable_pipe_open_error(error: &std::io::Error) -> bool {
    matches!(
        error.raw_os_error(),
        Some(code)
            if code == ERROR_FILE_NOT_FOUND.0 as i32
                || code == ERROR_PIPE_BUSY.0 as i32
    )
}

fn open_pipe() -> Result<File> {
    let pipe_name = configured_pipe_name()?;
    let mut last = None;
    for attempt in 0..40 {
        let mut options = std::fs::OpenOptions::new();
        options
            .access_mode(PIPE_CLIENT_ACCESS_MASK)
            .share_mode(0)
            // Without this the client file object is FO_SYNCHRONOUS_IO and the I/O manager
            // serializes every operation on it, so a queued write parks behind the reader
            // thread's outstanding read. Both ends of the pipe must be overlapped.
            .custom_flags(FILE_FLAG_OVERLAPPED.0)
            // Prevent a server from impersonating this client while endpoint identity is still
            // being authenticated. OpenOptionsExt adds SECURITY_SQOS_PRESENT automatically.
            .security_qos_flags(PIPE_CLIENT_SECURITY_QOS);
        match options.open(&pipe_name) {
            Ok(file) => {
                // An opened-but-untrusted endpoint is evidence of namespace compromise, not a
                // transient availability failure. Fail before ClientHello rather than retrying
                // across competing server instances and accidentally masking the condition.
                verify_connected_server(&file, &pipe_name)?;
                return Ok(file);
            }
            Err(error) if retryable_pipe_open_error(&error) => last = Some(error),
            Err(error) => return Err(IpcError::Io(error)),
        }
        if attempt + 1 < 40 {
            thread::sleep(Duration::from_millis(50));
        }
    }
    Err(IpcError::Io(last.unwrap_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "pipe unavailable")
    })))
}

// Count limits stop unbounded producer fan-out; byte limits stop a handful of maximum-size
// protobuf frames from turning a nominally bounded queue into a large per-session memory sink.
const SERVER_OUTBOUND_QUEUE_CAPACITY: usize = 32;
const SERVER_OUTBOUND_BYTE_BUDGET: usize = 16 * 1024 * 1024;
const CLIENT_OUTBOUND_QUEUE_CAPACITY: usize = 32;
const CLIENT_OUTBOUND_BYTE_BUDGET: usize = 4 * 1024 * 1024;
// DBT-P46-D1: one decider for the identity this peer check authenticates
// against. The name and the account it derives from used to be two independent
// literals here, and two more in apps/install-hardener.
use aethercore_product_identity::{SERVICE_NAME as TRUSTED_SERVICE_NAME, service_principal};
const MAX_ACCOUNT_SID_BYTES: u32 = 4 * 1024;
const MAX_ACCOUNT_DOMAIN_CHARS: u32 = 32 * 1024;
static TRUSTED_SERVICE_SID: OnceLock<String> = OnceLock::new();
// Client transport needs data read/write, READ_CONTROL to authenticate the pipe owner, and
// synchronization for synchronous I/O. Bit 0x0000_0004 (FILE_CREATE_PIPE_INSTANCE /
// FILE_APPEND_DATA) is intentionally absent.
const PIPE_CLIENT_ACCESS_MASK: u32 = 0x0012_0003;
// SECURITY_IDENTIFICATION. This lets the legitimate service inspect client identity for its own
// authorization decision but prevents it (or a squatted endpoint) from impersonating the client
// to access other local resources. OpenOptionsExt supplies SECURITY_SQOS_PRESENT.
const PIPE_CLIENT_SECURITY_QOS: u32 = 0x0001_0000;
#[cfg(debug_assertions)]
const DEV_PIPE_SECURITY_DESCRIPTOR: &str = "D:P(A;;GA;;;SY)(A;;GA;;;BA)(A;;0x00120003;;;AU)";

fn lookup_account_sid(account_name: &str) -> Result<Vec<u64>> {
    unsafe {
        let account = wide(account_name);
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
            Err(error) => {
                return Err(IpcError::Windows(format!(
                    "resolve Windows account SID size for {account_name}: {error}"
                )));
            }
            Ok(()) => {
                return Err(IpcError::Windows(format!(
                    "Windows account SID probe unexpectedly succeeded without a SID buffer for {account_name}"
                )));
            }
        }
        if sid_bytes == 0 || sid_bytes > MAX_ACCOUNT_SID_BYTES {
            return Err(IpcError::Windows(format!(
                "Windows account SID size for {account_name} is outside the safety bound: {sid_bytes} bytes"
            )));
        }
        if domain_chars > MAX_ACCOUNT_DOMAIN_CHARS {
            return Err(IpcError::Windows(format!(
                "Windows account domain size for {account_name} is outside the safety bound: {domain_chars} UTF-16 code units"
            )));
        }
        let sid_word_count = usize::try_from(sid_bytes)
            .ok()
            .and_then(|bytes| bytes.checked_add(std::mem::size_of::<u64>() - 1))
            .map(|bytes| bytes / std::mem::size_of::<u64>())
            .ok_or_else(|| {
                IpcError::Windows(format!(
                    "Windows account SID allocation size overflow for {account_name}"
                ))
            })?;
        let mut sid = vec![0u64; sid_word_count];
        let sid_capacity_bytes = sid
            .len()
            .checked_mul(std::mem::size_of::<u64>())
            .ok_or_else(|| {
                IpcError::Windows(format!(
                    "Windows account SID capacity overflow for {account_name}"
                ))
            })?;
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
        .map_err(|error| {
            IpcError::Windows(format!(
                "resolve Windows account SID for {account_name}: {error}"
            ))
        })?;
        if sid_bytes == 0
            || usize::try_from(sid_bytes).map_or(true, |bytes| bytes > sid_capacity_bytes)
        {
            return Err(IpcError::Windows(format!(
                "Windows account SID length changed outside its allocated buffer for {account_name}: {sid_bytes} bytes"
            )));
        }
        if !IsValidSid(sid_ptr).as_bool() {
            return Err(IpcError::Windows(format!(
                "Windows returned an invalid SID structure for {account_name}"
            )));
        }
        Ok(sid)
    }
}

fn sid_to_string(sid: PSID) -> Result<String> {
    unsafe {
        let mut raw = PWSTR::null();
        ConvertSidToStringSidW(sid, &mut raw)
            .map_err(|error| IpcError::Windows(format!("convert SID to string: {error}")))?;
        if raw.is_null() {
            return Err(IpcError::Windows(
                "ConvertSidToStringSidW returned a null string".into(),
            ));
        }
        let result = PCWSTR(raw.0)
            .to_string()
            .map_err(|error| IpcError::Windows(format!("decode SID string: {error}")));
        let _ = LocalFree(Some(HLOCAL(raw.0.cast::<c_void>())));
        result
    }
}

fn trusted_service_sid_string() -> Result<String> {
    if let Some(service_sid) = TRUSTED_SERVICE_SID.get() {
        return Ok(service_sid.clone());
    }

    let mut sid = lookup_account_sid(&service_principal())?;
    let resolved = sid_to_string(PSID(sid.as_mut_ptr().cast::<c_void>()))?;
    // Races are benign: every contender resolved the same local service account. Cache the first
    // successful value so listener churn and client endpoint authentication never turn LSA account
    // lookup into a hot-path dependency. Failed lookups are deliberately not cached.
    let _ = TRUSTED_SERVICE_SID.set(resolved.clone());
    Ok(TRUSTED_SERVICE_SID.get().cloned().unwrap_or(resolved))
}

fn production_pipe_security_descriptor(service_sid: &str) -> String {
    // The service-specific SID is both the object owner and the only principal allowed to create
    // server instances. Authenticated Users receive only the exact client mask. This keeps the
    // endpoint identity service-scoped without requiring a write-restricted process token.
    //
    // P36 Tranche 1 defect fix (Hermes): the previous AU ACE used the hex mask
    // 0x00120003, but Windows' SDDL parser SILENTLY DROPS the SYNCHRONIZE (0x00100000)
    // bit from hex access masks, so the materialized DACL granted AU only 0x120003.
    // Every client — including the real desktop app — opens with
    // PIPE_CLIENT_ACCESS_MASK = 0x00120003 (synchronous I/O requires SYNCHRONIZE) and
    // was denied: the production pipe was unusable as encoded. Lab-proven (A/B, live
    // DACL dump) remedy: express the client rights with named SDDL rights
    // FR (FILE_GENERIC_READ = READ_DATA|READ_ATTRIBUTES|READ_EA|READ_CONTROL|SYNCHRONIZE)
    // plus hex 0x2 (FILE_WRITE_DATA only, which FR does not include), so the AU grant
    // materializes as 0x12008B: it fully covers the client mask while still NOT granting
    // FILE_CREATE_PIPE_INSTANCE (0x4), FILE_APPEND_DATA, or any generic server authority —
    // the declared policy is unchanged, only the encoding is now expressible.
    format!("O:{service_sid}D:P(A;;GA;;;{service_sid})(A;;FR;;;AU)(A;;0x00000002;;;AU)")
}

fn security_descriptor_for_pipe(pipe_name: &str) -> Result<String> {
    #[cfg(debug_assertions)]
    if pipe_name != PIPE_NAME {
        return Ok(DEV_PIPE_SECURITY_DESCRIPTOR.to_owned());
    }
    Ok(production_pipe_security_descriptor(
        &trusted_service_sid_string()?,
    ))
}

/// Authenticate the connected endpoint before any AetherCore frame is sent.
///
/// Production trust uses only client-supported Win32 primitives: the connected named-pipe object's
/// owner must be the service-specific SID for NT SERVICE\AetherCoreMaintenance and the expected
/// own-process Windows service must currently be running. The service separately claims
/// FILE_FLAG_FIRST_PIPE_INSTANCE at startup, so a standard-user namespace squatter cannot coexist
/// with the trusted service. Debug console development uses a per-launch 128-bit pipe name instead;
/// that path is compiled out of release behavior.
fn verify_connected_server(file: &File, pipe_name: &str) -> Result<()> {
    #[cfg(debug_assertions)]
    if pipe_name != PIPE_NAME {
        return Ok(());
    }

    verify_pipe_owned_by_trusted_service(file)?;
    verify_registered_service_running()
}

fn verify_pipe_owned_by_trusted_service(file: &File) -> Result<()> {
    let pipe = HANDLE(file.as_raw_handle() as *mut c_void);
    let mut owner = PSID::default();
    let mut descriptor = PSECURITY_DESCRIPTOR::default();
    // Keep the runtime object-type contract identical to the native qualification probe.
    // The Windows qualification gate must prove GetSecurityInfo(SE_FILE_OBJECT) succeeds on
    // the production CreateFile-opened pipe handle before this endpoint-authentication path ships.
    let status = unsafe {
        GetSecurityInfo(
            pipe,
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION,
            Some(&mut owner),
            None,
            None,
            None,
            Some(&mut descriptor),
        )
    };
    if status.is_err() {
        if !descriptor.0.is_null() {
            unsafe {
                let _ = LocalFree(Some(HLOCAL(descriptor.0)));
            }
        }
        return Err(IpcError::UntrustedPeer(format!(
            "cannot read named-pipe owner security descriptor: Win32 error {}",
            status.0
        )));
    }
    let owner_sid = if owner.0.is_null() {
        Err(IpcError::UntrustedPeer(
            "named-pipe security descriptor has no owner SID".into(),
        ))
    } else {
        sid_to_string(owner).map_err(|error| {
            IpcError::UntrustedPeer(format!("cannot decode named-pipe owner SID: {error}"))
        })
    };
    if !descriptor.0.is_null() {
        unsafe {
            let _ = LocalFree(Some(HLOCAL(descriptor.0)));
        }
    }
    let owner_sid = owner_sid?;
    let expected_sid = trusted_service_sid_string().map_err(|error| {
        IpcError::UntrustedPeer(format!(
            "cannot resolve trusted maintenance service SID: {error}"
        ))
    })?;
    if owner_sid != expected_sid {
        return Err(IpcError::UntrustedPeer(format!(
            "named-pipe owner is not the trusted maintenance service SID: expected {expected_sid}, found {owner_sid}"
        )));
    }
    Ok(())
}

fn verify_registered_service_running() -> Result<()> {
    unsafe {
        let scm = OwnedServiceHandle::new(
            OpenSCManagerW(PCWSTR::null(), PCWSTR::null(), SC_MANAGER_CONNECT).map_err(
                |error| {
                    IpcError::Windows(format!(
                        "open Service Control Manager for IPC peer verification: {error}"
                    ))
                },
            )?,
        );
        let service_name = wide(TRUSTED_SERVICE_NAME);
        let service = OwnedServiceHandle::new(
            OpenServiceW(
                scm.get(),
                PCWSTR(service_name.as_ptr()),
                SERVICE_QUERY_STATUS,
            )
            .map_err(|error| {
                IpcError::UntrustedPeer(format!(
                    "trusted maintenance service is unavailable: {error}"
                ))
            })?,
        );
        let mut status = SERVICE_STATUS_PROCESS::default();
        let mut needed = 0u32;
        let bytes = std::slice::from_raw_parts_mut(
            (&mut status as *mut SERVICE_STATUS_PROCESS).cast::<u8>(),
            std::mem::size_of::<SERVICE_STATUS_PROCESS>(),
        );
        QueryServiceStatusEx(
            service.get(),
            SC_STATUS_PROCESS_INFO,
            Some(bytes),
            &mut needed,
        )
        .map_err(|error| {
            IpcError::UntrustedPeer(format!("cannot verify maintenance service state: {error}"))
        })?;
        if status.dwCurrentState != SERVICE_RUNNING
            || status.dwServiceType != SERVICE_WIN32_OWN_PROCESS
            || status.dwProcessId == 0
        {
            return Err(IpcError::UntrustedPeer(
                "trusted maintenance service is not a running own-process service".into(),
            ));
        }
    }
    Ok(())
}

struct QueuedServerFrame {
    frame: ServerFrame,
    bytes: usize,
}

struct QueuedClientFrame {
    frame: ClientFrame,
    bytes: usize,
}

fn try_reserve_bytes(counter: &AtomicUsize, amount: usize, limit: usize) -> bool {
    let mut current = counter.load(Ordering::Acquire);
    loop {
        let Some(next) = current.checked_add(amount) else {
            return false;
        };
        if next > limit {
            return false;
        }
        match counter.compare_exchange_weak(current, next, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => return true,
            Err(observed) => current = observed,
        }
    }
}

fn release_bytes(counter: &AtomicUsize, amount: usize) {
    let previous = counter.fetch_sub(amount, Ordering::AcqRel);
    debug_assert!(previous >= amount, "outbound byte accounting underflow");
}

fn notify_disconnect_once(notified: &AtomicBool, callback: &Arc<dyn Fn() + Send + Sync + 'static>) {
    if !notified.swap(true, Ordering::AcqRel) {
        callback();
    }
}

fn try_acquire_slot(counter: &AtomicUsize, limit: usize) -> bool {
    let mut current = counter.load(Ordering::Acquire);
    loop {
        if current >= limit {
            return false;
        }
        match counter.compare_exchange_weak(
            current,
            current + 1,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => return true,
            Err(observed) => current = observed,
        }
    }
}

/// A kernel handle held by value so it can cross threads.
///
/// `windows::Win32::Foundation::HANDLE` is intentionally `!Send + !Sync`; storing the opaque
/// numeric value keeps the cross-thread contract explicit, as the previous thread-handle
/// cancellation token did.
struct PipeHandle(usize);

impl PipeHandle {
    fn get(&self) -> HANDLE {
        HANDLE(self.0 as *mut c_void)
    }
}

impl Drop for PipeHandle {
    fn drop(&mut self) {
        if self.0 != 0 {
            unsafe {
                let _ = CloseHandle(self.get());
            }
        }
    }
}

/// Auto-reset: `GetOverlappedResult`'s wait consumes the signal, so every operation starts from a
/// non-signalled event without an explicit reset.
fn new_completion_event() -> Result<PipeHandle> {
    let handle = unsafe { CreateEventW(None, false, false, PCWSTR::null()) }
        .map_err(|error| IpcError::Windows(format!("create overlapped IPC event: {error}")))?;
    Ok(PipeHandle(handle.0 as usize))
}

/// Preserve the raw Win32 code, so a cancelled operation still surfaces as
/// `ERROR_OPERATION_ABORTED` through `IpcError::Io` exactly as the synchronous path did.
fn win32_io_error(error: windows::core::Error) -> std::io::Error {
    std::io::Error::from_raw_os_error(error.code().0 & 0xFFFF)
}

/// Aborts every outstanding operation on a connected pipe, from any thread.
///
/// `CancelIoEx` with a null OVERLAPPED cancels all in-flight I/O on the handle rather than the
/// calls issued by one thread. Every call site here already cancelled both directions, so this
/// reproduces the teardown `CancelSynchronousIo` performed against the two I/O threads.
#[derive(Clone)]
struct PipeCancel(Arc<PipeHandle>);

impl PipeCancel {
    fn cancel(&self) {
        if self.0.0 == 0 {
            return;
        }
        unsafe {
            let _ = CancelIoEx(self.0.get(), None);
        }
    }

    #[cfg(test)]
    fn noop_for_test() -> Self {
        Self(Arc::new(PipeHandle(0)))
    }
}

/// One direction of overlapped I/O over a shared named-pipe file object.
///
/// The reader and the writer thread hold separate `PipeIo` values referencing the same kernel
/// file object through the shared `Arc`. Each owns its own event, and each operation gets its own
/// `OVERLAPPED` on the calling stack, so a concurrent read and write never share completion state.
struct PipeIo {
    handle: Arc<PipeHandle>,
    event: PipeHandle,
}

impl PipeIo {
    fn from_connected(handle: HANDLE) -> Result<Self> {
        Ok(Self {
            handle: Arc::new(PipeHandle(handle.0 as usize)),
            event: new_completion_event()?,
        })
    }

    /// The opposite direction on the same file object, with its own event.
    fn split_direction(&self) -> Result<Self> {
        Ok(Self {
            handle: self.handle.clone(),
            event: new_completion_event()?,
        })
    }

    fn cancel(&self) -> PipeCancel {
        PipeCancel(self.handle.clone())
    }

    fn raw(&self) -> HANDLE {
        self.handle.get()
    }

    /// Issue one overlapped operation and wait for it to finish.
    ///
    /// The `OVERLAPPED` lives on this frame, so every path must leave the kernel done with it:
    /// `GetOverlappedResult` with `bWait` returns only once the operation completes or aborts.
    fn transfer<F>(&mut self, issue: F) -> std::io::Result<usize>
    where
        F: FnOnce(HANDLE, *mut OVERLAPPED) -> windows::core::Result<()>,
    {
        let mut overlapped = OVERLAPPED {
            hEvent: self.event.get(),
            ..Default::default()
        };
        let handle = self.handle.get();
        match issue(handle, &mut overlapped) {
            Ok(()) => {}
            Err(error) if error.code() == ERROR_IO_PENDING.to_hresult() => {}
            Err(error) => return Err(win32_io_error(error)),
        }
        let mut transferred = 0u32;
        unsafe { GetOverlappedResult(handle, &overlapped, &mut transferred, true) }
            .map_err(win32_io_error)?;
        Ok(transferred as usize)
    }
}

impl std::io::Read for PipeIo {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        // A byte-mode pipe may satisfy a read partially; `read_exact` in the frame codec loops.
        self.transfer(|handle, overlapped| unsafe {
            ReadFile(handle, Some(buf), None, Some(overlapped))
        })
    }
}

impl std::io::Write for PipeIo {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        // A byte-mode pipe can also accept a write partially, so drain the whole buffer here
        // instead of depending on the caller to loop.
        let mut written = 0usize;
        while written < buf.len() {
            let chunk = &buf[written..];
            let count = self.transfer(|handle, overlapped| unsafe {
                WriteFile(handle, Some(chunk), None, Some(overlapped))
            })?;
            if count == 0 {
                return Err(std::io::Error::from(std::io::ErrorKind::WriteZero));
            }
            written += count;
        }
        Ok(written)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        // The bytes are already handed to the kernel. FlushFileBuffers would additionally block
        // until the peer drains them, which the frame codec must never do.
        Ok(())
    }
}

#[derive(Clone)]
pub struct PipeServerWriter {
    tx: mpsc::SyncSender<QueuedServerFrame>,
    alive: Arc<AtomicBool>,
    queued_bytes: Arc<AtomicUsize>,
    cancel: PipeCancel,
}
impl PipeServerWriter {
    pub fn write(&self, frame: &ServerFrame) -> Result<()> {
        if !self.alive.load(Ordering::Acquire) {
            return Err(IpcError::Disconnected);
        }
        let bytes = frame.encoded_len();
        if !try_reserve_bytes(&self.queued_bytes, bytes, SERVER_OUTBOUND_BYTE_BUDGET) {
            self.shutdown();
            return Err(IpcError::OutboundBackpressure);
        }
        match self.tx.try_send(QueuedServerFrame {
            frame: frame.clone(),
            bytes,
        }) {
            Ok(()) => Ok(()),
            Err(mpsc::TrySendError::Full(queued)) => {
                release_bytes(&self.queued_bytes, queued.bytes);
                self.shutdown();
                Err(IpcError::OutboundBackpressure)
            }
            Err(mpsc::TrySendError::Disconnected(queued)) => {
                release_bytes(&self.queued_bytes, queued.bytes);
                self.shutdown();
                Err(IpcError::Disconnected)
            }
        }
    }
    /// Session bootstrap/replay is allowed to wait briefly for the dedicated writer queue to drain.
    /// Live request/event producers must use `write`, which never waits on pipe backpressure.
    pub fn write_bootstrap(&self, frame: &ServerFrame, max_wait: Duration) -> Result<()> {
        if !self.alive.load(Ordering::Acquire) {
            return Err(IpcError::Disconnected);
        }
        let deadline = Instant::now()
            .checked_add(max_wait)
            .unwrap_or_else(Instant::now);
        let bytes = frame.encoded_len();
        loop {
            if !self.alive.load(Ordering::Acquire) {
                return Err(IpcError::Disconnected);
            }
            if try_reserve_bytes(&self.queued_bytes, bytes, SERVER_OUTBOUND_BYTE_BUDGET) {
                match self.tx.try_send(QueuedServerFrame {
                    frame: frame.clone(),
                    bytes,
                }) {
                    Ok(()) => return Ok(()),
                    Err(mpsc::TrySendError::Full(queued)) => {
                        release_bytes(&self.queued_bytes, queued.bytes)
                    }
                    Err(mpsc::TrySendError::Disconnected(queued)) => {
                        release_bytes(&self.queued_bytes, queued.bytes);
                        self.shutdown();
                        return Err(IpcError::Disconnected);
                    }
                }
            }
            if Instant::now() >= deadline {
                self.shutdown();
                return Err(IpcError::OutboundBackpressure);
            }
            thread::sleep(Duration::from_millis(1));
        }
    }
    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::Acquire)
    }
    fn shutdown(&self) {
        self.alive.store(false, Ordering::Release);
        // One call aborts both directions of the file object - the writer's pending write and
        // the session loop's pending read - which is what the two cancellations here did.
        self.cancel.cancel();
    }
}

/// An unconnected server instance that owns the named-pipe namespace while waiting for a client.
///
/// The maintenance service always creates the successor listener before handing the currently
/// accepted session to a worker. This keeps at least one service-owned instance alive across
/// session churn, so a standard-user process never gets a gap in which it can become first creator.
pub struct PipeServerListener {
    handle: OwnedHandle,
}

impl PipeServerListener {
    /// Claim the namespace at service startup. A pre-existing instance is a terminal trust failure;
    /// the service never joins an already-created security-sensitive rendezvous name.
    pub fn claim_first() -> Result<Self> {
        Self::create_with_mode(true)
    }

    /// Create the next listening instance while another service-owned instance is still alive.
    pub fn create_successor() -> Result<Self> {
        Self::create_with_mode(false)
    }

    fn create_with_mode(first_instance: bool) -> Result<Self> {
        unsafe {
            let pipe_name = configured_pipe_name()?;
            let name = wide(&pipe_name);
            let security_descriptor = security_descriptor_for_pipe(&pipe_name)?;
            let sddl = wide(&security_descriptor);
            let mut sd = PSECURITY_DESCRIPTOR::default();
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                PCWSTR(sddl.as_ptr()),
                SDDL_REVISION_1,
                &mut sd,
                None,
            )
            .map_err(|e| IpcError::Windows(e.to_string()))?;
            let sa = SECURITY_ATTRIBUTES {
                nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: sd.0,
                bInheritHandle: false.into(),
            };
            let mode = PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS;
            // FILE_FLAG_OVERLAPPED is what stops the I/O manager serializing this session's
            // read and write on the one file object. Without it a response write parks behind
            // the session loop's outstanding read and no request ever completes.
            let open_mode = if first_instance {
                PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE | FILE_FLAG_OVERLAPPED
            } else {
                PIPE_ACCESS_DUPLEX | FILE_FLAG_OVERLAPPED
            };
            let handle = CreateNamedPipeW(
                PCWSTR(name.as_ptr()),
                open_mode,
                mode,
                PIPE_UNLIMITED_INSTANCES,
                64 * 1024,
                64 * 1024,
                0,
                Some(&sa),
            );
            let _ = LocalFree(Some(HLOCAL(sd.0)));
            if handle == INVALID_HANDLE_VALUE {
                return Err(IpcError::Windows(
                    windows::core::Error::from_thread().to_string(),
                ));
            }
            Ok(Self {
                handle: OwnedHandle::new(handle),
            })
        }
    }

    pub fn accept(self) -> Result<PipeServerSession> {
        // Under FILE_FLAG_OVERLAPPED, ConnectNamedPipe requires a real OVERLAPPED and reports
        // the wait as ERROR_IO_PENDING instead of blocking. ERROR_PIPE_CONNECTED still means a
        // client arrived before the call. All three outcomes accept exactly one session, so the
        // service accept loop keeps its existing shape and successor-listener ordering.
        let event = new_completion_event()?;
        let mut overlapped = OVERLAPPED {
            hEvent: event.get(),
            ..Default::default()
        };
        unsafe {
            match ConnectNamedPipe(self.handle.get(), Some(&mut overlapped)) {
                Ok(()) => {}
                Err(e) if e.code() == ERROR_PIPE_CONNECTED.to_hresult() => {}
                Err(e) if e.code() == ERROR_IO_PENDING.to_hresult() => {
                    let mut transferred = 0u32;
                    GetOverlappedResult(self.handle.get(), &overlapped, &mut transferred, true)
                        .map_err(|e| IpcError::Windows(e.to_string()))?;
                }
                Err(e) => return Err(IpcError::Windows(e.to_string())),
            }
        }
        PipeServerSession::from_connected_reader(self.handle.into_raw())
    }
}

pub struct PipeServerSession {
    reader: PipeIo,
    writer: PipeServerWriter,
}
impl PipeServerSession {
    fn from_connected_reader(handle: HANDLE) -> Result<Self> {
        let reader = PipeIo::from_connected(handle)?;
        // Same kernel file object, independent completion state. This is what lets the writer
        // thread complete a response while the session loop is parked in a read.
        let mut writer_io = reader.split_direction()?;
        let cancel = reader.cancel();
        let (tx, rx) = mpsc::sync_channel::<QueuedServerFrame>(SERVER_OUTBOUND_QUEUE_CAPACITY);
        let writer_alive = Arc::new(AtomicBool::new(true));
        let queued_bytes = Arc::new(AtomicUsize::new(0));
        let pump_alive = writer_alive.clone();
        let pump_bytes = queued_bytes.clone();
        let pump_cancel = cancel.clone();
        thread::Builder::new()
            .name("aether-ipc-server-writer".into())
            .spawn(move || {
                while let Ok(queued) = rx.recv() {
                    if !pump_alive.load(Ordering::Acquire) {
                        release_bytes(&pump_bytes, queued.bytes);
                        break;
                    }
                    let result = write_server_frame(&mut writer_io, &queued.frame);
                    release_bytes(&pump_bytes, queued.bytes);
                    if result.is_err() {
                        break;
                    }
                }
                pump_alive.store(false, Ordering::Release);
                pump_cancel.cancel();
            })
            .map_err(IpcError::Io)?;
        Ok(Self {
            reader,
            writer: PipeServerWriter {
                tx,
                alive: writer_alive,
                queued_bytes,
                cancel,
            },
        })
    }
    pub fn raw_handle(&self) -> std::os::windows::io::RawHandle {
        self.reader.raw().0
    }
    pub fn read(&mut self) -> Result<ClientFrame> {
        if !self.writer.is_alive() {
            return Err(IpcError::Disconnected);
        }
        read_client_frame(&mut self.reader)
    }
    pub fn writer(&self) -> PipeServerWriter {
        self.writer.clone()
    }
}
impl Drop for PipeServerSession {
    fn drop(&mut self) {
        self.writer.shutdown();
    }
}

#[derive(Clone)]
struct PipeClientWriter {
    tx: mpsc::SyncSender<QueuedClientFrame>,
    alive: Arc<AtomicBool>,
    queued_bytes: Arc<AtomicUsize>,
    cancel: PipeCancel,
}
impl PipeClientWriter {
    fn write(&self, frame: &ClientFrame) -> Result<()> {
        if !self.alive.load(Ordering::Acquire) {
            return Err(IpcError::Disconnected);
        }
        let bytes = frame.encoded_len();
        if !try_reserve_bytes(&self.queued_bytes, bytes, CLIENT_OUTBOUND_BYTE_BUDGET) {
            self.shutdown();
            return Err(IpcError::OutboundBackpressure);
        }
        match self.tx.try_send(QueuedClientFrame {
            frame: frame.clone(),
            bytes,
        }) {
            Ok(()) => Ok(()),
            Err(mpsc::TrySendError::Full(queued)) => {
                release_bytes(&self.queued_bytes, queued.bytes);
                self.shutdown();
                Err(IpcError::OutboundBackpressure)
            }
            Err(mpsc::TrySendError::Disconnected(queued)) => {
                release_bytes(&self.queued_bytes, queued.bytes);
                self.shutdown();
                Err(IpcError::Disconnected)
            }
        }
    }
    fn shutdown(&self) {
        self.alive.store(false, Ordering::Release);
        // Aborts the pending write and the reader thread's pending read in one call.
        self.cancel.cancel();
    }
}

struct ClientInflightGuard<'a> {
    count: &'a AtomicUsize,
}

impl Drop for ClientInflightGuard<'_> {
    fn drop(&mut self) {
        self.count.fetch_sub(1, Ordering::AcqRel);
    }
}

pub struct SessionClient {
    writer: PipeClientWriter,
    pending: Arc<Mutex<HashMap<String, mpsc::SyncSender<Response>>>>,
    alive: Arc<AtomicBool>,
    inflight: AtomicUsize,
    max_inflight: usize,
    pub hello: v1::ServerHello,
}

impl SessionClient {
    pub fn connect(
        client_name: &str,
        client_version: &str,
        replay_after_sequence: u64,
        on_event: Arc<dyn Fn(v1::EventEnvelope) + Send + Sync + 'static>,
        on_stream_reset: Arc<dyn Fn(v1::StreamReset) + Send + Sync + 'static>,
        on_disconnect: Arc<dyn Fn() + Send + Sync + 'static>,
    ) -> Result<Arc<Self>> {
        // `open_pipe` performs endpoint authentication on a `File`; take the handle over
        // afterwards so the verification path stays exactly as qualified.
        let mut reader = PipeIo::from_connected(HANDLE(open_pipe()?.into_raw_handle()))?;
        // Independent completion state per direction on the one file object.
        let mut writer_io = reader.split_direction()?;
        let cancel = reader.cancel();
        write_client_frame(
            &mut reader,
            &ClientFrame {
                payload: Some(client_frame::Payload::Hello(ClientHello {
                    protocol_version: PROTOCOL_VERSION,
                    client_name: client_name.into(),
                    client_version: client_version.into(),
                    replay_after_sequence,
                })),
            },
        )?;
        let first = read_server_frame(&mut reader)?;
        let hello = match first.payload {
            Some(server_frame::Payload::Hello(v)) if v.protocol_version == PROTOCOL_VERSION => v,
            Some(server_frame::Payload::Hello(v)) => {
                return Err(IpcError::Protocol(format!(
                    "server protocol {}",
                    v.protocol_version
                )));
            }
            _ => return Err(IpcError::Protocol("server hello required".into())),
        };
        let pending: Arc<Mutex<HashMap<String, mpsc::SyncSender<Response>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let alive = Arc::new(AtomicBool::new(true));
        let disconnect_notified = Arc::new(AtomicBool::new(false));
        let (writer_tx, writer_rx) =
            mpsc::sync_channel::<QueuedClientFrame>(CLIENT_OUTBOUND_QUEUE_CAPACITY);
        let writer_bytes = Arc::new(AtomicUsize::new(0));
        let writer_alive = alive.clone();
        let writer_pending = pending.clone();
        let writer_notified = disconnect_notified.clone();
        let writer_disconnect = on_disconnect.clone();
        let writer_cancel = cancel.clone();
        let pump_bytes = writer_bytes.clone();
        thread::Builder::new()
            .name("aether-ipc-client-writer".into())
            .spawn(move || {
                while let Ok(queued) = writer_rx.recv() {
                    if !writer_alive.load(Ordering::Acquire) {
                        release_bytes(&pump_bytes, queued.bytes);
                        while let Ok(discarded) = writer_rx.try_recv() {
                            release_bytes(&pump_bytes, discarded.bytes);
                        }
                        break;
                    }
                    let result = write_client_frame(&mut writer_io, &queued.frame);
                    release_bytes(&pump_bytes, queued.bytes);
                    if result.is_err() {
                        break;
                    }
                }
                writer_alive.store(false, Ordering::Release);
                writer_pending
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .clear();
                writer_cancel.cancel();
                notify_disconnect_once(&writer_notified, &writer_disconnect);
            })
            .map_err(IpcError::Io)?;
        let reader_alive = alive.clone();
        let reader_pending = pending.clone();
        let reader_notified = disconnect_notified.clone();
        let reader_disconnect = on_disconnect.clone();
        let reader_cancel = cancel.clone();
        // Cancellation targets the pipe handle, which exists before either thread starts, so the
        // reader needs no registration handshake before entering its loop.
        if let Err(error) = thread::Builder::new()
            .name("aether-ipc-client-reader".into())
            .spawn(move || {
                let mut reader = reader;
                while let Ok(frame) = read_server_frame(&mut reader) {
                    match frame.payload {
                        Some(server_frame::Payload::Response(resp)) => {
                            if let Some(id) = resp.header.as_ref().map(|h| h.request_id.clone()) {
                                if let Some(tx) = reader_pending
                                    .lock()
                                    .unwrap_or_else(|p| p.into_inner())
                                    .remove(&id)
                                {
                                    let _ = tx.send(resp);
                                }
                            }
                        }
                        Some(server_frame::Payload::Event(event)) => on_event(event),
                        Some(server_frame::Payload::StreamReset(reset)) => on_stream_reset(reset),
                        Some(server_frame::Payload::Hello(_)) | None => {}
                    }
                }
                reader_alive.store(false, Ordering::Release);
                reader_pending
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .clear();
                reader_cancel.cancel();
                notify_disconnect_once(&reader_notified, &reader_disconnect);
            })
        {
            alive.store(false, Ordering::Release);
            cancel.cancel();
            drop(writer_tx);
            return Err(IpcError::Io(error));
        }
        let max_inflight = (hello.max_inflight_requests as usize).max(1);
        Ok(Arc::new(Self {
            writer: PipeClientWriter {
                tx: writer_tx,
                alive: alive.clone(),
                queued_bytes: writer_bytes,
                cancel,
            },
            pending,
            alive,
            inflight: AtomicUsize::new(0),
            max_inflight,
            hello,
        }))
    }

    fn try_acquire_inflight(&self) -> Result<ClientInflightGuard<'_>> {
        if !try_acquire_slot(&self.inflight, self.max_inflight) {
            return Err(IpcError::OutboundBackpressure);
        }
        Ok(ClientInflightGuard {
            count: &self.inflight,
        })
    }

    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::Acquire)
    }
    pub fn request(&self, request: Request, timeout: Duration) -> Result<Response> {
        if !self.is_alive() {
            return Err(IpcError::Disconnected);
        }
        let _inflight_guard = self.try_acquire_inflight()?;
        let request_id = request
            .header
            .as_ref()
            .map(|h| h.request_id.clone())
            .filter(|v| !v.is_empty())
            .ok_or_else(|| IpcError::Protocol("request id required".into()))?;
        let timeout_ms = (timeout.as_millis().min(MAX_REQUEST_DEADLINE_MS as u128) as i64).max(1);
        let cancellation_id = format!("cancel:{request_id}");
        let (tx, rx) = mpsc::sync_channel(1);
        {
            let mut pending = self.pending.lock().unwrap_or_else(|p| p.into_inner());
            if pending.contains_key(&request_id) {
                return Err(IpcError::Protocol(format!(
                    "duplicate in-flight request id: {request_id}"
                )));
            }
            pending.insert(request_id.clone(), tx);
        }
        let frame = ClientFrame {
            payload: Some(client_frame::Payload::Request(SessionRequest {
                request: Some(request),
                deadline_unix_ms: unix_ms().saturating_add(timeout_ms),
                cancellation_id: cancellation_id.clone(),
            })),
        };
        if let Err(e) = self.writer.write(&frame) {
            self.pending
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .remove(&request_id);
            return Err(e);
        }
        let effective_timeout = Duration::from_millis(timeout_ms as u64);
        match rx.recv_timeout(effective_timeout) {
            Ok(resp) => Ok(resp),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                self.pending
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .remove(&request_id);
                let cancel = ClientFrame {
                    payload: Some(client_frame::Payload::Cancel(v1::CancelRequest {
                        cancellation_id,
                    })),
                };
                let _ = self.writer.write(&cancel);
                Err(IpcError::DeadlineExceeded)
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                self.pending
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .remove(&request_id);
                Err(IpcError::Disconnected)
            }
        }
    }
}

impl Drop for SessionClient {
    fn drop(&mut self) {
        self.writer.shutdown();
    }
}

/// Compatibility helper for fixed-purpose one-shot clients such as the elevated consent broker.
/// Desktop UI uses one persistent SessionClient instead.
pub fn connect(request: &Request) -> Result<Response> {
    let client = SessionClient::connect(
        "aethercore-one-shot",
        env!("CARGO_PKG_VERSION"),
        0,
        Arc::new(|_| {}),
        Arc::new(|_| {}),
        Arc::new(|| {}),
    )?;
    client.request(
        request.clone(),
        Duration::from_millis(DEFAULT_REQUEST_DEADLINE_MS as u64),
    )
}

#[cfg(test)]
mod enterprise_tests {
    use super::*;

    #[test]
    fn pipe_open_retry_policy_retries_only_absence_or_busy_conditions() {
        for code in [ERROR_FILE_NOT_FOUND.0 as i32, ERROR_PIPE_BUSY.0 as i32] {
            assert!(retryable_pipe_open_error(
                &std::io::Error::from_raw_os_error(code)
            ));
        }
        for code in [5, 87, 1314] {
            assert!(!retryable_pipe_open_error(
                &std::io::Error::from_raw_os_error(code)
            ));
        }
    }

    #[test]
    fn authenticated_client_access_cannot_create_pipe_instances() {
        const FILE_CREATE_PIPE_INSTANCE: u32 = 0x0000_0004;
        const EXPECTED_CLIENT_ACCESS: u32 = 0x0012_0003;
        assert_eq!(PIPE_CLIENT_ACCESS_MASK, EXPECTED_CLIENT_ACCESS);
        assert_eq!(PIPE_CLIENT_ACCESS_MASK & FILE_CREATE_PIPE_INSTANCE, 0);
        let service_sid = "S-1-5-80-1-2-3-4-5";
        let descriptor = production_pipe_security_descriptor(service_sid);
        // P36 Tranche 1: the AU grant must use NAMED SDDL rights (FR + hex 0x2). The previous
        // hex 0x00120003 form was silently stripped of SYNCHRONIZE by the SDDL parser, making
        // the production pipe unusable. FR|0x2 materializes as 0x12008B: covers the client mask
        // (READ_DATA|WRITE_DATA|READ_CONTROL|SYNCHRONIZE) without FILE_CREATE_PIPE_INSTANCE.
        assert_eq!(
            descriptor,
            "O:S-1-5-80-1-2-3-4-5D:P(A;;GA;;;S-1-5-80-1-2-3-4-5)(A;;FR;;;AU)(A;;0x00000002;;;AU)"
        );
        // The AU ACE must not contain rights beyond the client mask: no append (0x4 bit family),
        // no create-instance, no generic write/all, no trustee expansion to BA/SY.
        assert!(!descriptor.contains("0x00120003")); // broken encoding must not return
        assert!(!descriptor.contains(";;;BA)"));
        assert!(!descriptor.contains(";;;SY)"));
        assert!(!descriptor.contains("(A;;GW;;;AU)"));
        assert!(!descriptor.contains("(A;;GRGW;;;AU)"));
        assert!(!descriptor.contains("(A;;GA;;;AU)"));
    }

    #[test]
    fn named_pipe_client_limits_server_impersonation_to_identification() {
        const SECURITY_IDENTIFICATION: u32 = 0x0001_0000;
        const SECURITY_IMPERSONATION: u32 = 0x0002_0000;
        const SECURITY_DELEGATION: u32 = 0x0003_0000;
        assert_eq!(PIPE_CLIENT_SECURITY_QOS, SECURITY_IDENTIFICATION);
        assert_ne!(PIPE_CLIENT_SECURITY_QOS, SECURITY_IMPERSONATION);
        assert_ne!(PIPE_CLIENT_SECURITY_QOS, SECURITY_DELEGATION);
    }

    #[test]
    fn server_outbound_queue_saturates_fail_closed_without_blocking_request_workers() {
        let (tx, _rx) = mpsc::sync_channel(1);
        let alive = Arc::new(AtomicBool::new(true));
        let queued_bytes = Arc::new(AtomicUsize::new(0));
        let writer = PipeServerWriter {
            tx,
            alive: alive.clone(),
            queued_bytes: queued_bytes.clone(),
            cancel: PipeCancel::noop_for_test(),
        };
        let frame = ServerFrame { payload: None };
        assert!(writer.write(&frame).is_ok());
        assert!(matches!(
            writer.write(&frame),
            Err(IpcError::OutboundBackpressure)
        ));
        assert!(!writer.is_alive());
        assert!(!alive.load(Ordering::Acquire));
        assert_eq!(queued_bytes.load(Ordering::Acquire), frame.encoded_len());
    }

    #[test]
    fn server_outbound_byte_budget_is_fail_closed_even_when_frame_slots_remain() {
        let (tx, _rx) = mpsc::sync_channel(4);
        let alive = Arc::new(AtomicBool::new(true));
        let queued_bytes = Arc::new(AtomicUsize::new(SERVER_OUTBOUND_BYTE_BUDGET));
        let writer = PipeServerWriter {
            tx,
            alive: alive.clone(),
            queued_bytes,
            cancel: PipeCancel::noop_for_test(),
        };
        let frame = ServerFrame {
            payload: Some(server_frame::Payload::Response(Response {
                status_code: 1,
                ..Default::default()
            })),
        };
        assert!(matches!(
            writer.write(&frame),
            Err(IpcError::OutboundBackpressure)
        ));
        assert!(!alive.load(Ordering::Acquire));
    }

    #[test]
    fn client_outbound_queue_saturates_fail_closed_and_requests_transport_cancellation() {
        let (tx, _rx) = mpsc::sync_channel(1);
        let alive = Arc::new(AtomicBool::new(true));
        let queued_bytes = Arc::new(AtomicUsize::new(0));
        let writer = PipeClientWriter {
            tx,
            alive: alive.clone(),
            queued_bytes: queued_bytes.clone(),
            cancel: PipeCancel::noop_for_test(),
        };
        let frame = ClientFrame { payload: None };
        assert!(writer.write(&frame).is_ok());
        assert!(matches!(
            writer.write(&frame),
            Err(IpcError::OutboundBackpressure)
        ));
        assert!(!alive.load(Ordering::Acquire));
        assert_eq!(queued_bytes.load(Ordering::Acquire), frame.encoded_len());
    }

    #[test]
    fn byte_reservation_never_exceeds_limit_under_contention() {
        let counter = Arc::new(AtomicUsize::new(0));
        let mut workers = Vec::new();
        for _ in 0..32 {
            let counter = counter.clone();
            workers.push(thread::spawn(move || try_reserve_bytes(&counter, 1, 7)));
        }
        let admitted = workers
            .into_iter()
            .map(|w| w.join().unwrap())
            .filter(|admitted| *admitted)
            .count();
        assert_eq!(admitted, 7);
        assert_eq!(counter.load(Ordering::Acquire), 7);
    }
    #[test]
    fn client_inflight_admission_matches_server_advertised_limit() {
        let inflight = AtomicUsize::new(0);
        assert!(try_acquire_slot(&inflight, 2));
        assert!(try_acquire_slot(&inflight, 2));
        assert!(!try_acquire_slot(&inflight, 2));
        inflight.fetch_sub(1, Ordering::AcqRel);
        assert!(try_acquire_slot(&inflight, 2));
        assert_eq!(inflight.load(Ordering::Acquire), 2);
    }

    #[test]
    fn disconnect_notification_is_at_most_once_across_reader_and_writer_paths() {
        let notified = AtomicBool::new(false);
        let calls = Arc::new(AtomicUsize::new(0));
        let callback_calls = calls.clone();
        let callback: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
            callback_calls.fetch_add(1, Ordering::AcqRel);
        });
        notify_disconnect_once(&notified, &callback);
        notify_disconnect_once(&notified, &callback);
        assert_eq!(calls.load(Ordering::Acquire), 1);
    }
}
