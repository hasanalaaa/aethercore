//! Boot performance evidence: event 100 of `Microsoft-Windows-Diagnostics-Performance` (P83-01A).
//!
//! The fields are read by name from the event's data, never by position, and only when the event's
//! own schema version is the one this reader was built against (2, as observed on a real Windows
//! 11 machine). A different version, or an event without the fields, is not a boot record: it is
//! left out as unknown rather than read at guessed offsets. Boot duration is `BootTime` in
//! milliseconds; uptime is a different quantity and is not used here.

use serde::{Deserialize, Serialize};

/// How many boots are kept: the most recent, one per boot instance.
pub const MAX_BOOTS: usize = 20;
#[cfg(any(windows, test))]
const SCHEMA_VERSION: &str = "2";

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BootEvidence {
    /// `SystemBootInstance`: which boot this is. Two events for one instance are one boot.
    pub system_boot_instance: Option<u64>,
    /// When the event was written (milliseconds since the Unix epoch).
    pub recorded_unix_ms: i64,
    /// `BootTime`: the whole boot, in milliseconds.
    pub boot_time_ms: Option<u64>,
    /// `MainPathBootTime`, in milliseconds.
    pub main_path_ms: Option<u64>,
    /// `BootPostBootTime`, in milliseconds.
    pub post_boot_ms: Option<u64>,
    /// `BootNumStartupApps`.
    pub startup_apps: Option<u32>,
}

/// Builds a boot record from the event's named values. `field` returns a value's text by its name.
/// `None` when the schema version is not the known one or when the event carries neither a boot
/// duration nor a boot instance (it is not a boot-performance record).
#[cfg(any(windows, test))]
pub(crate) fn boot_from_fields(
    field: &dyn Fn(&str) -> Option<String>,
    recorded_unix_ms: i64,
) -> Option<BootEvidence> {
    if field("BootTsVersion")?.trim() != SCHEMA_VERSION {
        return None;
    }
    let number = |name: &str| field(name).and_then(|v| v.trim().parse::<u64>().ok());
    let boot = BootEvidence {
        system_boot_instance: number("SystemBootInstance"),
        recorded_unix_ms,
        boot_time_ms: number("BootTime"),
        main_path_ms: number("MainPathBootTime"),
        post_boot_ms: number("BootPostBootTime"),
        startup_apps: number("BootNumStartupApps").and_then(|n| u32::try_from(n).ok()),
    };
    (boot.boot_time_ms.is_some() || boot.system_boot_instance.is_some()).then_some(boot)
}

/// Newest first, one record per boot instance (the newest wins), at most [`MAX_BOOTS`]. A record
/// with no instance cannot be matched to another and is kept as it is.
#[cfg(any(windows, test))]
pub(crate) fn dedupe_boots(mut boots: Vec<BootEvidence>) -> Vec<BootEvidence> {
    boots.sort_by_key(|b| std::cmp::Reverse(b.recorded_unix_ms));
    let mut seen = std::collections::BTreeSet::new();
    boots.retain(|b| {
        b.system_boot_instance
            .is_none_or(|instance| seen.insert(instance))
    });
    boots.truncate(MAX_BOOTS);
    boots
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    /// The named values of a real event 100 (a Windows 11 machine, schema version 2).
    fn real_event() -> BTreeMap<&'static str, &'static str> {
        BTreeMap::from([
            ("BootTsVersion", "2"),
            ("SystemBootInstance", "9"),
            ("BootTime", "25414"),
            ("MainPathBootTime", "8714"),
            ("BootPostBootTime", "16700"),
            ("BootNumStartupApps", "12"),
        ])
    }
    fn read<'a>(
        map: &'a BTreeMap<&'static str, &'static str>,
    ) -> impl Fn(&str) -> Option<String> + 'a {
        move |name: &str| map.get(name).map(|v| (*v).to_string())
    }

    #[test]
    fn a_real_event_gives_its_boot_duration_in_milliseconds_and_no_uptime() {
        let map = real_event();
        let boot = boot_from_fields(&read(&map), 1_000).expect("a boot record");
        assert_eq!(
            boot.boot_time_ms,
            Some(25_414),
            "25.4 seconds once the UI divides by 1000"
        );
        assert_eq!(
            (boot.main_path_ms, boot.post_boot_ms, boot.startup_apps),
            (Some(8_714), Some(16_700), Some(12))
        );
        assert_eq!(boot.system_boot_instance, Some(9));
    }

    #[test]
    fn a_different_schema_or_a_missing_field_is_unknown_not_a_guess() {
        let mut other = real_event();
        other.insert("BootTsVersion", "3");
        assert!(
            boot_from_fields(&read(&other), 1).is_none(),
            "an unknown schema version is not read"
        );
        let mut no_version = real_event();
        no_version.remove("BootTsVersion");
        assert!(boot_from_fields(&read(&no_version), 1).is_none());
        let mut no_time = real_event();
        no_time.remove("BootTime");
        let boot =
            boot_from_fields(&read(&no_time), 1).expect("the instance still identifies a boot");
        assert_eq!(
            boot.boot_time_ms, None,
            "a missing duration is absent, not 0"
        );
        let mut nothing = real_event();
        nothing.remove("BootTime");
        nothing.remove("SystemBootInstance");
        assert!(boot_from_fields(&read(&nothing), 1).is_none());
        let mut bad = real_event();
        bad.insert("BootTime", "abc");
        assert_eq!(
            boot_from_fields(&read(&bad), 1).unwrap().boot_time_ms,
            None,
            "not a number is absent"
        );
    }

    #[test]
    fn one_boot_instance_is_one_boot_and_the_newest_first_within_the_limit() {
        let boot = |instance: Option<u64>, at: i64| BootEvidence {
            system_boot_instance: instance,
            recorded_unix_ms: at,
            ..Default::default()
        };
        let kept = dedupe_boots(vec![
            boot(Some(1), 10),
            boot(Some(2), 30),
            boot(Some(1), 20),
            boot(None, 5),
            boot(None, 6),
        ]);
        assert_eq!(
            kept.iter().map(|b| b.recorded_unix_ms).collect::<Vec<_>>(),
            vec![30, 20, 6, 5],
            "instance 1 once (the newer), newest first"
        );
        let many: Vec<_> = (0..50).map(|i| boot(Some(i), i as i64)).collect();
        assert_eq!(dedupe_boots(many).len(), MAX_BOOTS);
    }
}
