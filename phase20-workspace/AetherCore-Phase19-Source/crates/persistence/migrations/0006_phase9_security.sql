ALTER TABLE plans ADD COLUMN owner_principal_key TEXT NOT NULL DEFAULT '';
CREATE INDEX IF NOT EXISTS idx_plans_owner_updated ON plans(owner_principal_key, updated_unix_ms DESC);

-- Phase 8 plans have no trustworthy Windows-principal owner. Never resume them under Phase 9.
-- Preserve terminal history for forensic compatibility, but quarantine every incomplete legacy plan
-- before any recovery code can observe it.
INSERT INTO plan_events(plan_id,from_state,to_state,event_kind,detail,created_unix_ms)
SELECT id,state,'Failed','phase9_owner_quarantine',
       'Pre-Phase-9 plan quarantined because no trusted Windows principal ownership exists.',
       updated_unix_ms
FROM plans
WHERE owner_principal_key='' AND state NOT IN ('Completed','Failed');
UPDATE plans
SET state='Failed'
WHERE owner_principal_key='' AND state NOT IN ('Completed','Failed');

-- Phase 9 permanently retires the reusable challenge/grant authorization model. Upgrade consumes
-- no legacy authorization authority: old rows and tables are removed before consent intents exist.
DROP TABLE IF EXISTS authorization_challenges;
DROP TABLE IF EXISTS authorization_grants;

CREATE TABLE consent_intents (
  intent_id TEXT PRIMARY KEY,
  plan_id TEXT NOT NULL REFERENCES plans(id) ON DELETE CASCADE,
  digest TEXT NOT NULL,
  owner_principal_key TEXT NOT NULL,
  created_unix_ms INTEGER NOT NULL,
  expires_unix_ms INTEGER NOT NULL,
  approved_unix_ms INTEGER,
  consumed_unix_ms INTEGER,
  broker_pid INTEGER
);
CREATE INDEX idx_consent_intents_plan ON consent_intents(plan_id, owner_principal_key, expires_unix_ms DESC);
CREATE INDEX idx_consent_intents_pending ON consent_intents(owner_principal_key, approved_unix_ms, consumed_unix_ms, expires_unix_ms);

-- Diagnostic history can contain device identifiers, event evidence and dump metadata. Bind it to
-- the same kernel-derived principal key as operation plans; pre-Phase-9 rows remain ownerless and
-- are intentionally invisible to Phase-9 clients.
ALTER TABLE diagnostic_snapshots ADD COLUMN owner_principal_key TEXT NOT NULL DEFAULT '';
CREATE INDEX idx_diagnostic_snapshots_owner_collected
ON diagnostic_snapshots(owner_principal_key, collected_unix_ms DESC);
