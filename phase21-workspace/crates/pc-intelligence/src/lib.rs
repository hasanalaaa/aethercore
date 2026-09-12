#![forbid(unsafe_code)]

mod coordinator;
mod fingerprint;
mod lifecycle;
mod model;
mod normalize;
mod rules;
mod run_ownership;
mod sources;

pub use coordinator::{DeepScanCoordinator, IntelligenceError};
pub use fingerprint::machine_state_fingerprint;
pub use model::*;
pub use rules::{RULES, RuleDescriptor, evaluate as evaluate_rules, remediation_candidates};
pub use sources::{DeepScanBackend, ExistingSubsystemBackend, SourceError};
