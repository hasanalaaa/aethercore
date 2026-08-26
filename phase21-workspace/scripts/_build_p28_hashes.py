#!/usr/bin/env python3
"""Generates PHASE_28_EXPECTED_FULL_SHA256.json for the delivered tree and upgrades the
patch MANIFEST with per-file sha256 (restoring the full P28 hash standard)."""
import hashlib
import json
import pathlib
import sys

ROOT = pathlib.Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else pathlib.Path(__file__).resolve().parents[1]
PATCH = ROOT / "PHASE_28_BINARY_SAFE_PATCH"
EXCLUDED_DIRS = {"target", "node_modules", ".git", "dist", "state", "support-staging"}

files: dict[str, str] = {}
for path in sorted(ROOT.rglob("*")):
    if not path.is_file():
        continue
    rel = str(path.relative_to(ROOT))
    if any(part in EXCLUDED_DIRS for part in path.parts):
        continue
    files[rel] = hashlib.sha256(path.read_bytes()).hexdigest()

(PATCH / "PHASE_28_EXPECTED_FULL_SHA256.json").write_text(
    json.dumps({"schema": "aethercore.phase28.full-tree-sha256.v1", "files": files},
               indent=2, sort_keys=True) + "\n"
)

# Also upgrade MANIFEST.json: add sha256 per changed file (keep existing fields).
manifest = json.loads((PATCH / "MANIFEST.json").read_text())
manifest["sha256"] = {f[1:] if f.startswith("+") else f: files[f[1:] if f.startswith("+") else f]
                      for f in manifest.get("files", [])
                      if (f[1:] if f.startswith("+") else f) in files}
(PATCH / "MANIFEST.json").write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")
print(f"full-tree entries: {len(files)}; manifest hashed entries: {len(manifest['sha256'])}")
