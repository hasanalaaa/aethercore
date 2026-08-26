#!/usr/bin/env python3
"""Phase 28 binary-safe verification — FULL STANDARD (restored).

Two modes:
  1. PATCH MODE (default, no --full-tree): hash-verifies every modified/added file
     listed in MANIFEST.json against the sha256 recorded there.
  2. FULL-TREE MODE (--full-tree): hash-verifies the ENTIRE delivered tree against
     PHASE_28_EXPECTED_FULL_SHA256.json — byte-level provability as a first-class gate,
     closing the P27 existence-only documentation gap.

Exit 0 = PASS; exit 1 = any mismatch/missing file.
"""
from __future__ import annotations

import hashlib
import json
import pathlib
import sys

EXCLUDED_DIRS = {"target", "node_modules", ".git", "dist", "state", "support-staging"}


def sha256(path: pathlib.Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def patch_mode(root: pathlib.Path, here: pathlib.Path) -> dict:
    manifest = json.loads((here / "MANIFEST.json").read_text())
    problems: list[str] = []
    checked = 0
    for entry in manifest["files"]:
        added = entry.startswith("+")
        rel = entry[1:] if added else entry
        p = root / rel
        if not p.is_file():
            problems.append(f"missing: {rel}")
            continue
        digest = sha256(p)
        expected = manifest.get("sha256", {}).get(rel)
        if expected is None:
            problems.append(f"no recorded sha256 for: {rel}")
            continue
        if digest != expected:
            problems.append(f"hash mismatch: {rel}")
            continue
        checked += 1
    return {
        "schema": "aethercore.phase28.binary-safe-verify.v1",
        "mode": "patch",
        "root": str(root),
        "checked": checked,
        "problems": problems,
        "status": "PASS" if not problems else "FAIL",
    }


def full_tree_mode(root: pathlib.Path, here: pathlib.Path) -> dict:
    expected = json.loads((here / "PHASE_28_EXPECTED_FULL_SHA256.json").read_text())
    entries = expected.get("files", {})
    # Self-exclusion: the hash ledger cannot contain its own digest (fixed-point
    # impossibility). The file's authenticity is bound by the master archive SHA-256.
    SELF = "PHASE_28_BINARY_SAFE_PATCH/PHASE_28_EXPECTED_FULL_SHA256.json"
    problems: list[str] = []
    checked = 0
    seen: set[str] = set()
    for path in sorted(root.rglob("*")):
        rel = str(path.relative_to(root))
        if rel == SELF:
            seen.add(rel)
            continue
        if any(part in EXCLUDED_DIRS for part in path.parts):
            continue
        if not path.is_file():
            continue
        seen.add(rel)
        want = entries.get(rel)
        if want is None:
            problems.append(f"unrecorded file: {rel}")
            continue
        got = sha256(path)
        checked += 1
        if got != want:
            problems.append(f"hash mismatch: {rel}")
    for rel in sorted(set(entries) - seen):
        problems.append(f"missing: {rel}")
    return {
        "schema": "aethercore.phase28.binary-safe-verify.v1",
        "mode": "full-tree",
        "root": str(root),
        "checked": checked,
        "expected_total": len(entries),
        "self_excluded": SELF,
        "problems": problems[:50],
        "problem_count": len(problems),
        "status": "PASS" if not problems else "FAIL",
    }


def main() -> int:
    root = pathlib.Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else pathlib.Path.cwd().resolve()
    here = pathlib.Path(__file__).resolve().parent
    full = "--full-tree" in sys.argv
    result = full_tree_mode(root, here) if full else patch_mode(root, here)
    print(json.dumps(result, indent=2))
    return 0 if result["status"] == "PASS" else 1


if __name__ == "__main__":
    sys.exit(main())
