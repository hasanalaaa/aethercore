//! P80-02A (D6): the measurement contract keeps "not measured" apart from "measured zero",
//! survives a snapshot stored before it existed, and stays inside its limits.

use aethercore_diagnostic_engine::DiagnosticsSnapshot;
use aethercore_hardware_telemetry::StorageReliability;
use aethercore_hardware_telemetry::measurements::{
    Availability, Battery, BootRecord, Coverage, MAX_BATTERIES, MAX_BOOTS, MAX_NETWORK_ADAPTERS,
    MAX_THERMAL_ZONES, NetworkAdapter, ThermalZone, capped,
};

fn coverage() -> Coverage {
    Coverage {
        source: "ACPI thermal zone".into(),
        observed_unix_ms: Some(1_700_000_000_000),
        window_days: None,
        availability: Availability::Measured,
        reason_key: String::new(),
    }
}

#[test]
fn a_snapshot_stored_before_measurements_decodes_without_inventing_sensors() {
    let mut old = serde_json::to_value(DiagnosticsSnapshot::default()).unwrap();
    for key in ["thermalZones", "batteries", "boots", "networkAdapters"] {
        assert!(
            old.as_object_mut().unwrap().remove(key).is_some(),
            "{key} is part of the snapshot"
        );
    }
    let decoded: DiagnosticsSnapshot = serde_json::from_value(old).unwrap();
    assert!(decoded.thermal_zones.is_empty() && decoded.batteries.is_empty());
    assert!(decoded.boots.is_empty() && decoded.network_adapters.is_empty());
}

#[test]
fn an_absent_reading_is_not_a_zero_reading() {
    let absent = ThermalZone {
        stable_id: "tz0".into(),
        coverage: coverage(),
        ..Default::default()
    };
    let zero = ThermalZone {
        temperature_c: Some(0),
        ..absent.clone()
    };
    let back = |z: &ThermalZone| -> ThermalZone {
        serde_json::from_str(&serde_json::to_string(z).unwrap()).unwrap()
    };
    assert_eq!(back(&absent).temperature_c, None);
    assert_eq!(back(&zero).temperature_c, Some(0));
    assert_ne!(back(&absent), back(&zero));
}

#[test]
fn an_unknown_availability_from_a_newer_writer_decodes_as_unknown() {
    let c: Coverage =
        serde_json::from_str(r#"{"source":"x","availability":"somethingNewer"}"#).unwrap();
    assert_eq!(c.availability, Availability::Unknown);
    for (text, value) in [
        ("unsupported", Availability::Unsupported),
        ("denied", Availability::Denied),
        ("notMeasured", Availability::NotMeasured),
    ] {
        let c: Coverage = serde_json::from_str(&format!(r#"{{"availability":"{text}"}}"#)).unwrap();
        assert_eq!(
            c.availability, value,
            "empty, unsupported and denied are different answers"
        );
    }
}

#[test]
fn design_capacity_is_kept_apart_from_current_and_rated_from_observed() {
    let battery = Battery {
        design_capacity_mwh: Some(50_000),
        full_charge_capacity_mwh: Some(41_000),
        ..Default::default()
    };
    let zone = ThermalZone {
        highest_observed_c: Some(88),
        critical_c: Some(105),
        ..Default::default()
    };
    let b: Battery = serde_json::from_str(&serde_json::to_string(&battery).unwrap()).unwrap();
    let z: ThermalZone = serde_json::from_str(&serde_json::to_string(&zone).unwrap()).unwrap();
    assert_eq!(
        (b.design_capacity_mwh, b.full_charge_capacity_mwh),
        (Some(50_000), Some(41_000))
    );
    assert_eq!((z.highest_observed_c, z.critical_c), (Some(88), Some(105)));
}

#[test]
fn nvme_counters_keep_all_128_bits_and_the_spare_threshold_survives() {
    let r = StorageReliability {
        nvme_media_errors: Some(u128::MAX.to_string()),
        nvme_available_spare_threshold_percent: Some(10),
        ..Default::default()
    };
    let back: StorageReliability =
        serde_json::from_str(&serde_json::to_string(&r).unwrap()).unwrap();
    assert_eq!(
        back.nvme_media_errors.as_deref(),
        Some("340282366920938463463374607431768211455")
    );
    assert_eq!(back.nvme_available_spare_threshold_percent, Some(10));
    let old: StorageReliability = serde_json::from_str("{}").unwrap();
    assert_eq!(
        old.nvme_available_spare_threshold_percent, None,
        "an older snapshot has no threshold, not 0"
    );
}

#[test]
fn the_limits_are_applied_and_the_largest_snapshot_fits_its_byte_budget() {
    let (zones, cut) = capped(
        (0..40)
            .map(|i| ThermalZone {
                stable_id: format!("tz{i}"),
                ..Default::default()
            })
            .collect(),
        MAX_THERMAL_ZONES,
    );
    assert_eq!((zones.len(), cut), (32, true));
    let (kept, cut) = capped(vec![1, 2, 3], 3);
    assert_eq!((kept.len(), cut), (3, false), "at the limit nothing is cut");

    let name = "N".repeat(256);
    let long = || Coverage {
        source: name.clone(),
        reason_key: name.clone(),
        ..coverage()
    };
    let snapshot = DiagnosticsSnapshot {
        thermal_zones: (0..MAX_THERMAL_ZONES)
            .map(|_| ThermalZone {
                stable_id: name.clone(),
                display_name: name.clone(),
                temperature_c: Some(1),
                critical_c: Some(1),
                highest_observed_c: Some(1),
                coverage: long(),
            })
            .collect(),
        batteries: (0..MAX_BATTERIES)
            .map(|_| Battery {
                stable_id: name.clone(),
                display_name: name.clone(),
                coverage: long(),
                ..Default::default()
            })
            .collect(),
        boots: (0..MAX_BOOTS)
            .map(|_| BootRecord {
                coverage: long(),
                ..Default::default()
            })
            .collect(),
        network_adapters: (0..MAX_NETWORK_ADAPTERS)
            .map(|_| NetworkAdapter {
                stable_id: name.clone(),
                display_name: name.clone(),
                coverage: long(),
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    };
    let bytes = serde_json::to_string(&snapshot).unwrap().len();
    assert!(
        bytes < 256 * 1024,
        "the four domains at their limits take {bytes} bytes"
    );
}
