#!/usr/bin/env python3
"""Phase 32 binary-safe verification — dual mode (patch hashes / full tree).

Ledger scoping (P31 lessons): the full-tree ledger excludes ALL later-phase
patch dirs (PHASE_30_/PHASE_31_/PHASE_32_BINARY_SAFE_PATCH) plus .DS_Store;
those are proven by GG round-trips and phase audits, not by this ledger.
"""
from __future__ import annotations
import hashlib, json, pathlib, sys

EXCLUDED_DIRS = {"target", "node_modules", ".git", "dist", "state", "support-staging",
                 "__pycache__"}
SELF = "PHASE_32_BINARY_SAFE_PATCH/PHASE_32_EXPECTED_FULL_SHA256.json"
EXCLUDED_PREFIXES = ("PHASE_30_BINARY_SAFE_PATCH/", "PHASE_31_BINARY_SAFE_PATCH/",
                     "PHASE_32_BINARY_SAFE_PATCH/")


def sha256(p):
    return hashlib.sha256(p.read_bytes()).hexdigest()


def patch_mode(root: pathlib.Path, here: pathlib.Path):
    manifest = json.loads((here / "MANIFEST.json").read_text())
    problems, checked = [], 0
    for entry in manifest.get("files", []):
        rel = entry[1:] if entry.startswith("+") else entry
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
    return {"schema": "aethercore.phase32.binary-safe-verify.v1", "mode": "patch",
            "root": str(root), "checked": checked, "problems": problems,
            "status": "PASS" if not problems else "FAIL"}


def full_tree_mode(root: pathlib.Path, here: pathlib.Path):
    entries = json.loads((here / "PHASE_32_EXPECTED_FULL_SHA256.json").read_text()).get("files", {})
    problems, checked, seen = [], 0, set()
    for path in sorted(root.rglob("*")):
        rel = str(path.relative_to(root))
        if rel.startswith(EXCLUDED_PREFIXES):
            # Self-referential deliverables excluded per ledger note; proven by GG.
            seen.add(rel)
            continue
        if any(part in EXCLUDED_DIRS for part in path.parts) or rel == ".DS_Store" \
                or not path.is_file():
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
        problems.append(f"missing: {rel}")
    return {"schema": "aethercore.phase32.binary-safe-verify.v1", "mode": "full-tree",
            "root": str(root), "checked": checked, "expected_total": len(entries),
            "self_excluded": SELF, "problems": problems[:50],
            "problem_count": len(problems),
            "status": "PASS" if not problems else "FAIL"}


def main():
    root = pathlib.Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else pathlib.Path.cwd().resolve()
    here = pathlib.Path(__file__).resolve().parent
    result = full_tree_mode(root, here) if "--full-tree" in sys.argv else patch_mode(root, here)
    print(json.dumps(result, indent=2))
    return 0 if result["status"] == "PASS" else 1


if __name__ == "__main__":
    sys.exit(main())
