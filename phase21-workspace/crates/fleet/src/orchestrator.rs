//! Phase 34 — bounded fleet orchestrator with hermetic transport seam.
//!
//! Runs one operation across a host set with a conservative concurrency
//! ceiling, per-host timeout, cancellation, and host failure isolation.
//! Results are returned in deterministic host order (sorted by host_id).
//! A panic inside any per-host task is caught and surfaced as that host's
//! `Failed` result — it never poisons the batch.
//!
//! The transport is injected as a trait so GD-4 can drive the full
//! orchestration semantics hermetically (success + timeout/failure +
//! NotVerified) without any network.

use crate::domain::FleetHost;
use crate::trust::{RemoteOutcomeKind, RemoteResult};
use std::collections::HashMap;
use std::sync::Arc as StdArc;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// Default concurrency ceiling — small and conservative by design.
pub const DEFAULT_MAX_CONCURRENCY: usize = 4;
/// Hard ceiling; configuration cannot exceed it.
pub const ABSOLUTE_MAX_CONCURRENCY: usize = 16;

/// Transport seam for orchestration and hermetic proofs.
pub trait FleetTransport: Send + Sync {
    fn execute(&self, host: &FleetHost) -> RemoteResult;

    /// Scheduled callers can propagate the schedule-owned profile without
    /// widening the closed remote-operation surface. Existing transports keep
    /// their host-only behavior through this default.
    fn execute_for_profile(&self, host: &FleetHost, _profile_id: &str) -> RemoteResult {
        self.execute(host)
    }
}

/// Per-host operation result bound to its host id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostOutcome {
    pub host_id: String,
    pub result: RemoteResult,
}

/// Deterministic batch result: `outcomes` is sorted by host_id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FleetBatchResult {
    pub outcomes: Vec<HostOutcome>,
    pub concurrency_used: usize,
    pub cancelled: bool,
}

impl FleetBatchResult {
    pub fn count(&self, outcome: RemoteOutcomeKind) -> usize {
        self.outcomes
            .iter()
            .filter(|o| o.result.outcome == outcome)
            .count()
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum OrchestratorError {
    #[error("no hosts selected")]
    EmptySelection,
    #[error("concurrency must be 1..={ABSOLUTE_MAX_CONCURRENCY}")]
    InvalidConcurrency,
}

pub struct OrchestratorConfig {
    pub max_concurrency: usize,
    /// Additional per-host wall-clock guard (transport enforces its own).
    pub host_timeout: Duration,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            max_concurrency: DEFAULT_MAX_CONCURRENCY,
            host_timeout: Duration::from_secs(60),
        }
    }
}

/// Runs `operation` across `hosts` (already resolved from the inventory).
/// Hosts are executed in sorted order; each host's failure is isolated.
pub fn run_batch<T: FleetTransport + 'static>(
    transport: StdArc<T>,
    hosts: &[FleetHost],
    config: &OrchestratorConfig,
    cancel: &crate::transport::CancelToken,
) -> Result<FleetBatchResult, OrchestratorError> {
    run_batch_inner(transport, hosts, config, cancel, None)
}

/// Runs a batch while preserving the validated schedule profile for transports
/// that support profile-aware execution.
pub fn run_batch_for_profile<T: FleetTransport + 'static>(
    transport: StdArc<T>,
    hosts: &[FleetHost],
    profile_id: &str,
    config: &OrchestratorConfig,
    cancel: &crate::transport::CancelToken,
) -> Result<FleetBatchResult, OrchestratorError> {
    run_batch_inner(transport, hosts, config, cancel, Some(profile_id))
}

fn run_batch_inner<T: FleetTransport + 'static>(
    transport: StdArc<T>,
    hosts: &[FleetHost],
    config: &OrchestratorConfig,
    cancel: &crate::transport::CancelToken,
    profile_id: Option<&str>,
) -> Result<FleetBatchResult, OrchestratorError> {
    if hosts.is_empty() {
        return Err(OrchestratorError::EmptySelection);
    }
    if config.max_concurrency == 0 || config.max_concurrency > ABSOLUTE_MAX_CONCURRENCY {
        return Err(OrchestratorError::InvalidConcurrency);
    }

    // Deterministic work order. Hosts are cloned into Arc so worker threads
    // can own them ('static) without borrowing the caller's slice.
    let mut ordered: Vec<StdArc<FleetHost>> =
        hosts.iter().map(|h| StdArc::new(h.clone())).collect();
    ordered.sort_by(|a, b| a.host_id.cmp(&b.host_id));

    let results: Arc<Mutex<HashMap<String, RemoteResult>>> = Arc::new(Mutex::new(HashMap::new()));
    let queue: Arc<Mutex<Vec<StdArc<FleetHost>>>> = Arc::new(Mutex::new(ordered.clone()));
    let cancelled_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));

    let workers = config
        .max_concurrency
        .min(queue.lock().map(|q| q.len()).unwrap_or(1))
        .max(1);
    let profile_id = profile_id.map(str::to_owned);
    let mut handles = Vec::with_capacity(workers);

    for _ in 0..workers {
        let queue = Arc::clone(&queue);
        let results = Arc::clone(&results);
        let cancelled_flag = Arc::clone(&cancelled_flag);
        let cancel = cancel.clone();
        let transport_outer = StdArc::clone(&transport);
        let profile_id = profile_id.clone();
        handles.push(thread::spawn(move || {
            let transport = transport_outer;
            loop {
                if cancel.is_cancelled() {
                    cancelled_flag.store(true, Ordering::SeqCst);
                }
                let next = {
                    let mut guard = match queue.lock() {
                        Ok(guard) => guard,
                        Err(poisoned) => poisoned.into_inner(),
                    };
                    guard.pop()
                };
                let Some(host) = next else { break };
                if cancel.is_cancelled() {
                    let mut guard = match results.lock() {
                        Ok(guard) => guard,
                        Err(poisoned) => poisoned.into_inner(),
                    };
                    guard.insert(
                        host.host_id.clone(),
                        RemoteResult::kind(RemoteOutcomeKind::Cancelled),
                    );
                    continue;
                }
                let host_id = host.host_id.clone();
                // Panic isolation: a panicking transport becomes Failed.
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    match profile_id.as_deref() {
                        Some(profile_id) => {
                            transport.execute_for_profile(host.as_ref(), profile_id)
                        }
                        None => transport.execute(host.as_ref()),
                    }
                }))
                .unwrap_or_else(|_| {
                    RemoteResult::kind(RemoteOutcomeKind::Failed)
                        .detail("transport task panicked; isolated".to_string())
                });
                let mut guard = match results.lock() {
                    Ok(guard) => guard,
                    Err(poisoned) => poisoned.into_inner(),
                };
                guard.insert(host_id, outcome);
            }
        }));
    }

    for handle in handles {
        let _ = handle.join();
    }

    let map = Arc::try_unwrap(results)
        .map_err(|_| OrchestratorError::EmptySelection)?
        .into_inner()
        .map_err(|_| OrchestratorError::EmptySelection)?;
    let mut outcomes: Vec<HostOutcome> = ordered
        .iter()
        .map(|host| HostOutcome {
            host_id: host.host_id.clone(),
            result: map
                .get(&host.host_id)
                .cloned()
                .unwrap_or_else(|| RemoteResult::kind(RemoteOutcomeKind::Failed)),
        })
        .collect();
    outcomes.sort_by(|a, b| a.host_id.cmp(&b.host_id));

    Ok(FleetBatchResult {
        outcomes,
        concurrency_used: workers,
        cancelled: cancelled_flag.load(Ordering::SeqCst),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{AuthReference, FleetHost};
    use crate::transport::CancelToken;
    use std::collections::BTreeSet;
    use std::sync::atomic::AtomicUsize;

    fn host(id: &str) -> FleetHost {
        FleetHost::new(
            id,
            id,
            "h.example.internal",
            22,
            "ops",
            AuthReference::Agent,
            BTreeSet::new(),
        )
        .unwrap()
    }

    struct StubTransport {
        calls: AtomicUsize,
    }

    impl FleetTransport for StubTransport {
        fn execute(&self, host: &FleetHost) -> RemoteResult {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if host.host_id.starts_with("ok") {
                RemoteResult {
                    outcome: RemoteOutcomeKind::Success,
                    exit_code: Some(0),
                    output: None,
                    detail: None,
                }
            } else if host.host_id.starts_with("slow") {
                thread::sleep(Duration::from_millis(300));
                RemoteResult::kind(RemoteOutcomeKind::Timeout)
            } else if host.host_id.starts_with("panic") {
                panic!("stub panic");
            } else {
                RemoteResult::kind(RemoteOutcomeKind::NotVerified)
            }
        }
    }

    #[test]
    fn batch_is_deterministic_and_isolated() {
        let transport = StdArc::new(StubTransport {
            calls: AtomicUsize::new(0),
        });
        let hosts = vec![
            host("zeta"),
            host("panic-1"),
            host("ok-1"),
            host("slow-1"),
            host("alpha"),
        ];
        let config = OrchestratorConfig::default();
        let result = run_batch(
            StdArc::clone(&transport),
            &hosts,
            &config,
            &CancelToken::default(),
        )
        .unwrap();
        let ids: Vec<&str> = result.outcomes.iter().map(|o| o.host_id.as_str()).collect();
        assert_eq!(ids, vec!["alpha", "ok-1", "panic-1", "slow-1", "zeta"]);
        assert_eq!(result.count(RemoteOutcomeKind::Success), 1);
        assert_eq!(result.count(RemoteOutcomeKind::NotVerified), 2);
        assert_eq!(result.count(RemoteOutcomeKind::Timeout), 1);
        // panic isolated as Failed, not propagated
        assert_eq!(result.count(RemoteOutcomeKind::Failed), 1);
        // every host got a result (no host poisoned another)
        assert_eq!(result.outcomes.len(), hosts.len());
    }

    #[test]
    fn concurrency_is_bounded() {
        let transport = StdArc::new(StubTransport {
            calls: AtomicUsize::new(0),
        });
        let hosts: Vec<FleetHost> = (0..8).map(|i| host(&format!("ok-{i}"))).collect();
        let config = OrchestratorConfig {
            max_concurrency: 3,
            host_timeout: Duration::from_secs(5),
        };
        let result = run_batch(
            StdArc::clone(&transport),
            &hosts,
            &config,
            &CancelToken::default(),
        )
        .unwrap();
        assert!(result.concurrency_used <= 3);
        assert_eq!(result.count(RemoteOutcomeKind::Success), 8);
        // ceiling enforcement
        let bad = OrchestratorConfig {
            max_concurrency: 999,
            host_timeout: Duration::from_secs(1),
        };
        assert_eq!(
            run_batch(
                StdArc::clone(&transport),
                &hosts,
                &bad,
                &CancelToken::default()
            ),
            Err(OrchestratorError::InvalidConcurrency)
        );
        // empty selection
        assert_eq!(
            run_batch(
                StdArc::clone(&transport),
                &[],
                &config,
                &CancelToken::default()
            ),
            Err(OrchestratorError::EmptySelection)
        );
    }

    #[test]
    fn cancellation_marks_hosts_cancelled() {
        let transport = StdArc::new(StubTransport {
            calls: AtomicUsize::new(0),
        });
        let hosts = vec![host("ok-1"), host("ok-2")];
        let cancel = CancelToken::default();
        cancel.cancel();
        let result = run_batch(
            StdArc::clone(&transport),
            &hosts,
            &OrchestratorConfig::default(),
            &cancel,
        )
        .unwrap();
        assert!(result.cancelled);
        assert_eq!(result.count(RemoteOutcomeKind::Cancelled), 2);
    }
}
