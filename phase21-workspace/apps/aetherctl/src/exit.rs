//! Exit-code registry — single source of truth for Phase 28 (docs/phase28/EXIT_CODES.md
//! mirrors this enum; scripts/phase28-adversarial-audit.py asserts tri-equality between
//! this file, that document, and the test pins below).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitCode {
    /// Every requested operation completed; the envelope (JSON) or table (text) is valid.
    Ok = 0,
    /// Argument parsing failed, unknown flag/subcommand, malformed value.
    Usage = 2,
    /// The maintenance-service endpoint is not usable for a service-backed command.
    ServiceUnreachable = 3,
    /// A deadline (--timeout-ms) elapsed before the service answered.
    Timeout = 4,
    /// The service answered with a typed rejection (non-zero status_code).
    RejectedByService = 5,
    /// Session consent is required, was refused, or the digest confirmation mismatched.
    ConsentRequired = 6,
    /// A capability is honestly NotAvailable in this build/platform.
    CapabilityUnavailable = 7,
    /// Local I/O or output failure (manifest missing, unreadable file, stdout broken).
    LocalIo = 8,
    /// SIGINT delivered; default OS disposition terminates the process (shell reports 130).
    /// Registry-completeness variant: never constructed in-process by design.
    #[allow(dead_code)]
    Sigint = 130,
}

impl ExitCode {
    pub fn as_i32(self) -> i32 {
        self as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pins every registry code so docs and enum cannot drift apart silently.
    #[test]
    fn exit_code_registry_matches_docs_phase28() {
        assert_eq!(ExitCode::Ok.as_i32(), 0);
        assert_eq!(ExitCode::Usage.as_i32(), 2);
        assert_eq!(ExitCode::ServiceUnreachable.as_i32(), 3);
        assert_eq!(ExitCode::Timeout.as_i32(), 4);
        assert_eq!(ExitCode::RejectedByService.as_i32(), 5);
        assert_eq!(ExitCode::ConsentRequired.as_i32(), 6);
        assert_eq!(ExitCode::CapabilityUnavailable.as_i32(), 7);
        assert_eq!(ExitCode::LocalIo.as_i32(), 8);
        assert_eq!(ExitCode::Sigint.as_i32(), 130);
    }

    /// Trigger-path pinning: each code maps from exactly one error kind family.
    #[test]
    fn trigger_paths_map_one_to_one() {
        use crate::error::CliError;
        assert_eq!(
            CliError::usage("cli.usage.test").exit_code(),
            ExitCode::Usage
        );
        assert_eq!(
            CliError::service_unreachable("cli.detect.offline").exit_code(),
            ExitCode::ServiceUnreachable
        );
        assert_eq!(CliError::Timeout.exit_code(), ExitCode::Timeout);
        assert_eq!(
            CliError::Rejected {
                message_key: "perf.error.executionRequiresConsent".into(),
                detail: None,
            }
            .exit_code(),
            ExitCode::RejectedByService
        );
        assert_eq!(
            CliError::ConsentRequired {
                message_key: "cli.consent.required".into()
            }
            .exit_code(),
            ExitCode::ConsentRequired
        );
        assert_eq!(
            CliError::capability_unavailable("cap.reason.notCompiled").exit_code(),
            ExitCode::CapabilityUnavailable
        );
        assert_eq!(
            CliError::local_io("cli.selfCheck.manifestMissing").exit_code(),
            ExitCode::LocalIo
        );
    }
}
