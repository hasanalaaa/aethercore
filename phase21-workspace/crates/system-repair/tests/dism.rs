//! P85-02: what the DISM API's callback and HRESULTs mean, decided where they can be tested.

use aethercore_system_repair::{
    RepairError,
    dism::{progress_percent, restore_error},
};

#[test]
fn a_callback_is_a_percent_only_when_it_has_a_total() {
    assert_eq!(
        progress_percent(0, 0),
        None,
        "no total: unknown, not 0% and not 100%"
    );
    assert_eq!(progress_percent(40, 100), Some(40));
    assert_eq!(progress_percent(100, 100), Some(100));
    assert_eq!(progress_percent(1, 3), Some(33));
    assert_eq!(progress_percent(5, 0), None);
    assert_eq!(
        progress_percent(101, 100),
        None,
        "a current past the total is malformed, not 100%"
    );
    assert_eq!(
        progress_percent(u32::MAX, u32::MAX),
        Some(100),
        "no overflow on large totals"
    );
}

#[test]
fn restore_hresults_are_typed() {
    // 0x800F081F: the component store source files are missing. Local-only restore cannot proceed.
    assert!(matches!(
        restore_error(0x800F_081Fu32 as i32),
        RepairError::SourceRequired(_)
    ));
    // 0x800704C7: the owner's cancel reached the API. The image is unknown until verified again.
    assert!(matches!(
        restore_error(0x8007_04C7u32 as i32),
        RepairError::RepairStopped
    ));
    match restore_error(0x8007_0005u32 as i32) {
        RepairError::Command(detail) => assert!(detail.contains("0x80070005"), "{detail}"),
        other => panic!("{other:?}"),
    }
}
