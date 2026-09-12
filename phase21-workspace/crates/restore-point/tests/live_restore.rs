#![cfg(windows)]

use std::time::{SystemTime, UNIX_EPOCH};

#[test]
#[ignore = "creates then cancels a real Windows System Restore point; run explicitly as administrator"]
fn creates_verifies_and_cancels_a_fresh_restore_point() {
    aethercore_restore_point::initialize_process_com_security()
        .expect("process COM security must be available before System Restore");
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_secs();
    let plan_id = format!("phase3-live-probe-{nonce}");
    let evidence = aethercore_restore_point::begin_driver_install(&plan_id)
        .expect("Windows must create and WMI must verify a fresh AetherCore restore point");
    assert!(evidence.verified_fresh);
    assert!(evidence.sequence_number > 0);
    aethercore_restore_point::cancel_driver_install(
        evidence.sequence_number,
        &evidence.description,
    )
    .expect("probe restore point must be cancelled cleanly");
}
