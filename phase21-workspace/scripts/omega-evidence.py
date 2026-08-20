#!/usr/bin/env python3
"""Generate source-of-truth Omega/Sigma verification evidence from executable gates.

Sigma integrity contract:
- verify the delivered source manifest before executing repository code;
- execute every repository-owned Python gate in an isolated disposable source clone;
- route generated evidence only to the caller-selected external evidence directory;
- detect any attempted mutation of the disposable audit source tree;
- prove the delivered source tree is byte-identical before/after verification;
- include source-manifest and execution-integrity state in the final success condition.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import re
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path, PurePosixPath

ROOT = Path(__file__).resolve().parents[1]
AUDITS = {
    "zenith_recursive": "scripts/zenith-recursive-audit.py",
    "zenith_adversarial": "scripts/zenith-adversarial-audit.py",
    "enterprise_adversarial": "scripts/enterprise-adversarial-audit.py",
    "static_validation": "scripts/static_validate.py",
    "phase13_reliability": "scripts/phase13-reliability-audit.py",
    "phase14_scheduler": "scripts/phase14-scheduler-audit.py",
    "phase15_security": "scripts/phase15-security-audit.py",
    "phase16_policy": "scripts/phase16-ga-audit.py",
}
EXCLUDED_MANIFEST_DIRS = {".git", "target", "node_modules", "out", "__pycache__"}
EXCLUDED_MANIFEST_SUFFIXES = {".pyc", ".pyo"}
EXECUTION_INTEGRITY: list[dict[str, object]] = []


def run(cmd: list[str], cwd: Path) -> dict[str, object]:
    started = time.monotonic()
    p = subprocess.run(cmd, cwd=cwd, text=True, capture_output=True)
    return {
        "command": cmd,
        "exit_code": p.returncode,
        "duration_ms": round((time.monotonic() - started) * 1000),
        "stdout": p.stdout,
        "stderr": p.stderr,
    }


def trailing_json(text: str):
    positions = [m.start() for m in re.finditer(r"\{", text)]
    for pos in reversed(positions):
        try:
            return json.loads(text[pos:])
        except Exception:
            pass
    return None


def sha(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for b in iter(lambda: f.read(1024 * 1024), b""):
            h.update(b)
    return h.hexdigest()


def tree_state(root: Path) -> dict[str, str]:
    state: dict[str, str] = {}
    for p in sorted(root.rglob("*"), key=lambda q: q.relative_to(root).as_posix()):
        rel = p.relative_to(root).as_posix()
        if p.is_symlink():
            state[rel] = "symlink:" + os.readlink(p)
        elif p.is_file():
            state[rel] = "file:" + sha(p)
    return state


def tree_digest(state: dict[str, str]) -> str:
    h = hashlib.sha256()
    for rel, value in sorted(state.items()):
        h.update(rel.encode("utf-8"))
        h.update(b"\0")
        h.update(value.encode("utf-8"))
        h.update(b"\n")
    return h.hexdigest()


def state_changes(before: dict[str, str], after: dict[str, str], limit: int = 32) -> list[dict[str, str]]:
    changes: list[dict[str, str]] = []
    for rel in sorted(set(before) | set(after)):
        b = before.get(rel)
        a = after.get(rel)
        if a == b:
            continue
        kind = "modified"
        if b is None:
            kind = "added"
        elif a is None:
            kind = "removed"
        changes.append({"path": rel, "change": kind})
        if len(changes) >= limit:
            break
    return changes


def run_repo_script(label: str, rel: str, args: list[str] | None = None) -> dict[str, object]:
    """Run a repository Python script against a disposable source clone and detect writes."""
    args = args or []
    with tempfile.TemporaryDirectory(prefix="aethercore-sigma-audit-") as temp:
        clone = Path(temp) / "source"
        shutil.copytree(ROOT, clone, symlinks=True)
        before = tree_state(clone)
        result = run([sys.executable, str(clone / rel), *args], clone)
        after = tree_state(clone)
        changes = state_changes(before, after)
        unchanged = before == after
        EXECUTION_INTEGRITY.append({
            "label": label,
            "script": rel,
            "source_tree_unchanged": unchanged,
            "changes": changes,
        })
        result["source_tree_unchanged"] = unchanged
        result["source_tree_changes"] = changes
        return result


def manifest_included(path: Path) -> bool:
    rel = path.relative_to(ROOT)
    return (
        path.is_file()
        and not path.is_symlink()
        and path != ROOT / "MANIFEST.sha256"
        and not any(part in EXCLUDED_MANIFEST_DIRS for part in rel.parts)
        and path.suffix not in EXCLUDED_MANIFEST_SUFFIXES
    )


def validate_manifest_relpath(rel: str) -> str | None:
    """Return a fail-closed reason for non-canonical/cross-platform-unsafe manifest paths."""
    if not rel or "\x00" in rel:
        return "invalid_path"
    # MANIFEST.sha256 is a portable POSIX-path format even when verification runs on Windows.
    # Backslashes/drive-like prefixes are rejected so one byte stream cannot name different files
    # on Linux and Windows. Dot segments and absolute paths are never meaningful source entries.
    if "\\" in rel or ":" in rel:
        return "non_portable_path"
    raw_parts = rel.split("/")
    if rel.startswith("/") or any(part in {"", ".", ".."} for part in raw_parts):
        return "path_escape_or_noncanonical"
    parsed = PurePosixPath(rel)
    if parsed.is_absolute() or parsed.as_posix() != rel:
        return "path_escape_or_noncanonical"
    return None


def path_has_symlink_component(root: Path, target: Path) -> bool:
    current = root
    for part in target.relative_to(root).parts:
        current = current / part
        if current.is_symlink():
            return True
    return False


def verify_manifest(path: Path) -> dict[str, object]:
    if not path.is_file():
        return {"ok": False, "reason": "missing MANIFEST.sha256", "verified": 0, "total": 0, "failed": []}
    total = 0
    verified = 0
    failed: list[dict[str, str]] = []
    listed: set[str] = set()
    for raw in path.read_text("utf-8").splitlines():
        if not raw.strip():
            continue
        total += 1
        m = re.fullmatch(r"([0-9a-fA-F]{64})  (.+)", raw)
        if not m:
            failed.append({"line": raw, "reason": "malformed"})
            continue
        expected, rel = m.groups()
        if rel in listed:
            failed.append({"path": rel, "reason": "duplicate"})
            continue
        listed.add(rel)
        path_reason = validate_manifest_relpath(rel)
        if path_reason:
            failed.append({"path": rel, "reason": path_reason})
            continue
        target = ROOT / PurePosixPath(rel)
        try:
            target.resolve(strict=False).relative_to(ROOT.resolve())
        except ValueError:
            failed.append({"path": rel, "reason": "path_escape"})
            continue
        if path_has_symlink_component(ROOT, target):
            failed.append({"path": rel, "reason": "symlink_not_allowed"})
            continue
        if not target.is_file():
            failed.append({"path": rel, "reason": "missing"})
            continue
        actual = sha(target)
        if actual.lower() != expected.lower():
            failed.append({"path": rel, "reason": "hash", "expected": expected.lower(), "actual": actual})
        else:
            verified += 1
    actual_files = {
        p.relative_to(ROOT).as_posix()
        for p in ROOT.rglob("*")
        if manifest_included(p)
    }
    source_symlinks = sorted(
        p.relative_to(ROOT).as_posix()
        for p in ROOT.rglob("*")
        if p.is_symlink()
        and not any(part in EXCLUDED_MANIFEST_DIRS for part in p.relative_to(ROOT).parts)
    )
    for rel in source_symlinks:
        if not any(item.get("path") == rel and item.get("reason") == "symlink_not_allowed" for item in failed):
            failed.append({"path": rel, "reason": "symlink_not_allowed"})
    unlisted = sorted(actual_files - listed)
    stale = sorted(listed - actual_files)
    for rel in unlisted:
        failed.append({"path": rel, "reason": "unlisted"})
    for rel in stale:
        if not any(item.get("path") == rel and item.get("reason") == "missing" for item in failed):
            failed.append({"path": rel, "reason": "stale_manifest_entry"})
    return {
        "ok": not failed,
        "verified": verified,
        "total": total,
        "listed_files": len(listed),
        "deliverable_files": len(actual_files),
        "symlink_entries": source_symlinks,
        "failed": failed,
    }


def cleanliness() -> dict[str, object]:
    bad: list[str] = []
    generated = {"out", "target", "node_modules", "__pycache__", ".devdata"}
    for p in ROOT.rglob("*"):
        rel = p.relative_to(ROOT).as_posix()
        name = p.name
        if (
            p.is_symlink()
            or "__MACOSX" in p.parts
            or any(part in generated for part in p.relative_to(ROOT).parts)
            or re.search(r" \(\d+\)(?:\.[^/]*)?$", name)
            or name.endswith(("~", ".bak", ".tmp", ".orig", ".rej"))
            or name in {".DS_Store"}
        ):
            bad.append(rel)
    return {"ok": not bad, "violations": sorted(set(bad))}


def report_consistency() -> dict[str, object]:
    forbidden = {"117/117": [], "117 checks": [], "108/108": [], "87 checks passed": []}
    for name in (
        "ZENITH_RECURSIVE_EXECUTIVE_REPORT.md",
        "ZENITH_RECURSIVE_TRANSFORMATION_MATRIX.md",
        "ZENITH_RECURSIVE_VERIFICATION.md",
        "ZENITH_VERIFICATION_SUMMARY.md",
    ):
        path = ROOT / name
        if not path.is_file():
            continue
        text = path.read_text(encoding="utf-8", errors="replace")
        for token in forbidden:
            if token in text:
                forbidden[token].append(name)
    violations = [{"token": token, "files": files} for token, files in forbidden.items() if files]
    return {
        "ok": not violations,
        "violations": violations,
        "source_of_truth": "scripts/omega-evidence.py executes each gate and records its returned count",
    }


def dependency_status() -> dict[str, object]:
    r = run_repo_script("dependency_freeze", "scripts/check-dependency-freeze.py", ["--json"])
    try:
        value = json.loads(str(r["stdout"]))
    except Exception:
        value = {"approved": False, "reason": "dependency freeze checker output was not JSON"}
    value["checker_exit_code"] = r["exit_code"]
    value["source_tree_unchanged"] = r["source_tree_unchanged"]
    value["source_tree_changes"] = r["source_tree_changes"]
    return value


def tool(name: str) -> dict[str, object]:
    p = shutil.which(name)
    return {"available": bool(p), "path": p}


def path_within_root(path: Path) -> bool:
    try:
        path.expanduser().resolve().relative_to(ROOT.resolve())
        return True
    except ValueError:
        return False


def write_outputs(args: argparse.Namespace, evidence: dict[str, object], blockers: list[dict[str, str]]) -> None:
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(evidence, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    if args.blockers_output:
        args.blockers_output.parent.mkdir(parents=True, exist_ok=True)
        args.blockers_output.write_text(
            json.dumps({"schema": "aethercore.omega-release-blockers.v1", "blockers": blockers}, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--output", type=Path, required=True)
    ap.add_argument("--blockers-output", type=Path)
    ap.add_argument("--strict", action="store_true")
    args = ap.parse_args()

    if path_within_root(args.output) or (args.blockers_output and path_within_root(args.blockers_output)):
        ap.error("Sigma evidence outputs must be outside the source tree; verification is read-only by contract.")

    args.output = args.output.expanduser().resolve()
    if args.blockers_output:
        args.blockers_output = args.blockers_output.expanduser().resolve()

    source_before = tree_state(ROOT)
    manifest_before = verify_manifest(ROOT / "MANIFEST.sha256")
    evidence: dict[str, object] = {
        "schema": "aethercore.omega-evidence.v3",
        "generated_unix_ms": int(time.time() * 1000),
        "platform": platform.platform(),
        "python": sys.version.split()[0],
        "audits": {},
        "toolchains": {n: tool(n) for n in ["rustc", "cargo", "pnpm", "node", "npm", "pwsh", "powershell", "tsc"]},
        "source_manifest_before": manifest_before,
    }

    blockers: list[dict[str, str]] = []
    if not manifest_before.get("ok"):
        blockers.append({
            "id": "OMEGA-RB-003",
            "severity": "delivery_blocker",
            "area": "source_manifest",
            "condition": "MANIFEST.sha256 does not match the delivered source tree before verification",
            "closure": "Regenerate the source manifest after final edits and independently reverify it before executing any repository gate.",
        })
        source_after = tree_state(ROOT)
        evidence["source_manifest"] = verify_manifest(ROOT / "MANIFEST.sha256")
        evidence["source_tree_integrity"] = {
            "ok": source_before == source_after,
            "before_sha256": tree_digest(source_before),
            "after_sha256": tree_digest(source_after),
            "file_entries": len(source_before),
            "changes": state_changes(source_before, source_after),
        }
        evidence["execution_integrity"] = {"ok": True, "runs": [], "mutating_runs": []}
        evidence["release_blockers"] = [b["id"] for b in blockers]
        write_outputs(args, evidence, blockers)
        return 2 if args.strict else 1

    audit_dir = args.output.parent / "audits"
    for name, rel in AUDITS.items():
        detail_out = (audit_dir / f"{name}.json").resolve()
        result = run_repo_script(name, rel, ["--output", str(detail_out)])
        parsed = trailing_json(str(result["stdout"]))
        entry: dict[str, object] = {
            "exit_code": result["exit_code"],
            "duration_ms": result["duration_ms"],
            "path": str(detail_out.relative_to(args.output.parent)),
            "source_tree_unchanged": result["source_tree_unchanged"],
            "source_tree_changes": result["source_tree_changes"],
        }
        if isinstance(parsed, dict):
            entry.update({k: parsed.get(k) for k in ("ok", "checks", "failed") if k in parsed})
        else:
            entry.update({
                "ok": False,
                "parse_error": "no trailing JSON object",
                "stdout_tail": str(result["stdout"])[-2000:],
                "stderr_tail": str(result["stderr"])[-2000:],
            })
        evidence["audits"][name] = entry  # type: ignore[index]

    loc = run_repo_script("localization", "scripts/phase12-localization-audit.py")
    m = re.search(r"Catalog entries:\s*en=(\d+)\s+ar=(\d+)\s+parity=(\d+)", str(loc["stdout"]))
    evidence["localization"] = {
        "exit_code": loc["exit_code"],
        "ok": loc["exit_code"] == 0,
        "en_keys": int(m.group(1)) if m else None,
        "ar_keys": int(m.group(2)) if m else None,
        "parity": int(m.group(3)) if m else None,
        "source_tree_unchanged": loc["source_tree_unchanged"],
        "source_tree_changes": loc["source_tree_changes"],
    }
    evidence["dependency_freeze"] = dependency_status()

    pipe_run = run_repo_script("pipe_teardown", "scripts/check-pipe-teardown-qualification.py", ["--json"])
    try:
        evidence["pipe_teardown"] = json.loads(str(pipe_run["stdout"]))
    except Exception:
        evidence["pipe_teardown"] = {"approved": False, "reason": "pipe qualification checker output was not JSON"}
    evidence["pipe_teardown"]["checker_exit_code"] = pipe_run["exit_code"]  # type: ignore[index]
    evidence["pipe_teardown"]["source_tree_unchanged"] = pipe_run["source_tree_unchanged"]  # type: ignore[index]
    evidence["pipe_teardown"]["source_tree_changes"] = pipe_run["source_tree_changes"]  # type: ignore[index]

    evidence["clean_source_tree"] = cleanliness()
    evidence["report_consistency"] = report_consistency()

    unsafe_out = (args.output.parent / "unsafe-inventory.json").resolve()
    u = run_repo_script("unsafe_inventory", "scripts/omega-unsafe-inventory.py", ["--output", str(unsafe_out)])
    try:
        unsafe_payload = json.loads(unsafe_out.read_text(encoding="utf-8"))
    except Exception:
        unsafe_payload = {}
    evidence["unsafe_inventory"] = {
        "exit_code": u["exit_code"],
        "path": unsafe_out.name,
        "production_boundaries": unsafe_payload.get("production_boundaries"),
        "policy_classified_boundaries": unsafe_payload.get("policy_classified_boundaries"),
        "unclassified_files": unsafe_payload.get("unclassified_files", []),
        "stale_policy_entries": unsafe_payload.get("stale_policy_entries", []),
        "source_tree_unchanged": u["source_tree_unchanged"],
        "source_tree_changes": u["source_tree_changes"],
    }

    ui_out = (args.output.parent / "ui-browser-evidence.json").resolve()
    ui = run_repo_script("ui_browser", "scripts/omega-ui-interaction-harness.py", ["--json", str(ui_out)])
    try:
        ui_payload = json.loads(ui_out.read_text(encoding="utf-8"))
    except Exception:
        ui_payload = {}
    evidence["ui_browser"] = {
        "exit_code": ui["exit_code"],
        "path": ui_out.name,
        "summary": ui_payload.get("summary"),
        "native_equivalence": False,
        "source_tree_unchanged": ui["source_tree_unchanged"],
        "source_tree_changes": ui["source_tree_changes"],
    }

    source_after = tree_state(ROOT)
    manifest_after = verify_manifest(ROOT / "MANIFEST.sha256")
    evidence["source_manifest"] = manifest_after
    evidence["source_tree_integrity"] = {
        "ok": source_before == source_after,
        "before_sha256": tree_digest(source_before),
        "after_sha256": tree_digest(source_after),
        "file_entries": len(source_before),
        "changes": state_changes(source_before, source_after),
    }
    mutating_runs = [r for r in EXECUTION_INTEGRITY if not r.get("source_tree_unchanged")]
    evidence["execution_integrity"] = {
        "ok": not mutating_runs,
        "runs": EXECUTION_INTEGRITY,
        "mutating_runs": mutating_runs,
    }

    if not evidence["dependency_freeze"].get("approved"):  # type: ignore[union-attr]
        blockers.append({"id": "OMEGA-RB-001", "severity": "release_blocker", "area": "dependency_freeze", "condition": "Cargo.lock, pnpm-lock.yaml and reviewed freeze baselines/metadata are not approved", "closure": "Run scripts/freeze-dependencies.ps1 -Refresh on the trusted dependency-freeze workstation, review the graph, commit generated locks/baselines, then rerun -VerifyOnly and all --locked gates."})
    if not evidence["pipe_teardown"].get("approved"):  # type: ignore[union-attr]
        blockers.append({"id": "OMEGA-RB-002", "severity": "release_blocker", "area": "named_pipe_teardown", "condition": "Synchronous named-pipe cancellation does not yet have accepted Windows slow-peer/race evidence", "closure": "Produce out/omega-pipe-teardown.json satisfying scripts/check-pipe-teardown-qualification.py; if the synchronous design misses any bound, migrate to overlapped/event-driven I/O and rerun the campaign."})
    if not manifest_after.get("ok"):
        blockers.append({"id": "OMEGA-RB-003", "severity": "delivery_blocker", "area": "source_manifest", "condition": "MANIFEST.sha256 does not match the current source tree", "closure": "Regenerate the source manifest after final edits and independently reverify it."})
    if not evidence["report_consistency"].get("ok"):  # type: ignore[union-attr]
        blockers.append({"id": "OMEGA-RB-004", "severity": "delivery_blocker", "area": "verification_truth", "condition": "Current reports contain known stale manually duplicated executable totals", "closure": "Remove mutable totals from narrative reports and regenerate status exclusively from executable evidence."})
    if evidence["unsafe_inventory"].get("exit_code") != 0:  # type: ignore[union-attr]
        blockers.append({"id": "OMEGA-RB-005", "severity": "source_verification_blocker", "area": "unsafe_governance", "condition": "Production unsafe boundary inventory contains unclassified or stale policy entries", "closure": "Classify every production unsafe file in release/unsafe-boundary-policy.json and rerun the inventory."})
    if evidence["ui_browser"].get("exit_code") != 0:  # type: ignore[union-attr]
        blockers.append({"id": "OMEGA-RB-006", "severity": "source_verification_blocker", "area": "ui_browser", "condition": "Browser interaction source harness did not pass", "closure": "Repair the interaction/accessibility regression or restore the required browser/tooling and rerun scripts/omega-ui-interaction-harness.py."})
    if not evidence["source_tree_integrity"].get("ok"):  # type: ignore[union-attr]
        blockers.append({"id": "SIGMA-RB-001", "severity": "delivery_blocker", "area": "verification_hermeticity", "condition": "Verification changed delivered source bytes", "closure": "Route every generated artifact outside the source tree and rerun from a pristine delivery."})
    if mutating_runs:
        blockers.append({"id": "SIGMA-RB-002", "severity": "source_verification_blocker", "area": "audit_write_containment", "condition": "One or more repository audits attempted to mutate their isolated source clone", "closure": "Make the listed audit read-only by default and require an explicit external --output for generated evidence."})

    for n in ("cargo", "rustc", "pnpm", "pwsh"):
        if not evidence["toolchains"][n]["available"]:  # type: ignore[index]
            blockers.append({"id": f"OMEGA-ENV-{n.upper()}", "severity": "environment_limit", "area": "toolchain", "condition": f"{n} is unavailable in this execution environment", "closure": "Execute the corresponding locked/native gate on the qualified Windows verification host."})
    if os.name != "nt":
        blockers.append({"id": "OMEGA-ENV-WINDOWS", "severity": "environment_limit", "area": "native_qualification", "condition": "Current host is not Windows", "closure": "Run service/token/pipe/SCM/WiX/AuthentiCode/hardware/update/native stress qualification on the qualified Windows host and retain evidence."})

    evidence["release_blockers"] = [b["id"] for b in blockers]
    audit_entries = evidence["audits"].values()  # type: ignore[union-attr]
    all_audits = (
        all(v.get("ok") is True and v.get("exit_code") == 0 and v.get("source_tree_unchanged") is True for v in audit_entries)
        and evidence["localization"]["ok"]  # type: ignore[index]
        and evidence["localization"]["source_tree_unchanged"]  # type: ignore[index]
        and evidence["clean_source_tree"]["ok"]  # type: ignore[index]
        and evidence["report_consistency"]["ok"]  # type: ignore[index]
        and evidence["unsafe_inventory"]["exit_code"] == 0  # type: ignore[index]
        and evidence["unsafe_inventory"]["source_tree_unchanged"]  # type: ignore[index]
        and evidence["ui_browser"]["exit_code"] == 0  # type: ignore[index]
        and evidence["ui_browser"]["source_tree_unchanged"]  # type: ignore[index]
        and evidence["source_manifest_before"].get("ok") is True  # type: ignore[union-attr]
        and evidence["source_manifest"].get("ok") is True  # type: ignore[union-attr]
        and evidence["source_tree_integrity"]["ok"] is True  # type: ignore[index]
        and evidence["execution_integrity"]["ok"] is True  # type: ignore[index]
    )
    evidence["source_verification_pass"] = bool(all_audits)
    write_outputs(args, evidence, blockers)
    if args.strict and (not all_audits or blockers):
        return 2
    return 0 if all_audits else 1


if __name__ == "__main__":
    raise SystemExit(main())
