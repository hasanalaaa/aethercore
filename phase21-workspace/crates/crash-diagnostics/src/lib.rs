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

mod whea;

pub const DEFAULT_EVENT_WINDOW_DAYS: u32 = 30;

/// The crash window in milliseconds.
pub const EVENT_WINDOW_MS: u64 = DEFAULT_EVENT_WINDOW_DAYS as u64 * 24 * 60 * 60 * 1000;

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

/// Classifies one System-channel event. `whea_record` is the raw CPER a WHEA-Logger event carries as
/// binary data, when it carried one. The component category comes only from that record's typed
/// sections: matching words in the (localized) payload text said "memory" on an English Windows and
/// nothing on an Arabic one, so it is not used. No record, or one that does not validate, is a
/// hardware error of unknown component.
pub fn classify_event(
    provider: &str,
    event_id: u32,
    _payload_values: &[String],
    recorded_unix_ms: i64,
    whea_record: Option<&[u8]>,
) -> EventEvidence {
    if provider.eq_ignore_ascii_case("Microsoft-Windows-WHEA-Logger") {
        use whea::{WheaSection, WheaSeverity};
        let record = whea_record.and_then(whea::decode_cper);
        let has = |kind: WheaSection| record.as_ref().is_some_and(|r| r.sections.contains(&kind));
        let corrected = record
            .as_ref()
            .is_some_and(|r| r.severity == WheaSeverity::Corrected);
        let (category, detail) = if has(WheaSection::Memory) && corrected {
            (
                "MemoryHardwareEvidence",
                "WHEA logged a memory error that the hardware corrected. A single corrected error does not show that a memory module is failing.",
            )
        } else if has(WheaSection::Memory) {
            (
                "MemoryHardwareEvidence",
                "WHEA logged a memory-related hardware error. This is hardware evidence, but it does not identify a specific DIMM without deeper decoding/testing.",
            )
        } else if has(WheaSection::Processor) {
            (
                "ProcessorHardwareEvidence",
                "WHEA logged a processor/cache-related hardware error. Treat this as evidence, not a complete root-cause attribution.",
            )
        } else if has(WheaSection::Pcie) {
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
    // Windows Memory Diagnostic's own result events: id 1201 says the test finished without
    // errors, 1202 that it reported errors. The id decides, never the localized message text.
    if provider.eq_ignore_ascii_case("Microsoft-Windows-MemoryDiagnostics-Results")
        && matches!(event_id, 1201 | 1202)
    {
        let errors = event_id == 1202;
        return EventEvidence {
            event_id,
            provider: provider.into(),
            recorded_unix_ms,
            category: "MemoryTestResult".into(),
            severity: if errors { "Attention" } else { "Info" }.into(),
            confidence: "EventOnly".into(),
            summary: "Windows Memory Diagnostic recorded a test result.".into(),
            detail: if errors {
                "Windows Memory Diagnostic reported memory errors when it ran."
            } else {
                "Windows Memory Diagnostic finished when it ran and reported no errors. This is the result of that run, not a statement about the memory now."
            }
            .into(),
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
        let e = classify_event("Microsoft-Windows-Kernel-Power", 41, &[], 1, None);
        assert!(e.detail.contains("does not identify why"));
        assert_eq!(e.category, "UnexpectedShutdown");
    }

    const WHEA: &str = "Microsoft-Windows-WHEA-Logger";

    #[test]
    fn whea_memory_is_evidence_not_dimm_diagnosis() {
        let record = whea::fixtures::cper(1, &[whea::fixtures::MEMORY_GUID]);
        let e = classify_event(WHEA, 18, &[], 1, Some(&record));
        assert_eq!(e.category, "MemoryHardwareEvidence");
        assert!(e.detail.contains("does not identify a specific DIMM"));
    }

    // P82-03A: the category comes from the record's typed sections, never from words.
    // Pinned by phase13_structured_whea_classifier: classification reads structured data, not XML
    // and not prose. It now needs no payload text at all, only the typed record.
    #[test]
    fn structured_payload_classification_does_not_require_xml() {
        let record = whea::fixtures::cper(1, &[whea::fixtures::PROCESSOR_GUID]);
        let e = classify_event(WHEA, 18, &[], 1, Some(&record));
        assert_eq!(e.category, "ProcessorHardwareEvidence");
    }

    #[test]
    fn english_and_arabic_windows_give_the_same_category_for_the_same_record() {
        let record = whea::fixtures::cper(1, &[whea::fixtures::PROCESSOR_GUID]);
        let english = classify_event(
            WHEA,
            18,
            &["Memory hierarchy error".into()],
            1,
            Some(&record),
        );
        let arabic = classify_event(WHEA, 18, &["خطأ في تسلسل الذاكرة".into()], 1, Some(&record));
        assert_eq!(
            english.category, "ProcessorHardwareEvidence",
            "the record says processor; the words say memory"
        );
        assert_eq!(english.category, arabic.category);
        assert_eq!(english.detail, arabic.detail);
    }

    #[test]
    fn words_alone_never_name_a_component_and_a_bad_record_is_unknown() {
        for words in ["Memory hierarchy error", "Cache", "PCI Express", "ذاكرة"] {
            let e = classify_event(WHEA, 18, &[words.into()], 1, None);
            assert_eq!(e.category, "HardwareError", "{words}");
        }
        let mut bad = whea::fixtures::cper(1, &[whea::fixtures::MEMORY_GUID]);
        bad[0] = 0;
        assert_eq!(
            classify_event(WHEA, 18, &[], 1, Some(&bad)).category,
            "HardwareError"
        );
        assert_eq!(
            classify_event(WHEA, 18, &[], 1, Some(&[1, 2, 3])).category,
            "HardwareError"
        );
    }

    // P82-03B: a test result is read from the event id, in any Windows language.
    #[test]
    fn a_memory_test_result_is_read_from_the_event_id_and_dated_not_certified() {
        let p = "Microsoft-Windows-MemoryDiagnostics-Results";
        let ok = classify_event(p, 1201, &["لم يتم اكتشاف أخطاء".into()], 7, None);
        assert_eq!(
            (ok.category.as_str(), ok.severity.as_str()),
            ("MemoryTestResult", "Info")
        );
        assert!(ok.detail.contains("not a statement about the memory now"));
        let bad = classify_event(p, 1202, &[], 7, None);
        assert_eq!(
            (bad.category.as_str(), bad.severity.as_str()),
            ("MemoryTestResult", "Attention")
        );
        assert_eq!(
            classify_event(p, 999, &[], 7, None).category,
            "SystemEvent",
            "another id is not a result"
        );
        assert_eq!(
            classify_event("Some-Other", 1201, &[], 7, None).category,
            "SystemEvent",
            "and another provider's 1201 is not ours"
        );
    }

    #[test]
    fn a_corrected_memory_error_is_not_called_a_failing_module() {
        let corrected = whea::fixtures::cper(2, &[whea::fixtures::MEMORY_GUID]);
        let e = classify_event(WHEA, 19, &[], 1, Some(&corrected));
        assert_eq!(e.category, "MemoryHardwareEvidence");
        assert!(e.detail.contains("corrected"), "{}", e.detail);
        assert!(
            e.detail
                .contains("does not show that a memory module is failing")
        );
        let pcie = whea::fixtures::cper(2, &[whea::fixtures::PCIE_GUID]);
        assert_eq!(
            classify_event(WHEA, 19, &[], 1, Some(&pcie)).category,
            "PcieHardwareEvidence"
        );
        let other = whea::fixtures::cper(1, &[[9u8; 16]]);
        assert_eq!(
            classify_event(WHEA, 18, &[], 1, Some(&other)).category,
            "HardwareError",
            "an unknown section is not a component"
        );
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
