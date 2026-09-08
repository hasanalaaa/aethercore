<script lang="ts">
  /**
   * Phase 23 — InsightsPanel (advisory-only, air-gapped).
   *
   * Mounted on report views (Performance / Health / Care). "Explain this" triggers
   * ON-DEMAND inference; the badge always names the engine that served the insights
   * (local model or deterministic fallback) and states local · offline.
   *
   * Every insight shown here carries the observation it cites. An insight without
   * citations is not rendered dimly or with a caveat — it is dropped, and the
   * panel says how many were dropped rather than quietly showing a shorter list.
   * `citedOnly` is the gate; `EvidenceChip` takes a required `Evidence`, so there
   * is no code path that renders an uncitable claim.
   *
   * Apple interaction contract: pointerdown fluidPress, springs only, reduced-motion
   * and reduced-transparency honored via shared tokens, TechnicalText bidi isolation
   * for evidence ids. Loading uses the shared progress primitive. Dismissing the
   * panel cancels interest (next open refreshes from the ephemeral session).
   */
  import { shellState } from '../../app/shell-state';
  import { streamState } from '../../platform/stream-state';
  import { Pressable } from '../../design/primitives';
  import { EmptyState, EvidenceChip, citedOnly, type Evidence } from '../../design/signature';
  import { t, td, hasMessageKey } from '../../lib/i18n';
  import type { MessageKey } from '../../lib/i18n';
  import type { Insight } from '../../lib/contracts';
  import {
    dismissInsight,
    insightsUi,
    requestInsights,
    refreshInsights,
  } from './controller';

  $: locale = $shellState.locale;
  $: response = $streamState.insights;
  $: loading = $insightsUi.loading;
  $: engineLabel = response?.engineLabel ?? 'ruleFallback';

  function surfaceLabel(surface: string): string {
    switch (surface) {
      case 'bottleneckReport': return td('insight.surface.bottleneckReport', locale);
      case 'repairDiagnosis': return td('insight.surface.repairDiagnosis', locale);
      case 'timelinePattern': return td('insight.surface.timelinePattern', locale);
      default: return td('insight.surface.maintenanceHistory', locale);
    }
  }

  function confidenceKey(c: string): MessageKey {
    switch (c) {
      case 'Strong': return 'insight.confidence.strong';
      case 'Moderate': return 'insight.confidence.moderate';
      default: return 'insight.confidence.weak';
    }
  }

  /**
   * The observation an insight rests on, built only from fields the service
   * actually sent. Nothing here is composed or estimated: the chip face names
   * the surfaces cited and how many, and the expanded block lists each evidence
   * id against the surface it came from, plus the engine that drew the
   * conclusion. An insight citing nothing returns null and is dropped.
   */
  function evidenceFor(insight: Insight): Evidence | null {
    if (!insight.citations.length) return null;
    const surfaces = [...new Set(insight.citations.map((c) => surfaceLabel(c.surface)))];
    const engine = td(engineLabel === 'localModel' ? 'insight.engine.localModel' : 'insight.engine.ruleFallback', locale);
    return {
      cite: `${surfaces.join(' · ')} · ${insight.citations.length}`,
      raw: [
        ...insight.citations.map((c) => `${c.evidenceId}  ${c.surface}`),
        `engine  ${engine}`,
      ].join('\n'),
    };
  }

  $: gate = citedOnly<Insight>(response?.insights ?? [], evidenceFor);
  $: insights = gate.cited;
  $: uncitable = gate.dropped;
  let question = '';

  async function askLocal(): Promise<void> {
    const text = question.trim();
    if (!text) return;
    question = '';
    await requestInsights('chat', text);
  }

  /**
   * Clears the session's insights from this view. It is not a panel dismissal:
   * AppShell renders this panel unconditionally on Overview and Activity and
   * there is no affordance that would bring it back, so a control that removed
   * it would be a one-way door. The label says "clear" for that reason.
   */
  function onDismiss(): void {
    streamState.update((s) => ({ ...s, insights: null }));
  }
</script>

<section class="panel insights-panel" aria-live="polite">
  <div class="panel-head">
    <div>
      <p class="eyebrow">{t('insight.eyebrow', locale)}</p>
      <h3>{t('insight.title', locale)}</h3>
      <p class="insight-badge">
        <span class="insight-badge-dot" class:model={engineLabel === 'localModel'}></span>
        {t('insight.badgeAdvisory', locale)} · {t('insight.badgeLocal', locale)} · {t('insight.badgeOffline', locale)}
        · {td(engineLabel === 'localModel' ? 'insight.engine.localModel' : 'insight.engine.ruleFallback', locale)}
      </p>
    </div>
    <div class="insight-actions">
      <form class="local-chat" onsubmit={(event) => { event.preventDefault(); askLocal(); }}>
        <label class="sr-only" for="local-question">{t('insight.askLocal', locale)}</label>
        <input id="local-question" bind:value={question} maxlength="500" autocomplete="off" placeholder={t('insight.askLocal', locale)} />
        <Pressable className="primary" type="submit" disabled={loading || !question.trim()}>{t('insight.sendLocal', locale)}</Pressable>
      </form>
      <Pressable className="ghost" onclick={refreshInsights}>{t('common.refresh', locale)}</Pressable>
      <Pressable className="primary" onclick={() => requestInsights('explain')} disabled={loading}>
        {loading ? t('insight.thinking', locale) : t('insight.explainThis', locale)}
      </Pressable>
      <Pressable className="ghost" ariaLabel={t('insight.clearInsights', locale)} onclick={onDismiss}>✕</Pressable>
    </div>
  </div>

  {#if loading}
    <div class="ac-progress" role="progressbar" aria-label={t('insight.thinking', locale)}><span></span></div>
  {:else if insights.length === 0}
    <EmptyState title={t('common.notCollected', locale)} body={t('insight.empty', locale)} />
  {:else}
    <ul class="insight-list">
      {#each insights as insight (insight.id)}
        <li class="insight-card">
          <div class="insight-card-top">
            <span class="insight-confidence" data-level={insight.confidence.toLowerCase()}>
              {td(confidenceKey(insight.confidence), locale)}
            </span>
            <Pressable className="ghost small" onclick={() => dismissInsight(insight.id)}>{t('common.dismiss', locale)}</Pressable>
          </div>
          <p class="insight-summary">{hasMessageKey(insight.summaryKey) ? td(insight.summaryKey, locale) : insight.summaryKey}</p>
          <p class="insight-explanation">{insight.explanation}</p>
          <EvidenceChip evidence={insight.evidence} {locale} />
        </li>
      {/each}
    </ul>
    {#if uncitable > 0}
      <!-- Said out loud rather than shown as a shorter list. -->
      <p class="insight-dropped">{t('insight.uncitableDropped', locale, { count: uncitable })}</p>
    {/if}
  {/if}
</section>

<style>
  .insight-badge {
    margin-block-start: var(--ac-space-1);
    margin-block-end: 0;
    color: var(--ac-text-3);
    font-size: var(--ac-type-caption);
  }
  .insight-badge-dot {
    display: inline-block;
    inline-size: 0.5rem;
    block-size: 0.5rem;
    border-radius: var(--ac-radius-pill);
    background: var(--ac-text-4);
    margin-inline-end: var(--ac-space-2);
  }
  .insight-badge-dot.model { background: var(--role-healthy); }

  .insight-actions { display: flex; gap: var(--ac-space-2); align-items: center; flex-wrap: wrap; }
  .local-chat { display: flex; gap: var(--ac-space-2); flex: 1 1 100%; }
  .local-chat input { min-width: 0; flex: 1; border: 1px solid var(--ac-border); border-radius: var(--ac-radius-sm); background: var(--ac-surface-2); color: var(--ac-text-1); padding: var(--ac-space-2) var(--ac-space-3); }

  .insight-list {
    list-style: none;
    margin: var(--ac-space-4) 0 0;
    padding: 0;
    display: grid;
    gap: var(--ac-space-3);
  }

  .insight-card {
    display: flex;
    flex-direction: column;
    gap: var(--ac-space-2);
    padding: var(--ac-space-4) var(--ac-space-5);
    border: 1px solid var(--ac-edge);
    border-radius: var(--ac-radius-lg);
    background: var(--ac-glass-2);
  }

  .insight-card-top { display: flex; justify-content: space-between; align-items: center; gap: var(--ac-space-3); }

  /* Confidence is the service's own category, shown as a word. It is never
     rendered as a number: a numeric score nobody measured is exactly the kind of
     invented figure this design forbids. */
  .insight-confidence {
    color: var(--ac-text-3);
    font-size: var(--ac-type-technical);
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: var(--ac-tracking-tag);
  }
  .insight-confidence[data-level='strong'] { color: var(--role-healthy); }
  .insight-confidence[data-level='moderate'] { color: var(--ac-text-2); }

  .insight-summary { margin: 0; font-size: var(--ac-type-body); font-weight: 600; color: var(--ac-text-1); }
  .insight-explanation { margin: 0 0 var(--ac-space-2); font-size: var(--ac-type-callout); line-height: 1.65; color: var(--ac-text-3); text-wrap: pretty; }

  .insight-dropped {
    margin: var(--ac-space-3) 0 0;
    font-size: var(--ac-type-technical);
    color: var(--ac-text-4);
  }
</style>
