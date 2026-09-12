<script lang="ts">
  /**
   * The Overview, rebuilt to `docs/phase56/DIRECTION.md`.
   *
   * P50 ported the approved shell's composition; P51 fixed where things sit
   * (4,110px → 1,715px). Neither pass touched what the screen SAYS or how it
   * ranks what it shows, and that is what the owner was reacting to. Measured
   * before this pass (`tools/measure-density.mjs`): **15 distinct type sizes and
   * 20 distinct surface treatments — the most of any screen in the product** —
   * against 63 words of prose, the second-fewest. The complaint was "too much
   * text"; the defect was that nothing outranked anything.
   *
   * So this pass is hierarchy and copy, not layout. Three things changed:
   *
   * 1. **One hero.** The orb is now a real dial — an SVG ring gauge whose arc
   *    length IS the headroom reading — with the figure at `--ac-type-hero`
   *    (48px). It is the only thing on the screen at that size. The decorative
   *    radial fill is gone: a gauge that draws the value does not need a glow to
   *    look alive, and removing it also removed a surface level.
   * 2. **Six type sizes, every one a token.** 48 hero · 34 display · 20 title ·
   *    15 headline · 13.5 body · 11 technical. Nothing on this screen sets a
   *    size literal any more.
   * 3. **Three neutral surface levels.** card (`--ac-material-base`), well
   *    (`--ac-sunken`, for the action rows and the log), control
   *    (`--ac-glass-2`, for buttons and evidence chips). A card never sits
   *    inside a card: the action rows and the telemetry tiles used to be cards
   *    on cards, and are now a well and a bordered cell respectively.
   *
   * Copy cuts, each one deleting a sentence and keeping the fact it carried:
   *
   *   - the subtitle ("A unified workspace for safe maintenance…") — marketing;
   *     it asserts nothing measurable. Gone, with the eyebrow that repeated the
   *     title above it.
   *   - "No sparklines: this build retains the latest sample, not a series" —
   *     the fact is *how many samples there are*, and it is now the telemetry
   *     section's own meta, as a number.
   *   - the orb's empty body ("reads the counters itself, every 5 seconds…") —
   *     the fact is the cadence, now the section meta.
   *   - three empty-state bodies that explained how the screen works.
   *   - the service pill: connection state was rendered three times on this
   *     screen (rail, shell context bar, pill). One idea per region.
   *
   * Every reading still comes from `instrument.ts`, which names the IPC field it
   * read or returns nothing, and `undefined` still renders `—`.
   */
  import { onDestroy, onMount } from 'svelte';
  import { fluidPress } from '../../design/motion';
  import { shellState, setPage } from '../../app/shell-state';
  import { OVERVIEW_READ_INTERVAL_MS, readTelemetryNow, startOverviewTelemetry } from './controller';
  import { streamState } from '../../platform/stream-state';
  import { openCareConsent } from '../care/controller';
  import { authorizeDriverPlan, startDriverInstall } from '../drivers/controller';
  import { shortDigest } from '../shared';
  import { localizePlanKind, localizeRisk, localizeState, t, td, tp } from '../../lib/i18n';
  import { TechnicalText } from '../../design/primitives';
  import { EmptyState, EvidenceChip, citedOnly } from '../../design/signature';
  import { actionItems, headroom, headroomEvidence, headroomLabel, healthChannels, samplingNote, telemetryTiles, type ActionItem } from './instrument';

  $: snapshot = $streamState.snapshot;
  $: performance = $streamState.performance;
  $: performanceWindow = $streamState.performanceWindow;
  $: serviceLog = $streamState.serviceLog;
  $: busy = $shellState.busy;
  $: locale = $shellState.locale;

  $: channels = healthChannels(performance, locale, performanceWindow);
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

  /** SVG polyline points for a channel sparkline in a 60x16 viewBox, LTR always. */
  function channelSparkPath(history: number[]): string {
    if (history.length < 2) return '';
    const max = 100;
    const step = 60 / (history.length - 1);
    return history
      .map((val, idx) => {
        const clamped = Math.max(0, Math.min(max, val));
        const x = (idx * step).toFixed(1);
        const y = (16 - (clamped / max) * 14 - 1).toFixed(1);
        return `${x},${y}`;
      })
      .join(' ');
  }

  /**
   * The dial's arc. The ring is r=52 in a 120 viewBox, so one full turn is
   * 2πr = 326.73 units; the arc is that length minus the share the reading did
   * not claim. There is no threshold and no state hue — the arc reports the
   * number, and the number is already traced by `headroomEvidence`.
   */
  const DIAL_CIRCUMFERENCE = 2 * Math.PI * 52;
  function dialOffset(value: number | undefined): number {
    if (value === undefined) return DIAL_CIRCUMFERENCE;
    const clamped = Math.max(0, Math.min(100, value));
    return DIAL_CIRCUMFERENCE * (1 - clamped / 100);
  }
</script>

<header>
  <h1>{t('overview.title', locale)}</h1>
  <div class="overview-header-side">
    <button use:fluidPress={{ pressedScale: 0.985 }} class="secondary" onclick={openCareConsent} disabled={!snapshot.connected || busy}>{t('care.start', locale)}</button>
    <button use:fluidPress={{ pressedScale: 0.985 }} class="primary" onclick={() => setPage('deepScan')}>{t('overview.scanMyPc',locale)}</button>
  </div>
</header>

<div class="instrument-grid">
  <!-- §A — the headroom dial and its four channel rails. -->
  <section class="instrument-section span-orb" aria-label={t('overview.orbSection',locale)}>
    <div class="instrument-head">
      <h2 class="instrument-title">{t('overview.orbSection',locale)}</h2>
      <!-- The cadence, as a token. It used to be a 34-word paragraph under the
           channels explaining that this screen reads the counters itself. -->
      <span class="instrument-meta" class:reading={index !== undefined}>
        <TechnicalText value={index === undefined ? t('overview.orbMetaAwaiting',locale,{seconds:OVERVIEW_READ_INTERVAL_MS/1000}) : t('overview.orbMetaLive',locale,{seconds:OVERVIEW_READ_INTERVAL_MS/1000})} />
      </span>
    </div>

    <div class="orb-row">
      <div class="orb-dial" class:reading={index !== undefined}>
        <!-- LTR always: a gauge is a technical chart, and mirroring it under RTL
             would reverse the direction the value grows in. -->
        <svg class="orb-gauge" viewBox="0 0 120 120" aria-hidden="true" focusable="false">
          <circle class="orb-track" cx="60" cy="60" r="52" />
          {#if index !== undefined}
            <circle
              class="orb-arc"
              cx="60" cy="60" r="52"
              stroke-dasharray={DIAL_CIRCUMFERENCE}
              stroke-dashoffset={dialOffset(index)}
            />
          {/if}
        </svg>
        <span class="orb-value">{headroomLabel(index, locale)}</span>
        <span class="orb-caption">{index === undefined ? t('overview.noBaseline',locale) : t('overview.headroomLabel',locale)}</span>
      </div>

      <div class="orb-channels">
        {#if orbEvidence}<EvidenceChip evidence={orbEvidence} {locale} />{/if}
        {#each channels as channel (channel.id)}
          <div class="channel-group">
            <div class="channel">
              <span class="channel-label">{channel.label}</span>
              <!-- A channel with no scale to normalize against is hatched, not
                   flat: a bar at zero is a reading and must not look like a
                   channel that has no scale at all. Both states are drawn from
                   the interactive role, so the track is a lighter step of the
                   fill's own ramp rather than a fourth neutral surface. -->
              <span class="channel-track" data-normalized={channel.pct !== undefined}>
                {#if channel.pct !== undefined}<span class="channel-fill" style="inline-size:{Math.min(100, Math.max(0, channel.pct))}%"></span>{/if}
              </span>
              {#if channel.history && channel.history.length >= 2}
                <svg class="channel-sparkline" viewBox="0 0 60 16" preserveAspectRatio="none" aria-hidden="true">
                  <polyline points={channelSparkPath(channel.history)} />
                </svg>
              {:else}
                <span class="channel-sparkline channel-sparkline-empty" aria-hidden="true">—</span>
              {/if}
              <span class="channel-value">{channel.value ?? '—'}</span>
            </div>
            {#if channel.serviceState}<span class="channel-state">{channel.serviceState}</span>{/if}
          </div>
        {/each}
        {#if index === undefined}
          <button use:fluidPress={{ pressedScale: 0.985 }} class="secondary" onclick={readNow} disabled={reading}>{reading ? t('overview.reading',locale) : t('overview.readNow',locale)}</button>
        {/if}
      </div>
    </div>
  </section>

  <!-- §B — action items, each one a scan that has actually reported. -->
  <section class="instrument-section span-items" aria-label={t('overview.actionItems',locale)}>
    <div class="instrument-head">
      <h2 class="instrument-title">{t('overview.actionItems',locale)}</h2>
      <span class="instrument-meta"><TechnicalText value={String(gate.cited.length)} /></span>
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

  <!-- §C — diagnostic telemetry. -->
  <section class="instrument-section span-instrument" aria-label={t('overview.telemetry',locale)}>
    <div class="instrument-head">
      <h2 class="instrument-title">{t('overview.telemetry',locale)}</h2>
      <!-- How many samples this build is holding. Below two there is no series
           to draw, which is the fact the deleted footnote spent 20 words on. -->
      <span class="instrument-meta" class:reading={sampled}>
        <TechnicalText value={sampled ? `${tp('unit.sample', locale, performanceWindow.length)} · ${samplingNote(performance, locale)}` : t('overview.telemetryMetaNone',locale)} />
      </span>
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
  </section>

  <!-- §D — the core service log: the kernel's own event stream, not boot prose. -->
  <section class="instrument-section span-instrument" aria-label={t('overview.serviceLog',locale)}>
    <div class="instrument-head">
      <h2 class="instrument-title">{t('overview.serviceLog',locale)}</h2>
      <span class="instrument-meta" class:reading={serviceLog.length > 0}><TechnicalText value={tp('unit.event',locale,serviceLog.length)} /></span>
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
    <div class="panel-head">
      <h3>{t('overview.currentOperation',locale)}</h3>
      {#if snapshot.activePlan}<span class="risk">{localizeRisk(snapshot.activePlan.risk,locale)}</span>{/if}
    </div>
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

<style>
  /* ---- the composition ---------------------------------------------------
     Twelve columns, unchanged from P51: the orb, the telemetry and the log
     stack in the left column and the action list spans all three rows. What
     changed is the RHYTHM. P51 used `--ac-space-4` between sections and
     `--ac-space-5` inside them, so the gap separating two ideas was smaller
     than the gap separating a heading from its own content, and nine regions
     read as one undifferentiated field of boxes. Space now groups: `space-7`
     between sections, `space-5` inside one. (DIRECTION.md, principle 3.)

     Everything below derives from the token layer; this block introduces no
     literal colour, radius, spacing or TYPE SIZE. The last of those is new —
     the previous version set 2.75rem on the orb and 1.6875rem on the tiles. */
  .instrument-grid {
    display: grid;
    grid-template-columns: repeat(12, minmax(0, 1fr));
    /* Sections size to their own content. Stretching them to a shared row height
       leaves the orb sitting in 350px of nothing whenever more than three scans
       have reported into the column beside it. */
    align-items: start;
    gap: var(--ac-space-7);
    margin-block-end: var(--ac-space-7);
  }
  .span-orb, .span-instrument { grid-column: span 7; }
  /* Three rows: the orb, the telemetry and the log stack beside this one list. */
  .span-items { grid-column: span 5; grid-row: span 3; }

  /* The rail collapses on its own breakpoints, so these answer to the space the
     content area actually has rather than to the window. */
  @container ac-main (max-width: 58rem) {
    .span-orb, .span-items, .span-instrument { grid-column: span 12; }
    /* One column: the row span would otherwise leave two empty rows. */
    .span-items { grid-row: auto; }
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
    /* SURFACE LEVEL 2 of 3 — the card. Level 1 is the page; level 3 is the
       well below. Nothing on this screen adds a fourth. */
    background: var(--ac-material-base);
    box-shadow: var(--ac-shadow-card);
  }

  .instrument-head { display: flex; align-items: baseline; justify-content: space-between; gap: var(--ac-space-4); }
  /* A real heading in real words, at a size the reader can rank. It was an 11px
     uppercase kicker, which is decoration wearing a title's job. */
  .instrument-title {
    margin: 0;
    font-size: var(--ac-type-headline);
    font-weight: 600;
    color: var(--ac-text-1);
  }
  .instrument-meta {
    flex-shrink: 0;
    color: var(--ac-text-4);
  }
  .instrument-meta :global(.technical-isolate) {
    font-family: var(--ac-font-mono);
    font-size: var(--ac-type-technical);
    white-space: nowrap;
  }
  .instrument-meta.reading :global(.technical-isolate) { color: var(--role-healthy); }

  /* ---- §A the dial ------------------------------------------------------
     The orb is the screen's one hero figure, and it now DRAWS the number it
     reports: the arc's length is the reading. Before, the number sat inside a
     decorative radial gradient that meant nothing and painted a fourth neutral
     surface. A gauge that shows the value does not need a glow to look alive.

     No state hue. A health threshold is not a measurement, and `snapshot.health`
     is a free-form service string rather than an enum, so the dial reports only
     the value and whether it has one at all. */
  .orb-row { display: flex; align-items: center; gap: var(--ac-space-7); flex-wrap: wrap; }

  .orb-dial {
    position: relative;
    inline-size: 10rem;
    block-size: 10rem;
    flex-shrink: 0;
    display: grid;
    place-items: center;
    align-content: center;
    gap: var(--ac-space-1);
  }
  .orb-gauge {
    position: absolute;
    inset: 0;
    inline-size: 100%;
    block-size: 100%;
    /* A gauge is a technical chart. Mirrored under RTL the arc would grow the
       wrong way, which is the same defect TechnicalText prevents for tokens. */
    direction: ltr;
    transform: rotate(-90deg);
  }
  .orb-track { fill: none; stroke: var(--role-interactive-wash); stroke-width: 6; }
  .orb-arc {
    fill: none;
    stroke: var(--role-interactive);
    stroke-width: 6;
    stroke-linecap: round;
    transition: stroke-dashoffset var(--ac-feedback-normal) var(--ac-ease-state);
  }
  @media (prefers-reduced-motion: reduce) { .orb-arc { transition: none; } }

  .orb-value {
    z-index: 1;
    font-size: var(--ac-type-hero);
    font-weight: 300;
    letter-spacing: var(--ac-tracking-display);
    line-height: 1;
    color: var(--ac-text-1);
  }
  .orb-caption {
    z-index: 1;
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
  .channel-label { inline-size: 5.5rem; flex-shrink: 0; font-size: var(--ac-type-body); color: var(--ac-text-3); }
  /* A meter's unfilled track is a lighter step of the fill's OWN ramp, so the
     reading is legible across the whole bar. It used to be `--ac-material-
     elevated`, a neutral surface, which made it a fourth structural level and
     broke the fill's relationship to its track. */
  .channel-track {
    flex: 1;
    min-inline-size: 4rem;
    block-size: 0.375rem;
    border-radius: var(--ac-radius-pill);
    /* `-edge` rather than `-wash`: the wash is 0.16 alpha and, on the card this
       sits on, the unfilled part of the bar was invisible — so a 41% meter read
       as a full one. Same ramp, one step up, which is what the track needs to
       do its job. */
    background: var(--role-interactive-edge);
    overflow: hidden;
  }
  .channel-track[data-normalized='false'] {
    background:
      repeating-linear-gradient(135deg, var(--role-interactive) 0 0.125rem, transparent 0.125rem 0.375rem),
      var(--role-interactive-wash);
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
    font-size: var(--ac-type-body);
    color: var(--ac-text-1);
  }
  /* The trend is CONTEXT beside the reading, so it wears the de-emphasis ink
     rather than the accent. Painted in `--role-interactive` at 60px it was the
     same width and the same hue as the meter track beside it, and the row read
     as two bars competing to be the reading. One mark carries the value; the
     other carries where it came from. */
  .channel-sparkline {
    inline-size: 3rem;
    block-size: 1rem;
    flex-shrink: 0;
    direction: ltr;
  }
  .channel-sparkline polyline {
    fill: none;
    stroke: var(--ac-text-4);
    stroke-width: 1.25;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .channel-sparkline-empty {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    color: var(--ac-text-4);
    font-family: var(--ac-font-mono);
    font-size: var(--ac-type-technical);
  }
  .orb-channels .secondary { align-self: flex-start; }

  /* ---- §B action items -------------------------------------------------- */
  .item-list { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; flex-wrap: nowrap; gap: var(--ac-space-2); }
  .item-row { display: flex; flex-direction: column; flex-wrap: nowrap; gap: var(--ac-space-2); min-inline-size: 0; }

  /* SURFACE LEVEL 3 of 3 — the well. A row inside a card is recessed, not
     stacked: these used to carry `--ac-material-base`, the card fill, which put
     a card inside a card five times over. */
  .item-open {
    display: flex;
    align-items: stretch;
    gap: var(--ac-space-4);
    inline-size: 100%;
    min-block-size: var(--ac-target-min);
    padding: var(--ac-space-4) var(--ac-space-4);
    border: 1px solid var(--ac-edge);
    border-radius: var(--ac-radius-lg);
    background: var(--ac-sunken);
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
  .item-title { font-size: var(--ac-type-headline); font-weight: 500; color: var(--ac-text-1); }
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
  .item-value { flex-shrink: 0; font-family: var(--ac-font-mono); font-size: var(--ac-type-body); color: var(--ac-text-1); }
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

  /* ---- §C telemetry tiles -----------------------------------------------
     A cell, not a card: a hairline and the space around it group these four
     readings without adding a surface level inside the card that holds them. */
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
    background: transparent;
  }
  .tile-label { font-size: var(--ac-type-body); color: var(--ac-text-3); }
  .tile-reading { display: flex; flex-wrap: nowrap; align-items: baseline; gap: var(--ac-space-1); min-inline-size: 0; }
  /* Proportional figures, not tabular: these are standalone readings, and
     `tabular-nums` gives every digit a zero's width, which reads loose at
     display size. Tabular is for columns that must line up. */
  .tile-reading strong { font-size: var(--ac-type-title); font-weight: 500; letter-spacing: var(--ac-tracking-display); color: var(--ac-text-1); }
  .tile-reading em { font-family: var(--ac-font-mono); font-size: var(--ac-type-technical); font-style: normal; color: var(--ac-text-3); }
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
  .overview-header-side .primary,
  .overview-header-side .secondary { white-space: nowrap; }

  /* One panel, not a column of a two-column grid. `.grid` splits 1.25fr / .55fr
     for a pair, and the driver panel that used to be the second one is gone.
     `.panel { min-height: 310px }` is a floor for cards that sit BESIDE each
     other, so a pair of them line up. A single full-width panel has nothing to
     line up with, and this one renders 106px of content — measured — inside it.
     Scoped here rather than on `.panel`, which nine other screens rely on. */
  .state-engine { margin-block-start: var(--ac-space-7); }
  .state-engine .panel { min-block-size: auto; }
  /* feature-layout.css sets 17px on `.panel-head h3`, 13px on `.risk` and 13px
     on the `.meta` row — three sizes that exist nowhere in the token scale.
     Scoped here rather than changed globally: nine other screens use `.panel`,
     and this pass is one screen. */
  .state-engine .panel-head h3 { font-size: var(--ac-type-headline); font-weight: 600; }
  /* Same rule for surfaces: `.risk` and `.plan-title` both carry
     `--ac-material-elevated`, a fourth neutral level inside a card. The risk
     chip already states its role in its text colour and border, so its fill
     follows that role; the plan title is a readout inside the panel, so it
     takes the well. */
  .state-engine .risk { font-size: var(--ac-type-technical); background: var(--role-attention-wash); }
  .state-engine .plan-title { background: var(--ac-sunken); }
  .state-engine .plan-title strong { font-size: var(--ac-type-title); }
  .state-engine .plan-title span,
  .state-engine .meta,
  .state-engine .meta span,
  .state-engine .meta code { font-size: var(--ac-type-technical); }
</style>
