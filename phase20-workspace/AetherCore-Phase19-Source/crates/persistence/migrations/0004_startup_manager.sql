CREATE TABLE IF NOT EXISTS startup_change_records (
  change_id TEXT PRIMARY KEY,
  origin_change_id TEXT NOT NULL,
  plan_id TEXT NOT NULL REFERENCES plans(id) ON DELETE CASCADE,
  item_id TEXT NOT NULL,
  kind TEXT NOT NULL,
  display_name TEXT NOT NULL,
  direction TEXT NOT NULL,
  original_json TEXT NOT NULL,
  applied_json TEXT NOT NULL,
  state TEXT NOT NULL,
  detail TEXT NOT NULL DEFAULT '',
  created_unix_ms INTEGER NOT NULL,
  updated_unix_ms INTEGER NOT NULL,
  restored_unix_ms INTEGER
);
CREATE INDEX IF NOT EXISTS idx_startup_changes_plan ON startup_change_records(plan_id, updated_unix_ms DESC);
CREATE INDEX IF NOT EXISTS idx_startup_changes_item ON startup_change_records(item_id, updated_unix_ms DESC);
CREATE INDEX IF NOT EXISTS idx_startup_changes_state ON startup_change_records(state, updated_unix_ms DESC);
