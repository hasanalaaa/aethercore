-- Phase 15: rollback-resistant update metadata floor and restart-safe machine mutation guard.
CREATE TABLE IF NOT EXISTS update_manifest_floor (
    channel TEXT PRIMARY KEY NOT NULL CHECK(channel IN ('stable','beta')),
    highest_sequence INTEGER NOT NULL CHECK(highest_sequence >= 0),
    generated_unix_ms INTEGER NOT NULL,
    manifest_sha256 TEXT NOT NULL CHECK(length(manifest_sha256) = 64),
    updated_unix_ms INTEGER NOT NULL
);

-- Exactly one update execution may span a service restart. The row carries no URL, command line,
-- consent secret, or arbitrary executable authority; it only re-establishes the Update mutation
-- lease for the already verified staged artifact until the elevated fixed broker reports completion.
CREATE TABLE IF NOT EXISTS active_update_execution (
    slot INTEGER PRIMARY KEY NOT NULL CHECK(slot = 1),
    ticket_id TEXT NOT NULL UNIQUE,
    owner_principal_key TEXT NOT NULL,
    release_id TEXT NOT NULL,
    release_version TEXT NOT NULL,
    channel TEXT NOT NULL CHECK(channel IN ('stable','beta')),
    notes_message_key TEXT NOT NULL,
    minimum_windows_build INTEGER NOT NULL CHECK(minimum_windows_build > 0),
    staged_path TEXT NOT NULL,
    expected_sha256 TEXT NOT NULL CHECK(length(expected_sha256) = 64),
    expected_size INTEGER NOT NULL CHECK(expected_size > 0),
    expires_unix_ms INTEGER NOT NULL,
    created_unix_ms INTEGER NOT NULL
);
