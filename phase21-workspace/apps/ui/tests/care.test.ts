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

test('the service refusing a changed plan is shown as a new plan, other failures are not', async () => {
  const { isPlanChanged } = await import('../src/features/care/approval.ts');
  assert.equal(isPlanChanged('care.error.planChanged'), true);
  assert.equal(isPlanChanged(new Error('care.error.startFailed')), false);
});

// P76 DBT-P76-008 (the owner's Windows install): with the one default cleanup category empty,
// care found nothing to run (chosen=0) and the panel said "Run a domain scan first" — after a
// scan. It now says which of three things is true.
test('an empty care plan says why: no scan, nothing eligible (and what needs opt-in), or no plan yet', async () => {
  const { careEmptyReason } = await import('../src/features/care/approval.ts');
  const candidate = (title: string, byDefault: boolean) => ({
    candidateId: title, provider: 'x', title, description: '', reclaimableBytes: 1, fileCount: 1,
    selectedByDefault: byDefault, requiresExplicitConfirmation: !byDefault, truncated: false, specialKind: 'Files',
  });
  const snapshot = (state: string, candidates: ReturnType<typeof candidate>[]) => ({
    scanId: 's', state, inventoryEpoch: 1, startedUnixMs: 1, completedUnixMs: 2, errorMessage: '',
    totalReclaimableBytes: 0, totalFileCount: 0, candidates, warnings: [],
  });
  assert.deepEqual(careEmptyReason(snapshot('Idle', [])), { kind: 'noScan' });
  assert.deepEqual(
    careEmptyReason(snapshot('Ready', [candidate('Alice temporary files', false), candidate('Windows minidumps', false)])),
    { kind: 'nothingEligible', optIn: ['Alice temporary files', 'Windows minidumps'] },
  );
  assert.deepEqual(careEmptyReason(snapshot('Ready', [])), { kind: 'nothingEligible', optIn: [] });
  assert.deepEqual(careEmptyReason(snapshot('Ready', [candidate('Windows temporary files', true)])), { kind: 'noPlan' });
});
