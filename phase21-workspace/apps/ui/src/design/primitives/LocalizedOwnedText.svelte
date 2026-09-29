<script lang="ts">
  import type { Locale } from '../../lib/i18n';
  import { localizeOwnedText, segmentBidiEvidence } from '../../lib/i18n';
  import TechnicalText from './TechnicalText.svelte';

  export let value = '';
  export let locale: Locale = 'en';
  export let as: 'span' | 'p' | 'small' | 'strong' = 'span';
  // For a value the user's machine wrote (a command, a path): unknown text is shown isolated as
  // data. Prose leaves this false, and unknown prose is replaced, never shown (P77-01).
  export let data = false;

  $: localized = localizeOwnedText(value, locale, { data });
  $: segments = localized.localized ? segmentBidiEvidence(localized.text) : [];
</script>

{#snippet content()}
  {#if localized.localized}
    {#each segments as segment}
      {#if segment.technical}<TechnicalText value={segment.text}/>{:else}{segment.text}{/if}
    {/each}
  {:else}
    <TechnicalText value={localized.text}/>
  {/if}
{/snippet}

{#if as === 'p'}
  <p>{@render content()}</p>
{:else if as === 'small'}
  <small>{@render content()}</small>
{:else if as === 'strong'}
  <strong>{@render content()}</strong>
{:else}
  <span>{@render content()}</span>
{/if}
