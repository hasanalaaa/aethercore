-- Exact optional domain result; old/unmeasured Care steps retain NULL.
-- TEXT preserves all u64 values beyond SQLite's signed INTEGER range.
ALTER TABLE care_steps ADD COLUMN actual_deleted_bytes TEXT;
