# Retired audits

Phase-scoped audits whose verdict no CI job or release gate consumes, moved here in P76 by owner
decision (`DBT-P75-077`, LEDGER §1); `phase35-adversarial-audit.py` followed on 2026-10-06 when the
owner approved dropping the release step that ran it. They are kept, not deleted, and not
re-baselined: each file is the script as it stood on `main` (`1f8aff9`, and `5fdb97b` for the
Phase 35 audit), except one import path in `phase19-windows-repair-audit.py` and
`phase35-adversarial-audit.py` (the shared `gate_reader` stays in `scripts/`). Each still runs
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
| `phase35-adversarial-audit.py` | Phase 35 metadata audit; it ran in `release.yml` as a gate while red, so it protected nothing. The owner approved dropping that step on 2026-10-06 (`DBT-P75-077`, option A). The sibling step `cargo test -p aethercore-release-authority` stays | `p35-inherits-p34-930`, `p35-version-consistency`, `p35-update-product-match`, `p35-no-secret-regression` (re-run 2026-10-06, unchanged) |

## Named in the decision and not moved

These were listed with the retired audits, but they are imported in-process by a chain that CI
runs, so moving them would break it. They stay in `scripts/`. `DBT-P75-077` is closed for the
audits and the release step; these two sets of checks are recorded there as the residual.

* `p27-wirefreeze:*` (`scripts/phase27-adversarial-audit.py`) and `p28-deps:allowlist-only`,
  `p28-renderer-sealed-snapshot-available` (`scripts/phase28-adversarial-audit.py`) — checks,
  not scripts. Both scripts are imported in-process by the `phase29`…`phase34` chain, which
  inherits and filters their findings, and `scripts/test_gate_readers.py` runs `phase29` in CI.
