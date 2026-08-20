use std::sync::Arc;

use aethercore_operation_engine::{OperationEngine, PlanState, PlanView, Result};

/// Owner-scoped facade over the durable operation state machine.
///
/// Phase 10 deliberately does not expose an unowned transition helper: callers must prove the
/// authenticated principal can resolve the plan before a durable transition is attempted.
#[derive(Clone)]
pub struct StateMachine {
    engine: Arc<OperationEngine>,
}

impl StateMachine {
    pub fn new(engine: Arc<OperationEngine>) -> Self {
        Self { engine }
    }

    pub fn plan_for_owner(&self, id: &str, owner: &str) -> Result<PlanView> {
        self.engine.get_plan_for_owner(id, owner)
    }

    pub fn transition_for_owner(
        &self,
        id: &str,
        owner: &str,
        from: PlanState,
        to: PlanState,
        detail: &str,
    ) -> Result<PlanView> {
        self.engine.get_plan_for_owner(id, owner)?;
        self.engine.transition(id, from, to, detail)
    }

    pub fn latest_for_owner(&self, owner: &str) -> Result<Option<PlanView>> {
        self.engine.latest_plan_for_owner(owner)
    }
}
