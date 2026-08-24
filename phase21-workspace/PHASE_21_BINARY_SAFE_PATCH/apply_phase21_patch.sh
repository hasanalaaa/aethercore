#!/usr/bin/env bash
# Reconstruct the Phase 21 tree from the sealed Phase 20 source.
set -euo pipefail
PHASE20="${1:?usage: apply_phase21_patch.sh <phase20-tree> <destination>}"
DEST="${2:?usage: apply_phase21_patch.sh <phase20-tree> <destination>}"
HERE="$(cd "$(dirname "$0")" && pwd)"
cp -R "$PHASE20" "$DEST"
cd "$DEST"
patch -p1 --batch < "$HERE/changes.patch" || true
cp -R "$HERE/new-files/." "$DEST/"
echo "Applied Phase 21 patch. Run verify_phase21.py to check integrity."
