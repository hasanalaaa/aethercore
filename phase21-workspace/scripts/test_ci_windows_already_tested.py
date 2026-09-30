#!/usr/bin/env python3
"""Lane 1 step 0: the `tested` job may let a push to main skip the windows job only when the same SHA
already passed its own windows job in a pull request of this repository. Every other outcome -
nothing found, a failed or skipped windows job, a fork, a different SHA, a lookup that errors - must
leave `skip=false`, so the windows job runs. `gh` is replaced by a stub that answers from the scenario."""
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

SCRIPT = Path(__file__).with_name("ci_windows_already_tested.py")
REPO = "owner/repo"
SHA = "a" * 40

STUB = """#!/usr/bin/env python3
import json, os, sys
path = sys.argv[2]
scenario = json.loads(os.environ["FAKE_GH"])
if "/jobs" in path:
    run_id = path.split("/runs/")[1].split("/")[0]
    if scenario.get("jobs_fail"):
        sys.exit(1)
    print(json.dumps({"jobs": scenario["jobs"].get(run_id, [])}))
else:
    if scenario.get("runs_fail"):
        sys.exit(1)
    print(json.dumps({"workflow_runs": scenario["runs"]}))
"""


def run(scenario: dict) -> tuple[str, int]:
    with tempfile.TemporaryDirectory() as directory:
        stub = Path(directory) / "gh"
        stub.write_text(STUB)
        stub.chmod(0o755)
        output = Path(directory) / "github_output"
        env = {
            **os.environ,
            "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
            "GITHUB_REPOSITORY": REPO,
            "GITHUB_SHA": SHA,
            "GITHUB_OUTPUT": str(output),
            "FAKE_GH": json.dumps(scenario),
        }
        proc = subprocess.run([sys.executable, str(SCRIPT)], env=env, capture_output=True, text=True)
        written = output.read_text() if output.exists() else ""
        return written.strip(), proc.returncode


def pr_run(run_id: int, sha: str = SHA, repo: str = REPO) -> dict:
    return {"id": run_id, "head_sha": sha, "head_repository": {"full_name": repo}}


def windows(conclusion: str) -> list:
    return [{"name": "deny-check", "conclusion": "success"}, {"name": "windows", "conclusion": conclusion}]


CASES = [
    ("the same SHA passed windows in a PR of this repository", {"runs": [pr_run(1)], "jobs": {"1": windows("success")}}, "skip=true"),
    ("a later run qualifies after an earlier one did not", {"runs": [pr_run(1), pr_run(2)], "jobs": {"1": windows("failure"), "2": windows("success")}}, "skip=true"),
    ("no run for the SHA", {"runs": []}, "skip=false"),
    ("windows failed in the run", {"runs": [pr_run(1)], "jobs": {"1": windows("failure")}}, "skip=false"),
    ("windows was skipped in the run", {"runs": [pr_run(1)], "jobs": {"1": windows("skipped")}}, "skip=false"),
    ("the run has no windows job", {"runs": [pr_run(1)], "jobs": {"1": [{"name": "deny-check", "conclusion": "success"}]}}, "skip=false"),
    ("the run came from a fork", {"runs": [pr_run(1, repo="someone/fork")], "jobs": {"1": windows("success")}}, "skip=false"),
    ("the run is for another SHA", {"runs": [pr_run(1, sha="b" * 40)], "jobs": {"1": windows("success")}}, "skip=false"),
    ("the run lookup fails", {"runs_fail": True}, "skip=false"),
    ("the jobs lookup fails", {"runs": [pr_run(1)], "jobs_fail": True}, "skip=false"),
]


def main() -> int:
    failed = 0
    for name, scenario, expected in CASES:
        got, code = run(scenario)
        ok = got == expected and code == 0
        failed += not ok
        print(f"{'PASS' if ok else 'FAIL'}  {name}  -- got {got!r} exit={code}")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
