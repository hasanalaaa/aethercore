//! P75 trial run: what the macOS provider published on this Mac, against what the OS
//! itself reports.

#[cfg(target_os = "macos")]
mod macos {
    use std::time::Duration;

    use aethercore_performance_telemetry::{MacosPerfPlatform, PerfPlatform};

    /// statfs has no busy time. The provider put used capacity in `active_time_bp`, so a
    /// 90%-full idle disk read as 90% active and fired IO_SATURATION. Active time is not
    /// measured here and must say so, not read as a number.
    #[test]
    fn disk_active_time_is_not_capacity() {
        let snapshot = MacosPerfPlatform::new().sample(Duration::from_millis(300));
        assert!(!snapshot.storage.is_empty(), "{snapshot:?}");
        for device in &snapshot.storage {
            assert_eq!(device.active_time_bp, 0, "{device:?}");
            assert!(
                device.total_space_bytes > 0,
                "capacity is still reported: {device:?}"
            );
        }
        assert!(
            snapshot
                .collector_faults
                .iter()
                .any(|fault| fault.collector == "storage.activeTime"),
            "{:?}",
            snapshot.collector_faults
        );
    }
}
