use std::{collections::HashMap, sync::{Arc, Mutex}};
use thiserror::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ReadWorkload { DriverDiscovery, RepairAssessment, CleanupDiscovery, StartupDiscovery, Diagnostics }
impl ReadWorkload { pub fn as_str(self) -> &'static str { match self { Self::DriverDiscovery=>"DriverDiscovery",Self::RepairAssessment=>"RepairAssessment",Self::CleanupDiscovery=>"CleanupDiscovery",Self::StartupDiscovery=>"StartupDiscovery",Self::Diagnostics=>"Diagnostics" } } }

#[derive(Default)]
struct State { total: usize, by_kind: HashMap<ReadWorkload, usize> }
#[derive(Clone)]
pub struct ReadBudgetManager { max_total: usize, state: Arc<Mutex<State>> }
pub struct ReadBudgetLease { manager: ReadBudgetManager, kind: ReadWorkload }
impl ReadBudgetLease { pub fn matches(&self, kind: ReadWorkload) -> bool { self.kind == kind } }
#[derive(Debug, Error)] pub enum ReadBudgetError { #[error("read-only resource budget exhausted")] Exhausted }

impl ReadBudgetManager {
    pub fn new(max_total: usize) -> Self { Self { max_total: max_total.max(1), state: Arc::new(Mutex::new(State::default())) } }
    pub fn try_acquire(&self, kind: ReadWorkload) -> Result<ReadBudgetLease, ReadBudgetError> {
        let mut s=self.state.lock().unwrap_or_else(|p|p.into_inner());
        // One scan of each expensive kind at a time and a small global cap. This permits useful
        // concurrency without allowing WUA/WMI/PnP/filesystem discovery to stampede the machine.
        if s.total>=self.max_total || s.by_kind.get(&kind).copied().unwrap_or(0)>=1 { return Err(ReadBudgetError::Exhausted); }
        s.total+=1; *s.by_kind.entry(kind).or_default()+=1;
        Ok(ReadBudgetLease{manager:self.clone(),kind})
    }
    pub fn active_total(&self)->usize{self.state.lock().unwrap_or_else(|p|p.into_inner()).total}
}
impl Drop for ReadBudgetLease { fn drop(&mut self){let mut s=self.manager.state.lock().unwrap_or_else(|p|p.into_inner());s.total=s.total.saturating_sub(1);if let Some(v)=s.by_kind.get_mut(&self.kind){*v=v.saturating_sub(1);}} }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_only_workloads_can_overlap_within_global_budget() {
        let budget = ReadBudgetManager::new(2);
        let drivers = budget.try_acquire(ReadWorkload::DriverDiscovery).unwrap();
        let diagnostics = budget.try_acquire(ReadWorkload::Diagnostics).unwrap();
        assert_eq!(budget.active_total(), 2);
        assert!(budget.try_acquire(ReadWorkload::CleanupDiscovery).is_err());
        drop(drivers);
        assert!(budget.try_acquire(ReadWorkload::CleanupDiscovery).is_ok());
        drop(diagnostics);
    }

    #[test]
    fn read_budget_lease_identity_is_exact() {
        let budget = ReadBudgetManager::new(2);
        let lease = budget.try_acquire(ReadWorkload::Diagnostics).unwrap();
        assert!(lease.matches(ReadWorkload::Diagnostics));
        assert!(!lease.matches(ReadWorkload::DriverDiscovery));
    }

    #[test]
    fn poisoned_budget_mutex_recovers_and_preserves_capacity() {
        let budget = ReadBudgetManager::new(2);
        let poison = budget.clone();
        assert!(std::thread::spawn(move || {
            let _guard = poison.state.lock().unwrap();
            panic!("intentional mutex poison for deterministic recovery test");
        }).join().is_err());
        let first = budget.try_acquire(ReadWorkload::Diagnostics).unwrap();
        let second = budget.try_acquire(ReadWorkload::DriverDiscovery).unwrap();
        assert!(budget.try_acquire(ReadWorkload::StartupDiscovery).is_err());
        drop(first);
        assert!(budget.try_acquire(ReadWorkload::StartupDiscovery).is_ok());
        drop(second);
    }

    #[test]
    fn same_expensive_read_workload_is_single_flight() {
        let budget = ReadBudgetManager::new(4);
        let lease = budget.try_acquire(ReadWorkload::RepairAssessment).unwrap();
        assert!(budget.try_acquire(ReadWorkload::RepairAssessment).is_err());
        drop(lease);
        assert!(budget.try_acquire(ReadWorkload::RepairAssessment).is_ok());
    }
}
