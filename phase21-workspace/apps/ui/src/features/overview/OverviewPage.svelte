<script lang="ts">
  /**
   * The Overview, rebuilt on the approved shell's composition.
   *
   * The shell (`AetherCore.html` at the repo root) draws this screen as an
   * instrument: a health orb with four channel rails beside action items, then
   * four telemetry tiles, then a service log. P47 ported the design system and
   * left the composition; this file is the composition.
   *
   * Every reading below comes from `instrument.ts`, which names the IPC field it
   * read or returns nothing. The shell's own numbers are a mockup's — `94`,
   * `96`, `11.4 s`, `214 signals` — and none of them are here. What replaced
   * them is either a measurement or an em dash. See §50.1 of the ledger for the
   * element-by-element mapping and the thirteen elements deliberately dropped.
   *
   * The app's own sections that the shell has no slot for are kept below the
   * four, in the order they were in: the protected operation, driver servicing,
   * the module row, system care and about. Nothing was deleted to fit a layout.
   */
  import { onDestroy, onMount } from 'svelte';
  import { fluidPress } from '../../design/motion';
  import { shellState, setPage } from '../../app/shell-state';
  import { OVERVIEW_READ_INTERVAL_MS, readTelemetryNow, startOverviewTelemetry } from './controller';
  import { streamState } from '../../platform/stream-state';
  import { authorizeDriverPlan, startDriverInstall } from '../drivers/controller';
  import { shortDigest } from '../shared';
  import { localizePlanKind, localizeRisk, localizeState, t, td, tp } from '../../lib/i18n';
  import SystemCarePanel from '../system-care/SystemCarePanel.svelte';
  import AboutPanel from '../../components/AboutPanel.svelte';
  import { TechnicalText } from '../../design/primitives';
  import { EmptyState, EvidenceChip, citedOnly } from '../../design/signature';
  import { actionItems, headroom, headroomEvidence, headroomLabel, healthChannels, telemetryTiles, type ActionItem } from './instrument';

  $: snapshot = $streamState.snapshot;
  $: performance = $streamState.performance;
  $: serviceLog = $streamState.serviceLog;
  $: busy = $shellState.busy;
  $: locale = $shellState.locale;

  $: channels = healthChannels(performance, locale);
  $: index = headroom(channels);
  $: orbEvidence = headroomEvidence(channels, performance, locale);
  /**
   * The citation gate, applied to the rows this screen builds itself. A row
   * whose scan reported no identity cannot cite it, and an uncitable row is not
   * rendered dimly — it is not rendered.
   */
  $: gate = citedOnly<ActionItem>(actionItems($streamState, locale), (item) => item.evidence);
  $: tiles = telemetryTiles(performance, snapshot, locale);
  $: sampled = performance.capturedUnixMs > 0;

  /**
   * The instrument fills itself. The four channels, the orb and three of the
   * four tiles read `performance`, which arrives from `get_performance_snapshot`
   * and from nowhere else — so a screen that never asked was a screen that was
   * correct, honest and permanently blank. It asks now, on open and every
   * `OVERVIEW_READ_INTERVAL_MS` while it is the page in front of the user; the
   * loop is disposed with the component, so nothing samples for a screen nobody
   * is looking at. The argument and the measured cost are in `controller.ts`.
   */
  let stopTelemetry: (() => void) | undefined;
  onMount(() => { stopTelemetry = startOverviewTelemetry(); });
  onDestroy(() => stopTelemetry?.());

  /** The empty state's one control: ask again, now. */
  let reading = false;
  async function readNow(): Promise<void> {
    reading = true;
    try { await readTelemetryNow(); } finally { reading = false; }
  }

  /** Wall-clock, zero-padded, locale-independent — it sits inside LTR mono. */
  function logTime(unixMs: number): string {
    const at = new Date(unixMs);
    const pad = (value: number): string => String(value).padStart(2, '0');
    return `${pad(at.getHours())}:${pad(at.getMinutes())}:${pad(at.getSeconds())}`;
  }
</script>

<header>
  <div><p class="eyebrow">{t('overview.eyebrow',locale)}</p><h1>{t('overview.title',locale)}</h1><p class="sub">{t('overview.subtitle',locale)}</p></div>
  <div class="overview-header-side">
    <div class="service-pill"><span class:online={snapshot.connected}></span>{snapshot.connected ? t('common.engineOnline',locale,{version:snapshot.serviceVersion}) : t('common.engineOffline',locale)}</div>
    <button use:fluidPress={{ pressedScale: 0.985 }} class="primary" onclick={() => setPage('deepScan')}>{t('overview.scanMyPc',locale)}</button>
  </div>
</header>

<div class="instrument-grid">
  <!-- §A — the health orb and its four channel rails. -->
  <section class="instrument-section span-orb" aria-label={t('overview.orbSection',locale)}>
    <div class="instrument-head">
      <span class="instrument-title">{t('overview.orbSection',locale)}</span>
      <span class="instrument-meta" class:reading={index !== undefined}>{index === undefined ? t('overview.orbMetaAwaiting',locale) : t('overview.orbMetaLive',locale)}</span>
    </div>

    <div class="orb-row">
      <div class="orb-dial" class:reading={index !== undefined}>
        <span class="orb-value">{headroomLabel(index, locale)}</span>
        <span class="orb-caption">{index === undefined ? t('overview.noBaseline',locale) : t('overview.headroomLabel',locale)}</span>
      </div>

      <div class="orb-channels">
        {#if orbEvidence}<EvidenceChip evidence={orbEvidence} {locale} />{/if}
        {#each channels as channel (channel.id)}
          <div class="channel-group">
            <div class="channel">
              <span class="channel-label">{channel.label}</span>
              <!-- An empty track is hatched, not flat: a bar at zero is a reading
                   and must not look like a channel that has no scale at all. -->
              <span class="channel-track" data-normalized={channel.pct !== undefined}>
                {#if channel.pct !== undefined}<span class="channel-fill" style="inline-size:{Math.min(100, Math.max(0, channel.pct))}%"></span>{/if}
              </span>
              <span class="channel-value">{channel.value ?? '—'}</span>
            </div>
            {#if channel.serviceState}<span class="channel-state">{channel.serviceState}</span>{/if}
          </div>
        {/each}
        {#if index === undefined}
          <p class="channel-note">{t('overview.orbEmptyBody',locale,{seconds:OVERVIEW_READ_INTERVAL_MS/1000})}</p>
          <button use:fluidPress={{ pressedScale: 0.985 }} class="secondary" onclick={readNow} disabled={reading}>{reading ? t('overview.reading',locale) : t('overview.readNow',locale)}</button>
        {/if}
      </div>
    </div>
  </section>

  <!-- §B — action items, each one a scan that has actually reported. -->
  <section class="instrument-section span-items" aria-label={t('overview.actionItems',locale)}>
    <div class="instrument-head">
      <span class="instrument-title">{t('overview.actionItems',locale)}</span>
      <span class="instrument-meta">{gate.cited.length}</span>
    </div>

    {#if gate.cited.length}
      <ul class="item-list">
        {#each gate.cited as item (item.id)}
          <li class="item-row" data-tone={item.tone}>
            <button use:fluidPress={{ pressedScale: 0.99 }} class="item-open" onclick={() => setPage(item.page)}>
              <span class="item-mark" aria-hidden="true"></span>
              <span class="item-body">
                <span class="item-title">{item.title}</span>
                <span class="item-meta"><TechnicalText value={item.meta} /></span>
                <span class="item-foot">
                  <span class="item-value">{item.value}</span>
                  <span class="item-tag">{item.tag}</span>
                </span>
              </span>
            </button>
            <EvidenceChip evidence={item.evidence} {locale} />
          </li>
        {/each}
      </ul>
    {:else}
      <EmptyState
        title={t('common.notCollected',locale)}
        body={t('overview.actionItemsEmptyBody',locale)}
        channels={[{ label: td('nav.deepScan',locale) }, { label: td('nav.drivers',locale) }, { label: td('nav.hardware',locale) }]}
      >
        <button use:fluidPress={{ pressedScale: 0.985 }} class="secondary" onclick={() => setPage('deepScan')}>{t('overview.scanMyPc',locale)}</button>
      </EmptyState>
    {/if}
  </section>

  <!-- §C — diagnostic telemetry. Four tiles, no sparklines: see §50.4. -->
  <section class="instrument-section span-full" aria-label={t('overview.telemetry',locale)}>
    <div class="instrument-head">
      <span class="instrument-title">{t('overview.telemetry',locale)}</span>
      <span class="instrument-meta" class:reading={sampled}>{sampled ? t('overview.noteSampling',locale,{interval:performance.intervalMs}) : t('overview.telemetryMetaNone',locale)}</span>
    </div>

    <div class="tile-grid">
      {#each tiles as tile (tile.id)}
        <article class="tile">
          <span class="tile-label">{tile.label}</span>
          <span class="tile-reading">
            <strong>{tile.value ?? '—'}</strong>
            {#if tile.value !== undefined && tile.unit}<em>{tile.unit}</em>{/if}
          </span>
          <span class="tile-note"><TechnicalText value={tile.note} /></span>
        </article>
      {/each}
    </div>
    <p class="tile-footnote">{t('overview.telemetryNoHistory',locale)}</p>
  </section>

  <!-- §D — the core service log: the kernel's own event stream, not boot prose. -->
  <section class="instrument-section span-full" aria-label={t('overview.serviceLog',locale)}>
    <div class="instrument-head">
      <span class="instrument-title">{t('overview.serviceLog',locale)}</span>
      <span class="instrument-meta" class:reading={serviceLog.length > 0}>{tp('unit.event',locale,serviceLog.length)}</span>
    </div>

    {#if serviceLog.length}
      <div class="log-body">
        {#each serviceLog as entry (entry.sequence)}
          <div class="log-line">
            <TechnicalText value={logTime(entry.emittedUnixMs)} />
            <TechnicalText value={`[${entry.kind}]`} />
            <TechnicalText value={entry.planId ? `seq ${entry.sequence} · ${entry.planId}` : `seq ${entry.sequence}`} />
          </div>
        {/each}
        <span class="log-cursor" aria-hidden="true">▊</span>
      </div>
    {:else}
      <EmptyState title={t('common.notCollected',locale)} body={t('overview.serviceLogEmptyBody',locale)} />
    {/if}
  </section>
</div>

<!-- BELONGS (§51.3). The one active protected operation, service-wide: what the
     machine is currently having done to it, its risk tier, its plan digest and
     the authorization window. Every other section on this page reports a
     measurement; this one reports the state engine. -->
<section class="state-engine">
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
</section>

<SystemCarePanel />

<AboutPanel />

<style>
  /* ---- the ported composition -------------------------------------------
     Twelve columns, exactly as the shell lays this screen out: orb 7, action
     items 5, telemetry 12, log 12. Everything below derives from the token
     layer; this block introduces no literal colour, radius or spacing. */
  .instrument-grid {
    display: grid;
    grid-template-columns: repeat(12, minmax(0, 1fr));
    /* Sections size to their own content. Stretching them to a shared row height
       leaves the orb sitting in 350px of nothing whenever more than three scans
       have reported into the column beside it. */
    align-items: start;
    gap: var(--ac-space-4);
    margin-block-end: var(--ac-space-5);
  }
  .span-orb { grid-column: span 7; }
  .span-items { grid-column: span 5; }
  .span-full { grid-column: span 12; }

  /* The rail collapses on its own breakpoints, so these answer to the space the
     content area actually has rather than to the window. */
  @container ac-main (max-width: 58rem) {
    .span-orb, .span-items { grid-column: span 12; }
  }

  /* `main :where(*) { flex-wrap: wrap }` in feature-layout.css is a deliberate
     zero-specificity rule for content ROWS. Inherited by a flex COLUMN it sizes
     the column as a wrapping one — measured here at 595px for 141px of content —
     so every column in this file opts out explicitly, which is what that rule's
     own comment says to do. */
  .instrument-section {
    display: flex;
    flex-direction: column;
    flex-wrap: nowrap;
    gap: var(--ac-space-5);
    min-inline-size: 0;
    padding: var(--ac-space-6) var(--ac-space-6) var(--ac-space-7);
    border: 1px solid var(--ac-edge);
    border-radius: var(--ac-radius-xl);
    background: var(--ac-material-base);
    box-shadow: var(--ac-shadow-card);
  }

  .instrument-head { display: flex; align-items: center; justify-content: space-between; gap: var(--ac-space-4); }
  .instrument-title {
    font-size: var(--ac-type-kicker);
    font-weight: 600;
    letter-spacing: var(--ac-tracking-kicker);
    text-transform: uppercase;
    color: var(--ac-text-3);
  }
  .instrument-meta {
    flex-shrink: 0;
    font-family: var(--ac-font-mono);
    font-size: var(--ac-type-technical);
    color: var(--ac-text-4);
  }
  .instrument-meta.reading { color: var(--role-healthy); }

  /* ---- §A the orb ------------------------------------------------------- */
  .orb-row { display: flex; align-items: center; gap: var(--ac-space-7); flex-wrap: wrap; }

  /* No state hue. A health threshold is not a measurement, and `snapshot.health`
     is a free-form service string rather than an enum, so the dial reports only
     whether it has a reading at all. */
  .orb-dial {
    position: relative;
    inline-size: 10rem;
    block-size: 10rem;
    flex-shrink: 0;
    display: grid;
    place-items: center;
    align-content: center;
    gap: var(--ac-space-1);
    border-radius: var(--ac-radius-pill);
    border: 1px solid var(--ac-edge-strong);
    background:
      radial-gradient(circle at 32% 28%, var(--ac-highlight), transparent 58%),
      radial-gradient(circle at 70% 78%, var(--role-interactive-wash), transparent 70%),
      var(--ac-material-base);
  }
  .orb-dial.reading { border-color: var(--role-interactive-edge); }
  .orb-value {
    font-size: 2.75rem;
    font-weight: 300;
    letter-spacing: var(--ac-tracking-display);
    line-height: 1;
    color: var(--ac-text-1);
  }
  .orb-caption {
    font-family: var(--ac-font-mono);
    font-size: var(--ac-type-technical);
    letter-spacing: 0.12em;
    color: var(--ac-text-3);
    text-align: center;
  }

  .orb-channels { flex: 1; min-inline-size: 15rem; display: flex; flex-direction: column; flex-wrap: nowrap; gap: var(--ac-space-4); }
  /* One channel, one line: label, track, reading. The track is the only elastic
     part, so a long reading narrows the bar rather than pushing itself onto a
     line of its own. A service state hangs under the row, indented to the track,
     because it belongs to that channel and must not compete for its width. */
  .channel-group { display: flex; flex-direction: column; flex-wrap: nowrap; gap: var(--ac-space-2); }
  .channel { display: flex; flex-wrap: nowrap; align-items: center; gap: var(--ac-space-4); }
  .channel-label { inline-size: 5.5rem; flex-shrink: 0; font-size: var(--ac-type-callout); color: var(--ac-text-3); }
  .channel-track {
    flex: 1;
    min-inline-size: 0;
    block-size: 0.375rem;
    border-radius: var(--ac-radius-pill);
    background: var(--ac-material-elevated);
    overflow: hidden;
  }
  .channel-track[data-normalized='false'] {
    background: repeating-linear-gradient(135deg, var(--ac-edge-strong) 0 0.125rem, transparent 0.125rem 0.375rem);
  }
  .channel-fill { display: block; block-size: 100%; border-radius: var(--ac-radius-pill); background: var(--role-interactive); }
  /* The one colour on this section that a provider asserted rather than a
     threshold inferred: `power.throttleActive`. */
  .channel-state {
    align-self: flex-start;
    margin-inline-start: 6.375rem;
    padding: 0.125rem 0.5rem;
    border-radius: var(--ac-radius-pill);
    border: 1px solid var(--role-attention-edge);
    background: var(--role-attention-wash);
    color: var(--role-attention);
    font-size: var(--ac-type-technical);
  }
  .channel-value {
    inline-size: 4rem;
    flex-shrink: 0;
    text-align: end;
    font-family: var(--ac-font-mono);
    font-size: var(--ac-type-caption);
    color: var(--ac-text-1);
  }
  .channel-note { margin: 0; max-inline-size: 58ch; font-size: var(--ac-type-callout); line-height: 1.65; color: var(--ac-text-4); text-wrap: pretty; }
  .orb-channels .secondary { align-self: flex-start; }

  /* ---- §B action items -------------------------------------------------- */
  .item-list { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; flex-wrap: nowrap; gap: var(--ac-space-2); }
  .item-row { display: flex; flex-direction: column; flex-wrap: nowrap; gap: var(--ac-space-2); min-inline-size: 0; }

  .item-open {
    display: flex;
    align-items: stretch;
    gap: var(--ac-space-4);
    inline-size: 100%;
    min-block-size: var(--ac-target-min);
    padding: var(--ac-space-4) var(--ac-space-4);
    border: 1px solid var(--ac-edge);
    border-radius: var(--ac-radius-lg);
    background: var(--ac-material-base);
    color: inherit;
    font: inherit;
    text-align: start;
    cursor: pointer;
  }
  .item-open:hover { background: var(--ac-layer-hover); }

  .item-mark {
    inline-size: 0.5rem;
    flex-shrink: 0;
    border-radius: var(--ac-radius-pill);
    background: var(--role-info);
  }
  [data-tone='critical'] .item-mark { background: var(--role-critical); }
  [data-tone='attention'] .item-mark { background: var(--role-attention); }
  [data-tone='healthy'] .item-mark { background: var(--role-healthy); }

  .item-body { flex: 1; min-inline-size: 0; display: flex; flex-direction: column; flex-wrap: nowrap; gap: 0.25rem; }
  .item-title { font-size: var(--ac-type-body); font-weight: 500; color: var(--ac-text-1); }
  .item-meta { min-inline-size: 0; overflow: hidden; }
  .item-meta :global(.technical-isolate) {
    display: block;
    font-family: var(--ac-font-mono);
    font-size: var(--ac-type-technical);
    color: var(--ac-text-3);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .item-foot { display: flex; align-items: center; gap: var(--ac-space-2); flex-wrap: wrap; margin-block-start: 0.125rem; }
  .item-value { flex-shrink: 0; font-family: var(--ac-font-mono); font-size: var(--ac-type-caption); color: var(--ac-text-1); }
  .item-tag {
    flex-shrink: 0;
    padding: 0.3125rem 0.6875rem;
    border-radius: var(--ac-radius-pill);
    border: 1px solid var(--role-info-wash);
    background: var(--role-info-wash);
    color: var(--role-info);
    font-size: var(--ac-type-technical);
    font-weight: 600;
    letter-spacing: var(--ac-tracking-tag);
    text-transform: uppercase;
    white-space: nowrap;
  }
  [data-tone='critical'] .item-tag { border-color: var(--role-critical-edge); background: var(--role-critical-wash); color: var(--role-critical); }
  [data-tone='attention'] .item-tag { border-color: var(--role-attention-edge); background: var(--role-attention-wash); color: var(--role-attention); }
  [data-tone='healthy'] .item-tag { border-color: var(--role-healthy-edge); background: var(--role-healthy-wash); color: var(--role-healthy); }

  /* ---- §C telemetry tiles ----------------------------------------------- */
  .tile-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(11.25rem, 1fr)); gap: var(--ac-space-3); }
  .tile {
    display: flex;
    flex-direction: column;
    flex-wrap: nowrap;
    gap: var(--ac-space-3);
    min-inline-size: 0;
    padding: var(--ac-space-5);
    border: 1px solid var(--ac-edge);
    border-radius: var(--ac-radius-lg);
    background: var(--ac-material-base);
  }
  .tile-label { font-size: var(--ac-type-caption); color: var(--ac-text-3); }
  .tile-reading { display: flex; flex-wrap: nowrap; align-items: baseline; gap: var(--ac-space-1); min-inline-size: 0; }
  .tile-reading strong { font-size: 1.6875rem; font-weight: 500; letter-spacing: var(--ac-tracking-display); color: var(--ac-text-1); }
  .tile-reading em { font-family: var(--ac-font-mono); font-size: var(--ac-type-caption); font-style: normal; color: var(--ac-text-3); }
  .tile-note { min-inline-size: 0; overflow: hidden; }
  .tile-note :global(.technical-isolate) {
    display: block;
    font-family: var(--ac-font-mono);
    font-size: var(--ac-type-technical);
    color: var(--ac-text-4);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  /* The shell put a sparkline in every tile. This build has one sample, not a
     series, so the absence is stated rather than drawn. */
  .tile-footnote { margin: 0; max-inline-size: 72ch; font-size: var(--ac-type-callout); line-height: 1.65; color: var(--ac-text-4); text-wrap: pretty; }

  /* ---- §D the service log ----------------------------------------------- */
  .log-body {
    display: flex;
    flex-direction: column;
    flex-wrap: nowrap;
    gap: 0.4375rem;
    max-block-size: 11.875rem;
    overflow: auto;
    padding: var(--ac-space-5);
    border: 1px solid var(--ac-edge);
    border-radius: var(--ac-radius-lg);
    background: var(--ac-sunken);
  }
  /* A log line is one technical token sequence, not prose. Under RTL the row
     would otherwise reverse and print the sequence number before the timestamp,
     which is the same defect TechnicalText exists to prevent, one level up. */
  .log-line { display: flex; flex-wrap: nowrap; gap: var(--ac-space-4); direction: ltr; }
  .log-line :global(.technical-isolate) {
    font-family: var(--ac-font-mono);
    font-size: var(--ac-type-technical);
    color: var(--ac-text-3);
    white-space: nowrap;
  }
  .log-line :global(.technical-isolate):first-child { color: var(--ac-text-4); }
  .log-cursor { font-family: var(--ac-font-mono); font-size: var(--ac-type-technical); color: var(--role-interactive); animation: ac-log-blink 1.2s steps(2) infinite; }
  @keyframes ac-log-blink { 0%, 100% { opacity: 1 } 50% { opacity: 0 } }
  @media (prefers-reduced-motion: reduce) { .log-cursor { animation: none; } }

  /* ---- the header action row -------------------------------------------- */
  .overview-header-side { display: flex; align-items: center; gap: var(--ac-space-3); flex-wrap: wrap; justify-content: flex-end; }
  .overview-header-side .primary { white-space: nowrap; }

  /* One panel, not a column of a two-column grid. `.grid` splits 1.25fr / .55fr
     for a pair, and the driver panel that used to be the second one is gone. */
  .state-engine { margin-block-start: var(--ac-space-5); }
</style>
