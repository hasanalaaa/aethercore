<script lang="ts">
  import { fluidPress } from '../../design/motion';
  import { shellState } from '../../app/shell-state';
  import { streamState } from '../../platform/stream-state';
  import { LocalizedOwnedText, Pressable, TechnicalText } from '../../design/primitives';
  import { localizeConfidence, localizeDomain, localizeSeverity, localizeState, t, tp } from '../../lib/i18n';
  import { diagnosticRunning, eventTone, startDiagnosticsScan } from './controller';
  import ProviderFaultsPanel from './ProviderFaultsPanel.svelte';
  import { formatBytes, formatWhen } from '../shared';

  $: snapshot = $streamState.snapshot;
  $: diagnostics = $streamState.diagnostics;
  $: diagnosticHistory = $streamState.diagnosticHistory;
  $: busy = $shellState.busy;
  $: locale = $shellState.locale;
  $: windowDays = diagnostics.eventWindowDays || 30;
  $: localizedWindow = tp('unit.day', locale, windowDays);
</script>

<header>
  <div><p class="eyebrow">{t('crash.eyebrow',locale)}</p><h1>{t('crash.title',locale)}</h1><p class="sub">{t('crash.subtitle',locale,{days:localizedWindow})}</p></div>
  <div class="header-actions"><div class="service-pill"><span class:online={snapshot.connected}></span>{snapshot.connected ? t('common.engineOnline',locale,{version:snapshot.serviceVersion}) : t('common.engineOffline',locale)}</div><Pressable className="scan-button" onclick={() => startDiagnosticsScan('crash')} disabled={busy || diagnosticRunning() || !snapshot.connected}><span>↻</span>{diagnosticRunning() ? t('crash.collecting',locale) : t('crash.refresh',locale)}</Pressable></div>
</header>
{#if diagnosticRunning()}<div class="indeterminate cleanup-scan-progress"><span></span></div>{/if}
{#if diagnostics.warnings.length}<section class="warning-strip"><span>◇</span><div><strong>{t('hardware.partial',locale)}</strong>{#each diagnostics.warnings as warning}<LocalizedOwnedText value={warning} {locale} as="p"/>{/each}</div></section>{/if}
<ProviderFaultsPanel faults={diagnostics.providerFaults} {locale}/>
<section class="hardware-summary crash-summary">
  <article><span>{t('crash.minidumps',locale)}</span><strong>{diagnostics.crashes.length}</strong><small>{t('crash.minidumpsHint',locale)}</small></article>
  <article><span>{t('crash.whea',locale)}</span><strong>{diagnostics.events.filter((event) => event.provider === 'Microsoft-Windows-WHEA-Logger').length}</strong><small>{t('crash.wheaHint',locale)}</small></article>
  <article><span>{t('crash.shutdowns',locale)}</span><strong>{diagnostics.events.filter((event) => event.category === 'UnexpectedShutdown').length}</strong><small>{t('crash.shutdownsHint',locale)}</small></article>
  <article><span>{t('crash.snapshots',locale)}</span><strong>{diagnosticHistory.length}</strong><small>{t('crash.snapshotsHint',locale)}</small></article>
</section>
{#if diagnostics.state === 'Idle'}<section class="panel activity-empty"><div class="empty"><div class="empty-icon">⌁</div><h4>{t('crash.emptyTitle',locale)}</h4><p>{t('crash.emptyCopy',locale,{days:localizedWindow})}</p><button use:fluidPress={{ pressedScale:0.985 }} class="primary" onclick={() => startDiagnosticsScan('crash')} disabled={busy || !snapshot.connected}>{t('crash.refresh',locale)}</button></div></section>{/if}

{#if diagnostics.crashes.length}
  <section class="crash-list"><div class="panel-head"><div><p class="eyebrow">{t('crash.dumpsEyebrow',locale)}</p><h3>{t('crash.dumpsTitle',locale)}</h3></div></div>
    {#each diagnostics.crashes as crash (crash.crashId)}
      <article class="crash-card"><div><strong><TechnicalText value={crash.dumpFile}/></strong><LocalizedOwnedText value={crash.summary} {locale} as="p"/><small>{formatWhen(crash.recordedUnixMs,locale)} · {formatBytes(crash.dumpSizeBytes,locale)} · <TechnicalText value={crash.source}/></small></div><div class="bugcheck"><span>{t('crash.bugcheck',locale)}</span>{#if crash.hasBugcheckCode}<TechnicalText value={crash.bugcheckHex} as="code"/>{:else}<strong>{t('crash.notParsed',locale)}</strong>{/if}<small>{localizeConfidence(crash.confidence,locale)}</small></div></article>
    {/each}
  </section>
{/if}

<section class="event-list"><div class="panel-head"><div><p class="eyebrow">{t('crash.eventsEyebrow',locale)}</p><h3>{t('crash.eventsTitle',locale,{days:localizedWindow})}</h3></div></div>
  {#if diagnostics.events.length === 0}<div class="empty compact"><p>{t('crash.eventsEmpty',locale,{days:localizedWindow})}</p></div>{/if}
  {#each diagnostics.events as event (event.provider + event.recordedUnixMs + event.eventId)}
    <article class="event-row"><span class:warn={eventTone(event.severity)==='warn'} class="event-dot"></span><div><LocalizedOwnedText value={event.summary} {locale} as="strong"/><LocalizedOwnedText value={event.detail} {locale} as="p"/><small><TechnicalText value={event.provider}/> · {t('crash.eventRef',locale,{id:event.eventId})} · {formatWhen(event.recordedUnixMs,locale)}</small></div><em>{localizeConfidence(event.confidence,locale)}</em></article>
  {/each}
</section>

<section class="diagnostic-cards crash-triage"><div class="panel-head"><div><p class="eyebrow">{t('crash.triageEyebrow',locale)}</p><h3>{t('crash.triageTitle',locale)}</h3></div></div>
  {#each diagnostics.cards.filter((card) => card.domain === 'Crash' || card.domain === 'Hardware') as card (card.cardId)}
    <article class="triage-card"><div class="triage-head"><div><span>{localizeDomain(card.domain,locale)}</span><h4><LocalizedOwnedText value={card.title} {locale}/></h4></div><em>{localizeSeverity(card.severity,locale)} · {localizeConfidence(card.confidence,locale)}</em></div><LocalizedOwnedText value={card.summary} {locale} as="p"/>{#if card.evidence.length}<ul>{#each card.evidence as evidence}<li><LocalizedOwnedText value={evidence} {locale}/></li>{/each}</ul>{/if}<div class="guided-actions">{#each card.actions as action}<span class="guided-action"><LocalizedOwnedText value={action} {locale}/></span>{/each}</div></article>
  {/each}
</section>

<section class="diagnostic-history-list"><div class="panel-head"><div><p class="eyebrow">{t('crash.historyEyebrow',locale)}</p><h3>{t('crash.historyTitle',locale)}</h3></div></div>
  {#if diagnosticHistory.length === 0}<div class="empty compact"><p>{t('crash.historyEmpty',locale)}</p></div>{/if}
  {#each diagnosticHistory.slice(0,8) as entry (entry.scanId)}
    <article class="diagnostic-history-row"><div><strong>{localizeState(entry.state,locale)}</strong><p>{t('crash.historyCounts',locale,{cards:tp('unit.triageCard',locale,entry.cardCount),warnings:tp('unit.warning',locale,entry.warningCount)})}</p></div><small>{formatWhen(entry.collectedUnixMs,locale)} · {t('crash.scanRef',locale,{id:entry.scanId.slice(0,8)})}</small></article>
  {/each}
</section>
<section class="safety-note"><span>⌁</span><div><strong>{t('crash.safetyTitle',locale)}</strong><p>{t('crash.safetyCopy',locale)}</p></div></section>
