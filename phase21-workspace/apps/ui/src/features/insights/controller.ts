import { writable } from 'svelte/store';
import type { InsightsResponse } from '../../lib/contracts';
import { runBusy } from '../../app/shell-state';
import { serviceInvoke } from '../../platform/service-client';
import { patchStreamState } from '../../platform/stream-state';

/** Panel-local state: loading flag for the shared progress primitive. */
export const insightsUi = writable({
  loading: false,
});

/**
 * "Explain this" affordance: on-demand inference (I4 — strictly user-initiated).
 * Cancellation on dismiss is inherent: the response only lands if the panel is
 * still open, because the stream slice is overwritten on next open.
 */
export async function requestInsights(questionKey = 'explain', question = ''): Promise<void> {
  insightsUi.update((s) => ({ ...s, loading: true }));
  try {
    const response = await serviceInvoke<InsightsResponse>('request_insight', { questionKey, question });
    patchStreamState({ insights: response });
  } catch {
    // I3: the AI layer never surfaces an error to the UI; empty state renders.
    patchStreamState({ insights: null });
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
    await serviceInvoke<void>('dismiss_insight', { insightId });
  } catch {
    /* offline: dismissal is session-only anyway */
  }
}
