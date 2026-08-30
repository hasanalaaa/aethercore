#!/usr/bin/env bash
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
