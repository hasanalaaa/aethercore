<script lang="ts">
  /**
   * The honest empty state.
   *
   * Nothing has been measured yet, and the UI says exactly that. No fake zero, no
   * invented score, no scare copy. A meter at rest reads `—`, because `0` is a
   * real measurement and claiming one that was never taken is the same lie as
   * inventing a percentage.
   *
   * The channels are what the screen is waiting on, named before there is data,
   * so an empty screen still tells the user what a full one will contain.
   */
  import type { EmptyChannel } from './contracts';

  /** Short, mono, uppercase. "Not collected yet", not "No data found". */
  export let title: string;
  /** What will fill this, and what it costs to collect. Never a warning. */
  export let body: string;
  /** The readings this screen is waiting on. Each shows `—` until it has one. */
  export let channels: readonly EmptyChannel[] = [];
  /** Which role the mark takes. Absence of data is not a fault, so the default
      is the neutral text colour rather than attention or critical. */
  export let tone: 'neutral' | 'healthy' = 'neutral';
</script>

<div class="empty-state">
  <div class="empty-state-head">
    <span class="empty-state-mark" data-tone={tone} aria-hidden="true">
      <slot name="icon">
        <svg viewBox="0 0 24 24" width="20" height="20" fill="none"><circle cx="12" cy="12" r="8" stroke-dasharray="2.6 3.2"/></svg>
      </slot>
    </span>
    <div class="empty-state-copy">
      <span class="empty-state-title" data-tone={tone}>{title}</span>
      <p class="empty-state-body">{body}</p>
      <!-- The one action that would end this empty state, when there is one.
           Five of the seven screens with an empty state have exactly one. -->
      <div class="empty-state-action"><slot /></div>
    </div>
  </div>

  {#if channels.length}
    <div class="empty-state-channels">
      {#each channels as channel (channel.label)}
        <div class="empty-state-channel">
          <div class="empty-state-channel-head">
            <span class="empty-state-channel-label">{channel.label}</span>
            <span class="empty-state-channel-pip" aria-hidden="true"></span>
          </div>
          <!-- `undefined` means not collected. A real 0 is a reading and prints as 0. -->
          <span class="empty-state-channel-value">{channel.value ?? '—'}</span>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  /* `main :where(*) { flex-wrap: wrap }` in feature-layout.css is a deliberate
     zero-specificity rule for content ROWS — a control label is not prose, so a
     row wraps rather than squeezes, and a column opts out. This is a column, and
     it never did. Measured at 1280 on `index.html`: with channels it rendered
     496px for 396px of content, on every screen this component appears on.
     DBT-P50-004, found by P50 and left because it is not one screen. */
  .empty-state { display: flex; flex-direction: column; flex-wrap: nowrap; gap: var(--ac-space-6); }
  .empty-state-head { display: flex; align-items: flex-start; gap: var(--ac-space-4); }

  .empty-state-mark {
    display: grid;
    place-items: center;
    inline-size: 2.625rem;
    block-size: 2.625rem;
    flex-shrink: 0;
    border-radius: var(--ac-radius-sm);
    border: 1px solid var(--ac-edge);
    background: var(--ac-glass-2);
    color: var(--ac-text-3);
  }
  .empty-state-mark[data-tone='healthy'] { color: var(--role-healthy); }
  .empty-state-mark :global(svg) { stroke: currentColor; stroke-width: 1.5; stroke-linecap: round; stroke-linejoin: round; fill: none; }

  .empty-state-copy { display: flex; flex-direction: column; gap: 0.375rem; padding-block-start: 0.125rem; min-inline-size: 0; }

  .empty-state-title {
    font-family: var(--ac-font-mono);
    font-size: var(--ac-type-technical);
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--ac-text-3);
  }
  .empty-state-title[data-tone='healthy'] { color: var(--role-healthy); }

  .empty-state-body {
    margin: 0;
    max-inline-size: 58ch;
    font-size: var(--ac-type-callout);
    line-height: 1.65;
    color: var(--ac-text-4);
    text-wrap: pretty;
  }

  .empty-state-action:empty { display: none; }
  .empty-state-action { margin-block-start: var(--ac-space-2); }

  .empty-state-channels {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(10rem, 1fr));
    gap: var(--ac-space-3);
  }

  .empty-state-channel {
    display: flex;
    flex-direction: column;
    gap: var(--ac-space-3);
    padding: 0.875rem 1rem;
    border-radius: var(--ac-radius-md);
    border: 1px solid var(--ac-edge);
    background: var(--ac-glass-2);
  }
  .empty-state-channel-head { display: flex; align-items: center; gap: var(--ac-space-2); }
  .empty-state-channel-label {
    flex: 1;
    min-inline-size: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--ac-type-technical);
    color: var(--ac-text-3);
  }
  .empty-state-channel-pip {
    inline-size: 0.3125rem;
    block-size: 0.3125rem;
    flex-shrink: 0;
    border-radius: var(--ac-radius-pill);
    background: var(--ac-text-4);
    animation: ac-empty-pulse 2.6s ease-in-out infinite;
  }
  .empty-state-channel-value {
    font-family: var(--ac-font-mono);
    font-size: 1.1875rem;
    letter-spacing: -0.02em;
    color: var(--ac-text-4);
  }

  @keyframes ac-empty-pulse { 0%, 100% { opacity: .35 } 50% { opacity: 1 } }
  @media (prefers-reduced-motion: reduce) {
    .empty-state-channel-pip { animation: none; opacity: .7; }
  }
</style>
