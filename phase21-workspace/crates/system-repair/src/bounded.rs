//! P78-02: an assessment check that can hang runs on its own thread under its own deadline.
//!
//! A deadline alone does not stop a stuck Windows API, so a check that passes it is told to stop
//! and reported as unknown, and its slot stays taken until its thread really returns: a second
//! assessment then reports "still running" instead of starting a second DISM or SFC beside the
//! first. A late result goes to a channel nobody reads, so it cannot write over a newer assessment.
//!
//! `collector-runtime` has the same idea (`run_isolated_gated_with_token`). It is not reused here
//! because adding it as a dependency changes this crate's hashed manifest and `Cargo.lock`, which
//! means a dependency-freeze run for what is a few dozen lines.

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

use crate::{RepairCheck, RepairError, Result};

/// One provider's place in line: at most one attempt runs at a time, however many assessments ask.
pub struct ProviderSlot(AtomicBool);

pub enum Bounded<T> {
    Done(T),
    /// The deadline passed. The attempt was told to stop; it may still be running.
    TimedOut,
    /// An earlier attempt has not returned. Nothing was started.
    Busy,
    /// The owner's cancel was raised while waiting.
    Cancelled,
    /// The attempt could not start, or stopped without an answer.
    Failed(String),
}

impl Default for ProviderSlot {
    fn default() -> Self {
        Self::new()
    }
}

impl ProviderSlot {
    pub const fn new() -> Self {
        Self(AtomicBool::new(false))
    }

    /// Runs `work` on its own thread. `work` receives a flag that is raised on the deadline and on
    /// the owner's `cancel`; the wait polls both every 50 ms.
    pub fn run<T: Send + 'static>(
        &'static self,
        name: &str,
        deadline: Duration,
        cancel: &AtomicBool,
        work: impl FnOnce(Arc<AtomicBool>) -> T + Send + 'static,
    ) -> Bounded<T> {
        if self
            .0
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Bounded::Busy;
        }
        let stop = Arc::new(AtomicBool::new(false));
        let (send, receive) = mpsc::channel();
        let for_work = stop.clone();
        let slot: &'static AtomicBool = &self.0;
        let spawned = thread::Builder::new()
            .name(format!("aether-assess-{name}"))
            .spawn(move || {
                // Released when `work` returns or panics, and before the answer is sent.
                struct Release(&'static AtomicBool);
                impl Drop for Release {
                    fn drop(&mut self) {
                        self.0.store(false, Ordering::SeqCst);
                    }
                }
                let release = Release(slot);
                let answer = work(for_work);
                drop(release);
                let _ = send.send(answer);
            });
        if let Err(error) = spawned {
            self.0.store(false, Ordering::SeqCst);
            return Bounded::Failed(error.to_string());
        }
        let end = Instant::now() + deadline;
        loop {
            if cancel.load(Ordering::SeqCst) {
                stop.store(true, Ordering::SeqCst);
                return Bounded::Cancelled;
            }
            let left = end.saturating_duration_since(Instant::now());
            if left.is_zero() {
                stop.store(true, Ordering::SeqCst);
                return Bounded::TimedOut;
            }
            match receive.recv_timeout(left.min(Duration::from_millis(50))) {
                Ok(answer) => return Bounded::Done(answer),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Bounded::Failed("the check stopped without an answer".into());
                }
            }
        }
    }
}

/// Runs one assessment check under `deadline`. Every way of not finishing is a check whose result
/// is unknown, with its reason; only the owner's cancel is an error.
pub fn run_check(
    slot: &'static ProviderSlot,
    (id, title, log_hint): (&str, &str, &str),
    deadline: Duration,
    cancel: &AtomicBool,
    work: impl FnOnce(Arc<AtomicBool>) -> RepairCheck + Send + 'static,
) -> Result<RepairCheck> {
    let unknown = |result_code: &str, detail: String| RepairCheck {
        id: id.into(),
        title: title.into(),
        stage: "Unknown".into(),
        result_code: result_code.into(),
        exit_code: -1,
        detail,
        log_hint: log_hint.into(),
    };
    match slot.run(id, deadline, cancel, work) {
        Bounded::Done(check) => Ok(check),
        Bounded::Cancelled => Err(RepairError::Cancelled),
        Bounded::TimedOut => Ok(unknown(
            "CheckTimedOut",
            "This check did not finish in time and was stopped; its result is unknown.".into(),
        )),
        Bounded::Busy => Ok(unknown(
            "CheckStillRunning",
            "An earlier attempt at this check is still running, so it was not started again; its result is unknown.".into(),
        )),
        // A panic or a thread that could not start: the reason is an OS or panic string, not a sentence
        // for the reader, so the check says only that it produced no answer.
        Bounded::Failed(_) => Ok(unknown(
            "ProbeUnavailable",
            "This check stopped without an answer; its result is unknown.".into(),
        )),
    }
}
