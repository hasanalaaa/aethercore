#!/usr/bin/env python3
"""Phase 30 binary-safe verification — dual mode, full standard continued.

Modes:
  default (patch)  : hash-verifies every modified/added file in MANIFEST.json against
                     the sha256 recorded there.
  --full-tree      : hash-verifies the ENTIRE delivered tree against
                     PHASE_30_EXPECTED_FULL_SHA256.json. The ledger cannot contain its
                     own digest; that file is self-excluded and bound by the master
                     archive SHA-256.
"""
from __future__ import annotations

import hashlib
import json
import pathlib
import sys

EXCLUDED_DIRS = {"target", "node_modules", ".git", "dist", "state", "support-staging"}
SELF = "PHASE_30_BINARY_SAFE_PATCH/PHASE_30_EXPECTED_FULL_SHA256.json"


def sha256(path: pathlib.Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def patch_mode(root: pathlib.Path, here: pathlib.Path) -> dict:
    manifest = json.loads((here / "MANIFEST.json").read_text())
    problems = []
    checked = 0
    for entry in manifest.get("files", []):
        rel = entry[1:] if entry.startswith("+") else entry
        # The P31 patch dir is verified by verify_phase31.py; its MANIFEST is refreshed
        # by the P31 seal flow, so it never belongs to this patch's hash set.
        if rel.startswith("PHASE_31_BINARY_SAFE_PATCH/") or rel == ".DS_Store":
            continue
        p = root / rel
        if not p.is_file():
            problems.append(f"missing: {rel}")
            continue
        want = manifest.get("sha256", {}).get(rel)
        if want is None:
            problems.append(f"no recorded sha256 for: {rel}")
            continue
        checked += 1
        if sha256(p) != want:
            problems.append(f"hash mismatch: {rel}")
    return {"schema": "aethercore.phase30.binary-safe-verify.v1", "mode": "patch",
            "root": str(root), "checked": checked, "problems": problems,
            "status": "PASS" if not problems else "FAIL"}


def full_tree_mode(root: pathlib.Path, here: pathlib.Path) -> dict:
    ledger = json.loads((here / "PHASE_30_EXPECTED_FULL_SHA256.json").read_text())
    entries = ledger.get("files", {})
    problems = []
    checked = 0
    seen = set()
    for path in sorted(root.rglob("*")):
        rel = str(path.relative_to(root))
        if rel == SELF or rel.startswith("PHASE_31_BINARY_SAFE_PATCH/"):
            # The P31 patch dir is verified by verify_phase31.py instead.
            seen.add(rel)
            continue
        if any(part in EXCLUDED_DIRS for part in path.parts) or rel == ".DS_Store":
            continue
        if not path.is_file():
            continue
        seen.add(rel)
        want = entries.get(rel)
        if want is None:
            problems.append(f"unrecorded file: {rel}")
            continue
        checked += 1
        if sha256(path) != want:
            problems.append(f"hash mismatch: {rel}")
    for rel in sorted(set(entries) - seen):
        if rel == ".DS_Store":
            continue
        problems.append(f"missing: {rel}")
    return {"schema": "aethercore.phase30.binary-safe-verify.v1", "mode": "full-tree",
            "root": str(root), "checked": checked,
            "expected_total": len(entries), "self_excluded": SELF,
            "problems": problems[:50], "problem_count": len(problems),
            "status": "PASS" if not problems else "FAIL"}


def main() -> int:
    root = pathlib.Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else pathlib.Path.cwd().resolve()
    here = pathlib.Path(__file__).resolve().parent
    full = "--full-tree" in sys.argv
    result = full_tree_mode(root, here) if full else patch_mode(root, here)
    print(json.dumps(result, indent=2))
    return 0 if result["status"] == "PASS" else 1


if __name__ == "__main__":
    sys.exit(main())
