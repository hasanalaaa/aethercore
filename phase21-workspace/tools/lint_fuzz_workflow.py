#!/usr/bin/env python3
"""Static lint for .github/workflows/fuzz.yml (W3).

P58 / DBT-P55-001: the workflow moved to `.github/workflows` at the REPOSITORY
root, the only place GitHub Actions reads it from. This tool is invoked with the
workspace as the working directory, so the path is resolved from this file's
location rather than from the cwd - a cwd-relative `open` would have raised
FileNotFoundError from the workspace and read the wrong tree from anywhere else.
"""
from pathlib import Path

WORKFLOW = Path(__file__).resolve().parents[2] / '.github' / 'workflows' / 'fuzz.yml'
s = WORKFLOW.read_text(encoding='utf-8')
targets = ['export_envelope_parse', 'pg_conf_parse', 'mysql_conf_parse',
           'pg_slow_log_parse', 'mysql_slow_log_parse']
checks = {
    'name': 'name: fuzz' in s,
    'on_pr': 'pull_request:' in s,
    'all_5_targets': all(t in s for t in targets),
    'runs_200': '-runs=200' in s,
    'timeout': 'timeout-minutes' in s,
    'artifacts_on_crash': 'upload-artifact' in s and 'if: always()' in s,
    'fail_fast_false': 'fail-fast: false' in s,
}
print(checks)
assert all(checks.values()), f"workflow lint failed: {[k for k,v in checks.items() if not v]}"
print("FUZZ_WORKFLOW_LINT_OK")
