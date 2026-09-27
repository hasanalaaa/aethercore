#!/usr/bin/env python3
"""The delivered-source seal: read the committed manifest, hash the tree, diff.

What it seals against and what belongs inside it are stated and defended in
`docs/SOURCE_SEAL.md`. The short form: every git-tracked file under the sealed
root, and nothing else. Tracked is the definition of delivered, so the seal
covers everything an auditor receives and nothing a build produces.

Why this file exists at all -- DBT-P59-002: the previous verifier regenerated its
expected hashes from the tree it then compared against, so its expected value was
minted from its measured value. It reported green in every phase it ever ran in.
Nothing here writes, regenerates, or repairs: the manifest is read as committed,
the tree is hashed independently, and the two are diffed.

    python3 scripts/source_seal.py [--root DIR] [--json]

exit 0 = every tracked file is listed and every listed hash matches.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
from pathlib import Path, PurePosixPath

ROOT = Path(__file__).resolve().parents[1]
MANIFEST_NAME = "MANIFEST.sha256"


class SealError(Exception):
    """The seal could not be evaluated. Never swallowed into a passing verdict."""


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def relpath_reason(rel: str) -> str | None:
    """Why this manifest path is not safe to resolve, or None if it is."""
    if not rel or rel != rel.strip():
        return "malformed"
    if "\\" in rel or rel.startswith("/") or ":" in rel:
        return "unsafe_path"
    parts = PurePosixPath(rel).parts
    if any(p in ("..", ".") for p in parts):
        return "unsafe_path"
    return None


def load_manifest(path: Path) -> tuple[dict[str, str], list[dict[str, str]]]:
    if not path.is_file():
        raise SealError(f"manifest is missing: {path}")
    listed: dict[str, str] = {}
    bad: list[dict[str, str]] = []
    for raw in path.read_text(encoding="utf-8").splitlines():
        if not raw.strip():
            continue
        digest, sep, rel = raw.partition("  ")
        if not sep or len(digest) != 64 or any(c not in "0123456789abcdefABCDEF" for c in digest):
            bad.append({"path": raw, "reason": "malformed"})
            continue
        reason = relpath_reason(rel)
        if reason:
            bad.append({"path": rel, "reason": reason})
            continue
        if rel in listed:
            bad.append({"path": rel, "reason": "duplicate"})
            continue
        listed[rel] = digest.lower()
    return listed, bad


def tracked_files(root: Path) -> set[str]:
    """The delivered set: what git tracks under `root`, manifest excluded.

    git is required. An auditor holding a tree with no git metadata cannot be told
    which files were delivered, only which ones happen to be on disk -- and the
    difference between those two is exactly the 914-file hole DBT-P59-003 recorded.
    So this fails loudly rather than falling back to a directory walk.
    """
    proc = subprocess.run(
        ["git", "ls-files", "-z", "--cached", "--", "."],
        cwd=root,
        capture_output=True,
    )
    if proc.returncode != 0:
        raise SealError(
            f"git ls-files failed in {root}: {proc.stderr.decode('utf-8', 'replace').strip()}"
        )
    files = {p for p in proc.stdout.decode("utf-8").split("\0") if p}
    files.discard(MANIFEST_NAME)
    return files


def verify(root: Path) -> dict:
    root = root.resolve()
    listed, failed = load_manifest(root / MANIFEST_NAME)
    tracked = tracked_files(root)
    verified = 0
    for rel in sorted(listed):
        if rel not in tracked:
            failed.append({"path": rel, "reason": "not_tracked"})
            continue
        target = root / PurePosixPath(rel)
        if target.is_symlink():
            failed.append({"path": rel, "reason": "symlink"})
            continue
        if not target.is_file():
            failed.append({"path": rel, "reason": "missing"})
            continue
        actual = sha256(target)
        if actual != listed[rel]:
            failed.append(
                {"path": rel, "reason": "hash", "expected": listed[rel], "actual": actual}
            )
            continue
        verified += 1
    for rel in sorted(tracked - set(listed)):
        failed.append({"path": rel, "reason": "unlisted"})
    return {
        "schema": "aethercore.source-seal.v1",
        "root": str(root),
        "listed": len(listed),
        "tracked": len(tracked),
        "verified": verified,
        "failed": failed,
        "ok": not failed and verified == len(tracked),
    }


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--root", type=Path, default=ROOT)
    ap.add_argument("--json", action="store_true")
    args = ap.parse_args()
    try:
        result = verify(args.root)
        # DBT-P60-002: the repository root's .github/ has its own manifest, verified with the
        # same rules. Reported beside the workspace result, so the workspace's counts keep
        # their meaning for every reader of this JSON.
        github = args.root.resolve().parent / ".github"
        if github.is_dir():
            root_result = verify(github)
            result["repository_root"] = root_result
            result["ok"] = result["ok"] and root_result["ok"]
    except SealError as exc:
        if args.json:
            print(json.dumps({"schema": "aethercore.source-seal.v1", "ok": False, "error": str(exc)}))
        else:
            print(f"Source seal: UNEVALUATED - {exc}", file=sys.stderr)
        return 2
    if args.json:
        print(json.dumps(result, indent=2, sort_keys=True))
    else:
        root_result = result.get("repository_root")
        failed = result["failed"] + [
            dict(entry, path=f".github/{entry['path']}") for entry in (root_result or {}).get("failed", [])
        ]
        counts = f"{result['verified']} of {result['tracked']} tracked files verified"
        if root_result:
            counts += f"; .github: {root_result['verified']} of {root_result['tracked']}"
        if result["ok"]:
            print(f"Source seal: OK - {counts}")
        else:
            print(f"Source seal: FAILED - {counts}, {len(failed)} problems", file=sys.stderr)
            for entry in failed[:32]:
                print(f"  {entry['reason']:12} {entry['path']}", file=sys.stderr)
            if len(failed) > 32:
                print(f"  ... and {len(failed) - 32} more", file=sys.stderr)
    return 0 if result["ok"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
