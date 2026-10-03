import { get } from 'svelte/store';
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { registerHooks } from 'node:module';
import { compile } from 'svelte/compiler';
import { render } from 'svelte/server';
import { measuredDeletedBytes } from '../src/features/care/result.ts';
import { td } from '../src/lib/i18n/index.ts';
import type { CareStepReport } from '../src/lib/contracts.ts';

registerHooks({ resolve(specifier, context, next) {
  try { return next(specifier, context); } catch (error) {
    if ((error as { code?: string }).code === 'ERR_UNSUPPORTED_DIR_IMPORT') return next(`${specifier}/index.ts`, context);
    throw error;
  }
}, load(url, context, next) {
  if (url.endsWith('.svelte')) return { format: 'module', shortCircuit: true,
    source: compile(readFileSync(new URL(url), 'utf8'), { filename: new URL(url).pathname, generate: 'server' }).js.code };
  // The same explicitly enabled fixture transport used by Vite, before the module graph loads.
  if (url.endsWith('/platform/transport.ts')) return { format: 'module-typescript', shortCircuit: true,
    source: readFileSync(new URL(url), 'utf8').replace("import.meta.env.VITE_AETHERCORE_TEST_TRANSPORT", "'1'") };
  return next(url, context);
} });
Object.assign(globalThis, {
  document: { documentElement: { dataset: {}, style: {} } },
  window: { matchMedia: () => ({ matches: false }) },
  __AETHERCORE_TEST_TRANSPORT__: { invoke: async () => page, listen: async () => () => {} },
});
const { shellState } = await import('../src/app/shell-state.ts');
const { patchStreamState, streamState } = await import('../src/platform/stream-state.ts');
const { default: CarePanel } = await import('../src/features/care/CarePanel.svelte');
const step = { stepIndex: 0, domainPlanId: 'cleanup-plan', domainKind: 'Cleanup', safetyLevel: 0,
  state: 'Completed', outcome: 'VerifiedByDomain', domainVerificationState: 'Verified', failureMessageKey: '',
  hasActualDeletedBytes: true, actualDeletedBytes: '0' } as CareStepReport;
const status = { runId: 'run-1', state: 'Completed', stage: 'Report', sessionConsentGranted: false,
  planDigestSha256: 'abc', steps: [step], updatedUnixMs: 10, summaryKey: 'care.summary.completed' };
let page: unknown = null;
for (const locale of ['en', 'ar'] as const) {
  test(`${locale}: exact measured zero, maximum and invalid/unmeasured outcomes`, () => {
    assert.equal(measuredDeletedBytes(step, locale), '0 B');
    assert.equal(measuredDeletedBytes({ ...step, actualDeletedBytes: '1536' }, locale), '1.50 KiB');
    assert.equal(measuredDeletedBytes({ ...step, actualDeletedBytes: '18446744073709551615' }, locale), '16.00 EiB');
    for (const value of ['', '-1', '01', '18446744073709551616', '0.5'])
      assert.equal(measuredDeletedBytes({ ...step, actualDeletedBytes: value }, locale), null);
    for (const update of [{ hasActualDeletedBytes: false }, { state: 'Failed' }, { outcome: 'Skipped' },
      { domainVerificationState: 'Unverified' }, { domainKind: 'Startup' }])
      assert.equal(measuredDeletedBytes({ ...step, ...update }, locale), null);
  });
  test(`${locale}: real Care panel renders measured logical bytes, dated report, details and unavailable values`, () => {
    shellState.update((state) => ({ ...state, locale }));
    patchStreamState({ careStatus: status });
    let html = render(CarePanel).body;
    assert.ok(html.includes(td('care.deletedLogicalBytes' as never, locale)));
    assert.ok(html.includes('0 B'));
    assert.ok(html.includes(td('care.details' as never, locale)));
    assert.ok(html.includes(td('care.resultAt' as never, locale, { time: '' }).split(':')[0]));
    patchStreamState({ careStatus: { ...status, steps: [{ ...step, hasActualDeletedBytes: false }] } });
    html = render(CarePanel).body;
    assert.ok(html.includes(td('care.notMeasured' as never, locale)));
    assert.ok(!html.includes('0 B'));
    patchStreamState({ careStatus: { ...status, steps: [], summaryKey: 'care.summary.historyIncomplete' } });
    html = render(CarePanel).body;
    assert.ok(html.includes(td('care.summary.historyIncomplete', locale)));
    assert.ok(html.includes(td('care.details' as never, locale)));
  });
}

const { openCareDetails, careDetails } = await import('../src/features/care/history.ts');
const { timelineUi } = await import('../src/features/timeline/controller.ts');
test('Care details selects the exact loaded run, reports absent history and rejects a late session reply', async () => {
  patchStreamState({ careStatus: status });
  page = { entries: [{ sourceId: 'care-run:run-1' }, { sourceId: 'care-run:other' }], digestSha256: 'snapshot', reloadRequired: false };
  await openCareDetails('run-1');
  assert.equal(get(shellState).activePage, 'activity');
  assert.equal(get(timelineUi).selectedEventSourceId, 'care-run:run-1');
  assert.equal(get(careDetails), 'idle');
  page = { entries: [{ sourceId: 'care-run:other' }], digestSha256: 'next', reloadRequired: false };
  await openCareDetails('run-1');
  assert.equal(get(timelineUi).selectedEventSourceId, '');
  assert.equal(get(careDetails), 'missing');
  let resolve!: (value: unknown) => void;
  page = new Promise((done) => { resolve = done; });
  const pending = openCareDetails('run-1');
  patchStreamState({ session: { ...get(streamState).session, sessionId: 'different-session' } });
  resolve({ entries: [{ sourceId: 'care-run:run-1' }], digestSha256: 'stale', reloadRequired: false });
  await pending;
  assert.equal(get(timelineUi).selectedEventSourceId, '');
  assert.equal(get(careDetails), 'idle');
});
