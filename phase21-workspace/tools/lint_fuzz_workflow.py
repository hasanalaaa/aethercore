#!/usr/bin/env python3
"""Static lint for .github/workflows/fuzz.yml (W3)."""
s = open('.github/workflows/fuzz.yml').read()
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
