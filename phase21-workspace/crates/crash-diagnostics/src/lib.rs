#![deny(unsafe_op_in_unsafe_fn)]

use aethercore_collector_runtime::CollectorFaultRecord;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CrashError {
    #[error("Windows diagnostics error: {0}")]
    Windows(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("collector unavailable: {0}")]
    Unavailable(String),
    #[error("collector permission denied: {0}")]
    PermissionDenied(String),
    #[error("collector timed out: {0}")]
    Timeout(String),
    #[error("collector cancelled: {0}")]
    Cancelled(String),
    #[error("malformed event/log response: {0}")]
    MalformedResponse(String),
}
pub type Result<T> = std::result::Result<T, CrashError>;

pub const DEFAULT_EVENT_WINDOW_DAYS: u32 = 30;

/// The crash window in milliseconds.
pub const EVENT_WINDOW_MS: u64 = DEFAULT_EVENT_WINDOW_DAYS as u64 * 24 * 60 * 60 * 1000;

/// The System-channel XPath for the crash window. Kernel-Power is taken for id 41
/// only: it is the one id `classify_event` reads, and its routine power-transition
/// ids would otherwise fill the collector's event cap ahead of WHEA records.
pub fn event_query(window_ms: u64) -> String {
    format!(
        "*[System[(Provider[@Name='Microsoft-Windows-WHEA-Logger'] or (Provider[@Name='Microsoft-Windows-Kernel-Power'] and EventID=41) or Provider[@Name='Microsoft-Windows-WER-SystemErrorReporting']) and TimeCreated[timediff(@SystemTime) <= {window_ms}]]]"
    )
}

/// Whether a minidump written at `modified` belongs to the crash window ending `now`.
/// A dump whose time cannot be read is left out: it cannot be shown as recent. A time
/// after `now` (clock skew) counts as recent.
pub fn dump_in_window(
    modified: Option<std::time::SystemTime>,
    now: std::time::SystemTime,
    window_ms: u64,
) -> bool {
    modified.is_some_and(|written| {
        now.duration_since(written)
            .map_or(true, |age| age.as_millis() <= u128::from(window_ms))
    })
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EventEvidence {
    pub event_id: u32,
    pub provider: String,
    pub recorded_unix_ms: i64,
    pub category: String,
    pub severity: String,
    pub confidence: String,
    pub summary: String,
    pub detail: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CrashRecord {
    pub crash_id: String,
    /// DBT-P46-B6: None when the dump file's mtime could not be read. Was
    /// flattened to 0, which renders as 1970-01-01 — a plausible-looking wrong
    /// date is worse than an absent one.
    pub recorded_unix_ms: Option<i64>,
    pub bugcheck_code: Option<u32>,
    pub bugcheck_hex: String,
    pub parameters: Vec<String>,
    pub dump_file: String,
    pub dump_size_bytes: u64,
    pub source: String,
    pub confidence: String,
    pub summary: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CrashDiagnosticsSnapshot {
    #[serde(default)]
    pub event_window_days: u32,
    /// False when the Event Log could not be read: `events` is then empty because nothing was
    /// read, not because nothing was logged. Defaults to false so a snapshot that does not say
    /// it read the log never vouches for an empty one.
    #[serde(default)]
    pub event_log_read: bool,
    pub events: Vec<EventEvidence>,
    pub crashes: Vec<CrashRecord>,
    #[serde(default)]
    pub provider_faults: Vec<CollectorFaultRecord>,
    pub warnings: Vec<String>,
}

/// Folds one collection pass into a snapshot. `event_log` is `None` when the Event Log read
/// failed.
pub fn assemble_snapshot(
    event_log: Option<Vec<EventEvidence>>,
    crashes: Vec<CrashRecord>,
    provider_faults: Vec<CollectorFaultRecord>,
    warnings: Vec<String>,
) -> CrashDiagnosticsSnapshot {
    let event_log_read = event_log.is_some();
    CrashDiagnosticsSnapshot {
        event_window_days: if event_log_read {
            DEFAULT_EVENT_WINDOW_DAYS
        } else {
            0
        },
        event_log_read,
        events: event_log.unwrap_or_default(),
        crashes,
        provider_faults,
        warnings,
    }
}

pub fn classify_event(
    provider: &str,
    event_id: u32,
    payload_values: &[String],
    recorded_unix_ms: i64,
) -> EventEvidence {
    let lower = payload_values.join(" ").to_ascii_lowercase();
    if provider.eq_ignore_ascii_case("Microsoft-Windows-WHEA-Logger") {
        let (category, detail) = if lower.contains("memory") || lower.contains("memhierarchy") {
            (
                "MemoryHardwareEvidence",
                "WHEA logged a memory-related hardware error. This is hardware evidence, but it does not identify a specific DIMM without deeper decoding/testing.",
            )
        } else if lower.contains("cache")
            || lower.contains("processor")
            || lower.contains("machine check")
        {
            (
                "ProcessorHardwareEvidence",
                "WHEA logged a processor/cache-related hardware error. Treat this as evidence, not a complete root-cause attribution.",
            )
        } else if lower.contains("pci") || lower.contains("pcie") {
            (
                "PcieHardwareEvidence",
                "WHEA logged PCI/PCIe-related hardware-error evidence.",
            )
        } else {
            (
                "HardwareError",
                "WHEA logged a hardware error; the summarized event data is not sufficient to name a failed component with confidence.",
            )
        };
        return EventEvidence {
            event_id,
            provider: provider.into(),
            recorded_unix_ms,
            category: category.into(),
            severity: "Attention".into(),
            confidence: "HighEvidence/RootCauseUnknown".into(),
            summary: "Windows Hardware Error Architecture recorded a hardware error.".into(),
            detail: detail.into(),
        };
    }
    if provider.eq_ignore_ascii_case("Microsoft-Windows-Kernel-Power") && event_id == 41 {
        return EventEvidence { event_id, provider:provider.into(), recorded_unix_ms, category:"UnexpectedShutdown".into(), severity:"Attention".into(), confidence:"EventHigh/CauseLow".into(), summary:"Windows recorded an unexpected shutdown or restart.".into(), detail:"Kernel-Power Event 41 confirms an unclean shutdown; by itself it does not identify why power was lost or the system crashed.".into() };
    }
    if provider.eq_ignore_ascii_case("Microsoft-Windows-WER-SystemErrorReporting") {
        return EventEvidence { event_id, provider:provider.into(), recorded_unix_ms, category:"BugcheckReport".into(), severity:"Attention".into(), confidence:"HighEvidence".into(), summary:"Windows Error Reporting recorded a system crash/bugcheck.".into(), detail:"The event is crash evidence. A precise driver/module attribution may require the matching dump plus symbols.".into() };
    }
    EventEvidence {
        event_id,
        provider: provider.into(),
        recorded_unix_ms,
        category: "SystemEvent".into(),
        severity: "Info".into(),
        confidence: "EventOnly".into(),
        summary: "Relevant system diagnostic event.".into(),
        detail: "Event retained as supporting evidence.".into(),
    }
}

#[cfg(windows)]
mod windows_impl;
#[cfg(windows)]
pub use windows_impl::{collect, collect_with_cancellation};
#[cfg(not(windows))]
pub fn collect() -> Result<CrashDiagnosticsSnapshot> {
    Err(CrashError::Unavailable(
        "crash diagnostics are available on Windows only".into(),
    ))
}
#[cfg(not(windows))]
pub fn collect_with_cancellation(
    _token: aethercore_collector_runtime::CancellationToken,
) -> Result<CrashDiagnosticsSnapshot> {
    collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_kernel_power_41_competes_for_the_event_cap() {
        let q = event_query(EVENT_WINDOW_MS);
        assert!(
            q.contains("(Provider[@Name='Microsoft-Windows-Kernel-Power'] and EventID=41)"),
            "{q}"
        );
        assert!(q.contains("Microsoft-Windows-WHEA-Logger"), "{q}");
        assert!(q.contains(&format!("<= {EVENT_WINDOW_MS}")), "{q}");
    }
    #[test]
    fn minidumps_outside_the_window_or_without_a_time_are_left_out() {
        use std::time::{Duration, SystemTime};
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000_000);
        let day = Duration::from_secs(86_400);
        assert!(dump_in_window(Some(now - day), now, EVENT_WINDOW_MS));
        assert!(dump_in_window(Some(now - 30 * day), now, EVENT_WINDOW_MS));
        assert!(!dump_in_window(Some(now - 31 * day), now, EVENT_WINDOW_MS));
        assert!(!dump_in_window(None, now, EVENT_WINDOW_MS));
        assert!(dump_in_window(Some(now + day), now, EVENT_WINDOW_MS));
    }
    #[test]
    fn kernel_power_never_claims_root_cause() {
        let e = classify_event("Microsoft-Windows-Kernel-Power", 41, &[], 1);
        assert!(e.detail.contains("does not identify why"));
        assert_eq!(e.category, "UnexpectedShutdown");
    }
    #[test]
    fn whea_memory_is_evidence_not_dimm_diagnosis() {
        let e = classify_event(
            "Microsoft-Windows-WHEA-Logger",
            18,
            &["Memory hierarchy error".into()],
            1,
        );
        assert_eq!(e.category, "MemoryHardwareEvidence");
        assert!(e.detail.contains("does not identify a specific DIMM"));
    }
    #[test]
    fn structured_payload_classification_does_not_require_xml() {
        let e = classify_event(
            "Microsoft-Windows-WHEA-Logger",
            18,
            &["Processor".into(), "Machine Check Exception".into()],
            1,
        );
        assert_eq!(e.category, "ProcessorHardwareEvidence");
    }

    #[test]
    fn a_failed_event_log_read_is_not_an_empty_one() {
        let failed = assemble_snapshot(None, Vec::new(), Vec::new(), Vec::new());
        assert!(!failed.event_log_read);
        assert_eq!(failed.event_window_days, 0);
        let empty = assemble_snapshot(Some(Vec::new()), Vec::new(), Vec::new(), Vec::new());
        assert!(empty.event_log_read);
        assert_eq!(empty.event_window_days, DEFAULT_EVENT_WINDOW_DAYS);
    }

    #[test]
    fn event_window_is_explicit_and_bounded() {
        assert_eq!(DEFAULT_EVENT_WINDOW_DAYS, 30);
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "read-only live Windows Event Log/minidump collection"]
    fn live_event_and_minidump_collection_is_read_only() {
        let _ = collect().expect("collect crash diagnostics");
    }
}
