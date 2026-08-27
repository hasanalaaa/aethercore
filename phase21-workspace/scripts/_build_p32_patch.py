#!/usr/bin/env python3
"""Builds PHASE_32_BINARY_SAFE_PATCH (dual-mode verify + full-tree ledger) relative to
the sealed P31 archive, using deterministic diff headers.

Ledger scoping rules (P31 lessons applied, documented in the JSON `note`):
  - EXCLUDED_DIRS: build/dependency noise never hashed.
  - ALL later-phase patch dirs (PHASE_30_/PHASE_31_/PHASE_32_BINARY_SAFE_PATCH)
    are self-referential deliverables and excluded from the full-tree ledger;
    their integrity is proven by GG round-trips + phase audits instead.
  - .DS_Store excluded (Finder noise; post-seal regeneration is not drift).
"""
import hashlib
import json
import os
import pathlib
import shutil
import subprocess
import sys

ROOT = pathlib.Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else pathlib.Path(__file__).resolve().parents[1]
PATCH = ROOT / "PHASE_32_BINARY_SAFE_PATCH"
SEAL = pathlib.Path("/tmp/p32_seal_extract/x")
EXCLUDED_DIRS = {"target", "node_modules", ".git", "dist", "state", "support-staging",
                 "__pycache__"}
SELF_LEDGER = "PHASE_32_BINARY_SAFE_PATCH/PHASE_32_EXPECTED_FULL_SHA256.json"


def sha256(p: pathlib.Path) -> bytes:
    return hashlib.sha256(p.read_bytes()).digest()


PATCH.mkdir(parents=True, exist_ok=True)

# Extract the P31 seal ONCE per machine into /tmp cache.
z_candidates = [
    ROOT.parent / "AetherCore-Phase31-Master-Delivery.zip",
]
if not SEAL.exists():
    z = next((c for c in z_candidates if c.exists()), None)
    if z is None:
        print("FATAL: sealed P31 archive not found for diff base", file=sys.stderr)
        sys.exit(2)
    SEAL.mkdir(parents=True)
    subprocess.run(["tar", "-xzf", str(z), "-C", str(SEAL), "--strip-components", "1"],
                   check=True, capture_output=True)

changed, added = [], []

def is_excluded(rel: str, path) -> bool:
    if any(part in EXCLUDED_DIRS for part in path.parts):
        return True
    if rel.startswith(("PHASE_30_BINARY_SAFE_PATCH/", "PHASE_31_BINARY_SAFE_PATCH/",
                       "PHASE_32_BINARY_SAFE_PATCH")):
        return True
    if rel.endswith(".DS_Store"):
        return True
    return False

for path in sorted(ROOT.rglob("*")):
    rel = str(path.relative_to(ROOT))
    if is_excluded(rel, path):
        continue
    if path.is_dir() or not path.is_file():
        continue
    sealed = SEAL / rel
    if not sealed.exists():
        added.append(rel)
    elif sealed.read_bytes() != path.read_bytes():
        changed.append(rel)

removed = []
for path in sorted(SEAL.rglob("*")):
    if not path.is_file():
        continue
    rel = str(path.relative_to(SEAL))
    if any(part in EXCLUDED_DIRS for part in path.parts):
        continue
    if not (ROOT / rel).exists():
        removed.append(rel)

print(f"modified={len(changed)} added={len(added)} removed={len(removed)}")

# ---- deterministic unified diffs (-U3) with normalized headers -------------------
# Added files get a full-content /dev/null -> file diff so the apply script can
# reconstruct them byte-exactly (P31 lesson applied to NEW files this time).
import tempfile
diffs = []
for f in sorted(set(changed)):
    r = subprocess.run(
        ["diff", "-U0", "-u",
         "--label", f"a/{f}", "--label", f"b/{f}",
         str(SEAL / f), str(ROOT / f)],
        capture_output=True, text=True)
    if r.stdout:
        diffs.append(r.stdout)
for f in sorted(set(added)):
    r = subprocess.run(
        ["diff", "-U0", "-u", "-N",
         "--label", "/dev/null", "--label", f"b/{f}",
         "/dev/null", str(ROOT / f)],
        capture_output=True, text=True)
    if r.stdout:
        diffs.append(r.stdout)

changes_patch = PATCH / "changes.patch"
changes_patch.write_text("".join(diffs), encoding="utf-8")

# ---- MANIFEST: sha256 of every changed+added file after the change ---------------
manifest_files = sorted(set(changed + added))
sha_map = {}
for f in manifest_files:
    sha_map[f] = hashlib.sha256((ROOT / f).read_bytes()).hexdigest()
manifest = {
    "schema": "aethercore.phase32.binary-safe-patch.v1",
    "base": "AetherCore-Phase31-Master-Delivery.zip",
    "files": ["+" + f for f in sorted(added)] + [f for f in sorted(changed)],
    "sha256": sha_map,
}
(PATCH / "MANIFEST.json").write_text(json.dumps(manifest, indent=1) + "\n", encoding="utf-8")

# ---- FULL-TREE ledger ------------------------------------------------------------
ledger_files = {}
for path in sorted(ROOT.rglob("*")):
    rel = str(path.relative_to(ROOT))
    if any(part in EXCLUDED_DIRS for part in path.parts):
        continue
    if rel.startswith(("PHASE_30_BINARY_SAFE_PATCH/", "PHASE_31_BINARY_SAFE_PATCH/",
                       "PHASE_32_BINARY_SAFE_PATCH")):
        continue
    if rel == ".DS_Store" or not path.is_file():
        continue
    ledger_files[rel] = hashlib.sha256(path.read_bytes()).hexdigest()

full = {
    "schema": "aethercore.phase32.full-sha256.v1",
    "base": "AetherCore-Phase31-Master-Delivery.zip",
    "note": (
        "Patch dirs PHASE_30_/PHASE_31_/PHASE_32_BINARY_SAFE_PATCH excluded as "
        "self-referential deliverables (P31 ledger-scoping lesson); .DS_Store "
        "excluded as Finder noise. Integrity of excluded dirs is proven by GG "
        "round-trips and phase audits."
    ),
    "files": ledger_files,
    "self_excluded": SELF_LEDGER,
}
(PATCH / "PHASE_32_EXPECTED_FULL_SHA256.json").write_text(
    json.dumps(full, indent=1) + "\n", encoding="utf-8")
print(f"ledger files={len(ledger_files)}")
