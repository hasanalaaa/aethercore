//! A driver search that ends when its deadline says so (P84-02B).
//!
//! WUA's `Search` blocks for as long as the agent takes, which online can be minutes, and a
//! search that never returns held the drivers page in "Searching" forever. The search now runs
//! on its own thread; the caller waits until the deadline, then raises the stop flag (the
//! Windows search asks the agent to abort, once) and returns. The agent may take a while to
//! honour that, so until the old search has really stopped a new one is refused rather than
//! started beside it: there is never more than one AetherCore search inside the agent.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::Duration;

use crate::{DiscoveryResult, Result, UpdateError};

/// Set while a search thread is alive, including one that was asked to stop and has not yet.
static IN_FLIGHT: AtomicBool = AtomicBool::new(false);

/// Clears `IN_FLIGHT` when the search thread ends, however it ends.
struct InFlight;
impl Drop for InFlight {
    fn drop(&mut self) {
        IN_FLIGHT.store(false, Ordering::SeqCst);
    }
}

/// Runs `search` on its own thread for at most `deadline`. `search` is handed the stop flag and
/// must check it while it waits on the agent.
pub(crate) fn search_bounded<F>(deadline: Duration, search: F) -> Result<DiscoveryResult>
where
    F: FnOnce(&AtomicBool) -> Result<DiscoveryResult> + Send + 'static,
{
    if IN_FLIGHT.swap(true, Ordering::SeqCst) {
        return Err(UpdateError::SearchStillStopping);
    }
    let stop = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&stop);
    let (sender, receiver) = mpsc::channel();
    let spawned = thread::Builder::new()
        .name("aether-wua-driver-search".into())
        .spawn(move || {
            let _in_flight = InFlight;
            // A caller that already gave up has dropped the receiver; nothing reads this then.
            let _ = sender.send(search(&flag));
        });
    if let Err(error) = spawned {
        IN_FLIGHT.store(false, Ordering::SeqCst);
        return Err(UpdateError::Wua(format!(
            "the driver search thread could not start: {error}"
        )));
    }
    match receiver.recv_timeout(deadline) {
        Ok(result) => result,
        Err(RecvTimeoutError::Timeout) => {
            stop.store(true, Ordering::SeqCst);
            Err(UpdateError::SearchTimedOut)
        }
        Err(RecvTimeoutError::Disconnected) => Err(UpdateError::Wua(
            "the driver search thread ended without an answer".into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use std::time::Instant;

    /// The runner is process-wide, as the agent is; the tests take turns.
    static SERIAL: Mutex<()> = Mutex::new(());

    fn wait_until_idle() {
        let started = Instant::now();
        while IN_FLIGHT.load(Ordering::SeqCst) {
            assert!(
                started.elapsed() < Duration::from_secs(5),
                "the search never stopped"
            );
            thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn a_search_that_answers_in_time_is_its_answer() {
        let _turn = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
        let result = search_bounded(Duration::from_secs(5), |_| Ok(DiscoveryResult::default()));
        assert!(result.is_ok());
        wait_until_idle();
    }

    /// A search the agent never finishes: the caller gets its answer at the deadline, the agent
    /// is asked to stop once, and no second search starts until the first has stopped.
    #[test]
    fn a_search_that_never_answers_is_stopped_and_blocks_the_next_until_it_has() {
        let _turn = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
        let aborts = Arc::new(Mutex::new(0u32));
        let seen = Arc::clone(&aborts);
        let started = Instant::now();
        let result = search_bounded(Duration::from_millis(50), move |stop| {
            while !stop.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_millis(2));
            }
            *seen.lock().unwrap() += 1;
            // The agent takes its time to honour the abort.
            thread::sleep(Duration::from_millis(1_000));
            Err(UpdateError::Wua("aborted".into()))
        });
        assert!(
            matches!(result, Err(UpdateError::SearchTimedOut)),
            "{result:?}"
        );
        assert!(
            started.elapsed() < Duration::from_millis(800),
            "{:?}",
            started.elapsed()
        );
        assert!(matches!(
            search_bounded(Duration::from_secs(1), |_| Ok(DiscoveryResult::default())),
            Err(UpdateError::SearchStillStopping)
        ));
        wait_until_idle();
        assert_eq!(*aborts.lock().unwrap(), 1);
        assert!(search_bounded(Duration::from_secs(1), |_| Ok(DiscoveryResult::default())).is_ok());
        wait_until_idle();
    }

    #[test]
    fn a_search_that_panics_frees_the_runner() {
        let _turn = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
        let result = search_bounded(Duration::from_secs(1), |_| panic!("the agent crashed"));
        assert!(matches!(result, Err(UpdateError::Wua(_))), "{result:?}");
        wait_until_idle();
        assert!(search_bounded(Duration::from_secs(1), |_| Ok(DiscoveryResult::default())).is_ok());
        wait_until_idle();
    }
}
