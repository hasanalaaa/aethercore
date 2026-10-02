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

import { TimelinePaging } from '../src/features/timeline/paging.ts';
import type { TimelineResponse } from '../src/lib/contracts.ts';
const page = (digest = 'snapshot-a', cursor: string | undefined = 'opaque'): TimelineResponse => ({
  entries: [], hasMore: true, nextBeforeSequence: 10, digestSha256: digest, duplicatesCollapsed: 0, nextSnapshotCursor: cursor,
});
test('timeline request presence starts a snapshot and keeps the legacy integer fallback', () => {
  const paging = new TimelinePaging(); const first = paging.begin(false)!;
  assert.equal(first.args.snapshotCursor, ''); assert.equal(first.args.beforeSequence, 0);
  paging.accept(first.ticket, page(), false); assert.equal(paging.begin(true)!.args.snapshotCursor, 'opaque');
  const legacy = new TimelinePaging(); const initial = legacy.begin(false)!;
  legacy.accept(initial.ticket, { ...page('old-server'), nextSnapshotCursor: undefined }, false);
  const older = legacy.begin(true)!; assert.equal(older.args.snapshotCursor, undefined); assert.equal(older.args.beforeSequence, 10);
});
test('timeline rejected cursors and changed legacy history require an explicit reload', () => {
  for (const rejected of [{ ...page(), reloadRequired: true }, page('changed')]) {
    const paging = new TimelinePaging(); const first = paging.begin(false)!;
    paging.accept(first.ticket, page(), false); const older = paging.begin(true)!;
    paging.accept(older.ticket, rejected, true);
    assert.equal(paging.state.reloadRequired, true); assert.equal(paging.state.page, null);
  }
});
test('timeline session changes and newer refreshes discard delayed RPC answers', () => {
  const paging = new TimelinePaging(); paging.observeSession('owner-a-session');
  const old = paging.begin(false)!; paging.observeSession('owner-b-session');
  assert.equal(paging.accept(old.ticket, page('owner-a'), false), false); assert.equal(paging.state.page, null);
  const first = paging.begin(false)!; const newest = paging.begin(false)!;
  assert.equal(paging.accept(first.ticket, page('stale'), false), false);
  assert.equal(paging.accept(newest.ticket, page('current'), false), true);
  assert.equal(paging.state.page!.digestSha256, 'current');
});
test('timeline live activity leaves pinned pages intact and reset invalidates their cursor', () => {
  const paging = new TimelinePaging(); const initial = page();
  paging.observeLivePage(initial); assert.equal(paging.state.page, initial);
  const request = paging.begin(false)!; paging.accept(request.ticket, initial, false);
  const pending = paging.begin(true)!;
  paging.observeLivePage(page('new-live-history'));
  assert.equal(paging.state.page, initial); assert.equal(paging.state.newActivity, true);
  paging.observeLivePage(null);
  assert.equal(paging.accept(pending.ticket, page(), true), false);
  assert.equal(paging.state.page, null); assert.equal(paging.state.reloadRequired, true);
});

import { t } from '../src/lib/i18n/index.ts';
test('timeline cursor reload and new activity notices are owned EN/AR messages', () => {
  for (const key of ['timeline.reloadRequired', 'timeline.newActivity', 'timeline.older'] as const) {
    assert.match(t(key, 'ar'), arabic); assert.notEqual(t(key, 'en'), key);
  }
});

test('timeline source navigation only selects a loaded event and clears on session replacement', () => {
  const paging = new TimelinePaging(); paging.observeSession('owner-a');
  const current = { ...page(), entries: [{ sourceId: 'care-run:recorded', class: 'TIMELINE_EVENT_CLASS_OPERATION' as const,
    domain: 'oneClickCare', code: 'care.run:Completed', outcome: 'TIMELINE_OUTCOME_NEUTRAL' as const,
    observedUnixMs: 1000, semanticIdentitySha256: 'identity' }] };
  paging.observeLivePage(current);
  assert.equal(paging.select('care-run:missing'), false); assert.equal(paging.state.selectedEventSourceId, '');
  assert.equal(paging.select('care-run:recorded'), true); assert.equal(paging.state.selectedEventSourceId, 'care-run:recorded');
  paging.observeSession('owner-b'); assert.equal(paging.state.selectedEventSourceId, '');
});
