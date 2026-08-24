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
  import { fluidPress } from '../../design/motion';
  import { shellState } from '../../app/shell-state';
  import { streamState } from '../../platform/stream-state';
  import { Pressable, TechnicalText } from '../../design/primitives';
  import { t, td, tp, hasMessageKey } from '../../lib/i18n';
  import type { MessageKey } from '../../lib/i18n';
  import type { CareStepReport } from '../../lib/contracts';
  import {
    authorizeAndStartCare,
    cancelCare,
    careUi,
    closeCareConsent,
    loadCareStatus,
    openCareConsent,
  } from './controller';

  $: locale = $shellState.locale;
  $: care = $streamState.careStatus;
  $: dialogOpen = $careUi.consentDialogOpen;

  const AUTO_LEVEL = 0;

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
      <Pressable on:press={openCareConsent}>
        <button class="primary-action" type="button">{t('care.start', locale)}</button>
      </Pressable>
    {:else if care.state === 'Running'}
      <Pressable on:press={cancelCare}>
        <button class="ghost-action" type="button">{t('care.cancel', locale)}</button>
      </Pressable>
    {:else}
      <Pressable on:press={loadCareStatus}>
        <button class="ghost-action" type="button">{t('care.refresh', locale)}</button>
      </Pressable>
    {/if}
  </div>

  {#if !care || care.steps.length === 0}
    <p class="empty">{t('care.empty', locale)}</p>
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
          <span class="kind"><TechnicalText value={step.domainKind} /></span>
          <span class="safety" class:auto={step.safetyLevel <= AUTO_LEVEL}>{safetyLabel(step.safetyLevel)}</span>
          <span class="outcome">{outcomeLabel(step)}</span>
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

{#if dialogOpen}
  <div class="consent-scrim" role="presentation">
    <div class="consent-card" role="dialog" aria-modal="true" aria-labelledby="care-consent-title">
      <h4 id="care-consent-title">{t('care.consentTitle', locale)}</h4>
      <p class="consent-copy">{t('care.consentCopy', locale)}</p>
      <ul class="consent-list">
        <li>{t('care.consentBulletScope', locale)}</li>
        <li>{t('care.consentBulletReview', locale)}</li>
        <li>{t('care.consentBulletSession', locale)}</li>
      </ul>
      <div class="consent-actions">
        <Pressable on:press={closeCareConsent}>
          <button class="ghost-action" type="button">{t('common.cancel', locale)}</button>
        </Pressable>
        <Pressable on:press={authorizeAndStartCare}>
          <button class="primary-action" type="button">{t('care.consentAuthorize', locale)}</button>
        </Pressable>
      </div>
    </div>
  </div>
{/if}

<style>
  .care-panel {
    margin-block-end: var(--space-5);
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
  .ghost-action {
    border: none;
    background: transparent;
    color: var(--accent);
    font: inherit;
    padding: var(--space-1) var(--space-2);
    border-radius: var(--radius-sm);
    cursor: pointer;
  }
  .empty {
    color: var(--muted-foreground);
    padding: var(--space-4);
  }
  .digest {
    margin-block: var(--space-2) var(--space-3);
    color: var(--muted-foreground);
    font-size: var(--font-size-caption);
  }
  .care-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: var(--space-2);
  }
  .care-row {
    display: flex;
    align-items: baseline;
    gap: var(--space-4);
    padding: var(--space-3) var(--space-4);
    border-radius: var(--radius-md);
    background: var(--material-thin);
  }
  .care-row.failed {
    background: var(--material-warning-soft, var(--material-thin));
  }
  .kind {
    min-inline-size: 0;
  }
  .safety {
    color: var(--muted-foreground);
    font-size: var(--font-size-caption);
  }
  .safety.auto {
    color: var(--accent);
  }
  .outcome {
    margin-inline-start: auto;
    color: var(--muted-foreground);
    font-size: var(--font-size-caption);
  }
  .summary {
    margin-block-start: var(--space-3);
    font-weight: 600;
  }
  .summary.verified {
    color: var(--accent);
  }
  .consent-scrim {
    position: fixed;
    inset: 0;
    display: grid;
    place-items: center;
    background: rgb(0 0 0 / 35%);
    z-index: 60;
  }
  .consent-card {
    inline-size: min(30rem, calc(100vw - 3rem));
    padding: var(--space-5);
    border-radius: var(--radius-lg);
    background: var(--card);
    box-shadow: 0 12px 40px rgb(0 0 0 / 25%);
  }
  .consent-card h4 {
    margin: 0 0 var(--space-2);
  }
  .consent-copy {
    color: var(--muted-foreground);
    margin-block: 0 var(--space-3);
  }
  .consent-list {
    margin: 0 0 var(--space-4);
    padding-inline-start: var(--space-5);
    color: var(--muted-foreground);
    font-size: var(--font-size-caption);
  }
  .consent-actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-3);
  }
</style>
