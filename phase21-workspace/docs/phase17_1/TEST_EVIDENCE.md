# Phase 17.1 Intelligence Integrity Test Evidence

## Claim boundary

This document records only evidence executable on the current non-Windows host. It does not convert deferred native qualification into PASS.

## Executed source/static gates

- `scripts/phase17_1-integrity-audit.py`: 22/22 executed checks PASS; Rust, installed Svelte runtime check, and Windows-native qualification are explicitly `NOT_EXECUTED`.
- `scripts/phase17-intelligence-audit.py`: 24/24 executed checks PASS; the same unavailable build/native categories remain `NOT_EXECUTED`.
- `scripts/static_validate.py`: 342/342 checks PASS.
- `scripts/test-phase12-localization.py` and `scripts/phase12-localization-audit.py`: 32/32 localization checks PASS with EN/AR catalog parity at 1218/1218 entries.
- SQLite migration execution: migrations 1 through 11 apply successfully in an in-memory database; the dedicated v10→v11 fixture preserves the prior Finding row, Finding ID, JSON payload, and `Ignored` lifecycle.

## Phase 17.1 deterministic regression coverage

### Finding lifecycle

Source-backed tests cover:

- authoritative successful reevaluation resolving an absent issue;
- owner collector failed/unavailable/timed out preventing resolution;
- `CompletedWithWarnings` preventing automatic resolution;
- cancellation before the owner domain preventing resolution;
- successful owner-domain reevaluation remaining authoritative when a later unrelated stage is cancelled;
- recurrence from `Resolved` to `Recurred`;
- preservation of `Ignored` state;
- remediation suppression when an old Finding is carried as not reverified.

### Canonical machine-state fingerprint

Source-backed tests cover:

- timestamp, evidence text, and input-order independence;
- driver-version value change changing the fingerprint;
- storage degradation changing the fingerprint;
- event resource identities containing collection timestamps not changing state identity by themselves;
- normal storage-temperature fluctuation and random cleanup-candidate churn not changing the machine-state fingerprint;
- diagnostic limitations not becoming state-bearing fingerprint inputs.

### Generation ownership

The run-ownership tests use direct generation transitions rather than arbitrary timing sleeps. Coverage includes:

- rapid restart after terminal publication;
- stale cleanup from generation N after generation N+1 starts;
- stale cancellation by old scan ID;
- cancellation of the current generation;
- duplicate terminal cleanup;
- late old-worker ownership checks.

### Correlation precision

Source-backed tests cover:

- fatal WHEA 42 seconds before a crash producing a strong but non-causal-claim correlation;
- corrected WHEA several days before a crash producing no combined high-severity Finding;
- repeated tight WHEA/crash clustering increasing correlation strength;
- crash without WHEA producing no fabricated hardware correlation;
- WHEA without crash remaining an independent hardware Finding;
- hardware evidence after a crash recording conflicting ordering and preventing strong causal inference;
- stale evidence not dominating current correlation.

## Runtime/build evidence not executed

The current host has no Cargo/Rust toolchain, no installed `svelte-check` dependency, and is not a Windows qualification host. Therefore Rust compile/test execution, Svelte runtime/type checking, WebView/Tauri behavior, Windows APIs, native IPC timing, and physical hardware scenarios remain `NOT_EXECUTED` and are retained in `QUALIFICATION_DEBT.json`.
