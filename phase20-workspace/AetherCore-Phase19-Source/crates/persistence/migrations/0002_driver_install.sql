CREATE TABLE IF NOT EXISTS plan_executions (
  plan_id TEXT PRIMARY KEY,
  stage TEXT NOT NULL,
  progress_known INTEGER NOT NULL DEFAULT 0,
  overall_percent INTEGER NOT NULL DEFAULT 0,
  current_candidate_id TEXT NOT NULL DEFAULT '',
  bytes_downloaded INTEGER NOT NULL DEFAULT 0,
  bytes_total INTEGER NOT NULL DEFAULT 0,
  detail TEXT NOT NULL DEFAULT '',
  reboot_required INTEGER NOT NULL DEFAULT 0,
  reboot_boot_marker_ms INTEGER NOT NULL DEFAULT 0,
  restore_point_sequence INTEGER,
  backup_root TEXT NOT NULL DEFAULT '',
  mutation_started INTEGER NOT NULL DEFAULT 0,
  recovery_required INTEGER NOT NULL DEFAULT 0,
  failure_message TEXT NOT NULL DEFAULT '',
  started_unix_ms INTEGER NOT NULL,
  updated_unix_ms INTEGER NOT NULL,
  completed_unix_ms INTEGER,
  FOREIGN KEY(plan_id) REFERENCES plans(id)
);

CREATE TABLE IF NOT EXISTS driver_install_items (
  plan_id TEXT NOT NULL,
  candidate_id TEXT NOT NULL,
  update_id TEXT NOT NULL,
  revision INTEGER NOT NULL,
  instance_id TEXT NOT NULL,
  title TEXT NOT NULL,
  stage TEXT NOT NULL,
  progress_known INTEGER NOT NULL DEFAULT 0,
  progress_percent INTEGER NOT NULL DEFAULT 0,
  result_code TEXT NOT NULL DEFAULT '',
  hresult INTEGER NOT NULL DEFAULT 0,
  reboot_required INTEGER NOT NULL DEFAULT 0,
  verified INTEGER NOT NULL DEFAULT 0,
  before_driver_json TEXT NOT NULL DEFAULT '',
  after_driver_json TEXT NOT NULL DEFAULT '',
  before_problem_code INTEGER NOT NULL DEFAULT 0,
  after_problem_code INTEGER NOT NULL DEFAULT 0,
  backup_path TEXT NOT NULL DEFAULT '',
  detail TEXT NOT NULL DEFAULT '',
  updated_unix_ms INTEGER NOT NULL,
  PRIMARY KEY(plan_id, candidate_id),
  FOREIGN KEY(plan_id) REFERENCES plans(id)
);

CREATE TABLE IF NOT EXISTS execution_checkpoints (
  seq INTEGER PRIMARY KEY AUTOINCREMENT,
  plan_id TEXT NOT NULL,
  candidate_id TEXT NOT NULL DEFAULT '',
  checkpoint TEXT NOT NULL,
  detail_json TEXT NOT NULL DEFAULT '{}',
  created_unix_ms INTEGER NOT NULL,
  FOREIGN KEY(plan_id) REFERENCES plans(id)
);

CREATE TABLE IF NOT EXISTS recovery_records (
  seq INTEGER PRIMARY KEY AUTOINCREMENT,
  plan_id TEXT NOT NULL,
  severity TEXT NOT NULL,
  kind TEXT NOT NULL,
  summary TEXT NOT NULL,
  detail TEXT NOT NULL,
  restore_point_sequence INTEGER,
  backup_root TEXT NOT NULL DEFAULT '',
  created_unix_ms INTEGER NOT NULL,
  FOREIGN KEY(plan_id) REFERENCES plans(id)
);

CREATE INDEX IF NOT EXISTS idx_execution_updated ON plan_executions(updated_unix_ms DESC);
CREATE INDEX IF NOT EXISTS idx_install_items_plan ON driver_install_items(plan_id, candidate_id);
CREATE INDEX IF NOT EXISTS idx_checkpoints_plan ON execution_checkpoints(plan_id, seq);
CREATE INDEX IF NOT EXISTS idx_recovery_plan ON recovery_records(plan_id, seq DESC);
