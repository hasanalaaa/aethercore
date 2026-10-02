import { get, writable } from 'svelte/store';
import { setPage } from '../../app/shell-state';
import { streamState } from '../../platform/stream-state';
import { loadTimeline, selectTimelineEntry } from '../timeline/controller';

export const careDetails = writable<'idle' | 'loading' | 'missing'>('idle');
let generation = 0;
let session = '';
streamState.subscribe((state) => {
  const key = `${state.session.sessionId}:${state.session.connected}:${state.resetGeneration}:${state.careStatus?.runId ?? ''}`;
  if (key !== session) { session = key; ++generation; careDetails.set('idle'); }
});
/** Navigate to the persisted run only when its exact source is in the loaded page. */
export async function openCareDetails(runId: string): Promise<void> {
  if (!runId || get(streamState).careStatus?.runId !== runId) return;
  const ticket = ++generation;
  careDetails.set('loading');
  setPage('activity');
  const page = await loadTimeline();
  if (ticket !== generation) return;
  if (get(streamState).careStatus?.runId !== runId) { careDetails.set('idle'); return; }
  careDetails.set(page && selectTimelineEntry(`care-run:${runId}`) ? 'idle' : 'missing');
}
