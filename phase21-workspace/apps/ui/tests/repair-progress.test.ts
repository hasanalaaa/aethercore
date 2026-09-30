// P76 DBT-P76-007: "Assessing…" for 10+ minutes on the owner's install. The page names the
// check running, for how long; every check id the Windows assessment reports has a label in
// both languages (the ids are read from crates/system-repair/src/windows_impl.rs).
// Run: node --experimental-strip-types --import ./tests/resolve-ts.mjs --test tests/*.test.ts
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { hasMessageKey, td } from '../src/lib/i18n/index.ts';

const arabic = /[؀-ۿ]/;

test('every assessment check the Windows provider reports has an English and an Arabic label', () => {
  const rust = readFileSync(new URL('../../../crates/system-repair/src/windows_impl.rs', import.meta.url), 'utf8');
  const assess = rust.slice(rust.indexOf('fn assess('), rust.indexOf('fn repair('));
  const ids = [...assess.matchAll(/step\(\s*&mut checks,\s*"([a-z-]+)"/g)].map((m) => m[1]);
  assert.ok(ids.includes('dism-scan') && ids.includes('sfc-verify') && ids.includes('disk-scan'), `${ids}`);
  for (const id of ids) {
    const key = `repair.check.${id}`;
    assert.ok(hasMessageKey(key), key);
    assert.match(td(key as never, 'ar'), arabic, key);
  }
});

// P78-04: the assessment screen must not be put back on "Assessing" by a late answer, must say when
// the connection is gone instead of pretending. (No ticking clock: the renderer does not poll.)
import { settleAssessment } from '../src/features/repair/settle.ts';
import type { RepairAssessment } from '../src/lib/contracts.ts';

const assessment = (assessmentId: string, state: RepairAssessment['state']): RepairAssessment =>
  ({ assessmentId, state, startedUnixMs: 1, completedUnixMs: 0, errorMessage: '', systemVolume: '', checks: [], currentCheckId: '' }) as unknown as RepairAssessment;

test('a late start or cancel answer never puts a finished assessment back on Scanning', () => {
  const finished = assessment('a1', 'Ready');
  assert.equal(settleAssessment(finished, assessment('a1', 'Scanning')), finished, 'the event stream already said Ready');
  const stopped = assessment('a1', 'Cancelled');
  assert.equal(settleAssessment(stopped, assessment('a1', 'Scanning')), stopped);
  const failed = assessment('a1', 'Failed');
  assert.equal(settleAssessment(failed, assessment('a1', 'Scanning')), failed);
});

test('an answer that is news still replaces what the screen holds', () => {
  const running = assessment('a1', 'Scanning');
  const answer = assessment('a1', 'Scanning');
  assert.equal(settleAssessment(running, answer), answer, 'the same assessment, still scanning: the answer is as good');
  const next = assessment('a2', 'Scanning');
  assert.equal(settleAssessment(assessment('a1', 'Ready'), next), next, 'a new assessment replaces the finished one');
  assert.equal(settleAssessment(assessment('', 'Idle'), next), next);
  const done = assessment('a1', 'Ready');
  assert.equal(settleAssessment(running, done), done, 'a terminal answer replaces Scanning');
});

test('the screen says when the connection is lost, and the cancel names its assessment', () => {
  for (const key of ['repair.disconnected']) {
    assert.ok(hasMessageKey(key), key);
    assert.match(td(key as never, 'ar'), arabic, key);
  }
  const page = readFileSync(new URL('../src/features/repair/RepairPage.svelte', import.meta.url), 'utf8');
  const connected = page.slice(page.indexOf("{#if repairAssessment.state === 'Scanning'}"), page.indexOf('{:else if repairAssessment.errorMessage}'));
  assert.ok(connected.includes('repair.disconnected'), 'a lost connection is not said on the assessment screen');
  assert.ok(connected.indexOf('snapshot.connected') < connected.indexOf('cancelRepairAssessment'), 'the stop button is offered while offline');
  const controller = readFileSync(new URL('../src/features/repair/controller.ts', import.meta.url), 'utf8');
  assert.ok(/cancel_repair_assessment',\s*\{\s*assessmentId/.test(controller), 'the cancel does not carry the assessment id');
  const desktop = readFileSync(new URL('../../desktop/src/main.rs', import.meta.url), 'utf8');
  const command = desktop.slice(desktop.indexOf('async fn cancel_repair_assessment'), desktop.indexOf('async fn create_system_repair_plan'));
  assert.ok(command.includes('assessment_id: String') && command.includes('CancelRepairAssessmentRequest { assessment_id'), 'the desktop drops the assessment id');
});
