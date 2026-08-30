ALTER TABLE maintenance_executions ADD COLUMN outcome TEXT NOT NULL DEFAULT '';
ALTER TABLE maintenance_executions ADD COLUMN machine_state_fingerprint TEXT NOT NULL DEFAULT '';
ALTER TABLE maintenance_executions ADD COLUMN repair_graph_digest TEXT NOT NULL DEFAULT '';
ALTER TABLE maintenance_executions ADD COLUMN reboot_required INTEGER NOT NULL DEFAULT 0;
ALTER TABLE maintenance_executions ADD COLUMN reboot_resume_token TEXT NOT NULL DEFAULT '';
ALTER TABLE maintenance_executions ADD COLUMN verification_state TEXT NOT NULL DEFAULT '';

CREATE TABLE IF NOT EXISTS repair_timeline_events (
  event_id TEXT PRIMARY KEY,
  plan_id TEXT NOT NULL DEFAULT '',
  assessment_id TEXT NOT NULL DEFAULT '',
  owner_principal_key TEXT NOT NULL DEFAULT '',
  event_kind TEXT NOT NULL,
  domain TEXT NOT NULL,
  action_id TEXT NOT NULL DEFAULT '',
  diagnosis_code TEXT NOT NULL DEFAULT '',
  outcome TEXT NOT NULL DEFAULT '',
  detail TEXT NOT NULL DEFAULT '',
  machine_state_fingerprint TEXT NOT NULL DEFAULT '',
  created_unix_ms INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_repair_timeline_created ON repair_timeline_events(created_unix_ms DESC);
CREATE INDEX IF NOT EXISTS idx_repair_timeline_plan ON repair_timeline_events(plan_id, created_unix_ms);

CREATE TABLE IF NOT EXISTS repair_reboot_resume_tickets (
  token_sha256 TEXT PRIMARY KEY,
  owner_principal_key TEXT NOT NULL,
  assessment_id TEXT NOT NULL,
  machine_state_fingerprint TEXT NOT NULL,
  repair_graph_digest TEXT NOT NULL,
  resume_policy TEXT NOT NULL,
  state TEXT NOT NULL,
  created_unix_ms INTEGER NOT NULL,
  consumed_unix_ms INTEGER
);
CREATE INDEX IF NOT EXISTS idx_repair_reboot_owner ON repair_reboot_resume_tickets(owner_principal_key, state, created_unix_ms DESC);
