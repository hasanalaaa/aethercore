import { writable } from 'svelte/store';
import type { InsightsResponse } from '../../lib/contracts';
import type { Locale } from '../../lib/i18n/runtime';
import { serviceInvoke } from '../../platform/service-client';
import { patchStreamState, streamState } from '../../platform/stream-state';
import { RequestScope } from './request-scope';

/** Panel-local state: loading flag for the shared progress primitive. */
export const insightsUi = writable({
  loading: false,
  error: false,
  /** The service answered that the on-device model is still loading (DBT-P75-078). */
  modelLoading: false,
});

const scope = new RequestScope();
let currentLocale: Locale = 'en';
let previousEvidence: readonly unknown[] = [];
streamState.subscribe((state) => {
  const evidence = [state.diagnostics, state.repairAssessment, state.careStatus, state.snapshot.activePlan, state.timelinePage];
  if (evidence.some((value, index) => value !== previousEvidence[index])) {
    previousEvidence = evidence;
    scope.invalidate();
    insightsUi.set({ loading: false, error: false, modelLoading: false });
    if (state.insights) patchStreamState({ insights: null });
  }
});

export function setInsightsLocale(locale: Locale): void {
  if (locale === currentLocale) return;
  currentLocale = locale;
  scope.invalidate();
  patchStreamState({ insights: null });
  insightsUi.set({ loading: false, error: false, modelLoading: false });
}

/**
 * "Explain this" affordance: on-demand inference (I4 — strictly user-initiated).
 * Cancellation on dismiss is inherent: the response only lands if the panel is
 * still open, because the stream slice is overwritten on next open.
 */
export async function requestInsights(locale: Locale, questionKey = 'explain', question = ''): Promise<boolean> {
  let alreadyLoading = false;
  insightsUi.update((s) => { alreadyLoading = s.loading; return alreadyLoading ? s : { ...s, loading: true, error: false, modelLoading: false }; });
  if (alreadyLoading) return false;
  const ticket = scope.begin();
  try {
    const response = await serviceInvoke<InsightsResponse>('request_insight', { questionKey, question, locale });
    if (!scope.current(ticket) || locale !== currentLocale) return false;
    patchStreamState({ insights: response });
    return true;
  } catch (error) {
    if (scope.current(ticket)) insightsUi.update((s) => ({ ...s, error: true, modelLoading: String(error).includes('insight.error.modelLoading') }));
    return false;
  } finally {
    if (scope.current(ticket)) insightsUi.update((s) => ({ ...s, loading: false }));
  }
}

/** Pulls the current session insights without running inference. */
export async function refreshInsights(): Promise<void> {
  const ticket = scope.begin();
  const locale = currentLocale;
  try {
    const response = await serviceInvoke<InsightsResponse>('list_insights', { locale });
    if (scope.current(ticket) && locale === currentLocale) patchStreamState({ insights: response });
  } catch {
    /* offline: keep current slice */
  }
}

/** Dismisses one session insight (ephemeral; server drops it from the session). */
export async function dismissInsight(insightId: string): Promise<void> {
  try {
    await serviceInvoke<InsightsResponse>('dismiss_insight', { insightId });
    await refreshInsights();
  } catch {
    /* offline: dismissal is session-only anyway */
  }
}
