//! P75 trial run: what the macOS provider published on this Mac, against what the OS
//! itself reports.

#[cfg(target_os = "macos")]
mod macos {
    use std::time::Duration;

    use aethercore_performance_telemetry::{MacosPerfPlatform, PerfPlatform};

    fn sysctl(name: &str) -> u64 {
        let out = std::process::Command::new("/usr/sbin/sysctl")
            .args(["-n", name])
            .output()
            .expect("sysctl runs");
        String::from_utf8_lossy(&out.stdout)
            .trim()
            .parse()
            .expect("a number")
    }

    /// The provider published 98% memory load while `memory_pressure` said 64% free:
    /// "available" counted free pages plus at most the purgeable ones, so every page of
    /// reclaimable file cache read as used. The kernel's own figure is
    /// `kern.memorystatus_level` (percent available).
    #[test]
    fn memory_load_is_the_kernels_not_free_pages() {
        let snapshot = MacosPerfPlatform::new().sample(Duration::from_millis(300));
        let memory = snapshot.memory;
        let kernel_load = 100 - sysctl("kern.memorystatus_level");
        let published = u64::from(memory.memory_load_percent);
        assert!(
            published.abs_diff(kernel_load) <= 10,
            "published {published}% load, the kernel says {kernel_load}%"
        );
    }

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
