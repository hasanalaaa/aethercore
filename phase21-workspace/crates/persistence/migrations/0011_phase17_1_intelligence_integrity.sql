ALTER TABLE intelligence_findings ADD COLUMN verification_status TEXT NOT NULL DEFAULT 'NotRechecked';
ALTER TABLE intelligence_findings ADD COLUMN resolved_at_unix_ms INTEGER;
ALTER TABLE intelligence_findings ADD COLUMN resolution_scan_id TEXT NOT NULL DEFAULT '';
ALTER TABLE intelligence_findings ADD COLUMN resolution_reason_key TEXT NOT NULL DEFAULT '';

CREATE INDEX IF NOT EXISTS idx_intelligence_findings_owner_verification
ON intelligence_findings(owner_principal_key, verification_status, lifecycle, last_observed_unix_ms DESC);
