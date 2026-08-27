#!/usr/bin/env python3
"""Build the Phase 33 text delta and full-tree SHA-256 ledger.

The delta is computed against the authoritative sealed Phase 32 archive. The
Phase 33 patch directory is excluded from both scans because hashing artifacts
that contain their own hashes would create a self-reference loop. Earlier
phase patch directories remain in scope.
"""
from __future__ import annotations

import hashlib
import json
import pathlib
import shutil
import subprocess
import sys

ROOT = (
    pathlib.Path(sys.argv[1]).resolve()
    if len(sys.argv) > 1
    else pathlib.Path(__file__).resolve().parents[1]
)
PATCH = ROOT / "PHASE_33_BINARY_SAFE_PATCH"
BASE_ARCHIVE = ROOT.parent / "AetherCore-Phase32-Master-Delivery.zip"
BASE_HASH_FILE = ROOT.parent / "PHASE32_FINAL_SHA256.txt"
BASE = pathlib.Path("/tmp/p33_p32_seal/base")
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
SELF_LEDGER = f"{SELF_PREFIX}PHASE_33_EXPECTED_FULL_SHA256.json"


def sha256(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def excluded(root: pathlib.Path, path: pathlib.Path) -> bool:
    rel = path.relative_to(root)
    return (
        path.name == ".DS_Store"
        or any(part in EXCLUDED_DIRS for part in rel.parts)
        or str(rel).startswith(SELF_PREFIX)
    )


def file_map(root: pathlib.Path) -> dict[str, pathlib.Path]:
    return {
        str(path.relative_to(root)): path
        for path in sorted(root.rglob("*"))
        if path.is_file() and not excluded(root, path)
    }


def ensure_base() -> None:
    if not BASE_ARCHIVE.is_file() or not BASE_HASH_FILE.is_file():
        raise SystemExit("FATAL: authoritative Phase 32 archive/hash is missing")
    expected = BASE_HASH_FILE.read_text(encoding="utf-8").split()[0]
    observed = sha256(BASE_ARCHIVE)
    if observed != expected:
        raise SystemExit(
            f"FATAL: Phase 32 archive hash mismatch: expected={expected} observed={observed}"
        )
    marker = BASE / "PHASE_32_BINARY_SAFE_PATCH" / "verify_phase32.py"
    if not marker.is_file():
        shutil.rmtree(BASE.parent, ignore_errors=True)
        BASE.mkdir(parents=True)
        subprocess.run(
            [
                "tar",
                "-xzf",
                str(BASE_ARCHIVE),
                "-C",
                str(BASE),
                "--strip-components",
                "1",
            ],
            check=True,
        )


def require_text(path: pathlib.Path, rel: str) -> None:
    raw = path.read_bytes()
    if b"\0" in raw:
        raise SystemExit(f"FATAL: binary Phase 33 delta requires payload support: {rel}")
    try:
        raw.decode("utf-8")
    except UnicodeDecodeError as error:
        raise SystemExit(f"FATAL: non-UTF-8 Phase 33 delta {rel}: {error}") from error


def unified_diff(old: pathlib.Path | None, new: pathlib.Path | None, rel: str) -> str:
    if old is not None:
        require_text(old, rel)
    if new is not None:
        require_text(new, rel)
    command = ["diff", "-U0", "-u"]
    if old is None or new is None:
        command.append("-N")
    command += [
        "--label",
        f"a/{rel}" if old is not None else "/dev/null",
        "--label",
        f"b/{rel}" if new is not None else "/dev/null",
        str(old) if old is not None else "/dev/null",
        str(new) if new is not None else "/dev/null",
    ]
    result = subprocess.run(command, capture_output=True, text=True)
    if result.returncode not in (0, 1):
        raise SystemExit(f"FATAL: diff failed for {rel}: {result.stderr.strip()}")
    return result.stdout


ensure_base()
PATCH.mkdir(parents=True, exist_ok=True)
live = file_map(ROOT)
sealed = file_map(BASE)
added = sorted(set(live) - set(sealed))
removed = sorted(set(sealed) - set(live))
modified = sorted(
    rel for rel in set(live) & set(sealed) if sha256(live[rel]) != sha256(sealed[rel])
)

diffs = [unified_diff(sealed[rel], live[rel], rel) for rel in modified]
diffs += [unified_diff(None, live[rel], rel) for rel in added]
diffs += [unified_diff(sealed[rel], None, rel) for rel in removed]
(PATCH / "changes.patch").write_text("".join(diffs), encoding="utf-8")

post_state = modified + added
sha_map = {rel: sha256(live[rel]) for rel in sorted(post_state)}
manifest = {
    "schema": "aethercore.phase33.binary-safe-patch.v1",
    "base": BASE_ARCHIVE.name,
    "base_sha256": sha256(BASE_ARCHIVE),
    "files": modified + [f"+{rel}" for rel in added] + [f"-{rel}" for rel in removed],
    "modifiedCount": len(modified),
    "addedCount": len(added),
    "removedCount": len(removed),
    "sha256": sha_map,
}
(PATCH / "MANIFEST.json").write_text(
    json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8"
)

ledger = {rel: sha256(path) for rel, path in sorted(live.items())}
full = {
    "schema": "aethercore.phase33.full-sha256.v1",
    "base": BASE_ARCHIVE.name,
    "note": (
        "Build/dependency caches and .DS_Store are structurally excluded. "
        "PHASE_33_BINARY_SAFE_PATCH is excluded as a self-referential deliverable; "
        "its integrity is established by two sealed-P32 reconstruction cycles. "
        "Earlier phase patch directories remain in the full-tree scope."
    ),
    "files": ledger,
    "self_excluded": SELF_LEDGER,
}
(PATCH / "PHASE_33_EXPECTED_FULL_SHA256.json").write_text(
    json.dumps(full, indent=2, sort_keys=True) + "\n", encoding="utf-8"
)
print(
    f"modified={len(modified)} added={len(added)} removed={len(removed)} "
    f"manifest={len(manifest['files'])} ledger={len(ledger)}"
)
