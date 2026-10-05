// P49-004: the Overview's sampling note names the window the counters were observed over
// when the service measured one, and the requested interval (as that, not as a window) when not.
// Run: node --experimental-strip-types --import ./tests/resolve-ts.mjs --test tests/*.test.ts
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { samplingNote } from '../src/features/overview/instrument.ts';
import { createInitialStreamState } from '../src/platform/stream-state.ts';

const perf = (measuredWindowMs?: number | null) =>
  ({ ...createInitialStreamState().performance, capturedUnixMs: 1, intervalMs: 1000, measuredWindowMs }) as never;

test('en: a measured window is shown as the observation window, not the requested interval', () => {
  assert.equal(samplingNote(perf(104), 'en'), '104 ms observation window');
});

test('ar: a measured window is shown as the observation window, not the requested interval', () => {
  // Instrument readings keep Latin digits in the Arabic UI (lib/i18n/runtime.ts NUMBER_LOCALE).
  const note = samplingNote(perf(104), 'ar');
  assert.equal(note, 'نافذة رصد 104 م.ث');
  assert.ok(!note.includes('1,000'), `the 1000 ms request leaked: ${note}`);
});

for (const absent of [undefined, null, 0]) {
  test(`en: a window the provider did not measure (${String(absent)}) falls back to the requested interval, worded as that`, () => {
    const note = samplingNote(perf(absent), 'en');
    assert.equal(note, '1,000 ms requested interval');
    assert.ok(!note.includes('observation window'), note);
  });
  test(`ar: a window the provider did not measure (${String(absent)}) falls back to the requested interval, worded as that`, () => {
    const note = samplingNote(perf(absent), 'ar');
    assert.equal(note, 'الفاصل المطلوب 1,000 م.ث');
    assert.ok(!note.includes('نافذة رصد'), note);
  });
}
