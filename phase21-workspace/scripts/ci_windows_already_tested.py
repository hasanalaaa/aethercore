#!/usr/bin/env python3
"""Decides whether a push to main can skip the CI `windows` job (lane 1, step 0).

A fast-forward push puts a commit on main that already ran the whole CI as a pull request of this
repository, on the same SHA. Running the ~40 minute windows job again on main tests nothing new.
It skips only when a `pull_request` run of ci.yml for exactly this SHA, from this repository's own
branch, has a `windows` job that succeeded. Every other outcome - nothing found, a failure, a skip,
a fork, another SHA, a lookup that errors - is `skip=false` and the job runs. Prints and appends
`skip=true|false` to $GITHUB_OUTPUT; never fails the job.
"""
import json
import os
import subprocess
import sys


def gh(path: str) -> dict:
    proc = subprocess.run(["gh", "api", path], capture_output=True, text=True, timeout=60)
    if proc.returncode != 0:
        raise RuntimeError(f"gh api {path} exited {proc.returncode}")
    return json.loads(proc.stdout)


def already_tested(repo: str, sha: str) -> bool:
    runs = gh(f"repos/{repo}/actions/workflows/ci.yml/runs?head_sha={sha}&event=pull_request&status=success&per_page=100")["workflow_runs"]
    for run in runs:
        if run.get("head_sha") != sha or (run.get("head_repository") or {}).get("full_name") != repo:
            continue
        jobs = gh(f"repos/{repo}/actions/runs/{run['id']}/jobs?per_page=100")["jobs"]
        if any(job.get("name") == "windows" and job.get("conclusion") == "success" for job in jobs):
            return True
    return False


def main() -> int:
    repo, sha = os.environ["GITHUB_REPOSITORY"], os.environ["GITHUB_SHA"]
    try:
        skip = already_tested(repo, sha)
    except Exception as error:  # fail closed: any doubt means the windows job runs
        print(f"lookup failed, the windows job will run: {error}", file=sys.stderr)
        skip = False
    line = f"skip={'true' if skip else 'false'}"
    print(line)
    if os.environ.get("GITHUB_OUTPUT"):
        with open(os.environ["GITHUB_OUTPUT"], "a", encoding="utf-8") as out:
            out.write(line + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
