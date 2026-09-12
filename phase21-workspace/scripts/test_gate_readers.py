#!/usr/bin/env python3
"""A gate pointed at a source it cannot read must fail. `DBT-P58-005`, ITEM 1.C.

Each case runs one gate in a subprocess with exactly **one** of its sources made
unreadable, and asserts the gate fails *because of that source* - it must name
the source in its output, not merely exit non-zero, because several of these
gates already exit non-zero on macOS for unrelated pre-existing reasons.

Nothing on disk is touched. The source is made unreadable by a `sitecustomize`
shim on `PYTHONPATH` that reports it absent and raises `OSError` on read, which
is what a moved file, a permission change or a bad mount looks like to
`read_text`. That is the exact failure P58 hit when three workflows moved to the
repository root: eight audits kept reporting normally while reading `""`.

Before the P59 repair **ten of the fourteen** fail. `scripts/phase16-ga-audit.py`,
for instance, reports `{"ok": true, "checks": 42, "failed": []}` and exit 0 with
one of the five files it audits gone. The four that already passed did so by
accident - a bare, already fail-closed read elsewhere in the same script reached
the source first. Nothing enforced that pairing, which is what "latent" means in
`docs/phase59/P59-READER-CENSUS.md`.

Run: `python3 scripts/test_gate_readers.py`
"""
from __future__ import annotations

import os
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

SHIM = '''
import os, pathlib
_target = os.environ["AETHERCORE_UNREADABLE"]
_read_text, _exists, _is_file = pathlib.Path.read_text, pathlib.Path.exists, pathlib.Path.is_file


def _hit(p):
    s = p.as_posix()
    return s == _target or s.endswith("/" + _target)


def read_text(self, *a, **k):
    if _hit(self):
        # Three arguments, so the exception carries `filename` and str() names
        # the path - exactly what a genuinely missing file raises. A two-argument
        # OSError would not, and the test would then be measuring its own shim
        # rather than whether the gate says which file.
        raise FileNotFoundError(2, "made absent by scripts/test_gate_readers.py", str(self))
    return _read_text(self, *a, **k)


pathlib.Path.read_text = read_text
pathlib.Path.exists = lambda self: False if _hit(self) else _exists(self)
pathlib.Path.is_file = lambda self: False if _hit(self) else _is_file(self)
'''

# (gate, source made unreadable, whether the gate reports a pass on it TODAY)
CASES: list[tuple[str, str, bool]] = [
    ("scripts/phase16-ga-audit.py", "scripts/phase13-fault-injection.ps1", True),
    ("scripts/phase15-security-audit.py", "crates/security/src/lib.rs", True),
    ("scripts/phase13-reliability-audit.py", "services/maintenance-service/src/protocol.rs", True),
    ("scripts/enterprise-adversarial-audit.py", "apps/ui/src/lib/i18n/catalog.en.ts", True),
    ("scripts/phase35-adversarial-audit.py", "apps/desktop/tauri.conf.json", True),
    ("scripts/static_validate.py", "scripts/setup-and-run.ps1", True),
    ("scripts/static_validate.py", "scripts/phase10-architecture-audit.ps1", True),
    ("scripts/phase14-scheduler-audit.py", "crates/idle-scheduler/src/policy.rs", False),
    ("scripts/zenith-adversarial-audit.py", "apps/ui/src/design/motion/fluid-press.ts", False),
    ("scripts/zenith-recursive-audit.py", "services/maintenance-service/src/streaming.rs", False),
    ("scripts/phase19-windows-repair-audit.py", "crates/operation-engine/src/lib.rs", False),
]

# A gate reads its own workflow from the repository root, not from the workspace
# (P58 / DBT-P55-001). The reader must fail on that one too, or the next move
# repeats P58 exactly.
CASES.append(("scripts/phase15-security-audit.py", ".github/workflows/ci.yml", False))


def run(gate: str, unreadable: str, shim_dir: str) -> subprocess.CompletedProcess:
    env = dict(os.environ)
    env["AETHERCORE_UNREADABLE"] = unreadable
    env["PYTHONPATH"] = shim_dir + os.pathsep + env.get("PYTHONPATH", "")
    return subprocess.run(
        [sys.executable, gate], cwd=ROOT, env=env, text=True, capture_output=True
    )


def main() -> int:
    failures: list[str] = []
    with tempfile.TemporaryDirectory(prefix="aethercore-p59-reader-") as td:
        Path(td, "sitecustomize.py").write_text(SHIM, encoding="utf-8")
        for gate, source, live in CASES:
            p = run(gate, source, td)
            blob = p.stdout + p.stderr
            # `source in blob` alone is not enough: several of these gates print
            # the paths they checked as evidence, so a gate that failed for an
            # unrelated pre-existing reason would name the source anyway. The
            # discriminating fact is that a READ is what aborted it - either the
            # shared reader (`UnreadableSource`) or one of the bare, already
            # fail-closed reads that sometimes reaches the source first.
            aborted = source in blob and (
                "UnreadableSource" in blob or "FileNotFoundError" in blob
            )
            ok = p.returncode != 0 and aborted
            mark = "PASS" if ok else "FAIL"
            print(f"{mark}  {gate}  ⟂ {source}  exit={p.returncode}  aborted_at_read={aborted}")
            if not ok:
                failures.append(
                    f"{gate} did not abort on an unreadable {source} "
                    f"(exit={p.returncode}, aborted_at_read={aborted})"
                    + ("  [reports a pass on it today]" if live else "")
                )

    # Case 2: a missing PHASE_29 patch manifest must not make the consistency
    # check pass. `phase29-adversarial-audit.py:256` substitutes `{}`, which
    # makes `inconsistent` empty and the check green.
    with tempfile.TemporaryDirectory(prefix="aethercore-p59-reader-") as td:
        Path(td, "sitecustomize.py").write_text(SHIM, encoding="utf-8")
        p = run("scripts/phase29-adversarial-audit.py",
                "PHASE_29_BINARY_SAFE_PATCH/MANIFEST.json", td)
        blob = p.stdout + p.stderr
        named = "MANIFEST.json" in blob and "FileNotFoundError" in blob
        ok = p.returncode != 0 and named
        print(f"{'PASS' if ok else 'FAIL'}  scripts/phase29-adversarial-audit.py  "
              f"⟂ PHASE_29_BINARY_SAFE_PATCH/MANIFEST.json  exit={p.returncode}  named={named}")
        if not ok:
            failures.append(
                "phase29-adversarial-audit.py reported p29-manifest-fulltree-consistency "
                "without opening PHASE_29_BINARY_SAFE_PATCH/MANIFEST.json"
            )

    # Case 3: `check-dependency-freeze.py` calls itself fail-closed. An unreadable
    # root package.json must not be flattened into "package.json pins no pnpm
    # version", which would let `pnpm_pin` pass when the metadata also omits it.
    import importlib.util

    spec = importlib.util.spec_from_file_location(
        "p59_freeze_checker", ROOT / "scripts/check-dependency-freeze.py"
    )
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    real_read_text = Path.read_text
    try:
        Path.read_text = lambda self, *a, **k: (
            (_ for _ in ()).throw(OSError(13, "unreadable"))
            if self.name == "package.json"
            else real_read_text(self, *a, **k)
        )
        result = module.inspect()
    finally:
        Path.read_text = real_read_text
    ok = result["approved"] is False and "package.json" in str(result.get("reason", ""))
    print(f"{'PASS' if ok else 'FAIL'}  scripts/check-dependency-freeze.py  ⟂ package.json  "
          f"reason={result.get('reason')!r}")
    if not ok:
        failures.append(
            "check-dependency-freeze.py did not name an unreadable package.json in its reason"
        )

    print()
    if failures:
        print(f"{len(failures)} reader(s) still fail open:")
        for f in failures:
            print(f"  - {f}")
        return 1
    print(f"all {len(CASES) + 2} readers fail closed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
