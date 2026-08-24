#!/usr/bin/env bash
# Reconstruct the Phase 23.1 tree from the sealed Phase 20 source.
set -euo pipefail
PHASE23="${1:?usage: apply_phase23_1_patch.sh <phase20-tree> <destination>}"
DEST="${2:?usage: apply_phase23_1_patch.sh <phase20-tree> <destination>}"
HERE="$(cd "$(dirname "$0")" && pwd)"
cp -R "$PHASE23" "$DEST"
cd "$DEST"
patch -p1 --batch < "$HERE/changes.patch" || true
cp -R "$HERE/new-files/." "$DEST/"
echo "Applied Phase 23.1 patch. Run verify_phase23_1.py to check integrity."
