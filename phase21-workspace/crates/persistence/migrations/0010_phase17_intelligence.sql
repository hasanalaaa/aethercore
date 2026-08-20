CREATE TABLE IF NOT EXISTS intelligence_scans (
  scan_id TEXT PRIMARY KEY,
  owner_principal_key TEXT NOT NULL,
  state TEXT NOT NULL,
  status TEXT NOT NULL,
  started_unix_ms INTEGER NOT NULL,
  completed_unix_ms INTEGER NOT NULL,
  machine_state_fingerprint TEXT NOT NULL,
  rule_engine_version TEXT NOT NULL,
  app_version TEXT NOT NULL,
  finding_count INTEGER NOT NULL,
  unavailable_collector_count INTEGER NOT NULL,
  snapshot_json TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_intelligence_scans_owner_completed ON intelligence_scans(owner_principal_key, completed_unix_ms DESC);

CREATE TABLE IF NOT EXISTS intelligence_findings (
  owner_principal_key TEXT NOT NULL,
  finding_id TEXT NOT NULL,
  finding_code TEXT NOT NULL,
  first_observed_unix_ms INTEGER NOT NULL,
  last_observed_unix_ms INTEGER NOT NULL,
  lifecycle TEXT NOT NULL,
  severity TEXT NOT NULL,
  confidence TEXT NOT NULL,
  finding_json TEXT NOT NULL,
  PRIMARY KEY(owner_principal_key, finding_id)
);
CREATE INDEX IF NOT EXISTS idx_intelligence_findings_owner_lifecycle ON intelligence_findings(owner_principal_key, lifecycle, last_observed_unix_ms DESC);

CREATE TABLE IF NOT EXISTS intelligence_overrides (
  owner_principal_key TEXT NOT NULL,
  override_id TEXT NOT NULL,
  scope_kind TEXT NOT NULL,
  scope_value_hash TEXT NOT NULL,
  behavior TEXT NOT NULL,
  expires_unix_ms INTEGER,
  created_unix_ms INTEGER NOT NULL,
  updated_unix_ms INTEGER NOT NULL,
  PRIMARY KEY(owner_principal_key, override_id)
);

CREATE TABLE IF NOT EXISTS intelligence_remediation_plans (
  plan_id TEXT PRIMARY KEY,
  owner_principal_key TEXT NOT NULL,
  scan_id TEXT NOT NULL,
  digest TEXT NOT NULL,
  immutable_json TEXT NOT NULL,
  created_unix_ms INTEGER NOT NULL,
  FOREIGN KEY(scan_id) REFERENCES intelligence_scans(scan_id) ON DELETE RESTRICT
);
