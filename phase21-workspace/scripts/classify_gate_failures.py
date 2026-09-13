#!/usr/bin/env python3
"""Classify failing gate checks: is the TREE wrong, or is the CHECK wrong?

`DBT-P61-001` is 108 failing checks across six gates. The two halves have
opposite cures - a tree failure is fixed by changing the product, a check
failure by changing the gate - so nothing should be fixed before it is sorted.

This sorts the part that can be sorted mechanically. Every one of these gates
asserts by substring: a token is searched for in a source file. When a token is
absent, there are three distinct reasons, and only the third is the product's:

  FORMAT   the token matches once whitespace is removed from both sides. The
           construct is in the tree; the token was written before rustfmt broke
           the line. The gate is wrong.
  ABSENT   the token does not match even then. This is a *candidate* tree
           failure and needs reading: rustfmt also inserts trailing commas when
           it wraps a signature, which whitespace removal alone cannot undo.
  (none)   the check failed on a predicate that is not a token search - a count,
           a line budget, a parse, an ordering. Reported as UNCLASSIFIED rather
           than guessed at.

It runs each gate in-process with its token helper wrapped, so the verdicts come
from the gate's own sources and its own tokens, not from a re-implementation.
"""
from __future__ import annotations

import argparse
import contextlib
import io
import json
import re
import sys
from pathlib import Path

# No bytecode: `omega-evidence.py` runs each gate against a disposable clone and
# treats ANY new file in it as a source mutation, so a `__pycache__` entry written
# while this tool execs a gate would be reported as the gate rewriting the tree.
sys.dont_write_bytecode = True

ROOT = Path(__file__).resolve().parents[1]

# Each gate names its own token helper. `static_validate.py` records the check
# name at the call (`marker`); the other five decide it one level up (`check`).
GATES = {
    "static_validation": ("static_validate.py", "marker"),
    "phase13_reliability": ("phase13-reliability-audit.py", "has"),
    "phase14_scheduler": ("phase14-scheduler-audit.py", "has"),
    "phase15_security": ("phase15-security-audit.py", "has"),
    "zenith_recursive": ("zenith-recursive-audit.py", "has"),
    "enterprise_adversarial": ("enterprise-adversarial-audit.py", "has"),
}

PROLOGUE = '''
_TOKENS = {}
_PENDING = []


def _squash(text):
    return "".join(text.split())


def _install():
    g = globals()
    if "marker" in g:
        inner = g["marker"]

        def marker(name, text, required):
            for token in required:
                if token not in text:
                    _TOKENS.setdefault(name, []).append(
                        (token, _squash(token) in _squash(text))
                    )
            return inner(name, text, required)

        g["marker"] = marker
    if "has" in g and "check" in g:
        has_inner, check_inner = g["has"], g["check"]

        def has(text, *tokens):
            for token in tokens:
                if token not in text:
                    _PENDING.append((token, _squash(token) in _squash(text)))
            return has_inner(text, *tokens)

        def check(name, ok, *args, **kwargs):
            if _PENDING and not ok:
                _TOKENS.setdefault(name, []).extend(_PENDING)
            _PENDING.clear()
            return check_inner(name, ok, *args, **kwargs)

        g["has"], g["check"] = has, check


_install()
'''


def instrument(source: str) -> str:
    """Insert the wrapper after the gate's helpers and before its first check."""
    lines = source.split("\n")
    for index, line in enumerate(lines):
        if re.match(r"^(check\(|checks\[|parse_group\(|\w+\s*=\s*read\()", line):
            lines.insert(index, PROLOGUE)
            return "\n".join(lines)
    raise SystemExit(f"no insertion point found; the gate's shape changed")


def run(script: Path) -> tuple[dict, dict]:
    namespace = {"__file__": str(script), "__name__": "__main__"}
    argv = sys.argv
    sys.argv = [script.name]
    # Each gate prints its own report; swallow it so this tool's stdout stays
    # parseable. Its stderr is left alone - an unreadable source must still shout.
    try:
        code = compile(instrument(script.read_text(encoding="utf-8")), str(script), "exec")
        with contextlib.redirect_stdout(io.StringIO()):
            exec(code, namespace)  # noqa: S102 - running the gate is the point
    except SystemExit:
        pass
    finally:
        sys.argv = argv
    return namespace.get("checks", {}), namespace.get("_TOKENS", {})


def classify(script: Path) -> dict:
    checks, tokens = run(script)
    failed = sorted(
        name
        for name, value in checks.items()
        if not (value.get("ok") if isinstance(value, dict) else value)
    )
    rows = {}
    for name in failed:
        recorded = tokens.get(name, [])
        fmt = [token for token, squashed in recorded if squashed]
        absent = [token for token, squashed in recorded if not squashed]
        if not recorded:
            verdict = "UNCLASSIFIED"
        elif absent:
            verdict = "ABSENT" if not fmt else "MIXED"
        else:
            verdict = "FORMAT"
        rows[name] = {"verdict": verdict, "format": fmt, "absent": absent}
    return {"checks": len(checks), "failed": len(failed), "rows": rows}


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--json", action="store_true")
    ap.add_argument("--gate", help="Classify one gate instead of all of them.")
    args = ap.parse_args()

    report = {}
    for gate, (filename, _helper) in GATES.items():
        if args.gate and gate != args.gate:
            continue
        report[gate] = classify(ROOT / "scripts" / filename)

    if args.json:
        print(json.dumps(report, indent=2, sort_keys=True))
        return 0

    totals = {"FORMAT": 0, "ABSENT": 0, "MIXED": 0, "UNCLASSIFIED": 0}
    for gate, result in report.items():
        print(f"\n{gate}  {result['failed']} failed of {result['checks']}")
        for name, row in sorted(result["rows"].items()):
            totals[row["verdict"]] += 1
            print(f"  {row['verdict']:13} {name}")
            for token in row["absent"]:
                print(f"       absent: {token[:96]!r}")
    print("\n" + "-" * 60)
    for verdict, count in totals.items():
        print(f"  {verdict:13} {count}")
    print(f"  {'TOTAL':13} {sum(totals.values())}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
