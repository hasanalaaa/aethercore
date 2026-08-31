#!/usr/bin/env python3
"""Round-trip: apply the patch onto a pristine sealed copy, compare byte-identical to
the live tree. Run twice (fresh extraction each time) and require identical results."""
import filecmp
import hashlib
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SEAL = Path("/tmp/p26_sealed_snapshot")
PATCH = ROOT / "PHASE_27_BINARY_SAFE_PATCH"
EXCLUDE_DIRS = {"target", "node_modules", ".git", "dist", "state", "support-staging"}
SKIP_PREFIXES = ("C:\\ProgramData",)

def snapshot(root: Path) -> dict:
    out = {}
    for path in sorted(root.rglob("*")):
        rel = path.relative_to(root)
        if set(rel.parts) & EXCLUDE_DIRS:
            continue
        s = str(rel)
        if any(s.startswith(p) for p in SKIP_PREFIXES):
            continue
        if path.is_file():
            out[s] = hashlib.sha256(path.read_bytes()).hexdigest()
    return out

expected = snapshot(ROOT)
# The patch directory is the deliverable being applied; it cannot be part of its own
# payload. Exclude it from the byte-identical comparison.
for key in [k for k in expected if k.startswith("PHASE_27_BINARY_SAFE_PATCH/")]:
    del expected[key]
results = []
for run in (1, 2):
    target = Path(f"/tmp/p27_roundtrip_run{run}")
    if target.exists():
        shutil.rmtree(target)
    shutil.copytree(SEAL, target)
    r = subprocess.run(["bash", str(PATCH / "apply_phase27_patch.sh"), str(target)],
                       capture_output=True, text=True)
    print(f"run{run} apply:", r.stdout.strip() or r.stderr.strip()[-300:])
    if r.returncode != 0:
        sys.exit(f"run{run} apply failed")
    actual = snapshot(target)
    missing = set(expected) - set(actual)
    extra = {k: v for k, v in actual.items() if k not in expected}
    diff = [k for k in expected.keys() & actual.keys() if expected[k] != actual[k]]
    identical = not missing and not extra and not diff
    results.append(identical)
    print(f"run{run}: files={len(actual)} missing={len(missing)} extra={len(extra)} "
          f"content-diff={len(diff)} IDENTICAL={identical}")
    if not identical:
        for k in list(missing)[:5]:
            print("  MISSING", k)
        for k in list(extra)[:5]:
            print("  EXTRA", k)
        for k in diff[:5]:
            print("  DIFF", k)

print("ROUNDTRIP_TWICE_IDENTICAL:", all(results))
