//! Phase 22 — One-Click Care domain model.
//!
//! The orchestrator composes EXISTING typed domain plans only. It never creates new
//! mutation semantics: every step wraps a plan that its own domain already created,
//! sealed, and digest-bound. Safety levels mirror the product safety contract
//! (PCD-ONE-CLICK-ORCHESTRATION completionTruth): SAFE_AUTO may run after explicit
//! per-session consent; everything else becomes review work, never auto-executed.

use serde::Serialize;
use sha2::{Digest, Sha256};

/// Hard bound on steps in one orchestrated plan.
pub const MAX_CARE_STEPS: usize = 16;

/// Safety classification of an existing domain plan.
///
/// `Auto` covers plans whose own domains classify their actions SAFE_AUTO
/// (cleanup allowlist evidence, reversible startup disables). Anything else —
/// driver installs, Windows integrity repair, anything sensitive or manual — is
/// `ReviewOnly` and the orchestrator refuses to execute it.
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CareSafety {
    /// May auto-run only after the owner granted explicit one-time session consent.
    Auto,
    /// Never executed by the orchestrator; surfaces as review guidance only.
    ReviewOnly,
}

impl CareSafety {
    /// Numeric level used on the wire and in the journal (0 = auto, 2+ = review).
    pub fn level(self) -> i64 {
        match self {
            CareSafety::Auto => 0,
            // Level 1 is reserved for future explicitly-approved dependent actions;
            // nothing shipped today qualifies, so all non-auto work sits at level 2.
            CareSafety::ReviewOnly => 2,
        }
    }

    pub fn parse_level(level: i64) -> Option<Self> {
        match level {
            0 => Some(CareSafety::Auto),
            _ if level >= 2 => Some(CareSafety::ReviewOnly),
            _ => None,
        }
    }
}

/// One step of a care plan: a reference to an existing, immutable, digest-bound
/// domain plan plus its safety class. No action content lives here.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CareStep {
    /// The EXISTING domain plan id (created by cleaner/startup/etc.).
    pub domain_plan_id: String,
    /// Domain kind string exactly as the owning domain reports it.
    pub domain_kind: String,
    pub safety: CareSafety,
    /// Human-presentable message key resolved in renderer catalogs.
    pub title_key: String,
}

/// A deterministic orchestration plan over existing domain plans.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CarePlan {
    /// Steps ordered by (safety level ascending, so auto work comes first and review
    /// work is listed after it, then domain_kind, then domain_plan_id) — a total
    /// order over stable content.
    pub steps: Vec<CareStep>,
    /// Deterministic digest over the sorted step content.
    pub plan_digest_sha256: String,
}

impl CarePlan {
    /// Builds a plan from candidate steps. Duplicates (same domain plan id) collapse;
    /// empty plan ids are rejected; the result is deterministic regardless of the
    /// caller's ordering. An empty input yields `None` — there is nothing to care-run.
    pub fn build(steps: Vec<CareStep>) -> Option<Self> {
        let mut seen = std::collections::BTreeSet::new();
        let mut unique: Vec<CareStep> = Vec::new();
        for step in steps {
            let id = step.domain_plan_id.trim();
            if id.is_empty() || !seen.insert(id.to_string()) {
                continue;
            }
            unique.push(step);
        }
        if unique.is_empty() || unique.len() > MAX_CARE_STEPS {
            return None;
        }
        // Total order: auto-level work first (review items are listed after), then
        // kind, then plan id.
        unique.sort_by(|a, b| {
            (
                a.safety.level(),
                a.domain_kind.as_str(),
                a.domain_plan_id.as_str(),
            )
                .cmp(&(
                    b.safety.level(),
                    b.domain_kind.as_str(),
                    b.domain_plan_id.as_str(),
                ))
        });
        let plan_digest_sha256 = digest_of(&unique);
        Some(Self {
            steps: unique,
            plan_digest_sha256,
        })
    }

    /// Steps eligible for automatic execution under an active session consent.
    pub fn auto_steps(&self) -> impl Iterator<Item = &CareStep> {
        self.steps
            .iter()
            .filter(|step| step.safety == CareSafety::Auto)
    }

    /// Steps the orchestrator will never run; they become review guidance.
    pub fn review_steps(&self) -> impl Iterator<Item = &CareStep> {
        self.steps
            .iter()
            .filter(|step| step.safety == CareSafety::ReviewOnly)
    }
}

fn digest_of(steps: &[CareStep]) -> String {
    let mut hasher = Sha256::new();
    hasher.update((steps.len() as u64).to_le_bytes());
    for step in steps {
        hasher.update((step.domain_plan_id.len() as u64).to_le_bytes());
        hasher.update(step.domain_plan_id.as_bytes());
        hasher.update((step.domain_kind.len() as u64).to_le_bytes());
        hasher.update(step.domain_kind.as_bytes());
        hasher.update([step.safety.level() as u8]);
        hasher.update((step.title_key.len() as u64).to_le_bytes());
        hasher.update(step.title_key.as_bytes());
    }
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let bytes = hasher.finalize();
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in &bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

/// Typed errors. Never stringly state; never panic on hostile input.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CareError {
    #[error("session consent has not been granted for this run")]
    ConsentRequired,
    #[error("another mutation already holds the machine-wide lease")]
    LeaseBusy,
    #[error("care plan digest changed between approval and execution")]
    DigestChanged,
    #[error("step {step_index} targets plan {domain_plan_id} which is not in the approved plan")]
    PlanMismatch {
        step_index: usize,
        domain_plan_id: String,
    },
    #[error("domain {domain_kind} rejected the request: {detail}")]
    DomainRejected { domain_kind: String, detail: String },
    #[error("care run {run_id} is not resumable in state {state}")]
    NotResumable { run_id: String, state: String },
    #[error("journal error: {0}")]
    Journal(String),
    /// DBT-P46-B33: the owner's existing plans could not be read, so what is
    /// due is unknown. Distinct from `Journal`, which is a failure to RECORD a
    /// run — this one means the run must not start, and a preview must not
    /// claim an empty plan.
    #[error("the plans this run would compose from could not be read: {0}")]
    PlanSourcesUnavailable(String),
}

/// Outcome polarity of a finished step. The orchestrator reports what the DOMAIN
/// verified — it never derives aggregate claims beyond these citations.
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum StepOutcome {
    /// The domain reported its own verification as passed.
    VerifiedByDomain,
    /// The domain completed but could not verify (verification unavailable).
    CompletedUnverified,
    /// The domain failed; failure_message carries the domain's own message key.
    Failed,
    /// Skipped because session consent did not cover it or a previous step failed hard.
    Skipped,
}

/// Truth-first report entry for one finished step.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CareStepReport {
    pub step_index: usize,
    pub domain_plan_id: String,
    pub domain_kind: String,
    pub outcome: StepOutcome,
    /// Verification state exactly as the domain reported it ("Verified", "" ...).
    pub domain_verification_state: String,
    /// Domain's own failure message key when outcome == Failed.
    pub failure_message_key: String,
}
