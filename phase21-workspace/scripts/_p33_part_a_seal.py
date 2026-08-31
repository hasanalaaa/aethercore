#!/usr/bin/env python3
"""P33 Part A — final seal script for the P32 seal-closure cycle.

Order (cross-phase ledger-coherence discipline):
  1. Clear /tmp seal caches.
  2. Refresh the P31 patch dir from its sealed base (scoping fixes included).
  3. Refresh the P32 patch dir on top of it.
  4. Rebuild the master archive TWICE; assert identical SHA-256;
     write PHASE32_FINAL_SHA256.txt (pointer target).
  5. Verify ALL FOUR modes (P32 full-tree/patch, P31 full-tree/patch).
  6. Re-run the phase32 adversarial audit (must stay 817/PASS).

ZERO tree writes after step 4.
"""
import json
import pathlib
import shutil
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
OUT = ROOT.parent


def run(cmd, **kw):
    r = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True, **kw)
    tail = "\n".join((r.stdout + r.stderr).strip().splitlines()[-4:])
    print(f"$ {' '.join(str(c) for c in cmd)}\n{tail}")
    if r.returncode != 0:
        print(f"!! exit={r.returncode}")
    return r


print("== step 1: clear seal caches ==")
for d in ("/tmp/p30_seal_extract", "/tmp/p31_seal_extract", "/tmp/p32_seal_extract"):
    shutil.rmtree(d, ignore_errors=True)
print("cleared")

print("\n== step 2: refresh P31 patch dir ==")
run([sys.executable, "scripts/_build_p31_patch.py", str(ROOT)])

print("\n== step 3: refresh P32 patch dir ==")
run([sys.executable, "scripts/_build_p32_patch.py", str(ROOT)])

print("\n== step 4: master archive x2 ==")
r = run([sys.executable, "scripts/_build_p32_archive.py"])
hashes = [ln.split()[-1] for ln in r.stdout.splitlines() if ln.startswith("run")]
identical = "ARCHIVE_TWICE_IDENTICAL: True" in r.stdout
final_hash = OUT.joinpath("PHASE32_FINAL_SHA256.txt").read_text().strip()
print(f"identical={identical} final_sha256_file='{final_hash}'")

print("\n== step 5: FOUR verify modes ==")
modes = [
    ("P32 full-tree", ["PHASE_32_BINARY_SAFE_PATCH/verify_phase32.py", ".", "--full-tree"]),
    ("P32 patch", ["PHASE_32_BINARY_SAFE_PATCH/verify_phase32.py", "."]),
    ("P31 full-tree", ["PHASE_31_BINARY_SAFE_PATCH/verify_phase31.py", ".", "--full-tree"]),
    ("P31 patch", ["PHASE_31_BINARY_SAFE_PATCH/verify_phase31.py", "."]),
]
all_pass = True
for label, cmd in modes:
    r = subprocess.run([sys.executable] + cmd, cwd=ROOT, capture_output=True, text=True)
    d = json.loads(r.stdout)
    pc = d.get("problem_count", len(d.get("problems", [])))
    print(f"{label:15s} checked={d['checked']} problems={pc} status={d['status']}")
    all_pass &= d["status"] == "PASS"

print("\n== step 6: phase32 adversarial audit ==")
r = subprocess.run([sys.executable, "scripts/phase32-adversarial-audit.py", "."],
                   cwd=ROOT, capture_output=True, text=True)
lines = [l for l in r.stdout.splitlines() if l.strip().startswith('"')]
# last JSON doc = phase32 result
start = r.stdout.rfind('{\n  "schema": "aethercore.phase32')
d = json.loads(r.stdout[start:])
print(f"checks={d['checks']} failures={len(d['failures'])} status={d['status']}")

ok = all_pass and identical and d["status"] == "PASS"
print(f"\nPART_A_SEAL_OK: {ok}")
sys.exit(0 if ok else 1)
