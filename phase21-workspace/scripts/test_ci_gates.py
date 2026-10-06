#!/usr/bin/env python3
"""Check P87 fast feedback and execute the aggregate's fail-closed predicate."""
import itertools
import json
import os
from pathlib import Path
import re
import subprocess
import unittest

ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / '.github/workflows/ci.yml'


def jobs():
    text = WORKFLOW.read_text(encoding='utf-8').split('\njobs:\n', 1)[1]
    return dict(re.findall(r'^  ([\w-]+):\n(.*?)(?=^  [\w-]+:|\Z)', text, re.M | re.S))


class CiGates(unittest.TestCase):
    def test_locale_and_units_do_not_wait_for_windows(self):
        blocks = jobs()
        self.assertTrue('fast-ui' in blocks, 'UI checks still wait behind native Windows tests')
        fast = blocks['fast-ui']
        self.assertNotIn('needs:', fast)
        for command in ('test-phase12-localization.py', 'pnpm --dir apps/ui check',
                        'node --experimental-strip-types', 'install --frozen-lockfile'):
            self.assertIn(command, fast)
        self.assertNotIn('cargo ', fast)
        self.assertNotIn('name: UI unit tests', blocks['windows'])
        self.assertIn('scripts/source_seal.py', blocks['windows'])
        self.assertIn('freeze-dependencies.ps1 -VerifyOnly', blocks['windows'])

    def test_workspace_tests_run_exactly_through_the_enterprise_gate(self):
        # The `windows` job has no standalone `cargo test` step: verify-enterprise.ps1 runs the same
        # workspace test, so a second copy only repeated ~7.5 minutes. The gate must stay wired.
        windows = jobs()['windows']
        self.assertIn('./scripts/verify-enterprise.ps1', windows)
        self.assertIsNone(re.search(r'^\s+run: cargo test', windows, re.M), 'a standalone cargo test step came back')
        enterprise = (ROOT / 'phase21-workspace/scripts/verify-enterprise.ps1').read_text(encoding='utf-8')
        self.assertIn('& cargo test --workspace --locked', enterprise)

    def test_the_packaging_candidate_is_its_own_wired_job(self):
        blocks = jobs()
        self.assertIn('windows-candidate', blocks, 'the packaging candidate job was removed')
        candidate, windows = blocks['windows-candidate'], blocks['windows']
        self.assertIn('./scripts/build-release.ps1 -SkipOnlineSupplyChain', candidate)
        self.assertIsNone(re.search(r'^\s+run: .*build-release\.ps1', windows, re.M), 'the candidate build is back in windows')
        self.assertIn('scripts/source_seal.py', candidate)
        for field in ('needs: tested', "if: ${{ !cancelled() && needs.tested.outputs.skip != 'true' }}"):
            self.assertIn(field, candidate)
            self.assertIn(field, windows)
        runner = lambda block: re.search(r'^    runs-on: (.+)$', block, re.M)[1]
        self.assertEqual(runner(candidate), runner(windows), 'the candidate must run where windows runs')
        # The 2.17 GB upload is kept for a push to main and a dispatch, never a pull request; the build
        # step itself must not be conditional, or the packaging gate would silently stop on pull requests.
        upload = re.search(r'- uses: actions/upload-artifact.*?(?=^      - |\Z)', candidate, re.M | re.S)[0]
        self.assertIn("if: github.event_name != 'pull_request'", upload)
        build = re.search(r'- name: Build unsigned production packaging candidate\n(.*?)(?=^      - )', candidate, re.M | re.S)[1]
        self.assertNotIn('if:', build)

    def test_aggregate_rejects_every_non_success_and_missing_gate(self):
        blocks = jobs()
        self.assertTrue('required' in blocks, 'No aggregate guards skipped/cancelled gates')
        required = blocks['required']
        self.assertIn('name: CI required', required)
        self.assertIn('if: ${{ always() }}', required)
        self.assertIn('NEEDS: ${{ toJSON(needs) }}', required)
        dependencies = re.search(r'needs: \[(.*)\]', required)[1].split(', ')
        self.assertEqual(set(dependencies), set(blocks) - {'required'})
        command = re.search(r'^        run: (.+)$', required, re.M)[1]
        # The count is part of the predicate: relaxing it (`>= 7`) would let a job go missing.
        self.assertIn(f'length == {len(dependencies)} and', command)
        results = {gate: {'result': 'success'} for gate in dependencies}
        results['tested'] = {'result': 'success', 'outputs': {'skip': 'false'}}
        cases = [(results, 0), ({}, 1)]
        for gate, state in itertools.product(dependencies, ('failure', 'cancelled', 'skipped')):
            # `tested` runs only for a push: a pull request leaves it skipped, which is not a failure.
            cases.append(({**results, gate: {'result': state}}, 0 if (gate, state) == ('tested', 'skipped') else 1))
        # Lane 1 step 0: windows may be skipped only when `tested` proved, for this exact SHA, that
        # its own pull request already passed the windows job. Every other skip is a failure.
        proven = {**results, 'windows': {'result': 'skipped'}, 'windows-candidate': {'result': 'skipped'},
                  'tested': {'result': 'success', 'outputs': {'skip': 'true'}}}
        cases += [
            (proven, 0),
            ({**proven, 'windows-candidate': {'result': 'success'}}, 0),
            ({**proven, 'windows': {'result': 'success'}}, 0),
            ({**results, 'windows-candidate': {'result': 'skipped'}}, 1),
            ({key: value for key, value in results.items() if key != 'windows-candidate'}, 1),
            ({**proven, 'windows-candidate': {'result': 'failure'}}, 1),
            ({**proven, 'tested': {'result': 'success', 'outputs': {'skip': 'false'}}}, 1),
            ({**proven, 'tested': {'result': 'success'}}, 1),
            ({**proven, 'tested': {'result': 'skipped'}}, 1),
            ({**proven, 'tested': {'result': 'failure', 'outputs': {'skip': 'true'}}}, 1),
            ({**proven, 'windows': {'result': 'failure'}}, 1),
        ]
        for needs, expected in cases:
            run = subprocess.run(['bash', '-c', command], env={**os.environ, 'NEEDS': json.dumps(needs)},
                                 capture_output=True)
            self.assertEqual(run.returncode, expected, (needs, run.stderr))


if __name__ == '__main__':
    unittest.main()
