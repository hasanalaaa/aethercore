# Synthetic Driver Lab Execution Mapping

Phase 18.1 upgrades the D18 fixture suite from specification-only evidence to source-backed behavioral mapping.

- `tests/fixtures/phase18_1/scenarios.json` defines corrected D18-01 through D18-16 plus D18.1-GPU-01.
- `tests/fixtures/phase18_1/execution-mapping.json` maps every scenario to a concrete Rust test function.
- `tests/fixtures/phase18_1/update-status-truth-matrix.json` supplies an independently evaluated update-status truth matrix.
- `scripts/phase18_1-driver-truth-audit.py` verifies every mapped test exists and contains behavioral assertions, and independently evaluates the truth-matrix expectations against declared status rules.

Covered behavior includes healthy, missing, OEM vs generic, GPU authority, unsigned, wrong publisher, hostile redirect, plan substitution, offline, incomplete coverage, firmware, scoped ignore, rollback, reboot aggregation, and GPU utility-without-update regression.

Rust execution is not claimed on this host because Cargo/rustc are unavailable; the mapping and semantic audit are source-level proof only.
