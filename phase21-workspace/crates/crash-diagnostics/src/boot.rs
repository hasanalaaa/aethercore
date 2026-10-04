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
pub const MAX_BOOT_DELAYS: usize = 32;
#[cfg(any(windows, test))]
pub(crate) const PERFORMANCE_PROVIDER: &str = "Microsoft-Windows-Diagnostics-Performance";
#[cfg(any(windows, test))]
const FILETIME_EPOCH: u64 = 116_444_736_000_000_000;

/// Exact Windows Kernel-Boot event 27/version1 token; no undocumented cold/fast mapping.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RawBootClass {
    pub event_version: u8,
    pub value: u32,
}

/// Event101 is an application; event103 identifies a service *binary*, never an SCM name.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BootDelayEvidence {
    pub boot_start_filetime: u64,
    pub event_id: u32,
    pub full_path: String,
    pub total_time_ms: u32,
    pub degradation_time_ms: u32,
    pub recorded_unix_ms: i64,
}
#[cfg(any(windows, test))]
#[derive(Clone, Debug)]
pub(crate) struct KernelBootClass {
    pub recorded_filetime: u64,
    pub event_version: u8,
    pub value: u32,
}

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
    #[serde(default)]
    pub source_version: Option<u8>,
    /// Preserve FILETIME 100ns precision for cross-provider and exact delay association.
    #[serde(default)]
    pub boot_start_filetime: Option<u64>,
    #[serde(default)]
    pub boot_end_filetime: Option<u64>,
    #[serde(default)]
    pub recorded_filetime: Option<u64>,
    #[serde(default)]
    pub completed_measurement: bool,
    #[serde(default)]
    pub raw_class: Option<RawBootClass>,
    #[serde(default)]
    pub delays: Vec<BootDelayEvidence>,
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
        ..Default::default()
    };
    (boot.boot_time_ms.is_some() || boot.system_boot_instance.is_some()).then_some(boot)
}

/// Qualifies actual event100 system metadata independently from payload BootTsVersion.
#[cfg(any(windows, test))]
pub(crate) fn boot_from_event(
    provider: &str,
    event_id: u32,
    version: Option<u8>,
    field: &dyn Fn(&str) -> Option<String>,
    recorded_filetime: u64,
    observed_unix_ms: i64,
) -> Option<BootEvidence> {
    if provider != PERFORMANCE_PROVIDER || event_id != 100 || version != Some(2) {
        return None;
    }
    let recorded_unix_ms = unix_ms(recorded_filetime)?;
    let mut boot = boot_from_fields(field, recorded_unix_ms)?;
    boot.source_version = version;
    boot.recorded_filetime = Some(recorded_filetime);
    let ticks = |name: &str| {
        field(name)
            .and_then(|v| v.parse::<u64>().ok())
            .filter(|v| *v > FILETIME_EPOCH)
    };
    boot.boot_start_filetime = ticks("BootStartTime");
    boot.boot_end_filetime = ticks("BootEndTime");
    boot.completed_measurement = boot
        .system_boot_instance
        .is_some_and(|v| v > 0 && u32::try_from(v).is_ok())
        && boot
            .boot_time_ms
            .is_some_and(|v| v > 0 && u32::try_from(v).is_ok())
        && recorded_unix_ms <= observed_unix_ms
        && matches!((boot.boot_start_filetime,boot.boot_end_filetime), (Some(start),Some(end)) if start<end && end<=recorded_filetime);
    Some(boot)
}
#[cfg(any(windows, test))]
fn unix_ms(ticks: u64) -> Option<i64> {
    i64::try_from(ticks.checked_sub(FILETIME_EPOCH)? / 10_000).ok()
}

/// Lexical absolute drive path only. No path access, environment expansion, basename or UNC guess.
#[cfg(any(windows, test))]
fn full_path_identity(path: &str) -> Option<String> {
    let bytes = path.as_bytes();
    if path.len() > 4096
        || bytes.len() < 4
        || !bytes[0].is_ascii_alphabetic()
        || bytes[1] != b':'
        || bytes[2] != b'\\'
    {
        return None;
    }
    if path[3..].split('\\').any(|part| {
        part.is_empty()
            || part == "."
            || part == ".."
            || part.ends_with([' ', '.'])
            || part
                .chars()
                .any(|c| c.is_control() || r#"<>:"|?*%/"#.contains(c))
    }) {
        return None;
    }
    Some(path.to_ascii_lowercase())
}

/// Read-only measured delay. Name is intentionally not interpreted as a service key.
#[cfg(any(windows, test))]
pub(crate) fn delay_from_event(
    provider: &str,
    event_id: u32,
    version: Option<u8>,
    field: &dyn Fn(&str) -> Option<String>,
    recorded_filetime: u64,
    observed_unix_ms: i64,
) -> Option<BootDelayEvidence> {
    if provider != PERFORMANCE_PROVIDER || !matches!(event_id, 101 | 103) || version != Some(1) {
        return None;
    }
    let recorded_unix_ms = unix_ms(recorded_filetime)?;
    if recorded_unix_ms > observed_unix_ms {
        return None;
    }
    let boot_start_filetime = field("StartTime")?.parse::<u64>().ok()?;
    if boot_start_filetime <= FILETIME_EPOCH || boot_start_filetime > recorded_filetime {
        return None;
    }
    let full_path = field("Path")?;
    full_path_identity(&full_path)?;
    let total_time_ms = field("TotalTime")?.parse::<u32>().ok()?;
    let degradation_time_ms = field("DegradationTime")?.parse::<u32>().ok()?;
    if total_time_ms == 0 || degradation_time_ms > total_time_ms {
        return None;
    }
    Some(BootDelayEvidence {
        boot_start_filetime,
        event_id,
        full_path,
        total_time_ms,
        degradation_time_ms,
        recorded_unix_ms,
    })
}

/// Join only complete, supported, unambiguous observations. Missing reads never become a class.
#[cfg(any(windows, test))]
pub(crate) fn bind_boot_evidence(
    mut boots: Vec<BootEvidence>,
    kernels: &[KernelBootClass],
    delays: &[BootDelayEvidence],
    kernel_complete: bool,
    delays_complete: bool,
) -> Vec<BootEvidence> {
    let unambiguous: Vec<bool> = boots.iter().enumerate().map(|(i,b)| b.completed_measurement
        && !boots.iter().enumerate().any(|(j,other)| i!=j && (b.system_boot_instance==other.system_boot_instance
            || matches!((b.boot_start_filetime,b.boot_end_filetime,other.boot_start_filetime,other.boot_end_filetime),
              (Some(start),Some(end),Some(other_start),Some(other_end)) if start<=other_end && other_start<=end)))).collect();
    for (i, boot) in boots.iter_mut().enumerate() {
        boot.raw_class = None;
        boot.delays.clear();
        if !unambiguous[i] {
            continue;
        }
        let (Some(start), Some(end)) = (boot.boot_start_filetime, boot.boot_end_filetime) else {
            continue;
        };
        if kernel_complete {
            let candidates: Vec<_> = kernels
                .iter()
                .filter(|k| start <= k.recorded_filetime && k.recorded_filetime <= end)
                .collect();
            if candidates.len() == 1 && candidates[0].event_version == 1 {
                boot.raw_class = Some(RawBootClass {
                    event_version: 1,
                    value: candidates[0].value,
                });
            }
        }
        if delays_complete {
            let candidates: Vec<_> = delays
                .iter()
                .filter(|d| {
                    d.boot_start_filetime == start
                        && matches!(d.event_id, 101 | 103)
                        && full_path_identity(&d.full_path).is_some()
                })
                .collect();
            if candidates.len() <= MAX_BOOT_DELAYS {
                boot.delays = candidates
                    .iter()
                    .filter(|d| {
                        candidates
                            .iter()
                            .filter(|other| {
                                other.event_id == d.event_id
                                    && full_path_identity(&other.full_path)
                                        == full_path_identity(&d.full_path)
                            })
                            .count()
                            == 1
                    })
                    .map(|d| (*d).clone())
                    .collect();
            }
        }
    }
    dedupe_boots(boots)
}

/// Newest first, one record per boot instance (the newest wins), at most [`MAX_BOOTS`]. A record
/// with no instance cannot be matched to another and is kept as it is.
#[cfg(any(windows, test))]
pub(crate) fn dedupe_boots(mut boots: Vec<BootEvidence>) -> Vec<BootEvidence> {
    boots.sort_by_key(|b| std::cmp::Reverse((b.recorded_unix_ms, b.recorded_filetime)));
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
    fn read<'a>(map: &'a BTreeMap<&'static str, &'a str>) -> impl Fn(&str) -> Option<String> + 'a {
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
    #[test]
    fn actual_event_identity_and_completion_are_required_before_kind_binding() {
        let fields = real_event();
        let event = |provider, id, version| {
            boot_from_event(
                provider,
                id,
                version,
                &read(&fields),
                116_444_736_000_010_000,
                1,
            )
        };
        assert!(event("Other", 100, Some(2)).is_none());
        assert!(event(PERFORMANCE_PROVIDER, 101, Some(2)).is_none());
        assert!(event(PERFORMANCE_PROVIDER, 100, None).is_none());
        assert!(event(PERFORMANCE_PROVIDER, 100, Some(3)).is_none());
        let boot = event(PERFORMANCE_PROVIDER, 100, Some(2)).unwrap();
        assert!(
            !boot.completed_measurement,
            "missing actual interval cannot qualify a kind"
        );
    }

    fn completed(instance: u64, start: u64, end: u64) -> BootEvidence {
        let mut fields = real_event();
        let instance = instance.to_string();
        let start = start.to_string();
        let end_text = end.to_string();
        fields.insert("SystemBootInstance", &instance);
        fields.insert("BootStartTime", &start);
        fields.insert("BootEndTime", &end_text);
        boot_from_event(
            PERFORMANCE_PROVIDER,
            100,
            Some(2),
            &read(&fields),
            end + 10,
            10_000,
        )
        .unwrap()
    }
    #[test]
    fn raw_class_binding_is_unique_exact_and_never_guesses_a_cold_boot() {
        let base = 116_444_736_001_000_000;
        let boots = vec![
            completed(1, base, base + 100),
            completed(2, base + 1_000, base + 1_100),
        ];
        let kernel = KernelBootClass {
            recorded_filetime: base + 22,
            event_version: 1,
            value: 0,
        };
        let bound = bind_boot_evidence(
            boots.clone(),
            std::slice::from_ref(&kernel),
            &[],
            true,
            true,
        );
        assert_eq!(bound[1].raw_class.as_ref().unwrap().value, 0);
        assert!(bound[0].raw_class.is_none());
        for candidates in [
            vec![],
            vec![kernel.clone(), kernel.clone()],
            vec![KernelBootClass {
                event_version: 2,
                ..kernel.clone()
            }],
            vec![KernelBootClass {
                recorded_filetime: base - 1,
                ..kernel.clone()
            }],
        ] {
            assert!(
                bind_boot_evidence(boots.clone(), &candidates, &[], true, true)
                    .iter()
                    .all(|b| b.raw_class.is_none())
            );
        }
        assert!(
            bind_boot_evidence(boots.clone(), &[kernel], &[], false, true)
                .iter()
                .all(|b| b.raw_class.is_none())
        );
    }
    #[test]
    fn exact_filetime_delay_join_keeps_full_path_and_rejects_one_tick_alias_or_ambiguous_identity()
    {
        let base = 116_444_736_001_000_000;
        let boot = completed(1, base, base + 100);
        let delay = BootDelayEvidence {
            boot_start_filetime: base,
            event_id: 101,
            full_path: r"C:\Program Files\Example\app.exe".into(),
            total_time_ms: 250,
            degradation_time_ms: 50,
            recorded_unix_ms: 1_000,
        };
        let bound = bind_boot_evidence(
            vec![boot.clone()],
            &[],
            std::slice::from_ref(&delay),
            true,
            true,
        );
        assert_eq!(bound[0].delays, vec![delay.clone()]);
        for wrong in [
            BootDelayEvidence {
                boot_start_filetime: base + 1,
                ..delay.clone()
            },
            BootDelayEvidence {
                full_path: "app.exe".into(),
                ..delay.clone()
            },
            BootDelayEvidence {
                event_id: 102,
                ..delay.clone()
            },
        ] {
            assert!(
                bind_boot_evidence(vec![boot.clone()], &[], &[wrong], true, true)[0]
                    .delays
                    .is_empty()
            );
        }
        assert!(
            bind_boot_evidence(
                vec![boot.clone()],
                &[],
                &[delay.clone(), delay.clone()],
                true,
                true
            )[0]
            .delays
            .is_empty()
        );
        assert!(
            bind_boot_evidence(vec![boot], &[], &[delay], true, false)[0]
                .delays
                .is_empty()
        );
    }
    #[test]
    fn conflict_overlapping_and_invalid_intervals_cannot_acquire_class_or_attribution() {
        let base = 116_444_736_001_000_000;
        let kernel = KernelBootClass {
            recorded_filetime: base + 22,
            event_version: 1,
            value: 0,
        };
        for boots in [
            vec![
                completed(1, base, base + 100),
                completed(1, base + 1_000, base + 1_100),
            ],
            vec![
                completed(1, base, base + 100),
                completed(2, base + 50, base + 150),
            ],
            vec![completed(0, base, base + 100)],
            vec![completed(1, base + 100, base)],
            vec![completed(1, base, base + 100_000_000)],
        ] {
            assert!(
                bind_boot_evidence(boots, std::slice::from_ref(&kernel), &[], true, true)
                    .iter()
                    .all(|b| b.raw_class.is_none() && b.delays.is_empty())
            );
        }
    }
    #[test]
    fn named_delay_parser_requires_supported_schema_absolute_subject_and_valid_measured_times() {
        let base = 116_444_736_001_000_000;
        let fields = std::collections::BTreeMap::from([
            ("StartTime", base.to_string()),
            ("Path", r"C:\Program Files\Example\app.exe".to_string()),
            ("TotalTime", "250".into()),
            ("DegradationTime", "50".into()),
        ]);
        let parse =
            |fields: &std::collections::BTreeMap<&str, String>, provider, id, version, observed| {
                delay_from_event(
                    provider,
                    id,
                    version,
                    &|key| fields.get(key).cloned(),
                    base + 10_000,
                    observed,
                )
            };
        for id in [101, 103] {
            let delay = parse(&fields, PERFORMANCE_PROVIDER, id, Some(1), 1_000).unwrap();
            assert_eq!(delay.boot_start_filetime, base);
            assert_eq!(delay.degradation_time_ms, 50);
        }
        assert!(parse(&fields, "Other", 101, Some(1), 1_000).is_none());
        assert!(parse(&fields, PERFORMANCE_PROVIDER, 102, Some(1), 1_000).is_none());
        assert!(parse(&fields, PERFORMANCE_PROVIDER, 101, Some(2), 1_000).is_none());
        assert!(parse(&fields, PERFORMANCE_PROVIDER, 101, None, 1_000).is_none());
        assert!(parse(&fields, PERFORMANCE_PROVIDER, 101, Some(1), 1).is_none());
        for (key, bad) in [
            ("StartTime", "0"),
            ("StartTime", "18446744073709551616"),
            ("Path", "app.exe"),
            ("Path", r"C:\Example\..\app.exe"),
            ("Path", r"\\host\share\app.exe"),
            ("Path", r"C:\%PROGRAMFILES%\app.exe"),
            ("TotalTime", "0"),
            ("TotalTime", "4294967296"),
            ("DegradationTime", "251"),
            ("DegradationTime", "not measured"),
        ] {
            let mut changed = fields.clone();
            changed.insert(key, bad.into());
            assert!(
                parse(&changed, PERFORMANCE_PROVIDER, 101, Some(1), 1_000).is_none(),
                "{key}={bad}"
            );
        }
        let mut missing = fields;
        missing.remove("Path");
        assert!(parse(&missing, PERFORMANCE_PROVIDER, 101, Some(1), 1_000).is_none());
    }
    #[test]
    fn attribution_cap_is_fail_closed_not_a_partially_qualified_sample() {
        let base = 116_444_736_001_000_000;
        let boot = completed(1, base, base + 100);
        let delays: Vec<_> = (0..=MAX_BOOT_DELAYS)
            .map(|i| BootDelayEvidence {
                boot_start_filetime: base,
                event_id: 101,
                full_path: format!(r"C:\Example\app{i}.exe"),
                total_time_ms: 250,
                degradation_time_ms: 50,
                recorded_unix_ms: 1_000,
            })
            .collect();
        assert!(
            bind_boot_evidence(vec![boot], &[], &delays, true, true)[0]
                .delays
                .is_empty()
        );
    }
    #[test]
    fn a_completion_flag_without_interval_fields_cannot_panic_or_qualify() {
        let malformed = BootEvidence {
            completed_measurement: true,
            ..Default::default()
        };
        let result = bind_boot_evidence(vec![malformed], &[], &[], true, true);
        assert!(result[0].raw_class.is_none() && result[0].delays.is_empty());
    }
}
