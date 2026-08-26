//! Phase 26 — GE gate: the unix --foreground run prints the honest capability matrix
//! and (via the intelligence-core activation contract) the embedded-reasoner line is
//! asserted by T1 in crates/intelligence-core; this test pins the matrix + scaffold.

use std::process::Command;

#[test]
#[cfg(unix)]
fn foreground_run_prints_honest_capability_matrix() {
    let bin = std::path::Path::new(env!("CARGO_BIN_EXE_aethercore-maintenance-service"));
    let output = Command::new(bin).arg("--foreground").output().expect("run service");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "foreground run exits 0");
    assert!(
        stdout.contains("starting in foreground mode"),
        "mode line missing: {stdout}"
    );
    // Honest matrix spot checks — both directions of honesty:
    assert!(stdout.contains("capability telemetryCpu: native"), "{stdout}");
    assert!(
        stdout.contains("capability driverServicing: not-available"),
        "NotAvailable must be printed, never simulated: {stdout}"
    );
    assert!(stdout.contains("capability careOrchestration: native"));
}
