#![deny(unsafe_op_in_unsafe_fn)]

mod diagnosis;
mod graph;
mod model;
mod planner;
mod taxonomy;

pub use diagnosis::{DiagnosisRuleVersion, diagnose};
pub use graph::GraphError;
pub use model::*;
pub use planner::{PlannerError, build_repair_graph};
pub use taxonomy::{KnownWindowsError, classify_windows_error};

pub fn analyze(input: &RepairObservationSet) -> RepairIntelligenceSnapshot {
    let diagnoses = diagnose(input);
    let graph = build_repair_graph(input, &diagnoses)
        .unwrap_or_else(|error| RepairGraph::invalid(error.to_string()));
    RepairIntelligenceSnapshot {
        schema: "aethercore.windows-repair-intelligence.v1".into(),
        observation_id: input.observation_id.clone(),
        machine_state_fingerprint: input.machine_state_fingerprint.clone(),
        facts: input.facts.clone(),
        diagnoses,
        recovery: input.recovery.clone(),
        graph,
    }
}

/// The fingerprint every stored assessment is keyed by. Fallible for the same reason
/// `graph::digest` is: a fingerprint has no honest fallback. Every field of `RepairFact` and
/// `RecoveryReadiness` is a `String`, an `i64` or a unit enum, so `to_vec` cannot actually
/// fail today - but "cannot fail today" is not a reason to panic in a service. P63.
pub fn canonical_machine_state_fingerprint(
    facts: &[RepairFact],
    recovery: &RecoveryReadiness,
) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    let mut canonical = facts.to_vec();
    canonical.sort_by(|a, b| a.id.cmp(&b.id).then(a.resource.cmp(&b.resource)));
    let bytes = serde_json::to_vec(&(canonical, recovery))
        .map_err(|error| format!("repair facts could not be serialized: {error}"))?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

/// Seals the workflow state at a reboot barrier. The token is never mutation authority: a fresh
/// post-reboot observation and a newly validated graph are mandatory before further mutation.
pub fn reboot_resume_token(
    assessment_id: &str,
    machine_state_fingerprint: &str,
    graph_digest: &str,
) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(b"aethercore.phase19.reboot-resume.v1\0");
    for part in [
        assessment_id,
        machine_state_fingerprint,
        graph_digest,
        "FreshAssessmentRequiredAfterReboot",
    ] {
        h.update(part.as_bytes());
        h.update([0]);
    }
    hex::encode(h.finalize())
}
