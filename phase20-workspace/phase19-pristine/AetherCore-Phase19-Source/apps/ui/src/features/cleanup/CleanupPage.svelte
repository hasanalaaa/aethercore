<script lang="ts">
  import { fluidPress } from '../../design/motion';
  import { shellState } from '../../app/shell-state';
  import { streamState } from '../../platform/stream-state';
  import { LocalizedOwnedText, Pressable, ProgressBar, TechnicalText } from '../../design/primitives';
  import { localizeOwnedText, localizeState, t, tp } from '../../lib/i18n';
  import { cleanupActive, cleanupUi, openCleanupReview, reviewCleanup, selectedCleanupBytes, selectedCleanupCandidates, startCleanupScan, toggleCleanup } from './controller';
  import { formatBytes, shortDigest, stageTone } from '../shared';

  $: snapshot = $streamState.snapshot;
  $: cleanupSnapshot = $streamState.cleanupSnapshot;
  $: cleanupPlan = $streamState.cleanupPlan;
  $: cleanupStatus = $streamState.cleanupStatus;
  $: cleanupSelected = $cleanupUi.selected;
  $: busy = $shellState.busy;
  $: locale = $shellState.locale;
</script>

<header>
  <div><p class="eyebrow">{t('cleanup.eyebrow', locale)}</p><h1>{t('cleanup.title', locale)}</h1><p class="sub">{t('cleanup.subtitle', locale)}</p></div>
  <div class="header-actions">
    <div class="service-pill"><span class:online={snapshot.connected}></span>{snapshot.connected ? t('common.engineOnline', locale, {version:snapshot.serviceVersion}) : t('common.engineOffline', locale)}</div>
    <Pressable className="scan-button" onclick={startCleanupScan} disabled={busy || cleanupSnapshot.state === 'Scanning' || cleanupActive() || !snapshot.connected}><span>✦</span>{cleanupSnapshot.state === 'Scanning' ? t('cleanup.scanning',locale) : cleanupSnapshot.state === 'Idle' ? t('cleanup.scan',locale) : t('cleanup.scanAgain',locale)}</Pressable>
  </div>
</header>

<section class="cleanup-summary">
  <article><span>{t('cleanup.reclaimable',locale)}</span><strong>{cleanupSnapshot.state === 'Ready' ? formatBytes(cleanupSnapshot.totalReclaimableBytes,locale) : '—'}</strong><small>{t('cleanup.reclaimableHint',locale)}</small></article>
  <article><span>{t('cleanup.files',locale)}</span><strong>{cleanupSnapshot.state === 'Ready' ? cleanupSnapshot.totalFileCount : '—'}</strong><small>{t('cleanup.filesHint',locale)}</small></article>
  <article><span>{t('cleanup.categories',locale)}</span><strong>{cleanupSnapshot.state === 'Ready' ? cleanupSnapshot.candidates.length : '—'}</strong><small>{t('cleanup.categoriesHint',locale)}</small></article>
  <article><span>{t('cleanup.selected',locale)}</span><strong>{selectedCleanupCandidates().length}</strong><small>{formatBytes(selectedCleanupBytes(),locale)}</small></article>
</section>
{#if cleanupSnapshot.state === 'Scanning'}<div class="indeterminate cleanup-scan-progress"><span></span></div>{/if}
{#if cleanupSnapshot.errorMessage}<div class="error-banner"><strong>{t('cleanup.scanFailed',locale)}</strong><LocalizedOwnedText value={cleanupSnapshot.errorMessage} {locale}/></div>{/if}
{#if cleanupSnapshot.warnings.length}<section class="warning-strip"><span>◇</span><div><strong>{t('cleanup.note',locale)}</strong>{#each cleanupSnapshot.warnings as warning}<LocalizedOwnedText value={warning} {locale} as="p"/>{/each}</div></section>{/if}

{#if cleanupSnapshot.state === 'Ready'}
  <section class="cleanup-list">
    {#each cleanupSnapshot.candidates as candidate (candidate.candidateId)}
      <label class:explicit={candidate.requiresExplicitConfirmation} class="cleanup-card">
        <input type="checkbox" checked={!!cleanupSelected[candidate.candidateId]} onchange={(e) => toggleCleanup(candidate.candidateId, (e.currentTarget as HTMLInputElement).checked)}/><span class="cleanup-check"></span>
        <div class="cleanup-copy">
          <div><LocalizedOwnedText value={candidate.title} {locale} as="strong"/>{#if candidate.requiresExplicitConfirmation}<em>{t('cleanup.explicitOptIn',locale)}</em>{/if}{#if candidate.truncated}<em class="warn-tag">{t('cleanup.scanCapped',locale)}</em>{/if}</div>
          <LocalizedOwnedText value={candidate.description} {locale} as="p"/>
          <small><TechnicalText value={candidate.provider}/> · {tp('unit.file',locale,candidate.fileCount)}{#if candidate.specialKind} · <TechnicalText value={candidate.specialKind}/>{/if}</small>
        </div>
        <div class="cleanup-size"><strong>{formatBytes(candidate.reclaimableBytes,locale)}</strong><small>{candidate.selectedByDefault ? t('cleanup.safeDefault',locale) : t('cleanup.notDefault',locale)}</small></div>
      </label>
    {/each}
  </section>
  <section class="selection-tray cleanup-selection"><div><span class="selection-count">{selectedCleanupCandidates().length}</span><div><strong>{t('cleanup.selectionTitle',locale)}</strong><p>{t('cleanup.selectionCopy',locale,{bytes:formatBytes(selectedCleanupBytes(),locale)})}</p></div></div><button use:fluidPress={{ pressedScale:0.985 }} class="install-button" onclick={reviewCleanup} disabled={busy || !selectedCleanupCandidates().length}>{t('cleanup.review',locale)}</button></section>
{:else if cleanupSnapshot.state === 'Idle'}
  <section class="panel activity-empty"><div class="empty"><div class="empty-icon">✦</div><h4>{t('cleanup.emptyTitle',locale)}</h4><p>{t('cleanup.emptyCopy',locale)}</p><button use:fluidPress={{ pressedScale:0.985 }} class="primary" onclick={startCleanupScan} disabled={busy || !snapshot.connected}>{t('cleanup.scan',locale)}</button></div></section>
{/if}

{#if cleanupPlan && !cleanupStatus && cleanupPlan.state === 'AwaitingAuthorization'}
  <section class="pending-install-card"><div><p class="eyebrow">{t('cleanup.immutablePlan',locale)}</p><h3>{t('cleanup.waiting',locale)}</h3><p>{t('cleanup.planSummary',locale,{categories:tp('unit.category',locale,cleanupPlan.actionCount),digest:shortDigest(cleanupPlan.digest)})}</p></div><button use:fluidPress={{ pressedScale:0.985 }} class="primary" onclick={openCleanupReview} disabled={busy}>{t('common.reviewAuthorize',locale)}</button></section>
{/if}

{#if cleanupStatus}
  <section class:terminal={['Completed','Failed'].includes(cleanupStatus.planState)} class="install-progress-card">
    <div class="install-progress-head"><div><p class="eyebrow">{t('cleanup.execution',locale)}</p><h3>{localizeState(cleanupStatus.stage,locale)}</h3>{#if cleanupStatus.detail}<LocalizedOwnedText value={cleanupStatus.detail} {locale} as="p"/>{/if}</div><span class:bad={stageTone(cleanupStatus.stage)==='bad'} class="execution-state">{localizeState(cleanupStatus.planState,locale)}</span></div>
    {#if cleanupStatus.progressKnown}<ProgressBar value={cleanupStatus.overallPercent} label={t('cleanup.progressLabel',locale)}/>{:else if cleanupActive()}<ProgressBar value={0} known={false} label={t('cleanup.progressUnknown',locale)}/>{/if}
    <div class="cleanup-result-grid"><div><span>{t('cleanup.reclaimed',locale)}</span><strong>{formatBytes(cleanupStatus.reclaimedBytes,locale)}</strong></div><div><span>{t('cleanup.skippedSafely',locale)}</span><strong>{formatBytes(cleanupStatus.skippedBytes,locale)}</strong></div><div><span>{t('cleanup.mutation',locale)}</span><strong>{cleanupStatus.mutationStarted ? t('cleanup.started',locale) : t('cleanup.notStarted',locale)}</strong></div></div>
    {#if cleanupStatus.failureMessage}<div class="install-failure"><strong>{t('cleanup.failure',locale)}</strong><LocalizedOwnedText value={cleanupStatus.failureMessage} {locale} as="p"/></div>{/if}
    {#if cleanupStatus.items.length}<div class="phase4-step-list">{#each cleanupStatus.items as item (item.itemId)}<div><span class="step-dot"></span><div><LocalizedOwnedText value={item.title} {locale} as="strong"/><LocalizedOwnedText value={item.detail} {locale} as="p"/></div><em><TechnicalText value={item.resultCode}/> · {formatBytes(item.bytesAffected,locale)}</em></div>{/each}</div>{/if}
  </section>
{/if}

<section class="safety-note"><span>✦</span><div><strong>{t('cleanup.safetyTitle',locale)}</strong><p>{t('cleanup.safetyCopy',locale)}</p></div></section>
