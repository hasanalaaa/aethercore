# Scan Generation & Cancellation Ownership

`RunOwnership` owns the single authoritative active run as `(scan_id, generation, started_at, cancellation_token)`. Installing a run monotonically advances the generation. All snapshot mutation, terminal publication, cancellation and cleanup are compare-by-identity operations.

Invariant: at most one authoritative Deep Scan generation is active. If generation N publishes a terminal snapshot and N+1 starts before N's worker returns from cleanup, `clear_if_owner(N)` is a no-op. A cancellation request naming N cannot cancel N+1. Collector workers communicate only through their generation-local channel; late exit after the receiver is dropped cannot mutate shared scan state.

The tests use direct ownership transitions rather than sleeps, making the formerly timing-dependent race deterministic.
