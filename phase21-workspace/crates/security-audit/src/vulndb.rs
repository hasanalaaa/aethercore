//! Phase 32 — Air-gapped vulnerability database (vulndb).
//!
//! The loader is FAIL-CLOSED: `vulndb.json` must byte-match the sha256 pin in
//! `vulndb.manifest.json` or the entire CVE lane degrades to
//! `NotAvailable(sec.notAvailable.vulndbIntegrity)`. Updating the DB is an
//! EXPLICIT owner action performed by external tooling writing a fresh manifest
//! entry; nothing in this crate performs network I/O (network-ban gate enforced
//! by scripts/phase32-adversarial-audit.py).

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use std::path::Path;

/// Hard clamp: maximum vulndb.json size (bytes).
pub const MAX_VULNDB_BYTES: u64 = 8 * 1024 * 1024;
/// Hard clamp: maximum manifest size (bytes).
pub const MAX_MANIFEST_BYTES: u64 = 64 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum VulnDbError {
    #[error("vulndb integrity: manifest missing at {0}")]
    ManifestMissing(String),
    #[error("vulndb integrity: db file missing at {0}")]
    DbMissing(String),
    #[error("vulndb integrity: sha256 mismatch for {path}: expected {expected}, got {actual}")]
    HashMismatch {
        path: String,
        expected: String,
        actual: String,
    },
    #[error("vulndb parse error: {0}")]
    Parse(String),
    #[error("vulndb oversize: {size} > limit {limit}")]
    Oversize { size: u64, limit: u64 },
    #[error("vulndb entry-count drift: db has {db}, manifest pins {manifest}")]
    EntryCountDrift { db: usize, manifest: usize },
}

/// One seeded vulnerability record (NVD-derived, curated during the P32 build).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VulnEntry {
    pub cve_id: String,
    pub package: String,
    /// Empty string = no lower bound.
    pub introduced: String,
    /// Empty string = never fixed in DB (not matched as vulnerable alone).
    pub fixed: String,
    pub summary: String,
}

/// Integrity pin checked before every use.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VulnDbManifest {
    pub schema: String,
    pub entries: usize,
    pub sha256: String,
}

/// Loads and verifies the DB. Any integrity problem is a typed error — never a
/// best-effort parse of tampered data.
pub fn load_verified(db_path: &Path, manifest_path: &Path) -> Result<Vec<VulnEntry>, VulnDbError> {
    let manifest_raw = std::fs::read(manifest_path)
        .map_err(|_| VulnDbError::ManifestMissing(manifest_path.display().to_string()))?;
    if manifest_raw.len() as u64 > MAX_MANIFEST_BYTES {
        return Err(VulnDbError::Oversize {
            size: manifest_raw.len() as u64,
            limit: MAX_MANIFEST_BYTES,
        });
    }
    let manifest: VulnDbManifest = serde_json::from_slice(&manifest_raw)
        .map_err(|e| VulnDbError::Parse(format!("manifest: {e}")))?;
    let meta = std::fs::metadata(db_path)
        .map_err(|_| VulnDbError::DbMissing(db_path.display().to_string()))?;
    if meta.len() > MAX_VULNDB_BYTES {
        return Err(VulnDbError::Oversize {
            size: meta.len(),
            limit: MAX_VULNDB_BYTES,
        });
    }
    let raw = std::fs::read(db_path)
        .map_err(|_| VulnDbError::DbMissing(db_path.display().to_string()))?;
    let digest = format!("{:x}", Sha256::digest(&raw));
    if digest != manifest.sha256 {
        return Err(VulnDbError::HashMismatch {
            path: db_path.display().to_string(),
            expected: manifest.sha256,
            actual: digest,
        });
    }
    let entries: Vec<VulnEntry> =
        serde_json::from_slice(&raw).map_err(|e| VulnDbError::Parse(format!("db: {e}")))?;
    if entries.len() != manifest.entries {
        return Err(VulnDbError::EntryCountDrift {
            db: entries.len(),
            manifest: manifest.entries,
        });
    }
    Ok(entries)
}
