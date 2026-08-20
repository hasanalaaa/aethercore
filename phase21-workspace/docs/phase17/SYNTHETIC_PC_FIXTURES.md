# Phase 17 Synthetic PC Fixtures

Machine-readable fixture inventory: `tests/fixtures/phase17/scenarios.json`.

Executable Rust scenario tests: `crates/pc-intelligence/tests/scenarios.rs`.

Coverage includes healthy/no-action behavior, missing driver, explicit storage reliability error, recent driver-change/crash correlation, Windows integrity failure, startup threshold, diagnostic limitation, memory and cleanup boundary values, stale crash rejection, future-dated evidence rejection, unrelated updater failure rejection, and deterministic remediation-plan sealing.

These tests are source-complete but were not executed with Cargo on the current host because the Rust toolchain is absent. This is recorded as `NOT_EXECUTED`, not PASS.
