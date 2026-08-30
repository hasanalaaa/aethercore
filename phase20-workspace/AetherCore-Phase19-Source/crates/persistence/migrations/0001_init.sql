CREATE TABLE IF NOT EXISTS plans (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL,
  state TEXT NOT NULL,
  digest TEXT NOT NULL UNIQUE,
  risk TEXT NOT NULL,
  immutable_json TEXT NOT NULL,
  created_unix_ms INTEGER NOT NULL,
  updated_unix_ms INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS plan_events (
  seq INTEGER PRIMARY KEY AUTOINCREMENT,
  plan_id TEXT NOT NULL,
  from_state TEXT,
  to_state TEXT NOT NULL,
  event_kind TEXT NOT NULL,
  detail TEXT NOT NULL,
  created_unix_ms INTEGER NOT NULL,
  FOREIGN KEY(plan_id) REFERENCES plans(id)
);

CREATE TABLE IF NOT EXISTS authorization_challenges (
  challenge TEXT PRIMARY KEY,
  plan_id TEXT NOT NULL,
  digest TEXT NOT NULL,
  expires_unix_ms INTEGER NOT NULL,
  consumed_unix_ms INTEGER,
  FOREIGN KEY(plan_id) REFERENCES plans(id)
);

CREATE TABLE IF NOT EXISTS authorization_grants (
  grant_id TEXT PRIMARY KEY,
  plan_id TEXT NOT NULL,
  digest TEXT NOT NULL,
  expires_unix_ms INTEGER NOT NULL,
  created_unix_ms INTEGER NOT NULL,
  FOREIGN KEY(plan_id) REFERENCES plans(id)
);

CREATE INDEX IF NOT EXISTS idx_plan_events_plan ON plan_events(plan_id, seq);
CREATE INDEX IF NOT EXISTS idx_grants_plan ON authorization_grants(plan_id, expires_unix_ms);
