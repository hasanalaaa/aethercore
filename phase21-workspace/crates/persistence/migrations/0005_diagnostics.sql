CREATE TABLE IF NOT EXISTS diagnostic_snapshots (
    snapshot_id TEXT PRIMARY KEY,
    state TEXT NOT NULL,
    collected_unix_ms INTEGER NOT NULL,
    warning_count INTEGER NOT NULL,
    snapshot_json TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_diagnostic_snapshots_collected ON diagnostic_snapshots(collected_unix_ms DESC);
