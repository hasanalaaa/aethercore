#![deny(unsafe_op_in_unsafe_fn)]

use chrono::{Duration, NaiveDate};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum UpdateError {
    #[error("Windows Update discovery is only available on Windows")]
    UnsupportedPlatform,
    #[error("Windows Update Agent error: {0}")]
    Wua(String),
    #[error("Windows Update is offline: {0}")]
    Offline(String),
    #[error("Windows Update returned an invalid size value")]
    InvalidSize,
    #[error("another update installation is active")]
    Busy,
    #[error("Windows requires a reboot before another driver installation")]
    RebootPending,
    #[error("selected update is no longer applicable: {0}")]
    OfferNoLongerApplicable(String),
    #[error("selected update requires an EULA that has not been accepted: {0}")]
    EulaRequired(String),
    #[error("Windows Update operation timed out during {0}")]
    Timeout(String),
    #[error("pre-install protection failed: {0}")]
    Protection(String),
    #[error("Windows Update did not finish the driver search in time and was asked to stop")]
    SearchTimedOut,
    #[error("Windows Update is still stopping an earlier driver search")]
    SearchStillStopping,
}

pub type Result<T> = std::result::Result<T, UpdateError>;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum VersionSource {
    TitleHeuristic,
    #[default]
    Unavailable,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DriverOffer {
    pub update_id: String,
    pub revision: i32,
    pub title: String,
    pub hardware_id: String,
    pub driver_class: String,
    pub manufacturer: String,
    pub model: String,
    pub provider: String,
    pub driver_date_iso: String,
    pub device_problem_number: i32,
    pub device_status: i32,
    pub min_download_bytes: u64,
    pub max_download_bytes: u64,
    pub target_version: String,
    pub target_version_source: VersionSource,
    pub support_url: String,
}

/// Where a driver search may look. No network at rest: background work searches
/// `LocalCacheOnly`; only a user-initiated scan may search `Online`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchScope {
    /// `IUpdateSearcher.Online = VARIANT_FALSE`: WUA answers from local data only
    /// ([MS-UAMG] 3.38.4.13), i.e. whatever Windows itself last synchronised.
    LocalCacheOnly,
    /// `IUpdateSearcher.Online = VARIANT_TRUE`: WUA may contact the configured update server.
    Online,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryResult {
    pub offers: Vec<DriverOffer>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct UpdateIdentity {
    pub update_id: String,
    pub revision: i32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ExecutionStage {
    Revalidating,
    Downloading,
    Installing,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WuaProgress {
    pub stage: ExecutionStage,
    pub percent: u32,
    pub current_update_index: u32,
    pub current_update_percent: u32,
    /// DBT-P46-B16: None when this tick's WUA DECIMAL->u64 conversion failed.
    /// A real 0 ("nothing transferred yet") and a failed read are different
    /// facts; the caller keeps the last known value rather than publishing a
    /// fabricated zero.
    pub bytes_downloaded: Option<u64>,
    /// DBT-P46-B16: None when unknown. An unknown total and a zero-length
    /// download are different facts.
    pub bytes_total: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WuaUpdateResult {
    pub identity: UpdateIdentity,
    pub result_code: String,
    pub hresult: i32,
    pub reboot_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WuaExecutionResult {
    pub result_code: String,
    pub hresult: i32,
    pub reboot_required: bool,
    pub updates: Vec<WuaUpdateResult>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateHealthProbe {
    pub result_code: String,
    pub hresult: i32,
    pub pending_update_count: u32,
    pub detail: String,
}

/// What an update attempt was: only installations are judged; an uninstall says nothing about
/// whether an update took.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum HistoryOperation {
    Installation,
    Uninstallation,
    Other,
}

/// `OperationResultCode` of a history entry.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum HistoryResult {
    NotStarted,
    InProgress,
    Succeeded,
    SucceededWithErrors,
    Failed,
    Aborted,
    Unknown,
}

/// One row of the local Windows Update history (`IUpdateHistoryEntry`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateHistoryEntry {
    pub update_id: String,
    pub revision: i32,
    pub title: String,
    /// When the attempt was recorded; `None` when the agent gave no usable date.
    pub unix_ms: Option<i64>,
    pub operation: HistoryOperation,
    pub result: HistoryResult,
    pub hresult: i32,
}

/// The local history as read. `truncated` says older entries exist that were not read: an empty or
/// short list is then "not everything", and an unreadable history is an error, never an empty list.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateHistory {
    pub entries: Vec<UpdateHistoryEntry>,
    pub truncated: bool,
}

/// An update whose installation failed more than once with no later success of the same revision.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RepeatedFailure {
    pub update_id: String,
    pub title: String,
    pub failures: u32,
    pub last_failure_unix_ms: Option<i64>,
    /// Distinct failure codes, newest first, at most four.
    pub hresults: Vec<i32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryAnalysis {
    /// Installation failures with no later success of the same update and revision.
    pub unresolved_failures: u32,
    pub repeated_failures: Vec<RepeatedFailure>,
}

/// Reads the history as a record of attempts. A failure is closed only by a later success of the
/// same update and revision (an undated success cannot be shown to come later). Nothing here
/// says the machine is or is not up to date: the last success of some update is not compliance.
pub fn analyze_history(entries: &[UpdateHistoryEntry]) -> HistoryAnalysis {
    let installs = || {
        entries
            .iter()
            .filter(|e| e.operation == HistoryOperation::Installation)
    };
    let closes = |failure: &UpdateHistoryEntry| {
        installs().any(|success| {
            matches!(
                success.result,
                HistoryResult::Succeeded | HistoryResult::SucceededWithErrors
            ) && success.update_id == failure.update_id
                && success.revision == failure.revision
                && matches!((success.unix_ms, failure.unix_ms), (Some(after), Some(before)) if after >= before)
        })
    };
    let mut unresolved: Vec<&UpdateHistoryEntry> = installs()
        .filter(|e| matches!(e.result, HistoryResult::Failed | HistoryResult::Aborted))
        .filter(|failure| !closes(failure))
        .collect();
    unresolved.sort_by_key(|e| std::cmp::Reverse(e.unix_ms));
    let mut repeated = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for failure in &unresolved {
        if !seen.insert(failure.update_id.clone()) {
            continue;
        }
        let same: Vec<_> = unresolved
            .iter()
            .filter(|e| e.update_id == failure.update_id)
            .collect();
        if same.len() < 2 {
            continue;
        }
        let mut hresults = Vec::new();
        for e in &same {
            if !hresults.contains(&e.hresult) && hresults.len() < 4 {
                hresults.push(e.hresult);
            }
        }
        repeated.push(RepeatedFailure {
            update_id: failure.update_id.clone(),
            title: failure.title.clone(),
            failures: u32::try_from(same.len()).unwrap_or(u32::MAX),
            last_failure_unix_ms: same[0].unix_ms,
            hresults,
        });
    }
    HistoryAnalysis {
        unresolved_failures: u32::try_from(unresolved.len()).unwrap_or(u32::MAX),
        repeated_failures: repeated,
    }
}

/// One version-checked Error record from the update client Operational or System channel.
/// Event 25 is discovery evidence, event 20 installation evidence; neither is another WUA attempt.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ClientError {
    pub event_id: u32,
    pub error_code: u32,
    pub unix_ms: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ClientErrorSummary {
    pub count: u32,
    pub newest_unix_ms: Option<i64>,
    /// Distinct error codes, newest first, at most four.
    pub codes: Vec<u32>,
}

/// The bounded subset of error events read locally; unsupported schemas are counted separately.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClientErrors {
    pub errors: Vec<ClientError>,
    pub unknown_events: u32,
    pub truncated: bool,
    pub unavailable_channels: u32,
    /// Historical notifications, never the current WUA reboot-required flag.
    pub reboot_notifications: Vec<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ClientEvent {
    Error(ClientError),
    RebootNotification(i64),
}

fn xml_between<'a>(xml: &'a str, open: &str, close: &str) -> Option<&'a str> {
    if xml.matches(open).count() != 1 {
        return None;
    }
    let start = xml.find(open)? + open.len();
    let end = xml[start..].find(close)? + start;
    Some(&xml[start..end])
}

/// Windows-rendered XML only: measured provider/channel/id/System-Version and named fields.
/// Unsupported schemas and conflicting duplicate fields stay unknown.
pub fn parse_client_error_xml(xml: &str) -> Option<ClientError> {
    match parse_client_event_xml(xml)? {
        ClientEvent::Error(error) => Some(error),
        ClientEvent::RebootNotification(_) => None,
    }
}

pub(crate) fn parse_client_event_xml(xml: &str) -> Option<ClientEvent> {
    if xml.len() > 64 * 1024 {
        return None;
    }
    let normalized = xml.replace('\'', "\"");
    let xml = normalized.as_str();
    let provider = xml_between(xml, "<Provider ", "/>")?;
    if xml_between(provider, "Name=\"", "\"")? != "Microsoft-Windows-WindowsUpdateClient" {
        return None;
    }
    let event_id: u32 = xml_between(xml, "<EventID>", "</EventID>")?
        .trim()
        .parse()
        .ok()?;
    let version = xml_between(xml, "<Version>", "</Version>")?;
    let channel = xml_between(xml, "<Channel>", "</Channel>")?;
    let level = xml_between(xml, "<Level>", "</Level>")?;
    let error_schema = level == "2"
        && matches!(version, "0" | "1")
        && ((event_id == 25 && channel == "Microsoft-Windows-WindowsUpdateClient/Operational")
            || (event_id == 20 && channel == "System"));
    let reboot_schema = event_id == 21 && version == "0" && channel == "System" && level == "4";
    if !error_schema && !reboot_schema {
        return None;
    }
    let created = xml_between(xml, "<TimeCreated ", "/>")?;
    let stamp = xml_between(created, "SystemTime=\"", "\"")?;
    let unix_ms = chrono::DateTime::parse_from_rfc3339(stamp)
        .ok()?
        .timestamp_millis();
    if unix_ms <= 0 {
        return None;
    }
    if reboot_schema {
        let updates = xml_between(xml, "<Data Name=\"updatelist\">", "</Data>")?;
        return (!updates.trim().is_empty()).then_some(ClientEvent::RebootNotification(unix_ms));
    }
    let code = xml_between(xml, "<Data Name=\"errorCode\">", "</Data>")?.trim();
    let error_code = u32::from_str_radix(
        code.strip_prefix("0x")
            .or_else(|| code.strip_prefix("0X"))?,
        16,
    )
    .ok()?;
    Some(ClientEvent::Error(ClientError {
        event_id,
        error_code,
        unix_ms,
    }))
}

/// Counts the errors as records of distinct (event, code, time), newest first.
pub fn summarize_client_errors(errors: &[ClientError]) -> ClientErrorSummary {
    // ponytail: O(n²) deduplication over the collector's 200-event cap; use a set if the cap grows.
    let mut unique: Vec<ClientError> = Vec::new();
    for error in errors {
        if !unique.contains(error) {
            unique.push(*error);
        }
    }
    unique.sort_by_key(|e| std::cmp::Reverse(e.unix_ms));
    let mut codes = Vec::new();
    for error in &unique {
        if !codes.contains(&error.error_code) && codes.len() < 4 {
            codes.push(error.error_code);
        }
    }
    ClientErrorSummary {
        count: u32::try_from(unique.len()).unwrap_or(u32::MAX),
        newest_unix_ms: unique.first().map(|e| e.unix_ms),
        codes,
    }
}

/// An OLE Automation date (days since 1899-12-30) as Unix milliseconds. 0 is "no date", and a value
/// that is not finite or not a plausible date is `None`.
pub fn ole_date_to_unix_ms(value: f64) -> Option<i64> {
    if !value.is_finite() || value <= 0.0 || value > 1_000_000.0 {
        return None;
    }
    Some(((value - 25_569.0) * 86_400_000.0).round() as i64)
}

/// A Unix-millisecond timestamp as a UTC calendar date (`YYYY-MM-DD`); `None` when out of range.
pub fn unix_ms_to_iso_date(unix_ms: i64) -> Option<String> {
    chrono::DateTime::from_timestamp_millis(unix_ms).map(|dt| dt.format("%Y-%m-%d").to_string())
}

pub fn extract_version_from_update_title(title: &str) -> Option<String> {
    // WUA does not expose a universal target driver-version property. AetherCore therefore
    // treats a version parsed from the title as display-only metadata, never as a ranking
    // signal. Be deliberately conservative: only accept a dotted numeric token at the very
    // end of the title so a date/version mentioned earlier cannot be mistaken for the target.
    let token = title
        .split_whitespace()
        .last()?
        .trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '.' && c != '_' && c != '-')
        .trim_start_matches(['v', 'V']);

    if token.len() < 5 || token.len() > 64 {
        return None;
    }
    let pieces: Vec<&str> = token.split('.').collect();
    if !(3..=6).contains(&pieces.len())
        || !pieces
            .iter()
            .all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()))
    {
        return None;
    }
    Some(token.to_owned())
}

pub fn ole_automation_date_to_iso(value: f64) -> Option<String> {
    if !value.is_finite() || !(-100_000.0..=1_000_000.0).contains(&value) {
        return None;
    }
    let base = NaiveDate::from_ymd_opt(1899, 12, 30)?.and_hms_opt(0, 0, 0)?;
    let millis = (value * 86_400_000.0).round();
    if millis < i64::MIN as f64 || millis > i64::MAX as f64 {
        return None;
    }
    base.checked_add_signed(Duration::milliseconds(millis as i64))
        .map(|dt| dt.date().format("%Y-%m-%d").to_string())
}

#[cfg(windows)]
mod client_events_windows;
#[cfg(windows)]
pub use client_events_windows::query_client_errors;
#[cfg(not(windows))]
pub fn query_client_errors(_max_entries: usize) -> Result<ClientErrors> {
    Err(UpdateError::UnsupportedPlatform)
}

#[cfg(windows)]
mod execution_windows;
#[cfg(windows)]
mod windows_impl;

#[cfg_attr(not(windows), allow(dead_code))]
mod bounded;

#[cfg(windows)]
pub use execution_windows::{ensure_servicing_available, execute_driver_updates};
#[cfg(windows)]
pub use windows_impl::{
    discover_driver_offers, discover_driver_offers_with_keepalive, last_online_search_iso,
    probe_update_health, query_update_history,
};

#[cfg(not(windows))]
pub fn probe_update_health() -> UpdateHealthProbe {
    UpdateHealthProbe {
        result_code: "UpdateUnknown".into(),
        hresult: 0,
        pending_update_count: 0,
        detail: "Windows Update Agent is only available on Windows".into(),
    }
}

#[cfg(not(windows))]
pub fn query_update_history(_max_entries: usize) -> Result<UpdateHistory> {
    Err(UpdateError::UnsupportedPlatform)
}

#[cfg(not(windows))]
pub fn discover_driver_offers(_scope: SearchScope) -> Result<DiscoveryResult> {
    Err(UpdateError::UnsupportedPlatform)
}

#[cfg(not(windows))]
pub fn discover_driver_offers_with_keepalive<G: Send + 'static>(
    _scope: SearchScope,
    _keepalive: G,
) -> Result<DiscoveryResult> {
    Err(UpdateError::UnsupportedPlatform)
}

#[cfg(not(windows))]
pub fn last_online_search_iso() -> Option<String> {
    None
}

#[cfg(not(windows))]
pub fn ensure_servicing_available() -> Result<()> {
    Err(UpdateError::UnsupportedPlatform)
}

#[cfg(not(windows))]
pub fn execute_driver_updates<P, B>(
    _identities: &[UpdateIdentity],
    _progress: P,
    _before_install: B,
) -> Result<WuaExecutionResult>
where
    P: FnMut(WuaProgress),
    B: FnOnce() -> std::result::Result<(), String>,
{
    Err(UpdateError::UnsupportedPlatform)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conservatively_extracts_dotted_driver_version() {
        assert_eq!(
            extract_version_from_update_title("Intel - Extension - 31.0.101.5590"),
            Some("31.0.101.5590".into())
        );
        assert_eq!(
            extract_version_from_update_title("NVIDIA - Display - (32.0.15.6094)"),
            Some("32.0.15.6094".into())
        );
    }

    #[test]
    fn does_not_invent_target_version() {
        assert_eq!(
            extract_version_from_update_title("Realtek Semiconductor Corp. - MEDIA"),
            None
        );
        assert_eq!(extract_version_from_update_title("Driver 2026.08"), None);
        assert_eq!(
            extract_version_from_update_title("Vendor 31.0.101.5590 - Extension"),
            None
        );
    }

    // P83-03A: history is read as a record of attempts, not as a compliance statement.
    fn entry(
        id: &str,
        revision: i32,
        op: HistoryOperation,
        result: HistoryResult,
        at: Option<i64>,
        hr: i32,
    ) -> UpdateHistoryEntry {
        UpdateHistoryEntry {
            update_id: id.into(),
            revision,
            title: format!("Update {id}"),
            unix_ms: at,
            operation: op,
            result,
            hresult: hr,
        }
    }
    use HistoryOperation::{Installation as Install, Uninstallation as Uninstall};
    use HistoryResult::{Aborted, Failed, Succeeded, SucceededWithErrors};

    #[test]
    fn a_failure_followed_by_a_success_of_the_same_update_is_not_unresolved() {
        let history = [
            entry("A", 1, Install, Succeeded, Some(20), 0),
            entry("A", 1, Install, Failed, Some(10), 0x8024_2016_u32 as i32),
        ];
        let analysis = analyze_history(&history);
        assert!(analysis.repeated_failures.is_empty());
        assert_eq!(analysis.unresolved_failures, 0);
    }

    #[test]
    fn another_updates_success_or_another_revision_does_not_close_a_failure() {
        let history = [
            entry("B", 1, Install, Succeeded, Some(30), 0),
            entry("A", 2, Install, Succeeded, Some(30), 0),
            entry("A", 1, Install, Failed, Some(10), 1),
        ];
        assert_eq!(analyze_history(&history).unresolved_failures, 1);
        let earlier_success = [
            entry("A", 1, Install, Succeeded, Some(5), 0),
            entry("A", 1, Install, Failed, Some(10), 1),
        ];
        assert_eq!(
            analyze_history(&earlier_success).unresolved_failures,
            1,
            "a success before the failure does not resolve it"
        );
    }

    #[test]
    fn two_unresolved_failures_of_one_update_are_repeated_with_their_dates_and_codes() {
        let history = [
            entry("A", 1, Install, Failed, Some(300), 0x8007_0005_u32 as i32),
            entry("A", 1, Install, Aborted, Some(200), 0x8024_2016_u32 as i32),
            entry("A", 1, Install, Failed, Some(100), 0x8007_0005_u32 as i32),
            entry("C", 1, Install, Failed, Some(50), 7),
        ];
        let analysis = analyze_history(&history);
        assert_eq!(analysis.unresolved_failures, 4);
        assert_eq!(
            analysis.repeated_failures.len(),
            1,
            "C failed once: not repeated"
        );
        let repeated = &analysis.repeated_failures[0];
        assert_eq!(
            (
                repeated.update_id.as_str(),
                repeated.failures,
                repeated.last_failure_unix_ms
            ),
            ("A", 3, Some(300))
        );
        assert_eq!(
            repeated.hresults,
            vec![0x8007_0005_u32 as i32, 0x8024_2016_u32 as i32],
            "distinct codes, newest first"
        );
    }

    #[test]
    fn uninstalls_are_ignored_an_undated_failure_stays_unresolved_and_a_success_with_errors_resolves()
     {
        let history = [
            entry("A", 1, Uninstall, Failed, Some(10), 1),
            entry("D", 1, Install, Failed, None, 2),
            entry("D", 1, Install, Succeeded, None, 0),
            entry("E", 1, Install, Failed, Some(10), 3),
            entry("E", 1, Install, SucceededWithErrors, Some(20), 0),
        ];
        let analysis = analyze_history(&history);
        assert_eq!(
            analysis.unresolved_failures, 1,
            "only D: an undated success cannot be shown to come after"
        );
    }

    #[test]
    fn a_unix_time_becomes_its_utc_date() {
        assert_eq!(unix_ms_to_iso_date(0).as_deref(), Some("1970-01-01"));
        assert_eq!(
            unix_ms_to_iso_date(1_790_669_658_556).as_deref(),
            Some("2026-09-29")
        );
    }

    // P83-03B: what the update client logged as errors, read by field name from the event XML.
    // The XML is a real event read on a Windows 11 machine (computer and SID redacted).
    const CLIENT_ERROR_XML: &str = "<Event xmlns='http://schemas.microsoft.com/win/2004/08/events/event'><System><Provider Name='Microsoft-Windows-WindowsUpdateClient' Guid='{945a8954-c147-4acd-923f-40c45405a658}'/><EventID>25</EventID><Version>1</Version><Level>2</Level><TimeCreated SystemTime='2026-09-28T19:26:29.4304781Z'/><Channel>Microsoft-Windows-WindowsUpdateClient/Operational</Channel></System><EventData><Data Name='errorCode'>0x80240438</Data><Data Name='serviceGuid'>{8b24b027-1dee-babb-9a95-3517dfb9c552}</Data></EventData></Event>";

    #[test]
    fn a_real_client_error_event_gives_its_id_code_and_time() {
        let e = parse_client_error_xml(CLIENT_ERROR_XML).expect("an error");
        assert_eq!((e.event_id, e.error_code), (25, 0x8024_0438));
        assert_eq!(e.unix_ms, 1_790_623_589_430, "2026-09-28T19:26:29.430Z");
    }

    #[test]
    fn system_installation_error_uses_only_its_measured_channel_and_versions() {
        let system = CLIENT_ERROR_XML
            .replace("<EventID>25</EventID>", "<EventID>20</EventID>")
            .replace(
                "Microsoft-Windows-WindowsUpdateClient/Operational</Channel>",
                "System</Channel>",
            );
        for version in [0, 1] {
            let xml = system.replace(
                "<Version>1</Version>",
                &format!("<Version>{version}</Version>"),
            );
            assert_eq!(
                parse_client_error_xml(&xml)
                    .expect("System 20 error")
                    .event_id,
                20
            );
        }
        assert!(
            parse_client_error_xml(&system.replace("<Version>1</Version>", "<Version>2</Version>"))
                .is_none()
        );
        assert!(
            parse_client_error_xml(
                &system.replace("<EventID>20</EventID>", "<EventID>19</EventID>")
            )
            .is_none()
        );
        assert!(
            parse_client_error_xml(&system.replace("<Level>2</Level>", "<Level>4</Level>"))
                .is_none()
        );
        assert!(
            parse_client_error_xml(&system.replace(
                "<Data Name='errorCode'>0x80240438</Data>",
                "<Data Name='errorCode'>0x80240438</Data><Data Name='errorCode'>0x80070005</Data>"
            ))
            .is_none()
        );
    }

    #[test]
    fn reboot_event_is_historical_notification_never_an_error_or_current_reboot_flag() {
        let xml = CLIENT_ERROR_XML
            .replace("<EventID>25</EventID>", "<EventID>21</EventID>")
            .replace("<Version>1</Version>", "<Version>0</Version>")
            .replace("<Level>2</Level>", "<Level>4</Level>")
            .replace(
                "Microsoft-Windows-WindowsUpdateClient/Operational</Channel>",
                "System</Channel>",
            )
            .replace(
                "<Data Name='errorCode'>0x80240438</Data>",
                "<Data Name='updatelist'>Example update</Data>",
            );
        assert_eq!(
            parse_client_event_xml(&xml),
            Some(ClientEvent::RebootNotification(1_790_623_589_430))
        );
        assert!(parse_client_error_xml(&xml).is_none());
        for unsupported in [
            xml.replace("<Version>0</Version>", "<Version>1</Version>"),
            xml.replace("updatelist", "wrongField"),
            xml.replace(
                "System</Channel>",
                "Microsoft-Windows-WindowsUpdateClient/Operational</Channel>",
            ),
        ] {
            assert!(parse_client_event_xml(&unsupported).is_none());
        }
    }

    #[test]
    fn an_event_without_its_fields_or_from_another_provider_is_not_an_error_record() {
        assert!(
            parse_client_error_xml(&CLIENT_ERROR_XML.replace("errorCode", "other")).is_none(),
            "no errorCode field"
        );
        assert!(
            parse_client_error_xml(
                &CLIENT_ERROR_XML.replace("Microsoft-Windows-WindowsUpdateClient'", "Some-Other'")
            )
            .is_none(),
            "another provider"
        );
        assert!(
            parse_client_error_xml(
                &CLIENT_ERROR_XML.replace("<Level>2</Level>", "<Level>4</Level>")
            )
            .is_none(),
            "not an error level"
        );
        assert!(
            parse_client_error_xml(&CLIENT_ERROR_XML.replace("0x80240438", "not-a-code")).is_none()
        );
        assert!(parse_client_error_xml("").is_none());
    }

    #[test]
    fn client_error_schema_is_checked_and_native_xml_quotes_are_supported() {
        assert_eq!(
            parse_client_error_xml(&CLIENT_ERROR_XML.replace("'", "\"")),
            parse_client_error_xml(CLIENT_ERROR_XML)
        );
        assert!(
            parse_client_error_xml(
                &CLIENT_ERROR_XML.replace("<Version>1</Version>", "<Version>2</Version>")
            )
            .is_none()
        );
        assert!(
            parse_client_error_xml(
                &CLIENT_ERROR_XML.replace("<EventID>25</EventID>", "<EventID>20</EventID>")
            )
            .is_none()
        );
        assert!(
            parse_client_error_xml(&CLIENT_ERROR_XML.replace(
                "<Channel>Microsoft-Windows-WindowsUpdateClient/Operational</Channel>",
                "<Channel>System</Channel>"
            ))
            .is_none()
        );
    }

    #[test]
    fn errors_are_counted_by_distinct_code_newest_first_and_a_repeat_is_not_a_second_attempt() {
        let e = |id, code, at| ClientError {
            event_id: id,
            error_code: code,
            unix_ms: at,
        };
        let summary = summarize_client_errors(&[
            e(25, 0x8024_0438, 300),
            e(25, 0x8024_0438, 300),
            e(20, 0x8007_0005, 200),
            e(25, 0x8024_0438, 100),
        ]);
        assert_eq!(
            summary.count, 3,
            "the same event, same code, same instant is one record"
        );
        assert_eq!(summary.newest_unix_ms, Some(300));
        assert_eq!(summary.codes, vec![0x8024_0438, 0x8007_0005]);
        assert_eq!(summarize_client_errors(&[]), ClientErrorSummary::default());
    }

    #[test]
    fn ole_dates_become_unix_milliseconds_and_nonsense_becomes_none() {
        assert_eq!(
            ole_date_to_unix_ms(25_569.0),
            Some(0),
            "the OLE date of 1970-01-01"
        );
        assert_eq!(ole_date_to_unix_ms(25_570.5), Some(129_600_000));
        assert_eq!(ole_date_to_unix_ms(f64::NAN), None);
        assert_eq!(ole_date_to_unix_ms(0.0), None, "0 is 'no date', not 1899");
    }

    #[test]
    fn converts_modern_ole_dates_to_iso() {
        assert_eq!(
            ole_automation_date_to_iso(2.0).as_deref(),
            Some("1900-01-01")
        );
    }
}
