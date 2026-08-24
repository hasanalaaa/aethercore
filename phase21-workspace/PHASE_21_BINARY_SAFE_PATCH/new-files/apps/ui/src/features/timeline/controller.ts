import { writable } from 'svelte/store';
import type { RecurrencePatternsResponse, TimelineResponse } from '../../lib/contracts';
import { runBusy } from '../../app/shell-state';
import { serviceInvoke } from '../../platform/service-client';
import { patchStreamState } from '../../platform/stream-state';

/** Server-enforced page bound; the service clamps again regardless. */
export const TIMELINE_PAGE_SIZE = 100;

/** UI-only selection state for the recurrence inspector. */
export const timelineUi = writable({
  selectedPatternIdentity: '' as string,
});

/** Fetches the newest timeline page and mirrors it into the stream-state slice. */
export async function loadTimeline(): Promise<TimelineResponse | null> {
  let fetched: TimelineResponse | null = null;
  await runBusy(async () => {
    try {
      const page = await serviceInvoke<TimelineResponse>('get_timeline_page', {
        pageSize: TIMELINE_PAGE_SIZE,
        beforeSequence: 0,
      });
      patchStreamState({ timelinePage: page });
      fetched = page;
    } catch {
      // Offline / not yet hydrated is a normal early state, not an error dialog.
      patchStreamState({ timelinePage: null });
      fetched = null;
    }
  });
  return fetched;
}

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
