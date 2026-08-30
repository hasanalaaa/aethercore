# ADR-0003: SQLite WAL journal

**Status:** Accepted

SQLite is embedded in the maintenance service. WAL mode, full synchronous durability, and transactional plan/event writes provide a restart-safe local journal without a separate database process.
