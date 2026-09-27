#!/usr/bin/env python3
"""Prove the source seal verifier can fail.

DBT-P59-002: `p30-fulltree-spot-hash-ok` regenerated its ledger from the tree it
then compared against, so its expected value was minted from its measured value.
It reported green for every phase it ever ran in and that green meant nothing.

These cases exist to make that impossible to repeat. Each one puts a specific
defect into a sealed tree and asserts the verifier exits non-zero AND names the
file and the reason -- exit code alone is too weak an assertion, because a
verifier that crashes for an unrelated reason also exits non-zero.

Run: python3 scripts/test_source_seal.py
"""
from __future__ import annotations

import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

sys.dont_write_bytecode = True
SCRIPTS = Path(__file__).resolve().parent
ROOT = SCRIPTS.parent
SEAL = SCRIPTS / "source_seal.py"
REGEN = SCRIPTS / "regenerate-source-manifest.py"

RESULTS: list[tuple[str, bool, str]] = []


def record(name: str, ok: bool, detail: str = "") -> None:
    RESULTS.append((name, ok, detail))
    print(f"{'PASS' if ok else 'FAIL'}  {name}" + (f"  -- {detail}" if detail else ""))


def git(cwd: Path, *args: str) -> None:
    subprocess.run(["git", *args], cwd=cwd, check=True, capture_output=True)


def run_seal(root: Path) -> tuple[int, dict]:
    p = subprocess.run(
        [sys.executable, str(SEAL), "--root", str(root), "--json"],
        capture_output=True,
        text=True,
    )
    try:
        return p.returncode, json.loads(p.stdout)
    except Exception:
        return p.returncode, {"stdout": p.stdout, "stderr": p.stderr}


def fixture(tmp: Path) -> Path:
    """A miniature sealed tree: a git repo with a manifest over its tracked files."""
    root = tmp / "tree"
    (root / "src").mkdir(parents=True)
    (root / "src" / "a.txt").write_text("alpha\n", encoding="utf-8")
    (root / "src" / "b.txt").write_text("beta\n", encoding="utf-8")
    (root / "notes.md").write_text("# notes\n", encoding="utf-8")
    (root / ".gitignore").write_text("build/\n", encoding="utf-8")
    (root / "build").mkdir()
    (root / "build" / "out.bin").write_bytes(b"\x00\x01\x02")
    git(root, "init", "-q")
    git(root, "config", "user.email", "test@example.invalid")
    git(root, "config", "user.name", "seal test")
    git(root, "add", "-A")
    git(root, "commit", "-qm", "fixture")
    subprocess.run(
        [sys.executable, str(REGEN), "--root", str(root)], check=True, capture_output=True
    )
    return root


def failed_paths(report: dict, reason: str) -> list[str]:
    return [f["path"] for f in report.get("failed", []) if f.get("reason") == reason]


def case_pristine(tmp: Path) -> None:
    root = fixture(tmp)
    code, report = run_seal(root)
    record(
        "pristine sealed tree verifies",
        code == 0 and report.get("ok") is True and report.get("verified") == 4,
        f"exit={code} verified={report.get('verified')} failed={report.get('failed')}",
    )


def case_one_byte(tmp: Path) -> None:
    root = fixture(tmp)
    target = root / "src" / "a.txt"
    data = bytearray(target.read_bytes())
    data[0] ^= 0x01  # 'a' -> '`'; one byte, same length
    target.write_bytes(bytes(data))
    code, report = run_seal(root)
    record(
        "one flipped byte is reported as a hash mismatch",
        code != 0 and "src/a.txt" in failed_paths(report, "hash"),
        f"exit={code} failed={report.get('failed')}",
    )


def case_missing(tmp: Path) -> None:
    root = fixture(tmp)
    (root / "src" / "b.txt").unlink()
    code, report = run_seal(root)
    record(
        "a listed file that is gone is reported as missing",
        code != 0 and "src/b.txt" in failed_paths(report, "missing"),
        f"exit={code} failed={report.get('failed')}",
    )


def case_unlisted(tmp: Path) -> None:
    root = fixture(tmp)
    (root / "src" / "c.txt").write_text("gamma\n", encoding="utf-8")
    git(root, "add", "src/c.txt")
    code, report = run_seal(root)
    record(
        "a tracked file the manifest never listed is reported as unlisted",
        code != 0 and "src/c.txt" in failed_paths(report, "unlisted"),
        f"exit={code} failed={report.get('failed')}",
    )


def case_ignored_build_output_is_out_of_scope(tmp: Path) -> None:
    root = fixture(tmp)
    (root / "build" / "out.bin").write_bytes(b"\xff\xff\xff\xff")
    (root / "build" / "extra.bin").write_bytes(b"\x09")
    code, report = run_seal(root)
    record(
        "rebuilding ignored output does not break the seal",
        code == 0 and report.get("ok") is True,
        f"exit={code} failed={report.get('failed')}",
    )


def case_truncated_manifest(tmp: Path) -> None:
    """The defect that made this whole class possible: a seal that covers a
    fraction of the tree and reports green over the fraction it covers."""
    root = fixture(tmp)
    manifest = root / "MANIFEST.sha256"
    lines = manifest.read_text(encoding="utf-8").splitlines(keepends=True)
    manifest.write_text("".join(lines[:1]), encoding="utf-8", newline="")
    code, report = run_seal(root)
    record(
        "a manifest covering part of the tree cannot report ok",
        code != 0 and len(failed_paths(report, "unlisted")) == 3,
        f"exit={code} failed={report.get('failed')}",
    )


def case_real_tree(tmp: Path) -> None:
    code, report = run_seal(ROOT)
    record(
        "the delivered workspace verifies against its committed manifest",
        code == 0 and report.get("ok") is True,
        f"exit={code} listed={report.get('listed')} verified={report.get('verified')} "
        f"failed={len(report.get('failed', []))}",
    )


def case_real_tree_one_byte(tmp: Path) -> None:
    """The fixture proves the instrument; this proves it on the delivered tree.

    A tracked file is corrupted in place and restored from git afterwards, so the
    working tree is exactly as it was found. P59 learned this the hard way: a test
    that dirties a tracked file is a test that lies about the tree it measured.
    """
    target = ROOT / "README.md"
    original = target.read_bytes()
    try:
        data = bytearray(original)
        data[0] ^= 0x01
        target.write_bytes(bytes(data))
        code, report = run_seal(ROOT)
        ok = code != 0 and "README.md" in failed_paths(report, "hash")
        detail = f"exit={code} failed={report.get('failed', [])[:3]}"
    finally:
        target.write_bytes(original)
    after = subprocess.run(
        ["git", "status", "--porcelain", "--", "README.md"],
        cwd=ROOT,
        capture_output=True,
        text=True,
    ).stdout.strip()
    record("one flipped byte in the delivered tree fails the seal", ok, detail)
    record("the corruption case leaves the tree clean", after == "", f"git status: {after!r}")


def repo_fixture(tmp: Path, with_manifest: bool = True) -> Path:
    """A repository whose sealed workspace `ws/` has a `.github/` beside it, as this one does."""
    repo = tmp / "repo"
    (repo / "ws" / "src").mkdir(parents=True)
    (repo / "ws" / "src" / "a.txt").write_text("alpha\n", encoding="utf-8")
    (repo / ".github" / "workflows").mkdir(parents=True)
    (repo / ".github" / "workflows" / "ci.yml").write_text("on: push\n", encoding="utf-8")
    git(repo, "init", "-q")
    git(repo, "config", "user.email", "test@example.invalid")
    git(repo, "config", "user.name", "seal test")
    git(repo, "add", "-A")
    git(repo, "commit", "-qm", "fixture")
    if with_manifest:
        subprocess.run(
            [sys.executable, str(REGEN), "--root", str(repo / "ws")], check=True, capture_output=True
        )
    return repo / "ws"


def case_repository_root_workflow_change(tmp: Path) -> None:
    """DBT-P60-002: a workflow at the repository root is sealed like the workspace."""
    ws = repo_fixture(tmp)
    code, report = run_seal(ws)
    clean = code == 0 and report.get("repository_root", {}).get("verified") == 1
    ci = ws.parent / ".github" / "workflows" / "ci.yml"
    ci.write_text("on: pull_request\n", encoding="utf-8")
    code2, report2 = run_seal(ws)
    root_failed = [f["path"] for f in report2.get("repository_root", {}).get("failed", [])]
    record(
        "a changed workflow at the repository root fails the seal",
        clean and code2 != 0 and report2.get("ok") is False and root_failed == ["workflows/ci.yml"],
        f"clean exit={code} then exit={code2} root failed={root_failed}",
    )


def case_repository_root_without_manifest(tmp: Path) -> None:
    ws = repo_fixture(tmp, with_manifest=False)
    subprocess.run(
        [sys.executable, str(REGEN), "--root", str(ws)], check=True, capture_output=True
    )
    (ws.parent / ".github" / "MANIFEST.sha256").unlink()
    code, report = run_seal(ws)
    record(
        "a .github with no manifest is unevaluated, never passed",
        code == 2 and report.get("ok") is False,
        f"exit={code} report={report}",
    )


CASES = [
    case_pristine,
    case_one_byte,
    case_missing,
    case_unlisted,
    case_ignored_build_output_is_out_of_scope,
    case_truncated_manifest,
    case_real_tree,
    case_real_tree_one_byte,
    case_repository_root_workflow_change,
    case_repository_root_without_manifest,
]


def main() -> int:
    if shutil.which("git") is None:
        print("git is required to run these cases", file=sys.stderr)
        return 2
    # The first version of this file called the generator without a --root it did
    # not yet accept, so the generator ignored argv and rewrote the delivered
    # tree's own manifest. Hold the bytes and prove they came back untouched.
    sealed = (ROOT / "MANIFEST.sha256").read_bytes()
    root_sealed = (ROOT.parent / ".github" / "MANIFEST.sha256").read_bytes()
    for case in CASES:
        with tempfile.TemporaryDirectory() as td:
            try:
                case(Path(td))
            except Exception as exc:  # a crashing case is a failing case, named
                record(case.__name__, False, f"{type(exc).__name__}: {exc}")
    record(
        "the run left the delivered manifests untouched",
        (ROOT / "MANIFEST.sha256").read_bytes() == sealed
        and (ROOT.parent / ".github" / "MANIFEST.sha256").read_bytes() == root_sealed,
        "MANIFEST.sha256 or .github/MANIFEST.sha256 was rewritten by a test case",
    )
    passed = sum(1 for _, ok, _ in RESULTS if ok)
    print(f"\n{passed} of {len(RESULTS)} pass")
    return 0 if passed == len(RESULTS) else 1


if __name__ == "__main__":
    raise SystemExit(main())
