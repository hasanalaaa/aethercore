use aethercore_operation_engine::{ConsentIntentView, OperationEngine, Result};
use std::sync::Arc;

#[derive(Clone)]
pub struct AuthorizationManager {
    engine: Arc<OperationEngine>,
}
impl AuthorizationManager {
    pub fn new(engine: Arc<OperationEngine>) -> Self {
        Self { engine }
    }
    pub fn begin(&self, plan_id: &str, owner: &str) -> Result<ConsentIntentView> {
        self.engine.begin_consent_intent(plan_id, owner)
    }
    pub fn for_broker(&self, intent_id: &str, owner: &str) -> Result<ConsentIntentView> {
        self.engine.consent_intent_for_broker(intent_id, owner)
    }
    pub fn approve(
        &self,
        intent_id: &str,
        owner: &str,
        broker_pid: u32,
    ) -> Result<(ConsentIntentView, i64)> {
        self.engine
            .approve_consent_intent(intent_id, owner, broker_pid)
    }
}
