<script lang="ts">
  /**
   * Phase 21 — Timeline Intelligence panel (Activity route).
   *
   * Apple interaction contract, same as every feature page:
   * - Press feedback fires on pointerdown via `fluidPress`, never on release.
   * - All motion is spring-driven and interruptible through the shared design system.
   * - Reduced-motion / reduced-transparency are honored by the shared preference store
   *   and CSS custom properties; no bespoke media queries here.
   * - Bidi: timestamps, digests and counts are isolated with TechnicalText so Arabic
   *   layout never mirrors technical values.
   */
  import { fluidPress } from '../../design/motion';
  import { shellState } from '../../app/shell-state';
  import { streamState } from '../../platform/stream-state';
  import { Pressable, TechnicalText } from '../../design/primitives';
  import { t, td } from '../../lib/i18n';
  import type { MessageKey } from '../../lib/i18n';
  import {
    loadRecurrencePatterns,
    loadTimeline,
  } from './controller';

  $: locale = $shellState.locale;
  $: page = $streamState.timelinePage;

  function outcomeKey(outcome: string): MessageKey {
    switch (outcome) {
      case 'TIMELINE_OUTCOME_SUCCEEDED': return 'timeline.outcome.succeeded';
      case 'TIMELINE_OUTCOME_FAILED': return 'timeline.outcome.failed';
      default: return 'timeline.outcome.neutral';
    }
  }

  function classKey(value: string): MessageKey {
    switch (value) {
      case 'TIMELINE_EVENT_CLASS_OPERATION': return 'timeline.class.operation';
      case 'TIMELINE_EVENT_CLASS_FINDING': return 'timeline.class.finding';
      case 'TIMELINE_EVENT_CLASS_VERIFICATION': return 'timeline.class.verification';
      case 'TIMELINE_EVENT_CLASS_RECOVERY': return 'timeline.class.recovery';
      case 'TIMELINE_EVENT_CLASS_ESCALATION': return 'timeline.class.escalation';
      default: return 'common.unknown';
    }
  }

  function confidenceKey(value: string): MessageKey {
    switch (value) {
      case 'RECURRENCE_CONFIDENCE_WEAK': return 'timeline.confidence.weak';
      case 'RECURRENCE_CONFIDENCE_MODERATE': return 'timeline.confidence.moderate';
      case 'RECURRENCE_CONFIDENCE_STRONG': return 'timeline.confidence.strong';
      default: return 'common.unknown';
    }
  }

  function formatTime(unixMs: number): string {
    if (!unixMs) return '—';
    try {
      return new Intl.DateTimeFormat(locale === 'ar' ? 'ar' : 'en', {
        dateStyle: 'medium',
        timeStyle: 'short',
        numberingSystem: 'latn',
      }).format(new Date(unixMs));
    } catch {
      return String(unixMs);
    }
  }
</script>

<section class="panel timeline-panel" aria-live="polite">
  <div class="panel-head">
    <div>
      <p class="eyebrow">{t('timeline.eyebrow', locale)}</p>
      <h3>{t('timeline.title', locale)}</h3>
    </div>
    <Pressable on:press={loadTimeline}>
      <button class="ghost-action" type="button">{t('timeline.refresh', locale)}</button>
    </Pressable>
  </div>

  {#if page && page.entries.length > 0}
    <ol class="timeline-list">
      {#each page.entries as entry (entry.sourceId)}
        <li class="timeline-row" class:failed={entry.outcome === 'TIMELINE_OUTCOME_FAILED'}>
          <span class="when"><TechnicalText value={formatTime(entry.observedUnixMs)} /></span>
          <span class="what">
            {td(classKey(entry.class), locale)}
            <TechnicalText value={entry.code} />
          </span>
          <span class="state">{td(outcomeKey(entry.outcome), locale)}</span>
        </li>
      {/each}
    </ol>
    <p class="digest">
      {t('timeline.digest', locale)}
      <TechnicalText value={page.digestSha256.slice(0, 16)} />
    </p>
  {:else}
    <p class="empty">{t('timeline.empty', locale)}</p>
  {/if}
</section>

<section class="panel recurrence-panel" aria-live="polite">
  <div class="panel-head">
    <div>
      <p class="eyebrow">{t('timeline.recurrenceEyebrow', locale)}</p>
      <h3>{t('timeline.recurrenceTitle', locale)}</h3>
    </div>
    <Pressable on:press={() => loadRecurrencePatterns()}>
      <button class="ghost-action" type="button">{t('timeline.analyze', locale)}</button>
    </Pressable>
  </div>
  {#if $streamState.timelinePage === null}
    <p class="empty">{t('timeline.empty', locale)}</p>
  {/if}
  <div class="recurrence-note">
    <p>{t('timeline.correlationNote', locale)}</p>
  </div>
</section>

<style>
  .timeline-panel,
  .recurrence-panel {
    margin-block-end: var(--ac-space-5);
  }
  .timeline-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: var(--ac-space-2);
  }
  .timeline-row {
    display: flex;
    align-items: baseline;
    gap: var(--ac-space-4);
    padding: var(--ac-space-3) var(--ac-space-4);
    border-radius: var(--ac-radius-md);
    background: var(--ac-material-base);
  }
  .timeline-row.failed {
    background: var(--role-attention-wash);
  }
  .when {
    color: var(--ac-text-3);
    font-size: var(--ac-type-caption);
  }
  .what {
    display: inline-flex;
    align-items: center;
    gap: var(--ac-space-2);
    min-inline-size: 0;
  }
  .state {
    margin-inline-start: auto;
    color: var(--ac-text-3);
    font-size: var(--ac-type-caption);
  }
  .digest {
    margin-block-start: var(--ac-space-3);
    color: var(--ac-text-3);
    font-size: var(--ac-type-caption);
  }
  .empty {
    color: var(--ac-text-3);
    padding: var(--ac-space-4);
  }
  .ghost-action {
    border: none;
    background: transparent;
    color: var(--ac-accent);
    cursor: pointer;
    font: inherit;
    padding: var(--ac-space-1) var(--ac-space-2);
    border-radius: var(--ac-radius-sm);
  }
  .recurrence-note p {
    color: var(--ac-text-3);
    font-size: var(--ac-type-caption);
    padding: var(--ac-space-3) var(--ac-space-4);
  }
</style>
