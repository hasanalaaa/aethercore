#![cfg(windows)]

use std::{fs, time::{SystemTime, UNIX_EPOCH}};

#[test]
#[ignore = "exports a real bound OEM driver package; run explicitly as administrator"]
fn exports_a_bound_oem_driver_package_with_manifest() {
    let devices = aethercore_windows_pnp::scan_present_devices()
        .expect("PnP inventory must succeed");
    let Some(inf) = devices.iter()
        .filter_map(|device| device.driver.as_ref())
        .map(|driver| driver.inf_path.clone())
        .find(|inf| aethercore_driver_backup::validate_oem_inf_name(inf))
    else {
        eprintln!("No present device with an OEM INF was found; backup probe has nothing applicable to export.");
        return;
    };

    let nonce = SystemTime::now().duration_since(UNIX_EPOCH).expect("clock").as_millis();
    let root = std::env::temp_dir().join(format!("aethercore-phase3-backup-probe-{nonce}"));
    let evidence = aethercore_driver_backup::export_driver_package(&inf, &root)
        .expect("PnPUtil OEM driver export must succeed");
    assert!(evidence.file_count > 0);
    assert!(!evidence.not_applicable);
    assert!(std::path::Path::new(&evidence.manifest_path).is_file());
    let _ = fs::remove_dir_all(root);
}
