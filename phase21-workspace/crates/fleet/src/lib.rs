//! AetherCore Fleet — Phase 34.
//!
//! Typed fleet domain, fail-closed host-key trust, OS-OpenSSH transport,
//! bounded orchestration, and deterministic scheduling. All acceptance
//! evidence for this crate is its executable test suite plus the GD proofs
//! in `tests/`.

pub mod domain;
pub mod orchestrator;
pub mod scheduler;
pub mod scheduler_runner;
pub mod transport;
pub mod trust;

pub use domain::{
    AuthReference, FLEET_HOST_SCHEMA, FLEET_INVENTORY_SCHEMA, FLEET_SCHEDULE_SCHEMA, FleetCadence,
    FleetDomainError, FleetHost, FleetHostTrust, FleetInventory, FleetSchedule,
    FleetScheduleLastResult, MAX_FLEET_HOSTS, MIN_CADENCE_SECS,
};
pub use orchestrator::{
    ABSOLUTE_MAX_CONCURRENCY, DEFAULT_MAX_CONCURRENCY, FleetBatchResult, FleetTransport,
    HostOutcome, OrchestratorConfig, OrchestratorError, run_batch, run_batch_for_profile,
};
pub use scheduler::{
    Clock, FixedClock, OverlapLock, ScheduleError, SystemClock, begin_run, is_due, record_result,
};
pub use scheduler_runner::{
    MISSED_RUN_CATCHUP, RunnerError, ScheduleRunRecord, ScheduleRunSummary, SchedulerStore,
    resolve_schedule_scope, run_due_schedules, run_one_schedule,
};
pub use transport::{
    REMOTE_CONTRACT_VERSION, RemoteCompatibility, RemoteOperation, SpawnHook, SshTransport,
    TransportError, TrustedSshTransport, compatibility_verdict, parse_compatibility,
};
pub use trust::{
    FLEET_TRUST_SCHEMA, FORBIDDEN_SSH_OPTION_FRAGMENTS, MAX_CAPTURE_BYTES, RemoteOutcomeKind,
    RemoteOutput, RemoteResult, SUPPORTED_KEY_TYPES, TrustDecision, TrustError, TrustStore,
    TrustedHostKey, build_remote_argv, classify_ssh_exit, decide, fingerprint_of_blob,
    known_hosts_line, shell_quote, ssh_binary,
};
