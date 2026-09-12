#![forbid(unsafe_code)]

use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const DEFAULT_COLLECTOR_TIMEOUT: Duration = Duration::from_secs(8);
pub const WMI_NEXT_SLICE: Duration = Duration::from_millis(750);
pub const EVENTLOG_NEXT_SLICE: Duration = Duration::from_millis(500);
pub const STORAGE_IOCTL_TIMEOUT: Duration = Duration::from_millis(2_000);
pub const MAX_FAULT_DETAIL_BYTES: usize = 4 * 1024;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum FaultKind {
    Timeout,
    Cancelled,
    Unavailable,
    PermissionDenied,
    MalformedResponse,
    ProviderFailure,
    Io,
    Internal,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
#[error("{provider}/{operation}: {kind:?}: {detail}")]
pub struct CollectorFault {
    pub provider: &'static str,
    pub operation: &'static str,
    pub kind: FaultKind,
    pub detail: String,
}

impl FaultKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Timeout => "timeout",
            Self::Cancelled => "cancelled",
            Self::Unavailable => "unavailable",
            Self::PermissionDenied => "permissionDenied",
            Self::MalformedResponse => "malformedResponse",
            Self::ProviderFailure => "providerFailure",
            Self::Io => "io",
            Self::Internal => "internal",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CollectorFaultRecord {
    pub provider: String,
    pub operation: String,
    pub kind: FaultKind,
    pub detail: String,
}

impl CollectorFaultRecord {
    pub fn new(
        provider: impl Into<String>,
        operation: impl Into<String>,
        kind: FaultKind,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            provider: provider.into(),
            operation: operation.into(),
            kind,
            detail: bounded_detail(detail.into()),
        }
    }
}

impl From<&CollectorFault> for CollectorFaultRecord {
    fn from(fault: &CollectorFault) -> Self {
        Self::new(
            fault.provider,
            fault.operation,
            fault.kind,
            fault.detail.clone(),
        )
    }
}

fn bounded_detail(mut detail: String) -> String {
    if detail.len() <= MAX_FAULT_DETAIL_BYTES {
        return detail;
    }
    const SUFFIX: &str = "…[truncated]";
    let mut end = MAX_FAULT_DETAIL_BYTES.saturating_sub(SUFFIX.len());
    while end > 0 && !detail.is_char_boundary(end) {
        end -= 1;
    }
    detail.truncate(end);
    detail.push_str(SUFFIX);
    debug_assert!(detail.len() <= MAX_FAULT_DETAIL_BYTES);
    detail
}

impl CollectorFault {
    pub fn new(
        provider: &'static str,
        operation: &'static str,
        kind: FaultKind,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            provider,
            operation,
            kind,
            detail: bounded_detail(detail.into()),
        }
    }

    pub fn timeout(provider: &'static str, operation: &'static str) -> Self {
        Self::new(
            provider,
            operation,
            FaultKind::Timeout,
            "collector deadline exceeded",
        )
    }

    pub fn cancelled(provider: &'static str, operation: &'static str) -> Self {
        Self::new(
            provider,
            operation,
            FaultKind::Cancelled,
            "collector cancelled",
        )
    }
}

#[derive(Clone, Default)]
pub struct IsolationGate {
    active: Arc<AtomicBool>,
}

struct IsolationLease {
    active: Arc<AtomicBool>,
}

impl Drop for IsolationLease {
    fn drop(&mut self) {
        self.active.store(false, Ordering::Release);
    }
}

impl IsolationGate {
    fn try_enter(
        &self,
        provider: &'static str,
        operation: &'static str,
    ) -> Result<IsolationLease, CollectorFault> {
        self.active
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| {
                CollectorFault::new(
                    provider,
                    operation,
                    FaultKind::Unavailable,
                    "previous collector invocation is still active after a watchdog timeout",
                )
            })?;
        Ok(IsolationLease {
            active: self.active.clone(),
        })
    }

    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }
}

#[derive(Clone)]
pub struct CancellationToken {
    node: Arc<CancellationNode>,
}

struct CancellationNode {
    cancelled: AtomicBool,
    parent: Option<Arc<CancellationNode>>,
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self {
            node: Arc::new(CancellationNode {
                cancelled: AtomicBool::new(false),
                parent: None,
            }),
        }
    }
}

#[derive(Clone)]
pub struct CommitFence {
    state: Arc<std::sync::Mutex<CommitFenceState>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CommitFenceState {
    Active,
    Committed,
    Revoked,
}

impl Default for CommitFence {
    fn default() -> Self {
        Self::new()
    }
}

impl CommitFence {
    pub fn new() -> Self {
        Self {
            state: Arc::new(std::sync::Mutex::new(CommitFenceState::Active)),
        }
    }

    /// Revokes future publication only while the fence is still Active. A commit that already
    /// linearized is immutable: later user activity may cancel remaining work, but cannot relabel
    /// an already-published snapshot as stale.
    pub fn revoke(&self) {
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        if *state == CommitFenceState::Active {
            *state = CommitFenceState::Revoked;
        }
    }

    pub fn is_valid(&self) -> bool {
        *self.state.lock().unwrap_or_else(|p| p.into_inner()) != CommitFenceState::Revoked
    }

    pub fn is_committed(&self) -> bool {
        *self.state.lock().unwrap_or_else(|p| p.into_inner()) == CommitFenceState::Committed
    }

    /// Unconditional publication boundary. The closure runs only if no revocation linearized first.
    pub fn try_commit<T>(&self, commit: impl FnOnce() -> T) -> Option<T> {
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        if *state != CommitFenceState::Active {
            return None;
        }
        let value = commit();
        *state = CommitFenceState::Committed;
        Some(value)
    }

    /// Conditional publication boundary for stateful stores. The closure can reject publication
    /// while the same fence lock is held (for example if an interactive scan became active). Only
    /// `Some` marks the fence Committed; `None` leaves it Active for the caller to cancel/revoke.
    pub fn try_commit_checked<T>(&self, commit: impl FnOnce() -> Option<T>) -> Option<T> {
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        if *state != CommitFenceState::Active {
            return None;
        }
        let value = commit()?;
        *state = CommitFenceState::Committed;
        Some(value)
    }
}

impl CancellationToken {
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates an independently cancellable child that also observes cancellation of every
    /// ancestor. A child timeout therefore does not poison sibling collectors, while a parent
    /// cancellation tears down the complete diagnostic fan-out cooperatively.
    pub fn child(&self) -> Self {
        Self {
            node: Arc::new(CancellationNode {
                cancelled: AtomicBool::new(false),
                parent: Some(self.node.clone()),
            }),
        }
    }

    pub fn cancel(&self) {
        self.node.cancelled.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        let mut current = Some(self.node.as_ref());
        while let Some(node) = current {
            if node.cancelled.load(Ordering::Acquire) {
                return true;
            }
            current = node.parent.as_deref();
        }
        false
    }
}

#[derive(Clone)]
pub struct CollectorControl {
    token: CancellationToken,
    deadline: Instant,
}

impl CollectorControl {
    pub fn with_timeout(timeout: Duration) -> Self {
        Self {
            token: CancellationToken::new(),
            deadline: Instant::now() + timeout,
        }
    }

    pub fn with_token(timeout: Duration, token: CancellationToken) -> Self {
        Self {
            token,
            deadline: Instant::now() + timeout,
        }
    }

    pub fn cancellation(&self) -> CancellationToken {
        self.token.clone()
    }
    pub fn cancel(&self) {
        self.token.cancel();
    }
    pub fn is_cancelled(&self) -> bool {
        self.token.is_cancelled()
    }
    pub fn expired(&self) -> bool {
        Instant::now() >= self.deadline
    }

    pub fn checkpoint(
        &self,
        provider: &'static str,
        operation: &'static str,
    ) -> Result<(), CollectorFault> {
        if self.is_cancelled() {
            return Err(CollectorFault::cancelled(provider, operation));
        }
        if self.expired() {
            return Err(CollectorFault::timeout(provider, operation));
        }
        Ok(())
    }

    pub fn remaining(&self) -> Duration {
        self.deadline.saturating_duration_since(Instant::now())
    }

    pub fn remaining_ms_capped(&self, cap: Duration) -> u32 {
        let millis = self.remaining().min(cap).as_millis().max(1);
        u32::try_from(millis).unwrap_or(u32::MAX)
    }
}

/// Runs a collector inside a fault boundary. A timeout cancels the cooperative token and returns
/// immediately to the caller. Platform calls inside the worker must themselves use bounded waits;
/// this watchdog is defense-in-depth and prevents one failed provider from blocking its siblings.
pub fn run_isolated<T, F>(
    provider: &'static str,
    operation: &'static str,
    timeout: Duration,
    work: F,
) -> Result<T, CollectorFault>
where
    T: Send + 'static,
    F: FnOnce(CollectorControl) -> Result<T, CollectorFault> + Send + 'static,
{
    run_isolated_with_token(provider, operation, timeout, CancellationToken::new(), work)
}

pub fn run_isolated_with_token<T, F>(
    provider: &'static str,
    operation: &'static str,
    timeout: Duration,
    token: CancellationToken,
    work: F,
) -> Result<T, CollectorFault>
where
    T: Send + 'static,
    F: FnOnce(CollectorControl) -> Result<T, CollectorFault> + Send + 'static,
{
    run_isolated_inner(provider, operation, timeout, token, None, work)
}

pub fn run_isolated_gated<T, F>(
    gate: &IsolationGate,
    provider: &'static str,
    operation: &'static str,
    timeout: Duration,
    work: F,
) -> Result<T, CollectorFault>
where
    T: Send + 'static,
    F: FnOnce(CollectorControl) -> Result<T, CollectorFault> + Send + 'static,
{
    run_isolated_gated_with_token(
        gate,
        provider,
        operation,
        timeout,
        CancellationToken::new(),
        work,
    )
}

pub fn run_isolated_gated_with_token<T, F>(
    gate: &IsolationGate,
    provider: &'static str,
    operation: &'static str,
    timeout: Duration,
    token: CancellationToken,
    work: F,
) -> Result<T, CollectorFault>
where
    T: Send + 'static,
    F: FnOnce(CollectorControl) -> Result<T, CollectorFault> + Send + 'static,
{
    let lease = gate.try_enter(provider, operation)?;
    run_isolated_inner(provider, operation, timeout, token, Some(lease), work)
}

fn run_isolated_inner<T, F>(
    provider: &'static str,
    operation: &'static str,
    timeout: Duration,
    token: CancellationToken,
    lease: Option<IsolationLease>,
    work: F,
) -> Result<T, CollectorFault>
where
    T: Send + 'static,
    F: FnOnce(CollectorControl) -> Result<T, CollectorFault> + Send + 'static,
{
    let control = CollectorControl::with_token(timeout, token);
    if control.is_cancelled() {
        return Err(CollectorFault::cancelled(provider, operation));
    }
    let worker_control = control.clone();
    let (tx, rx) = mpsc::sync_channel(1);
    let worker_name = format!("aether-collector-{provider}-{operation}");
    let handle = thread::Builder::new()
        .name(worker_name)
        .spawn(move || {
            // Keep the isolation lease in the worker. If the watchdog times out, the provider stays
            // quarantined until this worker really exits instead of allowing unbounded stuck threads.
            let lease = lease;
            let result =
                catch_unwind(AssertUnwindSafe(|| work(worker_control))).unwrap_or_else(|_| {
                    Err(CollectorFault::new(
                        provider,
                        operation,
                        FaultKind::Internal,
                        "collector panicked inside isolation boundary",
                    ))
                });
            // Completion becomes externally visible only after the isolation lease is released.
            // This prevents an immediate retry from observing a stale active gate after success.
            drop(lease);
            let _ = tx.send(result);
        })
        .map_err(|error| {
            CollectorFault::new(
                provider,
                operation,
                FaultKind::Internal,
                format!("failed to spawn isolated collector worker: {error}"),
            )
        })?;
    // Dropping JoinHandle intentionally detaches the worker. On watchdog timeout the worker may
    // still be inside a platform call; its gate remains held until it really exits.
    drop(handle);

    // Poll the channel in short slices so an ancestor cancellation can stop this supervisor
    // promptly instead of waiting until its full watchdog interval expires.
    const SUPERVISOR_SLICE: Duration = Duration::from_millis(50);
    loop {
        if control.is_cancelled() {
            return Err(CollectorFault::cancelled(provider, operation));
        }
        let remaining = control.remaining();
        if remaining.is_zero() {
            control.cancel();
            return Err(CollectorFault::timeout(provider, operation));
        }
        match rx.recv_timeout(remaining.min(SUPERVISOR_SLICE)) {
            Ok(result) => return result,
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err(CollectorFault::new(
                    provider,
                    operation,
                    FaultKind::Internal,
                    "collector worker disconnected before returning a result",
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fault_detail_is_utf8_bounded_before_crossing_process_boundaries() {
        let detail = "عطل".repeat(4_000);
        let fault =
            CollectorFault::new("test", "detail", FaultKind::ProviderFailure, detail.clone());
        assert!(fault.detail.len() <= MAX_FAULT_DETAIL_BYTES);
        assert!(fault.detail.is_char_boundary(fault.detail.len()));
        assert!(fault.detail.ends_with("…[truncated]"));
        let record =
            CollectorFaultRecord::new("test", "detail", FaultKind::ProviderFailure, detail);
        assert!(record.detail.len() <= MAX_FAULT_DETAIL_BYTES);
        assert!(record.detail.ends_with("…[truncated]"));
    }

    #[test]
    fn cooperative_deadline_is_monotonic_and_bounded() {
        let control = CollectorControl::with_timeout(Duration::from_millis(10));
        assert!(control.remaining_ms_capped(Duration::from_secs(1)) <= 10);
        thread::sleep(Duration::from_millis(15));
        assert!(matches!(
            control.checkpoint("test", "deadline"),
            Err(CollectorFault {
                kind: FaultKind::Timeout,
                ..
            })
        ));
    }

    #[test]
    fn watchdog_cancels_and_returns_timeout() {
        let started = Instant::now();
        let result = run_isolated("test", "hang", Duration::from_millis(20), |control| {
            while !control.is_cancelled() {
                thread::sleep(Duration::from_millis(2));
            }
            Err::<(), _>(CollectorFault::cancelled("test", "hang"))
        });
        assert!(matches!(
            result,
            Err(CollectorFault {
                kind: FaultKind::Timeout,
                ..
            })
        ));
        assert!(started.elapsed() < Duration::from_millis(250));
    }

    #[test]
    fn timed_out_provider_remains_quarantined_until_worker_exits() {
        let gate = IsolationGate::default();
        let worker_gate = gate.clone();
        let first = run_isolated_gated(
            &gate,
            "test",
            "gate",
            Duration::from_millis(15),
            |control| {
                while !control.is_cancelled() {
                    thread::sleep(Duration::from_millis(2));
                }
                thread::sleep(Duration::from_millis(40));
                Ok::<_, CollectorFault>(())
            },
        );
        assert!(matches!(
            first,
            Err(CollectorFault {
                kind: FaultKind::Timeout,
                ..
            })
        ));
        assert!(worker_gate.is_active());
        let second = run_isolated_gated(
            &gate,
            "test",
            "gate",
            Duration::from_millis(15),
            |_control| Ok::<_, CollectorFault>(()),
        );
        assert!(matches!(
            second,
            Err(CollectorFault {
                kind: FaultKind::Unavailable,
                ..
            })
        ));
        // P36 (Hermes): wait until the worker really exits instead of assuming a fixed
        // 60 ms suffices — on a loaded ARM64 VM thread-scheduling jitter can exceed it
        // without changing the quarantined-until-exit contract being proven here.
        let released = {
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                if !worker_gate.is_active() {
                    break true;
                }
                if Instant::now() >= deadline {
                    break false;
                }
                thread::sleep(Duration::from_millis(10));
            }
        };
        assert!(
            !worker_gate.is_active(),
            "worker gate never released within 5s"
        );
        assert!(released);
    }

    #[test]
    fn successful_gated_provider_releases_before_result_is_visible() {
        let gate = IsolationGate::default();
        let result = run_isolated_gated(
            &gate,
            "test",
            "release-order",
            Duration::from_secs(1),
            |_control| Ok::<_, CollectorFault>(42u32),
        );
        assert_eq!(result.unwrap(), 42);
        assert!(!gate.is_active());
        let retry = run_isolated_gated(
            &gate,
            "test",
            "release-order",
            Duration::from_secs(1),
            |_control| Ok::<_, CollectorFault>(43u32),
        );
        assert_eq!(retry.unwrap(), 43);
    }

    #[test]
    fn panic_is_contained_as_internal_fault() {
        let result = run_isolated(
            "test",
            "panic",
            Duration::from_secs(1),
            |_control| -> Result<(), CollectorFault> {
                panic!("fault injection");
            },
        );
        assert!(matches!(
            result,
            Err(CollectorFault {
                kind: FaultKind::Internal,
                ..
            })
        ));
    }

    #[test]
    fn parent_cancellation_propagates_to_children_without_reverse_poisoning() {
        let parent = CancellationToken::new();
        let child_a = parent.child();
        let child_b = parent.child();
        child_a.cancel();
        assert!(child_a.is_cancelled());
        assert!(!child_b.is_cancelled());
        assert!(!parent.is_cancelled());
        parent.cancel();
        assert!(child_b.is_cancelled());
    }

    #[test]
    fn poisoned_commit_fence_mutex_recovers_with_single_publication_boundary() {
        let fence = CommitFence::new();
        let poison = fence.clone();
        assert!(
            thread::spawn(move || {
                let _guard = poison.state.lock().unwrap();
                panic!("intentional mutex poison for deterministic recovery test");
            })
            .join()
            .is_err()
        );
        assert_eq!(fence.try_commit(|| 11u32), Some(11));
        assert!(fence.is_committed());
        fence.revoke();
        assert_eq!(fence.try_commit(|| 12u32), None);
    }

    #[test]
    fn revoked_commit_fence_rejects_late_publication() {
        let fence = CommitFence::new();
        fence.revoke();
        let mut published = false;
        assert!(
            fence
                .try_commit(|| {
                    published = true;
                })
                .is_none()
        );
        assert!(!published);
    }

    #[test]
    fn commit_and_revoke_are_linearized_by_one_boundary() {
        let fence = CommitFence::new();
        let committed = fence.try_commit(|| 7u32);
        assert_eq!(committed, Some(7));
        assert!(fence.is_committed());
        fence.revoke();
        assert!(fence.is_valid());
        assert!(fence.is_committed());
        assert!(fence.try_commit(|| 8u32).is_none());
    }

    #[test]
    fn checked_commit_can_decline_without_claiming_publication() {
        let fence = CommitFence::new();
        assert_eq!(fence.try_commit_checked(|| None::<u32>), None);
        assert!(!fence.is_committed());
        fence.revoke();
        assert!(!fence.is_valid());
    }

    #[test]
    fn external_cancellation_interrupts_supervisor_before_watchdog_deadline() {
        let token = CancellationToken::new();
        let cancel = token.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(20));
            cancel.cancel();
        });
        let started = Instant::now();
        let result = run_isolated_with_token(
            "test",
            "external-cancel",
            Duration::from_secs(2),
            token,
            |control| {
                while !control.is_cancelled() {
                    thread::sleep(Duration::from_millis(2));
                }
                Err::<(), _>(CollectorFault::cancelled("test", "external-cancel"))
            },
        );
        assert!(matches!(
            result,
            Err(CollectorFault {
                kind: FaultKind::Cancelled,
                ..
            })
        ));
        assert!(started.elapsed() < Duration::from_millis(500));
    }
}
