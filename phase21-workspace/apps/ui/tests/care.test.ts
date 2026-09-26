// P75 DBT-P75-045. Run: node --experimental-strip-types --test tests/*.test.ts
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { afterApproval, approvedStepCount } from '../src/features/care/approval.ts';
import type { CareRunStatus } from '../src/lib/contracts.ts';

function status(digest: string, levels: number[] = [0]): CareRunStatus {
  return {
    runId: '', state: 'Idle', stage: 'Preview', sessionConsentGranted: false, planDigestSha256: digest,
    steps: levels.map((safetyLevel, stepIndex) => ({
      stepIndex, domainPlanId: `p${stepIndex}`, domainKind: 'Cleanup', safetyLevel, state: 'Pending',
      outcome: 'Pending', domainVerificationState: '', failureMessageKey: '',
    })),
    updatedUnixMs: 0, summaryKey: '',
  };
}

test('the approved plan is the one shown: start it', () => {
  assert.equal(afterApproval(status('aa'), status('aa')).start, true);
});

test('the plan changed between showing and approving: do not start, show it again', () => {
  const next = afterApproval(status('aa'), status('bb'));
  assert.equal(next.start, false);
  assert.equal(next.status.planDigestSha256, 'bb');
  assert.equal(next.status.state, 'AwaitingConsent');
  assert.equal(next.status.summaryKey, 'care.summary.planChanged');
});

test('nothing was shown: do not start', () => {
  assert.equal(afterApproval(null, status('aa')).start, false);
  assert.equal(afterApproval(status(''), status('')).start, false);
});

test('one approval covers the automatic steps only', () => {
  assert.equal(approvedStepCount(status('aa', [0, 2, 0])), 2);
  assert.equal(approvedStepCount(null), 0);
});
