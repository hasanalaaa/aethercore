//! Same recorded Windows class comparisons; OS restart context never supplies duration.
use aethercore_crash_diagnostics::boot::BootEvidence;
use aethercore_hardware_telemetry::measurements::{
    Availability, BootComparison, BootDelay, BootRecord, Coverage, OsRestartReference,
};
const FILETIME_EPOCH: u64 = 116_444_736_000_000_000;
fn unix_ms(ticks: u64) -> Option<i64> {
    i64::try_from(ticks.checked_sub(FILETIME_EPOCH)? / 10_000).ok()
}
/// Fixed CIM_DATETIME point with explicit timezone; wildcards/intervals are not an instant.
fn cim_filetime(value: &str) -> Option<u64> {
    let b = value.as_bytes();
    if b.len() != 25
        || b[14] != b'.'
        || !matches!(b[21], b'+' | b'-')
        || b.iter()
            .enumerate()
            .any(|(i, c)| !matches!(i, 14 | 21) && !c.is_ascii_digit())
    {
        return None;
    }
    let n = |a: usize, z: usize| value[a..z].parse::<u32>().ok();
    let date = chrono::NaiveDate::from_ymd_opt(i32::try_from(n(0, 4)?).ok()?, n(4, 6)?, n(6, 8)?)?;
    let local = date.and_hms_micro_opt(n(8, 10)?, n(10, 12)?, n(12, 14)?, n(15, 21)?)?;
    let offset = i64::from(n(22, 25)?) * (if b[21] == b'+' { 1 } else { -1 });
    let utc = local
        .checked_sub_signed(chrono::Duration::minutes(offset))?
        .and_utc();
    u64::try_from(utc.timestamp_micros())
        .ok()?
        .checked_mul(10)?
        .checked_add(FILETIME_EPOCH)
}
fn valid(b: &BootEvidence, observed: i64) -> bool {
    b.completed_measurement
        && b.source_version == Some(2)
        && b.system_boot_instance
            .is_some_and(|i| i > 0 && u32::try_from(i).is_ok())
        && b.boot_time_ms
            .is_some_and(|v| v > 0 && u32::try_from(v).is_ok())
        && b.raw_class.as_ref().is_some_and(|c| c.event_version == 1)
        && b.recorded_unix_ms > 0
        && b.recorded_unix_ms <= observed
        && observed.saturating_sub(b.recorded_unix_ms) <= i64::from(aethercore_crash_diagnostics::DEFAULT_EVENT_WINDOW_DAYS) * 86_400_000
        && matches!((b.boot_start_filetime,b.boot_end_filetime,b.recorded_filetime),
            (Some(start),Some(end),Some(record)) if start>FILETIME_EPOCH && start<end && end<=record
                && unix_ms(record)==Some(b.recorded_unix_ms))
}
pub(crate) fn compose_boots(
    raw: &[BootEvidence],
    reference: Option<&OsRestartReference>,
    scan_started: i64,
    observed: i64,
) -> Vec<BootRecord> {
    let mut rows: Vec<_> = raw.iter().collect();
    rows.sort_by_key(|b| std::cmp::Reverse((b.recorded_unix_ms, b.recorded_filetime)));
    rows.truncate(20);
    let qualified:Vec<_>=rows.iter().map(|b|valid(b,observed) && rows.iter().filter(|other|
        other.system_boot_instance==b.system_boot_instance).count()==1 && !rows.iter().any(|other|
        !std::ptr::eq(*other,*b) && matches!((b.boot_start_filetime,b.boot_end_filetime,other.boot_start_filetime,other.boot_end_filetime),
            (Some(start),Some(end),Some(a),Some(z)) if start<=z&&a<=end))).collect();
    // Full event paths are private and can be 4096 bytes each. Keep a shared
    // publication allowance, independently of the bounded raw event reader.
    let mut remaining_delays = 8usize;
    let mut result: Vec<_> = rows
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let duration = b
                .boot_time_ms
                .filter(|v| *v > 0 && u32::try_from(*v).is_ok());
            let kept_delays = if qualified[i] { b.delays.len().min(remaining_delays) } else { 0 };
            remaining_delays -= kept_delays;
            BootRecord {
                recorded_unix_ms: b.recorded_unix_ms,
                duration_ms: duration,
                coverage: Coverage {
                    source: "Microsoft-Windows-Diagnostics-Performance event100".into(),
                    observed_unix_ms: Some(b.recorded_unix_ms),
                    window_days: Some(aethercore_crash_diagnostics::DEFAULT_EVENT_WINDOW_DAYS),
                    availability: if duration.is_some() {
                        Availability::Measured
                    } else {
                        Availability::Unsupported
                    },
                    reason_key: if duration.is_some() {
                        String::new()
                    } else {
                        "measurement.reason.noBootTime".into()
                    },
                },
                system_boot_instance: b
                    .system_boot_instance
                    .and_then(|v| u32::try_from(v).ok())
                    .filter(|v| *v > 0),
                completed_measurement: qualified[i],
                raw_class_version: if qualified[i] {
                    b.raw_class.as_ref().map(|c| c.event_version)
                } else {
                    None
                },
                raw_class_value: if qualified[i] {
                    b.raw_class.as_ref().map(|c| c.value)
                } else {
                    None
                },
                boot_start_unix_ms: b.boot_start_filetime.and_then(unix_ms),
                boot_end_unix_ms: b.boot_end_filetime.and_then(unix_ms),
                delays: if qualified[i] {
                    b.delays
                        .iter()
                        .take(kept_delays)
                        .map(|d| BootDelay {
                            event_id: d.event_id,
                            full_path: d.full_path.clone(),
                            total_time_ms: d.total_time_ms,
                            degradation_time_ms: d.degradation_time_ms,
                            recorded_unix_ms: d.recorded_unix_ms,
                        })
                        .collect()
                } else {
                    vec![]
                },
                delays_truncated: qualified[i] && b.delays.len() > kept_delays,
                ..Default::default()
            }
        })
        .collect();
    // Event publication order must agree with completed interval order. A delayed
    // older record is history, never a new latest-boot/baseline assertion.
    if rows.windows(2).any(|pair| matches!((pair[0].boot_start_filetime,pair[1].boot_end_filetime),
        (Some(start),Some(end)) if start <= end)) { return result; }
    let Some(latest) = rows.first().filter(|_| qualified.first() == Some(&true)) else {
        return result;
    };
    if let Some(reference) =
        reference.filter(|r| r.observed_unix_ms >= scan_started && r.observed_unix_ms <= observed)
        && let Some(restart) = cim_filetime(&reference.cim_datetime)
        && latest.latest_kernel_boot
        && unix_ms(restart).is_some_and(|t| t <= reference.observed_unix_ms)
        && matches!((latest.boot_start_filetime,latest.boot_end_filetime),(Some(start),Some(end)) if start<=restart && restart<=end)
    {
        result[0].matches_os_restart = true;
        result[0].os_restart_observed_unix_ms = Some(reference.observed_unix_ms);
    }
    let comparable: Vec<_> = rows
        .iter()
        .enumerate()
        .filter(|(i, b)| qualified[*i] && b.raw_class == latest.raw_class)
        .map(|(_, b)| *b)
        .collect();
    if comparable.len() >= 3 {
        let mut values: Vec<_> = comparable[..3]
            .iter()
            .filter_map(|b| b.boot_time_ms)
            .collect();
        values.sort_unstable();
        let mut comparison = BootComparison {
            median_ms: values[1],
            sample_count: 3,
            window_start_unix_ms: comparable[2].recorded_unix_ms,
            window_end_unix_ms: comparable[0].recorded_unix_ms,
            ..Default::default()
        };
        if comparable.len() >= 6 {
            let mut prior: Vec<_> = comparable[1..6]
                .iter()
                .filter_map(|b| b.boot_time_ms)
                .collect();
            prior.sort_unstable();
            comparison.baseline_ms = Some(prior[2]);
            comparison.baseline_count = 5;
            comparison.baseline_window_start_unix_ms = comparable[5].recorded_unix_ms;
            comparison.baseline_window_end_unix_ms = comparable[1].recorded_unix_ms;
        }
        result[0].comparison = Some(comparison);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use aethercore_crash_diagnostics::boot::{BootEvidence, RawBootClass};
    use aethercore_hardware_telemetry::measurements::OsRestartReference;
    fn row(instance: u64, day: u32, duration: u64) -> BootEvidence {
        let start = chrono::NaiveDate::from_ymd_opt(2026, 9, day)
            .unwrap()
            .and_hms_opt(8, 14, 13)
            .unwrap()
            .and_utc()
            .timestamp_millis();
        let ticks = 116_444_736_000_000_000 + start as u64 * 10_000;
        BootEvidence {
            system_boot_instance: Some(instance),
            recorded_unix_ms: start + 120_000,
            boot_time_ms: Some(duration),
            source_version: Some(2),
            boot_start_filetime: Some(ticks),
            boot_end_filetime: Some(ticks + 110_000 * 10_000),
            recorded_filetime: Some(ticks + 120_000 * 10_000),
            completed_measurement: true,
            raw_class: Some(RawBootClass {
                event_version: 1,
                value: 0,
            }),
            latest_kernel_boot: day == 29,
            ..Default::default()
        }
    }
    fn observed() -> i64 {
        chrono::NaiveDate::from_ymd_opt(2026, 10, 4)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp_millis()
    }
    fn reference() -> OsRestartReference {
        OsRestartReference {
            cim_datetime: "20260929111416.408468+180".into(),
            observed_unix_ms: observed(),
        }
    }
    #[test]
    fn exact_cim_datetime_offset_and_microseconds_are_not_a_three_second_tolerance() {
        let ticks = cim_filetime(&reference().cim_datetime).unwrap();
        let utc = cim_filetime("20260929081416.408468+000").unwrap();
        assert_eq!(ticks, utc);
        assert_eq!(ticks % 10_000, 4680);
        for value in [
            "20260929111416.408468+***",
            "20260929111416.408468:180",
            "20260231081416.000000+000",
            "20260929111416.******+180",
        ] {
            assert!(cim_filetime(value).is_none());
        }
    }
    #[test]
    fn actual_native_offset_inside_a_unique_completed_interval_binds_os_restart_context() {
        let rows = vec![row(9, 29, 25414), row(8, 28, 29230), row(7, 27, 28545)];
        let result = compose_boots(&rows, Some(&reference()), observed() - 20_000, observed());
        assert!(result[0].matches_os_restart);
        let comparison = result[0].comparison.as_ref().unwrap();
        assert_eq!((comparison.median_ms, comparison.sample_count), (28545, 3));
        assert!(
            comparison.baseline_ms.is_none(),
            "three descriptive samples are not five prior samples"
        );
    }
    #[test]
    fn five_prior_same_tokens_exclude_current_from_the_baseline() {
        let rows = vec![
            row(9, 29, 90000),
            row(8, 28, 50000),
            row(7, 27, 40000),
            row(6, 26, 30000),
            row(5, 25, 20000),
            row(4, 24, 10000),
        ];
        let result = compose_boots(&rows, Some(&reference()), observed() - 20_000, observed());
        let comparison = result[0].comparison.as_ref().unwrap();
        assert_eq!(comparison.baseline_ms, Some(30000));
        assert_eq!(comparison.baseline_count, 5);
        assert!(comparison.baseline_window_end_unix_ms < result[0].recorded_unix_ms);
    }
    #[test]
    fn missing_mixed_future_duplicate_and_old_references_never_become_current_baselines() {
        let rows = vec![
            row(9, 29, 90000),
            row(8, 28, 50000),
            row(7, 27, 40000),
            row(6, 26, 30000),
            row(5, 25, 20000),
            row(4, 24, 10000),
        ];
        assert!(!compose_boots(&rows, None, observed() - 20_000, observed())[0].matches_os_restart);
        let mut wrong = reference();
        wrong.cim_datetime = "20260929121416.408468+180".into();
        assert!(
            !compose_boots(&rows, Some(&wrong), observed() - 20_000, observed())[0]
                .matches_os_restart
        );
        wrong = reference();
        wrong.observed_unix_ms = observed() + 1;
        assert!(
            !compose_boots(&rows, Some(&wrong), observed() - 20_000, observed())[0]
                .matches_os_restart
        );
        let mut unsupported = rows.clone();
        unsupported[1].raw_class.as_mut().unwrap().value = 1;
        assert!(
            compose_boots(
                &unsupported,
                Some(&reference()),
                observed() - 20_000,
                observed()
            )[0]
            .comparison
            .as_ref()
            .unwrap()
            .baseline_ms
            .is_none()
        );
        let mut ambiguous = rows.clone();
        ambiguous.push(rows[0].clone());
        assert!(
            compose_boots(
                &ambiguous,
                Some(&reference()),
                observed() - 20_000,
                observed()
            )
            .iter()
            .all(|row| !row.matches_os_restart)
        );
        let mut newer_kernel = rows.clone();
        newer_kernel[0].latest_kernel_boot = false;
        assert!(
            !compose_boots(
                &newer_kernel,
                Some(&reference()),
                observed() - 20_000,
                observed()
            )[0]
            .matches_os_restart
        );
    }
    #[test]
    fn delayed_reversed_records_and_outside_source_window_cannot_supply_a_baseline() {
        let mut rows = vec![row(9,29,90000),row(8,28,50000),row(7,27,40000),row(6,26,30000),row(5,25,20000),row(4,24,10000)];
        // An older completed boot written after the newest one is not a latest-boot proof.
        rows[1].recorded_filetime = Some(rows[0].recorded_filetime.unwrap()+10_000);
        rows[1].recorded_unix_ms = rows[0].recorded_unix_ms+1;
        assert!(compose_boots(&rows,Some(&reference()),observed()-20_000,observed()).iter().all(|b|b.comparison.is_none()&&!b.matches_os_restart));
        let old_observer = observed()+31*86_400_000;
        assert!(compose_boots(&[row(9,29,90000)],None,old_observer-1,old_observer).iter().all(|b|!b.completed_measurement));
    }
    #[test]
    fn maximal_boot_paths_do_not_break_the_existing_snapshot_budget_and_cuts_are_explicit() {
        use aethercore_crash_diagnostics::boot::BootDelayEvidence;
        use aethercore_hardware_telemetry::measurements::{NetworkAdapter, ThermalZone, Battery};
        let mut rows = (10..30).enumerate().map(|(i,day)| row(i as u64+1,day,30_000)).collect::<Vec<_>>();
        for b in &mut rows {
            b.delays = (0..32).map(|_|BootDelayEvidence{event_id:101,full_path:format!("C:\\{}", "\\".repeat(4000)),total_time_ms:1700,degradation_time_ms:300,recorded_unix_ms:b.recorded_unix_ms,boot_start_filetime:b.boot_start_filetime.unwrap()}).collect();
        }
        let boots=compose_boots(&rows,None,observed()-1000,observed());
        let name="N".repeat(256);let coverage=Coverage{source:name.clone(),reason_key:name.clone(),..Default::default()};
        let snapshot=crate::DiagnosticsSnapshot{boots,
            thermal_zones:(0..32).map(|_|ThermalZone{stable_id:name.clone(),display_name:name.clone(),coverage:coverage.clone(),..Default::default()}).collect(),
            batteries:(0..16).map(|_|Battery{stable_id:name.clone(),display_name:name.clone(),coverage:coverage.clone(),..Default::default()}).collect(),
            network_adapters:(0..32).map(|_|NetworkAdapter{stable_id:name.clone(),display_name:name.clone(),coverage:coverage.clone(),..Default::default()}).collect(),
            ..Default::default()};
        let bytes=serde_json::to_vec(&snapshot).unwrap().len();
        assert!(bytes<256*1024,"actual boot composition exceeded existing snapshot budget: {bytes}");
        assert!(snapshot.boots.iter().any(|b|b.delays_truncated));
    }
    #[test]
    fn absent_class_and_uncompleted_legacy_samples_keep_history_without_comparison() {
        let mut row = row(9, 29, 25414);
        row.raw_class = None;
        let result = compose_boots(&[row], Some(&reference()), observed() - 20_000, observed());
        assert_eq!(result[0].duration_ms, Some(25414));
        assert!(!result[0].matches_os_restart);
        assert!(result[0].comparison.is_none());
        let result = compose_boots(
            &[BootEvidence {
                boot_time_ms: Some(0),
                ..Default::default()
            }],
            None,
            observed() - 20_000,
            observed(),
        );
        assert!(result[0].duration_ms.is_none());
    }
}
