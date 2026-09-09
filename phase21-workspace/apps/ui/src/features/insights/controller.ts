import { writable } from 'svelte/store';
import type { InsightsResponse } from '../../lib/contracts';
import { serviceInvoke } from '../../platform/service-client';
import { patchStreamState } from '../../platform/stream-state';

/** Panel-local state: loading flag for the shared progress primitive. */
export const insightsUi = writable({
  loading: false,
  error: false,
});

/**
 * "Explain this" affordance: on-demand inference (I4 — strictly user-initiated).
 * Cancellation on dismiss is inherent: the response only lands if the panel is
 * still open, because the stream slice is overwritten on next open.
 */
export async function requestInsights(questionKey = 'explain', question = ''): Promise<boolean> {
  let alreadyLoading = false;
  insightsUi.update((s) => { alreadyLoading = s.loading; return alreadyLoading ? s : { ...s, loading: true, error: false }; });
  if (alreadyLoading) return false;
  try {
    const response = await serviceInvoke<InsightsResponse>('request_insight', { questionKey, question });
    patchStreamState({ insights: response });
    return true;
  } catch {
    insightsUi.update((s) => ({ ...s, error: true }));
    return false;
  } finally {
    insightsUi.update((s) => ({ ...s, loading: false }));
  }
}

/** Pulls the current session insights without running inference. */
export async function refreshInsights(): Promise<void> {
  try {
    const response = await serviceInvoke<InsightsResponse>('list_insights');
    patchStreamState({ insights: response });
  } catch {
    /* offline: keep current slice */
  }
}

/** Dismisses one session insight (ephemeral; server drops it from the session). */
export async function dismissInsight(insightId: string): Promise<void> {
  try {
    const response = await serviceInvoke<InsightsResponse>('dismiss_insight', { insightId });
    patchStreamState({ insights: response });
  } catch {
    /* offline: dismissal is session-only anyway */
  }
}
