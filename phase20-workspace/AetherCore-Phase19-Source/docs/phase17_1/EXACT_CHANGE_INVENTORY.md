# Phase 17.1 Exact Change Inventory

Generated against the delivered Phase 17 source baseline. This inventory is path-exact; byte hashes are recorded separately by the source manifest and delivery SHA-256 inventory.

- Added: **20** files
- Modified: **17** files
- Deleted: **0** files

## Added

- `crates/pc-intelligence/src/fingerprint.rs`
- `crates/pc-intelligence/src/lifecycle.rs`
- `crates/pc-intelligence/src/run_ownership.rs`
- `crates/persistence/migrations/0011_phase17_1_intelligence_integrity.sql`
- `docs/phase17_1/ADVERSARIAL_REVIEW.md`
- `docs/phase17_1/CANONICAL_STATE_FINGERPRINT.md`
- `docs/phase17_1/CORRELATION_STRENGTH.md`
- `docs/phase17_1/EXACT_CHANGE_INVENTORY.md`
- `docs/phase17_1/FINDING_LIFECYCLE_INTEGRITY.md`
- `docs/phase17_1/ISSUE_LEDGER.json`
- `docs/phase17_1/LOCALIZATION_EVIDENCE.md`
- `docs/phase17_1/MIGRATION_EVIDENCE.md`
- `docs/phase17_1/QUALIFICATION_DEBT_STATUS.md`
- `docs/phase17_1/REGRESSION_TEST_INVENTORY.md`
- `docs/phase17_1/RESOLUTION_AUTHORITY_MODEL.md`
- `docs/phase17_1/SCAN_GENERATION_OWNERSHIP.md`
- `docs/phase17_1/TEST_EVIDENCE.md`
- `reports/PHASE_17_1_INTEGRITY_EVIDENCE.json`
- `scripts/phase17_1-integrity-audit.py`
- `tests/fixtures/phase17_1/integrity_scenarios.json`

## Modified

- `MANIFEST.sha256`
- `QUALIFICATION_DEBT.json`
- `apps/ui/src/features/intelligence/FindingCard.svelte`
- `apps/ui/src/lib/contracts.ts`
- `apps/ui/src/lib/i18n/catalog.ar.ts`
- `apps/ui/src/lib/i18n/catalog.en.ts`
- `crates/contracts/proto/intelligence.proto`
- `crates/pc-intelligence/src/coordinator.rs`
- `crates/pc-intelligence/src/lib.rs`
- `crates/pc-intelligence/src/model.rs`
- `crates/pc-intelligence/src/normalize.rs`
- `crates/pc-intelligence/src/rules.rs`
- `crates/pc-intelligence/tests/scenarios.rs`
- `crates/persistence/src/lib.rs`
- `reports/PHASE_17_STATIC_EVIDENCE.json`
- `scripts/phase17-intelligence-audit.py`
- `services/maintenance-service/src/protocol.rs`

## Deleted

- None

## Scope statement

Changes are restricted to Phase 17.1 integrity semantics: Finding lifecycle/resolution truth, canonical state fingerprinting, generation-bound scan ownership, correlation precision/explainability, additive persistence/IPC/UI representation, deterministic regression fixtures/audits, localization, and qualification-debt scenario refinement. No Phase 18 driver-authority provider or future product-domain implementation is included.
