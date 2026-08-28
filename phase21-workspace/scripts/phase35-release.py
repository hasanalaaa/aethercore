#!/usr/bin/env python3
"""Deterministic Phase 35 release evidence and offline bundle tool.

The tool never creates production private keys and never publishes artifacts.  It
uses a fixed epoch (SOURCE_DATE_EPOCH, or an explicit --epoch) and sorted JSON/
ZIP members so the same source inputs produce the same unsigned evidence.
"""
from __future__ import annotations

import argparse, hashlib, json, os, re, struct, subprocess, sys, zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "release" / "phase35"
MAX_PACKAGE = 2 * 1024 * 1024 * 1024
SCHEMAS = {
    "identity": "aethercore.release.identity.v1",
    "manifest": "aethercore.release.manifest.v1",
    "metadata": "aethercore.update.metadata.v1",
    "provenance": "aethercore.release.provenance.v1",
    "sbom": "CycloneDX-1.5",
}

def digest(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as fh:
        for block in iter(lambda: fh.read(1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()

def write_json(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    # Canonical JSON is the exact UTF-8 byte sequence: no trailing newline or
    # insignificant whitespace is part of a signed/digested artifact.
    path.write_bytes(json.dumps(value, ensure_ascii=False, sort_keys=False, separators=(",", ":")).encode())

def workspace_version() -> str:
    text = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    match = re.search(r"(?ms)^\[workspace\.package\].*?^version\s*=\s*\"([^\"]+)\"", text)
    if not match:
        raise SystemExit("workspace version is missing")
    return match.group(1)

def identity(args: argparse.Namespace) -> dict[str, object]:
    version = args.version or workspace_version()
    epoch = int(args.epoch if args.epoch is not None else os.environ.get("SOURCE_DATE_EPOCH", "0"))
    architecture = "x86-64" if args.architecture == "x86_64" else args.architecture
    value = {"schema": SCHEMAS["identity"], "productId": "AetherCore", "version": version,
             "channel": args.channel, "releaseSequence": args.sequence, "platform": args.platform,
             "architecture": architecture, "protocolVersion": "p35", "minimumUpdaterVersion": workspace_version(),
             "sourceCommit": args.commit, "buildEpoch": epoch}
    return value

def sbom(path: Path) -> dict[str, object]:
    lock = ROOT / "Cargo.lock"
    packages = []
    data = lock.read_text(encoding="utf-8")
    for block in re.split(r"\n\[\[package\]\]\n", "\n" + data)[1:]:
        name = re.search(r'^name = "([^"]+)"', block, re.M)
        version = re.search(r'^version = "([^"]+)"', block, re.M)
        if name and version:
            packages.append({"type": "library", "bom-ref": f"pkg:cargo/{name.group(1)}@{version.group(1)}", "name": name.group(1), "version": version.group(1), "purl": f"pkg:cargo/{name.group(1)}@{version.group(1)}"})
    packages.sort(key=lambda v: (v["name"], v["version"]))
    value = {"bomFormat": "CycloneDX", "specVersion": "1.5", "version": 1, "metadata": {"component": {"type": "application", "name": "AetherCore", "version": workspace_version()}}, "components": packages}
    write_json(path, value)
    return value

def manifest(path: Path, identity_path: Path, package: Path, sbom_path: Path, provenance_path: Path) -> dict[str, object]:
    ident = json.loads(identity_path.read_text(encoding="utf-8"))
    value = {"schema": SCHEMAS["manifest"], "identity": ident,
             "package": {"packageType": "offline-zip", "filename": package.name, "byteLength": package.stat().st_size, "sha256": digest(package)},
             "sbomSha256": digest(sbom_path), "sbomReference": sbom_path.name,
             "provenanceSha256": digest(provenance_path), "provenanceReference": provenance_path.name,
             "createdEpoch": int(os.environ.get("SOURCE_DATE_EPOCH", "0")), "updateContractVersion": "aethercore.update-contract.v1"}
    write_json(path, value)
    return value

def safe_member(name: str) -> bool:
    p = Path(name)
    return bool(name) and not p.is_absolute() and not name.startswith(("/", "\\")) and ".." not in p.parts and not re.match(r"^[A-Za-z]:", name) and not name.startswith("\\\\") and "\\" not in name and not name.endswith("/")

def build_bundle(bundle: Path, files: list[Path]) -> None:
    bundle.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(bundle, "w", compression=zipfile.ZIP_STORED, allowZip64=True) as zf:
        for path in sorted(files, key=lambda p: p.name):
            if path.name in {".DS_Store"} or path.name.endswith((".key", ".pem", ".pfx")) or "target" in path.parts or "node_modules" in path.parts:
                raise SystemExit(f"forbidden offline bundle member: {path}")
            info = zipfile.ZipInfo(path.name, date_time=(1980, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_STORED
            info.external_attr = 0o100644 << 16
            zf.writestr(info, path.read_bytes())

def verify_bundle(bundle: Path) -> dict[str, object]:
    with zipfile.ZipFile(bundle) as zf:
        names = zf.namelist()
        if len(names) != len(set(names)):
            raise SystemExit("duplicate offline bundle member")
        if any(not safe_member(name) for name in names):
            raise SystemExit("unsafe offline bundle member")
        if any(name == ".DS_Store" or name.endswith((".key", ".pem", ".pfx")) for name in names):
            raise SystemExit("secret or developer artifact in offline bundle")
        infos = [zf.getinfo(name) for name in names]
        if sum(info.file_size for info in infos) > MAX_PACKAGE:
            raise SystemExit("offline bundle exceeds bounded size")
    return {"status": "verified", "bundleSha256": digest(bundle), "members": sorted(names), "network": "not-required"}

def main() -> int:
    parser = argparse.ArgumentParser()
    sub = parser.add_subparsers(dest="command", required=True)
    p_id = sub.add_parser("identity"); p_id.add_argument("--out", required=True); p_id.add_argument("--version"); p_id.add_argument("--channel", choices=("stable", "beta", "dev"), default="stable"); p_id.add_argument("--sequence", type=int, default=1); p_id.add_argument("--platform", choices=("windows", "macos", "linux"), default="windows"); p_id.add_argument("--architecture", choices=("x86_64", "aarch64"), default="x86_64"); p_id.add_argument("--commit", default=None); p_id.add_argument("--epoch", type=int)
    p_sb = sub.add_parser("sbom"); p_sb.add_argument("--out", required=True)
    p_m = sub.add_parser("manifest"); p_m.add_argument("--out", required=True); p_m.add_argument("--identity", required=True); p_m.add_argument("--package", required=True); p_m.add_argument("--sbom", required=True); p_m.add_argument("--provenance", required=True)
    p_pr = sub.add_parser("provenance"); p_pr.add_argument("--out", required=True); p_pr.add_argument("--package", required=True); p_pr.add_argument("--identity", required=True); p_pr.add_argument("--sbom", required=True); p_pr.add_argument("--epoch", type=int)
    p_b = sub.add_parser("bundle"); p_b.add_argument("--out", required=True); p_b.add_argument("files", nargs="+")
    p_v = sub.add_parser("verify-bundle"); p_v.add_argument("bundle")
    args = parser.parse_args()
    if args.command == "identity": write_json(Path(args.out), identity(args)); return 0
    if args.command == "sbom": sbom(Path(args.out)); return 0
    if args.command == "manifest": manifest(Path(args.out), Path(args.identity), Path(args.package), Path(args.sbom), Path(args.provenance)); return 0
    if args.command == "provenance":
        epoch = int(args.epoch if args.epoch is not None else os.environ.get("SOURCE_DATE_EPOCH", "0"))
        value = {"schema": SCHEMAS["provenance"], "productId": "AetherCore", "version": workspace_version(), "authoritativeBasePhase": "Phase 34", "builder": {"hostPlatform": sys.platform, "python": sys.version.split()[0]}, "toolchain": {"rust": "rust-toolchain.toml", "cargo": "Cargo.lock", "node": "package.json", "tauri": "apps/desktop/tauri.conf.json"}, "buildCommands": ["cargo test --workspace --jobs 2", "pnpm --dir apps/ui check"], "packageSha256": digest(Path(args.package)), "releaseIdentitySha256": digest(Path(args.identity)), "sbomSha256": digest(Path(args.sbom)), "createdEpoch": epoch, "reproducibility": "deterministic-unsigned-evidence", "signing": {"production": "NotAvailable", "authenticode": "NotAvailable"}, "windowsRuntimeQualification": "Phase 36 / NotAvailable"}
        write_json(Path(args.out), value); return 0
    if args.command == "bundle": build_bundle(Path(args.out), [Path(v) for v in args.files]); return 0
    if args.command == "verify-bundle": print(json.dumps(verify_bundle(Path(args.bundle)), sort_keys=True)); return 0
    return 2

if __name__ == "__main__": raise SystemExit(main())
