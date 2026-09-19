#![deny(unsafe_op_in_unsafe_fn)]

mod model;
mod policy;
mod resource;
mod runtime;
#[cfg(windows)]
mod windows_state;

pub use model::*;
pub use policy::{EligibilityEngine, WorkloadPolicy};
pub use resource::{Cancelled, ResourceGovernor};
pub use runtime::{
    IdleScheduler, PassiveWorkExecutor, SchedulerHandle, SchedulerStartError, SystemStateProbe,
};
#[cfg(windows)]
pub use windows_state::WindowsSystemStateProbe;

#[cfg(windows)]
pub(crate) fn run_in_background_mode<T>(
    f: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    windows_state::run_in_background_mode(f)
}

#[cfg(not(windows))]
pub(crate) fn run_in_background_mode<T>(
    f: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    f()
}
