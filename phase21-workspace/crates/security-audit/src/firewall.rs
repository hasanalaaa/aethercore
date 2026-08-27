//! Phase 32 — Firewall-state provider (config-file presence ONLY).
//!
//! Detects pf/ufw/nftables via their CONFIG FILES. No rule queries requiring
//! elevation, no socket probes. Absence yields honest Degraded reasons.

use crate::model::{Confidence, EvidenceRef, SecFinding, Severity};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum FirewallStatus {
    /// A firewall config was found (state inferred from config presence only).
    ConfigPresent { product: String, path: String },
    /// No firewall config found on this host.
    NotAvailable { reason: String },
}

/// Checks every supported firewall's config file locations.
pub fn audit_firewall_state() -> FirewallStatus {
    let candidates: [(&str, &[&str]); 3] = [
        ("pf", &["/etc/pf.conf", "/private/etc/pf.conf"]),
        ("ufw", &["/etc/ufw/ufw.conf", "/lib/ufw/ufw.conf"]),
        (
            "nftables",
            &["/etc/nftables.conf", "/etc/sysconfig/nftables.conf"],
        ),
    ];
    for (product, paths) in candidates {
        for p in paths {
            if Path::new(p).is_file() {
                return FirewallStatus::ConfigPresent {
                    product: product.to_string(),
                    path: p.to_string(),
                };
            }
        }
    }
    FirewallStatus::NotAvailable {
        reason: "no pf/ufw/nftables configuration file found; live state probing requires \
             elevation and stays out of scope (QD-032-001)"
            .to_string(),
    }
}

/// Wraps the status into either an advisory finding (config found ⇒ note its
/// presence as evidence-backed posture info) or an honest NotAvailable marker
/// finding so reports never show silent gaps.
pub fn firewall_findings(status: &FirewallStatus) -> Option<SecFinding> {
    match status {
        FirewallStatus::ConfigPresent { product, path } => SecFinding::try_new(
            "SEC-FW-001",
            "fw.config_present",
            Severity::Advisory,
            vec![EvidenceRef {
                fact: format!("{product} configuration file present at {path}"),
                observed: format!("{product}:config-present"),
                expected_or_threshold:
                    "host packet filter configured; rule review requires elevation lane".to_string(),
                source_location: path.clone(),
            }],
            None,
            "sec.fw.configPresent",
            Confidence::Inferred,
        ),
        FirewallStatus::NotAvailable { reason } => SecFinding::try_new(
            "SEC-FW-900",
            "fw.state_not_available",
            Severity::Advisory,
            vec![EvidenceRef {
                fact: reason.clone(),
                observed: "firewall=not-available".to_string(),
                expected_or_threshold: "active host firewall state (needs elevated probe lane)"
                    .to_string(),
                source_location: "host".to_string(),
            }],
            None,
            "sec.fw.notAvailable",
            Confidence::Heuristic,
        ),
    }
}
