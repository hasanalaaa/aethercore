// P76 DBT-P76-006: the timeline printed its entry codes raw — unseen while the newest page was
// always empty. Every code family crates/timeline-intelligence/src/ingest.rs emits reads as a
// sentence in both languages; the families are read from that file, so a new one fails here.
// Run: node --experimental-strip-types --import ./tests/resolve-ts.mjs --test tests/*.test.ts
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { describeTimelineEntry } from '../src/lib/i18n/index.ts';

const arabic = /[؀-ۿ]/;
const ingest = readFileSync(new URL('../../../crates/timeline-intelligence/src/ingest.rs', import.meta.url), 'utf8');

// One sample per family, in the shape ingest.rs formats it.
const samples: Record<string, { domain: string; code: string }> = {
  'journal.transition': { domain: 'operationJournal', code: 'journal.transition:Completed' },
  'execution.outcome': { domain: 'Cleanup', code: 'execution.outcome:Failed' },
  'care.run': { domain: 'oneClickCare', code: 'care.run:Completed' },
  'scan.completed': { domain: 'deepScan', code: 'scan.completed:findings=3' },
};

test('every timeline code family ingest.rs emits is described in English and Arabic', () => {
  const families = [...ingest.matchAll(/format!\("([a-z]+\.[a-z]+):/g)].map((m) => m[1]);
  assert.deepEqual([...new Set(families)].sort(), Object.keys(samples).sort());
  for (const [family, entry] of Object.entries(samples)) {
    const ar = describeTimelineEntry(entry, 'ar');
    const en = describeTimelineEntry(entry, 'en');
    assert.match(ar, arabic, family);
    assert.doesNotMatch(ar, new RegExp(family.replace('.', '\\.')), `${family} printed raw: ${ar}`);
    assert.notEqual(en, entry.code, family);
  }
  assert.equal(describeTimelineEntry({ domain: 'Cleanup', code: 'execution.outcome:Failed' }, 'en'), 'Cleanup: Failed');
  assert.match(describeTimelineEntry({ domain: 'oneClickCare', code: 'care.run:AwaitingConsent' }, 'ar'), /بانتظار الموافقة/);
});
