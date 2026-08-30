use std::sync::{Arc, Mutex};

use chrono::Utc;
use thiserror::Error;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MutationWorkload {
    DriverInstall,
    SystemRepair,
    Cleanup,
    Startup,
    Update,
    /// Phase 20: reversible performance optimizations governed by the same single-flight lease.
    Optimization,
}

impl MutationWorkload {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DriverInstall => "DriverInstall",
            Self::SystemRepair => "SystemRepair",
            Self::Cleanup => "Cleanup",
            Self::Startup => "Startup",
            Self::Update => "Update",
            Self::Optimization => "Optimization",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MutationLeaseSnapshot {
    pub lease_id: String,
    pub workload: MutationWorkload,
    pub plan_id: String,
    pub owner_principal_key: String,
    pub acquired_unix_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MutationLeaseChange {
    Acquired(MutationLeaseSnapshot),
    Released(MutationLeaseSnapshot),
}

#[derive(Debug, Error)]
pub enum MutationError {
    #[error("mutation lease requires a non-empty owner principal and plan id")]
    InvalidIdentity,
    #[error("machine mutation is already active for this principal: {workload} plan {plan_id}")]
    BusyOwned {
        workload: &'static str,
        plan_id: String,
    },
    #[error("machine mutation is already active")]
    BusyOtherPrincipal,
}

type Observer = Arc<dyn Fn(MutationLeaseChange) + Send + Sync + 'static>;

#[derive(Clone)]
pub struct MutationSupervisor {
    active: Arc<Mutex<Option<MutationLeaseSnapshot>>>,
    observer: Option<Observer>,
}

impl Default for MutationSupervisor {
    fn default() -> Self {
        Self {
            active: Arc::new(Mutex::new(None)),
            observer: None,
        }
    }
}

pub struct MutationLease {
    supervisor: MutationSupervisor,
    lease_id: String,
    snapshot: MutationLeaseSnapshot,
}

impl MutationSupervisor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_observer(observer: Observer) -> Self {
        Self {
            active: Arc::new(Mutex::new(None)),
            observer: Some(observer),
        }
    }

    pub fn try_acquire(
        &self,
        workload: MutationWorkload,
        plan_id: &str,
        owner_principal_key: &str,
    ) -> Result<MutationLease, MutationError> {
        if plan_id.trim().is_empty() || owner_principal_key.trim().is_empty() {
            return Err(MutationError::InvalidIdentity);
        }
        let snapshot = {
            let mut active = self.active.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(current) = active.as_ref() {
                return Err(if current.owner_principal_key == owner_principal_key {
                    MutationError::BusyOwned {
                        workload: current.workload.as_str(),
                        plan_id: current.plan_id.clone(),
                    }
                } else {
                    // A machine-wide lease necessarily coordinates principals, but contention must
                    // not disclose another principal's workload, plan identifier or ownership key.
                    MutationError::BusyOtherPrincipal
                });
            }
            let snapshot = MutationLeaseSnapshot {
                lease_id: Uuid::new_v4().to_string(),
                workload,
                plan_id: plan_id.to_owned(),
                owner_principal_key: owner_principal_key.to_owned(),
                acquired_unix_ms: Utc::now().timestamp_millis(),
            };
            *active = Some(snapshot.clone());
            snapshot
        };

        if let Some(observer) = self.observer.as_ref() {
            observer(MutationLeaseChange::Acquired(snapshot.clone()));
        }

        Ok(MutationLease {
            supervisor: self.clone(),
            lease_id: snapshot.lease_id.clone(),
            snapshot,
        })
    }

    /// Machine-wide observation used by passive background scheduling. This deliberately reveals
    /// only whether a mutation exists; it never exposes another principal's workload or plan.
    pub fn is_active(&self) -> bool {
        self.active
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .is_some()
    }

    pub fn snapshot_for_owner(&self, owner_principal_key: &str) -> Option<MutationLeaseSnapshot> {
        self.active
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_ref()
            .filter(|snapshot| snapshot.owner_principal_key == owner_principal_key)
            .cloned()
    }
}

impl MutationLease {
    pub fn snapshot(&self) -> &MutationLeaseSnapshot {
        &self.snapshot
    }

    /// Verify that a lease is being handed to the exact mutation worker it was admitted for.
    /// This prevents a caller from acquiring one workload/plan identity and accidentally using the
    /// guard to authorize the lifetime of a different mutation.
    pub fn matches(
        &self,
        workload: MutationWorkload,
        plan_id: &str,
        owner_principal_key: &str,
    ) -> bool {
        self.snapshot.workload == workload
            && self.snapshot.plan_id == plan_id
            && self.snapshot.owner_principal_key == owner_principal_key
    }
}

impl Drop for MutationLease {
    fn drop(&mut self) {
        let released = {
            let mut active = self
                .supervisor
                .active
                .lock()
                .unwrap_or_else(|p| p.into_inner());
            if active
                .as_ref()
                .is_some_and(|value| value.lease_id == self.lease_id)
            {
                active.take()
            } else {
                None
            }
        };
        if let (Some(observer), Some(snapshot)) =
            (self.supervisor.observer.as_ref(), released)
        {
            observer(MutationLeaseChange::Released(snapshot));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mutation_lease_rejects_unscoped_identity() {
        let supervisor = MutationSupervisor::new();
        assert!(matches!(
            supervisor.try_acquire(MutationWorkload::Cleanup, "p1", ""),
            Err(MutationError::InvalidIdentity)
        ));
        assert!(matches!(
            supervisor.try_acquire(MutationWorkload::Cleanup, "", "owner"),
            Err(MutationError::InvalidIdentity)
        ));
    }

    #[test]
    fn poisoned_mutex_recovers_without_weakening_mutation_exclusion() {
        let supervisor = MutationSupervisor::new();
        let poison = supervisor.clone();
        assert!(std::thread::spawn(move || {
            let _guard = poison.active.lock().unwrap();
            panic!("intentional mutex poison for deterministic recovery test");
        }).join().is_err());
        let lease = supervisor.try_acquire(MutationWorkload::Cleanup, "p1", "owner-a").unwrap();
        assert!(supervisor.is_active());
        assert!(matches!(
            supervisor.try_acquire(MutationWorkload::DriverInstall, "p2", "owner-b"),
            Err(MutationError::BusyOtherPrincipal)
        ));
        drop(lease);
        assert!(!supervisor.is_active());
    }

    #[test]
    fn active_probe_exposes_only_boolean_machine_contention() {
        let supervisor = MutationSupervisor::new();
        assert!(!supervisor.is_active());
        let lease = supervisor
            .try_acquire(MutationWorkload::Cleanup, "private-plan", "private-owner")
            .unwrap();
        assert!(supervisor.is_active());
        drop(lease);
        assert!(!supervisor.is_active());
    }

    #[test]
    fn exactly_one_mutation_can_own_the_machine() {
        let supervisor = MutationSupervisor::new();
        let lease = supervisor
            .try_acquire(MutationWorkload::Cleanup, "p1", "u1")
            .unwrap();
        assert!(matches!(
            supervisor.try_acquire(MutationWorkload::DriverInstall, "p2", "u2"),
            Err(MutationError::BusyOtherPrincipal)
        ));
        drop(lease);
        assert!(supervisor
            .try_acquire(MutationWorkload::DriverInstall, "p2", "u2")
            .is_ok());
    }

    #[test]
    fn cross_principal_contention_never_discloses_foreign_plan_identity() {
        let supervisor = MutationSupervisor::new();
        let _lease = supervisor
            .try_acquire(MutationWorkload::Cleanup, "private-plan-a", "owner-a")
            .unwrap();
        let error = match supervisor
            .try_acquire(MutationWorkload::DriverInstall, "plan-b", "owner-b")
        {
            Err(error) => error,
            Ok(_) => panic!("cross-principal mutation unexpectedly acquired the machine lease"),
        };
        assert!(matches!(error, MutationError::BusyOtherPrincipal));
        let display = error.to_string();
        assert!(!display.contains("private-plan-a"));
        assert!(!display.contains("Cleanup"));
        assert!(!display.contains("owner-a"));
    }

    #[test]
    fn active_lease_snapshot_is_owner_scoped() {
        let supervisor = MutationSupervisor::new();
        let _lease = supervisor
            .try_acquire(MutationWorkload::Cleanup, "plan-a", "owner-a")
            .unwrap();
        assert_eq!(
            supervisor.snapshot_for_owner("owner-a").map(|value| value.plan_id),
            Some("plan-a".into())
        );
        assert!(supervisor.snapshot_for_owner("owner-b").is_none());
    }

    #[test]
    fn same_principal_contention_may_identify_its_own_active_plan() {
        let supervisor = MutationSupervisor::new();
        let _lease = supervisor
            .try_acquire(MutationWorkload::Cleanup, "plan-a", "owner-a")
            .unwrap();
        assert!(matches!(
            supervisor.try_acquire(MutationWorkload::DriverInstall, "plan-b", "owner-a"),
            Err(MutationError::BusyOwned { workload: "Cleanup", plan_id }) if plan_id == "plan-a"
        ));
    }

    #[test]
    fn concurrent_contenders_never_overlap_machine_mutation_leases() {
        use std::sync::{
            Barrier,
            atomic::{AtomicUsize, Ordering},
        };

        const WORKERS: usize = 32;
        const ATTEMPTS: usize = 200;
        let supervisor = MutationSupervisor::new();
        let barrier = Arc::new(Barrier::new(WORKERS + 1));
        let observed_active = Arc::new(AtomicUsize::new(0));
        let acquisitions = Arc::new(AtomicUsize::new(0));
        let mut workers = Vec::with_capacity(WORKERS);
        for worker in 0..WORKERS {
            let supervisor = supervisor.clone();
            let barrier = barrier.clone();
            let observed_active = observed_active.clone();
            let acquisitions = acquisitions.clone();
            workers.push(std::thread::spawn(move || {
                let workload = match worker % 5 {
                    0 => MutationWorkload::DriverInstall,
                    1 => MutationWorkload::SystemRepair,
                    2 => MutationWorkload::Cleanup,
                    3 => MutationWorkload::Startup,
                    _ => MutationWorkload::Update,
                };
                barrier.wait();
                for attempt in 0..ATTEMPTS {
                    let plan = format!("stress-{worker}-{attempt}");
                    let owner = format!("owner-{worker}");
                    if let Ok(lease) = supervisor.try_acquire(workload, &plan, &owner) {
                        assert_eq!(observed_active.fetch_add(1, Ordering::SeqCst), 0);
                        acquisitions.fetch_add(1, Ordering::Relaxed);
                        std::thread::yield_now();
                        assert_eq!(observed_active.fetch_sub(1, Ordering::SeqCst), 1);
                        drop(lease);
                    } else {
                        std::thread::yield_now();
                    }
                }
            }));
        }
        barrier.wait();
        for worker in workers {
            worker.join().expect("mutation stress worker panicked");
        }
        assert_eq!(observed_active.load(Ordering::SeqCst), 0);
        assert!(acquisitions.load(Ordering::Relaxed) > 0);
    }

    #[test]
    fn lease_handoff_identity_is_exact() {
        let supervisor = MutationSupervisor::new();
        let lease = supervisor
            .try_acquire(MutationWorkload::DriverInstall, "plan-a", "owner-a")
            .unwrap();
        assert!(lease.matches(MutationWorkload::DriverInstall, "plan-a", "owner-a"));
        assert!(!lease.matches(MutationWorkload::Cleanup, "plan-a", "owner-a"));
        assert!(!lease.matches(MutationWorkload::DriverInstall, "plan-b", "owner-a"));
        assert!(!lease.matches(MutationWorkload::DriverInstall, "plan-a", "owner-b"));
    }

    #[test]
    fn release_is_observable_and_raii_backed() {
        let changes = Arc::new(Mutex::new(Vec::new()));
        let sink = changes.clone();
        let supervisor = MutationSupervisor::with_observer(Arc::new(move |change| {
            sink.lock().unwrap().push(change);
        }));
        let lease = supervisor
            .try_acquire(MutationWorkload::SystemRepair, "p1", "owner")
            .unwrap();
        drop(lease);
        let values = changes.lock().unwrap();
        assert!(matches!(values.first(), Some(MutationLeaseChange::Acquired(_))));
        assert!(matches!(values.get(1), Some(MutationLeaseChange::Released(_))));
    }
}
