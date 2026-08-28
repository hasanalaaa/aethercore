#!/usr/bin/env python3
"""Build the true P34→P35 binary-safe delta and scoped full-tree ledger."""
from __future__ import annotations
import difflib, hashlib, json, shutil, subprocess, sys
from pathlib import Path

ROOT = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else Path(__file__).resolve().parents[1]
PATCH = ROOT / "PHASE_35_BINARY_SAFE_PATCH"
ARCHIVE = ROOT.parent / "AetherCore-Phase34-Master-Delivery.zip"
HASH_FILE = ROOT.parent / "PHASE34_FINAL_SHA256.txt"
BASE = Path("/tmp/p35_p34_seal/base")
EXCLUDED = {"target", "node_modules", ".git", "dist", "state", "support-staging", "__pycache__"}
SELF_PREFIX = "PHASE_35_BINARY_SAFE_PATCH/"

def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as fh:
        for block in iter(lambda: fh.read(1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()

def excluded(root: Path, path: Path) -> bool:
    rel = path.relative_to(root)
    return path.name == ".DS_Store" or any(p in EXCLUDED for p in rel.parts) or str(rel).startswith(SELF_PREFIX)

def files(root: Path) -> dict[str, Path]:
    return {str(p.relative_to(root)): p for p in sorted(root.rglob("*")) if p.is_file() and not excluded(root, p)}

def utf8(path: Path) -> bool:
    try:
        path.read_bytes().decode("utf-8")
        return True
    except (UnicodeDecodeError, OSError):
        return False

expected = HASH_FILE.read_text(encoding="utf-8").split()[0]
if sha256(ARCHIVE) != expected or expected != "9c6aabb431fcb117afa19bb519db58dddb338dd63d31c2644db2490b084e419b":
    raise SystemExit("FATAL: P34 archive SHA does not match the authoritative baseline")
marker = BASE / "PHASE_34_BINARY_SAFE_PATCH" / "verify_phase34.py"
if not marker.is_file():
    BASE.parent.mkdir(parents=True, exist_ok=True)
    if BASE.exists():
        shutil.rmtree(BASE)
    BASE.mkdir(parents=True)
    subprocess.run(["tar", "-xzf", str(ARCHIVE), "-C", str(BASE), "--strip-components", "1"], check=True)

live, base = files(ROOT), files(BASE)
added = sorted(set(live) - set(base))
removed = sorted(set(base) - set(live))
modified = sorted(rel for rel in set(live) & set(base) if sha256(live[rel]) != sha256(base[rel]))
PATCH.mkdir(parents=True, exist_ok=True)
binary_root = PATCH / "BINARY_ARTIFACTS"
if binary_root.exists():
    shutil.rmtree(binary_root)
text_diffs: list[str] = []
binary_files: list[str] = []

def diff(old: Path | None, new: Path | None, rel: str) -> None:
    # Patch(1) normalizes missing EOF newlines. Canonical JSON artifacts must
    # retain their exact bytes, so carry newline-less files in the binary lane.
    newline_sensitive = (old is not None and not old.read_bytes().endswith(b"\n")) or (new is not None and not new.read_bytes().endswith(b"\n"))
    if newline_sensitive or (old is not None and not utf8(old)) or (new is not None and not utf8(new)):
        if new is not None:
            destination = binary_root / rel
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(new, destination)
            binary_files.append(rel)
        return
    old_lines = old.read_text(encoding="utf-8").splitlines(True) if old else []
    new_lines = new.read_text(encoding="utf-8").splitlines(True) if new else []
    text_diffs.extend(difflib.unified_diff(old_lines, new_lines, fromfile=f"a/{rel}" if old else "/dev/null", tofile=f"b/{rel}" if new else "/dev/null", n=0))

for rel in modified: diff(base[rel], live[rel], rel)
for rel in added: diff(None, live[rel], rel)
for rel in removed: diff(base[rel], None, rel)
(PATCH / "changes.patch").write_text("".join(text_diffs), encoding="utf-8")
manifest = {"schema":"aethercore.phase35.binary-safe-patch.v1", "base":ARCHIVE.name, "base_sha256":expected, "files":modified + [f"+{r}" for r in added] + [f"-{r}" for r in removed], "modifiedCount":len(modified), "addedCount":len(added), "removedCount":len(removed), "binaryFiles":sorted(binary_files), "sha256":{r:sha256(live[r]) for r in sorted(set(modified + added))}}
(PATCH / "MANIFEST.json").write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8")
ledger = {rel:sha256(path) for rel,path in sorted(live.items())}
(PATCH / "PHASE_35_EXPECTED_FULL_SHA256.json").write_text(json.dumps({"schema":"aethercore.phase35.full-sha256.v1", "base":ARCHIVE.name, "files":ledger}, indent=2, sort_keys=True) + "\n", encoding="utf-8")
print(f"modified={len(modified)} added={len(added)} removed={len(removed)} binary={len(binary_files)} manifest={len(manifest['files'])} ledger={len(ledger)}")
