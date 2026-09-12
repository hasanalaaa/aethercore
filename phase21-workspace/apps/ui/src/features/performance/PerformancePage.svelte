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
  import { onMount } from 'svelte';
  import { fluidPress } from '../../design/motion';
  import { shellState } from '../../app/shell-state';
  import type { PerformanceWindowResponse } from '../../lib/contracts';
  import { applyPerformanceWindow, streamState } from '../../platform/stream-state';
  import { Pressable, ProgressBar, TechnicalText } from '../../design/primitives';
  import { EmptyState } from '../../design/signature';
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
  import { serviceInvoke } from '../../platform/service-client';

  $: locale = $shellState.locale;
  $: busy = $shellState.busy;
  $: performance = $streamState.performance;
  $: performanceWindow = $streamState.performanceWindow;
  $: report = $streamState.bottleneckReport;
  $: sampling = $streamState.perfSampling;
  $: selectedIds = $perfUi.selectedFindingIds;
  $: plan = $perfUi.plan;

  // Phase 27 (T5): honest engine source (native/synthetic) pulled once per mount.
  let engineSource: { source: string; platform: string } | null = null;
  serviceInvoke<{ source: string; platform: string }>('get_engine_source')
    .then((source) => (engineSource = source))
    .catch(() => (engineSource = null));

  onMount(async () => {
    try {
      const resp = await serviceInvoke<PerformanceWindowResponse>('get_performance_window', { maxSamples: 60 });
      if (resp?.samples?.length) {
        applyPerformanceWindow(resp.samples);
      }
    } catch {
      // ignore
    }
  });

  const ROLE_ROOT_CAUSE = 1;
  const ROLE_CONTRIBUTING = 2;

  /**
   * Basis points to whole percent. Returns undefined when the counter reported
   * nothing, because a missing reading is not zero utilisation — see
   * `percentOrDash`, and `bytesToGb` below, which has always worked this way.
   */
  function bpToPercent(bp: number | undefined): number | undefined {
    return bp === undefined || bp === null ? undefined : Math.round(bp / 100);
  }

  /**
   * A meter at rest reads em dash, never 0.
   *
   * These four tiles rendered `0%` whenever the counter was absent — from
   * `?? 0`, from `: 0`, and from `Math.max(0, ...[])` over an empty engine list.
   * A confident 0% is a claim that the CPU is idle and the disk is quiet, which
   * is a measurement nobody took.
   */
  function percentOrDash(value: number | undefined): string {
    return value === undefined ? '—' : `${value}%`;
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

  $: if (performanceWindow && performanceWindow.length > 0) {
    cpuHistory = performanceWindow
      .map((s) => bpToPercent(s.cpu?.totalBusyBp))
      .filter((v): v is number => v !== undefined)
      .slice(-HISTORY_LENGTH);
    memoryHistory = performanceWindow
      .map((s) => s.memory?.memoryLoadPercent)
      .filter((v): v is number => v !== undefined)
      .slice(-HISTORY_LENGTH);
    storageHistory = performanceWindow
      .map((s) => peakStorage(s))
      .filter((v): v is number => v !== undefined)
      .slice(-HISTORY_LENGTH);
    gpuHistory = performanceWindow
      .map((s) => peakGpu(s))
      .filter((v): v is number => v !== undefined)
      .slice(-HISTORY_LENGTH);
  } else if (performance.capturedUnixMs > 0) {
    cpuHistory = pushHistory(cpuHistory, bpToPercent(performance.cpu?.totalBusyBp));
    memoryHistory = pushHistory(memoryHistory, performance.memory?.memoryLoadPercent);
    storageHistory = pushHistory(storageHistory, peakStorage(performance));
    gpuHistory = pushHistory(gpuHistory, peakGpu(performance));
  }

  /** A sample that was never taken is not plotted; it does not become a zero. */
  function pushHistory(history: number[], value: number | undefined): number[] {
    if (value === undefined) return history;
    const next = [...history, Math.max(0, Math.min(100, value))];
    return next.length > HISTORY_LENGTH ? next.slice(next.length - HISTORY_LENGTH) : next;
  }

  /** Undefined when no device reported, rather than a peak of zero over nothing. */
  function peakStorage(snapshot: typeof performance): number | undefined {
    const samples = snapshot.storage.map((device) => bpToPercent(device.activeTimeBp)).filter((v): v is number => v !== undefined);
    return samples.length ? Math.max(...samples) : undefined;
  }

  function peakGpu(snapshot: typeof performance): number | undefined {
    const samples = (snapshot.gpu?.engines ?? []).map((engine) => bpToPercent(engine.utilizationBp)).filter((v): v is number => v !== undefined);
    return samples.length ? Math.max(...samples) : undefined;
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
  <!-- Title only. The eyebrow repeated the rail, and the subtitle
       ("Passive telemetry, honest bottleneck attribution, reversible tuning")
       described the screen rather than reporting anything measured. -->
  <div><h1>{t('perf.title', locale)}</h1></div>
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

{#if engineSource}
  <section class="warning-strip engine-source"><span>◈</span>
    <div><strong>{engineSource.source === 'native' ? t('about.engineSourceNative', locale) : t('about.engineSourceSynthetic', locale)}</strong></div>
  </section>
{/if}

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
    <strong class="technical-isolate" dir="ltr">{percentOrDash(bpToPercent(performance.cpu?.totalBusyBp))}</strong>
    {#if cpuHistory.length >= 2}
      <svg class="sparkline" viewBox="0 0 100 28" preserveAspectRatio="none" aria-hidden="true"><polyline points={sparkPath(cpuHistory)} /></svg>
    {:else}
      <span class="sparkline sparkline-empty" aria-hidden="true">—</span>
    {/if}
    <small>{t('perf.dpcHint', locale)} <TechnicalText value={percentOrDash(bpToPercent(performance.cpu?.dpcIsrBusyBp))}/></small>
  </article>
  <article class="metric-card">
    <span>{t('perf.memory', locale)}</span>
    <strong class="technical-isolate" dir="ltr">{percentOrDash(performance.memory?.memoryLoadPercent)}</strong>
    {#if memoryHistory.length >= 2}
      <svg class="sparkline" viewBox="0 0 100 28" preserveAspectRatio="none" aria-hidden="true"><polyline points={sparkPath(memoryHistory)} /></svg>
    {:else}
      <span class="sparkline sparkline-empty" aria-hidden="true">—</span>
    {/if}
    <small>{t('perf.standby', locale)} <TechnicalText value={`${bytesToGb(performance.memory?.standbyCacheBytes)} GB`}/></small>
  </article>
  <article class="metric-card">
    <span>{t('perf.storage', locale)}</span>
    <strong class="technical-isolate" dir="ltr">{percentOrDash(peakStorage(performance))}</strong>
    {#if storageHistory.length >= 2}
      <svg class="sparkline" viewBox="0 0 100 28" preserveAspectRatio="none" aria-hidden="true"><polyline points={sparkPath(storageHistory)} /></svg>
    {:else}
      <span class="sparkline sparkline-empty" aria-hidden="true">—</span>
    {/if}
    <small>{t('perf.latency', locale)} <TechnicalText value={performance.storage[0]?.avgTransferLatencyUs === undefined ? '—' : `${performance.storage[0].avgTransferLatencyUs} µs`}/></small>
  </article>
  <article class="metric-card">
    <span>{t('perf.gpu', locale)}</span>
    <strong class="technical-isolate" dir="ltr">{percentOrDash(peakGpu(performance))}</strong>
    {#if gpuHistory.length >= 2}
      <svg class="sparkline" viewBox="0 0 100 28" preserveAspectRatio="none" aria-hidden="true"><polyline points={sparkPath(gpuHistory)} /></svg>
    {:else}
      <span class="sparkline sparkline-empty" aria-hidden="true">—</span>
    {/if}
    <small>{t('perf.vram', locale)} <TechnicalText value={`${bytesToGb(performance.gpu?.dedicatedUsedBytes)}/${bytesToGb(performance.gpu?.dedicatedTotalBytes)} GB`}/></small>
  </article>
</section>

{#if performance.power?.throttleActive}
  <!-- The reading is "a limit is active right now". The 26 words under it said
       the speeds would recover when it cleared, which is a prediction, and that
       no action is applied automatically, which the policy band already says on
       every screen. -->
  <section class="warning-strip throttle-warning"><span>△</span>
    <div><strong>{t('perf.throttleActive', locale)}</strong></div>
  </section>
{/if}

<section class="panel bottleneck-panel">
  <div class="panel-head"><div><p class="eyebrow">{t('perf.analysisEyebrow', locale)}</p><h3>{t('perf.analysisTitle', locale)}</h3></div>
    {#if report}<span class="risk">{tp('unit.finding', locale, report.findings.length)}</span>{/if}</div>
  {#if !report}
    <!-- Honest empty state: nothing has been analysed, and the two counts say
         what an analysis would be run over. -->
    <EmptyState
      title={t('common.notCollected', locale)}
      body={t('perf.noAnalysisYet', locale)}
      channels={[{ label: t('perf.samples', locale), value: performanceWindow.length || undefined }]}
    />
  {:else if report.findings.length === 0}
    <div class="empty compact healthy">
      <span class="analysis-clear">{t('perf.allClear', locale)}</span>
      <TechnicalText value={tp('unit.sample', locale, performanceWindow.length)} />
    </div>
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
          <!-- "Only reversible, evidence-backed actions are offered. Everything
               is journaled and restorable." is the policy band's own sentence,
               one region away and on every screen. -->
          <div><strong>{t('perf.planTitle', locale)}</strong></div></div>
        <button use:fluidPress={{ pressedScale: 0.985 }} class="install-button"
          onclick={() => reviewOptimizationPlan(selectedIds)} disabled={busy}>{t('perf.reviewPlan', locale)}</button>
      </section>
    {/if}
  {/if}
</section>

<!-- The 33-word safety contract is gone. It restated, on one screen, the promise
     the persistent policy band makes on every screen — and a promise repeated
     twice reads as a promise the product is unsure of. -->
