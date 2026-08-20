use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use chrono::Utc;

type Observer = Arc<dyn Fn(ProgressTelemetry) + Send + Sync + 'static>;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProgressTelemetry {
    pub owner_principal_key: String,
    pub plan_id: String,
    pub stage: String,
    pub progress_known: bool,
    pub overall_percent: u32,
    pub current_item_id: String,
    pub detail: String,
    pub bytes_completed: u64,
    pub bytes_total: u64,
    pub emitted_unix_ms: i64,
}

#[derive(Clone)]
pub struct ProgressTelemetryStore {
    inner: Arc<Mutex<HashMap<(String, String), ProgressTelemetry>>>,
    observer: Option<Observer>,
}

impl Default for ProgressTelemetryStore {
    fn default() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
            observer: None,
        }
    }
}

impl ProgressTelemetryStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_observer(observer: Observer) -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
            observer: Some(observer),
        }
    }

    pub fn publish(&self, mut value: ProgressTelemetry) {
        if value.emitted_unix_ms == 0 {
            value.emitted_unix_ms = Utc::now().timestamp_millis();
        }
        self.inner
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert((value.owner_principal_key.clone(), value.plan_id.clone()), value.clone());
        if let Some(observer) = self.observer.as_ref() {
            observer(value);
        }
    }

    pub fn get_for_owner(
        &self,
        owner_principal_key: &str,
        plan_id: &str,
    ) -> Option<ProgressTelemetry> {
        self.inner
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(&(owner_principal_key.to_owned(), plan_id.to_owned()))
            .cloned()
    }

    pub fn clear_for_owner(&self, owner_principal_key: &str, plan_id: &str) {
        self.inner
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(&(owner_principal_key.to_owned(), plan_id.to_owned()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observer_receives_transient_progress_without_a_database() {
        let observed = Arc::new(Mutex::new(Vec::new()));
        let sink = observed.clone();
        let store = ProgressTelemetryStore::with_observer(Arc::new(move |value| {
            sink.lock().unwrap().push(value);
        }));
        store.publish(ProgressTelemetry {
            owner_principal_key: "owner-a".into(),
            plan_id: "plan-a".into(),
            stage: "Downloading".into(),
            overall_percent: 37,
            ..Default::default()
        });
        assert_eq!(
            store.get_for_owner("owner-a", "plan-a").unwrap().overall_percent,
            37
        );
        assert_eq!(observed.lock().unwrap().len(), 1);
    }

    #[test]
    fn transient_telemetry_is_scoped_by_owner_even_for_the_same_plan_id() {
        let store = ProgressTelemetryStore::new();
        store.publish(ProgressTelemetry {
            owner_principal_key: "owner-a".into(),
            plan_id: "same-plan".into(),
            detail: "a".into(),
            ..Default::default()
        });
        store.publish(ProgressTelemetry {
            owner_principal_key: "owner-b".into(),
            plan_id: "same-plan".into(),
            detail: "b".into(),
            ..Default::default()
        });
        assert_eq!(store.get_for_owner("owner-a", "same-plan").unwrap().detail, "a");
        assert_eq!(store.get_for_owner("owner-b", "same-plan").unwrap().detail, "b");
        store.clear_for_owner("owner-a", "same-plan");
        assert!(store.get_for_owner("owner-a", "same-plan").is_none());
        assert_eq!(store.get_for_owner("owner-b", "same-plan").unwrap().detail, "b");
    }
}
