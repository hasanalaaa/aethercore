<script lang="ts">
  /**
   * The evidence chip.
   *
   * Every insight the product displays carries the observation it rests on, and
   * the chip expands to that raw observation — provider, sampling, window,
   * counts — not a prettier restatement of the claim.
   *
   * The invariant is enforced by the type, not by discipline: `evidence` is an
   * `Evidence`, never an `Evidence | undefined`. Uncitable insights are dropped
   * before display (see `citedOnly`), so there is no way to render one. Widening
   * this prop to optional is the one change that re-opens that hole.
   *
   * The raw block is monospace and LTR-isolated so a hash, a sensor id or a
   * sampling interval keeps its byte order inside Arabic prose.
   */
  import TechnicalText from '../primitives/TechnicalText.svelte';
  import { t, type Locale } from '../../lib/i18n';
  import type { Evidence } from './contracts';

  /** The observation this claim cites. Required — see the type. */
  export let evidence: Evidence;
  export let locale: Locale;

  let open = false;
  const toggle = (event: MouseEvent): void => {
    event.stopPropagation();
    open = !open;
  };
</script>

<div class="evidence">
  <button
    type="button"
    class="evidence-chip"
    data-evidence-chip
    aria-expanded={open}
    title={open ? t('evidence.hide', locale) : t('evidence.show', locale)}
    onclick={toggle}
  >
    <span class="evidence-icon" aria-hidden="true">
      {#if open}
        <svg viewBox="0 0 24 24" width="12" height="12" fill="none"><path d="m6 14 6-6 6 6"/></svg>
      {:else}
        <svg viewBox="0 0 24 24" width="12" height="12" fill="none"><path d="M9 4H7.5A2.5 2.5 0 0 0 5 6.5v3A2.5 2.5 0 0 1 2.5 12 2.5 2.5 0 0 1 5 14.5v3A2.5 2.5 0 0 0 7.5 20H9M15 4h1.5A2.5 2.5 0 0 1 19 6.5v3a2.5 2.5 0 0 0 2.5 2.5 2.5 2.5 0 0 0-2.5 2.5v3a2.5 2.5 0 0 1-2.5 2.5H15"/></svg>
      {/if}
    </span>
    <span class="evidence-cite">{evidence.cite}</span>
  </button>
  {#if open}
    <div class="evidence-raw">
      <TechnicalText value={evidence.raw} />
    </div>
  {/if}
</div>

<style>
  .evidence { display: flex; flex-direction: column; align-items: flex-start; gap: 0.5rem; min-inline-size: 0; }

  .evidence-chip {
    display: inline-flex;
    align-items: center;
    gap: 0.3125rem;
    max-inline-size: 100%;
    padding: 0.25rem 0.5625rem;
    border-radius: var(--ac-radius-pill);
    border: 1px solid var(--ac-edge);
    /* The control level, shared with `.secondary`. It was `--ac-glass-3`, half a
       step lighter, which bought no distinction and cost the product a fourth
       neutral surface on every screen a chip appears on. */
    background: var(--ac-glass-2);
    color: var(--ac-text-3);
    cursor: pointer;
    transition: border-color var(--ac-feedback-fast) var(--ac-ease-state),
                color var(--ac-feedback-fast) var(--ac-ease-state);
  }
  .evidence-chip:hover { border-color: var(--ac-edge-strong); color: var(--ac-text-2); }

  .evidence-icon { display: grid; place-items: center; flex-shrink: 0; }
  .evidence-icon svg { stroke: currentColor; stroke-width: 1.6; stroke-linecap: round; stroke-linejoin: round; }

  .evidence-cite {
    font-family: var(--ac-font-mono);
    font-size: var(--ac-type-technical);
    letter-spacing: 0.02em;
    min-inline-size: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .evidence-raw {
    inline-size: 100%;
    padding: 0.625rem 0.875rem;
    border-radius: var(--ac-radius-sm);
    border: 1px solid var(--ac-edge);
    background: var(--ac-sunken);
    color: var(--ac-text-3);
    text-wrap: pretty;
  }
  .evidence-raw :global(.technical-isolate) {
    font-family: var(--ac-font-mono);
    font-size: var(--ac-type-technical);
    line-height: 1.65;
    /* The raw observation is the one place long unbroken tokens must wrap rather
       than push the layout wider — a digest is exactly the shape that overflows. */
    overflow-wrap: anywhere;
    white-space: pre-wrap;
    display: block;
  }
</style>
