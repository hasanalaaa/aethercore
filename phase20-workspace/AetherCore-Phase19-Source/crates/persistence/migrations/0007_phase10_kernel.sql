-- Phase 10 Operation Kernel persistence tuning.
--
-- Live progress/event telemetry is intentionally NOT persisted here. The Operation Kernel event
-- bus and ProgressTelemetryStore are the high-frequency plane; SQLite remains the durable safety
-- ledger for state transitions, mutation evidence, checkpoints, recovery, and coarse progress.

CREATE INDEX IF NOT EXISTS idx_maintenance_executions_domain_updated
ON maintenance_executions(domain, updated_unix_ms DESC);

CREATE INDEX IF NOT EXISTS idx_maintenance_executions_stage_updated
ON maintenance_executions(stage, updated_unix_ms DESC);

CREATE INDEX IF NOT EXISTS idx_plan_executions_stage_updated
ON plan_executions(stage, updated_unix_ms DESC);

CREATE INDEX IF NOT EXISTS idx_consent_intents_owner_state
ON consent_intents(owner_principal_key, consumed_unix_ms, approved_unix_ms, expires_unix_ms DESC);
