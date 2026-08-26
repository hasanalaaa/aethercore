//! Lifecycle manager artifacts (T6 / CX-5): ARTIFACTS ONLY.
//!
//! `aetherctl service units --print` emits both unit files to stdout and writes NOTHING
//! to disk. Nothing anywhere auto-installs them — installation is an explicit, human,
//! out-of-band act (launchctl / systemctl by the administrator).

/// launchd user agent (validated live via `plutil -lint`; gate P28 records the result).
pub const LAUNCHD_PLIST: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../packaging/launchd/com.aethercore.maintenance.plist"
));

/// systemd service unit (statically asserted; runtime verification honestly deferred to
/// a Linux host, QD-028-002).
pub const SYSTEMD_UNIT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../packaging/systemd/aethercore-maintenance.service"
));

pub fn print_units() {
    println!("===== packaging/launchd/com.aethercore.maintenance.plist =====");
    print!("{LAUNCHD_PLIST}");
    println!("===== packaging/systemd/aethercore-maintenance.service =====");
    print!("{SYSTEMD_UNIT}");
}
