# Phase 15 Deliverables — Secure In-App Update & Cryptographic Diagnostic Export

## Secure update orchestration

- `crates/update-engine` — signed-manifest trust, channel policy, rollback/equivocation floor, service-owned staging, hash/AuthentiCode/hash validation, one-shot consent intents, durable execution guard and Update mutation orchestration.
- `crates/update-download` — non-elevated bounded HTTPS retrieval with redirects disabled, finite timeouts, streaming SHA-256 and temporary-file cleanup.
- `apps/update-broker` — fixed elevated broker accepting only intent ID + allowlisted locale, holding the cross-process Windows mutation mutex and executing only the service-issued staged Burn path with no arbitrary arguments.
- `tools/update-manifest` — offline release-manifest signing tool.
- `release/update-trust.template.json` plus trust generation/validation scripts — disabled repository template and externally provisioned production trust.
- `crates/persistence/migrations/0009_phase15_update.sql` — rollback floor and restart-safe single active-update execution guard.

## Privacy-first support export

- `crates/support-bundle` — pre-preview privacy sanitization, bounded retained-object quotas, deterministic USTAR generation, per-file SHA-256 manifest/root hash, installation-local Ed25519 proof and independent-fingerprint verification.
- `services/maintenance-service/src/support.rs` — allowlisted product/diagnostic/owner-journal/scheduler evidence only; no generic log/file zipper.
- `tools/support-bundle-verify` — offline verifier requiring an independently supplied installation public-key fingerprint.
- Non-elevated desktop export path — bounded owner-scoped chunk reads, no service destination path, no overwrite, whole-archive hash verification, strong proof verification before rename, and staging discard on success/failure.

## IPC, UI and installer integration

- `update.proto` and `support_bundle.proto` plus typed `operations.proto/events.proto` integration.
- Persistent IPC v7 update/support snapshots and events remain principal-scoped.
- `SystemCarePanel.svelte` and controller provide Stable/Beta update controls plus support preview/redaction inspection/export.
- EN/AR typed catalogs include update/support lifecycle, privacy and fault messages.
- WiX packages the fixed update broker and `update-trust.json`; Burn/MSI remains the single installer authority.

## Verification

- `scripts/phase15-security-audit.py/.ps1`
- `scripts/phase15-crypto-tests.ps1`
- `scripts/verify-phase15.ps1`
- inherited Phase 0–14 gates, localization checks, locked Rust/UI builds and release/signing/lifecycle gates.

Key Phase 15 regressions include signed-manifest rollback/equivocation rejection, minimum-Windows-build filtering, bounded/truncated upload rejection, one-shot claim linearization, cancel/expiry recovery, exact restart identity restoration, Update mutation exclusion, support TAR/hash/root/signature tamper detection, embedded SID/email redaction, retained-object quota release and independent-fingerprint enforcement.

See `docs/SECURE_UPDATE_AND_SUPPORT_EXPORT.md` and ADR 0017 for the canonical trust model. Final source/package qualification evidence is recorded in `PHASE_15_VALIDATION_SUMMARY.md` and the external Phase 15 validation summary generated at seal time.
