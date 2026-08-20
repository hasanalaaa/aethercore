# Phase 17.1 Final Adversarial Review

## Finding truth attacks

### Attempt: resolve an old Finding when its owner collector fails

Result: blocked. Failed, timed-out, unavailable, permission-denied and warning/partial owner states do not grant resolution authority. The persisted issue remains unresolved and is surfaced as verification unavailable without an executable remediation candidate.

### Attempt: resolve an old Finding when the scan is cancelled before its owner runs

Result: blocked. Missing/pending/cancelled owner scope produces `NotRechecked`, not `Resolved`.

### Attempt: prevent legitimate resolution merely because a later unrelated stage is cancelled

Result: blocked. A domain that already completed authoritatively may independently establish resolution; unrelated later cancellation does not revoke evidence already obtained.

### Attempt: turn recurrence into a fresh duplicate

Result: blocked. Stable Finding identity is preserved; a previously resolved issue returning transitions to `Recurred`.

## Machine-state attacks

### Attempt: keep the fingerprint unchanged after meaningful driver/storage state changes

Result: blocked. Canonical state contains normalized state-bearing values including installed/target versions and storage warning/error/wear semantics.

### Attempt: change the fingerprint by changing only timestamps, input ordering, scan IDs, event collection identity or cleanup UUIDs

Result: blocked by canonical exclusion/normalization. Event timestamps and observation metadata are not state-bearing; cache/cleanup churn is excluded; normal storage temperature movement is represented only when it crosses the documented attention boundary.

### Privacy attack: require raw user path/account identity for the hash

Result: blocked by design. Diagnostic limitations and raw evidence prose are excluded; canonical resource identity uses existing privacy-safe resource identities or non-sensitive semantic resource categories.

## Generation ownership attacks

### Attempt: old generation A clears generation B cancellation state after B starts

Result: blocked. Cleanup is compare-by-`scan_id + generation` and only succeeds for the current owner.

### Attempt: stale scan ID cancels the new generation

Result: blocked. Cancellation targets the authoritative current run identity only.

### Attempt: late old worker or duplicate terminal signal mutates current state

Result: blocked by ownership checks on shared snapshot mutation and idempotent owner clearing.

## Correlation attacks

### Attempt: corrected WHEA several days before an unrelated crash becomes a Critical combined Finding

Result: blocked. The former broad seven-day coincidence path is removed. Weak/out-of-window coincidence does not produce the combined hardware-crash Finding.

### Attempt: claim causation when hardware evidence occurred after the crash

Result: blocked. Ordering is represented explicitly; after-crash evidence contributes conflicting evidence and cannot establish strong pre-crash causal sequence.

### Attempt: inflate strong correlation from a single corrected event

Result: blocked. Strong correlation is constrained to fatal pre-crash evidence in the tight window or repeated causal tight clusters. Even strong correlation is reported as correlation with uncertainty, not proof that hardware caused the crash.

## Legacy enterprise invariant attack

### Attempt: introduce a production panic shortcut in the new correlation branch

Initial Omega execution detected one source regression: the Weak match arm used `unreachable!()` after an earlier Weak-return guard. Although logically filtered, this violated AetherCore's established total-production-path invariant. The shortcut was removed and replaced with a safe total fallback. The direct enterprise adversarial audit then returned 88/88 PASS.

## Residual claim boundary

No High/Critical source-level defect from the four Phase 17.1 integrity findings remains in the executed source/static review. Rust compilation, Svelte runtime checks and Windows-native adversarial scenarios remain unexecuted on this host and therefore are not represented as PASS.
