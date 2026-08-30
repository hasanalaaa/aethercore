# Exact Phase 18 → 18.1 Change Inventory

Baseline: `AetherCore-Phase18-Source`
Working source: `AetherCore-Phase18.1-Source`

This source-local inventory is exact for all non-recursive source paths. It deliberately excludes only `MANIFEST.sha256` and this inventory file itself. The Master Delivery `EXACT_CHANGE_INVENTORY.json` is authoritative for those two generated paths as well and includes old/new SHA-256 plus byte sizes.

Non-recursive changed paths: **53** (`ADDED` 27, `MODIFIED` 26, `DELETED` 0).

| Change | Path |
|---|---|
| ADDED | `DRIVER_PROVIDER_COVERAGE.json` |
| ADDED | `PHASE_18_1_STATUS.json` |
| MODIFIED | `QUALIFICATION_DEBT.json` |
| MODIFIED | `apps/ui/src/features/drivers/DriversPage.svelte` |
| MODIFIED | `apps/ui/src/lib/contracts.ts` |
| MODIFIED | `apps/ui/src/lib/i18n/catalog.ar.ts` |
| MODIFIED | `apps/ui/src/lib/i18n/catalog.en.ts` |
| MODIFIED | `apps/ui/src/lib/i18n/semantic.ts` |
| MODIFIED | `apps/ui/src/platform/stream-state.ts` |
| MODIFIED | `crates/contracts/proto/drivers.proto` |
| MODIFIED | `crates/driver-acquisition/src/lib.rs` |
| MODIFIED | `crates/driver-authority/Cargo.toml` |
| MODIFIED | `crates/driver-authority/src/lib.rs` |
| ADDED | `crates/driver-authority/src/provider_registry.rs` |
| ADDED | `crates/driver-authority/src/truth.rs` |
| MODIFIED | `crates/driver-hub/src/lib.rs` |
| MODIFIED | `crates/pc-intelligence/src/fingerprint.rs` |
| MODIFIED | `crates/pc-intelligence/src/lifecycle.rs` |
| MODIFIED | `crates/pc-intelligence/src/model.rs` |
| MODIFIED | `crates/pc-intelligence/src/normalize.rs` |
| MODIFIED | `crates/pc-intelligence/src/rules.rs` |
| MODIFIED | `crates/update-engine/src/coordinator.rs` |
| MODIFIED | `crates/update-engine/src/lib.rs` |
| MODIFIED | `crates/update-engine/src/platform.rs` |
| ADDED | `docs/phase18_1/DEEP_SCAN_REGRESSION_EVIDENCE.md` |
| ADDED | `docs/phase18_1/DEVICE_AUTHORITY_COVERAGE.md` |
| ADDED | `docs/phase18_1/DRIVER_TRUTH_MODEL.md` |
| ADDED | `docs/phase18_1/FINAL_ADVERSARIAL_REVIEW.md` |
| ADDED | `docs/phase18_1/GPU_MANAGEMENT_VS_UPDATE.md` |
| ADDED | `docs/phase18_1/ISSUE_LEDGER.md` |
| ADDED | `docs/phase18_1/OVERRIDE_SCOPING.md` |
| ADDED | `docs/phase18_1/PHASE_18_1_VALIDATION.md` |
| ADDED | `docs/phase18_1/PROVIDER_REGISTRY_COVERAGE.md` |
| ADDED | `docs/phase18_1/RANKING_POLICY.md` |
| ADDED | `docs/phase18_1/SIGNER_PUBLISHER_TRUST.md` |
| ADDED | `docs/phase18_1/SYNTHETIC_DRIVER_LAB_EXECUTION.md` |
| ADDED | `docs/phase18_1/UI_EN_AR_EVIDENCE.md` |
| ADDED | `reports/PHASE_15_SECURITY_REGRESSION.json` |
| MODIFIED | `reports/PHASE_17_1_INTEGRITY_EVIDENCE.json` |
| MODIFIED | `reports/PHASE_17_STATIC_EVIDENCE.json` |
| ADDED | `reports/PHASE_18_1_DRIVER_TRUTH_AUDIT.json` |
| ADDED | `reports/PHASE_18_1_REGRESSION_SUMMARY.json` |
| ADDED | `reports/PHASE_18_1_SOURCE_INTEGRITY.json` |
| ADDED | `reports/PHASE_18_1_STATIC_VALIDATION.json` |
| ADDED | `reports/PHASE_18_DRIVER_AUTHORITY_AUDIT.json` |
| MODIFIED | `scripts/phase18-driver-authority-audit.py` |
| MODIFIED | `scripts/phase18-windows-qualification.ps1` |
| ADDED | `scripts/phase18_1-driver-truth-audit.py` |
| MODIFIED | `services/maintenance-service/src/protocol.rs` |
| MODIFIED | `tests/fixtures/phase18/scenarios.json` |
| ADDED | `tests/fixtures/phase18_1/execution-mapping.json` |
| ADDED | `tests/fixtures/phase18_1/scenarios.json` |
| ADDED | `tests/fixtures/phase18_1/update-status-truth-matrix.json` |
