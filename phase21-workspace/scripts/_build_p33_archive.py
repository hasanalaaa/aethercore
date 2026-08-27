#!/usr/bin/env python3
"""Build the complete Phase 33 archive twice and seal its external hash."""
from __future__ import annotations

import hashlib
import os
import pathlib
import shutil
import subprocess

ROOT = pathlib.Path(__file__).resolve().parents[1]
OUT = ROOT.parent
NAME = "AetherCore-Phase33-Master-Delivery"
EXCLUDED_DIRS = {
    "target",
    "node_modules",
    ".git",
    "dist",
    "state",
    "support-staging",
    "__pycache__",
}


def sha256(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def build(destination: pathlib.Path) -> None:
    if destination.exists():
        destination.unlink()
    staging = pathlib.Path("/tmp/p33_archive_stage")
    shutil.rmtree(staging, ignore_errors=True)
    stage_root = staging / NAME
    stage_root.mkdir(parents=True)
    for path in sorted(ROOT.rglob("*")):
        rel = path.relative_to(ROOT)
        if (
            path.is_dir()
            or rel.name == ".DS_Store"
            or any(part in EXCLUDED_DIRS for part in rel.parts)
        ):
            continue
        target = stage_root / rel
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(path, target)
    list_file = staging / "_files.txt"
    entries = sorted(
        str(path.relative_to(staging)) for path in stage_root.rglob("*") if path.is_file()
    )
    list_file.write_text("\n".join(entries) + "\n", encoding="utf-8")
    environment = dict(os.environ, SOURCE_DATE_EPOCH="1787600000", COPYFILE_DISABLE="1")
    with destination.open("wb") as output:
        tar = subprocess.Popen(
            [
                "tar",
                "--no-xattrs",
                "--uid=0",
                "--gid=0",
                "--uname=",
                "--gname=",
                "-cf",
                "-",
                "-C",
                str(staging),
                "--files-from",
                str(list_file),
            ],
            stdout=subprocess.PIPE,
            env=environment,
        )
        assert tar.stdout is not None
        gzip = subprocess.Popen(["gzip", "-n", "-1"], stdin=tar.stdout, stdout=output)
        tar.stdout.close()
        gzip.communicate()
        if tar.wait() != 0 or gzip.returncode != 0:
            raise SystemExit("archive pipeline failed")
    shutil.rmtree(staging, ignore_errors=True)


hashes = []
destination = OUT / f"{NAME}.zip"
for run in (1, 2):
    build(destination)
    digest = sha256(destination)
    hashes.append(digest)
    print(f"run{run}: {digest}")
identical = hashes[0] == hashes[1]
print("ARCHIVE_TWICE_IDENTICAL:", identical)
if not identical:
    raise SystemExit(1)
(OUT / "PHASE33_FINAL_SHA256.txt").write_text(
    f"{hashes[0]}  {NAME}.zip\n", encoding="utf-8"
)
