<script lang="ts">
  import { fluidPress } from '../../design/motion';
  import { shellState } from '../../app/shell-state';
  import { streamState } from '../../platform/stream-state';
  import { LocalizedOwnedText, Pressable, ProgressBar, TechnicalText } from '../../design/primitives';
  import { formatNumber, localizeConfidence, localizeDomain, localizeHealthStatus, localizeMemoryPressure, localizeSeverity, t } from '../../lib/i18n';
  import { diagnosticRunning, memoryEvents, startDiagnosticsScan, storageActionCount } from './controller';
  import ProviderFaultsPanel from './ProviderFaultsPanel.svelte';
  import { formatBytes } from '../shared';
  import { EmptyState } from '../../design/signature';

  $: snapshot = $streamState.snapshot;
  $: diagnostics = $streamState.diagnostics;
  $: busy = $shellState.busy;
  $: locale = $shellState.locale;

  function metric(hasValue: boolean, value: number, suffix = ''): string {
    return hasValue ? `${formatNumber(value, locale)}${suffix}` : t('common.notReported', locale);
  }
</script>

<header>
  <div><p class="eyebrow">{t('hardware.eyebrow',locale)}</p><h1>{t('hardware.actualTitle',locale)}</h1><p class="sub">{t('hardware.actualSubtitle',locale)}</p></div>
  <div class="header-actions"><div class="service-pill"><span class:online={snapshot.connected}></span>{snapshot.connected ? t('common.engineOnline',locale,{version:snapshot.serviceVersion}) : t('common.engineOffline',locale)}</div><Pressable className="scan-button" onclick={() => startDiagnosticsScan('hardware')} disabled={busy || diagnosticRunning() || !snapshot.connected}><span>↻</span>{diagnosticRunning() ? t('hardware.collecting',locale) : diagnostics.state === 'Idle' ? t('hardware.collect',locale) : t('hardware.collectAgain',locale)}</Pressable></div>
</header>
{#if diagnosticRunning()}<div class="indeterminate cleanup-scan-progress"><span></span></div>{/if}
{#if diagnostics.warnings.length}<section class="warning-strip"><span>◇</span><div><strong>{t('hardware.partial',locale)}</strong>{#each diagnostics.warnings as warning}<LocalizedOwnedText value={warning} {locale} as="p"/>{/each}</div></section>{/if}
<ProviderFaultsPanel faults={diagnostics.providerFaults} {locale}/>
<section class="hardware-summary">
  <article><span>{t('hardware.storageDevices',locale)}</span><strong>{diagnostics.storage.length || '—'}</strong><small>{t('hardware.storageHint',locale)}</small></article>
  <article><span>{t('hardware.actionRequired',locale)}</span><strong>{storageActionCount()}</strong><small>{t('hardware.actionHint',locale)}</small></article>
  <article><span>{t('hardware.memoryLoad',locale)}</span><strong>{diagnostics.memory ? `${formatNumber(diagnostics.memory.memoryLoadPercent,locale)}%` : '—'}</strong><small>{diagnostics.memory ? localizeMemoryPressure(diagnostics.memory.pressureLabel,locale) : t('hardware.memoryMetricHint',locale)}</small></article>
  <article><span>{t('hardware.wheaMemory',locale)}</span><strong>{memoryEvents().length}</strong><small>{t('hardware.wheaHint',locale)}</small></article>
</section>

{#if diagnostics.state === 'Idle'}
  <section class="panel activity-empty"><EmptyState title={t('hardware.emptyTitle',locale)} body={t('hardware.emptyCopy',locale)}>
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
<section class="safety-note"><span>▱</span><div><strong>{t('hardware.safetyTitle',locale)}</strong><p>{t('hardware.safetyCopy',locale)}</p></div></section>
