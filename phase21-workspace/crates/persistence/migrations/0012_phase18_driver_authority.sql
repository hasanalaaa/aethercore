CREATE TABLE IF NOT EXISTS driver_authority_overrides (
  owner_principal_key TEXT NOT NULL,
  override_id TEXT NOT NULL,
  device_privacy_key TEXT NOT NULL,
  behavior TEXT NOT NULL,
  candidate_version TEXT NOT NULL DEFAULT '',
  provider_id TEXT NOT NULL DEFAULT '',
  expires_unix_ms INTEGER,
  created_unix_ms INTEGER NOT NULL,
  updated_unix_ms INTEGER NOT NULL,
  PRIMARY KEY(owner_principal_key, override_id)
);
CREATE INDEX IF NOT EXISTS idx_driver_authority_overrides_owner_device
  ON driver_authority_overrides(owner_principal_key, device_privacy_key, updated_unix_ms DESC);

CREATE TABLE IF NOT EXISTS driver_authority_scans (
  owner_principal_key TEXT NOT NULL,
  scan_id TEXT NOT NULL,
  inventory_epoch INTEGER NOT NULL,
  authority_coverage TEXT NOT NULL,
  snapshot_json TEXT NOT NULL,
  created_unix_ms INTEGER NOT NULL,
  PRIMARY KEY(owner_principal_key, scan_id)
);
CREATE INDEX IF NOT EXISTS idx_driver_authority_scans_owner_time
  ON driver_authority_scans(owner_principal_key, created_unix_ms DESC);
