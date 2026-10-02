import { writable } from 'svelte/store';
import type { RecurrencePatternsResponse, TimelineResponse } from '../../lib/contracts';
import { runBusy } from '../../app/shell-state';
import { serviceInvoke } from '../../platform/service-client';
import { streamState } from '../../platform/stream-state';
import { TimelinePaging } from './paging';

/** Server-enforced page bound; the service clamps again regardless. */
export const TIMELINE_PAGE_SIZE = 100;

/** UI-only selection state for the recurrence inspector. */
const paging = new TimelinePaging();
export const timelineUi = writable({
  ...paging.state,
  selectedPatternIdentity: '' as string,
  selectedEventSourceId: '' as string,
  /** The last read failed: said as such, not shown as an empty timeline (P76). */
  readFailed: false,
});

function mirrorPaging(): void {
  timelineUi.update((ui) => ({ ...ui, ...paging.state }));
}
let lastLivePage: TimelineResponse | null = null;
let observedSession = false;
streamState.subscribe((state) => {
  const changed = paging.observeSession(`${state.session.sessionId}:${state.session.connected}:${state.resetGeneration}`);
  if (changed) {
    lastLivePage = state.timelinePage;
    if (!observedSession && lastLivePage) paging.observeLivePage(lastLivePage);
  }
  else if (state.timelinePage !== lastLivePage) {
    lastLivePage = state.timelinePage;
    paging.observeLivePage(lastLivePage);
  }
  observedSession = true;
  mirrorPaging();
});

/** Refresh starts a new snapshot; older pages stay in that snapshot until reload. */
async function readPage(older: boolean): Promise<TimelineResponse | null> {
  const request = paging.begin(older);
  if (!request) return null;
  mirrorPaging();
  let fetched: TimelineResponse | null = null;
  await runBusy(async () => {
    try {
      const page = await serviceInvoke<TimelineResponse>('get_timeline_page', request.args);
      if (paging.accept(request.ticket, page, older)) fetched = paging.state.page;
    } catch { paging.fail(request.ticket); }
    mirrorPaging();
  });
  return fetched;
}
export const loadTimeline = (): Promise<TimelineResponse | null> => readPage(false);
export const loadOlderTimeline = (): Promise<TimelineResponse | null> => readPage(true);

/** Fetches recurrence patterns with their full evidence matrices. */
export async function loadRecurrencePatterns(): Promise<RecurrencePatternsResponse | null> {
  let fetched: RecurrencePatternsResponse | null = null;
  await runBusy(async () => {
    try {
      const response = await serviceInvoke<RecurrencePatternsResponse>(
        'get_recurrence_patterns',
      );
      if (response.patterns.length > 0) {
        selectPattern(response.patterns[0].semanticIdentitySha256);
      }
      return (fetched = response);
    } catch {
      return (fetched = null);
    }
  });
  return fetched;
}

export function selectPattern(identity: string): void {
  timelineUi.update((state) => ({ ...state, selectedPatternIdentity: identity }));
}

/** Select only a persisted event actually present in the loaded browsing window. */
export function selectTimelineEntry(sourceId: string): boolean {
  const loaded = paging.select(sourceId);
  mirrorPaging();
  return loaded;
}
