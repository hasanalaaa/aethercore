#!/usr/bin/env python3
# Verify a reconstructed Phase 23 tree against MANIFEST.json.
import hashlib, json, sys
from pathlib import Path
tree = Path(sys.argv[1]).resolve()
manifest = json.loads((Path(__file__).parent / "MANIFEST.json").read_text())
bad = []
for rel, expected in manifest["files"].items():
    path = tree / rel
    if not path.is_file():
        bad.append(f"missing: {rel}")
        continue
    digest = hashlib.sha256(path.read_bytes()).hexdigest()
    if digest != expected:
        bad.append(f"hash mismatch: {rel}")
print(json.dumps({"status": "PASS" if not bad else "FAIL", "checked": len(manifest['files']), "problems": bad[:40]}, indent=2))
sys.exit(0 if not bad else 1)
