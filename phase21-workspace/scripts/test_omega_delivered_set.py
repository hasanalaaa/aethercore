#!/usr/bin/env python3
"""Prove `omega-evidence.py` uses ONE definition of delivered, and a clone a gate
can actually audit.

DBT-P60-005: this script carried its own deliverable set -- a directory walk
minus a hardcoded exclusion list -- which disagreed with the seal over 14 files.
DBT-P60-004: the same walk is what made it take 13 copies of a 36 GB `target/`
per run. Both are one defect, and these cases hold the fix in place:

* the set it uses IS the seal's set, not a copy that agrees today;
* ignored build output is outside it, which is the whole 14-file disagreement;
* a disposable clone is repository-shaped, because eight gates read `.github/`
  from one level above the workspace and got a FileNotFoundError for two phases.

Run: python3 scripts/test_omega_delivered_set.py
"""
from __future__ import annotations

import importlib.util
import sys
import tempfile
from pathlib import Path

sys.dont_write_bytecode = True
SCRIPTS = Path(__file__).resolve().parent
ROOT = SCRIPTS.parent
sys.path.insert(0, str(SCRIPTS))

from source_seal import tracked_files  # noqa: E402

RESULTS: list[tuple[str, bool, str]] = []


def record(name: str, ok: bool, detail: str = "") -> None:
    RESULTS.append((name, ok, detail))
    print(f"{'PASS' if ok else 'FAIL'}  {name}" + (f"  -- {detail}" if detail else ""))


def omega():
    """Import the hyphenated script as a module so its own functions can be called."""
    spec = importlib.util.spec_from_file_location("omega_evidence", SCRIPTS / "omega-evidence.py")
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


def case_delivered_set_is_the_seals_set(module) -> None:
    record(
        "the deliverable set is the seal's tracked set",
        module.delivered_files() == tracked_files(ROOT),
        f"{len(module.delivered_files())} files, identical to source_seal.tracked_files()",
    )


def case_ignored_build_output_is_outside(module) -> None:
    """The 14 files. Named by kind rather than by path so the case survives a
    rebuild that renames Vite's content-hashed assets."""
    delivered = module.delivered_files()
    intruders = sorted(
        rel
        for rel in delivered
        if rel.startswith("apps/ui/dist/") or rel.endswith(".gguf") or "\\" in rel
    )
    record(
        "build output, the model and leaked Windows paths are not delivered",
        not intruders,
        f"{len(intruders)} intruders: {intruders[:4]}" if intruders else "none in the delivered set",
    )


def case_clone_is_repository_shaped(module) -> None:
    with tempfile.TemporaryDirectory() as td:
        clone = Path(td) / "repo" / ROOT.name
        module.clone_delivered(clone)
        gate_reader = clone / "scripts/gate_reader.py"
        workflow = clone.parent / ".github/workflows/ci.yml"
        record(
            "a gate clone can read .github from one level above the workspace",
            workflow.is_file() and gate_reader.is_file(),
            f".github/workflows/ci.yml={workflow.is_file()} scripts/gate_reader.py={gate_reader.is_file()}",
        )
        record(
            "the clone carries the seal it is meant to verify",
            (clone / "MANIFEST.sha256").is_file(),
        )


def case_clone_excludes_untracked(module) -> None:
    with tempfile.TemporaryDirectory() as td:
        clone = Path(td) / "repo" / ROOT.name
        module.clone_delivered(clone)
        copied = {p.relative_to(clone).as_posix() for p in clone.rglob("*") if p.is_file()}
        copied.discard("MANIFEST.sha256")
        record(
            "the clone holds the delivered set and nothing else",
            copied == module.delivered_files(),
            f"clone {len(copied)} files, delivered {len(module.delivered_files())}",
        )


def main() -> int:
    module = omega()
    for case in (
        case_delivered_set_is_the_seals_set,
        case_ignored_build_output_is_outside,
        case_clone_is_repository_shaped,
        case_clone_excludes_untracked,
    ):
        try:
            case(module)
        except Exception as exc:  # a crashing case is a failing case, named
            record(case.__name__, False, f"{type(exc).__name__}: {exc}")
    passed = sum(1 for _, ok, _ in RESULTS if ok)
    print(f"\n{passed} of {len(RESULTS)} pass")
    return 0 if passed == len(RESULTS) else 1


if __name__ == "__main__":
    raise SystemExit(main())
