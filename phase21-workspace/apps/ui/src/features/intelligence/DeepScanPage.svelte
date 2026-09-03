<script lang="ts">
  import { onMount } from 'svelte';
  import { MaterialSurface, Pressable, ProgressBar, TechnicalText } from '../../design/primitives';
  import { fluidPress } from '../../design/motion/fluid-press';
  import FindingCard from './FindingCard.svelte';
  import { formatDateTime, formatNumber, hasMessageKey, t, td, type Locale, type MessageKey } from '../../lib/i18n';
  import { streamState } from '../../platform/stream-state';
  import { shellState } from '../../app/shell-state';
  import { cancelDeepScan, refreshDeepScanHistory, startDeepScan } from './controller';
  import { citedOnly, type Evidence } from '../../design/signature';
  import type { PcFinding } from '../../lib/contracts';

  /**
   * The observation a finding rests on, assembled only from the evidence refs
   * the scan actually attached. The chip face names the sources and how many
   * facts were cited; the expanded block is one line per fact — source, kind,
   * the technical value, and when it was observed.
   *
   * A finding citing nothing returns null and `citedOnly` drops it, so the deep
   * scan can never show a claim it cannot support.
   */
  function evidenceFor(finding: PcFinding): Evidence | null {
    if (!finding.evidence.length) return null;
    const sources = [...new Set(finding.evidence.map((e) => e.source).filter(Boolean))];
    return {
      cite: `${sources.join(' · ') || finding.evidence[0].kind} · ${finding.evidence.length}`,
      raw: finding.evidence
        .map((e) => `${e.source} ${e.kind}  ${e.technicalValue}  ${formatDateTime(e.observedUnixMs, locale)}`)
        .join('\n'),
    };
  }

  const gateFindings = (items: readonly PcFinding[]) => citedOnly<PcFinding>(items, evidenceFor);


  type Filter = 'all' | 'drivers' | 'windows' | 'hardware' | 'storage' | 'performance' | 'startup' | 'cleanup' | 'diagnostics';
  const filters: readonly { id:Filter; key:MessageKey; domains:number[] }[] = [
    { id:'all', key:'deepScan.filter.all', domains:[] },
    { id:'drivers', key:'deepScan.filter.drivers', domains:[3] },
    { id:'windows', key:'deepScan.filter.windows', domains:[4] },
    { id:'hardware', key:'deepScan.filter.hardware', domains:[2,6] },
    { id:'storage', key:'deepScan.filter.storage', domains:[5] },
    { id:'performance', key:'deepScan.filter.performance', domains:[8] },
    { id:'startup', key:'deepScan.filter.startup', domains:[9] },
    { id:'cleanup', key:'deepScan.filter.cleanup', domains:[10] },
    { id:'diagnostics', key:'deepScan.filter.diagnostics', domains:[7,12] },
  ];

  let filter: Filter = 'all';
  $: scan = $streamState.deepScan;
  $: progress = scan.progress;
  $: summary = scan.summary;
  $: locale = $shellState.locale;
  $: scanning = scan.state === 2;
  $: terminal = scan.state >= 3;
  $: selectedDomains = filters.find((entry) => entry.id === filter)?.domains ?? [];
  $: visibleFindings = scan.findings.filter((finding) => !selectedDomains.length || selectedDomains.includes(finding.domain));
  // Uncitable findings are dropped before display, and the count of what was
  // dropped is shown rather than silently shortening the list.
  $: visibleGate = gateFindings(visibleFindings);
  $: citedVisible = visibleGate.cited;
  $: uncitableCount = visibleGate.dropped;
  $: liveGate = gateFindings(scan.findings.slice(0, 3));
  $: needsAction = citedVisible.filter((finding) => finding.severity >= 4);
  $: optional = citedVisible.filter((finding) => finding.severity <= 2 && finding.remediationAvailable && finding.remediationSafety >= 1 && finding.remediationSafety <= 2);
  $: optionalIds = new Set(optional.map((finding) => finding.id));
  $: recommended = citedVisible.filter((finding) => finding.severity < 4 && !optionalIds.has(finding.id));
  $: limitedCollectors = scan.collectors.filter((collector) => collector.state >= 4);
  $: hasContinuityLimitation = scan.warnings.some((warning) => warning.startsWith('persistence ') || warning.startsWith('scan history '));
  $: limitationCount = limitedCollectors.length + (hasContinuityLimitation ? 1 : 0);

  onMount(() => { refreshDeepScanHistory().catch(() => undefined); });

  function stateLabel(value:number,current:Locale):string {
    const key:MessageKey = value === 2 ? 'deepScan.state.scanning' : value === 3 ? 'deepScan.state.completed' : value === 4 ? 'deepScan.state.partial' : value === 5 ? 'deepScan.state.cancelled' : value === 6 ? 'deepScan.state.failed' : 'deepScan.state.idle';
    return td(key,current);
  }
  function statusLabel(value:number,current:Locale):string {
    const key:MessageKey = value === 4 ? 'deepScan.status.critical' : value === 3 ? 'deepScan.status.action' : value === 2 ? 'deepScan.status.attention' : 'deepScan.status.healthy';
    return td(key,current);
  }
  function collectorStateLabel(value:number,current:Locale):string {
    const key:MessageKey = value === 4 ? 'deepScan.collector.warning' : value === 5 ? 'deepScan.collector.unavailable' : value === 6 ? 'deepScan.collector.permissionDenied' : value === 7 ? 'deepScan.collector.timedOut' : value === 8 ? 'deepScan.collector.cancelled' : 'deepScan.collector.failed';
    return td(key,current);
  }

</script>

<header class="deep-scan-header">
  <div>
    <p class="eyebrow">{t('deepScan.eyebrow', locale)}</p>
    <h1>{t('deepScan.title', locale)}</h1>
    <p class="sub">{t('deepScan.subtitle', locale)}</p>
  </div>
  <div class="scan-actions">
    {#if scanning}
      <Pressable className="scan-cancel" onclick={() => cancelDeepScan(scan.scanId)} ariaLabel={t('deepScan.cancel', locale)}>{t('deepScan.cancel', locale)}</Pressable>
    {:else}
      <Pressable className="scan-primary" disabled={!$streamState.snapshot.connected || $shellState.busy} onclick={startDeepScan} ariaLabel={t('deepScan.scanMyPc', locale)}>{t('deepScan.scanMyPc', locale)}</Pressable>
    {/if}
  </div>
</header>

<MaterialSurface level="elevated" className="scan-hero">
  <div class="status-emblem" class:scanning aria-hidden="true"><span>{scanning ? '◌' : scan.status === 1 ? '✓' : '!'}</span></div>
  <div class="scan-hero-copy">
    <p class="eyebrow">{t('deepScan.systemStatus', locale)}</p>
    <h2>{scan.scanId ? statusLabel(scan.status, locale) : t('deepScan.readyTitle', locale)}</h2>
    <p>{scan.scanId ? stateLabel(scan.state, locale) : t('deepScan.readyCopy', locale)}</p>
  </div>
  {#if scan.scanId}
    <div class="scan-facts" aria-label={t('deepScan.summary', locale)}>
      <div><span>{t('deepScan.facts',locale)}</span><strong>{formatNumber(scan.factsCount,locale)}</strong></div>
      <div><span>{t('deepScan.findings',locale)}</span><strong>{formatNumber(scan.findings.length,locale)}</strong></div>
      <div><span>{t('deepScan.limitations',locale)}</span><strong>{formatNumber(limitationCount,locale)}</strong></div>
    </div>
  {/if}
</MaterialSurface>

{#if scanning && progress}
  <MaterialSurface level="structural" className="progress-card">
    <div class="progress-copy">
      <div><p class="eyebrow">{t('deepScan.currentStage',locale)}</p><h3>{hasMessageKey(progress.currentStageKey) ? td(progress.currentStageKey,locale) : progress.currentStageKey}</h3></div>
      <strong>{formatNumber(progress.percent,locale)}%</strong>
    </div>
    <ProgressBar value={progress.percent} label={t('deepScan.progressLabel',locale)} />
    <div class="task-counts">
      <span>{t('deepScan.tasksCompleted',locale,{completed:formatNumber(progress.completedTasks,locale),total:formatNumber(progress.totalTasks,locale)})}</span>
      <span>{t('deepScan.tasksActive',locale,{count:formatNumber(progress.activeTasks,locale)})}</span>
      {#if progress.unavailableTasks}<span>{t('deepScan.tasksUnavailable',locale,{count:formatNumber(progress.unavailableTasks,locale)})}</span>{/if}
    </div>
  </MaterialSurface>
{/if}

{#if scanning && scan.findings.length}
  <section class="live-findings" aria-labelledby="deep-scan-live-findings-heading">
    <h2 id="deep-scan-live-findings-heading" class="result-heading">{t('deepScan.findingsDuringScan',locale)}</h2>
    <p class="sr-only" aria-live="polite" aria-atomic="true">{t('deepScan.findingsDiscovered',locale,{count:formatNumber(scan.findings.length,locale)})}</p>
    <div class="finding-list">
      {#each liveGate.cited as finding (finding.id)}<FindingCard {finding} evidence={finding.evidence} {locale} />{/each}
    </div>
  </section>
{/if}

{#if terminal}
  <section class="summary-grid" aria-label={t('deepScan.summary',locale)}>
    <MaterialSurface level="structural" className="summary-tile"><span>{t('deepScan.summary.needsAction',locale)}</span><strong>{formatNumber((summary?.critical ?? 0)+(summary?.high ?? 0),locale)}</strong></MaterialSurface>
    <MaterialSurface level="structural" className="summary-tile"><span>{t('deepScan.summary.recommended',locale)}</span><strong>{formatNumber(summary?.recommendedActions ?? 0,locale)}</strong></MaterialSurface>
    <MaterialSurface level="structural" className="summary-tile"><span>{t('deepScan.summary.optional',locale)}</span><strong>{formatNumber(summary?.optionalOptimizations ?? 0,locale)}</strong></MaterialSurface>
    <MaterialSurface level="structural" className="summary-tile healthy"><span>{t('deepScan.summary.healthy',locale)}</span><strong>{formatNumber(summary?.healthyChecks ?? 0,locale)}</strong></MaterialSurface>
  </section>

  {#if scan.warnings.length}
    <MaterialSurface level="structural" className="limitation-card">
      <strong>{t('deepScan.diagnosticLimitations',locale)}</strong>
      <p>{t('deepScan.diagnosticLimitationsCopy',locale)}</p>
      <ul>
        {#each limitedCollectors as collector}<li><TechnicalText value={collector.id}/><span> — {collectorStateLabel(collector.state,locale)}</span></li>{/each}
        {#if hasContinuityLimitation}<li>{t('deepScan.continuityLimitation',locale)}</li>{/if}
      </ul>
    </MaterialSurface>
  {/if}

  <div class="result-toolbar" role="group" aria-label={t('deepScan.filter.label',locale)}>
    {#each filters as item}
      <button type="button" use:fluidPress={{ pressedScale:0.985 }} class:active={filter===item.id} aria-pressed={filter===item.id} onclick={() => filter=item.id}>{td(item.key,locale)}</button>
    {/each}
  </div>

  {#if uncitableCount > 0}
    <p class="uncitable-note">{t('deepScan.uncitableDropped',locale,{count:formatNumber(uncitableCount,locale)})}</p>
  {/if}

  {#if citedVisible.length === 0}
    <MaterialSurface level="focused" className="healthy-result">
      <div aria-hidden="true">✓</div>
      <h3>{t('deepScan.noActionTitle',locale)}</h3>
      <p>{t('deepScan.noActionCopy',locale)}</p>
    </MaterialSurface>
  {:else}
    {#if needsAction.length}<h2 class="result-heading">{t('deepScan.group.needsAction',locale)}</h2>{/if}
    <div class="finding-list">
      {#each needsAction as finding (finding.id)}
        <FindingCard {finding} evidence={finding.evidence} {locale} />
      {/each}
    </div>
    {#if recommended.length}<h2 class="result-heading">{t('deepScan.group.recommended',locale)}</h2>{/if}
    <div class="finding-list">
      {#each recommended as finding (finding.id)}<FindingCard {finding} evidence={finding.evidence} {locale} />{/each}
    </div>
    {#if optional.length}<h2 class="result-heading">{t('deepScan.group.optional',locale)}</h2>{/if}
    <div class="finding-list">
      {#each optional as finding (finding.id)}<FindingCard {finding} evidence={finding.evidence} {locale} />{/each}
    </div>
  {/if}

  {#if $streamState.deepScanHistory.length}
    <details class="history-panel">
      <summary>{t('deepScan.history',locale)}</summary>
      <div class="history-list">
        {#each $streamState.deepScanHistory as entry (entry.scanId)}
          <div><strong>{statusLabel(entry.status,locale)}</strong><span>{formatDateTime(entry.completedUnixMs,locale)}</span><span>{t('deepScan.historyFindingCount',locale,{count:formatNumber(entry.findingCount,locale)})}</span></div>
        {/each}
      </div>
    </details>
  {/if}
{/if}


<style>
  .deep-scan-header{display:flex;justify-content:space-between;align-items:flex-start;gap:24px;margin-bottom:22px}
  .deep-scan-header h1{margin:.3rem 0 .35rem;font-size:2rem;font-weight:560;letter-spacing:-.035em}.deep-scan-header .sub{max-width:760px;margin:0;color:var(--ac-text-2)}
  .scan-actions{display:flex;gap:10px}.scan-primary,.scan-cancel{min-inline-size:9.5rem;padding:.72rem 1rem;border-radius:12px}.scan-primary{background:var(--ac-accent-strong);color:var(--ac-text-inverse)}.scan-cancel{background:transparent;border:1px solid var(--ac-border-strong)}
  :global(.scan-hero){display:grid;grid-template-columns:auto minmax(0,1fr) auto;gap:18px;align-items:center;padding:22px;border-radius:20px;margin-bottom:14px}
  .status-emblem{inline-size:54px;block-size:54px;border-radius:17px;display:grid;place-items:center;border:1px solid var(--ac-border-strong);font-size:1.35rem}.status-emblem.scanning{box-shadow:inset 0 0 24px color-mix(in srgb,var(--ac-accent) 10%,transparent)}
  .scan-hero-copy h2{margin:.25rem 0;font-size:1.35rem;font-weight:560}.scan-hero-copy p:last-child{margin:0;color:var(--ac-text-2)}
  .scan-facts{display:grid;grid-template-columns:repeat(3,minmax(92px,1fr));gap:8px}.scan-facts div{padding:.65rem .8rem;border-inline-start:1px solid var(--ac-border-subtle)}.scan-facts span{display:block;color:var(--ac-text-3);font-size:.78rem}.scan-facts strong{font-size:1.2rem;font-weight:560}
  :global(.progress-card){padding:18px;border-radius:16px;margin-bottom:14px}.progress-copy{display:flex;justify-content:space-between;gap:18px;align-items:flex-start;margin-bottom:12px}.progress-copy h3{margin:.2rem 0 0;font-size:1rem;font-weight:560}.progress-copy>strong{font-size:1.15rem;font-variant-numeric:tabular-nums}.task-counts{display:flex;flex-wrap:wrap;gap:14px;margin-top:10px;color:var(--ac-text-3);font-size:.8rem}
  .summary-grid{display:grid;grid-template-columns:repeat(4,minmax(0,1fr));gap:10px;margin:14px 0}:global(.summary-tile){padding:15px;border-radius:14px}.summary-tile span{display:block;color:var(--ac-text-3);font-size:.8rem}.summary-tile strong{display:block;margin-top:.2rem;font-size:1.55rem;font-weight:540}.summary-tile.healthy strong{color:var(--ac-positive)}
  :global(.limitation-card){padding:15px 17px;border-radius:14px;margin:12px 0}.limitation-card p{margin:.25rem 0;color:var(--ac-text-2)}.limitation-card ul{margin:.5rem 0 0;padding-inline-start:1.25rem;color:var(--ac-text-3)}
  .result-toolbar{display:flex;gap:7px;overflow:auto;padding:3px 0 10px;margin-top:16px}.result-toolbar button{border:1px solid var(--ac-border-subtle);background:transparent;color:var(--ac-text-2);border-radius:999px;padding:.45rem .72rem;white-space:nowrap;cursor:pointer}.result-toolbar button.active{background:var(--ac-material-focused);border-color:var(--ac-border-strong);color:var(--ac-text-1)}
  .result-heading{margin:18px 0 8px;font-size:.86rem;text-transform:uppercase;letter-spacing:.08em;color:var(--ac-text-3)}.finding-list{display:grid;gap:9px}.history-panel summary{cursor:pointer;color:var(--ac-text-2);font-weight:600}
  :global(.healthy-result){text-align:center;padding:28px;border-radius:17px;margin-top:8px}.healthy-result>div{font-size:1.65rem}.healthy-result h3{margin:.4rem 0 .2rem}.healthy-result p{margin:0;color:var(--ac-text-2)}
  .history-panel{margin-top:20px;border-top:1px solid var(--ac-border-subtle);padding-top:14px}.history-list{display:grid;gap:6px;margin-top:10px}.history-list>div{display:grid;grid-template-columns:1fr auto auto;gap:12px;padding:9px 0;color:var(--ac-text-3);font-size:.8rem}.history-list strong{color:var(--ac-text-2)}
  @media (max-width:1050px){:global(.scan-hero){grid-template-columns:auto 1fr}.scan-facts{grid-column:1/-1}.summary-grid{grid-template-columns:repeat(2,1fr)}}
  @media (prefers-reduced-transparency:reduce){:global(.scan-hero),:global(.progress-card),:global(.finding-card){backdrop-filter:none}}
  @media (prefers-contrast:more){.result-toolbar button{border-width:2px}}
</style>
