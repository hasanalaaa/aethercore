-- Phase 34 — Fleet & Secure Remote Operations persistence.
-- Additive only: three new tables, no changes to existing tables or rows.
-- fleet_hosts: non-secret host configuration (NEVER passwords, key bodies,
--   passphrases or API keys — the schema has no column that could hold them;
--   auth_ref_kind is 'agent'|'key_file'|'certificate' and auth_ref_path is a
--   local path REFERENCE, never content).
-- fleet_schedules: compliance schedule definitions (non-secret).
-- fleet_run_history: append-only per-run metadata for schedules and manual
--   fleet runs (prior history is never erased by later failures).

CREATE TABLE IF NOT EXISTS fleet_hosts (
  host_id TEXT PRIMARY KEY,
  display_name TEXT NOT NULL,
  hostname TEXT NOT NULL,
  port INTEGER NOT NULL,
  username TEXT NOT NULL,
  auth_ref_kind TEXT NOT NULL,
  auth_ref_path TEXT,
  trusted_fingerprint TEXT,
  trusted_key_type TEXT,
  trusted_unix_ms INTEGER,
  enabled INTEGER NOT NULL DEFAULT 1,
  tags_json TEXT NOT NULL DEFAULT '[]',
  updated_unix_ms INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS fleet_schedules (
  schedule_id TEXT PRIMARY KEY,
  scope_json TEXT NOT NULL,
  profile_id TEXT NOT NULL,
  enabled INTEGER NOT NULL DEFAULT 1,
  cadence_kind TEXT NOT NULL,
  cadence_value INTEGER NOT NULL,
  next_run_unix_ms INTEGER NOT NULL,
  last_result_json TEXT,
  updated_unix_ms INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS fleet_run_history (
  run_seq INTEGER PRIMARY KEY AUTOINCREMENT,
  schedule_id TEXT,
  trigger_kind TEXT NOT NULL,
  started_unix_ms INTEGER NOT NULL,
  finished_unix_ms INTEGER,
  hosts_attempted INTEGER NOT NULL DEFAULT 0,
  hosts_ok INTEGER NOT NULL DEFAULT 0,
  hosts_failed INTEGER NOT NULL DEFAULT 0,
  outcome_summary TEXT NOT NULL DEFAULT ''
);

CREATE INDEX IF NOT EXISTS idx_fleet_run_history_schedule
  ON fleet_run_history(schedule_id, started_unix_ms DESC);
