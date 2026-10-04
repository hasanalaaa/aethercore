//! Post-owned-call admission remains charged until the SCM explicitly reports STOPPED.
//! RUNNING is uncertainty, not proof that servicing is active or complete.

#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn wait_until_stopped(
    mut stopped: impl FnMut() -> bool,
    mut pending: impl FnMut(),
    mut wait: impl FnMut(),
) {
    if stopped() {
        return;
    }
    pending();
    loop {
        wait();
        if stopped() {
            return;
        }
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn finish_dism_lifecycle(
    initialized: bool,
    session: u32,
    mut close: impl FnMut(u32) -> i32,
    mut shutdown: impl FnMut() -> i32,
    mut pending: impl FnMut(),
    wait: impl FnMut(),
) {
    if !initialized && session == 0 {
        return;
    }
    // Cleanup itself can block. Publish indeterminate drain before crossing that boundary.
    pending();
    let confirmed = (session == 0 || close(session) == 0) && (!initialized || shutdown() == 0);
    // Only exact S_OK acknowledges each owned cleanup. Do not retry one-shot shutdown,
    // consult SCM as a substitute, or infer success from another HRESULT.
    wait_until_stopped(|| confirmed, || {}, wait);
}

#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn explicitly_stopped<T: PartialEq>(observed: Option<T>, stopped: T) -> bool {
    observed.is_some_and(|state| state == stopped)
}

#[cfg(windows)]
static OWNED_SERVICING: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Owns the actual native-call serialization through lifecycle/reader and SCM drain.
/// An idle-running or unreadable service can retain this indefinitely; cancel is not idle proof.
#[cfg(windows)]
pub(crate) struct ServicingDrain<'a> {
    _owned: std::sync::MutexGuard<'static, ()>,
    pending: Option<&'a mut (dyn FnMut() + Send)>,
}

#[cfg(windows)]
impl<'a> ServicingDrain<'a> {
    pub(crate) fn before_owned_call(
        pending: Option<&'a mut (dyn FnMut() + Send)>,
    ) -> crate::Result<Self> {
        let owned = OWNED_SERVICING
            .try_lock()
            .map_err(|_| crate::RepairError::ServicingUnverified)?;
        // Also applied after service restart: no earlier in-memory lock or restart implies idle.
        if !super::windows_impl::trusted_installer_stopped() {
            return Err(crate::RepairError::ServicingUnverified);
        }
        Ok(Self {
            _owned: owned,
            pending,
        })
    }

    pub(crate) fn note_pending(&mut self) {
        if let Some(pending) = self.pending.as_mut() {
            // Drain must still happen when unwinding another failure.
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| pending()));
        }
    }
}

#[cfg(windows)]
impl Drop for ServicingDrain<'_> {
    fn drop(&mut self) {
        wait_until_stopped(
            super::windows_impl::trusted_installer_stopped,
            || self.note_pending(),
            || std::thread::sleep(std::time::Duration::from_millis(250)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    #[test]
    fn failed_dism_cleanup_retains_actual_admission_without_retry_or_scm_release() {
        use aethercore_operation_kernel::{MutationSupervisor, MutationWorkload};
        for (close_hr, shutdown_hr, expected_calls) in [
            (-1, 0, vec!["close"]),
            (0, -1, vec!["close", "shutdown"]),
            (1, 0, vec!["close"]), // non-S_OK success code is not cleanup proof
        ] {
            let supervisor = MutationSupervisor::new();
            let lease = supervisor
                .try_acquire(MutationWorkload::SystemRepair, "fixture", "owner-a")
                .unwrap();
            let calls = Arc::new(std::sync::Mutex::new(Vec::new()));
            let worker_calls = calls.clone();
            let (pending_tx, pending_rx) = std::sync::mpsc::channel();
            let (waiting_tx, waiting_rx) = std::sync::mpsc::channel();
            let (end_tx, end_rx) = std::sync::mpsc::channel();
            let worker = std::thread::spawn(move || {
                let _original_lease = lease;
                // Fixture-only unwind ends a deliberately retained worker; production never
                // forces exit. A true/STOPPED SCM observation cannot bypass this cleanup fence.
                let _ = std::panic::catch_unwind(|| {
                    finish_dism_lifecycle(
                        true,
                        41,
                        |_| {
                            worker_calls.lock().unwrap().push("close");
                            close_hr
                        },
                        || {
                            worker_calls.lock().unwrap().push("shutdown");
                            shutdown_hr
                        },
                        || pending_tx.send(()).unwrap(),
                        || {
                            waiting_tx.send(()).unwrap();
                            end_rx.recv().unwrap();
                            panic!("fixture-only worker unwind");
                        },
                    )
                });
            });
            pending_rx
                .recv_timeout(std::time::Duration::from_secs(2))
                .unwrap();
            let waiting = waiting_rx.recv_timeout(std::time::Duration::from_secs(2));
            if waiting.is_err() {
                worker.join().unwrap();
                panic!("failed cleanup released the owned worker/admission");
            }
            assert!(supervisor.is_active());
            assert!(
                supervisor
                    .try_acquire(MutationWorkload::Cleanup, "next", "owner-a")
                    .is_err()
            );
            assert_eq!(*calls.lock().unwrap(), expected_calls);
            end_tx.send(()).unwrap();
            worker.join().unwrap();
        }
    }

    #[test]
    fn successful_dism_cleanup_is_once_and_ordered_before_release() {
        let calls = std::cell::RefCell::new(Vec::new());
        finish_dism_lifecycle(
            true,
            41,
            |_| {
                calls.borrow_mut().push("close");
                0
            },
            || {
                calls.borrow_mut().push("shutdown");
                0
            },
            || {
                calls.borrow_mut().push("pending");
            },
            || panic!("verified cleanup needs no wait"),
        );
        assert_eq!(*calls.borrow(), ["pending", "close", "shutdown"]);
    }

    #[test]
    fn uninitialized_dism_with_no_session_needs_no_cleanup_or_pending() {
        finish_dism_lifecycle(
            false,
            0,
            |_| panic!("no session"),
            || panic!("not initialized"),
            || panic!("no owned cleanup to drain"),
            || panic!("no wait needed"),
        );
    }

    #[test]
    fn only_the_exact_scm_stopped_state_is_admissible() {
        assert!(explicitly_stopped(Some(1u32), 1));
        for state in [
            None,
            Some(0),
            Some(2),
            Some(3),
            Some(4),
            Some(5),
            Some(6),
            Some(7),
            Some(99),
        ] {
            assert!(
                !explicitly_stopped(state, 1),
                "unverified SCM state accepted: {state:?}"
            );
        }
    }

    #[test]
    fn actual_mutation_admission_is_retained_through_cancelled_os_drain() {
        use aethercore_operation_kernel::{MutationSupervisor, MutationWorkload};
        let supervisor = MutationSupervisor::new();
        let lease = supervisor
            .try_acquire(MutationWorkload::SystemRepair, "fixture-plan", "owner-a")
            .unwrap();
        let stopped = Arc::new(AtomicBool::new(false));
        let worker_stopped = stopped.clone();
        let (pending_tx, pending_rx) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            let _original_mutation_lease = lease;
            // The owned native boundary has returned, but OS lifetime has not.
            wait_until_stopped(
                || worker_stopped.load(Ordering::SeqCst),
                || pending_tx.send(()).unwrap(),
                || std::thread::sleep(std::time::Duration::from_millis(1)),
            );
        });
        pending_rx
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        assert!(supervisor.is_active());
        assert!(
            supervisor
                .try_acquire(MutationWorkload::Cleanup, "next", "owner-a")
                .is_err()
        );
        assert!(
            supervisor
                .try_acquire(MutationWorkload::Cleanup, "foreign", "owner-b")
                .is_err()
        );
        stopped.store(true, Ordering::SeqCst);
        worker.join().unwrap();
        assert!(!supervisor.is_active());
    }

    #[test]
    fn running_pending_or_unreadable_never_releases_before_explicit_stopped() {
        let mut observations = [false, false, false, true].into_iter();
        let mut reads = 0;
        let mut waits = 0;
        let mut notices = 0;
        wait_until_stopped(
            || {
                reads += 1;
                observations.next().expect("bounded fixture")
            },
            || notices += 1,
            || waits += 1,
        );
        assert_eq!(
            reads, 4,
            "admission released before the explicit STOPPED observation"
        );
        assert_eq!(waits, 3);
        assert_eq!(
            notices, 1,
            "pending should be published once without tick spam"
        );
    }

    #[test]
    fn already_stopped_does_not_invent_pending_work() {
        let mut notices = 0;
        wait_until_stopped(|| true, || notices += 1, || panic!("no wait needed"));
        assert_eq!(notices, 0);
    }

    #[test]
    fn accepted_cancellation_cannot_release_uncertain_servicing() {
        let cancelled = Arc::new(AtomicBool::new(false));
        let mut reads = 0;
        let mut waits = 0;
        wait_until_stopped(
            || {
                reads += 1;
                reads == 3
            },
            || cancelled.store(true, Ordering::SeqCst),
            || {
                assert!(cancelled.load(Ordering::SeqCst));
                waits += 1;
            },
        );
        assert_eq!(
            waits, 2,
            "cancel released admission while the OS remained uncertain"
        );
    }
}
