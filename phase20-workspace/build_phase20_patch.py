#!/usr/bin/env python3
"""Build the Phase 20 binary-safe reconstruction package.

Compares the delivered tree against the sealed Phase 19 source tree and emits:
  PHASE_20_BINARY_SAFE_PATCH/changes.patch   unified diff for modified files
  PHASE_20_BINARY_SAFE_PATCH/new-files/      full content of brand-new files
  PHASE_20_BINARY_SAFE_PATCH/MANIFEST.json   SHA-256 of every file in the delivered tree

Usage: python3 build_phase20_patch.py <phase19_tree> <delivered_tree> [out_dir]
"""
from __future__ import annotations

import difflib
import hashlib
import json
import shutil
import sys
from pathlib import Path

PHASE19 = Path(sys.argv[1]).resolve()
DELIVERED = Path(sys.argv[2]).resolve()
OUT = Path(sys.argv[3]).resolve() if len(sys.argv) > 3 else DELIVERED / "PHASE_20_BINARY_SAFE_PATCH"


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 16), b""):
            digest.update(chunk)
    return digest.hexdigest()


def all_files(root: Path) -> dict[str, Path]:
    return {
        str(path.relative_to(root)): path
        for path in sorted(root.rglob("*"))
        if path.is_file() and "target/" not in str(path.relative_to(root)) and "node_modules" not in str(path.relative_to(root))
    }


def main() -> int:
    if OUT.exists():
        shutil.rmtree(OUT)
    new_dir = OUT / "new-files"
    new_dir.mkdir(parents=True)

    old_files = all_files(PHASE19)
    new_files = all_files(DELIVERED)

    modified: list[str] = []
    added: list[str] = []
    removed: list[str] = []

    patch_parts: list[str] = []

    for rel, old_path in old_files.items():
        delivered_path = new_files.get(rel)
        if delivered_path is None:
            removed.append(rel)
            continue
        if sha256(old_path) != sha256(delivered_path):
            modified.append(rel)
            try:
                a = old_path.read_text(encoding="utf-8").splitlines(keepends=True)
                b = delivered_path.read_text(encoding="utf-8").splitlines(keepends=True)
                diff = difflib.unified_diff(
                    a, b,
                    fromfile=f"a/{rel}",
                    tofile=f"b/{rel}",
                )
                patch_parts.append("".join(diff))
            except UnicodeDecodeError:
                patch_parts.append(f"--- BINARY FILE MODIFIED: {rel} (deliver via new-files copy)\n")

    for rel in sorted(set(new_files) - set(old_files)):
        added.append(rel)
        target = new_dir / rel
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(new_files[rel], target)

    (OUT / "changes.patch").write_text("".join(patch_parts), encoding="utf-8")

    manifest = {
        "schema": "aethercore.phase20.manifest.v1",
        "base": "AetherCore-Phase19-Source",
        "modifiedCount": len(modified),
        "addedCount": len(added),
        "removedCount": len(removed),
        "modified": modified,
        "removed": removed,
        "files": {rel: sha256(path) for rel, path in new_files.items()},
    }
    (OUT / "MANIFEST.json").write_text(json.dumps(manifest, indent=2), encoding="utf-8")

    apply_script = """#!/usr/bin/env bash
# Reconstruct the Phase 20 tree from the sealed Phase 19 source.
set -euo pipefail
PHASE19="${1:?usage: apply_phase20_patch.sh <phase19-tree> <destination>}"
DEST="${2:?usage: apply_phase20_patch.sh <phase19-tree> <destination>}"
HERE="$(cd "$(dirname "$0")" && pwd)"
cp -R "$PHASE19" "$DEST"
cd "$DEST"
patch -p1 --batch < "$HERE/changes.patch" || true
cp -R "$HERE/new-files/." "$DEST/"
echo "Applied Phase 20 patch. Run verify_phase20.sh to check integrity."
"""
    verify_script = """#!/usr/bin/env python3
# Verify a reconstructed tree against MANIFEST.json.
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
"""
    (OUT / "apply_phase20_patch.sh").write_text(apply_script, encoding="utf-8")
    (OUT / "verify_phase20.py").write_text(verify_script, encoding="utf-8")

    print(json.dumps({
        "outDir": str(OUT),
        "modified": len(modified),
        "added": len(added),
        "removed": len(removed),
        "manifestFiles": len(new_files),
    }, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
