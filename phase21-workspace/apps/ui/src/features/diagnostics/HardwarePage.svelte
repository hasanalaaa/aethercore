<script lang="ts">
  import { fluidPress } from '../../design/motion';
  import { shellState } from '../../app/shell-state';
  import { streamState } from '../../platform/stream-state';
  import { LocalizedOwnedText, Pressable, ProgressBar, TechnicalText } from '../../design/primitives';
  import { formatDateTime, formatNumber, localizeConfidence, localizeDomain, localizeHealthStatus, localizeMemoryPressure, localizeSeverity, t, td, type MessageKey } from '../../lib/i18n';
  import { diagnosticRunning, memoryEvents, startDiagnosticsScan, storageActionCount } from './controller';
  import ProviderFaultsPanel from './ProviderFaultsPanel.svelte';
  import { formatBytes } from '../shared';
  import { EmptyState } from '../../design/signature';
  import { hardwareVerdict } from '../intelligence/headline';

  $: snapshot = $streamState.snapshot;
  $: diagnostics = $streamState.diagnostics;
  $: busy = $shellState.busy;
  $: locale = $shellState.locale;
  $: verdict = hardwareVerdict(diagnostics);
  $: nextKey = nextStep(verdict);
  // Shape and text carry the verdict, colour only repeats them: a critical disk is "!", a denied read is "⊘".
  $: mark = verdict.kind === 'action' ? '!' : verdict.kind === 'attention' ? '▲' : verdict.kind === 'noneFound' ? '✓' : '○';

  function nextStep(v: typeof verdict): MessageKey {
    if (v.kind === 'action') return 'hardware.verdict.next.action';
    if (v.kind === 'attention') return 'hardware.verdict.next.attention';
    return v.kind === 'noneFound' && v.coverage === 'complete' ? 'hardware.verdict.next.clear' : 'hardware.verdict.next.again';
  }

  function metric(hasValue: boolean, value: number, suffix = ''): string {
    return hasValue ? `${formatNumber(value, locale)}${suffix}` : t('common.notReported', locale);
  }
</script>

<header>
  <!-- The title is the screen's name. It was a sentence — "Measurements, not a
       made-up health score." — which argues with a score nobody is showing. -->
  <div><h1>{t('hardware.actualTitle',locale)}</h1></div>
  <div class="header-actions"><div class="service-pill"><span class:online={snapshot.connected}></span>{snapshot.connected ? t('common.engineOnline',locale,{version:snapshot.serviceVersion}) : t('common.engineOffline',locale)}</div><Pressable className="scan-button" onclick={() => startDiagnosticsScan('hardware')} disabled={busy || diagnosticRunning() || !snapshot.connected}><span>↻</span>{diagnosticRunning() ? t('hardware.collecting',locale) : diagnostics.state === 'Idle' ? t('hardware.collect',locale) : t('hardware.collectAgain',locale)}</Pressable></div>
</header>
{#if diagnosticRunning()}<div class="indeterminate cleanup-scan-progress"><span></span></div>{/if}
{#if diagnostics.warnings.length}<section class="warning-strip"><span>◇</span><div><strong>{t('hardware.partial',locale)}</strong>{#each diagnostics.warnings as warning}<LocalizedOwnedText value={warning} {locale} as="p"/>{/each}</div></section>{/if}
<ProviderFaultsPanel faults={diagnostics.providerFaults} {locale}/>
{#if verdict.kind !== 'collecting'}
  <section class="verdict-card" class:verdict-action={verdict.kind === 'action'} class:verdict-attention={verdict.kind === 'attention'} aria-labelledby="hardware-verdict-title">
    <span class="verdict-mark" aria-hidden="true">{mark}</span>
    <div>
      <h2 id="hardware-verdict-title">{td(`hardware.verdict.${verdict.kind}`,locale)}</h2>
      {#if verdict.kind !== 'notCollected'}
        <p>{t('hardware.verdict.measured',locale,{items:verdict.measured.map((key) => td(key,locale)).join(' · ') || '—'})}</p>
        {#if verdict.notMeasured.length}<p>{t('hardware.verdict.notMeasured',locale,{items:verdict.notMeasured.map((key) => td(key,locale)).join(' · ')})}</p>{/if}
        {#if verdict.coverage !== 'complete'}<p>{t('hardware.verdict.partial',locale)}</p>{/if}
        {#if verdict.denied}<p><span aria-hidden="true">⊘ </span>{t('hardware.verdict.denied',locale)}</p>{/if}
        <p>{t('hardware.verdict.when',locale,{time:formatDateTime(diagnostics.completedUnixMs,locale),days:formatNumber(diagnostics.eventWindowDays,locale)})} {t('hardware.verdict.source',locale)}</p>
        <p><strong>{td(nextKey,locale)}</strong></p>
      {/if}
    </div>
  </section>
{/if}
<section class="hardware-summary">
  <article><span>{t('hardware.storageDevices',locale)}</span><strong>{diagnostics.storage.length || '—'}</strong><small>{t('hardware.storageHint',locale)}</small></article>
  <article><span>{t('hardware.actionRequired',locale)}</span><strong>{storageActionCount()}</strong><small>{t('hardware.actionHint',locale)}</small></article>
  <article><span>{t('hardware.memoryLoad',locale)}</span><strong>{diagnostics.memory ? `${formatNumber(diagnostics.memory.memoryLoadPercent,locale)}%` : '—'}</strong><small>{diagnostics.memory ? localizeMemoryPressure(diagnostics.memory.pressureLabel,locale) : t('hardware.memoryMetricHint',locale)}</small></article>
  <article><span>{t('hardware.wheaMemory',locale)}</span><strong>{memoryEvents().length}</strong><small>{t('hardware.wheaHint',locale)}</small></article>
</section>

{#if diagnostics.state === 'Idle'}
  <section class="panel activity-empty"><EmptyState title={t('common.notCollected',locale)} body={t('hardware.emptyCopy',locale)} channels={[{ label: t('hardware.storageDevices',locale) }, { label: t('hardware.memoryLoad',locale) }, { label: t('hardware.wheaMemory',locale) }]}>
    <button use:fluidPress={{ pressedScale:0.985 }} class="primary" onclick={() => startDiagnosticsScan('hardware')} disabled={busy || !snapshot.connected}>{t('hardware.collect',locale)}</button>
  </EmptyState></section>
{:else}
  <section class="storage-grid">
    {#each diagnostics.storage as disk (disk.deviceId)}
      <article class="storage-card" class:attention={disk.severity === 'Attention'} class:critical={disk.severity === 'ActionRequired'}>
        <div class="storage-head"><div><p class="eyebrow"><TechnicalText value={`${disk.busType} · ${disk.mediaType}`}/></p><h3>{#if disk.friendlyName === 'Physical disk' || !disk.friendlyName}{t('tech.card.storagePhysical',locale)} {#if disk.deviceId}<TechnicalText value={disk.deviceId}/>{/if}{:else}<TechnicalText value={disk.friendlyName}/>{/if}</h3><LocalizedOwnedText value={disk.summary} {locale} as="p"/></div><span class="diagnostic-severity">{localizeSeverity(disk.severity,locale)}</span></div>
        <div class="metric-grid">
          <div><span>{t('hardware.windowsStatus',locale)}</span><strong>{localizeHealthStatus(disk.windowsHealthStatus || 'Unknown',locale)}</strong></div>
          <div><span>{t('hardware.temperature',locale)}</span><strong>{metric(!!disk.reliability?.hasTemperature,disk.reliability?.temperatureC ?? 0,' °C')}</strong></div>
          <div><span>{t('hardware.maxTemperature',locale)}</span><strong>{metric(!!disk.reliability?.hasTemperatureMax,disk.reliability?.temperatureMaxC ?? 0,' °C')}</strong></div>
          <div><span>{t('hardware.wear',locale)}</span><strong>{metric(!!disk.reliability?.hasWear,disk.reliability?.wearPercentUsed ?? 0,'%')}</strong></div>
          <div><span>{t('hardware.powerOn',locale)}</span><strong>{metric(!!disk.reliability?.hasPowerOnHours,disk.reliability?.powerOnHours ?? 0)}</strong></div>
          <div><span>{t('hardware.uncorrectedReads',locale)}</span><strong>{metric(!!disk.reliability?.hasReadErrorsUncorrected,disk.reliability?.readErrorsUncorrected ?? 0)}</strong></div>
          <div><span>{t('hardware.uncorrectedWrites',locale)}</span><strong>{metric(!!disk.reliability?.hasWriteErrorsUncorrected,disk.reliability?.writeErrorsUncorrected ?? 0)}</strong></div>
          {#if disk.reliability?.hasNvmeCriticalWarning}<div><span>{t('hardware.nvmeFlags',locale)}</span><TechnicalText value={`0x${disk.reliability.nvmeCriticalWarning.toString(16).padStart(2,'0').toUpperCase()}`} as="code"/></div>{/if}
          {#if disk.reliability?.hasNvmeAvailableSpare}<div><span>{t('hardware.nvmeSpare',locale)}</span><strong>{formatNumber(disk.reliability.nvmeAvailableSparePercent,locale)}%</strong></div>{/if}
          {#if disk.reliability?.hasNvmePercentageUsed}<div><span>{t('hardware.nvmeUsed',locale)}</span><strong>{formatNumber(disk.reliability.nvmePercentageUsed,locale)}%</strong></div>{/if}
          {#if disk.reliability?.nvmeMediaErrors}<div><span>{t('hardware.nvmeMediaErrors',locale)}</span><TechnicalText value={disk.reliability.nvmeMediaErrors}/></div>{/if}
          {#if disk.reliability?.nvmeUnsafeShutdowns}<div><span>{t('hardware.nvmeUnsafeShutdowns',locale)}</span><TechnicalText value={disk.reliability.nvmeUnsafeShutdowns}/></div>{/if}
          {#if disk.reliability?.nvmeErrorLogEntries}<div><span>{t('hardware.nvmeLogEntries',locale)}</span><TechnicalText value={disk.reliability.nvmeErrorLogEntries}/></div>{/if}
          {#if disk.reliability?.hasReadLatencyMax}<div><span>{t('hardware.maxReadLatency',locale)}</span><strong>{formatNumber(disk.reliability.readLatencyMaxMs,locale)} {t('unit.milliseconds.short',locale)}</strong></div>{/if}
          {#if disk.reliability?.hasWriteLatencyMax}<div><span>{t('hardware.maxWriteLatency',locale)}</span><strong>{formatNumber(disk.reliability.writeLatencyMaxMs,locale)} {t('unit.milliseconds.short',locale)}</strong></div>{/if}
          {#if disk.reliability?.hasFlushLatencyMax}<div><span>{t('hardware.maxFlushLatency',locale)}</span><strong>{formatNumber(disk.reliability.flushLatencyMaxMs,locale)} {t('unit.milliseconds.short',locale)}</strong></div>{/if}
        </div>
        <div class="metric-grid technical-facts">
          <div><span>{t('hardware.serial',locale)}</span><TechnicalText value={disk.serialNumber || '—'}/></div>
          <div><span>{t('hardware.firmware',locale)}</span><TechnicalText value={disk.firmwareVersion || '—'}/></div>
          <div><span>{t('hardware.capacity',locale)}</span><strong>{formatBytes(disk.sizeBytes,locale)}</strong></div>
          <div><span>{t('hardware.operational',locale)}</span><TechnicalText value={disk.operationalStatus.join(', ') || '—'}/></div>
        </div>
        {#if disk.ataSmartAttributes?.length}
          <details class="ata-smart-block"><summary>{t('hardware.ataSummary',locale,{count:disk.ataSmartAttributes.length})}</summary><p>{t('hardware.ataCopy',locale)}</p><div class="ata-smart-grid">{#each disk.ataSmartAttributes as attr (attr.id)}<div><span>{t('hardware.attributeId',locale)} <TechnicalText value={String(attr.id)}/></span><strong><TechnicalText value={`${attr.current} / ${attr.worst}`}/></strong><small><span>{t('hardware.rawValue',locale)} <TechnicalText value={`${attr.rawValueHex} · ${attr.rawValueDecimal}`}/></span></small></div>{/each}</div></details>
        {/if}
        {#if disk.reasons.length}<ul class="evidence-list">{#each disk.reasons as reason}<li><LocalizedOwnedText value={reason} {locale}/></li>{/each}</ul>{/if}
        <div class="source-line"><span>{t('hardware.sourceNotes',locale)}:</span>{#if disk.sourceNotes.length}{#each disk.sourceNotes as note}<LocalizedOwnedText value={note} {locale}/>{/each}{:else}<span>{t('hardware.noSource',locale)}</span>{/if}</div>
      </article>
    {/each}
  </section>
  <section class="memory-card">
    <div><p class="eyebrow">{t('hardware.memoryTelemetry',locale)}</p><h3>{diagnostics.memory ? t('hardware.memoryPressureTitle',locale,{pressure:localizeMemoryPressure(diagnostics.memory.pressureLabel,locale)}) : t('common.unknown',locale)}</h3>{#if diagnostics.memory}<LocalizedOwnedText value={diagnostics.memory.pressureExplanation} {locale} as="p"/>{:else}<p>{t('hardware.memoryUnavailable',locale)}</p>{/if}</div>
    {#if diagnostics.memory}<ProgressBar value={diagnostics.memory.memoryLoadPercent} label={t('hardware.currentMemoryLoad',locale)}/>{/if}
    <div class="memory-facts"><div><span>{t('hardware.available',locale)}</span><strong>{diagnostics.memory ? formatBytes(diagnostics.memory.availablePhysicalBytes,locale) : '—'}</strong></div><div><span>{t('hardware.totalPhysical',locale)}</span><strong>{diagnostics.memory ? formatBytes(diagnostics.memory.totalPhysicalBytes,locale) : '—'}</strong></div><div><span>{t('hardware.loggedMemoryWhea',locale)}</span><strong>{memoryEvents().length}</strong></div></div>
    <p class="truth-note">{t('hardware.truthNote',locale)}</p>
  </section>
  <section class="diagnostic-cards"><div class="panel-head"><div><p class="eyebrow">{t('hardware.triageEyebrow',locale)}</p><h3>{t('hardware.triageTitle',locale)}</h3></div></div>
    {#each diagnostics.cards.filter((card) => card.domain !== 'Crash') as card (card.cardId)}
      <article class="triage-card"><div class="triage-head"><div><span>{localizeDomain(card.domain,locale)}</span><h4><LocalizedOwnedText value={card.title} {locale}/></h4></div><em>{localizeSeverity(card.severity,locale)} · {localizeConfidence(card.confidence,locale)}</em></div><LocalizedOwnedText value={card.summary} {locale} as="p"/>{#if card.evidence.length}<ul>{#each card.evidence as evidence}<li><LocalizedOwnedText value={evidence} {locale}/></li>{/each}</ul>{/if}<div class="guided-actions">{#each card.actions as action}<span class="guided-action"><LocalizedOwnedText value={action} {locale}/></span>{/each}</div></article>
    {/each}
  </section>
{/if}
<!-- The diagnostic-honesty paragraph is gone. Every claim it made is already
     made by the screen itself: "Not reported" is printed where a counter is
     missing, and no percentage health score exists to disclaim. -->

<style>
  .verdict-card{display:grid;grid-template-columns:auto minmax(0,1fr);gap:12px;align-items:start;align-content:start;padding:14px 16px;margin-block:12px;border:1px solid var(--ac-border-subtle);border-radius:15px;background:var(--ac-material-base)}
  .verdict-card h2{margin:0 0 .3rem;font-size:var(--ac-type-headline);font-weight:590}
  .verdict-card p{margin:.2rem 0;color:var(--ac-text-2);line-height:1.5}
  .verdict-mark{inline-size:30px;block-size:30px;border-radius:9px;display:grid;place-items:center;border:1px solid var(--ac-border-strong);font-weight:700}
  .verdict-action .verdict-mark,.verdict-attention .verdict-mark{border-width:2px}
</style>
