-- DBT-P46-B16. plan_executions.bytes_downloaded/bytes_total are
-- NOT NULL DEFAULT 0, so a download whose WUA byte read failed and a download
-- that has genuinely transferred zero bytes are stored identically. The wire
-- now carries has_bytes_downloaded/has_bytes_total (drivers.proto 25/26), and
-- those flags have to survive a service restart, not just live telemetry.
--
-- Additive ALTER rather than a table rebuild: SQLite cannot drop NOT NULL in
-- place, and rebuilding a table that carries in-flight driver-install state is
-- a materially riskier operation than adding two columns.
--
-- Existing rows default to 0 = "not determined", which is the honest reading:
-- for a row written before this migration there is no record of whether its
-- stored zero was measured or defaulted, and claiming otherwise would be the
-- same confident-zero this fix exists to remove.
ALTER TABLE plan_executions ADD COLUMN bytes_downloaded_known INTEGER NOT NULL DEFAULT 0;
ALTER TABLE plan_executions ADD COLUMN bytes_total_known INTEGER NOT NULL DEFAULT 0;
