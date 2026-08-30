# Phase 17.1 Migration Evidence

## Schema transition

Phase 17.1 adds migration `0011_phase17_1_intelligence_integrity.sql` and advances the persistence migration set from schema v10 to v11.

The migration is additive. It does not recreate or delete `intelligence_findings`, does not change Finding IDs, and does not remove existing lifecycle, evidence, override, scan-history, or sealed-plan data.

Added persisted fields:

- `verification_status TEXT NOT NULL DEFAULT 'NotRechecked'`
- `resolved_at_unix_ms INTEGER`
- `resolution_scan_id TEXT NOT NULL DEFAULT ''`
- `resolution_reason_key TEXT NOT NULL DEFAULT ''`

An index is added for the new verification-state access pattern.

## Backward compatibility proof

The Phase 17.1 integrity audit constructs a schema-v10 database, inserts a representative pre-17.1 Finding with:

- stable Finding ID `finding-1`;
- lifecycle `Ignored`;
- retained Finding JSON;

and then applies migration 0011.

Observed row after migration:

`('finding-1', 'Ignored', '{"id":"finding-1","ignored":true}', 'NotRechecked', None, '', '')`

This proves under the executed SQLite fixture that the old row, ID, ignored lifecycle, and JSON are retained, while new verification/resolution columns receive deterministic compatibility defaults.

## Semantic migration policy

Legacy records are not silently declared current or resolved. `NotRechecked` is intentionally the safe default because Phase 17 history predates explicit resolution-authority evidence.

Windows deployed-WAL upgrade behavior remains a native qualification debt item (`P17-QD-011`) rather than an inferred PASS.
