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

/// Never returns: a stand-in for a PDH wildcard expansion that hangs.
struct Hung;

impl PerfPlatform for Hung {
    fn sample(&self, _interval: Duration) -> PerfSnapshot {
        loop {
            std::thread::park();
        }
    }
}

/// `lib.rs` promised that every collector runs under the `collector-runtime`
/// timeout; none did, so one hung platform call stalled the sampler forever and
/// the ring stayed empty with nothing said. The tick must now publish a fault
/// for every subsystem once the deadline passes, and keep ticking.
#[test]
fn a_hung_collector_becomes_a_timeout_fault_not_a_stalled_sampler() {
    let ring = PerformanceRing::new();
    ring.start(Arc::new(Hung), OWNER, 250).expect("start");
    let deadline = std::time::Instant::now() + Duration::from_secs(15);
    // The first tick is the one that timed out; under load a poll can wake after
    // the next tick (correctly `Unavailable`) is already the latest, so read the
    // ring's oldest sample, not its newest.
    let snapshot = loop {
        if let Some(snapshot) = ring.window(OWNER).into_iter().next() {
            break snapshot;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "a hung collector stalled the sampler: no snapshot after 15 s"
        );
        std::thread::sleep(Duration::from_millis(100));
    };
    for collector in ["cpu", "power", "memory", "storage", "gpu", "processTop"] {
        let fault = snapshot
            .collector_faults
            .iter()
            .find(|fault| fault.collector == collector)
            .unwrap_or_else(|| panic!("no {collector} fault in {:?}", snapshot.collector_faults));
        assert_eq!(fault.kind, "Timeout", "{fault:?}");
    }
    let measured = snapshot.measured_subsystems();
    assert!(!measured.cpu && !measured.memory && !measured.storage && !measured.gpu);
    assert_eq!(
        snapshot.interval_ms, 0,
        "nothing was measured, so no window"
    );

    // The worker is still stuck; later ticks must say so at once, not queue
    // another stuck thread behind it.
    let first = snapshot.captured_unix_ms;
    std::thread::sleep(Duration::from_millis(700));
    let later = ring.latest(OWNER).expect("later tick");
    ring.stop();
    assert!(
        later.captured_unix_ms > first,
        "the sampler stopped ticking"
    );
    assert!(
        later
            .collector_faults
            .iter()
            .any(|fault| fault.collector == "cpu" && fault.kind == "Unavailable"),
        "{:?}",
        later.collector_faults
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
