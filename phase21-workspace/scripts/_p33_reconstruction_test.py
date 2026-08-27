#!/usr/bin/env python3
"""Run two independent sealed-P32 -> P33 reconstruction cycles."""
from __future__ import annotations

import hashlib
import json
import pathlib
import shutil
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else ".").resolve()
ARCHIVE = ROOT.parent / "AetherCore-Phase32-Master-Delivery.zip"
HASH_FILE = ROOT.parent / "PHASE32_FINAL_SHA256.txt"
PATCH_DIR = "PHASE_33_BINARY_SAFE_PATCH"
EXCLUDED_DIRS = {
    "target",
    "node_modules",
    ".git",
    "dist",
    "state",
    "support-staging",
    "__pycache__",
    PATCH_DIR,
}


def sha256(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def scoped(root: pathlib.Path) -> dict[str, str]:
    result = {}
    for path in sorted(root.rglob("*")):
        rel = path.relative_to(root)
        if (
            path.is_file()
            and path.name != ".DS_Store"
            and not any(part in EXCLUDED_DIRS for part in rel.parts)
        ):
            result[str(rel)] = sha256(path)
    return result


expected_archive = HASH_FILE.read_text(encoding="utf-8").split()[0]
observed_archive = sha256(ARCHIVE)
if observed_archive != expected_archive:
    raise SystemExit(
        f"sealed P32 archive mismatch: expected={expected_archive} observed={observed_archive}"
    )

live = scoped(ROOT)
results = []
for cycle in (1, 2):
    temp = pathlib.Path(tempfile.mkdtemp(prefix=f"aethercore-p33-cycle-{cycle}-"))
    reconstructed = temp / "root"
    reconstructed.mkdir()
    try:
        subprocess.run(
            [
                "tar",
                "-xzf",
                str(ARCHIVE),
                "-C",
                str(reconstructed),
                "--strip-components",
                "1",
            ],
            check=True,
        )
        shutil.copytree(ROOT / PATCH_DIR, reconstructed / PATCH_DIR)
        apply = reconstructed / PATCH_DIR / "apply_phase33_patch.sh"
        applied = subprocess.run(
            [str(apply), str(reconstructed)], capture_output=True, text=True
        )
        patch_verify = subprocess.run(
            [
                sys.executable,
                str(reconstructed / PATCH_DIR / "verify_phase33.py"),
                str(reconstructed),
            ],
            capture_output=True,
            text=True,
        )
        full_verify = subprocess.run(
            [
                sys.executable,
                str(reconstructed / PATCH_DIR / "verify_phase33.py"),
                str(reconstructed),
                "--full-tree",
            ],
            capture_output=True,
            text=True,
        )
        rebuilt = scoped(reconstructed)
        mismatches = sorted(
            set(live) ^ set(rebuilt)
            | {rel for rel in set(live) & set(rebuilt) if live[rel] != rebuilt[rel]}
        )
        result = {
            "cycle": cycle,
            "apply_status": "PASS" if applied.returncode == 0 else "FAIL",
            "apply_error": applied.stderr.strip()[-500:],
            "patch_status": json.loads(patch_verify.stdout).get("status", "FAIL")
            if patch_verify.stdout
            else "FAIL",
            "full_tree_status": json.loads(full_verify.stdout).get("status", "FAIL")
            if full_verify.stdout
            else "FAIL",
            "compared_files": len(live),
            "mismatch_count": len(mismatches),
            "mismatches": mismatches[:20],
        }
        results.append(result)
    finally:
        shutil.rmtree(temp, ignore_errors=True)

ok = all(
    row["apply_status"] == row["patch_status"] == row["full_tree_status"] == "PASS"
    and row["mismatch_count"] == 0
    for row in results
)
print(
    json.dumps(
        {
            "schema": "aethercore.phase33.reconstruction.v1",
            "base_sha256": observed_archive,
            "cycles": results,
            "status": "PASS" if ok else "FAIL",
        },
        indent=2,
    )
)
raise SystemExit(0 if ok else 1)
