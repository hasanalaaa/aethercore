# Phase 29 — Architecture: Enterprise Outputs (Signed Exports, Structured Logging, Automation Recipes)

Status: SEALED · Extends docs/phase28/ARCHITECTURE.md (CX-1…CX-7 remain binding)

## 1. EXPORT_V1 — signed journal export (T1)

`crates/persistence/src/export.rs`

- Canonical envelope: header `{schema:"aethercore.export.v1", generated_unix_ms,
  record_count, source_db_fingerprint}` + ordered records + `digest` + honest
  `signed:false` when no key configured.
- Chain: `record_hash = sha256(kind || ordinal_le64 || canonical_json(payload))`,
  `chain_hash[i] = sha256(prev_chain ‖ record_hash)`, genesis = 64×"0". Final chain
  value seals the file as `digest`; an empty range seals at genesis.
- Signature: OPTIONAL Ed25519 over the ASCII digest hex. Keys come ONLY from
  `aetherctl keys generate --out <path>` (explicit owner action; seed from OS CSPRNG,
  written 0600). The service signs only when `AETHERCORE_EXPORT_KEY` points at a valid
  64-hex seed file; otherwise the envelope ships digest-only with `signed:false`.
- Offline verifier `aetherctl export verify <file>` recomputes every hash and the full
  chain locally (zero service dependency). Typed failures name the exact break:
  `RecordHashMismatch{index}` · `ChainBreak{index}` · `RecordCountMismatch` ·
  `DigestMismatch{expected}` · `SignatureFlagInconsistent` · `BadSignature`.
- Trust model honestly bounded: the signature proves LOCAL TAMPER-EVIDENCE against a
  known public key. It does NOT prove identity of origin (no PKI/Web-of-trust), and it
  does not make the file a legal record.

Wire: request tag **89** `ExportJournalRequest`, response tag **51**
`ExportJournalResponse{envelope_json, record_count, signed}` — additive only.

## 2. Structured logging registry (T2)

`aethercore_diagnostics::events` — typed const table:
`startup.matrix_line · daemon.ready · daemon.stopping · care.step_started ·
care.step_finished · perf.sample_tick · ipc.session_opened · export.produced ·
log.rotated`.

`emit_structured(level, event, fields)` pins the stable JSON-LINES schema
`{"ts","level","event","fields"}`. Rotation from P28 applies to json mode unchanged
(5 MiB × keep files). Event names are additions-only; renames forbidden.

## 3. Automation recipes (T4, docs/phase29/recipes/)

- R1 nightly-maintenance.sh: detect → doctor → telemetry-once → export journal →
  verify; strict `set -euo pipefail`, P28 exit codes. PROVEN live ×2 on this Mac.
- R2 systemd timer pair (`OnCalendar`, `Persistent=true`) — static lint only;
  runtime NOT_EXECUTED here (QD-028-002).
- R3 GitHub Actions matrix job (linux/macos self-hosted) running R1 and uploading the
  export artifact — NOT_EXECUTED (no GH runner locally).
- R4 launchd nightly plist equivalent for macOS fleets.

## 4. Honest limits / debt

- Engine-live lanes (Postgres/MySQL connections) were not part of this phase.
- QD-029-001 HSM/KMS-backed keys out of scope (local file seeds only).
- QD-029-002 long-term archival format governance for EXPORT_V1 open.
- QD-029-003 SBOM certification path open (inventory is CycloneDX-SHAPED, explicitly
  labeled "not a certified SBOM").
- D-029 parked: remote read-only fleet query protocol — deferred with rationale
  (needs auth design beyond local trust boundary).
