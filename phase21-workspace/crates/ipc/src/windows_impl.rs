use std::{
    collections::HashMap,
    ffi::c_void,
    fs::File,
    os::windows::{
        fs::OpenOptionsExt,
        io::{AsRawHandle, FromRawHandle},
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
            CloseHandle, ERROR_FILE_NOT_FOUND, ERROR_INSUFFICIENT_BUFFER, ERROR_PIPE_BUSY,
            ERROR_PIPE_CONNECTED, HANDLE, HLOCAL, INVALID_HANDLE_VALUE, LocalFree,
        },
        Security::{
            Authorization::{
                ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
                GetSecurityInfo, SDDL_REVISION_1, SE_FILE_OBJECT,
            },
            IsValidSid, LookupAccountNameW, OWNER_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID,
            SECURITY_ATTRIBUTES, SID_NAME_USE,
        },
        Storage::FileSystem::{FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_DUPLEX},
        System::{
            IO::CancelSynchronousIo,
            Pipes::{
                ConnectNamedPipe, CreateNamedPipeW, PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS,
                PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
            },
            Services::{
                OpenSCManagerW, OpenServiceW, QueryServiceStatusEx, SC_MANAGER_CONNECT,
                SC_STATUS_PROCESS_INFO, SERVICE_QUERY_STATUS, SERVICE_RUNNING,
                SERVICE_STATUS_PROCESS, SERVICE_WIN32_OWN_PROCESS,
            },
            Threading::{GetCurrentThreadId, OpenThread, THREAD_TERMINATE},
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
const TRUSTED_SERVICE_NAME: &str = "AetherCoreMaintenance";
const TRUSTED_SERVICE_ACCOUNT: &str = r"NT SERVICE\AetherCoreMaintenance";
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

    let mut sid = lookup_account_sid(TRUSTED_SERVICE_ACCOUNT)?;
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

/// Owns a real HANDLE to a writer thread so another thread can cancel a synchronous pipe write.
///
/// `windows::Win32::Foundation::HANDLE` is intentionally `!Send + !Sync`; storing the opaque
/// numeric value here keeps the cross-thread contract explicit. The handle was opened with only
/// `THREAD_TERMINATE`, which is the access right required by `CancelSynchronousIo`.
struct SyncIoCancellation {
    thread_handle: usize,
}

impl SyncIoCancellation {
    fn for_thread(thread_id: u32) -> Result<Arc<Self>> {
        let handle =
            unsafe { OpenThread(THREAD_TERMINATE, false, thread_id) }.map_err(|error| {
                IpcError::Windows(format!("open IPC I/O thread for cancellation: {error}"))
            })?;
        Ok(Arc::new(Self {
            thread_handle: handle.0 as usize,
        }))
    }

    fn cancel(&self) {
        if self.thread_handle == 0 {
            return;
        }
        let handle = HANDLE(self.thread_handle as *mut c_void);
        unsafe {
            let _ = CancelSynchronousIo(handle);
        }
    }

    #[cfg(test)]
    fn noop_for_test() -> Arc<Self> {
        Arc::new(Self { thread_handle: 0 })
    }
}

impl Drop for SyncIoCancellation {
    fn drop(&mut self) {
        if self.thread_handle == 0 {
            return;
        }
        let handle = HANDLE(self.thread_handle as *mut c_void);
        unsafe {
            let _ = CloseHandle(handle);
        }
    }
}

fn writer_thread_id(receiver: mpsc::Receiver<u32>) -> Result<u32> {
    match receiver.recv_timeout(Duration::from_secs(1)) {
        Ok(thread_id) => Ok(thread_id),
        Err(mpsc::RecvTimeoutError::Timeout) => Err(IpcError::Io(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            "IPC I/O thread registration timed out",
        ))),
        Err(mpsc::RecvTimeoutError::Disconnected) => Err(IpcError::Io(std::io::Error::new(
            std::io::ErrorKind::BrokenPipe,
            "IPC I/O thread exited before cancellation registration",
        ))),
    }
}

fn cancel_registered_io(slot: &Mutex<Option<Arc<SyncIoCancellation>>>) {
    let cancellation = slot.lock().unwrap_or_else(|p| p.into_inner()).clone();
    if let Some(cancellation) = cancellation {
        cancellation.cancel();
    }
}

#[derive(Clone)]
pub struct PipeServerWriter {
    tx: mpsc::SyncSender<QueuedServerFrame>,
    alive: Arc<AtomicBool>,
    queued_bytes: Arc<AtomicUsize>,
    cancellation: Arc<SyncIoCancellation>,
    peer_reader: Arc<Mutex<Option<Arc<SyncIoCancellation>>>>,
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
        self.cancellation.cancel();
        cancel_registered_io(&self.peer_reader);
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
            let open_mode = if first_instance {
                PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE
            } else {
                PIPE_ACCESS_DUPLEX
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
        unsafe {
            match ConnectNamedPipe(self.handle.get(), None) {
                Ok(()) => {}
                Err(e) if e.code() == ERROR_PIPE_CONNECTED.to_hresult() => {}
                Err(e) => return Err(IpcError::Windows(e.to_string())),
            }
            PipeServerSession::from_connected_reader(File::from_raw_handle(
                self.handle.into_raw().0,
            ))
        }
    }
}

pub struct PipeServerSession {
    reader: File,
    writer: PipeServerWriter,
    reader_cancellation: Arc<Mutex<Option<Arc<SyncIoCancellation>>>>,
}
impl PipeServerSession {
    fn from_connected_reader(reader: File) -> Result<Self> {
        let mut writer_file = reader.try_clone()?;
        let (tx, rx) = mpsc::sync_channel::<QueuedServerFrame>(SERVER_OUTBOUND_QUEUE_CAPACITY);
        let writer_alive = Arc::new(AtomicBool::new(true));
        let queued_bytes = Arc::new(AtomicUsize::new(0));
        let reader_cancellation = Arc::new(Mutex::new(None));
        let pump_alive = writer_alive.clone();
        let pump_bytes = queued_bytes.clone();
        let pump_peer_reader = reader_cancellation.clone();
        let (thread_id_tx, thread_id_rx) = mpsc::sync_channel(1);
        let writer_thread = thread::Builder::new()
            .name("aether-ipc-server-writer".into())
            .spawn(move || {
                if thread_id_tx.send(unsafe { GetCurrentThreadId() }).is_err() {
                    return;
                }
                while let Ok(queued) = rx.recv() {
                    if !pump_alive.load(Ordering::Acquire) {
                        release_bytes(&pump_bytes, queued.bytes);
                        break;
                    }
                    let result = write_server_frame(&mut writer_file, &queued.frame);
                    release_bytes(&pump_bytes, queued.bytes);
                    if result.is_err() {
                        break;
                    }
                }
                pump_alive.store(false, Ordering::Release);
                cancel_registered_io(&pump_peer_reader);
            })
            .map_err(IpcError::Io)?;
        let thread_id = match writer_thread_id(thread_id_rx) {
            Ok(value) => value,
            Err(error) => {
                writer_alive.store(false, Ordering::Release);
                drop(tx);
                let _ = writer_thread.join();
                return Err(error);
            }
        };
        let cancellation = match SyncIoCancellation::for_thread(thread_id) {
            Ok(value) => value,
            Err(error) => {
                writer_alive.store(false, Ordering::Release);
                drop(tx);
                let _ = writer_thread.join();
                return Err(error);
            }
        };
        drop(writer_thread);
        Ok(Self {
            reader,
            writer: PipeServerWriter {
                tx,
                alive: writer_alive,
                queued_bytes,
                cancellation,
                peer_reader: reader_cancellation.clone(),
            },
            reader_cancellation,
        })
    }
    /// Bind the blocking read side to the session worker so writer-side failure can cancel it.
    /// Must be called exactly once by the thread that will execute `read`.
    pub fn bind_reader_to_current_thread(&self) -> Result<()> {
        let cancellation = SyncIoCancellation::for_thread(unsafe { GetCurrentThreadId() })?;
        let mut slot = self
            .reader_cancellation
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        if slot.is_some() {
            return Err(IpcError::Protocol(
                "server reader thread already bound".into(),
            ));
        }
        *slot = Some(cancellation);
        Ok(())
    }
    pub fn raw_handle(&self) -> std::os::windows::io::RawHandle {
        self.reader.as_raw_handle()
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
    cancellation: Arc<SyncIoCancellation>,
    peer_reader: Arc<Mutex<Option<Arc<SyncIoCancellation>>>>,
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
        self.cancellation.cancel();
        cancel_registered_io(&self.peer_reader);
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
        let mut reader = open_pipe()?;
        let mut writer_file = reader.try_clone()?;
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
        let reader_cancellation = Arc::new(Mutex::new(None));
        let writer_alive = alive.clone();
        let writer_pending = pending.clone();
        let writer_notified = disconnect_notified.clone();
        let writer_disconnect = on_disconnect.clone();
        let writer_peer_reader = reader_cancellation.clone();
        let pump_bytes = writer_bytes.clone();
        let (thread_id_tx, thread_id_rx) = mpsc::sync_channel(1);
        let writer_thread = thread::Builder::new()
            .name("aether-ipc-client-writer".into())
            .spawn(move || {
                if thread_id_tx.send(unsafe { GetCurrentThreadId() }).is_err() {
                    return;
                }
                while let Ok(queued) = writer_rx.recv() {
                    if !writer_alive.load(Ordering::Acquire) {
                        release_bytes(&pump_bytes, queued.bytes);
                        while let Ok(discarded) = writer_rx.try_recv() {
                            release_bytes(&pump_bytes, discarded.bytes);
                        }
                        break;
                    }
                    let result = write_client_frame(&mut writer_file, &queued.frame);
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
                cancel_registered_io(&writer_peer_reader);
                notify_disconnect_once(&writer_notified, &writer_disconnect);
            })
            .map_err(IpcError::Io)?;
        let thread_id = match writer_thread_id(thread_id_rx) {
            Ok(value) => value,
            Err(error) => {
                alive.store(false, Ordering::Release);
                drop(writer_tx);
                let _ = writer_thread.join();
                return Err(error);
            }
        };
        let cancellation = match SyncIoCancellation::for_thread(thread_id) {
            Ok(value) => value,
            Err(error) => {
                alive.store(false, Ordering::Release);
                drop(writer_tx);
                let _ = writer_thread.join();
                return Err(error);
            }
        };
        let reader_alive = alive.clone();
        let reader_pending = pending.clone();
        let reader_notified = disconnect_notified.clone();
        let reader_disconnect = on_disconnect.clone();
        let reader_writer_cancellation = cancellation.clone();
        let (reader_thread_id_tx, reader_thread_id_rx) = mpsc::sync_channel(1);
        let (reader_start_tx, reader_start_rx) = mpsc::channel();
        let reader_thread = match thread::Builder::new()
            .name("aether-ipc-client-reader".into())
            .spawn(move || {
                if reader_thread_id_tx
                    .send(unsafe { GetCurrentThreadId() })
                    .is_err()
                {
                    return;
                }
                if reader_start_rx.recv().is_err() {
                    return;
                }
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
                reader_writer_cancellation.cancel();
                notify_disconnect_once(&reader_notified, &reader_disconnect);
            }) {
            Ok(value) => value,
            Err(error) => {
                alive.store(false, Ordering::Release);
                cancellation.cancel();
                drop(writer_tx);
                let _ = writer_thread.join();
                return Err(IpcError::Io(error));
            }
        };
        let reader_thread_id = match writer_thread_id(reader_thread_id_rx) {
            Ok(value) => value,
            Err(error) => {
                alive.store(false, Ordering::Release);
                cancellation.cancel();
                drop(reader_start_tx);
                drop(writer_tx);
                let _ = writer_thread.join();
                let _ = reader_thread.join();
                return Err(error);
            }
        };
        let reader_cancel = match SyncIoCancellation::for_thread(reader_thread_id) {
            Ok(value) => value,
            Err(error) => {
                alive.store(false, Ordering::Release);
                cancellation.cancel();
                drop(reader_start_tx);
                drop(writer_tx);
                let _ = writer_thread.join();
                let _ = reader_thread.join();
                return Err(error);
            }
        };
        *reader_cancellation
            .lock()
            .unwrap_or_else(|p| p.into_inner()) = Some(reader_cancel);
        if reader_start_tx.send(()).is_err() {
            alive.store(false, Ordering::Release);
            cancellation.cancel();
            drop(writer_tx);
            let _ = writer_thread.join();
            let _ = reader_thread.join();
            return Err(IpcError::Disconnected);
        };
        let max_inflight = (hello.max_inflight_requests as usize).max(1);
        let client = Arc::new(Self {
            writer: PipeClientWriter {
                tx: writer_tx,
                alive: alive.clone(),
                queued_bytes: writer_bytes,
                cancellation,
                peer_reader: reader_cancellation,
            },
            pending: pending.clone(),
            alive: alive.clone(),
            inflight: AtomicUsize::new(0),
            max_inflight,
            hello,
        });
        drop(writer_thread);
        drop(reader_thread);
        Ok(client)
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
            cancellation: SyncIoCancellation::noop_for_test(),
            peer_reader: Arc::new(Mutex::new(None)),
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
            cancellation: SyncIoCancellation::noop_for_test(),
            peer_reader: Arc::new(Mutex::new(None)),
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
            cancellation: SyncIoCancellation::noop_for_test(),
            peer_reader: Arc::new(Mutex::new(None)),
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
