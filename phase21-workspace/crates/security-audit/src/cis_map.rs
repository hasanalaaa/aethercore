//! Phase 32 — CIS control mapping asset loader + format gate support.
//!
//! `cis_map.json` maps implemented rule codes → CIS benchmark control ids.
//! The audit script asserts format independently; this module provides typed
//! access for report rendering (`compliance summary --profile cis-l1`).

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Maximum cis_map.json size.
pub const MAX_CIS_MAP_BYTES: u64 = 256 * 1024;

/// One mapping row. `control` is either a CIS id (`CIS L1 5.2.8`) or the
/// literal sentinel `unmapped` plus a reason note.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CisMapEntry {
    pub rule_code: String,
    pub control: String,
    #[serde(default)]
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CisMap {
    pub schema: String,
    pub profile: String,
    pub entries: Vec<CisMapEntry>,
}

impl CisMap {
    /// Format predicate used by tests and mirrored by the audit gate:
    /// every control matches `^CIS L[12] \d+\.\d+(\.\d+)?$` or is exactly
    /// `unmapped` WITH a non-empty note.
    pub fn entry_is_wellformed(e: &CisMapEntry) -> bool {
        if e.control == "unmapped" {
            return !e.note.trim().is_empty();
        }
        let b = e.control.as_bytes();
        let ok_prefix = b.starts_with(b"CIS L1 ") || b.starts_with(b"CIS L2 ");
        if !ok_prefix {
            return false;
        }
        let rest = &e.control["CIS L1 ".len()..];
        let mut parts = rest.split('.');
        let mut count = 0;
        for p in parts.by_ref() {
            if p.is_empty() || !p.bytes().all(|c| c.is_ascii_digit()) {
                return false;
            }
            count += 1;
            if count > 3 {
                return false;
            }
        }
        count >= 2
    }

    pub fn lookup(&self, rule_code: &str) -> Option<&CisMapEntry> {
        self.entries.iter().find(|e| e.rule_code == rule_code)
    }
}

/// Loads and parses the map; oversize is refused.
pub fn load(path: &Path) -> Result<CisMap, String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("cis_map stat: {e}"))?;
    if meta.len() > MAX_CIS_MAP_BYTES {
        return Err(format!(
            "cis_map oversize: {} > {MAX_CIS_MAP_BYTES}",
            meta.len()
        ));
    }
    let raw = std::fs::read(path).map_err(|e| format!("cis_map read: {e}"))?;
    serde_json::from_slice::<CisMap>(&raw).map_err(|e| format!("cis_map parse: {e}"))
}

/// True when every entry is wellformed (format-gate helper).
pub fn all_entries_wellformed(map: &CisMap) -> bool {
    map.entries.iter().all(CisMap::entry_is_wellformed)
}
