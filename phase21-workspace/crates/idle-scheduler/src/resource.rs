use std::{
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use aethercore_collector_runtime::CancellationToken;

/// Cooperative, process-local background governor. Autonomous work is single-flight; callers use
/// these checkpoints around scan phases and byte-heavy loops. The governor never raises process
/// priority or holds execution-state requests that would fight Windows power policy.
#[derive(Clone)]
pub struct ResourceGovernor {
    inner: Arc<Mutex<State>>,
    cpu_budget_per_second: Duration,
    io_budget_per_second: u64,
}

struct State {
    window: Instant,
    cpu_used: Duration,
    io_used: u64,
}

impl Default for ResourceGovernor {
    fn default() -> Self { Self::new(Duration::from_millis(50), 4 * 1024 * 1024) }
}

impl ResourceGovernor {
    pub fn new(cpu_budget_per_second: Duration, io_budget_per_second: u64) -> Self {
        Self {
            inner: Arc::new(Mutex::new(State {
                window: Instant::now(),
                cpu_used: Duration::ZERO,
                io_used: 0,
            })),
            cpu_budget_per_second,
            io_budget_per_second: io_budget_per_second.max(1),
        }
    }

    fn roll(state: &mut State) {
        if state.window.elapsed() >= Duration::from_secs(1) {
            state.window = Instant::now();
            state.cpu_used = Duration::ZERO;
            state.io_used = 0;
        }
    }

    pub fn account_cpu(&self, amount: Duration) {
        let token = CancellationToken::new();
        let _ = self.account_cpu_cancellable(&token, amount);
    }

    pub fn account_cpu_cancellable(&self, token: &CancellationToken, amount: Duration) -> Result<(), ()> {
        loop {
            if token.is_cancelled() { return Err(()); }
            let sleep_for = {
                let mut state = self.inner.lock().unwrap_or_else(|p| p.into_inner());
                Self::roll(&mut state);
                if state.cpu_used + amount <= self.cpu_budget_per_second {
                    state.cpu_used += amount;
                    None
                } else {
                    Some(Duration::from_secs(1).saturating_sub(state.window.elapsed()))
                }
            };
            match sleep_for {
                Some(value) if !value.is_zero() => sleep_cancellable(token, value.min(Duration::from_millis(50)))?,
                Some(_) => thread::yield_now(),
                None => return Ok(()),
            }
        }
    }

    pub fn account_io(&self, bytes: u64) {
        let token = CancellationToken::new();
        let _ = self.account_io_cancellable(&token, bytes);
    }

    pub fn account_io_cancellable(&self, token: &CancellationToken, bytes: u64) -> Result<(), ()> {
        let mut remaining = bytes;
        while remaining > 0 {
            if token.is_cancelled() { return Err(()); }
            let sleep_for = {
                let mut state = self.inner.lock().unwrap_or_else(|p| p.into_inner());
                Self::roll(&mut state);
                let available = self.io_budget_per_second.saturating_sub(state.io_used);
                if available > 0 {
                    let take = remaining.min(available);
                    state.io_used += take;
                    remaining -= take;
                    None
                } else {
                    Some(Duration::from_secs(1).saturating_sub(state.window.elapsed()))
                }
            };
            if let Some(value) = sleep_for {
                sleep_cancellable(token, value.min(Duration::from_millis(50)))?;
            }
        }
        Ok(())
    }

    pub const fn cpu_budget_per_second(&self) -> Duration { self.cpu_budget_per_second }
    pub const fn io_budget_per_second(&self) -> u64 { self.io_budget_per_second }
}

fn sleep_cancellable(token: &CancellationToken, duration: Duration) -> Result<(), ()> {
    let start = Instant::now();
    while start.elapsed() < duration {
        if token.is_cancelled() { return Err(()); }
        thread::sleep(Duration::from_millis(10).min(duration.saturating_sub(start.elapsed())));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancellation_interrupts_governor_wait() {
        let governor = ResourceGovernor::new(Duration::ZERO, 1);
        let token = CancellationToken::new();
        token.cancel();
        assert!(governor.account_cpu_cancellable(&token, Duration::from_millis(1)).is_err());
        assert!(governor.account_io_cancellable(&token, 2).is_err());
    }

    #[test]
    fn default_background_budget_is_deliberately_small() {
        let governor = ResourceGovernor::default();
        assert_eq!(governor.cpu_budget_per_second(), Duration::from_millis(50));
        assert_eq!(governor.io_budget_per_second(), 4 * 1024 * 1024);
    }
}
