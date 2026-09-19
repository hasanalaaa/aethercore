//! The governed RPC surface: one dispatcher, one module per request domain.
//!
//! `DBT-P63-004`. This was a single 1,868-line file whose `handle_request` ran
//! lines 86-1753 as one `match`. The decomposition is a MOVE: every verb's body
//! is the match arm it came from, unchanged, and `dispatch.rs` holds the
//! preamble all 81 of them ran behind. `phase10-architecture-audit.ps1:155`
//! throws above 220 lines here, and `static_validate.py`'s ratchet holds it.
//!
//! The gates that assert on this router read it as a MODULE TREE - `router.rs`
//! joined with `router/*.rs` in dispatch order - because what they assert is the
//! verbs, not the file that used to hold all of them.
//!
//! This module owns the vocabulary every domain module shares; each of them
//! opens with `use super::*;` so the import block below is written once.

use std::{path::PathBuf, sync::Arc};

use aethercore_cleaner::CleanupEngine;
use aethercore_contracts::{
    MAX_REQUEST_ID_BYTES, PROTOCOL_VERSION,
    v1::{self, EventKind, Request, Response, ResponseHeader, event_envelope, request, response},
};
use aethercore_diagnostic_engine::DiagnosticEngine;
use aethercore_driver_hub::DriverHub;
use aethercore_driver_install::DriverInstallCoordinator;
use aethercore_operation_engine::OperationEngine;
use aethercore_operation_kernel::{
    MutationWorkload, OperationKernel, ReadWorkload, RequestContext,
};
use aethercore_pc_intelligence::DeepScanCoordinator;
use aethercore_persistence::Database;
use aethercore_startup_manager::{StartupDecision, StartupManager};
use aethercore_support_bundle::SupportBundleEngine;
use aethercore_system_repair::RepairCoordinator;
use aethercore_update_engine::UpdateCoordinator;
use anyhow::{Context, Result};

use crate::{errors::ServiceError, performance::PerformanceEngine, protocol::*, streaming::*};

mod assistant;
mod care;
mod cleanup;
mod consent;
mod diagnostics;
// The two legacy update arms in dispatch.rs match deprecated wire variants in
// order to reject them; dropping the arms would let them fall through. The
// allow lives here because dispatch.rs is at its 258-line P10 ceiling exactly
// (static_validate.py:1482) and its arm text is asserted verbatim by three
// validators, so neither an attribute line nor a reflow can go in that file.
#[allow(deprecated)]
mod dispatch;
mod drivers;
mod insights;
mod journal;
mod performance;
mod platform;
mod repair;
mod security_audit;
mod session;
mod startup;
mod support;
mod timeline;
mod updates;

pub use dispatch::handle_request;

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// What every verb returns: the payload to send, or nothing to send, or the
/// typed error `failure` renders. The arms returned exactly this before they
/// were functions.
type Routed = std::result::Result<Option<response::Payload>, ServiceError>;

/// The four locals `handle_request` held while the match ran. Each verb binds
/// back the ones its body names, so the body is the arm it came from.
struct Call<'a> {
    ctx: &'a ServiceContext,
    peer: &'a aethercore_security::PrincipalContext,
    request_context: &'a RequestContext,
    principal_key: String,
}

#[derive(Clone)]
pub struct ServiceContext {
    pub kernel: Arc<OperationKernel>,
    pub engine: Arc<OperationEngine>,
    pub driver_hub: Arc<DriverHub>,
    pub installer: Arc<DriverInstallCoordinator>,
    pub repair: Arc<RepairCoordinator>,
    pub cleaner: Arc<CleanupEngine>,
    pub startup: Arc<StartupManager>,
    pub diagnostics: Arc<DiagnosticEngine>,
    pub intelligence: Arc<DeepScanCoordinator>,
    pub db: Arc<Database>,
    pub updates: Arc<UpdateCoordinator>,
    pub support: Arc<SupportBundleEngine>,
    /// Phase 20: performance telemetry ring, bottleneck analysis, and optimization governance.
    pub performance: Arc<PerformanceEngine>,
    /// Phase 21: Timeline Intelligence — read-only coordinator over persisted history.
    pub timeline: Arc<crate::timeline::TimelineCoordinator>,
    /// Phase 22: One-Click Care — orchestration over existing domain plans only.
    pub care: Arc<crate::care::CareCoordinator>,
    /// Phase 23: Embedded Local Intelligence — advisory-only, ephemeral session state.
    pub intelligence_core: Arc<crate::intelligence::IntelligenceCoordinator>,
    /// Phase 56: the grounded assistant — answers only from collected evidence.
    pub assistant: Arc<crate::assistant::AssistantCoordinator>,
}

impl ServiceContext {
    pub fn handle(
        &self,
        peer: &aethercore_security::PrincipalContext,
        request_context: &RequestContext,
        req: Request,
    ) -> Response {
        handle_request(self, peer, request_context, req)
    }
}

fn err<E: Into<ServiceError>>(error: E) -> ServiceError {
    error.into()
}
