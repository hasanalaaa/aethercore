<script lang="ts">
  /**
   * Phase 20 Domain D — Performance Intelligence page.
   *
   * Apple-grade fluid interface rules applied here:
   * - All press feedback fires on pointerdown via `fluidPress` (never on release).
   * - Charts render from live presentation values; every transition is a spring-driven,
   *   fully interruptible motion (see design/motion), never a fixed-duration CSS animation.
   * - Materials: translucent surfaces with backdrop blur and light-catching edges.
   * - Reduced-motion / reduced-transparency / high-contrast are honored through the shared
   *   preference store and CSS custom properties (no bespoke media queries here).
   * - Bidi: numbers and technical codes are isolated with TechnicalText so Arabic layout
   *   never mirrors metric values incorrectly.
   */
  import { fluidPress } from '../../design/motion';
  import { shellState } from '../../app/shell-state';
  import { streamState } from '../../platform/stream-state';
  import { Pressable, ProgressBar, TechnicalText } from '../../design/primitives';
  import { t, td, tp, hasMessageKey } from '../../lib/i18n';
  import {
    analyzeBottlenecks,
    closeOptimizationReview,
    perfUi,
    reviewOptimizationPlan,
    setSelectedFindingIds,
    startPerfSampling,
    stopPerfSampling,
  } from './controller';

  $: locale = $shellState.locale;
  $: busy = $shellState.busy;
  $: performance = $streamState.performance;
  $: report = $streamState.bottleneckReport;
  $: sampling = $streamState.perfSampling;
  $: selectedIds = $perfUi.selectedFindingIds;
  $: plan = $perfUi.plan;

  const ROLE_ROOT_CAUSE = 1;
  const ROLE_CONTRIBUTING = 2;

  function bpToPercent(bp: number | undefined): number {
    return Math.round((bp ?? 0) / 100);
  }

  function bytesToGb(bytes: number | undefined): string {
    if (!bytes) return '—';
    return (bytes / (1024 * 1024 * 1024)).toFixed(1);
  }

  /** Sparkline path from a bounded sample history kept in local component state. */
  let cpuHistory: number[] = [];
  let memoryHistory: number[] = [];
  let storageHistory: number[] = [];
  let gpuHistory: number[] = [];
  const HISTORY_LENGTH = 60;

  $: if (performance.capturedUnixMs > 0) {
    cpuHistory = pushHistory(cpuHistory, bpToPercent(performance.cpu?.totalBusyBp));
    memoryHistory = pushHistory(memoryHistory, performance.memory?.memoryLoadPercent ?? 0);
    storageHistory = pushHistory(storageHistory, peakStorage(performance));
    gpuHistory = pushHistory(gpuHistory, peakGpu(performance));
  }

  function pushHistory(history: number[], value: number): number[] {
    const next = [...history, Math.max(0, Math.min(100, value))];
    return next.length > HISTORY_LENGTH ? next.slice(next.length - HISTORY_LENGTH) : next;
  }

  function peakStorage(snapshot: typeof performance): number {
    return Math.max(0, ...snapshot.storage.map((device) => bpToPercent(device.activeTimeBp)));
  }

  function peakGpu(snapshot: typeof performance): number {
    return Math.max(0, ...(snapshot.gpu?.engines ?? []).map((engine) => bpToPercent(engine.utilizationBp)));
  }

  /** SVG polyline points for a sparkline in a 100x28 viewBox, LTR always (technical chart). */
  function sparkPath(history: number[]): string {
    if (!history.length) return '';
    const step = 100 / (HISTORY_LENGTH - 1);
    const offset = HISTORY_LENGTH - history.length;
    return history
      .map((value, index) => `${((offset + index) * step).toFixed(2)},${(28 - (value / 100) * 26 - 1).toFixed(2)}`)
      .join(' ');
  }

  function roleLabel(role: number): string {
    if (role === ROLE_ROOT_CAUSE) return t('perf.role.rootCause', locale);
    if (role === ROLE_CONTRIBUTING) return t('perf.role.contributing', locale);
    return t('perf.role.symptom', locale);
  }
</script>

<header>
  <div><p class="eyebrow">{t('perf.eyebrow', locale)}</p><h1>{t('perf.title', locale)}</h1><p class="sub">{t('perf.subtitle', locale)}</p></div>
  <div class="header-actions">
    {#if sampling}
      <Pressable className="scan-button" onclick={stopPerfSampling} disabled={busy}><span>■</span>{t('perf.stop', locale)}</Pressable>
    {:else}
      <Pressable className="scan-button" onclick={startPerfSampling} disabled={busy}><span>▶</span>{t('perf.start', locale)}</Pressable>
    {/if}
    <Pressable className="secondary analyze-button" onclick={() => analyzeBottlenecks()} disabled={busy || !sampling}>
      <span>◎</span>{report ? t('perf.analyzeAgain', locale) : t('perf.analyze', locale)}
    </Pressable>
  </div>
</header>

{#if performance.collectorFaults.length}
  <section class="warning-strip"><span>◇</span>
    <div><strong>{t('perf.degradedCollectors', locale)}</strong>
      {#each performance.collectorFaults as fault}
        <p><TechnicalText value={fault.collector}/> · {hasMessageKey(`perf.fault.${fault.kind}`) ? td(`perf.fault.${fault.kind}` as never, locale) : fault.kind}</p>
      {/each}
    </div>
  </section>
{/if}

<section class="cleanup-summary perf-summary">
  <article class="metric-card">
    <span>{t('perf.cpu', locale)}</span>
    <strong class="technical-isolate" dir="ltr">{bpToPercent(performance.cpu?.totalBusyBp)}%</strong>
    <svg class="sparkline" viewBox="0 0 100 28" preserveAspectRatio="none" aria-hidden="true"><polyline points={sparkPath(cpuHistory)} /></svg>
    <small>{t('perf.dpcHint', locale)} <TechnicalText value={`${bpToPercent(performance.cpu?.dpcIsrBusyBp)}%`}/></small>
  </article>
  <article class="metric-card">
    <span>{t('perf.memory', locale)}</span>
    <strong class="technical-isolate" dir="ltr">{performance.memory?.memoryLoadPercent ?? 0}%</strong>
    <svg class="sparkline" viewBox="0 0 100 28" preserveAspectRatio="none" aria-hidden="true"><polyline points={sparkPath(memoryHistory)} /></svg>
    <small>{t('perf.standby', locale)} <TechnicalText value={`${bytesToGb(performance.memory?.standbyCacheBytes)} GB`}/></small>
  </article>
  <article class="metric-card">
    <span>{t('perf.storage', locale)}</span>
    <strong class="technical-isolate" dir="ltr">{storageHistory.length ? peakStorage(performance) : 0}%</strong>
    <svg class="sparkline" viewBox="0 0 100 28" preserveAspectRatio="none" aria-hidden="true"><polyline points={sparkPath(storageHistory)} /></svg>
    <small>{t('perf.latency', locale)} <TechnicalText value={`${performance.storage[0]?.avgTransferLatencyUs ?? 0} µs`}/></small>
  </article>
  <article class="metric-card">
    <span>{t('perf.gpu', locale)}</span>
    <strong class="technical-isolate" dir="ltr">{gpuHistory.length ? peakGpu(performance) : 0}%</strong>
    <svg class="sparkline" viewBox="0 0 100 28" preserveAspectRatio="none" aria-hidden="true"><polyline points={sparkPath(gpuHistory)} /></svg>
    <small>{t('perf.vram', locale)} <TechnicalText value={`${bytesToGb(performance.gpu?.dedicatedUsedBytes)}/${bytesToGb(performance.gpu?.dedicatedTotalBytes)} GB`}/></small>
  </article>
</section>

{#if performance.power?.throttleActive}
  <section class="warning-strip throttle-warning"><span>△</span>
    <div><strong>{t('perf.throttleActive', locale)}</strong><p>{t('perf.throttleCopy', locale)}</p></div>
  </section>
{/if}

<section class="panel bottleneck-panel">
  <div class="panel-head"><div><p class="eyebrow">{t('perf.analysisEyebrow', locale)}</p><h3>{t('perf.analysisTitle', locale)}</h3></div>
    {#if report}<span class="risk">{tp('unit.finding', locale, report.findings.length)}</span>{/if}</div>
  {#if !report}
    <div class="empty compact"><p>{t('perf.noAnalysisYet', locale)}</p></div>
  {:else if report.findings.length === 0}
    <div class="empty compact healthy"><p>{t('perf.allClear', locale)}</p></div>
  {:else}
    <div class="finding-list">
      {#each report.findings as finding (finding.id)}
        <article class="finding-card" class:root-cause={finding.role === ROLE_ROOT_CAUSE}>
          <label class="finding-select">
            <input
              type="checkbox"
              checked={selectedIds.includes(finding.id)}
              disabled={!finding.applicableActionKinds.length}
              onchange={(event) => {
                const checked = (event.currentTarget as HTMLInputElement).checked;
                setSelectedFindingIds(checked ? [...selectedIds, finding.id] : selectedIds.filter((id) => id !== finding.id));
              }}
            />
            <span></span>
          </label>
          <div class="finding-body">
            <header><strong>{t(finding.titleKey as never, locale)}</strong><em>{roleLabel(finding.role)}</em></header>
            <p>{t(finding.summaryKey as never, locale, Object.fromEntries(finding.messageArgs.map((arg) => [arg.key, arg.value])) as never)}</p>
            <small class="evidence-line">
              {#each finding.evidence.slice(0, 2) as ev}
                <TechnicalText value={`${ev.factKey}: ${ev.observedValue >= 1_000_000 ? (ev.observedValue / 1_000_000).toFixed(1) + 'M' : Math.round(ev.observedValue)} / ${ev.threshold >= 1_000_000 ? (ev.threshold / 1_000_000).toFixed(1) + 'M' : Math.round(ev.threshold)}`}/>
              {/each}
            </small>
          </div>
        </article>
      {/each}
    </div>
    {#if selectedIds.length}
      <section class="selection-tray">
        <div><span class="selection-count">{selectedIds.length}</span>
          <div><strong>{t('perf.planTitle', locale)}</strong><p>{t('perf.planCopy', locale)}</p></div></div>
        <button use:fluidPress={{ pressedScale: 0.985 }} class="install-button"
          onclick={() => reviewOptimizationPlan(selectedIds)} disabled={busy}>{t('perf.reviewPlan', locale)}</button>
      </section>
    {/if}
  {/if}
</section>

<section class="safety-note"><span>◇</span>
  <div><strong>{t('perf.safetyTitle', locale)}</strong><p>{t('perf.safetyCopy', locale)}</p></div>
</section>
