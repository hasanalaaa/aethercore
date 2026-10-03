// P75 DBT-P75-045. Run: node --experimental-strip-types --test tests/*.test.ts
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { hasMessageKey, td } from '../src/lib/i18n/index.ts';
import { afterApproval, approvedStepCount } from '../src/features/care/approval.ts';
import type { CareRunStatus } from '../src/lib/contracts.ts';

function status(digest: string, levels: number[] = [0]): CareRunStatus {
  return {
    runId: '', state: 'Idle', stage: 'Preview', sessionConsentGranted: false, planDigestSha256: digest,
    steps: levels.map((safetyLevel, stepIndex) => ({
      stepIndex, domainPlanId: `p${stepIndex}`, domainKind: 'Cleanup', safetyLevel, state: 'Pending',
      outcome: 'Pending', domainVerificationState: '', failureMessageKey: '', hasActualDeletedBytes: false, actualDeletedBytes: '',
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

// P79-03: a plan that could not be read is neither an empty plan nor a reason to run a scan. When the
// service refuses to build the plan (or the read fails) the panel says so instead of "run a scan first".
test('an unread care plan is unavailable, not "no scan" or "nothing due"', async () => {
  const { careEmptyReason } = await import('../src/features/care/approval.ts');
  const ready = { scanId: 's', state: 'Ready', inventoryEpoch: 1, startedUnixMs: 1, completedUnixMs: 2, errorMessage: '', totalReclaimableBytes: 0, totalFileCount: 0, candidates: [], warnings: [] };
  const idle = { ...ready, state: 'Idle' };
  assert.deepEqual(careEmptyReason(idle, 'failed'), { kind: 'unavailable' }, 'a failed read is not "run a scan first"');
  assert.deepEqual(careEmptyReason(ready, 'failed'), { kind: 'unavailable' }, 'nor "nothing eligible"');
  assert.deepEqual(careEmptyReason(ready, 'loaded'), { kind: 'nothingEligible', optIn: [] }, 'a plan that was read keeps its reasons');
  const panel = readFileSync(new URL('../src/features/care/CarePanel.svelte', import.meta.url), 'utf8');
  assert.ok(panel.includes("emptyReason.kind === 'unavailable'"), 'the panel has no view for an unavailable plan');
  for (const key of ['care.unavailable', 'care.error.sourceLimit']) {
    assert.ok(hasMessageKey(key), key);
    assert.match(td(key as never, 'ar'), /[؀-ۿ]/, key);
  }
});

// P79-04B: "scan -> review -> approve -> result". The panel asks the service to prepare the preview
// (P79-04A), waits for the local scan without polling, and shows the owner what would be deleted, how
// much, and what was left out and why, before the existing digest-bound approval.
test('what to do after a prepare follows the domain reason, and only a ready preview with auto work opens the review', async () => {
  const { afterPrepare, eligibleCandidates } = await import('../src/features/care/approval.ts');
  const status = (auto: number) => ({
    runId: '', state: 'Idle', stage: 'Preview', sessionConsentGranted: false, planDigestSha256: 'd', updatedUnixMs: 1, summaryKey: '',
    steps: Array.from({ length: auto }, (_, i) => ({ stepIndex: i, domainPlanId: `p${i}`, domainKind: 'Cleanup', safetyLevel: 0, state: 'Pending', outcome: '', domainVerificationState: '', failureMessageKey: '' })),
  });
  const domain = (reason: string) => ({ domain: 'cleanup', reason, scannedUnixMs: 5, eligibleCandidates: 1, reviewRequiredCandidates: 0 });
  assert.equal(afterPrepare({ status: status(1), domains: [domain('ready')] }), 'review');
  assert.equal(afterPrepare({ status: status(0), domains: [domain('ready')] }), 'explain', 'ready without a prepared auto step is not approvable');
  assert.equal(afterPrepare({ status: status(0), domains: [domain('scanning')] }), 'wait');
  for (const reason of ['none', 'reviewRequired', 'unavailable']) {
    assert.equal(afterPrepare({ status: status(1), domains: [domain(reason)] }), 'explain', reason);
  }
  assert.equal(afterPrepare({ status: status(1), domains: [] }), 'explain', 'no domain answer is not a review');
  const c = (id: string, byDefault: boolean, confirm: boolean) => ({ candidateId: id, provider: 'x', title: id, description: '', reclaimableBytes: 10, fileCount: 1, selectedByDefault: byDefault, requiresExplicitConfirmation: confirm, truncated: false, specialKind: 'Files' });
  const scan = { scanId: 's', state: 'Ready', inventoryEpoch: 1, startedUnixMs: 1, completedUnixMs: 2, errorMessage: '', totalReclaimableBytes: 0, totalFileCount: 0, warnings: [], candidates: [c('a', true, false), c('b', false, true), c('d', true, true)] };
  assert.deepEqual(eligibleCandidates(scan).map((x) => x.candidateId), ['a'], 'the review lists only what the service prepares');
});

test('every reason the service gives has a label, and the client is wired to the new verb', () => {
  const rust = readFileSync(new URL('../../../services/maintenance-service/src/care.rs', import.meta.url), 'utf8');
  for (const reason of ['ready', 'scanning', 'none', 'reviewRequired', 'unavailable']) {
    assert.ok(rust.includes(`"${reason}"`), `the service no longer says ${reason}`);
    const key = `care.reason.${reason}`;
    assert.ok(hasMessageKey(key), key);
    assert.match(td(key as never, 'ar'), /[؀-ۿ]/, key);
  }
  const controller = readFileSync(new URL('../src/features/care/controller.ts', import.meta.url), 'utf8');
  assert.ok(controller.includes("'prepare_care_preview'"), 'the controller never asks for the preview');
  const desktop = readFileSync(new URL('../../desktop/src/main.rs', import.meta.url), 'utf8');
  assert.ok(desktop.includes('async fn prepare_care_preview') && desktop.includes('            prepare_care_preview,'), 'the desktop has no prepare_care_preview command');
});
