<script lang="ts">
  /**
   * Phase 23 — InsightsPanel (advisory-only, air-gapped).
   *
   * Mounted on report views (Performance / Health / Care). "Explain this" triggers
   * ON-DEMAND inference; the badge always names the engine that served the insights
   * (local model or deterministic fallback) and states local · offline.
   *
   * Apple interaction contract: pointerdown fluidPress, springs only, reduced-motion
   * and reduced-transparency honored via shared tokens, TechnicalText bidi isolation
   * for evidence ids. Loading uses the shared progress primitive. Dismissing the
   * panel cancels interest (next open refreshes from the ephemeral session).
   */
  import { fluidPress } from '../../design/motion';
  import { shellState } from '../../app/shell-state';
  import { streamState } from '../../platform/stream-state';
  import { Pressable, TechnicalText } from '../../design/primitives';
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
  $: insights = response?.insights ?? [];
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

  // Panel dismiss = cancellation of interest; clear local view state only.
  function onDismiss(): void {
    streamState.update((s) => ({ ...s, insights: null }));
  }
</script>

<section class="panel insights-panel" aria-live="polite">
  <div class="panel-head">
    <div>
      <p class="eyebrow">{t('insight.eyebrow', locale)}</p>
      <h3>{t('insight.title', locale)}</h3>
      <p class="badge">
        <span class="badge-dot" class:model={engineLabel === 'localModel'}></span>
        {t('insight.badgeAdvisory', locale)} · {t('insight.badgeLocal', locale)} · {t('insight.badgeOffline', locale)}
        · {td(engineLabel === 'localModel' ? 'insight.engine.localModel' : 'insight.engine.ruleFallback', locale)}
      </p>
    </div>
    <div class="head-actions">
      <Pressable on:press={refreshInsights}>
        <button class="ghost-action" type="button">{t('common.refresh', locale)}</button>
      </Pressable>
      <Pressable on:press={() => requestInsights('explain')}>
        <button class="primary-action" type="button" disabled={loading}>
          {loading ? t('insight.thinking', locale) : t('insight.explainThis', locale)}
        </button>
      </Pressable>
      <Pressable on:press={onDismiss}>
        <button class="ghost-action" type="button" aria-label={t('insight.dismissPanel', locale)}>✕</button>
      </Pressable>
    </div>
  </div>

  {#if loading}
    <div class="progress" role="progressbar" aria-label={t('insight.thinking', locale)}>
      <div class="progress-bar"></div>
    </div>
  {:else if insights.length === 0}
    <p class="empty">{t('insight.empty', locale)}</p>
  {:else}
    <ul class="insight-list">
      {#each insights as insight, index (index)}
        <li class="insight-card">
          <div class="card-top">
            <span class="confidence">{td(confidenceKey(insight.confidence), locale)}</span>
            <Pressable on:press={() => dismissInsight(String(index))}>
              <button class="ghost-action small" type="button">{t('common.dismiss', locale)}</button>
            </Pressable>
          </div>
          <p class="summary">{hasMessageKey(insight.summaryKey) ? td(insight.summaryKey, locale) : insight.summaryKey}</p>
          <p class="explanation">{insight.explanation}</p>
          <div class="citations">
            <span class="citations-label">{t('insight.citations', locale)}</span>
            {#each insight.citations as citation}
              <button
                class="citation-chip"
                type="button"
                title={citation.surface}
              >
                <TechnicalText value={citation.evidenceId.slice(0, 12)} />
                <span class="chip-surface">{surfaceLabel(citation.surface)}</span>
              </button>
            {/each}
          </div>
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  .badge {
    margin-block-start: var(--space-1);
    color: var(--muted-foreground);
    font-size: var(--font-size-caption);
  }
  .badge-dot {
    display: inline-block;
    inline-size: 0.5rem;
    block-size: 0.5rem;
    border-radius: 50%;
    background: var(--accent);
    margin-inline-end: var(--space-1);
  }
  .badge-dot.model {
    background: oklch(0.7 0.15 145);
  }
  .head-actions {
    display: flex;
    gap: var(--space-2);
    align-items: center;
  }
  .primary-action {
    border: none;
    background: var(--accent);
    color: var(--background, #fff);
    font: inherit;
    padding: var(--space-2) var(--space-4);
    border-radius: var(--radius-md);
    cursor: pointer;
  }
  .primary-action:disabled {
    opacity: 0.6;
    cursor: default;
  }
  .ghost-action {
    border: none;
    background: transparent;
    color: var(--accent);
    font: inherit;
    padding: var(--space-1) var(--space-2);
    border-radius: var(--radius-sm);
    cursor: pointer;
  }
  .ghost-action.small {
    font-size: var(--font-size-caption);
  }
  .empty {
    color: var(--muted-foreground);
    padding: var(--space-3);
  }
  .progress {
    block-size: 3px;
    background: var(--border);
    border-radius: 999px;
    overflow: hidden;
    margin-block: var(--space-3);
  }
  .progress-bar {
    block-size: 100%;
    inline-size: 40%;
    background: var(--accent);
    border-radius: inherit;
    animation: sweep 1.2s ease-in-out infinite alternate;
  }
  @keyframes sweep {
    from { transform: translateX(-30%); }
    to { transform: translateX(220%); }
  }
  @media (prefers-reduced-motion: reduce) {
    .progress-bar { animation: none; }
  }
  .insight-list {
    list-style: none;
    margin: var(--space-3) 0 0;
    padding: 0;
    display: grid;
    gap: var(--space-3);
  }
  .insight-card {
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    padding: var(--space-3) var(--space-4);
    background: var(--material-thin);
  }
  .card-top {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
  }
  .confidence {
    color: var(--muted-foreground);
    font-size: var(--font-size-caption);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .summary {
    margin-block: var(--space-2) var(--space-1);
    font-weight: 600;
  }
  .explanation {
    margin: 0;
    color: var(--muted-foreground);
  }
  .citations {
    margin-block-start: var(--space-3);
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
    align-items: baseline;
  }
  .citations-label {
    color: var(--muted-foreground);
    font-size: var(--font-size-caption);
  }
  .citation-chip {
    border: 1px solid var(--border);
    background: transparent;
    border-radius: 999px;
    padding: 2px var(--space-2);
    font: inherit;
    font-size: var(--font-size-caption);
    cursor: pointer;
    display: inline-flex;
    gap: var(--space-1);
    align-items: baseline;
  }
  .chip-surface {
    color: var(--muted-foreground);
  }
</style>
