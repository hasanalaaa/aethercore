# Update State Machine

The typed authority states are `Discovered → Validated → Downloading → Downloaded → Verified → Staged → ReadyToApply → Applying → Applied|RebootRequired`. Failure paths are `RollbackRequired → RollingBack → RolledBack`, or `Failed`; cancellation is explicit and must be re-planned before apply. Illegal transitions reject.

Each transaction has a UUID, source/target identities, channel, package digest, state sequence, timestamps and non-secret error code. The existing persistence execution guard and mutation supervisor provide durable restart recovery and one active apply per installation. A restart reconstructs a non-actionable verified ticket until it is revalidated.
