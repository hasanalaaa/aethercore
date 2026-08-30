# Correlation Strength — Phase 17.1

`CorrelationStrength` is a deterministic categorical reasoning dimension: `Weak`, `Moderate`, `Strong`. It is separate from severity and confidence.

For WHEA + crash, broad multi-day coincidence never creates a combined Finding. Candidate pairing is bounded to 30 minutes; 10 minutes is the tight window. Strong evidence requires either fatal WHEA before a crash in the tight window or repeated distinct WHEA-before-crash relationships in tight clusters. A single close relationship is Moderate. Weak 10–30 minute coincidence is retained only as internal reasoning and is not promoted to a combined Finding. Corrected WHEA and hardware evidence occurring after the crash are explicit conflicts; after-crash evidence cannot create Strong correlation.

A Strong combined result is at most `High` severity / `High` confidence and still does not assert causation. Driver-change correlation is bounded to 24 hours, emits `Moderate` / `Medium`, and always records that crash evidence lacks explicit module attribution.

Each combined Finding carries exact contributing fact IDs, time distance, shared scope/category, rule ID/version, rationale key and conflict/uncertainty keys.
