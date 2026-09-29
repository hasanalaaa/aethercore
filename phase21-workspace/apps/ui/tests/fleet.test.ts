// P75 ui-truth. Run: node --experimental-strip-types --test tests/*.test.ts
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { runDueOutcome } from '../src/features/fleet/run-due.ts';
import { localizeOwnedText, td } from '../src/lib/i18n/index.ts';

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

// P77-04B: what the fleet crate and the desktop hand the page as an error reads in the reader's
// language. The messages are read from the Rust sources, so a new one without a rendering fails
// here. Nothing the sentence carries as a payload is pasted into the Arabic text.
const arabic = /[؀-ۿ]/;

test('every error the fleet crate can raise reads in Arabic without its English or its payload', async () => {
  const { readdirSync, readFileSync } = await import('node:fs');
  const { join } = await import('node:path');
  const root = join(import.meta.dirname, '../../../crates/fleet/src');
  const templates: string[] = [];
  for (const file of readdirSync(root)) {
    if (!file.endsWith('.rs')) continue;
    const text = readFileSync(join(root, file), 'utf8');
    for (const m of text.matchAll(/#\[error\(\s*"((?:[^"\\]|\\.)*)"/g)) templates.push(m[1]);
  }
  assert.ok(templates.length >= 30, `read only ${templates.length} messages; the extraction is broken`);
  const fallback = td('text.unavailable', 'ar');
  for (const template of templates) {
    // The fleet's size is data the sentence shows, so its sample is a number.
    const sample = template.replace(/\{[^}]*\}/g, template.startsWith('fleet inventory full') ? '7' : 'zzInjected');
    const shown = localizeOwnedText(sample, 'ar');
    assert.equal(shown.localized, true, sample);
    assert.match(shown.text, arabic, sample);
    assert.notEqual(shown.text, fallback, `no rendering for the fleet message: ${sample}`);
    // A schedule failure names its schedule (data) and nests a detail (prose); the rest carry no payload.
    if (!template.startsWith('schedule {0} failed')) assert.ok(!shown.text.includes('zzInjected'), `${sample} → ${shown.text}`);
    if (template.startsWith('fleet inventory full')) assert.ok(shown.text.includes('7'), shown.text);
  }
  const failed = localizeOwnedText('schedule s1 failed: Backend exploded', 'ar').text;
  assert.ok(failed.includes('s1') && !failed.includes('Backend exploded'), failed);
});

test('the sentences the desktop hands the fleet page read in Arabic', async () => {
  const { readFileSync } = await import('node:fs');
  const { join } = await import('node:path');
  const main = readFileSync(join(import.meta.dirname, '../../desktop/src/main.rs'), 'utf8');
  const literals = new Set<string>();
  for (const m of main.matchAll(/fleet_action_error\(\s*&?[A-Za-z_.]+,\s*"([^"]+)"/g)) literals.add(m[1]);
  for (const m of main.matchAll(/detail: Some\("([^"]+)"\.to_string\(\)\)/g)) literals.add(m[1]);
  assert.ok(literals.size >= 4, `read only ${literals.size} literals: ${[...literals]}`);
  const fallback = td('text.unavailable', 'ar');
  // The schedule handlers write their failure details as `detail: "...".to_string()` next to success ones.
  for (const literal of [...literals, 'unknown schedule id', 'duplicate schedule id', 'open fleet trust store: zzInjected', 'schedules at C:\\x could not be read (zzInjected)',
    'schedules at C:\\x are unreadable (zzInjected); refusing to overwrite them']) {
    const shown = localizeOwnedText(literal, 'ar');
    assert.match(shown.text, arabic, literal);
    assert.notEqual(shown.text, fallback, `no rendering for: ${literal}`);
    assert.ok(!shown.text.includes('zzInjected'), `${literal} → ${shown.text}`);
  }
});

test('the fleet page shows no error as the sentence it was thrown with', async () => {
  const { readFileSync } = await import('node:fs');
  const { join } = await import('node:path');
  const page = readFileSync(join(import.meta.dirname, '../src/features/fleet/FleetPage.svelte'), 'utf8');
  assert.doesNotMatch(page, /String\(error\)/, 'a rejected call is shown as its raw text');
  assert.doesNotMatch(page, /<span>\{remote\.detail\}<\/span>/, 'a remote result detail is shown raw');
  assert.doesNotMatch(page, /<TechnicalText value=\{loadError\}/, 'the load error is shown as technical data');
});
