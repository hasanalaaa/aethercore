#![deny(unsafe_op_in_unsafe_fn)]

use aethercore_collector_runtime::CollectorFaultRecord;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TelemetryError {
    #[error("Windows telemetry error: {0}")]
    Windows(String),
    #[error("collector unavailable: {0}")]
    Unavailable(String),
    #[error("collector permission denied: {0}")]
    PermissionDenied(String),
    #[error("collector timed out: {0}")]
    Timeout(String),
    #[error("collector cancelled: {0}")]
    Cancelled(String),
    #[error("malformed platform response: {0}")]
    MalformedResponse(String),
}

pub type Result<T> = std::result::Result<T, TelemetryError>;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AtaSmartAttribute {
    pub id: u32,
    pub current: u32,
    pub worst: u32,
    /// ATA SMART raw values are vendor-defined. Decimal and hex preserve the six raw bytes without inventing units.
    pub raw_value_decimal: String,
    pub raw_value_hex: String,
}

const ATA_SMART_SECTOR_BYTES: usize = 512;
const ATA_SMART_ATTRIBUTE_COUNT: usize = 30;
const ATA_SMART_ATTRIBUTE_BYTES: usize = 12;

pub(crate) fn parse_ata_smart_sector(data: &[u8]) -> Result<Vec<AtaSmartAttribute>> {
    if data.len() < ATA_SMART_SECTOR_BYTES {
        return Err(TelemetryError::MalformedResponse(format!(
            "ATA SMART sector was {} bytes; expected at least {ATA_SMART_SECTOR_BYTES}",
            data.len()
        )));
    }
    let mut attrs = Vec::new();
    for i in 0..ATA_SMART_ATTRIBUTE_COUNT {
        let base = 2 + i * ATA_SMART_ATTRIBUTE_BYTES;
        if base + ATA_SMART_ATTRIBUTE_BYTES > ATA_SMART_SECTOR_BYTES {
            break;
        }
        let id = data[base];
        if id == 0 || id == 0xFF {
            continue;
        }
        let current = data[base + 3];
        let worst = data[base + 4];
        let raw = &data[base + 5..base + 11];
        let mut raw8 = [0u8; 8];
        raw8[..6].copy_from_slice(raw);
        let raw_value = u64::from_le_bytes(raw8);
        attrs.push(AtaSmartAttribute {
            id: u32::from(id),
            current: u32::from(current),
            worst: u32::from(worst),
            raw_value_decimal: raw_value.to_string(),
            raw_value_hex: format!("0x{raw_value:012X}"),
        });
    }
    Ok(attrs)
}

pub(crate) fn parse_ata_driver_response(
    output: &[u8],
    returned: usize,
    data_offset: usize,
) -> Result<Vec<AtaSmartAttribute>> {
    if returned > output.len() {
        return Err(TelemetryError::MalformedResponse(
            "ATA SMART driver reported more bytes than the supplied output buffer".into(),
        ));
    }
    if returned < 4 {
        return Err(TelemetryError::MalformedResponse(
            "ATA SMART response header was truncated".into(),
        ));
    }
    let required_end = data_offset
        .checked_add(ATA_SMART_SECTOR_BYTES)
        .ok_or_else(|| {
            TelemetryError::MalformedResponse("ATA SMART response offset overflowed".into())
        })?;
    if required_end > returned {
        return Err(TelemetryError::MalformedResponse(
            "ATA SMART response did not contain the 512-byte attribute sector".into(),
        ));
    }
    let declared = u32::from_le_bytes(output[0..4].try_into().map_err(|_| {
        TelemetryError::MalformedResponse("ATA SMART response header was malformed".into())
    })?) as usize;
    let available = returned.saturating_sub(data_offset);
    if declared < ATA_SMART_SECTOR_BYTES || declared > available {
        return Err(TelemetryError::MalformedResponse(
            "ATA SMART driver returned an inconsistent cBufferSize".into(),
        ));
    }
    parse_ata_smart_sector(&output[data_offset..required_end])
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct NvmeHealthValues {
    pub critical: u8,
    pub temperature_c: Option<i32>,
    pub spare: u8,
    pub used: u8,
    pub unsafe_shutdowns: String,
    pub media_errors: String,
    pub error_entries: String,
}

pub(crate) fn parse_nvme_health_log(data: &[u8]) -> Result<NvmeHealthValues> {
    const REQUIRED: usize = 192;
    if data.len() < REQUIRED {
        return Err(TelemetryError::MalformedResponse(format!(
            "NVMe SMART log was {} bytes; expected at least {REQUIRED}",
            data.len()
        )));
    }
    let temperature_kelvin = u16::from_le_bytes([data[1], data[2]]);
    let read_u128 = |offset: usize| -> u128 {
        let mut bytes = [0u8; 16];
        bytes.copy_from_slice(&data[offset..offset + 16]);
        u128::from_le_bytes(bytes)
    };
    Ok(NvmeHealthValues {
        critical: data[0],
        temperature_c: (temperature_kelvin > 0).then(|| i32::from(temperature_kelvin) - 273),
        spare: data[3],
        used: data[5],
        unsafe_shutdowns: read_u128(144).to_string(),
        media_errors: read_u128(160).to_string(),
        error_entries: read_u128(176).to_string(),
    })
}

pub(crate) fn checked_protocol_window(
    returned: usize,
    protocol_offset: usize,
    data_offset: u32,
    data_length: u32,
    minimum_data_offset: usize,
    required: usize,
) -> Result<std::ops::Range<usize>> {
    let data_offset = usize::try_from(data_offset).map_err(|_| {
        TelemetryError::MalformedResponse("protocol data offset did not fit usize".into())
    })?;
    let data_length = usize::try_from(data_length).map_err(|_| {
        TelemetryError::MalformedResponse("protocol data length did not fit usize".into())
    })?;
    if data_offset < minimum_data_offset {
        return Err(TelemetryError::MalformedResponse(
            "protocol payload overlapped the protocol-specific metadata header".into(),
        ));
    }
    if data_length < required {
        return Err(TelemetryError::MalformedResponse(format!(
            "protocol payload was {data_length} bytes; required at least {required}"
        )));
    }
    let start = protocol_offset.checked_add(data_offset).ok_or_else(|| {
        TelemetryError::MalformedResponse("protocol payload offset overflowed".into())
    })?;
    let end = start.checked_add(data_length).ok_or_else(|| {
        TelemetryError::MalformedResponse("protocol payload length overflowed".into())
    })?;
    if start > returned || end > returned {
        return Err(TelemetryError::MalformedResponse(
            "protocol payload escaped the bytes returned by the driver".into(),
        ));
    }
    let required_end = start.checked_add(required).ok_or_else(|| {
        TelemetryError::MalformedResponse("required payload length overflowed".into())
    })?;
    if required_end > end {
        return Err(TelemetryError::MalformedResponse(
            "protocol payload did not contain the required health-log prefix".into(),
        ));
    }
    Ok(start..required_end)
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StorageReliability {
    pub temperature_c: Option<i32>,
    pub temperature_max_c: Option<i32>,
    pub wear_percent_used: Option<u32>,
    pub power_on_hours: Option<u64>,
    pub read_errors_total: Option<u64>,
    pub read_errors_uncorrected: Option<u64>,
    pub write_errors_total: Option<u64>,
    pub write_errors_uncorrected: Option<u64>,
    pub read_latency_max_ms: Option<u64>,
    pub write_latency_max_ms: Option<u64>,
    pub flush_latency_max_ms: Option<u64>,
    pub nvme_critical_warning: Option<u8>,
    pub nvme_available_spare_percent: Option<u8>,
    pub nvme_percentage_used: Option<u8>,
    /// NVMe SMART counters are 128-bit values. Strings preserve the exact device-reported value.
    pub nvme_media_errors: Option<String>,
    pub nvme_unsafe_shutdowns: Option<String>,
    pub nvme_error_log_entries: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StorageDeviceTelemetry {
    pub device_id: String,
    pub friendly_name: String,
    pub firmware_version: String,
    pub serial_number: String,
    pub bus_type: String,
    pub media_type: String,
    pub size_bytes: u64,
    pub windows_health_status: String,
    pub operational_status: Vec<String>,
    #[serde(default)]
    pub ata_smart_attributes: Vec<AtaSmartAttribute>,
    pub reliability: StorageReliability,
    pub severity: String,
    pub summary: String,
    pub reasons: Vec<String>,
    pub source_notes: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MemoryTelemetry {
    pub total_physical_bytes: u64,
    pub available_physical_bytes: u64,
    pub memory_load_percent: u32,
    pub pressure_label: String,
    pub pressure_explanation: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HardwareTelemetrySnapshot {
    pub storage: Vec<StorageDeviceTelemetry>,
    pub memory: Option<MemoryTelemetry>,
    #[serde(default)]
    pub provider_faults: Vec<CollectorFaultRecord>,
    pub warnings: Vec<String>,
}

pub fn classify_storage(device: &mut StorageDeviceTelemetry) {
    let r = &device.reliability;
    let mut action = Vec::new();
    let mut attention = Vec::new();
    // DBT-P46-B2: an unreadable SMART counter (None) and a confirmed-zero one
    // (Some(0)) used to reach the same `.unwrap_or(0) > 0` check and come out
    // identical — a failed read silently looked like "confirmed no errors".
    // Tracked separately here so "Normal" can say which counters, if any, it
    // could not actually confirm, rather than presenting a gap as a clean bill
    // of health. `windows_health_status` is unaffected: it is Windows' own
    // independent verdict, read regardless of whether these counters exist.
    let mut unavailable = Vec::new();

    if device
        .windows_health_status
        .eq_ignore_ascii_case("Unhealthy")
    {
        action.push("Windows reports the physical disk as unhealthy.".to_string());
    } else if device.windows_health_status.eq_ignore_ascii_case("Warning") {
        attention
            .push("Windows reports a warning health state for this physical disk.".to_string());
    }
    match r.read_errors_uncorrected {
        Some(n) if n > 0 => action.push(format!("{n} uncorrected read error(s) were reported.")),
        Some(_) => {}
        None => unavailable.push("uncorrected read error count"),
    }
    match r.write_errors_uncorrected {
        Some(n) if n > 0 => action.push(format!("{n} uncorrected write error(s) were reported.")),
        Some(_) => {}
        None => unavailable.push("uncorrected write error count"),
    }
    match r.nvme_critical_warning {
        Some(n) if n != 0 => action.push(format!(
            "NVMe SMART critical-warning flags are set (0x{n:02X})."
        )),
        Some(_) => {}
        None => unavailable.push("NVMe critical-warning flags"),
    }
    match r.nvme_media_errors.as_deref() {
        Some(v) => {
            if parse_nonzero_counter(Some(v)) {
                action.push(
                    "NVMe SMART reports one or more media/data-integrity errors.".to_string(),
                );
            }
        }
        None => unavailable.push("NVMe media/data-integrity error count"),
    }
    if r.wear_percent_used.is_some_and(|v| v >= 100)
        || r.nvme_percentage_used.is_some_and(|v| v >= 100)
    {
        attention.push(
            "The device-reported wear estimate has reached or exceeded its estimated wear limit."
                .to_string(),
        );
    }
    if let (Some(t), Some(max)) = (r.temperature_c, r.temperature_max_c) {
        if max > 0 && t >= max {
            attention.push(format!("Current temperature ({t} °C) is at or above the device/Windows-reported maximum ({max} °C)."));
        }
    }
    for (label, latency) in [
        ("read", r.read_latency_max_ms),
        ("write", r.write_latency_max_ms),
        ("flush", r.flush_latency_max_ms),
    ] {
        if latency.is_some_and(|v| v > 10_000) {
            attention.push(format!("Windows reports a maximum {label} latency above 10 seconds in the storage reliability counters."));
        }
    }

    if !action.is_empty() {
        device.severity = "ActionRequired".into();
        device.summary = "Storage reliability evidence requires attention; back up important data before heavy write activity.".into();
        device.reasons = action.into_iter().chain(attention).collect();
    } else if !attention.is_empty() {
        device.severity = "Attention".into();
        device.summary =
            "One or more reported storage reliability indicators deserve review.".into();
        device.reasons = attention;
    } else if device.windows_health_status.eq_ignore_ascii_case("Healthy") {
        device.severity = "Normal".into();
        if unavailable.is_empty() {
            device.summary = "Windows/device-reported metrics that are available do not currently show a reliability warning.".into();
        } else {
            device.summary = format!(
                "Windows reports this disk healthy; {} SMART/reliability counter(s) were not reported and could not be independently checked.",
                unavailable.len()
            );
            device.reasons = unavailable
                .iter()
                .map(|name| format!("{name}: not reported"))
                .collect();
        }
    } else {
        device.severity = "Unknown".into();
        device.summary = "The device does not expose enough standardized reliability information for a health conclusion.".into();
    }
}

fn parse_nonzero_counter(v: Option<&str>) -> bool {
    v.and_then(|s| s.parse::<u128>().ok())
        .is_some_and(|n| n > 0)
}

pub fn classify_memory_pressure(load_percent: u32) -> (String, String) {
    let label = match load_percent {
        0..=79 => "Normal",
        80..=89 => "Elevated",
        _ => "High",
    };
    (
        label.into(),
        format!(
            "Windows currently reports {load_percent}% physical-memory load. This is resource pressure, not a RAM hardware-health verdict."
        ),
    )
}

#[cfg(windows)]
mod windows_impl;

#[cfg(windows)]
pub use windows_impl::{collect, collect_with_cancellation};

#[cfg(not(windows))]
pub fn collect() -> Result<HardwareTelemetrySnapshot> {
    Err(TelemetryError::Unavailable(
        "hardware telemetry is available on Windows only".into(),
    ))
}

#[cfg(not(windows))]
pub fn collect_with_cancellation(
    _token: aethercore_collector_runtime::CancellationToken,
) -> Result<HardwareTelemetrySnapshot> {
    collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_classifier_never_creates_a_score_and_escalates_uncorrected_errors() {
        let mut d = StorageDeviceTelemetry {
            windows_health_status: "Healthy".into(),
            ..Default::default()
        };
        d.reliability.read_errors_uncorrected = Some(1);
        classify_storage(&mut d);
        assert_eq!(d.severity, "ActionRequired");
        assert!(d.summary.contains("back up"));
    }

    #[test]
    fn absent_metrics_remain_unknown_instead_of_zero() {
        let mut d = StorageDeviceTelemetry::default();
        classify_storage(&mut d);
        assert_eq!(d.severity, "Unknown");
        assert_eq!(d.reliability.temperature_c, None);
        assert_eq!(d.reliability.wear_percent_used, None);
    }

    #[test]
    fn ata_smart_sector_parser_preserves_raw_six_bytes() {
        let mut sector = [0u8; ATA_SMART_SECTOR_BYTES];
        let base = 2;
        sector[base] = 5;
        sector[base + 3] = 99;
        sector[base + 4] = 98;
        sector[base + 5..base + 11].copy_from_slice(&[1, 2, 3, 4, 5, 6]);
        let attrs = parse_ata_smart_sector(&sector).unwrap();
        assert_eq!(attrs.len(), 1);
        assert_eq!(attrs[0].id, 5);
        assert_eq!(attrs[0].current, 99);
        assert_eq!(attrs[0].worst, 98);
        assert_eq!(attrs[0].raw_value_hex, "0x060504030201");
        assert_eq!(
            attrs[0].raw_value_decimal,
            u64::from_le_bytes([1, 2, 3, 4, 5, 6, 0, 0]).to_string()
        );
    }

    #[test]
    fn ata_smart_sector_parser_rejects_short_buffers() {
        assert!(matches!(
            parse_ata_smart_sector(&[0u8; 511]),
            Err(TelemetryError::MalformedResponse(_))
        ));
    }

    #[test]
    fn ata_smart_raw_values_are_not_converted_into_vendor_specific_health_claims() {
        let mut d = StorageDeviceTelemetry::default();
        d.ata_smart_attributes.push(AtaSmartAttribute {
            id: 5,
            current: 100,
            worst: 100,
            raw_value_decimal: "1".into(),
            raw_value_hex: "0x000000000001".into(),
        });
        classify_storage(&mut d);
        assert_eq!(d.severity, "Unknown");
        assert!(d.reasons.is_empty());
    }

    #[test]
    fn standardized_extreme_latency_is_attention_not_a_failure_verdict() {
        let mut d = StorageDeviceTelemetry {
            windows_health_status: "Healthy".into(),
            ..Default::default()
        };
        d.reliability.read_latency_max_ms = Some(10_001);
        classify_storage(&mut d);
        assert_eq!(d.severity, "Attention");
        assert!(d.reasons.iter().any(|r| r.contains("above 10 seconds")));
    }

    // DBT-P46-B2: Windows reports the disk healthy, but the SMART reliability
    // counters (read/write uncorrected errors, NVMe critical warning) were never
    // reported (None) rather than confirmed zero. Before the fix, `.unwrap_or(0)`
    // made this byte-identical to a disk that WAS checked and came back clean —
    // the summary text and the (empty) reasons list gave no way to tell them
    // apart.
    #[test]
    fn healthy_status_with_unreported_smart_counters_is_distinguishable_from_confirmed_clean() {
        let mut checked_clean = StorageDeviceTelemetry {
            windows_health_status: "Healthy".into(),
            ..Default::default()
        };
        checked_clean.reliability.read_errors_uncorrected = Some(0);
        checked_clean.reliability.write_errors_uncorrected = Some(0);
        checked_clean.reliability.nvme_critical_warning = Some(0);
        classify_storage(&mut checked_clean);

        let mut uncheckable = StorageDeviceTelemetry {
            windows_health_status: "Healthy".into(),
            ..Default::default()
        };
        // read_errors_uncorrected / write_errors_uncorrected / nvme_critical_warning
        // all stay None — never reported, not confirmed zero.
        classify_storage(&mut uncheckable);

        assert_eq!(checked_clean.severity, "Normal");
        assert_eq!(uncheckable.severity, "Normal");
        assert_ne!(
            checked_clean.summary, uncheckable.summary,
            "a disk that was actually checked and a disk whose counters were never \
             reported must not produce the identical summary"
        );
        assert!(
            !uncheckable.reasons.is_empty(),
            "the unreported-counter case must say which counters were unavailable, \
             got empty reasons: {uncheckable:?}"
        );
    }

    #[test]
    fn memory_pressure_text_explicitly_separates_pressure_from_hardware_health() {
        let (label, detail) = classify_memory_pressure(93);
        assert_eq!(label, "High");
        assert!(detail.contains("not a RAM hardware-health verdict"));
    }
    #[cfg(windows)]
    #[test]
    #[ignore = "read-only live Windows storage/memory telemetry"]
    fn live_storage_and_memory_collection_is_read_only() {
        let snapshot = collect().expect("collect hardware telemetry");
        assert!(
            snapshot
                .memory
                .as_ref()
                .is_some_and(|memory| memory.total_physical_bytes > 0)
        );
    }

    #[test]
    fn ata_parser_rejects_truncated_driver_response() {
        assert!(matches!(
            parse_ata_smart_sector(&[0u8; 511]),
            Err(TelemetryError::MalformedResponse(_))
        ));
    }

    #[test]
    fn nvme_parser_rejects_truncated_vendor_response() {
        assert!(matches!(
            parse_nvme_health_log(&[0u8; 191]),
            Err(TelemetryError::MalformedResponse(_))
        ));
    }

    #[test]
    fn nvme_parser_accepts_vendor_tail_without_reading_past_standard_prefix() {
        let mut data = vec![0u8; 640];
        data[0] = 2;
        data[1..3].copy_from_slice(&300u16.to_le_bytes());
        data[3] = 91;
        data[5] = 7;
        data[144..160].copy_from_slice(&5u128.to_le_bytes());
        data[160..176].copy_from_slice(&9u128.to_le_bytes());
        data[176..192].copy_from_slice(&11u128.to_le_bytes());
        let parsed = parse_nvme_health_log(&data).unwrap();
        assert_eq!(parsed.critical, 2);
        assert_eq!(parsed.temperature_c, Some(27));
        assert_eq!(parsed.media_errors, "9");
    }

    #[test]
    fn protocol_window_rejects_overflow_and_truncation() {
        assert!(checked_protocol_window(256, 64, 64, 32, 32, 64).is_err());
        assert!(checked_protocol_window(256, usize::MAX - 2, 8, 64, 8, 64).is_err());
        assert!(checked_protocol_window(256, 64, 64, 128, 32, 64).is_ok());
        assert!(checked_protocol_window(256, 64, 8, 128, 32, 64).is_err());
    }

    #[test]
    fn ata_driver_response_rejects_forged_returned_length() {
        let output = vec![0u8; 520];
        assert!(matches!(
            parse_ata_driver_response(&output, output.len() + 1, 8),
            Err(TelemetryError::MalformedResponse(_))
        ));
    }

    #[test]
    fn ata_driver_response_rejects_inconsistent_declared_payload() {
        let mut output = vec![0u8; 520];
        output[0..4].copy_from_slice(&1024u32.to_le_bytes());
        assert!(matches!(
            parse_ata_driver_response(&output, output.len(), 8),
            Err(TelemetryError::MalformedResponse(_))
        ));
    }

    #[test]
    fn ata_driver_response_accepts_exact_bounded_sector() {
        let mut output = vec![0u8; 520];
        output[0..4].copy_from_slice(&512u32.to_le_bytes());
        let base = 8 + 2;
        output[base] = 9;
        output[base + 3] = 100;
        output[base + 4] = 99;
        output[base + 5..base + 11].copy_from_slice(&[1, 0, 0, 0, 0, 0]);
        let parsed = parse_ata_driver_response(&output, output.len(), 8).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].id, 9);
    }
}
