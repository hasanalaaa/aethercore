#![cfg(windows)]

#[test]
#[ignore = "requires a real Windows PnP tree"]
fn inventories_present_hardware_nodes() {
    let devices = aethercore_windows_pnp::scan_present_devices()
        .expect("SetupAPI/Configuration Manager inventory should complete");

    assert!(
        !devices.is_empty(),
        "a physical Windows machine should expose present PnP devices"
    );
    assert!(
        devices
            .iter()
            .all(|device| !device.instance_id.trim().is_empty())
    );
    assert!(devices.iter().all(|device| {
        !device.enumerator.eq_ignore_ascii_case("SWD")
            && !device.instance_id.to_ascii_uppercase().starts_with("SWD\\")
    }));
    assert!(
        devices
            .iter()
            .all(|device| device.status.has_problem || device.status.problem_code == 0)
    );
    assert!(devices.iter().all(|device| {
        !device.status.missing_driver
            || (device.status.has_problem && device.status.problem_code == 28)
    }));
}
