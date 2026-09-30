//! P78-02B: the Windows assessment runs its four long checks through `bounded::run_check`. The
//! platform is Windows-only COM and process code, so this reads its source: a check put back as a
//! plain call would hang the screen on "Assessing" again.

#[test]
fn the_long_checks_run_under_a_deadline_and_a_slot() {
    let source: String = include_str!("../src/windows_impl.rs")
        .split_whitespace()
        .collect();
    for slot in [
        "DISM_SLOT",
        "SFC_SLOT",
        "DISK_SLOT",
        "UPDATE_SLOT",
        "UPDATE_HISTORY_SLOT",
    ] {
        assert!(
            source.contains(&format!("run_check(&{slot},")),
            "{slot} is not passed to run_check: its check runs unbounded"
        );
    }
}
