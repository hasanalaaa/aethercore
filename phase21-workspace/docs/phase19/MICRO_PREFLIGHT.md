# Phase 19 Micro-Preflight — Delivery Truth

## Baseline

Phase 19 starts from the supplied Phase 18.1 source bytes. No prior phase is restarted.

- Supplied outer delivery ZIP SHA-256: `b98bace61ef8e14894c0aecbda753c892c5714435fee35c3ecb8bfd3547c417e`
- Actual embedded Phase 18.1 Source ZIP SHA-256: `e7116e6dbd9f5054e1893970eada0b00b922ebaa64eabaa90e157ae2beac8d45`
- Phase 19 directive SHA-256: `e4069fde7f31d922312038cf75793d65f00eb5e8795865a2db8a969fe0763d98`
- Apple Design skill SHA-256: `11840b24a11d7f94f39c6aaab074750ae4e4de4ef54ee4b1dd97e16ebd485e61`

The source ZIP hash is recomputed from the delivered bytes. The mismatch with the stale external narrative is recorded as a delivery-reporting defect only; it is not treated as a Phase 18.1 driver-logic failure.

## Evidence-integrity policy

All Phase 19 hashes are generated from final bytes. `scripts/static_validate.py` is read-only by default and writes a report only when an explicit `--output` path is supplied. `scripts/phase19-windows-repair-audit.py` is read-only and emits JSON to stdout, allowing delivery tooling to capture evidence without modifying source during verification.
