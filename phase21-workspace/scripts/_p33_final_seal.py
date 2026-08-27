#!/usr/bin/env python3
"""Final Phase 33 delivery seal; no delivered-tree writes after archive build."""
from __future__ import annotations

import json
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
BASE = pathlib.Path("/tmp/p33_p32_seal/base")


def run(command: list[str], *, cwd: pathlib.Path = ROOT) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(command, cwd=cwd, text=True, capture_output=True)
    print(f"$ {' '.join(command)}")
    output = (result.stdout + result.stderr).strip()
    if output:
        print("\n".join(output.splitlines()[-30:]))
    if result.returncode != 0:
        raise SystemExit(f"seal command failed with exit {result.returncode}")
    return result


def verify(label: str, script: pathlib.Path, root: pathlib.Path, full: bool) -> None:
    command = [sys.executable, str(script), str(root)]
    if full:
        command.append("--full-tree")
    result = subprocess.run(command, cwd=ROOT, text=True, capture_output=True)
    parsed = json.loads(result.stdout)
    problems = parsed.get("problem_count", len(parsed.get("problems", [])))
    print(
        f"{label}: checked={parsed.get('checked')} problems={problems} "
        f"status={parsed.get('status')}"
    )
    if result.returncode != 0 or parsed.get("status") != "PASS":
        raise SystemExit(f"{label} failed")


if not (BASE / "PHASE_32_BINARY_SAFE_PATCH/verify_phase32.py").is_file():
    raise SystemExit("pristine sealed-P32 extraction missing at /tmp/p33_p32_seal/base")

print("== 1. Refresh Phase 33 delta and full-tree ledger ==")
run([sys.executable, "scripts/_build_p33_patch.py", str(ROOT)])

print("== 2. Verify Phase 33 patch/full-tree and adversarial audit ==")
verify(
    "P33 patch",
    ROOT / "PHASE_33_BINARY_SAFE_PATCH/verify_phase33.py",
    ROOT,
    False,
)
verify(
    "P33 full-tree",
    ROOT / "PHASE_33_BINARY_SAFE_PATCH/verify_phase33.py",
    ROOT,
    True,
)
run([sys.executable, "scripts/phase33-adversarial-audit.py", str(ROOT)])

print("== 3. Two independent sealed-P32 reconstruction cycles ==")
run([sys.executable, "scripts/_p33_reconstruction_test.py", str(ROOT)])

print("== 4. Inherited verifiers against the immutable sealed-P32 base ==")
verify(
    "P32 full-tree",
    BASE / "PHASE_32_BINARY_SAFE_PATCH/verify_phase32.py",
    BASE,
    True,
)
verify("P32 patch", BASE / "PHASE_32_BINARY_SAFE_PATCH/verify_phase32.py", BASE, False)
verify(
    "P31 full-tree",
    BASE / "PHASE_31_BINARY_SAFE_PATCH/verify_phase31.py",
    BASE,
    True,
)
verify("P31 patch", BASE / "PHASE_31_BINARY_SAFE_PATCH/verify_phase31.py", BASE, False)

print("== 5. Build Phase 33 archive twice; write external final hash ==")
archive = run([sys.executable, "scripts/_build_p33_archive.py"])
if "ARCHIVE_TWICE_IDENTICAL: True" not in archive.stdout:
    raise SystemExit("archive determinism marker missing")

print("== 6. Post-archive READ-ONLY verification; no delivered-tree writes ==")
verify(
    "P33 patch post-seal",
    ROOT / "PHASE_33_BINARY_SAFE_PATCH/verify_phase33.py",
    ROOT,
    False,
)
verify(
    "P33 full-tree post-seal",
    ROOT / "PHASE_33_BINARY_SAFE_PATCH/verify_phase33.py",
    ROOT,
    True,
)
verify(
    "P32 full-tree post-seal",
    BASE / "PHASE_32_BINARY_SAFE_PATCH/verify_phase32.py",
    BASE,
    True,
)
verify(
    "P31 full-tree post-seal",
    BASE / "PHASE_31_BINARY_SAFE_PATCH/verify_phase31.py",
    BASE,
    True,
)
print("PHASE33_FINAL_SEAL_OK: True")
