use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

use chrono::Utc;
use thiserror::Error;

#[derive(Clone, Default)]
pub struct CancellationRegistry {
    inner: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
}

#[derive(Clone)]
pub struct CancellationToken {
    id: String,
    cancelled: Arc<AtomicBool>,
}

#[derive(Clone)]
pub struct RequestContext {
    pub session_id: String,
    pub request_id: String,
    pub owner_principal_key: String,
    pub deadline_unix_ms: i64,
    pub cancellation: CancellationToken,
}

#[derive(Debug, Error)]
pub enum CancellationError {
    #[error("cancellation id is already active in this session")]
    AlreadyRegistered,
}

#[derive(Debug, Error)]
pub enum RequestContextError {
    #[error("request deadline exceeded")]
    DeadlineExceeded,
    #[error("request cancelled")]
    Cancelled,
}

impl CancellationRegistry {
    pub fn try_register(&self, id: &str) -> Result<CancellationToken, CancellationError> {
        let flag = Arc::new(AtomicBool::new(false));
        let mut inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        if inner.contains_key(id) {
            return Err(CancellationError::AlreadyRegistered);
        }
        inner.insert(id.to_owned(), flag.clone());
        Ok(CancellationToken {
            id: id.to_owned(),
            cancelled: flag,
        })
    }

    pub fn cancel(&self, id: &str) -> bool {
        self.inner
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(id)
            .is_some_and(|flag| {
                flag.store(true, Ordering::Release);
                true
            })
    }

    pub fn cancel_session(&self, session_id: &str) -> usize {
        let prefix = format!("{session_id}:");
        let mut inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        let mut cancelled = 0usize;
        inner.retain(|id, flag| {
            if id.starts_with(&prefix) {
                flag.store(true, Ordering::Release);
                cancelled = cancelled.saturating_add(1);
                false
            } else {
                true
            }
        });
        cancelled
    }

    pub fn remove(&self, id: &str) {
        self.inner
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(id);
    }

    #[cfg(test)]
    fn active_count(&self) -> usize {
        self.inner.lock().unwrap_or_else(|p| p.into_inner()).len()
    }
}

impl CancellationToken {
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    pub fn id(&self) -> &str {
        &self.id
    }
}

impl RequestContext {
    pub fn checkpoint(&self) -> Result<(), RequestContextError> {
        if self.cancellation.is_cancelled() {
            return Err(RequestContextError::Cancelled);
        }
        if self.deadline_unix_ms > 0 && Utc::now().timestamp_millis() > self.deadline_unix_ms {
            return Err(RequestContextError::DeadlineExceeded);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poisoned_registry_mutex_recovers_without_rebinding_or_leaking_tokens() {
        let registry = CancellationRegistry::default();
        let poison = registry.clone();
        assert!(std::thread::spawn(move || {
            let _guard = poison.inner.lock().unwrap();
            panic!("intentional mutex poison for deterministic recovery test");
        }).join().is_err());
        let token = registry.try_register("s1:recover").unwrap();
        assert!(registry.cancel("s1:recover"));
        assert!(token.is_cancelled());
        registry.remove("s1:recover");
        assert_eq!(registry.active_count(), 0);
    }

    #[test]
    fn duplicate_cancellation_ids_are_rejected_instead_of_rebinding_tokens() {
        let registry = CancellationRegistry::default();
        let token = registry.try_register("s1:c1").unwrap();
        assert!(matches!(registry.try_register("s1:c1"), Err(CancellationError::AlreadyRegistered)));
        assert!(!token.is_cancelled());
    }

    #[test]
    fn disconnect_cancels_only_the_owning_session_requests() {
        let registry = CancellationRegistry::default();
        let a = registry.try_register("s1:a").unwrap();
        let b = registry.try_register("s1:b").unwrap();
        let other = registry.try_register("s2:a").unwrap();
        assert_eq!(registry.cancel_session("s1"), 2);
        assert!(a.is_cancelled() && b.is_cancelled());
        assert!(!other.is_cancelled());
        assert_eq!(registry.active_count(), 1);
    }
}
