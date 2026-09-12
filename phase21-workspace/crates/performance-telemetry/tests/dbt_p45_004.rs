//! `DBT-P45-004` — the residual `total == 0` tick tie, measured rather than remembered.
//!
//! §45.2 pushed the tie to the field boundary: `busy_bp_from_ticks` returns
//! `Option<u32>`, `None` when the tick counters did not advance, and `sample_cpu`
//! spends one bounded extended wait before degrading honestly. §45.3 measured what
//! was left at **1/50 (2%)** — and then threw the harness away. The rate has been
//! folk memory ever since: quoted in three later phase records, re-measurable by
//! nobody.
//!
//! That is the same shape as every other defect in this codebase's list. A number
//! nothing can reproduce is a belief, not a measurement, so the harness is
//! committed here.
//!
//! It is `#[ignore]`d because it pins every core for a minute or more; it is not
//! part of the normal suite. Run it deliberately:
//!
//!     cargo test -p aethercore-performance-telemetry --test dbt_p45_004 \
//!         -- --ignored --nocapture --test-threads=1
//!
//! `AETHERCORE_TIE_TRIALS` overrides the trial count (default 50, matching §45.3
//! so the two numbers are comparable).
//!
//! WHAT A FAILURE HERE MEANS. The tie itself is not a defect — a sampling window
//! in which the kernel's tick counters did not advance is a real thing that can
//! happen, and reporting it honestly is the fix that already landed. What this
//! guards is that the honest path is the one taken: every tie must arrive as a
//! `cpu` `Unavailable` fault naming the tick delta, and never as a confident `0`.
//! A silent zero is the defect, at any rate above zero.

use std::time::Duration;

#[cfg(target_os = "macos")]
mod macos {
    use super::*;
    use aethercore_performance_telemetry::{MacosPerfPlatform, PerfPlatform};

    fn under_load<T>(body: impl FnOnce() -> T) -> T {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicBool, Ordering};
        let stop = Arc::new(AtomicBool::new(false));
        let threads: Vec<_> = (0..std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4))
            .map(|_| {
                let stop = Arc::clone(&stop);
                std::thread::spawn(move || {
                    let mut acc = 0u64;
                    while !stop.load(Ordering::Relaxed) {
                        for i in 0..50_000u64 {
                            acc = acc.wrapping_add(i.wrapping_mul(2_654_435_761));
                        }
                    }
                    acc
                })
            })
            .collect();
        std::thread::sleep(Duration::from_millis(300));
        let out = body();
        stop.store(true, Ordering::Relaxed);
        for thread in threads {
            let _ = thread.join();
        }
        out
    }

    #[test]
    #[ignore = "pins every core for a minute or more; run deliberately"]
    fn residual_tick_tie_rate_is_measured_and_never_a_silent_zero() {
        let trials: usize = std::env::var("AETHERCORE_TIE_TRIALS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(50);

        let mut ties = 0usize;
        let mut silent_zeros: Vec<String> = Vec::new();
        let mut busy_readings: Vec<u32> = Vec::new();

        under_load(|| {
            let platform = MacosPerfPlatform::new();
            for trial in 0..trials {
                let snapshot = platform.sample(Duration::from_millis(1_000));
                let tie_fault = snapshot
                    .collector_faults
                    .iter()
                    .find(|fault| fault.collector == "cpu" && fault.detail.contains("tick delta"));
                match (snapshot.cpu.total_busy_bp, tie_fault) {
                    // A tie, honestly reported. This is the behaviour §45.2 landed.
                    (_, Some(fault)) => {
                        ties += 1;
                        eprintln!("  trial {trial:>3}: TIE  {}", fault.detail);
                    }
                    // A real reading.
                    (bp, None) if bp > 0 => busy_readings.push(bp),
                    // A zero with no fault: the defect DBT-P45-001 removed. If this
                    // ever recurs the contract has regressed, whatever the rate.
                    (0, None) => silent_zeros.push(format!(
                        "trial {trial}: totalBusyBp=0 with NO cpu tick-delta fault; faults={:?}",
                        snapshot.collector_faults
                    )),
                    _ => unreachable!(),
                }
            }
        });

        let rate = ties as f64 / trials as f64;
        let min = busy_readings.iter().copied().min().unwrap_or(0);
        let max = busy_readings.iter().copied().max().unwrap_or(0);
        eprintln!(
            "\nDBT-P45-004 residual tick-tie rate on this host\n  \
             trials            {trials}\n  \
             ties (honest)     {ties}  ({:.1}%)\n  \
             real readings     {}  totalBusyBp {min}..{max} bp\n  \
             SILENT ZEROS      {}   <- must be 0\n",
            rate * 100.0,
            busy_readings.len(),
            silent_zeros.len()
        );

        assert!(
            silent_zeros.is_empty(),
            "a tick tie was published as a confident 0% with no cpu fault — this is \
             DBT-P45-001 regressed, and the rate is irrelevant:\n{}",
            silent_zeros.join("\n")
        );
        assert!(
            !busy_readings.is_empty(),
            "every logical processor was spinning for the whole run and not one trial \
             produced a real reading; {ties}/{trials} ties is not a residual, it is a \
             broken tick source"
        );
    }
}

#[cfg(not(target_os = "macos"))]
#[test]
#[ignore = "DBT-P45-004 is a macOS host_statistics64 tick-source property"]
fn residual_tick_tie_rate_is_macos_only() {}
