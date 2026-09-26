// P75 ui-truth. Run: node --experimental-strip-types --test tests/*.test.ts
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { runDueOutcome } from '../src/features/fleet/run-due.ts';

test('a run that could not load its schedules is a failure', () => {
  const outcome = runDueOutcome({ ran: 0, runs: [], error: 'schedule store unreadable' });
  assert.equal(outcome.ok, false);
  assert.equal(outcome.detail, 'schedule store unreadable');
});

test('a run with a failed host is a failure, and says how many', () => {
  const outcome = runDueOutcome({
    ran: 1,
    runs: [{ scheduleId: 's1', hostsAttempted: 3, hostsOk: 2, hostsFailed: 1, error: null }],
  });
  assert.deepEqual([outcome.ok, outcome.failed, outcome.attempted], [false, 1, 3]);
});

test('a schedule that errored is a failure even with no host counted', () => {
  const outcome = runDueOutcome({
    ran: 1,
    runs: [{ scheduleId: 's1', hostsAttempted: 0, hostsOk: 0, hostsFailed: 0, error: 'no host in scope' }],
  });
  assert.equal(outcome.ok, false);
  assert.equal(outcome.detail, 'no host in scope');
});

test('every host ok is a success', () => {
  const outcome = runDueOutcome({
    ran: 1,
    runs: [{ scheduleId: 's1', hostsAttempted: 2, hostsOk: 2, hostsFailed: 0, error: null }],
  });
  assert.deepEqual([outcome.ok, outcome.failed, outcome.attempted], [true, 0, 2]);
});
