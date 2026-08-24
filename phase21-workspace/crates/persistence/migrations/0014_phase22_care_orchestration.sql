-- Phase 22 — One-Click Care orchestration journal.
-- Additive only: two new tables, no changes to existing tables or rows.
-- care_runs: one row per orchestrated run (crash-safe resume anchor).
-- care_steps: per-step records; each step wraps an EXISTING domain plan and cites
-- that domain's own verification outcome (truth-first reporting).

CREATE TABLE IF NOT EXISTS care_runs (
  run_id TEXT PRIMARY KEY,
  owner_principal_key TEXT NOT NULL,
  state TEXT NOT NULL,
  stage TEXT NOT NULL DEFAULT '',
  plan_digest_sha256 TEXT NOT NULL,
  session_consent_granted INTEGER NOT NULL DEFAULT 0,
  steps_total INTEGER NOT NULL DEFAULT 0,
  steps_done INTEGER NOT NULL DEFAULT 0,
  detail TEXT NOT NULL DEFAULT '',
  created_unix_ms INTEGER NOT NULL,
  updated_unix_ms INTEGER NOT NULL,
  completed_unix_ms INTEGER
);

CREATE TABLE IF NOT EXISTS care_steps (
  run_id TEXT NOT NULL,
  step_index INTEGER NOT NULL,
  domain_plan_id TEXT NOT NULL,
  domain_kind TEXT NOT NULL,
  safety_level INTEGER NOT NULL,
  state TEXT NOT NULL,
  outcome TEXT NOT NULL DEFAULT '',
  verification_state TEXT NOT NULL DEFAULT '',
  failure_message TEXT NOT NULL DEFAULT '',
  started_unix_ms INTEGER NOT NULL,
  updated_unix_ms INTEGER NOT NULL,
  PRIMARY KEY (run_id, step_index)
);

CREATE INDEX IF NOT EXISTS idx_care_runs_owner
  ON care_runs(owner_principal_key, created_unix_ms DESC);
