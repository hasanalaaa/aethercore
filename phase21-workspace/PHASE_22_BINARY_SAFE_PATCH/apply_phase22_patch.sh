#!/usr/bin/env bash
# Reconstruct the Phase 22 tree from the sealed Phase 20 source.
set -euo pipefail
PHASE21="${1:?usage: apply_phase22_patch.sh <phase20-tree> <destination>}"
DEST="${2:?usage: apply_phase22_patch.sh <phase20-tree> <destination>}"
HERE="$(cd "$(dirname "$0")" && pwd)"
cp -R "$PHASE21" "$DEST"
cd "$DEST"
patch -p1 --batch < "$HERE/changes.patch" || true
cp -R "$HERE/new-files/." "$DEST/"
echo "Applied Phase 22 patch. Run verify_phase22.py to check integrity."
