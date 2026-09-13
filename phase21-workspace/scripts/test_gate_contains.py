#!/usr/bin/env python3
"""The gate token comparison ignores whitespace and nothing else. `DBT-P61-001`.

81 of the 108 failing checks were the gates spelling a construct that is in the
tree - `hardware_gate:IsolationGate` against a `rustfmt`-clean
`hardware_gate: IsolationGate`. `gate_reader.contains` drops whitespace from both
sides, once, for all six token gates.

A comparison that is widened to make failures go away is worth nothing unless it
can still fail. Both halves are asserted here against the **real tree**, through
the gate's **own** helper and the gate's **own** loaded sources:

  present    a token whose only difference from the tree is spacing must match
  absent     a token naming something that is not in the tree must NOT match,
             and neither must one whose characters are merely reordered

The absent case is not invented: `cancel_registered_io` is the identifier
`482d480` deliberately removed when Windows IPC moved to overlapped I/O, and it
is one of the 22 checks that still fail after this change - see
`docs/phase62/P62-GATE-CLASSIFICATION.md`.

Run: `python3 scripts/test_gate_contains.py`
"""
from __future__ import annotations

import contextlib
import io
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
from gate_reader import SourceReader, contains  # noqa: E402

read = SourceReader(ROOT).read

failures: list[str] = []


def expect(label: str, actual: bool, wanted: bool) -> None:
    ok = actual is wanted
    print(f"{'PASS' if ok else 'FAIL'}  {label}  -> {actual}")
    if not ok:
        failures.append(f"{label}: expected {wanted}, got {actual}")


def run_gate(filename: str) -> dict:
    """Exec a gate and hand back its namespace - its helpers and its sources."""
    script = ROOT / "scripts" / filename
    namespace = {"__file__": str(script), "__name__": "__main__"}
    argv, sys.argv = sys.argv, [script.name]
    try:
        code = compile(script.read_text(encoding="utf-8"), str(script), "exec")
        with contextlib.redirect_stdout(io.StringIO()):
            exec(code, namespace)  # noqa: S102 - running the gate is the point
    except SystemExit:
        pass
    finally:
        sys.argv = argv
    return namespace


def main() -> int:
    diag = read("crates/diagnostic-engine/src/lib.rs")
    ipc = read("crates/ipc/src/windows_impl.rs")

    # 1. The widening does real work: the gate's spelling is not in the tree
    #    literally, and is once the spaces are gone.
    expect("literal 'hardware_gate:IsolationGate' in diagnostic-engine",
           "hardware_gate:IsolationGate" in diag, False)
    expect("contains 'hardware_gate:IsolationGate'",
           contains(diag, "hardware_gate:IsolationGate"), True)

    # 2. It is not a blanket pass. An identifier that is not in the tree stays
    #    absent, spaces or no spaces.
    expect("contains 'hardware_gate:QuarantineGate' (invented)",
           contains(diag, "hardware_gate: QuarantineGate"), False)
    expect("contains 'cancel_registered_io(&self.peer_reader)' (removed by 482d480)",
           contains(ipc, "cancel_registered_io(&self.peer_reader)"), False)

    # 3. Order still matters - squashing removes whitespace, it does not sort.
    expect("contains 'IsolationGate:hardware_gate' (reordered)",
           contains(diag, "IsolationGate: hardware_gate"), False)

    # 4. A trailing comma is not whitespace. This is why the classification
    #    separates ABSENT from FORMAT and why those checks were read by hand
    #    rather than swept up by this comparison.
    expect("contains 'Active, Committed, Revoked }' spelled without the last comma",
           contains("enum E {\n    Active,\n    Committed,\n    Revoked,\n}",
                    "{ Active, Committed, Revoked }"), False)

    # 5. End to end: the gate's own `has`, over the source the gate itself read.
    gate = run_gate("phase13-reliability-audit.py")
    gate_has, gate_diag = gate["has"], gate["diag"]
    expect("phase13 gate has(diag, 'hardware_gate:IsolationGate')",
           gate_has(gate_diag, "hardware_gate:IsolationGate"), True)
    expect("phase13 gate has(diag, 'hardware_gate:QuarantineGate')",
           gate_has(gate_diag, "hardware_gate:QuarantineGate"), False)
    expect("phase13 gate has(diag, present, absent) - one absent token is enough",
           gate_has(gate_diag, "hardware_gate:IsolationGate",
                    "hardware_gate:QuarantineGate"), False)

    print()
    if failures:
        print(f"{len(failures)} case(s) failed:")
        for item in failures:
            print(f"  - {item}")
        return 1
    print("gate token comparison: whitespace-insensitive, and still fails on absence")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
