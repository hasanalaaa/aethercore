<script lang="ts">
  import { fluidPress } from '../../design/motion';
  import { shellState, setPage } from '../../app/shell-state';
  import { streamState } from '../../platform/stream-state';
  import { authorizeDriverPlan, startDriverInstall, startDriverScan as startScan } from '../drivers/controller';
  import { shortDigest } from '../shared';
  import { localizePlanKind, localizeRisk, localizeState, t, td } from '../../lib/i18n';
  import SystemCarePanel from '../system-care/SystemCarePanel.svelte';
  import AboutPanel from '../../components/AboutPanel.svelte';
  import { EmptyState } from '../../design/signature';

  $: snapshot = $streamState.snapshot;
  $: hub = $streamState.hub;
  $: busy = $shellState.busy;
  $: locale = $shellState.locale;
  const modules = [
    { page:'repair', label:'nav.repair', copy:'overview.moduleRepairCopy' },
    { page:'cleanup', label:'nav.cleanup', copy:'overview.moduleCleanupCopy' },
    { page:'startup', label:'nav.startup', copy:'overview.moduleStartupCopy' },
    { page:'hardware', label:'nav.hardware', copy:'overview.moduleHardwareCopy' },
    { page:'crash', label:'nav.crash', copy:'overview.moduleCrashCopy' },
  ] as const;
</script>

<header>
  <div><p class="eyebrow">{t('overview.eyebrow',locale)}</p><h1>{t('overview.title',locale)}</h1><p class="sub">{t('overview.subtitle',locale)}</p></div>
  <div class="service-pill"><span class:online={snapshot.connected}></span>{snapshot.connected ? t('common.engineOnline',locale,{version:snapshot.serviceVersion}) : t('common.engineOffline',locale)}</div>
</header>

<div class="overview-scan-cta">
  <div><p class="eyebrow">{t('overview.intelligenceEyebrow',locale)}</p><strong>{t('overview.intelligenceTitle',locale)}</strong><span>{t('overview.intelligenceCopy',locale)}</span></div>
  <button use:fluidPress={{ pressedScale: 0.985 }} class="primary" onclick={() => setPage('deepScan')}>{t('overview.scanMyPc',locale)}</button>
</div>

<section class="hero-card">
  <div class="orb"><div class="orb-core">{snapshot.connected ? '✓' : '!'}</div></div>
  <div class="hero-copy"><p class="eyebrow">{t('overview.platformStatus',locale)}</p><h2>{snapshot.connected ? t('overview.ready',locale) : t('overview.unavailable',locale)}</h2><p>{t('overview.heroCopy',locale)}</p></div>
  <div class="hero-metrics"><div><span>{t('overview.protocol',locale)}</span><strong class="technical-isolate" dir="ltr">v7</strong></div><div><span>{t('overview.journal',locale)}</span><strong>{snapshot.connected ? snapshot.journalEventCount : t('common.notAvailable',locale)}</strong></div><div><span>{t('overview.driverScan',locale)}</span><strong>{hub.state === 'Ready' ? hub.summary.deviceCount : t('common.notCollected',locale)}</strong></div></div>
</section>

<section class="grid">
  <article class="panel">
    <div class="panel-head"><div><p class="eyebrow">{t('overview.stateEngine',locale)}</p><h3>{t('overview.currentOperation',locale)}</h3></div>{#if snapshot.activePlan}<span class="risk">{localizeRisk(snapshot.activePlan.risk,locale)}</span>{/if}</div>
    {#if snapshot.activePlan}
      <div class="plan-title"><strong>{localizePlanKind(snapshot.activePlan.kind,locale)}</strong><span>{localizeState(snapshot.activePlan.state,locale)}</span></div>
      <div class="meta"><span>{t('common.digest',locale)} <code>{shortDigest(snapshot.activePlan.digest)}</code></span><span>{t('overview.events',locale,{count:snapshot.journalEventCount})}</span></div>
      <div class="actions">
        {#if snapshot.activePlan.kind === 'Startup'}<button use:fluidPress={{ pressedScale: 0.985 }} class="primary" onclick={() => setPage('startup')}>{t('overview.openStartup',locale)}</button>
        {:else if snapshot.activePlan.kind === 'SystemRepair'}<button use:fluidPress={{ pressedScale: 0.985 }} class="primary" onclick={() => setPage('repair')}>{t('overview.openRepair',locale)}</button>
        {:else if snapshot.activePlan.kind === 'Cleanup'}<button use:fluidPress={{ pressedScale: 0.985 }} class="primary" onclick={() => setPage('cleanup')}>{t('overview.openCleanup',locale)}</button>
        {:else if snapshot.activePlan.kind === 'DriverInstall'}
          {#if snapshot.activePlan.state === 'AwaitingAuthorization'}
            {#if snapshot.activePlan.consentReadyUntilUnixMs > Date.now()}<button use:fluidPress={{ pressedScale: 0.985 }} class="primary" onclick={startDriverInstall} disabled={busy}>{t('overview.startInstall',locale)}</button>
            {:else}<button use:fluidPress={{ pressedScale: 0.985 }} class="primary" onclick={authorizeDriverPlan} disabled={busy}>{t('overview.approveDriver',locale)}</button>{/if}
            <button use:fluidPress={{ pressedScale: 0.985 }} class="secondary" onclick={() => setPage('drivers')}>{t('overview.openDrivers',locale)}</button>
          {:else}<button use:fluidPress={{ pressedScale: 0.985 }} class="primary" onclick={() => setPage('drivers')}>{t('overview.viewDriverExecution',locale)}</button>{/if}
        {/if}
      </div>
    {:else}
      <EmptyState title={t('overview.noActive',locale)} body={t('overview.noActiveCopy',locale)} />
    {/if}
  </article>

  <article class="panel driver-preview">
    <div class="panel-head"><div><p class="eyebrow">{t('overview.driverServicing',locale)}</p><h3>{t('overview.safeDriver',locale)}</h3></div><span class="live-badge">{t('common.live',locale)}</span></div>
    {#if hub.state === 'Ready'}<div class="preview-number">{hub.summary.selectableUpdateCount}</div>{:else}<div class="preview-state"><strong>{t('common.notCollected',locale)}</strong></div>{/if}
    <p>{hub.state === 'Ready' ? t('overview.driverReadyCopy',locale) : t('overview.driverIdleCopy',locale)}</p>
    <button use:fluidPress={{ pressedScale: 0.985 }} class="secondary" onclick={() => { setPage('drivers'); if (hub.state === 'Idle') startScan(); }}>{t('overview.openDrivers',locale)}</button>
  </article>
</section>

<SystemCarePanel />

<section class="module-row">
  <article class="module-live"><span>{t('common.live',locale)}</span><h4>{t('overview.moduleDrivers',locale)}</h4><p>{t('overview.moduleDriversCopy',locale)}</p><div class="locked">{t('overview.protectedInstall',locale)}</div></article>
  {#each modules as m}
    <article><span>{t('common.live',locale)}</span><h4>{td(m.label,locale)}</h4><p>{td(m.copy,locale)}</p><div class="locked">{t('common.availableNow',locale)}</div></article>
  {/each}
</section>

<AboutPanel />

<style>
  .overview-scan-cta{display:flex;align-items:center;justify-content:space-between;gap:18px;padding:16px 18px;margin:0 0 14px;border:1px solid var(--ac-border-subtle);border-radius:var(--ac-radius-md);background:var(--ac-material-elevated);box-shadow:var(--ac-shadow-card)}
  .overview-scan-cta strong,.overview-scan-cta span{display:block}.overview-scan-cta strong{font-size:1rem;margin:.15rem 0}.overview-scan-cta span{color:var(--ac-text-3);font-size:.82rem}.overview-scan-cta button{padding:.68rem 1rem;border-radius:var(--ac-radius-sm);white-space:nowrap}
  /* Where a count would be. The ringed glyph went with the empty state it
     belonged to; the reading itself is what matters here. */
  .preview-state{display:flex;align-items:center;gap:.65rem;margin-block:.7rem 1rem;color:var(--ac-text-3);font-family:var(--ac-font-mono);font-size:var(--ac-type-technical);letter-spacing:.1em;text-transform:uppercase}
</style>
