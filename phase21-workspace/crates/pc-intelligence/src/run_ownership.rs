use aethercore_collector_runtime::CancellationToken;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RunIdentity {
    pub scan_id: String,
    pub generation: u64,
    pub started_unix_ms: i64,
}

#[derive(Clone)]
struct ActiveRun {
    identity: RunIdentity,
    token: CancellationToken,
}

#[derive(Default)]
pub(crate) struct RunOwnership {
    next_generation: u64,
    active: Option<ActiveRun>,
}

impl RunOwnership {
    pub fn install(
        &mut self,
        scan_id: String,
        started_unix_ms: i64,
        token: CancellationToken,
    ) -> RunIdentity {
        self.next_generation = self.next_generation.saturating_add(1);
        let identity = RunIdentity {
            scan_id,
            generation: self.next_generation,
            started_unix_ms,
        };
        self.active = Some(ActiveRun {
            identity: identity.clone(),
            token,
        });
        identity
    }

    pub fn owns(&self, identity: &RunIdentity) -> bool {
        self.active
            .as_ref()
            .is_some_and(|run| run.identity == *identity)
    }

    pub fn cancel_if_current(&self, scan_id: &str) -> bool {
        let Some(run) = self.active.as_ref() else {
            return false;
        };
        if run.identity.scan_id != scan_id {
            return false;
        }
        run.token.cancel();
        true
    }

    pub fn clear_if_owner(&mut self, identity: &RunIdentity) -> bool {
        if !self.owns(identity) {
            return false;
        }
        self.active = None;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rapid_restart_old_cleanup_cannot_clear_new_generation() {
        let mut ownership = RunOwnership::default();
        let a = ownership.install("scan-a".into(), 1, CancellationToken::new());
        let b = ownership.install("scan-b".into(), 2, CancellationToken::new());
        assert!(!ownership.clear_if_owner(&a));
        assert!(ownership.owns(&b));
    }

    #[test]
    fn stale_cancel_cannot_cancel_new_generation() {
        let mut ownership = RunOwnership::default();
        let _a = ownership.install("scan-a".into(), 1, CancellationToken::new());
        let b_token = CancellationToken::new();
        let b = ownership.install("scan-b".into(), 2, b_token.clone());
        assert!(!ownership.cancel_if_current("scan-a"));
        assert!(!b_token.is_cancelled());
        assert!(ownership.owns(&b));
    }

    #[test]
    fn cancel_always_targets_current_authoritative_generation() {
        let mut ownership = RunOwnership::default();
        let token = CancellationToken::new();
        let current = ownership.install("scan-current".into(), 1, token.clone());
        assert!(ownership.cancel_if_current("scan-current"));
        assert!(token.is_cancelled());
        assert!(ownership.owns(&current));
    }

    #[test]
    fn duplicate_terminal_cleanup_is_idempotent() {
        let mut ownership = RunOwnership::default();
        let a = ownership.install("scan-a".into(), 1, CancellationToken::new());
        assert!(ownership.clear_if_owner(&a));
        assert!(!ownership.clear_if_owner(&a));
    }

    #[test]
    fn late_worker_identity_cannot_mutate_new_generation() {
        let mut ownership = RunOwnership::default();
        let old = ownership.install("scan-old".into(), 1, CancellationToken::new());
        let new = ownership.install("scan-new".into(), 2, CancellationToken::new());
        assert!(!ownership.owns(&old));
        assert!(ownership.owns(&new));
    }
}
