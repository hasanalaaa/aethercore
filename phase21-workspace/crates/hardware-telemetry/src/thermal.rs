//! ACPI thermal zones (`MSAcpi_ThermalZoneTemperature`), P82-02A.
//!
//! What firmware publishes is a zone, not a component: nothing here names a zone "CPU". A
//! temperature is a reading only when it is plausible; zero or absurd values are said to be
//! unavailable, never shown as a temperature.

use crate::measurements::{Availability, Coverage, ThermalZone};

const SOURCE: &str = "MSAcpi_ThermalZoneTemperature";
/// -40 °C .. 150 °C in tenths of a kelvin: outside it the firmware is not reporting a temperature.
const PLAUSIBLE_DECIKELVIN: std::ops::RangeInclusive<u32> = 2_332..=4_232;

/// Tenths of a kelvin to whole degrees Celsius (3000 → 27, from 26.85).
fn celsius(decikelvin: u32) -> i32 {
    (f64::from(decikelvin) / 10.0 - 273.15).round() as i32
}

/// Builds one zone from a WMI row. `current` and `critical` are tenths of a kelvin as WMI
/// returns them; `None` is a property the row did not carry.
pub(crate) fn zone_from_wmi(
    instance_name: &str,
    current: Option<u32>,
    critical: Option<u32>,
    observed_unix_ms: i64,
) -> ThermalZone {
    let plausible = |v: Option<u32>| v.filter(|dk| PLAUSIBLE_DECIKELVIN.contains(dk));
    let (availability, reason_key) = match current {
        Some(dk) if PLAUSIBLE_DECIKELVIN.contains(&dk) => (Availability::Measured, ""),
        // Present but zero (or not carried): the zone exists and gives no reading.
        None | Some(0) => (Availability::Unsupported, "measurement.reason.noReading"),
        Some(_) => (
            Availability::Failed,
            "measurement.reason.implausibleReading",
        ),
    };
    ThermalZone {
        stable_id: instance_name.to_string(),
        display_name: instance_name.to_string(),
        temperature_c: plausible(current).map(celsius),
        critical_c: plausible(critical).map(celsius),
        highest_observed_c: None,
        coverage: Coverage {
            source: SOURCE.into(),
            observed_unix_ms: Some(observed_unix_ms),
            window_days: None,
            availability,
            reason_key: reason_key.into(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tenths_of_a_kelvin_become_degrees_and_a_zone_is_not_named_after_a_component() {
        let zone = zone_from_wmi(r"ACPI\ThermalZone\TZ00_0", Some(3_000), Some(3_732), 5);
        assert_eq!(zone.temperature_c, Some(27), "3000 deci-K is 26.85 °C");
        assert_eq!(zone.critical_c, Some(100), "3732 deci-K is 100.05 °C");
        assert_eq!(zone.coverage.availability, Availability::Measured);
        assert_eq!(
            zone.display_name, r"ACPI\ThermalZone\TZ00_0",
            "firmware names a zone, not a CPU"
        );
        assert_eq!(zone.coverage.observed_unix_ms, Some(5));
    }

    #[test]
    fn zero_missing_and_absurd_readings_are_unavailable_never_a_temperature() {
        let zero = zone_from_wmi("z", Some(0), None, 1);
        assert_eq!(
            (zero.temperature_c, zero.coverage.availability),
            (None, Availability::Unsupported)
        );
        let missing = zone_from_wmi("z", None, None, 1);
        assert_eq!(
            (missing.temperature_c, missing.coverage.availability),
            (None, Availability::Unsupported)
        );
        let absurd = zone_from_wmi("z", Some(9_999), Some(1), 1);
        assert_eq!(
            (absurd.temperature_c, absurd.critical_c),
            (None, None),
            "-273 °C and 726 °C are not readings"
        );
        assert_eq!(absurd.coverage.availability, Availability::Failed);
        assert_eq!(
            absurd.coverage.reason_key,
            "measurement.reason.implausibleReading"
        );
    }

    #[test]
    fn a_critical_trip_point_is_kept_only_when_plausible_and_apart_from_the_reading() {
        let zone = zone_from_wmi("z", Some(3_100), Some(0), 1);
        assert_eq!(zone.temperature_c, Some(37));
        assert_eq!(zone.critical_c, None, "0 is not a rated threshold");
    }
}
