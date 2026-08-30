#!/usr/bin/env python3
"""Deterministic Phase 23 master archive builder.

Fixed mtimes, fixed member order, gzip -m level → identical bytes across rebuilds.
"""
from __future__ import annotations

import hashlib
import gzip
import sys
import tarfile
from pathlib import Path

SRC = Path(sys.argv[1]).resolve()
OUT = Path(sys.argv[2]).resolve()
EXCLUDE_PARTS = {"target", "node_modules", ".git", ".hermes"}

def included(path: Path) -> bool:
    rel = path.relative_to(SRC)
    return not (set(rel.parts) & EXCLUDE_PARTS)

members = sorted(
    (p for p in SRC.rglob("*") if p.is_file() and included(p)),
    key=lambda p: str(p.relative_to(SRC)),
)
if OUT.exists():
    OUT.unlink()
with open(OUT, "wb") as raw_out:
    # filename="" and mtime=0 keep the gzip header constant across rebuilds.
    with gzip.GzipFile(filename="", mode="wb", fileobj=raw_out, compresslevel=9, mtime=0) as gz:
        with tarfile.open(fileobj=gz, mode="w", format=tarfile.GNU_FORMAT) as tar:
            for path in members:
                arcname = "AetherCore-Phase23-Master-Delivery/" + str(path.relative_to(SRC))
                info = tar.gettarinfo(str(path), arcname=arcname)
                info.mtime = 1787000000          # fixed timestamp for determinism
                info.uid = info.gid = 501
                info.uname = info.gname = "aethercore"
                info.mode = 0o644 if path.suffix != ".sh" else 0o755
                with path.open("rb") as handle:
                    tar.addfile(info, handle)

digest = hashlib.sha256(OUT.read_bytes()).hexdigest()
print(digest, OUT)
