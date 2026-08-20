# Phase 17 Fact Schema

`SystemFact` is the stable normalized observation unit.

Required fields implemented in `crates/pc-intelligence/src/model.rs`:

- deterministic `id` derived from domain, source, privacy-safe resource identity and payload kind;
- `domain` as a closed Rust enum;
- explicit `source` attribution;
- typed `ResourceRef` with privacy-preserving stable ID support;
- `observed_unix_ms` and `Freshness`;
- independent `Confidence`;
- structured `FactPayload` enum, never an untyped JSON blob;
- one or more `EvidenceRef` records with fact linkage, kind, source, observation time and bounded technical value.

Current payload variants cover inventory summary, device health, driver update/change, Windows integrity, storage health, memory pressure, hardware events, crashes, startup footprint, cleanup opportunity, application update state, recovery readiness and diagnostic limitation.

Facts are intentionally ephemeral in the coordinator. The persisted scan snapshot contains the finding/evidence state needed for continuity instead of accumulating large raw native payloads.
