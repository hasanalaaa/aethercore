import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { registerHooks } from 'node:module';
import { compile } from 'svelte/compiler';
import { render } from 'svelte/server';
import type { RepairAssessment, UiKernelEvent } from '../src/lib/contracts.ts';
import { hasMessageKey, td } from '../src/lib/i18n/index.ts';
import { assessmentElapsedMs, runElapsedClock } from '../src/features/repair/elapsed.ts';

registerHooks({ resolve(specifier, context, next) {
  try { return next(specifier, context); } catch (error) {
    if ((error as { code?: string }).code === 'ERR_UNSUPPORTED_DIR_IMPORT') return next(`${specifier}/index.ts`, context);
    throw error;
  }
}, load(url, context, next) {
  if (url.endsWith('.svelte')) return { format: 'module', shortCircuit: true,
    source: compile(readFileSync(new URL(url), 'utf8'), { filename: new URL(url).pathname, generate: 'server' }).js.code };
  if (url.endsWith('/platform/transport.ts')) return { format: 'module-typescript', shortCircuit: true,
    source: readFileSync(new URL(url), 'utf8').replace("import.meta.env.VITE_AETHERCORE_TEST_TRANSPORT", "'1'") };
  return next(url, context);
} });
let rpcAnswer: unknown = null;
Object.assign(globalThis, {
  document: { documentElement: { dataset: {}, style: {} } },
  window: { matchMedia: () => ({ matches: false }) },
  __AETHERCORE_TEST_TRANSPORT__: { invoke: async () => rpcAnswer, listen: async () => () => {} },
});
Object.defineProperty(globalThis, 'localStorage', { configurable: true,
  value: { getItem: () => null, setItem: () => {}, removeItem: () => {} } });
const { shellState } = await import('../src/app/shell-state.ts');
const { streamState, createInitialStreamState, reduceKernelEvent, applyStreamReset } = await import('../src/platform/stream-state.ts');
const { startRepairAssessment, cancelRepairAssessment } = await import('../src/features/repair/controller.ts');
const { default: RepairPage } = await import('../src/features/repair/RepairPage.svelte');
const now = Date.now();
const assessment = { assessmentId: 'owned-assessment', state: 'Scanning', startedUnixMs: now - 90_000,
  completedUnixMs: 0, errorMessage: '', systemVolume: '', checks: [], intelligence: null,
  currentCheckId: 'sfc-verify' } as RepairAssessment;
const event = (sequence: number, emittedUnixMs: number, payload = assessment): UiKernelEvent =>
  ({ sequence, emittedUnixMs, kind: 'repairAssessment', planId: '', payload });

test('actual ordered assessment reducer binds observation time and rejects a late frame', () => {
  const state = reduceKernelEvent(createInitialStreamState(), event(1, now));
  assert.equal(state.repairAssessmentObservedUnixMs, now);
  assert.equal(reduceKernelEvent(state, event(1, now + 500)), state);
  const next = reduceKernelEvent(state, event(2, now - 1_000));
  assert.equal(next.repairAssessmentObservedUnixMs, 0, 'reversed service epoch remains unknown');
  streamState.set(state);
  applyStreamReset({ reason: 'sequenceReset', currentSequence: 5, replayFloorSequence: 5, messageKey: '' });
  assert.equal((awaitState()).repairAssessmentObservedUnixMs, 0, 'a new session cannot retain an old observation');
});
function awaitState() { let value = createInitialStreamState(); const stop = streamState.subscribe((state) => value = state); stop(); return value; }

test('actual start RPC clears a different assessment clock and preserves a streamed terminal result', async () => {
  streamState.set(reduceKernelEvent(createInitialStreamState(), event(1, now)));
  rpcAnswer = { ...assessment, assessmentId: 'new-assessment' };
  await startRepairAssessment();
  assert.equal(awaitState().repairAssessment.assessmentId, 'new-assessment');
  assert.equal(awaitState().repairAssessmentObservedUnixMs, 0, 'an RPC has no ordered envelope timestamp');
  let resolve!: (value: unknown) => void;
  rpcAnswer = new Promise((done) => { resolve = done; });
  const pending = startRepairAssessment();
  const terminal = { ...assessment, assessmentId: 'new-assessment', state: 'Ready', completedUnixMs: now };
  streamState.set(reduceKernelEvent(awaitState(), event(2, now, terminal)));
  resolve({ ...terminal, state: 'Scanning', completedUnixMs: 0 });
  await pending;
  assert.equal(awaitState().repairAssessment.state, 'Ready');
  assert.equal(awaitState().repairAssessmentObservedUnixMs, now);
});

test('late assessment RPC cannot replace a new streamed identity or a reset session', async () => {
  for (const operation of [startRepairAssessment, cancelRepairAssessment]) {
    for (const reset of [false, true]) {
      streamState.set(reduceKernelEvent(createInitialStreamState(), event(1, now)));
      let resolve!: (value: unknown) => void;
      rpcAnswer = new Promise((done) => { resolve = done; });
      const pending = operation();
      await Promise.resolve();
      if (reset) applyStreamReset({ reason: 'sequenceReset', currentSequence: 2, replayFloorSequence: 2, messageKey: '' });
      const newer = { ...assessment, assessmentId: 'new-streamed-assessment' };
      streamState.set(reduceKernelEvent(awaitState(), event(3, now, newer)));
      resolve({ ...assessment, state: 'Cancelled', completedUnixMs: now });
      await pending;
      assert.equal(awaitState().repairAssessment.assessmentId, newer.assessmentId);
      assert.equal(awaitState().repairAssessmentObservedUnixMs, now);
    }
  }
});

test('local monotonic frame clock throttles seconds, includes hidden-tab time, and cannot publish after stop', () => {
  const oldPerformance = globalThis.performance;
  let monotonic = 0, id = 0, latest = 0;
  const callbacks = new Map<number, FrameRequestCallback>(), cancelled: number[] = [], updates: number[] = [];
  const oldRequest = globalThis.requestAnimationFrame, oldCancel = globalThis.cancelAnimationFrame;
  Object.defineProperty(globalThis, 'performance', { configurable: true, value: { now: () => monotonic } });
  globalThis.requestAnimationFrame = (callback) => { latest = ++id;callbacks.set(latest, callback);return latest; };
  globalThis.cancelAnimationFrame = (frame) => { cancelled.push(frame); };
  const paint = (time: number) => { monotonic = time;callbacks.get(latest)!(time); };
  try {
    const stop = runElapsedClock((value) => updates.push(value));
    paint(350);paint(1_050);paint(1_250);paint(6_100);
    assert.deepEqual(updates, [1_000, 6_000], 'five seconds in a background tab is time, not one frame');
    stop();paint(8_000);
    assert.deepEqual(updates, [1_000, 6_000]);
    assert.ok(cancelled.includes(latest));
    const resumed = runElapsedClock((value) => updates.push(value), 6_000);
    paint(8_500);paint(9_100);resumed();
    assert.equal(updates.at(-1), 7_000, 'resuming starts from the frozen local elapsed value');
  } finally {
    Object.defineProperty(globalThis, 'performance', { configurable: true, value: oldPerformance });
    globalThis.requestAnimationFrame = oldRequest;globalThis.cancelAnimationFrame = oldCancel;
  }
});

test('elapsed timestamp validation preserves terminal completion and refuses missing, future and reversed epochs', () => {
  assert.equal(assessmentElapsedMs(assessment, now, 5_000, now), 95_000);
  const terminal = { ...assessment, state: 'Cancelled', completedUnixMs: now - 30_000 };
  assert.equal(assessmentElapsedMs(terminal, now, 500_000, now), 60_000, 'terminal time is never advanced by local frames');
  for (const update of [0, assessment.startedUnixMs - 1, now + 1, NaN, Infinity])
    assert.equal(assessmentElapsedMs(assessment, update, 0, now), null);
  for (const start of [0, now + 1, NaN, Number.MAX_SAFE_INTEGER + 1])
    assert.equal(assessmentElapsedMs({ ...assessment, startedUnixMs: start }, now, 0, now), null);
});

for (const locale of ['en', 'ar'] as const) {
  test(`${locale}: actual RepairPage shows elapsed and last service observation outside the live region`, () => {
    shellState.update((state) => ({ ...state, locale }));
    const state = reduceKernelEvent(createInitialStreamState(), event(1, now));
    state.snapshot.connected = true;
    streamState.set(state);
    const html = render(RepairPage).body;
    assert.match(html, /class="assessment-timing(?:\s[^"]*)?"/);
    assert.ok(html.includes(td('repair.elapsed' as never, locale, { minutes: 1, seconds: 30 })));
    assert.ok(html.includes(td('repair.lastServiceUpdate' as never, locale)));
    const hero = html.slice(html.indexOf('phase4-hero'), html.indexOf('class="assessment-timing"'));
    assert.ok(hero.includes('</section>'), 'timing must be outside the phase live region');
    assert.match(html, /role="timer"[^>]*aria-live="off"/);
    assert.doesNotMatch(html, /aria-valuenow="100"/, 'elapsed time is never a completion percentage');
  });
  test(`${locale}: disconnected, terminal and invalid service times do not invent a current clock`, () => {
    assert.ok(hasMessageKey('repair.elapsed') && hasMessageKey('repair.timeUnavailable'));
    shellState.update((state) => ({ ...state, locale }));
    for (const payload of [assessment, { ...assessment, state: 'Cancelled', completedUnixMs: now - 30_000 }]) {
      const state = reduceKernelEvent(createInitialStreamState(), event(1, now, payload));
      state.snapshot.connected = false;streamState.set(state);
      const html = render(RepairPage).body;
      assert.ok(html.includes(td('repair.elapsed' as never, locale, { minutes: 1, seconds: payload.state === 'Scanning' ? 30 : 0 })));
      if (payload.state === 'Scanning') {
        assert.ok(hasMessageKey('repair.elapsedPaused'));
        assert.ok(html.includes(td('repair.elapsedPaused' as never, locale)));
      }
    }
    for (const emitted of [0, assessment.startedUnixMs - 1, now + 60_000]) {
      streamState.set(reduceKernelEvent(createInitialStreamState(), event(1, emitted)));
      const html = render(RepairPage).body;
      assert.ok(html.includes(td('repair.timeUnavailable' as never, locale)));
    }
  });
}
