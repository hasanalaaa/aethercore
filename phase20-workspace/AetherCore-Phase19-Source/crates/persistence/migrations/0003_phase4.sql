CREATE TABLE IF NOT EXISTS maintenance_executions (
  plan_id TEXT PRIMARY KEY REFERENCES plans(id) ON DELETE CASCADE,
  domain TEXT NOT NULL,
  stage TEXT NOT NULL,
  progress_known INTEGER NOT NULL DEFAULT 0,
  overall_percent INTEGER NOT NULL DEFAULT 0,
  current_item_id TEXT NOT NULL DEFAULT '',
  detail TEXT NOT NULL DEFAULT '',
  mutation_started INTEGER NOT NULL DEFAULT 0,
  recovery_required INTEGER NOT NULL DEFAULT 0,
  failure_message TEXT NOT NULL DEFAULT '',
  started_unix_ms INTEGER NOT NULL,
  updated_unix_ms INTEGER NOT NULL,
  completed_unix_ms INTEGER
);

CREATE TABLE IF NOT EXISTS maintenance_execution_items (
  plan_id TEXT NOT NULL REFERENCES plans(id) ON DELETE CASCADE,
  item_id TEXT NOT NULL,
  kind TEXT NOT NULL,
  stage TEXT NOT NULL,
  result_code TEXT NOT NULL DEFAULT '',
  bytes_affected INTEGER NOT NULL DEFAULT 0,
  detail TEXT NOT NULL DEFAULT '',
  updated_unix_ms INTEGER NOT NULL,
  PRIMARY KEY(plan_id,item_id)
);
CREATE INDEX IF NOT EXISTS idx_maintenance_execution_items_plan ON maintenance_execution_items(plan_id);
