#![forbid(unsafe_code)]

mod authorization;
mod cancellation;
mod event_bus;
mod mutation;
mod recovery;
mod state_machine;
mod telemetry;
mod work_budget;

use std::sync::Arc;

use aethercore_contracts::v1::{
    self, EventKind, MutationLeaseState, MutationWorkloadKind, event_envelope,
};
use aethercore_operation_engine::OperationEngine;
use aethercore_persistence::Database;
use chrono::Utc;

pub use authorization::AuthorizationManager;
pub use cancellation::{
    CancellationError, CancellationRegistry, CancellationToken, RequestContext, RequestContextError,
};
pub use event_bus::{
    EventBus, EventBusMetrics, EventSubscription, OwnerEventBusMetrics, PublishedEvent,
    ReplayBatch, SubscriptionItem,
};
pub use mutation::{
    MutationError, MutationLease, MutationLeaseChange, MutationLeaseSnapshot, MutationSupervisor,
    MutationWorkload,
};
pub use recovery::{RecoveryError, RecoverySupervisor};
pub use state_machine::StateMachine;
pub use telemetry::{ProgressTelemetry, ProgressTelemetryStore};
pub use work_budget::{ReadBudgetError, ReadBudgetLease, ReadBudgetManager, ReadWorkload};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SchedulerCadenceState {
    pub failure_count: u32,
    pub next_eligible_unix_ms: i64,
    pub last_completed_unix_ms: Option<i64>,
}

/// The Phase 10 in-process control plane. Domain engines remain specialized and retain their
/// durable domain transition logic in OperationEngine, while service-level authorization, request
/// control, live events, resource budgets, recovery and machine mutation ownership are composed
/// through one kernel root.
#[derive(Clone)]
pub struct OperationKernel {
    engine: Arc<OperationEngine>,
    database: Arc<Database>,
    authorization: AuthorizationManager,
    state_machine: StateMachine,
    mutations: MutationSupervisor,
    reads: ReadBudgetManager,
    events: EventBus,
    cancellations: CancellationRegistry,
    recovery: RecoverySupervisor,
    telemetry: ProgressTelemetryStore,
}

impl OperationKernel {
    pub fn new(engine: Arc<OperationEngine>, database: Arc<Database>) -> Self {
        let events = EventBus::new(512);

        let mutation_events = events.clone();
        let mutations = MutationSupervisor::with_observer(Arc::new(move |change| {
            let (snapshot, state) = match change {
                MutationLeaseChange::Acquired(snapshot) => (snapshot, MutationLeaseState::Acquired),
                MutationLeaseChange::Released(snapshot) => (snapshot, MutationLeaseState::Released),
            };
            let workload = match snapshot.workload {
                MutationWorkload::DriverInstall => MutationWorkloadKind::DriverInstall,
                MutationWorkload::SystemRepair => MutationWorkloadKind::SystemRepair,
                MutationWorkload::Cleanup => MutationWorkloadKind::Cleanup,
                MutationWorkload::Startup => MutationWorkloadKind::Startup,
                MutationWorkload::Update => MutationWorkloadKind::Update,
                // Phase 20 workload reuses the Update wire tag: the enum is protocol-frozen and
                // the renderer treats any lease as machine-busy identically.
                MutationWorkload::Optimization => MutationWorkloadKind::Update,
                // Phase 22 workload gets its own wire value (additive; see events.proto).
                MutationWorkload::OneClickCare => MutationWorkloadKind::OneClickCare,
            };
            // Distinct clones: publish borrows these for routing while the event moves the
            // originals out of the snapshot, so reference and moved values never alias.
            let owner_key = snapshot.owner_principal_key.clone();
            let plan_ref = snapshot.plan_id.clone();
            let mutation_event = v1::MutationLeaseEvent {
                lease_id: snapshot.lease_id,
                workload: workload as i32,
                plan_id: snapshot.plan_id,
                state: state as i32,
                acquired_unix_ms: snapshot.acquired_unix_ms,
                changed_unix_ms: Utc::now().timestamp_millis(),
            };
            mutation_events.publish(
                &owner_key,
                EventKind::MutationLease,
                &plan_ref,
                Some(event_envelope::Payload::MutationLease(mutation_event)),
            );
        }));

        let telemetry_events = events.clone();
        let telemetry = ProgressTelemetryStore::with_observer(Arc::new(move |value| {
            if value.owner_principal_key.is_empty() {
                return;
            }
            // Distinct clones keep the routing borrow separate from the moved payload fields.
            let owner_key = value.owner_principal_key.clone();
            let plan_ref = value.plan_id.clone();
            let progress_event = v1::ProgressTelemetryEvent {
                plan_id: value.plan_id,
                stage: value.stage,
                progress_known: value.progress_known,
                overall_percent: value.overall_percent,
                current_item_id: value.current_item_id,
                detail: value.detail,
                bytes_completed: value.bytes_completed,
                bytes_total: value.bytes_total,
            };
            telemetry_events.publish(
                &owner_key,
                EventKind::ProgressTelemetry,
                &plan_ref,
                Some(event_envelope::Payload::ProgressTelemetry(progress_event)),
            );
        }));

        Self {
            authorization: AuthorizationManager::new(engine.clone()),
            state_machine: StateMachine::new(engine.clone()),
            mutations,
            reads: ReadBudgetManager::new(4),
            events,
            cancellations: CancellationRegistry::default(),
            recovery: RecoverySupervisor::new(),
            telemetry,
            engine,
            database,
        }
    }

    pub fn engine(&self) -> Arc<OperationEngine> {
        self.engine.clone()
    }
    pub fn database(&self) -> Arc<Database> {
        self.database.clone()
    }
    pub fn authorization(&self) -> &AuthorizationManager {
        &self.authorization
    }
    pub fn state_machine(&self) -> &StateMachine {
        &self.state_machine
    }
    pub fn mutations(&self) -> &MutationSupervisor {
        &self.mutations
    }
    pub fn reads(&self) -> &ReadBudgetManager {
        &self.reads
    }
    pub fn events(&self) -> &EventBus {
        &self.events
    }
    pub fn cancellations(&self) -> &CancellationRegistry {
        &self.cancellations
    }
    pub fn recovery(&self) -> &RecoverySupervisor {
        &self.recovery
    }
    pub fn telemetry(&self) -> &ProgressTelemetryStore {
        &self.telemetry
    }
    pub fn scheduler_cadence(
        &self,
        owner_principal_key: &str,
        workload: &str,
    ) -> Result<Option<SchedulerCadenceState>, String> {
        self.database
            .scheduler_run(owner_principal_key, workload)
            .map(|value| {
                value.map(|record| SchedulerCadenceState {
                    failure_count: record.failure_count,
                    next_eligible_unix_ms: record.next_eligible_unix_ms,
                    last_completed_unix_ms: record.last_completed_unix_ms,
                })
            })
            .map_err(|error| error.to_string())
    }

    pub fn save_scheduler_cadence(
        &self,
        owner_principal_key: &str,
        workload: &str,
        failure_count: u32,
        next_eligible_unix_ms: i64,
        last_outcome: &str,
        last_completed_unix_ms: Option<i64>,
    ) -> Result<(), String> {
        if owner_principal_key.trim().is_empty() || workload.trim().is_empty() {
            return Err("scheduler cadence requires a principal and workload".into());
        }
        let record = aethercore_persistence::SchedulerRunRecord {
            owner_principal_key: owner_principal_key.to_owned(),
            workload: workload.to_owned(),
            failure_count,
            next_eligible_unix_ms,
            last_outcome: last_outcome.to_owned(),
            last_completed_unix_ms,
            updated_unix_ms: Utc::now().timestamp_millis(),
        };
        self.database
            .upsert_scheduler_run(&record)
            .map_err(|error| error.to_string())
    }
}
