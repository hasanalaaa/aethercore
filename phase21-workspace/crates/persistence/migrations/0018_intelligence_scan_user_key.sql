-- Quality pass B6: deep scan history follows the Windows user across sign-ins. The existing
-- owner_principal_key is logon-session scoped and stays the authority for a running scan.
-- Rows written before this migration keep '' and are never returned for a user.
ALTER TABLE intelligence_scans ADD COLUMN owner_user_key TEXT NOT NULL DEFAULT '';
CREATE INDEX IF NOT EXISTS idx_intelligence_scans_user_completed ON intelligence_scans(owner_user_key, completed_unix_ms DESC);
