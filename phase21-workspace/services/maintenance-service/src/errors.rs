use aethercore_cleaner::CleanerError;
use aethercore_contracts::v1::ErrorCode;
use aethercore_diagnostic_engine::DiagnosticError;
use aethercore_driver_hub::HubError;
use aethercore_driver_install::InstallError;
use aethercore_operation_engine::EngineError;
use aethercore_operation_kernel::{MutationError, ReadBudgetError, RequestContextError};
use aethercore_pc_intelligence::IntelligenceError;
use aethercore_startup_manager::StartupError;
use aethercore_support_bundle::SupportBundleError;
use aethercore_system_repair::RepairError;
use aethercore_update_engine::UpdateEngineError;

#[derive(Debug)]
pub(crate) struct ServiceError {
    pub status: u32,
    pub code: ErrorCode,
    pub domain: &'static str,
    pub message_key: &'static str,
    pub detail: String,
    pub retryable: bool,
}

impl ServiceError {
    pub fn new(
        status: u32,
        code: ErrorCode,
        domain: &'static str,
        message_key: &'static str,
        detail: impl Into<String>,
        retryable: bool,
    ) -> Self {
        Self {
            status,
            code,
            domain,
            message_key,
            detail: detail.into(),
            retryable,
        }
    }

    pub fn invalid(
        domain: &'static str,
        message_key: &'static str,
        detail: impl Into<String>,
    ) -> Self {
        Self::new(
            400,
            ErrorCode::InvalidRequest,
            domain,
            message_key,
            detail,
            false,
        )
    }

    pub fn internal(
        domain: &'static str,
        message_key: &'static str,
        detail: impl Into<String>,
    ) -> Self {
        Self::new(500, ErrorCode::Internal, domain, message_key, detail, false)
    }

    pub fn forbidden(
        domain: &'static str,
        message_key: &'static str,
        detail: impl Into<String>,
    ) -> Self {
        Self::new(
            403,
            ErrorCode::Forbidden,
            domain,
            message_key,
            detail,
            false,
        )
    }

    fn conflict(
        domain: &'static str,
        message_key: &'static str,
        detail: impl Into<String>,
    ) -> Self {
        Self::new(409, ErrorCode::Conflict, domain, message_key, detail, false)
    }

    fn busy(domain: &'static str, message_key: &'static str, detail: impl Into<String>) -> Self {
        Self::new(429, ErrorCode::Busy, domain, message_key, detail, true)
    }

    fn not_found(
        domain: &'static str,
        message_key: &'static str,
        detail: impl Into<String>,
    ) -> Self {
        Self::new(404, ErrorCode::NotFound, domain, message_key, detail, false)
    }
}

impl From<&str> for ServiceError {
    fn from(value: &str) -> Self {
        Self::invalid("service", "service.invalidRequest", value)
    }
}

impl From<String> for ServiceError {
    fn from(value: String) -> Self {
        Self::invalid("service", "service.invalidRequest", value)
    }
}

impl From<anyhow::Error> for ServiceError {
    fn from(value: anyhow::Error) -> Self {
        Self::internal("service", "service.internal", value.to_string())
    }
}

impl From<RequestContextError> for ServiceError {
    fn from(value: RequestContextError) -> Self {
        match value {
            RequestContextError::DeadlineExceeded => Self::new(
                408,
                ErrorCode::DeadlineExceeded,
                "ipc",
                "ipc.deadlineExceeded",
                value.to_string(),
                true,
            ),
            RequestContextError::Cancelled => Self::new(
                499,
                ErrorCode::Cancelled,
                "ipc",
                "ipc.cancelled",
                value.to_string(),
                false,
            ),
        }
    }
}

impl From<MutationError> for ServiceError {
    fn from(value: MutationError) -> Self {
        match value {
            MutationError::BusyOwned { .. } => Self::busy(
                "operation-kernel",
                "kernel.mutationBusyOwned",
                value.to_string(),
            ),
            MutationError::BusyOtherPrincipal => Self::busy(
                "operation-kernel",
                "kernel.mutationBusy",
                "machine mutation is already active",
            ),
            MutationError::InvalidIdentity => Self::internal(
                "operation-kernel",
                "kernel.invalidMutationIdentity",
                value.to_string(),
            ),
        }
    }
}

impl From<ReadBudgetError> for ServiceError {
    fn from(value: ReadBudgetError) -> Self {
        Self::busy(
            "operation-kernel",
            "kernel.readBudgetExhausted",
            value.to_string(),
        )
    }
}

impl From<EngineError> for ServiceError {
    fn from(value: EngineError) -> Self {
        let detail = value.to_string();
        match value {
            EngineError::NotFound => Self::not_found("operation-engine", "plan.notFound", detail),
            EngineError::OwnershipMismatch => {
                Self::forbidden("operation-engine", "plan.ownerMismatch", detail)
            }
            EngineError::AuthorizationRequired | EngineError::ConsentIntentInvalid => {
                Self::forbidden("authorization", "authorization.requiredOrInvalid", detail)
            }
            EngineError::InvalidState
            | EngineError::InvalidTransition(_, _)
            | EngineError::DigestMismatch
            | EngineError::IntegrityMismatch => {
                Self::conflict("operation-engine", "plan.stateConflict", detail)
            }
            EngineError::Persistence(_) | EngineError::Serialization(_) => {
                Self::internal("operation-engine", "plan.persistenceFailure", detail)
            }
            EngineError::EmptyDriverPlan
            | EngineError::WrongActionType
            | EngineError::EmptyRepairPlan
            | EngineError::EmptyCleanupPlan
            | EngineError::EmptyStartupPlan => {
                Self::invalid("operation-engine", "plan.invalidMaterial", detail)
            }
        }
    }
}

impl From<HubError> for ServiceError {
    fn from(value: HubError) -> Self {
        let detail = value.to_string();
        match value {
            HubError::Busy => Self::busy("drivers", "drivers.scanBusy", detail),
            HubError::Cancelled => Self::new(
                499,
                ErrorCode::Cancelled,
                "drivers",
                "ipc.cancelled",
                detail,
                false,
            ),
            HubError::OwnershipMismatch => Self::not_found(
                "drivers",
                "drivers.stateUnavailable",
                "driver state is unavailable",
            ),
            HubError::SnapshotNotReady | HubError::StaleSnapshot => {
                Self::conflict("drivers", "drivers.snapshotConflict", detail)
            }
            HubError::CandidateNotFound(_) => {
                Self::not_found("drivers", "drivers.candidateNotFound", detail)
            }
            HubError::CandidateNotSelectable(_) => {
                Self::forbidden("drivers", "drivers.candidateProtected", detail)
            }
            HubError::Inventory(_) => Self::internal("drivers", "drivers.inventoryFailure", detail),
            HubError::InvalidPolicy(_) | HubError::Persistence(_) => {
                Self::internal("drivers", "drivers.inventoryFailure", detail)
            }
            HubError::EmptySelection
            | HubError::TooManySelections
            | HubError::DuplicateCandidate(_) => {
                Self::invalid("drivers", "drivers.invalidSelection", detail)
            }
        }
    }
}

impl From<InstallError> for ServiceError {
    fn from(value: InstallError) -> Self {
        let detail = value.to_string();
        match value {
            InstallError::Busy => Self::busy("driver-install", "drivers.installBusy", detail),
            InstallError::AuthorizationRequired => {
                Self::forbidden("driver-install", "authorization.required", detail)
            }
            InstallError::InvalidPlan => {
                Self::invalid("driver-install", "drivers.invalidInstallPlan", detail)
            }
            InstallError::Preflight(_) => {
                Self::conflict("driver-install", "drivers.preflightRejected", detail)
            }
            InstallError::Protection(_)
            | InstallError::Execution(_)
            | InstallError::Verification(_)
            | InstallError::Database(_) => {
                Self::internal("driver-install", "drivers.installFailure", detail)
            }
            InstallError::Engine(error) => error.into(),
            InstallError::Hub(error) => error.into(),
        }
    }
}

impl From<RepairError> for ServiceError {
    fn from(value: RepairError) -> Self {
        let detail = value.to_string();
        match value {
            RepairError::Busy | RepairError::AlreadyRunning | RepairError::ServicingBusy => {
                Self::busy("repair", "repair.busy", detail)
            }
            RepairError::OwnershipMismatch => Self::not_found(
                "repair",
                "repair.stateUnavailable",
                "repair state is unavailable",
            ),
            RepairError::AuthorizationRequired => {
                Self::forbidden("repair", "authorization.required", detail)
            }
            RepairError::AssessmentNotReady
            | RepairError::StaleAssessment
            | RepairError::RebootPending => {
                Self::conflict("repair", "repair.stateConflict", detail)
            }
            RepairError::UnsupportedPlatform
            | RepairError::Command(_)
            | RepairError::Persistence(_)
            | RepairError::InvalidRepairGraph(_)
            | RepairError::NoRepairRecommended
            | RepairError::VerificationFailed
            | RepairError::RebootBoundary
            | RepairError::SourceRequired(_)
            | RepairError::RecoveryUnavailable(_) => {
                Self::internal("repair", "repair.executionFailure", detail)
            }
            RepairError::Engine(error) => error.into(),
        }
    }
}

impl From<CleanerError> for ServiceError {
    fn from(value: CleanerError) -> Self {
        let detail = value.to_string();
        match value {
            CleanerError::Busy | CleanerError::AlreadyRunning | CleanerError::MutationBusy => {
                Self::busy("cleanup", "cleanup.busy", detail)
            }
            CleanerError::Cancelled => Self::new(
                499,
                ErrorCode::Cancelled,
                "cleanup",
                "ipc.cancelled",
                detail,
                false,
            ),
            CleanerError::OwnershipMismatch => Self::not_found(
                "cleanup",
                "cleanup.stateUnavailable",
                "cleanup state is unavailable",
            ),
            CleanerError::AuthorizationRequired => {
                Self::forbidden("cleanup", "authorization.required", detail)
            }
            CleanerError::ScanNotReady | CleanerError::StaleScan | CleanerError::Safety(_) => {
                Self::conflict("cleanup", "cleanup.stateConflict", detail)
            }
            CleanerError::CandidateInvalid(_) => {
                Self::not_found("cleanup", "cleanup.candidateInvalid", detail)
            }
            CleanerError::TooManyFiles => {
                Self::invalid("cleanup", "cleanup.selectionTooLarge", detail)
            }
            CleanerError::UnsupportedPlatform
            | CleanerError::Io(_)
            | CleanerError::Persistence(_) => {
                Self::internal("cleanup", "cleanup.executionFailure", detail)
            }
            CleanerError::Engine(error) => error.into(),
        }
    }
}

impl From<StartupError> for ServiceError {
    fn from(value: StartupError) -> Self {
        let detail = value.to_string();
        match value {
            StartupError::Busy | StartupError::AlreadyRunning | StartupError::MutationBusy => {
                Self::busy("startup", "startup.busy", detail)
            }
            StartupError::Cancelled => Self::new(
                499,
                ErrorCode::Cancelled,
                "startup",
                "ipc.cancelled",
                detail,
                false,
            ),
            StartupError::OwnershipMismatch => Self::not_found(
                "startup",
                "startup.stateUnavailable",
                "startup state is unavailable",
            ),
            StartupError::AuthorizationRequired => {
                Self::forbidden("startup", "authorization.required", detail)
            }
            StartupError::ScanNotReady
            | StartupError::StaleScan
            | StartupError::ServiceConfirmationRequired
            | StartupError::Drift(_) => Self::conflict("startup", "startup.stateConflict", detail),
            StartupError::UnknownItem(_) => {
                Self::not_found("startup", "startup.itemNotFound", detail)
            }
            StartupError::Protected(_) | StartupError::NotManageable(_) => {
                Self::forbidden("startup", "startup.itemProtected", detail)
            }
            StartupError::PassiveDefault | StartupError::TooManyActions => {
                Self::invalid("startup", "startup.invalidSelection", detail)
            }
            StartupError::UnsupportedPlatform
            | StartupError::Serialization(_)
            | StartupError::Persistence(_)
            | StartupError::Platform(_) => {
                Self::internal("startup", "startup.executionFailure", detail)
            }
            StartupError::Engine(error) => error.into(),
        }
    }
}

impl From<DiagnosticError> for ServiceError {
    fn from(value: DiagnosticError) -> Self {
        let detail = value.to_string();
        match value {
            DiagnosticError::Busy => Self::busy("diagnostics", "diagnostics.busy", detail),
            DiagnosticError::Cancelled => Self::new(
                499,
                ErrorCode::Cancelled,
                "diagnostics",
                "ipc.cancelled",
                detail,
                false,
            ),
            DiagnosticError::Provider(_) => {
                Self::internal("diagnostics", "diagnostics.providerFailure", detail)
            }
            DiagnosticError::OwnershipMismatch => Self::not_found(
                "diagnostics",
                "diagnostics.stateUnavailable",
                "diagnostic state is unavailable",
            ),
            DiagnosticError::Persistence(_) => {
                Self::internal("diagnostics", "diagnostics.persistenceFailure", detail)
            }
            DiagnosticError::Internal(_) => {
                Self::internal("diagnostics", "service.internal", detail)
            }
        }
    }
}

impl From<UpdateEngineError> for ServiceError {
    fn from(value: UpdateEngineError) -> Self {
        let detail = value.to_string();
        match value {
            UpdateEngineError::Disabled => {
                Self::conflict("update", "update.error.disabled", detail)
            }
            UpdateEngineError::ResponseTooLarge
            | UpdateEngineError::SizeMismatch
            | UpdateEngineError::HashMismatch
            | UpdateEngineError::Manifest(_)
            | UpdateEngineError::Rollback => {
                Self::conflict("update", "update.error.integrity", detail)
            }
            UpdateEngineError::Trust(_) | UpdateEngineError::Authenticode(_) => {
                Self::forbidden("update", "update.error.trust", detail)
            }
            UpdateEngineError::UnknownRelease
            | UpdateEngineError::IntentUnavailable
            | UpdateEngineError::TicketUnavailable => {
                Self::not_found("update", "update.error.unavailable", detail)
            }
            UpdateEngineError::Ownership => Self::not_found(
                "update",
                "update.error.unavailable",
                "update state is unavailable",
            ),
            UpdateEngineError::InvalidState => {
                Self::conflict("update", "update.error.stateConflict", detail)
            }
            UpdateEngineError::Busy => Self::busy("update", "update.error.busy", detail),
            UpdateEngineError::Io(_)
            | UpdateEngineError::Persistence(_)
            | UpdateEngineError::UnsupportedPlatform => {
                Self::internal("update", "update.error.internal", detail)
            }
        }
    }
}
impl From<SupportBundleError> for ServiceError {
    fn from(value: SupportBundleError) -> Self {
        let detail = value.to_string();
        match value {
            SupportBundleError::PreviewUnavailable
            | SupportBundleError::BundleUnavailable
            | SupportBundleError::Ownership => Self::not_found(
                "support-bundle",
                "support.error.unavailable",
                "support bundle state is unavailable",
            ),
            SupportBundleError::SectionTooLarge
            | SupportBundleError::BundleTooLarge
            | SupportBundleError::InvalidArchive(_) => {
                Self::invalid("support-bundle", "support.error.invalid", detail)
            }
            SupportBundleError::ResourceLimit => {
                Self::busy("support-bundle", "support.error.busy", detail)
            }
            SupportBundleError::InvalidProof | SupportBundleError::HashMismatch => {
                Self::conflict("support-bundle", "support.error.integrity", detail)
            }
            SupportBundleError::Io(_) | SupportBundleError::Json(_) => {
                Self::internal("support-bundle", "support.error.internal", detail)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kernel_contention_and_ownership_failures_have_typed_semantics() {
        let foreign_busy = ServiceError::from(MutationError::BusyOtherPrincipal);
        assert_eq!(foreign_busy.status, 429);
        assert_eq!(foreign_busy.code, ErrorCode::Busy);
        assert!(foreign_busy.retryable);
        assert_eq!(foreign_busy.detail, "machine mutation is already active");

        let ownership = ServiceError::from(EngineError::OwnershipMismatch);
        assert_eq!(ownership.status, 403);
        assert_eq!(ownership.code, ErrorCode::Forbidden);
        assert!(!ownership.retryable);
    }

    #[test]
    fn deadline_and_cancellation_are_not_collapsed_into_invalid_request() {
        let deadline = ServiceError::from(RequestContextError::DeadlineExceeded);
        assert_eq!(deadline.status, 408);
        assert_eq!(deadline.code, ErrorCode::DeadlineExceeded);
        assert!(deadline.retryable);

        let cancelled = ServiceError::from(RequestContextError::Cancelled);
        assert_eq!(cancelled.status, 499);
        assert_eq!(cancelled.code, ErrorCode::Cancelled);
        assert!(!cancelled.retryable);
    }

    #[test]
    fn foreign_domain_snapshot_state_is_non_enumerable() {
        for error in [
            ServiceError::from(HubError::OwnershipMismatch),
            ServiceError::from(RepairError::OwnershipMismatch),
            ServiceError::from(CleanerError::OwnershipMismatch),
            ServiceError::from(StartupError::OwnershipMismatch),
            ServiceError::from(DiagnosticError::OwnershipMismatch),
        ] {
            assert_eq!(error.status, 404);
            assert_eq!(error.code, ErrorCode::NotFound);
            assert!(!error.detail.to_ascii_lowercase().contains("principal"));
            assert!(!error.detail.to_ascii_lowercase().contains("owner"));
        }
    }

    #[test]
    fn domain_busy_and_state_conflict_remain_distinct() {
        let busy = ServiceError::from(HubError::Busy);
        assert_eq!(busy.status, 429);
        assert_eq!(busy.code, ErrorCode::Busy);
        let stale = ServiceError::from(HubError::StaleSnapshot);
        assert_eq!(stale.status, 409);
        assert_eq!(stale.code, ErrorCode::Conflict);
    }
}

impl From<IntelligenceError> for ServiceError {
    fn from(value: IntelligenceError) -> Self {
        let detail = value.to_string();
        match value {
            IntelligenceError::Busy => Self::busy("pc-intelligence", "deepScan.busy", detail),
            IntelligenceError::Ownership | IntelligenceError::UnknownScan => Self::not_found(
                "pc-intelligence",
                "deepScan.stateUnavailable",
                "deep scan state is unavailable",
            ),
            IntelligenceError::Persistence(_) | IntelligenceError::Internal(_) => {
                Self::internal("pc-intelligence", "deepScan.failure", detail)
            }
        }
    }
}

// Phase 27: the unix build compiles router.rs without the Windows-only broker helpers,
// so this conversion must live here (the Windows path reaches it identically).
impl From<aethercore_persistence::PersistenceError> for ServiceError {
    fn from(value: aethercore_persistence::PersistenceError) -> Self {
        Self::internal(
            "persistence",
            "persistence.error.internal",
            value.to_string(),
        )
    }
}
