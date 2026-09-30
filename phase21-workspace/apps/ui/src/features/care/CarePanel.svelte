<script lang="ts">
  /**
   * Phase 22 — One-Click Care section (Dashboard route).
   *
   * Apple interaction contract, same as every feature page:
   * - Press feedback fires on pointerdown via `fluidPress`, never on release.
   * - All motion is spring-driven and interruptible through the shared design system.
   * - Reduced-motion / reduced-transparency are honored by the shared preference
   *   store and CSS custom properties; no bespoke media queries here.
   * - Bidi: plan digests, counts and technical codes are isolated with TechnicalText.
   *
   * Safety contract: the run only starts after the explicit consent dialog; the
   * report is truth-first — every step cites its own domain verification outcome,
   * and the summary never claims more than the evidence shows.
   */
  import { onMount } from 'svelte';
  import { fluidPress } from '../../design/motion';
  import { shellState } from '../../app/shell-state';
  import { streamState } from '../../platform/stream-state';
  import { Pressable, TechnicalText } from '../../design/primitives';
  import { formatDateTime, localizeOwnedText, localizePlanKind, t, td, hasMessageKey } from '../../lib/i18n';
  import type { MessageKey } from '../../lib/i18n';
  import type { CareStepReport } from '../../lib/contracts';
  import { EmptyState } from '../../design/signature';
  import {
    cancelCare,
    loadCareStatus,
    careLoad,
    careUi,
    prepareCare,
  } from './controller';
  import { careEmptyReason } from './approval';

  $: locale = $shellState.locale;
  $: care = $streamState.careStatus;
  $: emptyReason = careEmptyReason($streamState.cleanupSnapshot, care ? 'loaded' : $careLoad);
  $: optInList = emptyReason.kind === 'nothingEligible'
    ? emptyReason.optIn.map((title) => localizeOwnedText(title, locale).text).join(locale === 'ar' ? '، ' : ', ')
    : '';

  // Waiting for the local scan care asked for: continue when the stream says it ended. Event-driven,
  // not polling; a scan still running keeps the page waiting.
  $: if ($careUi.preparing && $streamState.cleanupSnapshot.state !== 'Scanning') void prepareCare();

  // The panel shows the plan as the service composes it; an unloaded plan is not an empty one.
  onMount(() => {
    if (!$streamState.careStatus) void loadCareStatus();
  });

  const AUTO_LEVEL = 0;

  function reasonKey(reason: string): MessageKey | null {
    const key = `care.reason.${reason}`;
    return hasMessageKey(key) ? key : null;
  }

  function stepStateKey(state: string): MessageKey {
    switch (state) {
      case 'Executing': return 'care.step.executing';
      case 'Completed': return 'care.step.completed';
      case 'Failed': return 'care.step.failed';
      case 'Skipped': return 'care.step.skipped';
      default: return 'care.step.pending';
    }
  }

  function outcomeLabel(step: CareStepReport): string {
    switch (step.outcome) {
      case 'VerifiedByDomain': return td('care.outcome.verifiedByDomain', locale);
      case 'CompletedUnverified': return td('care.outcome.unverified', locale);
      case 'Failed': return td('care.outcome.failed', locale);
      case 'Skipped': return td('care.step.skipped', locale);
      default: return td('care.step.pending', locale);
    }
  }

  function safetyLabel(level: number): string {
    if (level <= AUTO_LEVEL) return td('care.safety.auto', locale);
    return td('care.safety.review', locale);
  }
</script>

<section class="panel care-panel" aria-live="polite">
  <div class="panel-head">
    <div>
      <p class="eyebrow">{t('care.eyebrow', locale)}</p>
      <h3>{t('care.title', locale)}</h3>
    </div>
    {#if !care || care.state === 'Idle' || care.state === 'AwaitingConsent' || care.state === 'Cancelled'}
      <Pressable className="primary-action" onclick={prepareCare}>{t('care.start', locale)}</Pressable>
    {:else if care.state === 'Running'}
      <Pressable className="ghost-action" onclick={cancelCare}>{t('care.cancel', locale)}</Pressable>
    {:else}
      <Pressable className="ghost-action" onclick={loadCareStatus}>{t('care.refresh', locale)}</Pressable>
    {/if}
  </div>

  {#each $careUi.domains as domain (domain.domain)}
    {#if reasonKey(domain.reason)}
      <p class="summary">
        {td(reasonKey(domain.reason) ?? 'care.empty', locale)}
        {#if domain.scannedUnixMs > 0}<span>{t('care.scannedAt', locale, { time: formatDateTime(domain.scannedUnixMs, locale) })}</span>{/if}
      </p>
    {/if}
  {/each}

  {#if !care || care.steps.length === 0}
    {#if emptyReason.kind === 'unavailable'}
      <EmptyState title={t('care.unavailable', locale)} body={t('care.unavailable.body', locale)} />
    {:else if emptyReason.kind === 'nothingEligible'}
      <EmptyState title={t('common.notCollected', locale)} body={t('care.empty.nothingEligible', locale)} />
      {#if optInList}<p class="summary">{t('care.empty.optIn', locale, { list: optInList })}</p>{/if}
    {:else if emptyReason.kind === 'noPlan'}
      <EmptyState title={t('common.notCollected', locale)} body={t('care.empty.noPlan', locale)} />
    {:else}
      <EmptyState title={t('common.notCollected', locale)} body={t('care.empty', locale)} />
    {/if}
  {:else}
    {#if care.planDigestSha256}
      <p class="digest">
        {t('care.planDigest', locale)}
        <TechnicalText value={care.planDigestSha256.slice(0, 16)} />
      </p>
    {/if}
    <ol class="care-list">
      {#each care.steps as step (step.stepIndex)}
        <li class="care-row" class:failed={step.outcome === 'Failed'}>
          <span class="kind">{localizePlanKind(step.domainKind, locale)}</span>
          <span class="safety" class:auto={step.safetyLevel <= AUTO_LEVEL}>{safetyLabel(step.safetyLevel)}</span>
          <span class="outcome">{outcomeLabel(step)}</span>
          {#if step.failureMessageKey && hasMessageKey(step.failureMessageKey)}
            <span class="failure">{td(step.failureMessageKey, locale)}</span>
          {/if}
        </li>
      {/each}
    </ol>
    {#if care.summaryKey && hasMessageKey(care.summaryKey)}
      <p class="summary" class:verified={care.summaryKey === 'care.summary.completedVerified'}>
        {td(care.summaryKey, locale)}
      </p>
    {/if}
  {/if}
</section>

<style>
  .care-panel {
    margin-block-end: var(--ac-space-5);
  }
  .digest {
    margin-block: var(--ac-space-2) var(--ac-space-3);
    color: var(--ac-text-3);
    font-size: var(--ac-type-caption);
  }
  .care-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: var(--ac-space-2);
  }
  .care-row {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: var(--ac-space-4);
    padding: var(--ac-space-3) var(--ac-space-4);
    border-radius: var(--ac-radius-md);
    background: var(--ac-material-base);
  }
  .care-row.failed {
    background: var(--role-attention-wash);
  }
  .kind {
    min-inline-size: 0;
  }
  .safety {
    color: var(--ac-text-3);
    font-size: var(--ac-type-caption);
  }
  .safety.auto {
    color: var(--ac-accent);
  }
  .outcome {
    margin-inline-start: auto;
    color: var(--ac-text-3);
    font-size: var(--ac-type-caption);
  }
  .failure {
    flex-basis: 100%;
    color: var(--ac-text-2);
    font-size: var(--ac-type-caption);
  }
  .summary {
    margin-block-start: var(--ac-space-3);
    font-weight: 600;
  }
  .summary.verified {
    color: var(--ac-accent);
  }
</style>
