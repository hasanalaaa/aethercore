# Retired audits

Phase-scoped audits whose verdict no CI job or release gate consumes, moved here in P76 by owner
decision (`DBT-P75-077`, LEDGER §1). They are kept, not deleted, and not re-baselined: each file is
the script as it stood on `main` at `1f8aff9`, except one import path in
`phase19-windows-repair-audit.py` (the shared `gate_reader` stays in `scripts/`). Each still runs
from the workspace root:

```bash
python3 retired-audits/<name>.py
```

This directory sits at the same depth as `scripts/`, so every script's
`ROOT = Path(__file__).resolve().parents[1]` still resolves to the workspace.

| script | why it is retired | red on `main` at `1f8aff9` (measured P76) |
|---|---|---|
| `phase18_1-driver-truth-audit.py` | Phase 18.1 source assertions; no caller | `P18.1-TRUTH-003`, `P18.1-COVERAGE-002`, `P18.1-COVERAGE-003`, `P18.1-MACHINE-001` |
| `phase19-windows-repair-audit.py` | Phase 19 source assertions; its only CI use is `scripts/test_gate_readers.py`, which checks that its reader fails closed and ignores its verdict (the case follows the file) | `P19-PLAN-002` |
| `phase35-gd-proofs.py` | Phase 35 GD-1..GD-10 proofs; no caller | `GD-6` (offline bundle) |

## Named in the decision and not moved

These were listed with the retired audits, but a gate that is still live runs them, so moving
them would remove or break that gate. They stay in `scripts/`, and `DBT-P75-077` stays open for
them.

* `scripts/phase35-adversarial-audit.py` — `.github/workflows/release.yml:68` runs it as the
  "Phase 35 release authority and adversarial metadata gate". Red on `main`:
  `p35-inherits-p34-930`, `p35-version-consistency`, `p35-update-product-match`,
  `p35-no-secret-regression`. Retiring it means removing that release step.
* `p27-wirefreeze:*` (`scripts/phase27-adversarial-audit.py`) and `p28-deps:allowlist-only`,
  `p28-renderer-sealed-snapshot-available` (`scripts/phase28-adversarial-audit.py`) — checks,
  not scripts. Both scripts are imported in-process by the `phase29`…`phase34` chain, which
  inherits and filters their findings, and `scripts/test_gate_readers.py` runs `phase29` in CI.
