#!/usr/bin/env python3
"""GH gate: deterministic Phase 32 master archive built twice with identical SHA-256."""
import hashlib
import os
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = (
    Path(sys.argv[1]).resolve()
    if len(sys.argv) > 1
    else Path(__file__).resolve().parents[1]
)
OUT = (
    Path(sys.argv[2]).resolve()
    if len(sys.argv) > 2
    else Path(__file__).resolve().parents[2]
)
NAME = "AetherCore-Phase32-Master-Delivery"
EXCLUDE_DIRS = {"target", "node_modules", ".git", "dist", "state", "support-staging"}
SKIP_PREFIXES = ("C:\\ProgramData",)


def build(dest: Path):
    if dest.exists():
        dest.unlink()
    staging = Path("/tmp/p32_archive_stage")
    if staging.exists():
        shutil.rmtree(staging)
    stage_root = staging / NAME
    stage_root.mkdir(parents=True)
    for path in sorted(ROOT.rglob("*")):
        rel = path.relative_to(ROOT)
        if set(rel.parts) & EXCLUDE_DIRS:
            continue
        s = str(rel)
        if any(s.startswith(p) for p in SKIP_PREFIXES):
            continue
        # P33 seal-closure (A2): Finder noise must never enter the seal — a
        # mid-seal .DS_Store regeneration is the exact failure that produced the
        # P32 crates/.DS_Store blemish.
        if rel.name == ".DS_Store":
            continue
        if path.is_dir():
            continue
        dest_path = stage_root / rel
        dest_path.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(path, dest_path)
    env = dict(os.environ, SOURCE_DATE_EPOCH="1787600000", COPYFILE_DISABLE="1")
    list_file = staging / "_files.txt"
    entries = sorted(str(p.relative_to(staging)) for p in staging.rglob("*") if p.is_file())
    list_file.write_text("\n".join(entries) + "\n")
    tar_cmd = [
        "tar", "--no-xattrs", "--uid=0", "--gid=0", "--uname=", "--gname=",
        "-cf", "-", "-C", str(staging), "--files-from", str(list_file),
    ]
    tar = subprocess.Popen(tar_cmd, stdout=subprocess.PIPE, env=env)
    # Level 1 keeps the archive byte-deterministic while making the seal
    # feasible on this host's multi-gigabyte binary tree.
    gzip = subprocess.Popen(["gzip", "-n", "-1"], stdin=tar.stdout, stdout=open(dest, "wb"))
    tar.stdout.close()
    gzip.communicate()
    assert tar.wait() == 0 and gzip.returncode == 0
    shutil.rmtree(staging)


hashes = []
for run in (1, 2):
    dest = OUT / f"{NAME}.zip"
    build(dest)
    h = hashlib.sha256(dest.read_bytes()).hexdigest()
    hashes.append(h)
    print(f"run{run}: {h}")

identical = hashes[0] == hashes[1]
print("ARCHIVE_TWICE_IDENTICAL:", identical)
if identical:
    (OUT / "PHASE32_FINAL_SHA256.txt").write_text(f"{hashes[0]}  {NAME}.zip\n")
