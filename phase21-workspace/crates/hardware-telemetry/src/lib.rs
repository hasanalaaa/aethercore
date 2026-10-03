#![deny(unsafe_op_in_unsafe_fn)]

use aethercore_collector_runtime::CollectorFaultRecord;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[cfg(any(windows, test))]
mod battery;
pub mod measurements;
#[cfg(any(windows, test))]
mod network;
#[cfg(windows)]
mod network_windows;
#[cfg(any(windows, test))]
mod thermal;

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

// The storage-IOCTL parsers below serve windows_impl; off Windows they are compiled only
// for their host-agnostic tests.
#[cfg(any(windows, test))]
const ATA_SMART_SECTOR_BYTES: usize = 512;
#[cfg(any(windows, test))]
const ATA_SMART_ATTRIBUTE_COUNT: usize = 30;
#[cfg(any(windows, test))]
const ATA_SMART_ATTRIBUTE_BYTES: usize = 12;

#[cfg(any(windows, test))]
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

#[cfg(any(windows, test))]
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

#[cfg(any(windows, test))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct NvmeHealthValues {
    pub critical: u8,
    pub temperature_c: Option<i32>,
    pub spare: u8,
    /// The device's own threshold for `spare`; below it the device flags a critical warning.
    pub spare_threshold: u8,
    pub used: u8,
    pub unsafe_shutdowns: String,
    pub media_errors: String,
    pub error_entries: String,
}

#[cfg(any(windows, test))]
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
        spare_threshold: data[4],
        used: data[5],
        unsafe_shutdowns: read_u128(144).to_string(),
        media_errors: read_u128(160).to_string(),
        error_entries: read_u128(176).to_string(),
    })
}

#[cfg(any(windows, test))]
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
    /// The device's own threshold below which its spare counts as low; absent in snapshots
    /// written before it was read.
    #[serde(default)]
    pub nvme_available_spare_threshold_percent: Option<u8>,
    /// The drive's own failure prediction (`IOCTL_STORAGE_PREDICT_FAILURE`); absent when it could
    /// not be asked, which is not "no failure predicted".
    #[serde(default)]
    pub smart_predict_failure: Option<bool>,
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
    /// Whether the ATA SMART table could be read: `None` when it was never asked (not an ATA disk),
    /// `Some(false)` when it was asked and did not answer. Coverage only; the table's values never
    /// drive a verdict.
    #[serde(default)]
    pub ata_table_available: Option<bool>,
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
    pub thermal_zones: Vec<measurements::ThermalZone>,
    #[serde(default)]
    pub batteries: Vec<measurements::Battery>,
    #[serde(default)]
    pub network_adapters: Vec<measurements::NetworkAdapter>,
    #[serde(default)]
    pub provider_faults: Vec<CollectorFaultRecord>,
    pub warnings: Vec<String>,
}

/// WMI HRESULTs that say the class or namespace is not there (invalid class, not found, not
/// supported, invalid namespace): a machine that does not publish a source, which is "not
/// measured". Any other WMI failure (an invalid query, an access error) is a real fault.
#[cfg(any(windows, test))]
pub(crate) fn is_absent_wmi_class(hresult: i32) -> bool {
    matches!(
        hresult as u32,
        0x8004_1010 | 0x8004_1002 | 0x8004_100C | 0x8004_100E
    )
}

/// What a disk says it is, as reported either by WMI (`MSFT_PhysicalDisk`) or by an opened
/// `\\.\PhysicalDriveN` handle. Never leaves this crate: the serial number is not exposed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DiskIdentity {
    pub serial: String,
    pub bus: String,
    pub size_bytes: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HandleBinding {
    /// Both sides report the same serial, bus and size: the handle is this disk.
    Confirmed,
    /// A field both sides report differs: the handle is another disk.
    Mismatch,
    /// Nothing proves the handle is this disk (a missing serial); nothing proves it is not.
    Unproven,
}

fn serial_key(serial: &str) -> String {
    serial
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// ATA and SATA are one family; every other bus compares by name.
fn bus_family(bus: &str) -> String {
    let bus = bus.trim().to_ascii_lowercase();
    if bus == "ata" || bus == "sata" {
        "sata".into()
    } else {
        bus
    }
}

/// Decides whether the IOCTL answers of `opened` may be attributed to the WMI record `reported`.
/// The WMI `DeviceId` is not always the PhysicalDrive index (Storage Spaces, hot-swapped disks),
/// so the index alone proves nothing. A disk without a serial is not confirmed by size and bus:
/// two disks of one model and size would be mixed.
pub fn bind_handle(reported: &DiskIdentity, opened: &DiskIdentity) -> HandleBinding {
    let (a, b) = (bus_family(&reported.bus), bus_family(&opened.bus));
    let bus_differs = !a.is_empty() && !b.is_empty() && a != b;
    let size_differs =
        matches!((reported.size_bytes, opened.size_bytes), (Some(x), Some(y)) if x != y);
    let (sa, sb) = (serial_key(&reported.serial), serial_key(&opened.serial));
    let serial_differs = !sa.is_empty() && !sb.is_empty() && sa != sb;
    if bus_differs || size_differs || serial_differs {
        HandleBinding::Mismatch
    } else if sa.is_empty() || sb.is_empty() {
        HandleBinding::Unproven
    } else {
        HandleBinding::Confirmed
    }
}

/// Reads the bus type and serial number out of a `STORAGE_DEVICE_DESCRIPTOR`
/// (`StorageDeviceProperty`). Offsets are the documented layout: `SerialNumberOffset` at 24,
/// `BusType` at 28. An offset that points outside the returned bytes reads as "no serial", never
/// as a panic or a read past the buffer.
#[cfg(any(windows, test))]
pub(crate) fn parse_storage_device_descriptor(bytes: &[u8]) -> Option<(u32, String)> {
    let word = |at: usize| -> Option<u32> {
        Some(u32::from_le_bytes(
            bytes.get(at..at.checked_add(4)?)?.try_into().ok()?,
        ))
    };
    let size = usize::try_from(word(4)?).ok()?;
    if size < 36 || size > bytes.len() {
        return None;
    }
    let bus = word(28)?;
    let serial = match usize::try_from(word(24)?) {
        Ok(offset) if offset != 0 && offset < size => {
            let raw = &bytes[offset..size];
            let end = raw.iter().position(|b| *b == 0).unwrap_or(raw.len());
            String::from_utf8_lossy(&raw[..end]).trim().to_string()
        }
        _ => String::new(),
    };
    Some((bus, serial))
}

/// `STORAGE_PREDICT_FAILURE`: a `u32` that is non-zero when the drive predicts its own failure,
/// followed by vendor bytes that are not interpreted. Fewer than four bytes is no answer.
#[cfg(any(windows, test))]
pub(crate) fn parse_predict_failure(bytes: &[u8]) -> Option<bool> {
    let word: [u8; 4] = bytes.get(..4)?.try_into().ok()?;
    Some(u32::from_le_bytes(word) != 0)
}

/// Counter history needs a complete identity on both observations. Handle binding may tolerate
/// an unavailable bus or size while validating an opened handle; a historical trend cannot.
pub fn same_counter_device(
    previous: &StorageDeviceTelemetry,
    current: &StorageDeviceTelemetry,
) -> bool {
    let same = |d: &StorageDeviceTelemetry| DiskIdentity {
        serial: d.serial_number.clone(),
        bus: d.bus_type.clone(),
        size_bytes: Some(d.size_bytes).filter(|size| *size > 0),
    };
    let known_bus = |d: &StorageDeviceTelemetry| {
        let bus = d.bus_type.trim();
        !bus.is_empty()
            && !bus.eq_ignore_ascii_case("Unknown")
            && !bus.eq_ignore_ascii_case("BusType 0")
    };
    known_bus(previous)
        && known_bus(current)
        && previous.size_bytes > 0
        && current.size_bytes > 0
        && bind_handle(&same(previous), &same(current)) == HandleBinding::Confirmed
}

/// Adds what changed since `previous` to `current.reasons`, for the same disk only (same serial,
/// bus and size). A counter that went down is a reset or a replacement, not an improvement, and
/// says nothing; a counter that could not be read on either side says nothing. The caller must
/// first establish documented, ordered observation windows; this function does no clock reading.
pub fn compare_counters(previous: &StorageDeviceTelemetry, current: &mut StorageDeviceTelemetry) {
    if !same_counter_device(previous, current) {
        return;
    }
    let (p, c) = (&previous.reliability, &current.reliability);
    let big = |v: &Option<String>| v.as_deref().and_then(|s| s.parse::<u128>().ok());
    let pairs: [(u8, Option<u128>, Option<u128>); 3] = [
        (
            0,
            p.read_errors_uncorrected.map(u128::from),
            c.read_errors_uncorrected.map(u128::from),
        ),
        (
            1,
            p.write_errors_uncorrected.map(u128::from),
            c.write_errors_uncorrected.map(u128::from),
        ),
        (2, big(&p.nvme_media_errors), big(&c.nvme_media_errors)),
    ];
    let mut compared_nonzero = false;
    let mut grew = false;
    // Missing supported observations, malformed reported NVMe values and resets cannot justify
    // a sentence reassuring about all reported counters. Both sides absent means unsupported.
    let mut incomplete = [&p.nvme_media_errors, &c.nvme_media_errors]
        .iter()
        .any(|v| v.is_some() && big(v).is_none());
    for (kind, before, now) in pairs {
        let (Some(before), Some(now)) = (before, now) else {
            incomplete |= before.is_some() || now.is_some();
            continue;
        };
        incomplete |= now < before;
        compared_nonzero |= now > 0;
        if now > before {
            grew = true;
            let more = now - before;
            current.reasons.push(match kind {
                0 => format!("{more} more uncorrected read error(s) than at the previous scan."),
                1 => format!("{more} more uncorrected write error(s) than at the previous scan."),
                _ => format!(
                    "{more} more NVMe media/data-integrity error(s) than at the previous scan."
                ),
            });
        }
    }
    if compared_nonzero && !grew && !incomplete {
        current
            .reasons
            .push("No increase in the reported error counters since the previous scan.".into());
    }
}

/// Folds an NVMe health log into the reliability record. `PercentageUsed` may exceed 100 and is
/// kept as reported; a temperature the device did not report stays absent.
#[cfg(any(windows, test))]
pub(crate) fn merge_nvme(r: &mut StorageReliability, n: NvmeHealthValues) {
    r.nvme_critical_warning = Some(n.critical);
    r.nvme_available_spare_percent = Some(n.spare);
    r.nvme_available_spare_threshold_percent = Some(n.spare_threshold);
    r.nvme_percentage_used = Some(n.used);
    r.nvme_unsafe_shutdowns = Some(n.unsafe_shutdowns);
    r.nvme_media_errors = Some(n.media_errors);
    r.nvme_error_log_entries = Some(n.error_entries);
    if r.temperature_c.is_none() {
        r.temperature_c = n.temperature_c;
    }
    if r.wear_percent_used.is_none() {
        r.wear_percent_used = Some(u32::from(n.used));
    }
}

/// The NVMe critical-warning byte, bit by bit, as the specification defines it.
fn critical_warning_meanings(flags: u8) -> Vec<&'static str> {
    let named: [(u8, &str); 6] = [
        (
            0x01,
            "NVMe reports the available spare has fallen below its threshold.",
        ),
        (
            0x02,
            "NVMe reports the temperature is outside its operating limits.",
        ),
        (0x04, "NVMe reports the device's reliability is degraded."),
        (0x08, "NVMe reports the device has become read-only."),
        (0x10, "NVMe reports its volatile memory backup has failed."),
        (
            0x20,
            "NVMe reports its persistent memory region is unreliable.",
        ),
    ];
    let mut out: Vec<&'static str> = named
        .iter()
        .filter(|(bit, _)| flags & bit != 0)
        .map(|(_, text)| *text)
        .collect();
    if flags & 0xC0 != 0 {
        out.push("NVMe critical-warning flags include bits this reader does not recognize.");
    }
    out
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
    // Coverage follows the bus: a SATA disk is not charged with counters only NVMe has, and an
    // NVMe disk is not charged with an ATA table.
    let is_nvme = device.bus_type.eq_ignore_ascii_case("NVMe");
    let is_ata =
        device.bus_type.eq_ignore_ascii_case("SATA") || device.bus_type.eq_ignore_ascii_case("ATA");

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
        Some(n) if n != 0 => {
            action.push(format!(
                "NVMe SMART critical-warning flags are set (0x{n:02X})."
            ));
            action.extend(critical_warning_meanings(n).into_iter().map(String::from));
        }
        Some(_) => {}
        None if is_nvme => unavailable.push("NVMe critical-warning flags"),
        None => {}
    }
    // Below the device's own threshold, and the device has not (yet) raised the flag itself.
    if let (Some(spare), Some(threshold)) = (
        r.nvme_available_spare_percent,
        r.nvme_available_spare_threshold_percent,
    ) && threshold > 0
        && spare < threshold
        && r.nvme_critical_warning.unwrap_or(0) & 0x01 == 0
    {
        attention.push(format!(
            "NVMe available spare ({spare}%) is below the device's own threshold ({threshold}%)."
        ));
    }
    match r.nvme_media_errors.as_deref() {
        Some(v) => {
            if parse_nonzero_counter(Some(v)) {
                action.push(
                    "NVMe SMART reports one or more media/data-integrity errors.".to_string(),
                );
            }
        }
        None if is_nvme => unavailable.push("NVMe media/data-integrity error count"),
        None => {}
    }
    if is_ata && device.ata_table_available == Some(false) {
        unavailable.push("ATA SMART attribute table");
    }
    if r.smart_predict_failure == Some(true) {
        action.push("The drive's own SMART self-assessment predicts a failure.".to_string());
    }
    if r.wear_percent_used.is_some_and(|v| v >= 100)
        || r.nvme_percentage_used.is_some_and(|v| v >= 100)
    {
        attention.push(
            "The device-reported wear estimate has reached or exceeded its estimated wear limit."
                .to_string(),
        );
    }
    if let (Some(t), Some(max)) = (r.temperature_c, r.temperature_max_c)
        && max > 0
        && t >= max
    {
        attention.push(format!("Current temperature ({t} °C) is at or above the device/Windows-reported maximum ({max} °C)."));
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

    fn identity(serial: &str, bus: &str, size: Option<u64>) -> DiskIdentity {
        DiskIdentity {
            serial: serial.into(),
            bus: bus.into(),
            size_bytes: size,
        }
    }

    // P81-01: WMI says which disk this is; the opened `\\.\PhysicalDriveN` says which disk it
    // is. Only when they agree may the IOCTL answers be attributed to the WMI record.
    #[test]
    fn a_handle_is_bound_only_to_the_disk_whose_serial_it_reports() {
        let wmi = identity("0025_38B1_2320_C7E3.", "NVMe", Some(2_000_398_934_016));
        let same = identity("002538B12320C7E3", "NVMe", Some(2_000_398_934_016));
        assert_eq!(
            bind_handle(&wmi, &same),
            HandleBinding::Confirmed,
            "punctuation and case are formatting"
        );
        // Two disks of one model and size differ only by serial: they must not be mixed.
        let sibling = identity("002538B12320FFFF", "NVMe", Some(2_000_398_934_016));
        assert_eq!(bind_handle(&wmi, &sibling), HandleBinding::Mismatch);
    }

    #[test]
    fn the_device_descriptor_yields_bus_and_serial_and_never_reads_out_of_bounds() {
        let mut d = vec![0u8; 64];
        d[4..8].copy_from_slice(&64u32.to_le_bytes());
        d[24..28].copy_from_slice(&40u32.to_le_bytes());
        d[28..32].copy_from_slice(&17u32.to_le_bytes());
        d[40..48].copy_from_slice(b"SN 42   ");
        assert_eq!(
            parse_storage_device_descriptor(&d),
            Some((17, "SN 42".into()))
        );
        // An offset past the returned bytes is "no serial", and a short buffer is "no answer".
        d[24..28].copy_from_slice(&9_000u32.to_le_bytes());
        assert_eq!(
            parse_storage_device_descriptor(&d),
            Some((17, String::new()))
        );
        assert_eq!(parse_storage_device_descriptor(&d[..20]), None);
        d[4..8].copy_from_slice(&4_000u32.to_le_bytes());
        assert_eq!(
            parse_storage_device_descriptor(&d),
            None,
            "a size larger than the buffer is malformed"
        );
    }

    // P81-03: coverage follows the bus, and a counter is compared only with the same disk's own past.
    fn disk(bus: &str) -> StorageDeviceTelemetry {
        let mut d = StorageDeviceTelemetry {
            windows_health_status: "Healthy".into(),
            bus_type: bus.into(),
            serial_number: "S1".into(),
            size_bytes: 1_000,
            ..Default::default()
        };
        d.reliability.read_errors_uncorrected = Some(0);
        d.reliability.write_errors_uncorrected = Some(0);
        d
    }

    fn with_attribute(mut d: StorageDeviceTelemetry) -> StorageDeviceTelemetry {
        d.ata_smart_attributes = vec![AtaSmartAttribute {
            id: 5,
            current: 100,
            worst: 100,
            ..Default::default()
        }];
        d
    }

    #[test]
    fn a_healthy_sata_disk_is_not_charged_with_missing_nvme_counters() {
        let mut d = with_attribute(disk("SATA"));
        classify_storage(&mut d);
        assert_eq!(d.severity, "Normal");
        assert!(
            d.reasons.iter().all(|r| !r.contains("NVMe")),
            "{:?}",
            d.reasons
        );
        assert!(d.summary.contains("do not currently show"), "{}", d.summary);
    }

    #[test]
    fn a_sata_disk_whose_smart_table_could_not_be_read_says_so_and_is_not_called_clean() {
        let mut d = disk("SATA");
        d.ata_table_available = Some(false);
        classify_storage(&mut d);
        assert!(
            d.reasons
                .iter()
                .any(|r| r.contains("ATA SMART attribute table")),
            "{:?}",
            d.reasons
        );
        assert!(
            !d.summary.contains("do not currently show"),
            "{}",
            d.summary
        );
        let mut usb = disk("USB");
        classify_storage(&mut usb);
        assert!(
            usb.reasons
                .iter()
                .all(|r| !r.contains("NVMe") && !r.contains("ATA SMART")),
            "a bridge we do not query is not charged with either: {:?}",
            usb.reasons
        );
    }

    #[test]
    fn a_failure_prediction_from_the_drive_is_an_action_and_absence_is_not_a_verdict() {
        let mut predicted = with_attribute(disk("SATA"));
        predicted.reliability.smart_predict_failure = Some(true);
        classify_storage(&mut predicted);
        assert_eq!(predicted.severity, "ActionRequired");
        assert!(
            predicted.reasons.iter().any(|r| r.contains("predicts")),
            "{:?}",
            predicted.reasons
        );
        let mut fine = with_attribute(disk("SATA"));
        fine.reliability.smart_predict_failure = Some(false);
        classify_storage(&mut fine);
        assert_eq!(fine.severity, "Normal");
        assert_eq!(parse_predict_failure(&[1, 0, 0, 0, 9, 9]), Some(true));
        assert_eq!(parse_predict_failure(&[0, 0, 0, 0]), Some(false));
        assert_eq!(
            parse_predict_failure(&[1, 0]),
            None,
            "a short answer is no answer"
        );
    }

    #[test]
    fn counters_are_compared_only_with_the_same_disk_and_never_go_negative() {
        let mut before = disk("NVMe");
        before.reliability.read_errors_uncorrected = Some(2);
        before.reliability.nvme_media_errors = Some((u128::MAX - 3).to_string());
        let mut now = before.clone();
        now.reliability.read_errors_uncorrected = Some(5);
        now.reliability.nvme_media_errors = Some(u128::MAX.to_string());
        compare_counters(&before, &mut now);
        let text = now.reasons.join(" | ");
        assert!(
            text.contains("3 more uncorrected read error(s) than at the previous scan."),
            "{text}"
        );
        assert!(
            text.contains("3 more NVMe media/data-integrity error(s)"),
            "128-bit delta is exact: {text}"
        );

        let mut unchanged = before.clone();
        compare_counters(&before, &mut unchanged);
        assert!(
            unchanged.reasons.iter().any(|r| r.contains("No increase")),
            "{:?}",
            unchanged.reasons
        );

        let mut reset = before.clone();
        reset.reliability.read_errors_uncorrected = Some(1);
        reset.reliability.nvme_media_errors = Some("0".into());
        compare_counters(&before, &mut reset);
        assert!(
            reset.reasons.is_empty(),
            "a reset is not an improvement or a delta: {:?}",
            reset.reasons
        );

        let mut replaced = before.clone();
        replaced.serial_number = "S2".into();
        replaced.reliability.read_errors_uncorrected = Some(9);
        compare_counters(&before, &mut replaced);
        assert!(
            replaced.reasons.is_empty(),
            "another disk has no past to compare with: {:?}",
            replaced.reasons
        );

        let mut unread = before.clone();
        unread.reliability.read_errors_uncorrected = None;
        compare_counters(&before, &mut unread);
        assert!(
            unread.reasons.iter().all(|r| !r.contains("read")),
            "{:?}",
            unread.reasons
        );
    }

    #[test]
    fn a_reset_or_missing_counter_cannot_reassure_about_the_other_counters() {
        let mut before = disk("NVMe");
        before.reliability.read_errors_uncorrected = Some(2);
        before.reliability.write_errors_uncorrected = Some(4);
        for read in [Some(1), None] {
            let mut now = before.clone();
            now.reliability.read_errors_uncorrected = read;
            compare_counters(&before, &mut now);
            assert!(now.reasons.is_empty(), "{:?}", now.reasons);
        }
        for malformed in ["invalid", "-1", "340282366920938463463374607431768211456"] {
            let mut now = before.clone();
            now.reliability.nvme_media_errors = Some(malformed.into());
            compare_counters(&before, &mut now);
            assert!(now.reasons.is_empty(), "{:?}", now.reasons);
        }
        let mut now = before.clone();
        now.reliability.read_errors_uncorrected = Some(1);
        now.reliability.write_errors_uncorrected = Some(7);
        compare_counters(&before, &mut now);
        assert_eq!(
            now.reasons,
            ["3 more uncorrected write error(s) than at the previous scan."]
        );
    }

    #[test]
    fn a_counter_trend_requires_complete_matching_disk_identity() {
        let mut before = disk("NVMe");
        before.reliability.read_errors_uncorrected = Some(2);
        for field in 0..10 {
            let mut before = before.clone();
            let mut now = before.clone();
            now.reliability.read_errors_uncorrected = Some(5);
            match field {
                0 => before.bus_type.clear(),
                1 => now.bus_type.clear(),
                2 => before.size_bytes = 0,
                3 => now.size_bytes = 0,
                4 => now.size_bytes += 1,
                5 => now.serial_number = "another-disk".into(),
                6 => before.bus_type = "Unknown".into(),
                7 => {
                    before.bus_type = "Unknown".into();
                    now.bus_type = "Unknown".into();
                }
                8 => {
                    before.bus_type = "BusType 0".into();
                    now.bus_type = "BusType 0".into();
                }
                _ => now.serial_number.clear(),
            }
            compare_counters(&before, &mut now);
            assert!(
                now.reasons.is_empty(),
                "identity case {field}: {:?}",
                now.reasons
            );
        }
    }

    // P81-02: the NVMe health log, read for what it says.
    fn nvme_log(critical: u8, kelvin: u16, spare: u8, threshold: u8, used: u8) -> Vec<u8> {
        let mut log = vec![0u8; 512];
        log[0] = critical;
        log[1..3].copy_from_slice(&kelvin.to_le_bytes());
        log[3] = spare;
        log[4] = threshold;
        log[5] = used;
        log
    }

    fn nvme_disk(critical: u8, spare: u8, threshold: u8, used: u8) -> StorageDeviceTelemetry {
        let mut device = StorageDeviceTelemetry {
            windows_health_status: "Healthy".into(),
            ..Default::default()
        };
        let values =
            parse_nvme_health_log(&nvme_log(critical, 310, spare, threshold, used)).unwrap();
        merge_nvme(&mut device.reliability, values);
        device.reliability.read_errors_uncorrected = Some(0);
        device.reliability.write_errors_uncorrected = Some(0);
        classify_storage(&mut device);
        device
    }

    #[test]
    fn the_spare_threshold_is_read_and_a_spare_below_it_asks_for_review() {
        let values = parse_nvme_health_log(&nvme_log(0, 310, 15, 20, 5)).unwrap();
        assert_eq!(values.spare_threshold, 20);
        let low = nvme_disk(0, 15, 20, 5);
        assert_eq!(
            low.severity, "Attention",
            "spare 15% under its own 20% threshold"
        );
        assert!(
            low.reasons
                .iter()
                .any(|r| r.contains("below the device's own threshold")),
            "{:?}",
            low.reasons
        );
        assert_eq!(nvme_disk(0, 100, 10, 5).severity, "Normal");
        assert_eq!(
            nvme_disk(0, 10, 10, 5).severity,
            "Normal",
            "equal is not below"
        );
    }

    #[test]
    fn critical_warning_bits_are_named_by_meaning_and_unknown_bits_are_not_dropped() {
        let device = nvme_disk(0b1100_0101, 100, 10, 5);
        assert_eq!(device.severity, "ActionRequired");
        let reasons = device.reasons.join(" | ");
        for meaning in [
            "spare has fallen below its threshold",
            "reliability is degraded",
            "recognize",
        ] {
            assert!(
                reasons.contains(meaning),
                "{meaning} missing from {reasons}"
            );
        }
        assert!(
            !reasons.contains("read-only"),
            "bit 3 was not set: {reasons}"
        );
        assert!(
            device.summary.contains("back up important data"),
            "the first advice is a backup, not a repair"
        );
    }

    #[test]
    fn wear_over_a_hundred_is_kept_and_a_zero_temperature_is_not_minus_273() {
        let hot = parse_nvme_health_log(&nvme_log(0, 0, 100, 10, 105)).unwrap();
        assert_eq!(hot.temperature_c, None, "0 K is 'not reported'");
        assert_eq!(hot.used, 105);
        let mut r = StorageReliability::default();
        merge_nvme(&mut r, hot);
        assert_eq!(r.nvme_percentage_used, Some(105));
        assert_eq!(r.wear_percent_used, Some(105), "not clamped to 100");
        assert_eq!(r.nvme_available_spare_threshold_percent, Some(10));
        assert_eq!(r.temperature_c, None);
    }

    #[test]
    fn counters_beyond_64_bits_keep_every_digit() {
        let mut log = nvme_log(0, 310, 100, 10, 1);
        log[160..176].copy_from_slice(&u128::MAX.to_le_bytes());
        let values = parse_nvme_health_log(&log).unwrap();
        assert_eq!(
            values.media_errors,
            "340282366920938463463374607431768211455"
        );
        assert!(
            parse_nvme_health_log(&log[..100]).is_err(),
            "a short buffer is an error, not a panic"
        );
    }

    #[test]
    fn only_an_absent_class_is_not_a_fault() {
        for absent in [0x8004_1010_u32, 0x8004_1002, 0x8004_100C, 0x8004_100E] {
            assert!(is_absent_wmi_class(absent as i32), "{absent:#X}");
        }
        assert!(
            !is_absent_wmi_class(0x8004_1017_u32 as i32),
            "an invalid query is a bug, not an absent class"
        );
        assert!(
            !is_absent_wmi_class(0x8004_1003_u32 as i32),
            "access denied is a fault"
        );
        assert!(!is_absent_wmi_class(0));
    }

    #[test]
    fn a_different_bus_or_size_is_a_mismatch_even_when_the_serial_agrees() {
        let wmi = identity("S123", "NVMe", Some(1_000));
        assert_eq!(
            bind_handle(&wmi, &identity("S123", "USB", Some(1_000))),
            HandleBinding::Mismatch
        );
        assert_eq!(
            bind_handle(&wmi, &identity("S123", "NVMe", Some(2_000))),
            HandleBinding::Mismatch
        );
        assert_eq!(
            bind_handle(
                &identity("S123", "SATA", None),
                &identity("S123", "ATA", None)
            ),
            HandleBinding::Confirmed,
            "ATA and SATA are one family"
        );
    }

    #[test]
    fn a_disk_with_no_serial_is_never_confirmed_by_size_and_bus_alone() {
        let a = identity("", "SATA", Some(500));
        assert_eq!(
            bind_handle(&a, &identity("", "SATA", Some(500))),
            HandleBinding::Unproven
        );
        assert_eq!(
            bind_handle(&a, &identity("S9", "SATA", Some(500))),
            HandleBinding::Unproven
        );
        assert_eq!(
            bind_handle(&a, &identity("", "SATA", Some(900))),
            HandleBinding::Mismatch,
            "a size that differs is proof of a different disk"
        );
    }

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
        for (sample, value) in [
            (1, snapshot),
            (2, collect().expect("second read-only sample")),
        ] {
            println!(
                "read_only_hardware sample={sample} storage={} thermal={} batteries={} adapters={} counters={} deltas={} v4_default={} v6_default={} faults={}",
                value.storage.len(),
                value.thermal_zones.len(),
                value.batteries.len(),
                value.network_adapters.len(),
                value
                    .network_adapters
                    .iter()
                    .filter(|a| a.counters.is_some())
                    .count(),
                value
                    .network_adapters
                    .iter()
                    .filter(|a| a.counter_delta.is_some())
                    .count(),
                value
                    .network_adapters
                    .iter()
                    .filter(|a| a.default_route_v4 == Some(true))
                    .count(),
                value
                    .network_adapters
                    .iter()
                    .filter(|a| a.default_route_v6 == Some(true))
                    .count(),
                value.provider_faults.len()
            );
        }
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
