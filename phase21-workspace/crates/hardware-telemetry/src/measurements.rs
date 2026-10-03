//! The typed measurement domains of the diagnostics snapshot (P80-02A, owner decision D6):
//! thermal zones, batteries, boots and network adapters, each with the coverage of its reading.
//!
//! An absent reading is `None`, never a zero sentinel: a sensor that did not answer must not
//! read as "0 °C" or "0 Wh". Nothing here is a plugin registry or a JSON bag; a producer that
//! measures nothing leaves its list empty and says why through `Coverage`.

use serde::{Deserialize, Serialize};

pub const MAX_THERMAL_ZONES: usize = 32;
pub const MAX_BATTERIES: usize = 32;
pub const MAX_NETWORK_ADAPTERS: usize = 128;
pub const MAX_BOOTS: usize = 20;

/// Whether a domain was measured. `Unknown` is what an older reader makes of a value a newer
/// writer added; it is never read as "measured".
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Availability {
    Measured,
    /// No provider ran for this domain (a different answer from "the sensor read nothing").
    NotMeasured,
    Unsupported,
    Denied,
    Failed,
    // serde requires `other` on the last variant.
    #[default]
    #[serde(other)]
    Unknown,
}

/// Where a reading came from, when it was observed and over what window, and why it is missing.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Coverage {
    pub source: String,
    pub observed_unix_ms: Option<i64>,
    pub window_days: Option<u32>,
    pub availability: Availability,
    /// A catalog key for the reason; empty when there is nothing to explain.
    pub reason_key: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct ThermalZone {
    pub stable_id: String,
    pub display_name: String,
    pub temperature_c: Option<i32>,
    /// The device's rated critical threshold, not a reading.
    pub critical_c: Option<i32>,
    /// Exact firmware units; whole Celsius values above are for display only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature_decikelvin: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub critical_decikelvin: Option<u32>,
    /// The highest value observed, kept apart from the rated threshold above.
    pub highest_observed_c: Option<i32>,
    pub coverage: Coverage,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Battery {
    pub stable_id: String,
    pub display_name: String,
    /// What the battery was built for, kept apart from what it holds now.
    pub design_capacity_mwh: Option<u64>,
    pub full_charge_capacity_mwh: Option<u64>,
    /// Undefined relative units reported by BATTERY_CAPACITY_RELATIVE; never mWh.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub design_capacity_relative: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_charge_capacity_relative: Option<u32>,
    pub cycle_count: Option<u32>,
    pub coverage: Coverage,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct BootRecord {
    pub recorded_unix_ms: i64,
    pub duration_ms: Option<u64>,
    pub coverage: Coverage,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct NetworkAdapter {
    pub stable_id: String,
    pub display_name: String,
    pub link_speed_bps: Option<u64>,
    /// `IF_OPER_STATUS`: 1 up, 2 down, 3 testing, 4 unknown, 5 dormant, 6 not present,
    /// 7 lower layer down. Absent when not reported.
    pub operational_status: Option<u8>,
    /// The media connection (a cable or an association): absent when not reported.
    pub connected: Option<bool>,
    /// Windows' own flag for a virtual adapter (VPN, virtual switch): a name, not a fault.
    pub is_virtual: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub admin_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ipv4_apipa: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_route_v4: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_route_v6: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub counters: Option<NetworkCounters>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub counter_delta: Option<NetworkCounterDelta>,
    #[serde(skip_serializing_if = "unknown_availability")]
    pub counter_availability: Availability,
    #[serde(skip_serializing_if = "unknown_availability")]
    pub route_availability: Availability,
    pub coverage: Coverage,
}

fn unknown_availability(value: &Availability) -> bool {
    *value == Availability::Unknown
}

/// Cumulative interface counters; zero is a successful measurement.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct NetworkCounters {
    pub in_octets: u64,
    pub out_octets: u64,
    pub in_errors: u64,
    pub out_errors: u64,
    pub in_discards: u64,
    pub out_discards: u64,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct NetworkCounterDelta {
    pub elapsed_ms: u64,
    pub counts: NetworkCounters,
}

/// Applies a domain's limit. The flag says something was cut, so the producer can warn instead
/// of presenting a truncated list as the whole inventory.
pub fn capped<T>(mut items: Vec<T>, max: usize) -> (Vec<T>, bool) {
    let cut = items.len() > max;
    items.truncate(max);
    (items, cut)
}
