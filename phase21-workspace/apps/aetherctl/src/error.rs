//! Typed CLI errors. Every failure carries a closed-vocabulary `kind` (rendered in the
//! JSON envelope) and a stable `message_key`; refused operations always print WHY.

use crate::exit::ExitCode;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliError {
    Usage {
        message_key: String,
        detail: Option<String>,
    },
    ServiceUnreachable {
        message_key: String,
    },
    Timeout,
    Rejected {
        message_key: String,
        detail: Option<String>,
    },
    ConsentRequired {
        message_key: String,
    },
    CapabilityUnavailable {
        reason_key: String,
    },
    LocalIo {
        message_key: String,
        detail: Option<String>,
    },
    /// The wire answered with something the v7 session contract forbids.
    ProtocolViolation {
        detail: String,
    },
}

impl CliError {
    pub fn usage(message_key: &str) -> Self {
        CliError::Usage {
            message_key: message_key.to_string(),
            detail: None,
        }
    }

    pub fn service_unreachable(message_key: &str) -> Self {
        CliError::ServiceUnreachable {
            message_key: message_key.to_string(),
        }
    }

    pub fn capability_unavailable(reason_key: &str) -> Self {
        CliError::CapabilityUnavailable {
            reason_key: reason_key.to_string(),
        }
    }

    pub fn local_io(message_key: &str) -> Self {
        CliError::LocalIo {
            message_key: message_key.to_string(),
            detail: None,
        }
    }

    pub fn local_io_with(message_key: &str, detail: impl Into<String>) -> Self {
        CliError::LocalIo {
            message_key: message_key.to_string(),
            detail: Some(detail.into()),
        }
    }

    /// The single mapping every exit path funnels through.
    pub fn exit_code(&self) -> ExitCode {
        match self {
            CliError::Usage { .. } => ExitCode::Usage,
            CliError::ServiceUnreachable { .. } => ExitCode::ServiceUnreachable,
            CliError::Timeout => ExitCode::Timeout,
            CliError::Rejected { .. } => ExitCode::RejectedByService,
            CliError::ConsentRequired { .. } => ExitCode::ConsentRequired,
            CliError::CapabilityUnavailable { .. } => ExitCode::CapabilityUnavailable,
            CliError::LocalIo { .. } => ExitCode::LocalIo,
            CliError::ProtocolViolation { .. } => ExitCode::LocalIo,
        }
    }

    /// Closed vocabulary rendered into the envelope's error.kind.
    pub fn kind(&self) -> &'static str {
        match self {
            CliError::Usage { .. } => "Usage",
            CliError::ServiceUnreachable { .. } => "ServiceUnreachable",
            CliError::Timeout => "Timeout",
            CliError::Rejected { .. } => "RejectedByService",
            CliError::ConsentRequired { .. } => "ConsentRequired",
            CliError::CapabilityUnavailable { .. } => "CapabilityUnavailable",
            CliError::LocalIo { .. } => "LocalIo",
            CliError::ProtocolViolation { .. } => "ProtocolViolation",
        }
    }

    pub fn message_key(&self) -> String {
        match self {
            CliError::Usage { message_key, .. }
            | CliError::ServiceUnreachable { message_key }
            | CliError::Rejected { message_key, .. }
            | CliError::ConsentRequired { message_key }
            | CliError::LocalIo { message_key, .. } => message_key.clone(),
            CliError::Timeout => "cli.timeout.elapsed".to_string(),
            CliError::CapabilityUnavailable { reason_key } => {
                format!("cli.capability.{reason_key}")
            }
            CliError::ProtocolViolation { .. } => "cli.protocol.violation".to_string(),
        }
    }

    pub fn usage_detail(&self) -> Option<String> {
        match self {
            CliError::Usage { detail, .. } => detail.clone(),
            _ => None,
        }
    }

    /// Human-readable one-liner for text mode stderr.
    pub fn text_reason(&self) -> String {
        match self {
            CliError::Usage { detail, .. } => detail
                .clone()
                .unwrap_or_else(|| "invalid command line".to_string()),
            CliError::ServiceUnreachable { message_key } => {
                format!("maintenance service not reachable ({message_key})")
            }
            CliError::Timeout => "--timeout-ms elapsed before the service answered".to_string(),
            CliError::Rejected {
                message_key,
                detail,
            } => match detail {
                Some(detail) => format!("rejected by service ({message_key}): {detail}"),
                None => format!("rejected by service ({message_key})"),
            },
            CliError::ConsentRequired { message_key } => {
                format!("consent required or refused ({message_key})")
            }
            CliError::CapabilityUnavailable { reason_key } => {
                format!("capability not available ({reason_key})")
            }
            CliError::LocalIo { detail, .. } => detail
                .clone()
                .unwrap_or_else(|| "local i/o failed".to_string()),
            CliError::ProtocolViolation { detail } => format!("protocol violation: {detail}"),
        }
    }
}
