use crate::model::{RepairGraph as RepairGraphModel, RepairNode};
use crate::{RecoveryReadiness, RepairActionKind, RepairSafetyTier};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum GraphError {
    #[error("duplicate repair node id: {0}")]
    DuplicateNode(String),
    #[error("repair node {node} references missing dependency {dependency}")]
    MissingDependency { node: String, dependency: String },
    #[error("repair graph contains a dependency cycle")]
    Cycle,
    #[error("repair graph could not be serialized for its digest: {0}")]
    Digest(String),
    #[error("destructive recovery action cannot be SAFE_AUTO: {0}")]
    DestructiveAuto(String),
    #[error("repair action crosses a reboot boundary without a resume barrier: {0}")]
    RebootBoundary(String),
    #[error("repair node requires recovery protection but none is represented: {0}")]
    RecoveryPrerequisite(String),
    #[error("repair graph contains contradictory recovery actions: {0}")]
    ContradictoryActions(String),
}

impl RepairGraphModel {
    pub fn invalid(reason: String) -> Self {
        Self {
            schema: "aethercore.repair-graph.v1".into(),
            valid: false,
            invalid_reason: reason,
            nodes: Vec::new(),
            deterministic_order: Vec::new(),
            digest_sha256: String::new(),
        }
    }

    pub fn new(mut nodes: Vec<RepairNode>) -> Result<Self, GraphError> {
        nodes.sort_by(|a, b| a.id.cmp(&b.id));
        validate_nodes(&nodes)?;
        let order = topological_order(&nodes)?;
        let digest = digest(&nodes, &order)?;
        Ok(Self {
            schema: "aethercore.repair-graph.v1".into(),
            valid: true,
            invalid_reason: String::new(),
            nodes,
            deterministic_order: order,
            digest_sha256: digest,
        })
    }

    pub fn validate(&self) -> Result<(), GraphError> {
        validate_nodes(&self.nodes)?;
        let order = topological_order(&self.nodes)?;
        if order != self.deterministic_order {
            return Err(GraphError::Cycle);
        }
        validate_reboot_crossings(&self.nodes)?;
        Ok(())
    }

    pub fn validate_for_execution(&self, recovery: &RecoveryReadiness) -> Result<(), GraphError> {
        self.validate()?;
        for n in &self.nodes {
            if n.executable_automatically
                && n.requires_recovery_protection
                && !recovery.has_mandatory_protection()
            {
                return Err(GraphError::RecoveryPrerequisite(n.id.clone()));
            }
        }
        Ok(())
    }
}

fn validate_nodes(nodes: &[RepairNode]) -> Result<(), GraphError> {
    let mut ids = BTreeSet::new();
    for n in nodes {
        if !ids.insert(n.id.clone()) {
            return Err(GraphError::DuplicateNode(n.id.clone()));
        }
    }
    for n in nodes {
        for d in &n.dependencies {
            if !ids.contains(d) {
                return Err(GraphError::MissingDependency {
                    node: n.id.clone(),
                    dependency: d.clone(),
                });
            }
        }
        if matches!(
            n.action,
            RepairActionKind::GuidedResetPreservingFiles | RepairActionKind::GuidedCleanReinstall
        ) && (n.safety <= RepairSafetyTier::Level1SafeAuto || n.executable_automatically)
        {
            return Err(GraphError::DestructiveAuto(n.id.clone()));
        }
        if n.reboot_boundary_after
            && n.executable_automatically
            && matches!(
                n.action,
                RepairActionKind::GuidedRepairReinstall
                    | RepairActionKind::GuidedWinReRecovery
                    | RepairActionKind::GuidedResetPreservingFiles
                    | RepairActionKind::GuidedCleanReinstall
            )
        {
            return Err(GraphError::RebootBoundary(n.id.clone()));
        }
    }
    let has_reset = nodes
        .iter()
        .any(|n| n.action == RepairActionKind::GuidedResetPreservingFiles);
    let has_clean = nodes
        .iter()
        .any(|n| n.action == RepairActionKind::GuidedCleanReinstall);
    if has_reset && has_clean {
        return Err(GraphError::ContradictoryActions(
            "reset-preserving-files + clean-reinstall".into(),
        ));
    }
    validate_reboot_crossings(nodes)?;
    Ok(())
}

fn validate_reboot_crossings(nodes: &[RepairNode]) -> Result<(), GraphError> {
    let map: BTreeMap<&str, &RepairNode> = nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    for n in nodes {
        if !n.executable_automatically {
            continue;
        }
        let mut pending = n.dependencies.clone();
        let mut seen = BTreeSet::new();
        while let Some(id) = pending.pop() {
            if !seen.insert(id.clone()) {
                continue;
            }
            if let Some(dep) = map.get(id.as_str()) {
                if dep.reboot_boundary_after || dep.action == RepairActionKind::Reboot {
                    return Err(GraphError::RebootBoundary(n.id.clone()));
                }
                pending.extend(dep.dependencies.iter().cloned());
            }
        }
    }
    Ok(())
}

fn topological_order(nodes: &[RepairNode]) -> Result<Vec<String>, GraphError> {
    let mut indegree: BTreeMap<String, usize> = nodes
        .iter()
        .map(|n| (n.id.clone(), n.dependencies.len()))
        .collect();
    let mut outgoing: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for n in nodes {
        for d in &n.dependencies {
            outgoing.entry(d.clone()).or_default().push(n.id.clone());
        }
    }
    for v in outgoing.values_mut() {
        v.sort();
    }
    let mut q: VecDeque<String> = indegree
        .iter()
        .filter(|(_, v)| **v == 0)
        .map(|(k, _)| k.clone())
        .collect();
    let mut out = Vec::with_capacity(nodes.len());
    while let Some(id) = q.pop_front() {
        out.push(id.clone());
        if let Some(next) = outgoing.get(&id) {
            for n in next {
                // `indegree` was built from the same node set `outgoing` indexes, so this is
                // always Some; a let-else keeps a malformed set out of a panic, and the
                // `out.len() != nodes.len()` check below still rejects the result. P63.
                let Some(e) = indegree.get_mut(n) else {
                    continue;
                };
                *e -= 1;
                if *e == 0 {
                    let pos = q.iter().position(|x| x > n).unwrap_or(q.len());
                    q.insert(pos, n.clone());
                }
            }
        }
    }
    if out.len() != nodes.len() {
        return Err(GraphError::Cycle);
    }
    Ok(out)
}

fn digest<T: Serialize>(nodes: &T, order: &[String]) -> Result<String, GraphError> {
    // A digest has no honest fallback: a fabricated one would be indistinguishable from a
    // real fingerprint. So the error is propagated, not swallowed and not panicked on. P63.
    let bytes = serde_json::to_vec(&(nodes, order))
        .map_err(|error| GraphError::Digest(error.to_string()))?;
    Ok(hex::encode(Sha256::digest(bytes)))
}
