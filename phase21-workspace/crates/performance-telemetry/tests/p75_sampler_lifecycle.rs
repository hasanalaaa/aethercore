//! P75 lane `telemetry-windows`: the background sampler's lifecycle.
//!
//! Platform-independent, so these run on every host.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use aethercore_performance_telemetry::{PerfPlatform, PerfSnapshot, PerformanceRing};

const OWNER: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

/// Counts how often it is sampled and returns immediately.
#[derive(Default)]
struct Counting {
    calls: AtomicU64,
}

impl PerfPlatform for Counting {
    fn sample(&self, interval: Duration) -> PerfSnapshot {
        self.calls.fetch_add(1, Ordering::SeqCst);
        PerfSnapshot {
            interval_ms: interval.as_millis() as u32,
            ..PerfSnapshot::default()
        }
    }
}

/// `stop()` followed by `start()` inside one interval used to leave the first
/// sampler running: it slept through the stop, woke to `active == true` (set by
/// the second start) and kept sampling beside the new one, forever. The first
/// platform here must stop being sampled once the second start returns.
#[test]
fn stop_then_start_within_an_interval_leaves_exactly_one_sampler() {
    let ring = PerformanceRing::new();
    let first = Arc::new(Counting::default());
    let second = Arc::new(Counting::default());

    ring.start(first.clone(), OWNER, 250).expect("first start");
    std::thread::sleep(Duration::from_millis(50));
    ring.stop();
    ring.start(second.clone(), OWNER, 250)
        .expect("second start");
    // At most one call of the first sampler may already be in flight.
    let first_calls_at_restart = first.calls.load(Ordering::SeqCst);

    std::thread::sleep(Duration::from_millis(1_100));
    ring.stop();

    let first_calls_after = first.calls.load(Ordering::SeqCst);
    assert!(
        first_calls_after <= first_calls_at_restart + 1,
        "the stopped sampler kept running after a restart: {first_calls_at_restart} calls at \
         restart, {first_calls_after} after 1.1 s"
    );
    assert!(
        second.calls.load(Ordering::SeqCst) >= 3,
        "the restarted sampler must be the one sampling"
    );
}

/// Two concurrent starts must not both win (the check and the store were two
/// separate operations).
#[test]
fn concurrent_starts_admit_one_sampler() {
    let ring = Arc::new(PerformanceRing::new());
    let platform = Arc::new(Counting::default());
    let barrier = Arc::new(std::sync::Barrier::new(8));
    let winners: usize = (0..8)
        .map(|_| {
            let (ring, platform, barrier) = (ring.clone(), platform.clone(), barrier.clone());
            std::thread::spawn(move || {
                barrier.wait();
                ring.start(platform, OWNER, 250).is_ok()
            })
        })
        .collect::<Vec<_>>()
        .into_iter()
        .map(|handle| usize::from(handle.join().expect("join")))
        .sum();
    ring.stop();
    assert_eq!(winners, 1, "exactly one concurrent start may succeed");
}
