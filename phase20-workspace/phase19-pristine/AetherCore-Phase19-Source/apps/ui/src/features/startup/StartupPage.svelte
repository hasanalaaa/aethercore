<script lang="ts">
  import { fluidPress } from '../../design/motion';
  import { shellState } from '../../app/shell-state';
  import { streamState } from '../../platform/stream-state';
  import { LocalizedOwnedText, Pressable, ProgressBar, TechnicalText } from '../../design/primitives';
  import { localizeConfidence, localizeDirection, localizeImpact, localizeKind, localizePublisher, localizeState, localizeStartupScope, t, tp } from '../../lib/i18n';
  import { openStartupRestore, openStartupReview, reviewStartupPlan, selectedStartupDisables, selectedStartupServices, setStartupDecision, setStartupServiceConfirmed, startStartupScan, startupActive, startupUi } from './controller';
  import { formatWhen, shortDigest, stageTone } from '../shared';
  import type { StartupDecision, StartupItem } from '../../lib/contracts';

  $: snapshot = $streamState.snapshot;
  $: startupSnapshot = $streamState.startupSnapshot;
  $: startupPlan = $streamState.startupPlan;
  $: startupStatus = $streamState.startupStatus;
  $: startupHistory = $streamState.startupHistory;
  $: startupDecisions = $startupUi.decisions;
  $: startupServiceConfirmed = $startupUi.serviceConfirmed;
  $: busy = $shellState.busy;
  $: locale = $shellState.locale;

  const decisionOrder: readonly StartupDecision[] = ['Unreviewed', 'KeepEnabled', 'Disable'];

  function decisionFor(item: StartupItem): StartupDecision {
    return startupDecisions[item.itemId] ?? 'Unreviewed';
  }

  function availableDecisions(item: StartupItem): StartupDecision[] {
    return decisionOrder.filter((decision) => decision !== 'Disable' || (item.manageable && !item.protected));
  }

  function decisionKeydown(event: KeyboardEvent, item: StartupItem): void {
    const horizontalNext = locale === 'ar' ? 'ArrowLeft' : 'ArrowRight';
    const horizontalPrevious = locale === 'ar' ? 'ArrowRight' : 'ArrowLeft';
    const forward = event.key === 'ArrowDown' || event.key === horizontalNext;
    const backward = event.key === 'ArrowUp' || event.key === horizontalPrevious;
    if (!forward && !backward && event.key !== 'Home' && event.key !== 'End') return;
    event.preventDefault();

    const choices = availableDecisions(item);
    if (!choices.length) return;
    const current = Math.max(0, choices.indexOf(decisionFor(item)));
    let nextIndex = current;
    if (event.key === 'Home') nextIndex = 0;
    else if (event.key === 'End') nextIndex = choices.length - 1;
    else if (forward) nextIndex = (current + 1) % choices.length;
    else if (backward) nextIndex = (current - 1 + choices.length) % choices.length;

    const next = choices[nextIndex];
    setStartupDecision(item, next);
    const group = (event.currentTarget as HTMLElement).parentElement;
    requestAnimationFrame(() => group?.querySelector<HTMLElement>(`[data-startup-decision="${next}"]`)?.focus({ preventScroll: true }));
  }
</script>

<header>
  <div><p class="eyebrow">{t('startup.actualEyebrow',locale)}</p><h1>{t('startup.actualTitle',locale)}</h1><p class="sub">{t('startup.actualSubtitle',locale)}</p></div>
  <div class="header-actions"><div class="service-pill"><span class:online={snapshot.connected}></span>{snapshot.connected ? t('common.engineOnline',locale,{version:snapshot.serviceVersion}) : t('common.engineOffline',locale)}</div><Pressable className="scan-button" onclick={startStartupScan} disabled={busy || startupSnapshot.state === 'Scanning' || startupActive() || !snapshot.connected}><span>↻</span>{startupSnapshot.state === 'Scanning' ? t('startup.inspecting',locale) : startupSnapshot.state === 'Idle' ? t('startup.inspect',locale) : t('startup.inspectAgain',locale)}</Pressable></div>
</header>

<section class="cleanup-summary startup-summary">
  <article><span>{t('startup.targets',locale)}</span><strong>{startupSnapshot.state === 'Ready' ? startupSnapshot.summary.total : '—'}</strong><small>{t('startup.targetsHint',locale)}</small></article>
  <article><span>{t('startup.manageable',locale)}</span><strong>{startupSnapshot.state === 'Ready' ? startupSnapshot.summary.manageable : '—'}</strong><small>{t('startup.manageableHint',locale)}</small></article>
  <article><span>{t('startup.protected',locale)}</span><strong>{startupSnapshot.state === 'Ready' ? startupSnapshot.summary.protected : '—'}</strong><small>{t('startup.protectedHint',locale)}</small></article>
  <article><span>{t('startup.disableReviewed',locale)}</span><strong>{selectedStartupDisables().length}</strong><small>{t('startup.disableReviewedHint',locale)}</small></article>
</section>
{#if startupSnapshot.state === 'Scanning'}<div class="indeterminate cleanup-scan-progress"><span></span></div>{/if}
{#if startupSnapshot.errorMessage}<div class="error-banner"><strong>{t('startup.inventoryFailed',locale)}</strong><LocalizedOwnedText value={startupSnapshot.errorMessage} {locale}/></div>{/if}
{#if startupSnapshot.warnings.length}<section class="warning-strip"><span>◇</span><div><strong>{t('startup.inventoryNote',locale)}</strong>{#each startupSnapshot.warnings as warning}<LocalizedOwnedText value={warning} {locale} as="p"/>{/each}</div></section>{/if}

{#if startupSnapshot.state === 'Ready'}
  <section class="startup-list">
    {#each startupSnapshot.items as item (item.itemId)}
      <article class:protected={item.protected} class="startup-card">
        <div class="startup-main">
          <div class="startup-title"><strong><TechnicalText value={item.displayName}/></strong><span>{localizeKind(item.kind,locale)}</span>{#if item.protected}<em>{t('startup.protected',locale)}</em>{/if}</div>
          <p><LocalizedOwnedText value={item.command || item.source} {locale}/></p>
          <small>{localizeStartupScope(item.scope,locale)} · {localizePublisher(item.publisher,locale)} · <LocalizedOwnedText value={item.evidenceDetail} {locale}/></small>
          {#if item.protectionReason}<div class="startup-protection"><LocalizedOwnedText value={item.protectionReason} {locale}/></div>{/if}
        </div>
        <div class="startup-evidence"><span>{t('startup.impact',locale)}</span><strong>{localizeImpact(item.impact,locale)}</strong><small>{localizeConfidence(item.confidence,locale)}</small></div>
        <div class="decision-group" role="radiogroup" aria-label={t('startup.decisionFor',locale,{name:item.displayName})}>
          <button use:fluidPress={{ pressedScale:0.985 }} role="radio" data-startup-decision="Unreviewed" class:chosen={decisionFor(item) === 'Unreviewed'} aria-checked={decisionFor(item) === 'Unreviewed'} tabindex={decisionFor(item) === 'Unreviewed' ? 0 : -1} onkeydown={(event) => decisionKeydown(event,item)} onclick={() => setStartupDecision(item,'Unreviewed')}>{t('common.unreviewed',locale)}</button>
          <button use:fluidPress={{ pressedScale:0.985 }} role="radio" data-startup-decision="KeepEnabled" class:chosen={decisionFor(item) === 'KeepEnabled'} aria-checked={decisionFor(item) === 'KeepEnabled'} tabindex={decisionFor(item) === 'KeepEnabled' ? 0 : -1} onkeydown={(event) => decisionKeydown(event,item)} onclick={() => setStartupDecision(item,'KeepEnabled')}>{t('common.keep',locale)}</button>
          <button use:fluidPress={{ pressedScale:0.985 }} role="radio" data-startup-decision="Disable" class:chosen={decisionFor(item) === 'Disable'} aria-checked={decisionFor(item) === 'Disable'} tabindex={decisionFor(item) === 'Disable' ? 0 : -1} onkeydown={(event) => decisionKeydown(event,item)} class="disable-choice" disabled={!item.manageable || item.protected} onclick={() => setStartupDecision(item,'Disable')}>{t('common.disable',locale)}</button>
        </div>
      </article>
    {/each}
  </section>
  {#if selectedStartupServices().length > 0}
    <label use:fluidPress={{ pressedScale:0.992 }} class="service-confirm"><input type="checkbox" checked={startupServiceConfirmed} onchange={(event) => setStartupServiceConfirmed((event.currentTarget as HTMLInputElement).checked)}/><span></span><div><strong>{t('startup.serviceConfirmTitle',locale)}</strong><p>{t('startup.serviceConfirmCopy',locale,{targets:tp('unit.serviceTarget',locale,selectedStartupServices().length)})}</p></div></label>
  {/if}
  <section class="selection-tray"><div><span class="selection-count">{selectedStartupDisables().length}</span><div><strong>{t('startup.selectionTitle',locale)}</strong><p>{t('startup.selectionCopy',locale)}</p></div></div><button use:fluidPress={{ pressedScale:0.985 }} class="install-button" onclick={reviewStartupPlan} disabled={busy || !selectedStartupDisables().length || (selectedStartupServices().length > 0 && !startupServiceConfirmed)}>{t('startup.review',locale)}</button></section>
{:else if startupSnapshot.state === 'Idle'}
  <section class="panel activity-empty"><div class="empty"><div class="empty-icon">↗</div><h4>{t('startup.emptyTitle',locale)}</h4><p>{t('startup.emptyCopy',locale)}</p><button use:fluidPress={{ pressedScale:0.985 }} class="primary" onclick={startStartupScan} disabled={busy || !snapshot.connected}>{t('startup.inspect',locale)}</button></div></section>
{/if}

{#if startupPlan && !startupStatus && startupPlan.state === 'AwaitingAuthorization'}
  <section class="pending-install-card"><div><p class="eyebrow">{t('startup.immutablePlan',locale)}</p><h3>{t('startup.waiting',locale)}</h3><p>{t('startup.planSummary',locale,{actions:tp('unit.action',locale,startupPlan.actionCount),digest:shortDigest(startupPlan.digest)})}</p></div><button use:fluidPress={{ pressedScale:0.985 }} class="primary" onclick={openStartupReview} disabled={busy}>{t('common.reviewAuthorize',locale)}</button></section>
{/if}

{#if startupStatus}
  <section class:terminal={['Completed','Failed'].includes(startupStatus.planState)} class="install-progress-card">
    <div class="install-progress-head"><div><p class="eyebrow">{t('startup.execution',locale)}</p><h3>{localizeState(startupStatus.stage,locale)}</h3>{#if startupStatus.detail}<LocalizedOwnedText value={startupStatus.detail} {locale} as="p"/>{/if}</div><span class:bad={stageTone(startupStatus.stage)==='bad'} class="execution-state">{localizeState(startupStatus.planState,locale)}</span></div>
    {#if startupStatus.progressKnown}<ProgressBar value={startupStatus.overallPercent} label={t('startup.progressLabel',locale)}/>{:else if startupActive()}<ProgressBar value={0} known={false} label={t('startup.progressUnknown',locale)}/>{/if}
    <div class="protection-grid"><div class:ready={!startupStatus.recoveryRequired}><span>{t('common.journal',locale)}</span><strong>{startupStatus.mutationStarted ? t('startup.originalPersisted',locale) : t('startup.preflight',locale)}</strong></div><div class:warn={startupStatus.recoveryRequired}><span>{t('common.recovery',locale)}</span><strong>{startupStatus.recoveryRequired ? t('startup.reviewRequired',locale) : t('startup.noReplay',locale)}</strong></div><div><span>{t('common.progress',locale)}</span><strong>{startupStatus.overallPercent}%</strong></div><div><span>{t('common.plan',locale)}</span><strong>{localizeState(startupStatus.planState,locale)}</strong></div></div>
    {#if startupStatus.failureMessage}<div class="install-failure"><strong>{t('startup.failure',locale)}</strong><LocalizedOwnedText value={startupStatus.failureMessage} {locale} as="p"/></div>{/if}
    {#if startupStatus.items.length}<div class="install-items">{#each startupStatus.items as item (item.itemId)}<div class="install-item"><div><strong><TechnicalText value={item.displayName}/></strong>{#if item.detail}<LocalizedOwnedText value={item.detail} {locale} as="p"/>{/if}</div><span>{localizeState(item.stage,locale)}</span><div><strong>{#if item.resultCode}<TechnicalText value={item.resultCode}/>{:else}—{/if}</strong></div></div>{/each}</div>{/if}
  </section>
{/if}

<section class="recovery-panel startup-history">
  <div class="panel-head"><div><p class="eyebrow">{t('startup.historyEyebrow',locale)}</p><h3>{t('startup.historyTitle',locale)}</h3></div></div>
  {#if startupHistory.length === 0}<div class="empty compact"><p>{t('startup.historyEmpty',locale)}</p></div>{/if}
  {#each startupHistory as entry (entry.changeId)}
    <div class="startup-history-row"><div><strong><TechnicalText value={entry.displayName}/></strong><p>{localizeDirection(entry.direction,locale)} · {localizeKind(entry.kind,locale)} · {localizeState(entry.state,locale)}</p>{#if entry.detail}<LocalizedOwnedText value={entry.detail} {locale} as="small"/>{/if}<small>{formatWhen(entry.updatedUnixMs,locale)} · {t('startup.changeRef',locale,{id:entry.changeId.slice(0,8)})}</small></div>{#if entry.restorable}<button use:fluidPress={{ pressedScale:0.985 }} class="secondary" onclick={() => openStartupRestore(entry)} disabled={busy}>{t('startup.restoreOriginal',locale)}</button>{/if}</div>
  {/each}
</section>

<section class="safety-note"><span>◇</span><div><strong>{t('startup.safetyTitle',locale)}</strong><p>{t('startup.safetyCopy',locale)}</p></div></section>
