# Canonical Machine-State Fingerprint

The Phase 17 identity-only fingerprint is replaced by a deterministic canonical state fingerprint.

Each eligible fact contributes: domain, source, privacy-safe resource identity, payload kind, and explicitly selected state-bearing payload values. Entries are sorted before SHA-256 hashing. State-bearing examples include installed/target driver versions and stable candidate identity, device problem state, Windows integrity result, storage reliability/error/wear state plus threshold-crossing thermal attention, memory-pressure bucket, startup counts, update release identity/state, and recovery readiness.

Observation metadata is excluded: scan ID, observation/evidence timestamps, evidence prose, display names, collector duration/progress, IPC sequence, and diagnostic-limit details. Historical `DriverChange` is correlation evidence and is not current-state input. Cleanup/cache candidates are also excluded because their UUIDs and reclaimable-byte churn are transient optimization data rather than durable machine-health state. WHEA/crash resource IDs embed event timestamps in Phase 17, so canonicalization deliberately replaces those volatile IDs with stable event-type identities; semantic type/count and payload still participate.

No raw username, account ID, absolute user path, email, or hardware serial is required. `ResourceRef::private` hashes private device identities before this layer; diagnostic-limit details are excluded entirely.
