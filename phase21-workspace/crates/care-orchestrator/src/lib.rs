//! Phase 22 — One-Click Care orchestration.
//!
//! Deterministic sequencing of EXISTING typed domain plans behind their own safety
//! levels. The orchestrator creates no new mutation semantics: it acquires the same
//! machine-wide mutation lease every domain uses, journals every transition for
//! crash-safe resume, and reports only what each domain verified about itself.

pub mod engine;
pub mod model;

pub use engine::{
    CareJournal, CareRunResult, DomainDispatch, DomainStepExecutor, MutationLeaseGuard,
    run_care_plan,
};
pub use model::{
    CareError, CarePlan, CareSafety, CareStep, CareStepReport, MAX_CARE_STEPS, StepOutcome,
};

/// Re-exported for service-layer convenience.
pub use aethercore_operation_kernel as operation_kernel;
