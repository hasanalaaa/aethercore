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
  import { formatDateTime, t, td } from '../../lib/i18n';
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

  // P57 ITEM 2. This screen had its own formatter with `numberingSystem: 'latn'`
  // hard-coded — one screen that had already picked Latin digits while the rest
  // of the product rendered Arabic-Indic ones. That divergence IS the finding
  // ITEM 2 settles, so the local copy is gone and the shared formatter, which
  // now makes the same choice product-wide, is the only one.
  const formatTime = (unixMs: number): string => formatDateTime(unixMs, locale);
</script>

<section class="panel timeline-panel" aria-live="polite">
  <div class="panel-head">
    <div>
      <p class="eyebrow">{t('timeline.eyebrow', locale)}</p>
      <h3>{t('timeline.title', locale)}</h3>
    </div>
    <Pressable className="ghost-action" onclick={loadTimeline}>{t('timeline.refresh', locale)}</Pressable>
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
    <Pressable className="ghost-action" onclick={() => loadRecurrencePatterns()}>{t('timeline.analyze', locale)}</Pressable>
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
  .recurrence-note p {
    color: var(--ac-text-3);
    font-size: var(--ac-type-caption);
    padding: var(--ac-space-3) var(--ac-space-4);
  }
</style>
