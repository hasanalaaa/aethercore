# Phase 29 — SCORECARD

| Gate | Criterion | Status |
|---|---|---|
| Baseline P28 audit | 546 checks, failures=[], PASS | ✅ |
| Baseline cargo ×2 | 409 passed / 0 failed identical | ✅ |
| Baseline verify28 full-tree | 918/919 (self-excluded), PASS | ✅ |
| T1 EXPORT_V1 | chain+digest+optional Ed25519; typed verifier failures; wire 89/51 additive | ✅ |
| T2 log registry | typed events table + stable JSON-LINES schema; rotation applies | ✅ |
| T3 CLI additions | export journal/verify · keys generate/fingerprint proven live | ✅ |
| GD tamper demo | flipped byte → `record_hash mismatch at record 1` verbatim | ✅ |
| T4 recipes R1–R4 | R1 live ×2 green; byte-stable non-volatile fields | ✅ |
| T5 SBOM | 579 components, determinism ×2 (`66794d61…`), honest label | ✅ |
| T6 audit superset | 608 checks PASS (>546) | ✅ |
| GH archive | `AetherCore-Phase29-Master-Delivery.zip` twice-identical SHA-256 + FINAL txt | ✅ |

## Honest NOT_EXECUTED
- GitHub Actions workflow execution (R3) — no GH runner locally.
- systemd runtime verification (R2) — macOS host.
- HSM/KMS signing — out of scope (QD-029-001).

## Debt delta
- NEW: QD-029-001, QD-029-002, QD-029-003.
- CLOSED: none. QD-026-003 remains closed from P28 evidence.
