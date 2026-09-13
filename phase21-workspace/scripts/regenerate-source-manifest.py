#!/usr/bin/env python3
"""Regenerate MANIFEST.sha256 over the delivered source tree.

Scope, defended in docs/SOURCE_SEAL.md: every git-tracked file under the sealed
root. Tracked is the definition of delivered, and it is the one rule that cannot
drift -- the directory walk this replaced swept in nine `apps/ui/dist/` build
outputs, the downloaded model .gguf, a release .zip and three SQLite files under
a literal `C:\\ProgramData` directory, none of which its exclusion list caught
(DBT-P59-003).

Reads nothing from the previous manifest: a generator that consulted it could
carry a stale entry forward. `scripts/source_seal.py` is the verifier.
"""
from __future__ import annotations
import argparse
import sys
from pathlib import Path

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
from source_seal import MANIFEST_NAME, SealError, sha256, tracked_files  # noqa: E402

ROOT = Path(__file__).resolve().parents[1]


def source_symlinks(root: Path, files: set[str]) -> list[Path]:
    return sorted((root / f for f in files if (root / f).is_symlink()), key=lambda p: p.as_posix())


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--root", type=Path, default=ROOT)
    args = ap.parse_args()
    root = args.root.resolve()
    try:
        files = tracked_files(root)
    except SealError as exc:
        print(f"Cannot determine the delivered file set: {exc}", file=sys.stderr)
        return 2
    symlinks = source_symlinks(root, files)
    if symlinks:
        print("Refusing to generate a manifest for a source tree containing symlinks:", file=sys.stderr)
        for path in symlinks[:32]:
            print(f"  {path.relative_to(root).as_posix()} -> {path.readlink()}", file=sys.stderr)
        return 2
    absent = sorted(f for f in files if not (root / f).is_file())
    if absent:
        print("Refusing to seal a tree whose tracked files are not all present:", file=sys.stderr)
        for rel in absent[:32]:
            print(f"  {rel}", file=sys.stderr)
        return 2
    ordered = sorted(files)
    (root / MANIFEST_NAME).write_text(
        "".join(f"{sha256(root / rel)}  {rel}\n" for rel in ordered),
        encoding="utf-8",
        newline="\n",
    )
    print(f"{MANIFEST_NAME} regenerated over {len(ordered)} tracked files in {root} (manifest excludes itself).")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
