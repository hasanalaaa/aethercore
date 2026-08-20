# Phase 17.1 Intelligence Integrity Regression Suite

## Rust source tests
- `lifecycle.rs`: completed authority, failed/timeout/unavailable/permission/partial, cancelled/not-started, unrelated cancellation, durable resolution evidence, recurrence, ignored state.
- `fingerprint.rs`: timestamp/order/evidence independence, driver-version change, storage degradation, event volatility/privacy exclusion.
- `run_ownership.rs`: rapid restart, stale cancel, current cancel, duplicate cleanup, late worker.
- `rules.rs`: fatal-near, corrected-days, repeated tight cluster, crash-only, WHEA-only, conflicting order, stale evidence.
- `tests/scenarios.rs`: Phase 17 product scenarios retained with correlation expectations updated to v2 semantics.

## Environment-supported executable audit
`scripts/phase17_1-integrity-audit.py` validates source invariants, additive IPC fields, EN/AR parity, fixture coverage, SQLite v10→v11 preservation, qualification-debt continuity, and absence of the Phase 17 false-resolution method.

Cargo/Rust and Svelte runtime tests remain `NOT_EXECUTED` on hosts lacking those installed toolchains; that status is not converted to PASS.
