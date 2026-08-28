#!/usr/bin/env python3
"""Reconstruct the sealed P33 baseline twice and prove P34 file-count equations."""
from __future__ import annotations

import hashlib
import json
import pathlib
import shutil
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else ".").resolve()
ARCHIVE = ROOT.parent / "AetherCore-Phase33-Master-Delivery.zip"
HASH_FILE = ROOT.parent / "PHASE33_FINAL_SHA256.txt"
PATCH_DIR = "PHASE_34_BINARY_SAFE_PATCH"
LEDGER = ROOT / PATCH_DIR / "PHASE_34_EXPECTED_FULL_SHA256.json"
EXCLUDED_DIRS = {
    "target", "node_modules", ".git", "dist", "state", "support-staging", "__pycache__", PATCH_DIR,
}
RAW_EXCLUDED_DIRS = EXCLUDED_DIRS - {PATCH_DIR}


def sha256(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def file_map(root: pathlib.Path, excluded: set[str], omit_ds_store: bool) -> dict[str, str]:
    result: dict[str, str] = {}
    for path in sorted(root.rglob("*")):
        rel = path.relative_to(root)
        if path.is_file() and not any(part in excluded for part in rel.parts):
            if omit_ds_store and path.name == ".DS_Store":
                continue
            result[str(rel)] = sha256(path)
    return result


expected_archive = HASH_FILE.read_text(encoding="utf-8").split()[0]
observed_archive = sha256(ARCHIVE)
if observed_archive != expected_archive:
    raise SystemExit(f"sealed P33 archive mismatch: expected={expected_archive} observed={observed_archive}")

ledger = json.loads(LEDGER.read_text(encoding="utf-8"))["files"]
ledger_total = len(ledger)
live = file_map(ROOT, EXCLUDED_DIRS, True)
raw = file_map(ROOT, RAW_EXCLUDED_DIRS, False)
self_excluded_paths = sorted(set(raw) - set(live))
raw_tree_total = len(raw)
scoped_tree_total = len(live)
self_excluded_count = len(self_excluded_paths)
raw_equation = raw_tree_total == scoped_tree_total + self_excluded_count

cycles: list[dict[str, object]] = []
for cycle in (1, 2):
    temp = pathlib.Path(tempfile.mkdtemp(prefix=f"aethercore-p34-cycle-{cycle}-"))
    reconstructed = temp / "root"
    reconstructed.mkdir()
    try:
        subprocess.run([
            "tar", "-xzf", str(ARCHIVE), "-C", str(reconstructed), "--strip-components", "1",
        ], check=True)
        shutil.copytree(ROOT / PATCH_DIR, reconstructed / PATCH_DIR)
        apply = reconstructed / PATCH_DIR / "apply_phase34_patch.sh"
        applied = subprocess.run([str(apply), str(reconstructed)], capture_output=True, text=True)
        verify = reconstructed / PATCH_DIR / "verify_phase34.py"
        patch_verify = subprocess.run([sys.executable, str(verify), str(reconstructed)], capture_output=True, text=True)
        full_verify = subprocess.run([sys.executable, str(verify), str(reconstructed), "--full-tree"], capture_output=True, text=True)
        rebuilt = file_map(reconstructed, EXCLUDED_DIRS, True)
        mismatches = sorted(
            set(live) ^ set(rebuilt)
            | {rel for rel in set(live) & set(rebuilt) if live[rel] != rebuilt[rel]}
        )
        patch_json = json.loads(patch_verify.stdout) if patch_verify.stdout else {}
        full_json = json.loads(full_verify.stdout) if full_verify.stdout else {}
        cycles.append({
            "cycle": cycle,
            "apply_status": "PASS" if applied.returncode == 0 else "FAIL",
            "patch_status": patch_json.get("status", "FAIL"),
            "full_tree_status": full_json.get("status", "FAIL"),
            "comparison_total": len(rebuilt),
            "mismatch_count": len(mismatches),
            "mismatches": mismatches[:20],
        })
    finally:
        shutil.rmtree(temp, ignore_errors=True)

comparison_total = cycles[0]["comparison_total"] if cycles else 0
ledger_equals_comparison = ledger_total == comparison_total and all(
    row["comparison_total"] == ledger_total for row in cycles
)
mismatch_count = max((int(row["mismatch_count"]) for row in cycles), default=1)
status = "PASS" if (
    raw_equation and ledger_equals_comparison and mismatch_count == 0 and all(
        row["apply_status"] == row["patch_status"] == row["full_tree_status"] == "PASS" for row in cycles
    )
) else "FAIL"

report = {
    "raw_tree_total": raw_tree_total,
    "scoped_tree_total": scoped_tree_total,
    "ledger_total": ledger_total,
    "comparison_total": comparison_total,
    "self_excluded_count": self_excluded_count,
    "self_excluded_paths": self_excluded_paths,
    "ledger_equals_comparison": ledger_equals_comparison,
    "raw_equals_scoped_plus_self_excluded": raw_equation,
    "mismatch_count": mismatch_count,
    "status": status,
    "cycles": cycles,
}
print(json.dumps(report, indent=2))
raise SystemExit(0 if status == "PASS" else 1)
