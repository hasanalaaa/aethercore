#!/usr/bin/env python3
"""Regenerate MANIFEST.sha256 deterministically from deliverable source bytes."""
from __future__ import annotations
import hashlib
import sys
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
MANIFEST=ROOT/'MANIFEST.sha256'
EXCLUDED_DIRS={'.git','target','node_modules','out','__pycache__'}
EXCLUDED_SUFFIXES={'.pyc','.pyo'}

def included(path:Path)->bool:
    rel=path.relative_to(ROOT)
    return path.is_file() and not path.is_symlink() and path != MANIFEST and not any(part in EXCLUDED_DIRS for part in rel.parts) and path.suffix not in EXCLUDED_SUFFIXES

def source_symlinks()->list[Path]:
    return sorted(
        (p for p in ROOT.rglob('*') if p.is_symlink() and not any(part in EXCLUDED_DIRS for part in p.relative_to(ROOT).parts)),
        key=lambda p:p.relative_to(ROOT).as_posix(),
    )

def sha(path:Path)->str:
    h=hashlib.sha256()
    with path.open('rb') as f:
        for chunk in iter(lambda:f.read(1024*1024),b''): h.update(chunk)
    return h.hexdigest()

def main()->int:
    symlinks=source_symlinks()
    if symlinks:
        print("Refusing to generate a manifest for a source tree containing symlinks:", file=sys.stderr)
        for path in symlinks[:32]:
            print(f"  {path.relative_to(ROOT).as_posix()} -> {path.readlink()}", file=sys.stderr)
        return 2
    files=sorted((p for p in ROOT.rglob('*') if included(p)),key=lambda p:p.relative_to(ROOT).as_posix())
    MANIFEST.write_text(''.join(f"{sha(p)}  {p.relative_to(ROOT).as_posix()}\n" for p in files),encoding='utf-8',newline='\n')
    print(f"MANIFEST.sha256 regenerated from {len(files)} deliverable files (manifest excludes itself).")
    return 0
if __name__=='__main__': raise SystemExit(main())
