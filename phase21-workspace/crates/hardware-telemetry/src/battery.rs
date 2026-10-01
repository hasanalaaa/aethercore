//! Batteries (`GUID_DEVINTERFACE_BATTERY`, `IOCTL_BATTERY_QUERY_INFORMATION`), P82-02B.
//!
//! One record per battery, never averaged. Capacity is reported only in the absolute unit it was
//! measured in: a battery that reports relative capacity has no mWh, and a zero is "not reported",
//! never a value to divide by.

use crate::measurements::{Availability, Battery, Coverage};

const SOURCE: &str = "IOCTL_BATTERY_QUERY_INFORMATION";
/// `BATTERY_CAPACITY_RELATIVE` in `BATTERY_INFORMATION.Capabilities`: capacities are relative
/// units, not milliwatt-hours.
const CAPACITY_RELATIVE: u32 = 0x4000_0000;

/// Builds one battery from `BATTERY_INFORMATION`'s fields. A design or full-charge capacity of 0 is
/// absent; a cycle count of 0 is absent too, because many packs never report cycles and a reported
/// 0 cannot be told from "not supported".
pub(crate) fn battery_from_information(
    stable_id: &str,
    display_name: &str,
    capabilities: u32,
    designed_capacity: u32,
    full_charged_capacity: u32,
    cycle_count: u32,
    observed_unix_ms: i64,
) -> Battery {
    let relative = capabilities & CAPACITY_RELATIVE != 0;
    let absolute = |value: u32| (!relative && value > 0).then_some(u64::from(value));
    let (availability, reason_key) = if relative {
        (
            Availability::Unsupported,
            "measurement.reason.relativeCapacity",
        )
    } else if designed_capacity == 0 && full_charged_capacity == 0 {
        (Availability::Unsupported, "measurement.reason.noCapacity")
    } else {
        (Availability::Measured, "")
    };
    Battery {
        stable_id: stable_id.to_string(),
        display_name: display_name.to_string(),
        design_capacity_mwh: absolute(designed_capacity),
        full_charge_capacity_mwh: absolute(full_charged_capacity),
        cycle_count: (cycle_count > 0 && cycle_count != u32::MAX).then_some(cycle_count),
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
    fn absolute_capacities_are_kept_apart_and_a_zero_design_is_absent() {
        let b = battery_from_information("b0", "Battery", 0, 56_000, 41_000, 312, 9);
        assert_eq!(
            (b.design_capacity_mwh, b.full_charge_capacity_mwh),
            (Some(56_000), Some(41_000))
        );
        assert_eq!(b.cycle_count, Some(312));
        assert_eq!(b.coverage.availability, Availability::Measured);
        let no_design = battery_from_information("b0", "Battery", 0, 0, 41_000, 0, 9);
        assert_eq!(
            no_design.design_capacity_mwh, None,
            "0 is not a design capacity to divide by"
        );
        assert_eq!(no_design.full_charge_capacity_mwh, Some(41_000));
        assert_eq!(
            no_design.cycle_count, None,
            "0 cycles cannot be told from 'not reported'"
        );
    }

    #[test]
    fn relative_capacity_is_never_shown_as_milliwatt_hours() {
        let b = battery_from_information("b0", "Battery", CAPACITY_RELATIVE, 100, 87, 5, 9);
        assert_eq!(
            (b.design_capacity_mwh, b.full_charge_capacity_mwh),
            (None, None)
        );
        assert_eq!(b.coverage.availability, Availability::Unsupported);
        assert_eq!(b.coverage.reason_key, "measurement.reason.relativeCapacity");
    }

    #[test]
    fn a_battery_reporting_no_capacity_at_all_says_so_and_an_unset_cycle_count_is_absent() {
        let b = battery_from_information("b0", "Battery", 0, 0, 0, u32::MAX, 9);
        assert_eq!(b.coverage.availability, Availability::Unsupported);
        assert_eq!(b.coverage.reason_key, "measurement.reason.noCapacity");
        assert_eq!(b.cycle_count, None);
    }
}
