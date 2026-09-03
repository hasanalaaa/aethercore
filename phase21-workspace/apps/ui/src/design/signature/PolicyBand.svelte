<script lang="ts">
  /**
   * The persistent policy band.
   *
   * It is always on screen, on every page, whether or not anything has been
   * refused yet. That is the point: a user should learn what the product will
   * not do before they ask it to, not discover it as a failure afterwards.
   *
   * It carries the denied identity into the shell chrome — violet, dashed, 8px —
   * so the first time a real refusal appears in a list it is already familiar.
   */
  import PolicyDenied from './PolicyDenied.svelte';
  import { POLICY_RULES } from '../../lib/policy';
  import { t, td, type Locale } from '../../lib/i18n';

  export let locale: Locale;
</script>

<section class="policy-band" aria-label={t('policy.bandDescription', locale)}>
  <span class="policy-band-mark" aria-hidden="true">
    <svg viewBox="0 0 24 24" width="16" height="16" fill="none"><circle cx="12" cy="12" r="8.25"/><path d="M6.2 6.2l11.6 11.6"/></svg>
  </span>
  <span class="policy-band-label">{t('policy.bandLabel', locale)}</span>
  <span class="policy-band-divider" aria-hidden="true"></span>
  <div class="policy-band-rules">
    {#each POLICY_RULES as rule (rule.id)}
      <PolicyDenied
        denial={{ label: td(rule.labelKey, locale), rule: rule.id }}
        deniedLabel={t('policy.bandLabel', locale)}
      />
    {/each}
  </div>
</section>

<style>
  .policy-band {
    display: flex;
    align-items: center;
    gap: var(--ac-space-2);
    flex-wrap: wrap;
    flex-shrink: 0;
    padding: 0.5625rem 1rem;
    border-radius: var(--ac-radius-pill);
    background: var(--role-denied-band);
    border: 1px solid var(--role-denied-edge);
  }

  .policy-band-mark { display: grid; place-items: center; flex-shrink: 0; color: var(--role-denied); }
  .policy-band-mark svg { stroke: currentColor; stroke-width: 1.8; stroke-linecap: round; }

  .policy-band-label {
    flex-shrink: 0;
    white-space: nowrap;
    color: var(--role-denied);
    font-size: 0.625rem;
    font-weight: 700;
    letter-spacing: 0.12em;
    text-transform: uppercase;
  }

  .policy-band-divider {
    inline-size: 1px;
    align-self: stretch;
    flex-shrink: 0;
    background: var(--role-denied-edge);
  }

  .policy-band-rules { display: flex; align-items: center; gap: var(--ac-space-2); flex-wrap: wrap; }

  /* Below the point where three chips and the label can share a line, the band
     stacks rather than pushing the label off the row. It never scrolls
     horizontally and it is never hidden — a policy the user cannot see is a
     policy that will surprise them. */
  @media (max-width: 60rem) {
    .policy-band { border-radius: var(--ac-radius-md); }
  }
</style>
