use std::{
    fs,
    mem::{align_of, size_of},
    path::{Path, PathBuf},
    sync::OnceLock,
};

use aethercore_collector_runtime::{
    CancellationToken, CollectorControl, CollectorFault, CollectorFaultRecord,
    DEFAULT_COLLECTOR_TIMEOUT, EVENTLOG_NEXT_SLICE, FaultKind, IsolationGate,
    run_isolated_gated_with_token,
};
use chrono::{DateTime, Utc};
use windows::{
    Win32::{
        Foundation::{
            E_ACCESSDENIED, ERROR_INSUFFICIENT_BUFFER, ERROR_NO_MORE_ITEMS, ERROR_TIMEOUT,
        },
        System::{
            Diagnostics::Debug::{DUMP_HEADER32, DUMP_HEADER64},
            EventLog::{
                EVT_HANDLE, EVT_VARIANT, EvtClose, EvtCreateRenderContext, EvtNext, EvtQuery,
                EvtQueryChannelPath, EvtQueryReverseDirection, EvtRender, EvtRenderContextSystem,
                EvtRenderContextUser, EvtRenderEventValues,
            },
        },
    },
    core::PCWSTR,
};

use crate::{
    CrashDiagnosticsSnapshot, CrashError, CrashRecord, DEFAULT_EVENT_WINDOW_DAYS, EventEvidence,
    Result, classify_event,
};

const MAX_EVENTS: usize = 128;
const MAX_DUMPS: usize = 64;
const MAX_SYSTEM_RENDER_BYTES: usize = 64 * 1024;
const MAX_USER_RENDER_BYTES: usize = 256 * 1024;
const MAX_EVENT_PROPERTIES: usize = 256;
const MAX_EVENT_STRING_UTF16: usize = 16 * 1024;
const EVENT_WINDOW_MS: u64 = DEFAULT_EVENT_WINDOW_DAYS as u64 * 24 * 60 * 60 * 1000;
const FILETIME_UNIX_EPOCH_TICKS: u64 = 116_444_736_000_000_000;

static EVENTLOG_GATE: OnceLock<IsolationGate> = OnceLock::new();
static MINIDUMP_GATE: OnceLock<IsolationGate> = OnceLock::new();
fn eventlog_gate() -> &'static IsolationGate {
    EVENTLOG_GATE.get_or_init(IsolationGate::default)
}
fn minidump_gate() -> &'static IsolationGate {
    MINIDUMP_GATE.get_or_init(IsolationGate::default)
}

struct EventHandle(EVT_HANDLE);
impl Drop for EventHandle {
    fn drop(&mut self) {
        let _ = unsafe { EvtClose(self.0) };
    }
}

#[derive(Debug)]
struct RenderedEventSystem {
    provider: String,
    event_id: u32,
    recorded_unix_ms: i64,
}

struct RenderBuffer {
    words: Vec<u64>,
    used: usize,
    property_count: usize,
}

impl RenderBuffer {
    fn variants(&self) -> Result<&[EVT_VARIANT]> {
        let bytes = self
            .property_count
            .checked_mul(size_of::<EVT_VARIANT>())
            .ok_or_else(|| {
                CrashError::MalformedResponse("EvtRender property-count overflow".into())
            })?;
        if bytes > self.used || self.property_count > MAX_EVENT_PROPERTIES {
            return Err(CrashError::MalformedResponse(
                "EvtRender returned an invalid property count for its buffer".into(),
            ));
        }
        let address = self.words.as_ptr() as usize;
        if address % align_of::<EVT_VARIANT>() != 0 {
            return Err(CrashError::MalformedResponse(
                "EvtRender buffer was not aligned for EVT_VARIANT".into(),
            ));
        }
        Ok(unsafe {
            std::slice::from_raw_parts(
                self.words.as_ptr().cast::<EVT_VARIANT>(),
                self.property_count,
            )
        })
    }

    fn byte_range(&self) -> (*const u8, *const u8) {
        let start = self.words.as_ptr().cast::<u8>();
        let end = unsafe { start.add(self.used) };
        (start, end)
    }
}

pub fn collect() -> Result<CrashDiagnosticsSnapshot> {
    collect_with_cancellation(CancellationToken::new())
}

pub fn collect_with_cancellation(parent: CancellationToken) -> Result<CrashDiagnosticsSnapshot> {
    let mut warnings = Vec::new();
    let mut provider_faults = Vec::new();

    let events = match run_isolated_gated_with_token(
        eventlog_gate(),
        "crash-diagnostics",
        "eventlog",
        DEFAULT_COLLECTOR_TIMEOUT,
        parent.child(),
        |control| collect_events(&control).map_err(|error| collector_fault("eventlog", error)),
    ) {
        Ok((events, event_warnings, event_faults)) => {
            warnings.extend(event_warnings);
            provider_faults.extend(event_faults);
            events
        }
        Err(error) => {
            warnings.push(format!("Windows Event Log collection unavailable: {error}"));
            provider_faults.push(CollectorFaultRecord::from(&error));
            Vec::new()
        }
    };

    let crashes = match run_isolated_gated_with_token(
        minidump_gate(),
        "crash-diagnostics",
        "minidumps",
        DEFAULT_COLLECTOR_TIMEOUT,
        parent.child(),
        |control| collect_minidumps(&control).map_err(|error| collector_fault("minidumps", error)),
    ) {
        Ok((crashes, dump_warnings, dump_faults)) => {
            warnings.extend(dump_warnings);
            provider_faults.extend(dump_faults);
            crashes
        }
        Err(error) => {
            warnings.push(format!("Minidump metadata unavailable: {error}"));
            provider_faults.push(CollectorFaultRecord::from(&error));
            Vec::new()
        }
    };

    Ok(CrashDiagnosticsSnapshot {
        event_window_days: DEFAULT_EVENT_WINDOW_DAYS,
        events,
        crashes,
        provider_faults,
        warnings,
    })
}

fn collector_fault(operation: &'static str, error: CrashError) -> CollectorFault {
    let kind = match &error {
        CrashError::Timeout(_) => FaultKind::Timeout,
        CrashError::Cancelled(_) => FaultKind::Cancelled,
        CrashError::Unavailable(_) => FaultKind::Unavailable,
        CrashError::PermissionDenied(_) => FaultKind::PermissionDenied,
        CrashError::MalformedResponse(_) => FaultKind::MalformedResponse,
        CrashError::Io(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
            FaultKind::PermissionDenied
        }
        CrashError::Io(_) => FaultKind::Io,
        CrashError::Windows(_) => FaultKind::ProviderFailure,
    };
    CollectorFault::new("crash-diagnostics", operation, kind, error.to_string())
}

fn checkpoint(control: &CollectorControl, operation: &'static str) -> Result<()> {
    control
        .checkpoint("crash-diagnostics", operation)
        .map_err(|fault| match fault.kind {
            FaultKind::Timeout => CrashError::Timeout(fault.detail),
            FaultKind::Cancelled => CrashError::Cancelled(fault.detail),
            _ => CrashError::Unavailable(fault.detail),
        })
}

fn collect_events(
    control: &CollectorControl,
) -> Result<(Vec<EventEvidence>, Vec<String>, Vec<CollectorFaultRecord>)> {
    checkpoint(control, "eventlog.begin")?;
    let channel = w("System");
    let query = w(&format!(
        "*[System[(Provider[@Name='Microsoft-Windows-WHEA-Logger'] or Provider[@Name='Microsoft-Windows-Kernel-Power'] or Provider[@Name='Microsoft-Windows-WER-SystemErrorReporting']) and TimeCreated[timediff(@SystemTime) <= {EVENT_WINDOW_MS}]]]"
    ));
    let result = unsafe {
        EvtQuery(
            None,
            PCWSTR(channel.as_ptr()),
            PCWSTR(query.as_ptr()),
            EvtQueryChannelPath.0 | EvtQueryReverseDirection.0,
        )
    }
    .map_err(win)?;
    let result = EventHandle(result);
    let system_context = EventHandle(
        unsafe { EvtCreateRenderContext(None, EvtRenderContextSystem.0 as u32) }.map_err(win)?,
    );
    let user_context = EventHandle(
        unsafe { EvtCreateRenderContext(None, EvtRenderContextUser.0 as u32) }.map_err(win)?,
    );

    let mut out = Vec::new();
    let mut malformed_events = 0usize;
    while out.len() < MAX_EVENTS {
        checkpoint(control, "eventlog.next")?;
        let mut handles = [0isize; 16];
        let mut returned = 0u32;
        let timeout_ms = control.remaining_ms_capped(EVENTLOG_NEXT_SLICE);
        match unsafe { EvtNext(result.0, &mut handles, timeout_ms, 0, &mut returned) } {
            Ok(()) => {}
            Err(error) if error.code() == ERROR_TIMEOUT.to_hresult() => continue,
            Err(error) if error.code() == ERROR_NO_MORE_ITEMS.to_hresult() => break,
            Err(error) => return Err(win(error)),
        }
        let returned = usize::try_from(returned).map_err(|_| {
            CrashError::MalformedResponse("EvtNext handle count did not fit usize".into())
        })?;

        // Take ownership of every non-null handle returned in the fixed output array before any
        // cancellable/rendering work. If the provider reports an inconsistent count, the owned
        // handles are still dropped on this path instead of leaking Event Log handles.
        let mut owned = handles
            .into_iter()
            .enumerate()
            .filter_map(|(index, raw)| (raw != 0).then_some((index, EventHandle(EVT_HANDLE(raw)))))
            .collect::<Vec<_>>();
        if returned > 16 {
            return Err(CrashError::MalformedResponse(
                "EvtNext reported more event handles than the bounded output array".into(),
            ));
        }
        if owned.len() != returned
            || owned
                .iter()
                .enumerate()
                .any(|(expected, (actual, _))| *actual != expected)
        {
            return Err(CrashError::MalformedResponse(
                "EvtNext returned null, sparse, or trailing handles inconsistent with its reported count".into(),
            ));
        }
        if returned == 0 {
            return Err(CrashError::MalformedResponse(
                "EvtNext succeeded without returning an event handle".into(),
            ));
        }
        let events = owned.drain(..).map(|(_, event)| event).collect::<Vec<_>>();

        for event in events {
            checkpoint(control, "eventlog.render")?;
            match render_system(system_context.0, event.0) {
                Ok(system) => {
                    let payload = match render_user_values(user_context.0, event.0) {
                        Ok(values) => values,
                        Err(_) => {
                            malformed_events = malformed_events.saturating_add(1);
                            Vec::new()
                        }
                    };
                    out.push(classify_event(
                        &system.provider,
                        system.event_id,
                        &payload,
                        system.recorded_unix_ms,
                    ));
                }
                Err(_) => malformed_events = malformed_events.saturating_add(1),
            }
            if out.len() >= MAX_EVENTS {
                break;
            }
        }
    }

    let mut warnings = Vec::new();
    let mut provider_faults = Vec::new();
    if malformed_events > 0 {
        let detail = format!(
            "{malformed_events} Windows event(s) were skipped because their structured render payload was malformed or exceeded safety bounds."
        );
        warnings.push(detail.clone());
        provider_faults.push(CollectorFaultRecord::new(
            "crash-diagnostics",
            "eventlog.render",
            FaultKind::MalformedResponse,
            detail,
        ));
    }
    Ok((out, warnings, provider_faults))
}

fn render_system(context: EVT_HANDLE, event: EVT_HANDLE) -> Result<RenderedEventSystem> {
    // EvtRenderContextSystem returns EVT_VARIANT entries in EVT_SYSTEM_PROPERTY_ID order.
    // Indices: ProviderName=0, EventID=2, TimeCreated=8.
    let buffer = render_values(context, event, MAX_SYSTEM_RENDER_BYTES)?;
    let variants = buffer.variants()?;
    if variants.len() <= 8 {
        return Err(CrashError::MalformedResponse(
            "EvtRender system context omitted required properties".into(),
        ));
    }
    let provider = variant_unicode_string(&variants[0], &buffer)?
        .ok_or_else(|| CrashError::MalformedResponse("event provider name was absent".into()))?;
    let event_id = variant_u32_lossless(&variants[2]).ok_or_else(|| {
        CrashError::MalformedResponse("event ID had an unexpected EVT_VARIANT type".into())
    })?;
    let filetime = variant_filetime(&variants[8]).ok_or_else(|| {
        CrashError::MalformedResponse("event timestamp had an unexpected EVT_VARIANT type".into())
    })?;
    Ok(RenderedEventSystem {
        provider,
        event_id,
        recorded_unix_ms: filetime_to_unix_ms(filetime),
    })
}

fn render_user_values(context: EVT_HANDLE, event: EVT_HANDLE) -> Result<Vec<String>> {
    let buffer = render_values(context, event, MAX_USER_RENDER_BYTES)?;
    let variants = buffer.variants()?;
    let mut values = Vec::new();
    for variant in variants.iter().take(MAX_EVENT_PROPERTIES) {
        if let Some(value) = variant_to_bounded_text(variant, &buffer)? {
            if !value.is_empty() {
                values.push(value);
            }
        }
    }
    Ok(values)
}

fn checked_render_property_count(properties: u32) -> Result<usize> {
    let count = usize::try_from(properties).map_err(|_| {
        CrashError::MalformedResponse("EvtRender property count did not fit usize".into())
    })?;
    if count > MAX_EVENT_PROPERTIES {
        return Err(CrashError::MalformedResponse(format!(
            "EvtRender reported {count} properties, exceeding the {MAX_EVENT_PROPERTIES}-property safety cap"
        )));
    }
    Ok(count)
}

fn render_values(context: EVT_HANDLE, event: EVT_HANDLE, max_bytes: usize) -> Result<RenderBuffer> {
    let mut used = 0u32;
    let mut properties = 0u32;
    let first = unsafe {
        EvtRender(
            Some(context),
            event,
            EvtRenderEventValues.0 as u32,
            0,
            None,
            &mut used,
            &mut properties,
        )
    };
    if let Err(error) = first {
        if error.code() != ERROR_INSUFFICIENT_BUFFER.to_hresult() {
            return Err(win(error));
        }
    }
    // Property count is untrusted provider output. Reject pathological counts before allocating
    // the render buffer; the byte cap alone does not express this semantic invariant.
    let property_count = checked_render_property_count(properties)?;
    if used == 0 {
        return Ok(RenderBuffer {
            words: Vec::new(),
            used: 0,
            property_count,
        });
    }
    let used_usize = usize::try_from(used)
        .map_err(|_| CrashError::MalformedResponse("EvtRender size did not fit usize".into()))?;
    if used_usize > max_bytes {
        return Err(CrashError::MalformedResponse(format!(
            "EvtRender requested {used_usize} bytes, exceeding the {max_bytes}-byte safety cap"
        )));
    }
    let word_count = used_usize.checked_add(7).ok_or_else(|| {
        CrashError::MalformedResponse("EvtRender allocation size overflow".into())
    })? / 8;
    let mut words = vec![0u64; word_count];
    let capacity_bytes = words.len() * size_of::<u64>();
    let mut actual_used = used;
    let mut actual_properties = properties;
    unsafe {
        EvtRender(
            Some(context),
            event,
            EvtRenderEventValues.0 as u32,
            used,
            Some(words.as_mut_ptr().cast()),
            &mut actual_used,
            &mut actual_properties,
        )
    }
    .map_err(win)?;
    let actual_used = usize::try_from(actual_used).map_err(|_| {
        CrashError::MalformedResponse("EvtRender returned size did not fit usize".into())
    })?;
    if actual_used > capacity_bytes || actual_used > max_bytes {
        return Err(CrashError::MalformedResponse(
            "EvtRender returned more bytes than the validated output buffer".into(),
        ));
    }
    let actual_property_count = checked_render_property_count(actual_properties)?;
    Ok(RenderBuffer {
        words,
        used: actual_used,
        property_count: actual_property_count,
    })
}

fn variant_base_type(variant: &EVT_VARIANT) -> u32 {
    variant.Type & 0x7f
}
fn variant_is_array(variant: &EVT_VARIANT) -> bool {
    (variant.Type & 0x80) != 0
}

fn variant_unicode_string(variant: &EVT_VARIANT, buffer: &RenderBuffer) -> Result<Option<String>> {
    if variant_is_array(variant) || !matches!(variant_base_type(variant), 1 | 35) {
        return Ok(None);
    }
    let ptr = unsafe {
        if variant_base_type(variant) == 35 {
            variant.Anonymous.XmlVal.0
        } else {
            variant.Anonymous.StringVal.0
        }
    };
    if ptr.is_null() {
        return Ok(None);
    }
    let (start, end) = buffer.byte_range();
    let address = ptr.cast::<u8>();
    if address < start || address >= end {
        return Err(CrashError::MalformedResponse(
            "EVT_VARIANT string pointer escaped its render buffer".into(),
        ));
    }
    if (address as usize) % align_of::<u16>() != 0 {
        return Err(CrashError::MalformedResponse(
            "EVT_VARIANT UTF-16 string pointer was misaligned".into(),
        ));
    }
    let remaining_bytes = (end as usize).saturating_sub(address as usize);
    let max_units = (remaining_bytes / 2).min(MAX_EVENT_STRING_UTF16);
    let units = unsafe { std::slice::from_raw_parts(ptr, max_units) };
    let Some(length) = units.iter().position(|unit| *unit == 0) else {
        return Err(CrashError::MalformedResponse(
            "EVT_VARIANT string was not null terminated inside its bounded render buffer".into(),
        ));
    };
    Ok(Some(String::from_utf16_lossy(&units[..length])))
}

fn variant_u32_lossless(variant: &EVT_VARIANT) -> Option<u32> {
    if variant_is_array(variant) {
        return None;
    }
    unsafe {
        match variant_base_type(variant) {
            4 => Some(u32::from(variant.Anonymous.ByteVal)),
            6 => Some(u32::from(variant.Anonymous.UInt16Val)),
            8 | 20 => Some(variant.Anonymous.UInt32Val),
            10 | 21 => u32::try_from(variant.Anonymous.UInt64Val).ok(),
            _ => None,
        }
    }
}

fn variant_filetime(variant: &EVT_VARIANT) -> Option<u64> {
    if variant_is_array(variant) || variant_base_type(variant) != 17 {
        return None;
    }
    Some(unsafe { variant.Anonymous.FileTimeVal })
}

fn variant_to_bounded_text(variant: &EVT_VARIANT, buffer: &RenderBuffer) -> Result<Option<String>> {
    if variant_is_array(variant) {
        return Ok(None);
    }
    if let Some(value) = variant_unicode_string(variant, buffer)? {
        return Ok(Some(value));
    }
    let value = unsafe {
        match variant_base_type(variant) {
            0 => return Ok(None),
            3 => variant.Anonymous.SByteVal.to_string(),
            4 => variant.Anonymous.ByteVal.to_string(),
            5 => variant.Anonymous.Int16Val.to_string(),
            6 => variant.Anonymous.UInt16Val.to_string(),
            7 => variant.Anonymous.Int32Val.to_string(),
            8 => variant.Anonymous.UInt32Val.to_string(),
            9 => variant.Anonymous.Int64Val.to_string(),
            10 => variant.Anonymous.UInt64Val.to_string(),
            13 => (variant.Anonymous.BooleanVal.0 != 0).to_string(),
            20 => format!("0x{:08X}", variant.Anonymous.UInt32Val),
            21 => format!("0x{:016X}", variant.Anonymous.UInt64Val),
            // Binary/GUID/SID/array payloads are intentionally not decoded here. They may be
            // large or provider-specific; retaining system metadata is safer than guessing.
            _ => return Ok(None),
        }
    };
    Ok(Some(value))
}

fn filetime_to_unix_ms(filetime: u64) -> i64 {
    if filetime <= FILETIME_UNIX_EPOCH_TICKS {
        return 0;
    }
    let millis = (filetime - FILETIME_UNIX_EPOCH_TICKS) / 10_000;
    i64::try_from(millis).unwrap_or(i64::MAX)
}

fn io_fault_kind(error: &std::io::Error) -> FaultKind {
    if error.kind() == std::io::ErrorKind::PermissionDenied {
        FaultKind::PermissionDenied
    } else {
        FaultKind::Io
    }
}

fn collect_minidumps(
    control: &CollectorControl,
) -> Result<(Vec<CrashRecord>, Vec<String>, Vec<CollectorFaultRecord>)> {
    checkpoint(control, "minidump.begin")?;
    let root = std::env::var_os("SystemRoot")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
    let dir = root.join("Minidump");
    if !dir.exists() {
        return Ok((Vec::new(), Vec::new(), Vec::new()));
    }

    const MAX_MINIDUMP_FAULTS: usize = 16;
    let mut faults = Vec::new();
    let mut warnings = Vec::new();
    let mut entries = Vec::new();
    for entry in fs::read_dir(&dir)? {
        checkpoint(control, "minidump.enumerate")?;
        match entry {
            Ok(entry) => {
                let path = entry.path();
                if !path.extension().is_some_and(|extension| {
                    extension.to_string_lossy().eq_ignore_ascii_case("dmp")
                }) {
                    continue;
                }
                match entry.metadata() {
                    Ok(meta) => entries.push((path, meta.modified().ok())),
                    Err(error) => push_minidump_fault(
                        &mut faults,
                        "minidump.metadata",
                        io_fault_kind(&error),
                        format!("could not inspect minidump metadata: {error}"),
                        MAX_MINIDUMP_FAULTS,
                    ),
                }
            }
            Err(error) => push_minidump_fault(
                &mut faults,
                "minidump.enumerate",
                io_fault_kind(&error),
                format!("could not enumerate a minidump directory entry: {error}"),
                MAX_MINIDUMP_FAULTS,
            ),
        }
    }
    entries.sort_by_key(|(_, modified)| std::cmp::Reverse(modified.clone()));
    entries.truncate(MAX_DUMPS);

    let mut out = Vec::new();
    for (path, _) in entries {
        checkpoint(control, "minidump.parse")?;
        match parse_dump(&path) {
            Ok(record) => out.push(record),
            Err(error) => {
                let name = path
                    .file_name()
                    .map(|v| v.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "<unknown>.dmp".into());
                let kind = match &error {
                    CrashError::MalformedResponse(_) => FaultKind::MalformedResponse,
                    CrashError::Io(error) => io_fault_kind(error),
                    CrashError::PermissionDenied(_) => FaultKind::PermissionDenied,
                    CrashError::Timeout(_) => FaultKind::Timeout,
                    CrashError::Cancelled(_) => FaultKind::Cancelled,
                    CrashError::Unavailable(_) => FaultKind::Unavailable,
                    CrashError::Windows(_) => FaultKind::ProviderFailure,
                };
                push_minidump_fault(
                    &mut faults,
                    "minidump.parse",
                    kind,
                    format!("{name}: {error}"),
                    MAX_MINIDUMP_FAULTS,
                );
            }
        }
    }

    if !faults.is_empty() {
        warnings.push(format!(
            "{} minidump metadata item(s) could not be collected safely; usable crash evidence was retained.",
            faults.len()
        ));
    }
    Ok((out, warnings, faults))
}

fn push_minidump_fault(
    faults: &mut Vec<CollectorFaultRecord>,
    operation: &str,
    kind: FaultKind,
    detail: String,
    max: usize,
) {
    if faults.len() >= max {
        return;
    }
    faults.push(CollectorFaultRecord::new(
        "crash-diagnostics",
        operation,
        kind,
        detail,
    ));
}

fn parse_dump(path: &Path) -> Result<CrashRecord> {
    use std::io::Read;
    let meta = fs::metadata(path)?;
    let file = fs::File::open(path)?;
    let header_bytes = size_of::<DUMP_HEADER64>().max(size_of::<DUMP_HEADER32>());
    let mut header_data = Vec::with_capacity(header_bytes);
    file.take(header_bytes as u64)
        .read_to_end(&mut header_data)?;
    // DBT-P46-B6: no .unwrap_or(0) — a failed mtime read stays None rather than
    // becoming a 1970 timestamp the UI would render as a real crash date.
    let recorded = meta
        .modified()
        .ok()
        .map(DateTime::<Utc>::from)
        .map(|date| date.timestamp_millis());
    let mut code = None;
    let mut parameters = Vec::new();
    let mut source = "Minidump file metadata".to_string();
    if header_data.len() >= 8
        && &header_data[0..4] == b"PAGE"
        && &header_data[4..8] == b"DU64"
        && header_data.len() >= size_of::<DUMP_HEADER64>()
    {
        let header =
            unsafe { std::ptr::read_unaligned(header_data.as_ptr().cast::<DUMP_HEADER64>()) };
        code = Some(header.BugCheckCode);
        parameters = [
            header.BugCheckParameter1,
            header.BugCheckParameter2,
            header.BugCheckParameter3,
            header.BugCheckParameter4,
        ]
        .into_iter()
        .map(|value| format!("0x{value:016X}"))
        .collect();
        source = "DUMP_HEADER64 bugcheck metadata".into();
    } else if header_data.len() >= 8
        && &header_data[0..4] == b"PAGE"
        && &header_data[4..8] == b"DUMP"
        && header_data.len() >= size_of::<DUMP_HEADER32>()
    {
        let header =
            unsafe { std::ptr::read_unaligned(header_data.as_ptr().cast::<DUMP_HEADER32>()) };
        code = Some(header.BugCheckCode);
        parameters = [
            header.BugCheckParameter1,
            header.BugCheckParameter2,
            header.BugCheckParameter3,
            header.BugCheckParameter4,
        ]
        .into_iter()
        .map(|value| format!("0x{value:08X}"))
        .collect();
        source = "DUMP_HEADER32 bugcheck metadata".into();
    }
    let bugcheck_hex = code
        .map(|value| format!("0x{value:08X}"))
        .unwrap_or_default();
    let name = path
        .file_name()
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|| "minidump.dmp".into());
    // DBT-P46-B6: the id stays stable per dump file when the mtime is
    // unreadable — "unknown-time" rather than a 0 that would collide with a
    // dump genuinely stamped at the epoch.
    let crash_id = match recorded {
        Some(recorded) => format!("{name}:{recorded}"),
        None => format!("{name}:unknown-time"),
    };
    Ok(CrashRecord {
        crash_id,
        recorded_unix_ms: recorded,
        bugcheck_code: code,
        bugcheck_hex,
        parameters,
        dump_file: name,
        dump_size_bytes: meta.len(),
        source,
        confidence: if code.is_some() {
            "HeaderEvidence".into()
        } else {
            "MetadataOnly".into()
        },
        summary: if code.is_some() {
            "A Windows kernel dump is present with bugcheck header metadata. Full driver/module attribution requires symbol-assisted dump analysis.".into()
        } else {
            "A minidump file is present; its Windows dump header was not recognized by the lightweight metadata parser.".into()
        },
    })
}

fn w(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}
fn win(error: windows::core::Error) -> CrashError {
    if error.code() == E_ACCESSDENIED {
        CrashError::PermissionDenied(error.to_string())
    } else {
        CrashError::Windows(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filetime_conversion_is_saturating_and_epoch_aware() {
        assert_eq!(filetime_to_unix_ms(FILETIME_UNIX_EPOCH_TICKS), 0);
        assert_eq!(filetime_to_unix_ms(FILETIME_UNIX_EPOCH_TICKS + 10_000), 1);
    }

    #[test]
    fn render_caps_are_explicit_and_small() {
        assert!(MAX_SYSTEM_RENDER_BYTES <= 64 * 1024);
        assert!(MAX_USER_RENDER_BYTES <= 256 * 1024);
        assert!(MAX_EVENT_PROPERTIES <= 256);
    }

    #[test]
    fn render_property_count_is_rejected_before_allocation_when_pathological() {
        assert_eq!(
            checked_render_property_count(MAX_EVENT_PROPERTIES as u32).unwrap(),
            MAX_EVENT_PROPERTIES
        );
        let error = checked_render_property_count((MAX_EVENT_PROPERTIES + 1) as u32).unwrap_err();
        assert!(matches!(error, CrashError::MalformedResponse(_)));
    }
}
