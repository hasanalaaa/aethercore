#!/usr/bin/env python3
"""Phase 33 binary-safe verification: delta hashes or complete scoped tree."""
from __future__ import annotations

import hashlib
import json
import pathlib
import re
import sys

EXCLUDED_DIRS = {
    "target",
    "node_modules",
    ".git",
    "dist",
    "state",
    "support-staging",
    "__pycache__",
}
SELF_PREFIX = "PHASE_33_BINARY_SAFE_PATCH/"
SELF = f"{SELF_PREFIX}PHASE_33_EXPECTED_FULL_SHA256.json"


def sha256(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def later_patch(rel: str) -> bool:
    first = rel.split("/", 1)[0]
    match = re.fullmatch(r"PHASE_(\d+)_BINARY_SAFE_PATCH", first)
    return bool(match and int(match.group(1)) > 33)


def excluded(root: pathlib.Path, path: pathlib.Path) -> bool:
    rel_path = path.relative_to(root)
    rel = str(rel_path)
    return (
        path.name == ".DS_Store"
        or any(part in EXCLUDED_DIRS for part in rel_path.parts)
        or rel.startswith(SELF_PREFIX)
        or later_patch(rel)
    )


def patch_mode(root: pathlib.Path, here: pathlib.Path) -> dict[str, object]:
    manifest = json.loads((here / "MANIFEST.json").read_text(encoding="utf-8"))
    entries = manifest.get("files", [])
    hashes = manifest.get("sha256", {})
    problems: list[str] = []
    checked = 0
    if len(entries) != len(set(entries)):
        problems.append("duplicate manifest entry")
    expected_hashes: set[str] = set()
    for entry in entries:
        removed = entry.startswith("-")
        rel = entry[1:] if entry.startswith(("+", "-")) else entry
        path = root / rel
        if removed:
            if path.exists():
                problems.append(f"removed file still present: {rel}")
            checked += 1
            continue
        expected_hashes.add(rel)
        if not path.is_file():
            problems.append(f"missing: {rel}")
            continue
        want = hashes.get(rel)
        if want is None:
            problems.append(f"no recorded sha256 for: {rel}")
            continue
        checked += 1
        if sha256(path) != want:
            problems.append(f"hash mismatch: {rel}")
    for rel in sorted(set(hashes) - expected_hashes):
        problems.append(f"orphan manifest hash: {rel}")
    return {
        "schema": "aethercore.phase33.binary-safe-verify.v1",
        "mode": "patch",
        "root": str(root),
        "checked": checked,
        "expected_total": len(entries),
        "problems": problems,
        "problem_count": len(problems),
        "status": "PASS" if not problems else "FAIL",
    }


def full_tree_mode(root: pathlib.Path, here: pathlib.Path) -> dict[str, object]:
    entries = json.loads(
        (here / "PHASE_33_EXPECTED_FULL_SHA256.json").read_text(encoding="utf-8")
    ).get("files", {})
    problems: list[str] = []
    checked = 0
    seen: set[str] = set()
    for path in sorted(root.rglob("*")):
        if not path.is_file() or excluded(root, path):
            continue
        rel = str(path.relative_to(root))
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
    return {
        "schema": "aethercore.phase33.binary-safe-verify.v1",
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
    root = (
        pathlib.Path(sys.argv[1]).resolve()
        if len(sys.argv) > 1
        else pathlib.Path.cwd().resolve()
    )
    here = pathlib.Path(__file__).resolve().parent
    result = (
        full_tree_mode(root, here)
        if "--full-tree" in sys.argv
        else patch_mode(root, here)
    )
    print(json.dumps(result, indent=2))
    return 0 if result["status"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
