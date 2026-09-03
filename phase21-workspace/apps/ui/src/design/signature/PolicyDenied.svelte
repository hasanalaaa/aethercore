<script lang="ts">
  /**
   * DENIED BY POLICY — the single most important decision in the design system.
   *
   * A refusal is the product keeping its promise, not a failure. So this element
   * shares nothing with error styling, and it disagrees with the rest of the
   * product on purpose, in four ways at once:
   *
   *   colour  violet #AD4EBC — 64 deg from interactive on the wheel, and never in
   *           the critical/red family.
   *   border  1.5px DASHED. Every other border in the product is solid.
   *   radius  8px. Every other chip, tag and pill in the product is 999px.
   *   icon    the interdiction ring, used nowhere else in the app.
   *
   * All four have to agree before something reads as a policy refusal, which is
   * what keeps a refusal legible to someone who cannot see the hue at all.
   *
   * The rule is required and always shown. A refusal that will not say what
   * refused it is just an error wearing a different colour.
   */
  import TechnicalText from '../primitives/TechnicalText.svelte';
  import type { PolicyDenial } from './contracts';

  /** What was refused. Required — see the type. */
  export let denial: PolicyDenial;
  /** `chip` for the policy band and row tags; `row` for a full list entry. */
  export let variant: 'chip' | 'row' = 'chip';
  /** Announced label for the refusal, e.g. "Denied by policy". */
  export let deniedLabel: string;
  /** Longer explanation, shown only by the row form. */
  export let detail = '';
</script>

{#if variant === 'row'}
  <div class="policy-denied-row" role="group" aria-label={`${deniedLabel}: ${denial.label}`}>
    <span class="policy-denied-mark" aria-hidden="true">
      <svg viewBox="0 0 24 24" width="18" height="18" fill="none"><circle cx="12" cy="12" r="8.25"/><path d="M6.2 6.2l11.6 11.6"/></svg>
    </span>
    <div class="policy-denied-body">
      <span class="policy-denied-title">{denial.label}</span>
      {#if detail}<span class="policy-denied-detail">{detail}</span>{/if}
      <span class="policy-denied-rule">
        <TechnicalText value={denial.rule} />
      </span>
    </div>
    <span class="policy-denied-tag">{deniedLabel}</span>
  </div>
{:else}
  <span class="policy-denied-chip" title={`${deniedLabel}: ${denial.rule}`}>
    <span class="policy-denied-chip-label">{denial.label}</span>
    <TechnicalText value={denial.rule} />
  </span>
{/if}

<style>
  /* 8px, dashed, violet. The three geometric signals, in one place. */
  .policy-denied-chip,
  .policy-denied-tag {
    display: inline-flex;
    align-items: center;
    gap: 0.4375rem;
    border-radius: var(--ac-radius-denied);
    border: 1.5px dashed var(--role-denied-dash);
    background: var(--role-denied-wash);
    white-space: nowrap;
  }

  .policy-denied-chip { padding: 0.25rem 0.75rem; }
  .policy-denied-chip-label { color: var(--ac-text-1); font-size: var(--ac-type-technical); }
  .policy-denied-chip :global(.technical-isolate) {
    color: var(--role-denied);
    font-family: var(--ac-font-mono);
    font-size: 0.59375rem;
  }

  .policy-denied-row {
    display: flex;
    align-items: center;
    gap: 0.875rem;
    padding: 0.875rem 1rem;
    border-radius: var(--ac-radius-lg);
    background: var(--role-denied-wash);
    border: 1.5px dashed var(--role-denied-dash);
  }

  .policy-denied-mark {
    display: grid;
    place-items: center;
    inline-size: 2.125rem;
    block-size: 2.125rem;
    flex-shrink: 0;
    border-radius: var(--ac-radius-denied);
    background: var(--role-denied-wash);
    color: var(--role-denied);
  }
  .policy-denied-mark svg { stroke: currentColor; stroke-width: 1.65; stroke-linecap: round; }

  .policy-denied-body { display: flex; flex-direction: column; gap: 0.25rem; min-inline-size: 0; flex: 1; }
  .policy-denied-title { font-size: var(--ac-type-body); font-weight: 500; color: var(--ac-text-1); }
  .policy-denied-detail { font-size: var(--ac-type-caption); color: var(--ac-text-3); line-height: 1.55; text-wrap: pretty; }
  .policy-denied-rule :global(.technical-isolate) {
    color: var(--role-denied);
    font-family: var(--ac-font-mono);
    font-size: var(--ac-type-technical);
  }

  .policy-denied-tag {
    padding: 0.3125rem 0.6875rem;
    flex-shrink: 0;
    color: var(--role-denied);
    font-size: 0.65625rem;
    font-weight: 600;
    letter-spacing: var(--ac-tracking-tag);
    text-transform: uppercase;
  }

  /* When the hue is taken away entirely, the dash and the 8px corner are what
     still separate a refusal from a fault. */
  @media (forced-colors: active) {
    .policy-denied-chip,
    .policy-denied-tag,
    .policy-denied-row { border-style: dashed; border-color: LinkText; }
  }
</style>
