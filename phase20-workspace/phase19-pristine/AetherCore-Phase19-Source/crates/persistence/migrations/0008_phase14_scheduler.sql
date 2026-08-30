-- Phase 14 autonomous scheduler cadence ledger.
--
-- This table contains scheduling metadata only. It cannot authorize mutations and stores no
-- command, path, URL, consent capability, or diagnostic payload. Principal scoping prevents one
-- interactive session from inheriting another user's cadence state.
CREATE TABLE IF NOT EXISTS autonomous_scheduler_runs (
    owner_principal_key TEXT NOT NULL,
    workload TEXT NOT NULL,
    failure_count INTEGER NOT NULL DEFAULT 0 CHECK(failure_count >= 0),
    next_eligible_unix_ms INTEGER NOT NULL,
    last_outcome TEXT NOT NULL,
    last_completed_unix_ms INTEGER,
    updated_unix_ms INTEGER NOT NULL,
    PRIMARY KEY(owner_principal_key, workload)
);

CREATE INDEX IF NOT EXISTS idx_autonomous_scheduler_runs_due
ON autonomous_scheduler_runs(owner_principal_key, next_eligible_unix_ms);
