<script lang="ts">
  import { shellState } from '../../app/shell-state';
  import { streamState } from '../../platform/stream-state';
  import { t } from '../../lib/i18n';
  import { EmptyState } from '../../design/signature';
  import { TechnicalText } from '../../design/primitives';
  $: snapshot = $streamState.snapshot;
  $: recoveryEntries = $streamState.recoveryEntries;
  $: schedulerEvent = $streamState.schedulerEvent;
  $: locale = $shellState.locale;

  function schedulerWorkload(value: string): string {
    switch (value) {
      case 'HardwareTelemetry': return t('scheduler.workload.hardwareTelemetry',locale);
      case 'DriverDiscovery': return t('scheduler.workload.driverDiscovery',locale);
      case 'CleanupInventory': return t('scheduler.workload.cleanupInventory',locale);
      case 'StartupInventory': return t('scheduler.workload.startupInventory',locale);
      case 'EventLogTriage': return t('scheduler.workload.eventLogTriage',locale);
      default: return t('scheduler.workload.unknown',locale);
    }
  }
  function schedulerState(value: number): string {
    switch (value) {
      case 1: return t('scheduler.state.started',locale);
      case 2: return t('scheduler.state.completed',locale);
      case 3: return t('scheduler.state.preempted',locale);
      case 4: return t('scheduler.state.skipped',locale);
      case 5: return t('scheduler.state.failed',locale);
      default: return t('scheduler.state.unknown',locale);
    }
  }
  function schedulerReason(value: string): string {
    switch (value) {
      case 'started': return t('scheduler.reason.started',locale);
      case 'completed': return t('scheduler.reason.completed',locale);
      case 'cancelled': return t('scheduler.reason.cancelled',locale);
      case 'readBudgetBusy': return t('scheduler.reason.readBudgetBusy',locale);
      case 'preemptionMonitorUnavailable': return t('scheduler.reason.preemptionMonitorUnavailable',locale);
      case 'timeout': return t('scheduler.reason.timeout',locale);
      case 'unavailable': return t('scheduler.reason.unavailable',locale);
      case 'permissionDenied': return t('scheduler.reason.permissionDenied',locale);
      case 'malformedResponse': return t('scheduler.reason.malformedResponse',locale);
      case 'providerFailure': return t('scheduler.reason.providerFailure',locale);
      case 'io': return t('scheduler.reason.io',locale);
      case 'internal': return t('scheduler.reason.internal',locale);
      default: return t('scheduler.reason.unknown',locale);
    }
  }
</script>

<header>
  <!-- Title only. The 18-word subtitle promised that interrupted mutations are
       never replayed; the journal below shows each one, with its restore-point
       sequence and its backup root. -->
  <div><h1>{t('activity.title',locale)}</h1></div>
  <div class="service-pill"><span class:online={snapshot.connected}></span>{snapshot.connected ? t('common.engineOnline',locale,{version:snapshot.serviceVersion}) : t('common.engineOffline',locale)}</div>
</header>

<section class="panel scheduler-activity" aria-live="polite">
  <!-- A heading that is a heading. It was an eyebrow with a 16-word SENTENCE
       under it wearing an <h3>. -->
  <div class="panel-head"><div><h3>{t('activity.schedulerTitle',locale)}</h3></div></div>
  {#if schedulerEvent}
    <div class="scheduler-row">
      <div><strong>{schedulerWorkload(schedulerEvent.workload)}</strong><span>{schedulerState(schedulerEvent.state)}</span></div>
      <p>{schedulerReason(schedulerEvent.reason)}</p>
      <div class="meta"><span>{t('activity.schedulerEvidence',locale,{count:schedulerEvent.evidenceCount})}</span><span>{t('activity.schedulerWarnings',locale,{count:schedulerEvent.warningCount})}</span></div>
    </div>
  {:else}
    <span class="scheduler-none"><TechnicalText value={t('common.notCollected',locale)}/></span>
  {/if}
</section>
{#if recoveryEntries.length === 0}
  <section class="panel activity-empty"><EmptyState title={t('common.notCollected',locale)} body={t('activity.emptyCopy',locale)} /></section>
{/if}
